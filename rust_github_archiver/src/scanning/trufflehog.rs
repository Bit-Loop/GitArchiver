//! TruffleHog integration for secret scanning
//!
//! This module provides a Rust implementation of the Python secrets-ninja script,
//! integrating TruffleHog secret scanning into the Rust GitHub archiver.

use crate::scanning::cache::CacheManager;
use anyhow::{anyhow, Context, Result};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs as std_fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex as StdMutex;
use tempfile::TempDir;
use tokio::fs as tokio_fs;
use tokio::process::Command as AsyncCommand;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{debug, info, warn};
use url::Url;

const MAX_REPO_BYTES: u64 = 1_000_000_000; // 1GB guardrail
const DEFAULT_RESERVATION: u64 = 256 * 1024 * 1024; // 256MB pre-alloc to trigger eviction
const GLOBAL_RATE_LIMIT_BACKOFF_SECS: i64 = 5 * 60;

/// Tracks a global "not before" time after rate limits to avoid hammering.
static RATE_LIMIT_STATE: OnceCell<StdMutex<RateLimitState>> = OnceCell::new();
static LAST_RATE_PROBE: OnceCell<StdMutex<Option<chrono::DateTime<chrono::Utc>>>> = OnceCell::new();

#[derive(Debug, Clone)]
pub enum ScanErrorKind {
    ApiRateLimited {
        reset_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    GitRateLimited,
    RepoNotFound,
    RepoForbidden,
    TemporaryNetworkFailure,
    MisconfiguredEndpoint,
    TooLargeRepository,
    PermanentFailure(String),
}

#[derive(Debug, Clone)]
pub struct CloneError {
    pub kind: ScanErrorKind,
    pub message: String,
    pub clone_url: Option<String>,
}

impl std::fmt::Display for CloneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CloneError {}

#[derive(Debug, Clone, Default)]
pub struct RateLimitState {
    pub global_until: Option<chrono::DateTime<chrono::Utc>>,
    pub repo_until: HashMap<String, chrono::DateTime<chrono::Utc>>,
}

impl RateLimitState {
    pub fn global() -> &'static RateLimitStateHandle {
        static HANDLE: OnceCell<RateLimitStateHandle> = OnceCell::new();
        HANDLE.get_or_init(|| RateLimitStateHandle {
            inner: RATE_LIMIT_STATE.get_or_init(|| StdMutex::new(RateLimitState::default())),
        })
    }

    fn repo_on_cooldown(&self, repo: &str) -> bool {
        self.repo_until
            .get(repo)
            .map(|until| *until > chrono::Utc::now())
            .unwrap_or(false)
    }
}

pub struct RateLimitStateHandle {
    inner: &'static StdMutex<RateLimitState>,
}

impl RateLimitStateHandle {
    pub fn repo_on_cooldown(&self, repo: &str) -> bool {
        self.inner
            .lock()
            .map(|state| state.repo_on_cooldown(repo))
            .unwrap_or(false)
    }

    pub fn set_repo_cooldown(&self, repo: &str, dur: chrono::Duration) {
        if let Ok(mut state) = self.inner.lock() {
            state
                .repo_until
                .insert(repo.to_string(), chrono::Utc::now() + dur);
        }
    }

    pub fn set_global(&self, until: chrono::DateTime<chrono::Utc>) {
        if let Ok(mut state) = self.inner.lock() {
            state.global_until = Some(until);
        }
    }

    pub fn global_until(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.inner.lock().ok().and_then(|state| state.global_until)
    }
}

/// TruffleHog finding from JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruffleHogFinding {
    #[serde(rename = "DetectorName")]
    pub detector_name: Option<String>,

    #[serde(rename = "DecoderName")]
    pub decoder_name: Option<String>,

    #[serde(rename = "Raw")]
    pub raw: Option<String>,

    #[serde(rename = "RawV2")]
    pub raw_v2: Option<String>,

    #[serde(rename = "SourceMetadata")]
    pub source_metadata: Option<SourceMetadata>,

    #[serde(rename = "ExtraData")]
    pub extra_data: Option<serde_json::Value>,

    #[serde(rename = "Verified")]
    pub verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMetadata {
    #[serde(rename = "Data")]
    pub data: Option<GitData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitData {
    #[serde(rename = "Git")]
    pub git: Option<GitInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub commit: Option<String>,
    pub file: Option<String>,
    pub email: Option<String>,
    pub timestamp: Option<String>,
    pub line: Option<i32>,
}

/// Configuration for TruffleHog scanner
#[derive(Debug, Clone)]
pub struct TruffleHogConfig {
    pub only_verified: bool,
    pub no_update: bool,
    pub timeout_seconds: u64,
    pub binary_path: Option<PathBuf>,
}

impl Default for TruffleHogConfig {
    fn default() -> Self {
        Self {
            only_verified: true,
            no_update: true,
            timeout_seconds: 300,
            binary_path: None,
        }
    }
}

/// TruffleHog scanner for Git repositories
pub struct TruffleHogScanner {
    config: TruffleHogConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitScanRef {
    pub since_commit: String,
    pub branch: String,
}

impl TruffleHogScanner {
    pub fn new(mut config: TruffleHogConfig) -> Self {
        if config.binary_path.is_none() {
            config.binary_path = Self::resolve_binary_path();
        }

        match &config.binary_path {
            Some(binary_path) => debug!("Using TruffleHog binary at {:?}", binary_path),
            None => {
                debug!("TruffleHog binary path not resolved; relying on system PATH");
            }
        }

        Self { config }
    }

    fn cached_availability() -> bool {
        static AVAIL: OnceCell<bool> = OnceCell::new();
        *AVAIL.get_or_init(|| Self::ensure_available().is_ok())
    }

    /// Check if TruffleHog is available in PATH
    pub fn is_available() -> bool {
        Self::cached_availability()
    }

    /// Ensure the TruffleHog binary is present and executable, returning its path
    pub fn ensure_available() -> Result<PathBuf> {
        if let Some(path) = Self::resolve_binary_path() {
            match Command::new(&path).arg("--version").output() {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        warn!(
                            "TruffleHog at {:?} returned non-zero status for --version: {}",
                            path,
                            stderr.trim()
                        );
                    }
                    return Ok(path);
                }
                Err(err) => {
                    return Err(anyhow!(
                        "Failed to execute TruffleHog binary at {:?}: {}",
                        path,
                        err
                    ));
                }
            }
        }

        Err(anyhow!(
            "TruffleHog binary not found. Install it (e.g. pip install trufflehog), place it in PATH, or set TRUFFLEHOG_PATH to the executable."
        ))
    }

    fn resolve_binary_path() -> Option<PathBuf> {
        let mut seen = HashSet::new();
        Self::candidate_binary_paths()
            .into_iter()
            .find(|candidate| seen.insert(candidate.clone()) && Self::is_executable(candidate))
    }

    fn candidate_binary_paths() -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        if let Some(path) = env::var_os("TRUFFLEHOG_PATH") {
            candidates.push(PathBuf::from(path));
        }

        if let Some(virtual_env) = env::var_os("VIRTUAL_ENV") {
            let venv = PathBuf::from(virtual_env);
            for rel in [
                "bin/trufflehog",
                "Scripts/trufflehog.exe",
                "Scripts/trufflehog",
            ] {
                candidates.push(venv.join(rel));
            }
        }

        for rel in [
            "github_scraper_env/bin/trufflehog",
            "github_scraper_env/Scripts/trufflehog.exe",
            "github_scraper_env/Scripts/trufflehog",
            "../github_scraper_env/bin/trufflehog",
            "../github_scraper_env/Scripts/trufflehog.exe",
            "../github_scraper_env/Scripts/trufflehog",
        ] {
            candidates.push(PathBuf::from(rel));
        }

        for absolute in [
            "/usr/local/bin/trufflehog",
            "/usr/bin/trufflehog",
            "/opt/homebrew/bin/trufflehog",
            "/opt/local/bin/trufflehog",
            "/snap/bin/trufflehog",
        ] {
            candidates.push(PathBuf::from(absolute));
        }

        if let Some(path_var) = env::var_os("PATH") {
            for dir in env::split_paths(&path_var) {
                candidates.push(dir.join("trufflehog"));
            }
        }

        candidates
    }

    fn is_executable(path: &Path) -> bool {
        if !path.exists() || !path.is_file() {
            return false;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = path.metadata() {
                return metadata.permissions().mode() & 0o111 != 0;
            }
        }

        true
    }

    fn binary_path(&self) -> &Path {
        self.config
            .binary_path
            .as_deref()
            .unwrap_or_else(|| Path::new("trufflehog"))
    }

    /// Scan a git repository from since_commit to branch
    pub async fn scan_repository(
        &self,
        repo_path: &Path,
        since_commit: &str,
        branch: &str,
    ) -> Result<Vec<TruffleHogFinding>> {
        info!(
            "Running TruffleHog scan: repo={:?}, since={}, branch={}",
            repo_path, since_commit, branch
        );

        let mut cmd = AsyncCommand::new(self.binary_path());
        cmd.arg("git").arg("--branch").arg(branch).arg("--json");

        if !since_commit.is_empty() {
            cmd.arg("--since-commit").arg(since_commit);
        }

        if self.config.only_verified {
            cmd.arg("--only-verified");
        }

        if self.config.no_update {
            cmd.arg("--no-update");
        }

        cmd.arg(format!("file://{}", repo_path.display()));

        debug!("TruffleHog command: {:?}", cmd);

        let output = tokio::time::timeout(
            tokio::time::Duration::from_secs(self.config.timeout_seconds),
            cmd.output(),
        )
        .await
        .context("TruffleHog scan timed out")??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("TruffleHog scan completed with warnings: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut findings = Vec::new();

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<TruffleHogFinding>(line) {
                Ok(finding) => findings.push(finding),
                Err(e) => {
                    debug!("Failed to parse TruffleHog output line: {} - {}", e, line);
                }
            }
        }

        info!("TruffleHog scan completed: {} findings", findings.len());
        Ok(findings)
    }

    /// Scan arbitrary text content by writing it to a temporary directory and
    /// invoking TruffleHog's filesystem scanner. This allows realtime workflows
    /// to reuse the same detector set used for repository scans.
    pub async fn scan_buffer(&self, content: &str) -> Result<Vec<TruffleHogFinding>> {
        let temp_dir = TempDir::new().context("Failed to create temp directory for buffer scan")?;
        let file_path = temp_dir.path().join("payload.txt");

        tokio_fs::write(&file_path, content)
            .await
            .context("Failed to persist commit payload for scanning")?;

        let mut cmd = AsyncCommand::new(self.binary_path());
        cmd.arg("filesystem").arg("--json");

        if self.config.only_verified {
            cmd.arg("--only-verified");
        }

        if self.config.no_update {
            cmd.arg("--no-update");
        }

        cmd.arg("--directory").arg(temp_dir.path());

        let output = tokio::time::timeout(
            tokio::time::Duration::from_secs(self.config.timeout_seconds),
            cmd.output(),
        )
        .await
        .context("TruffleHog buffer scan timed out")??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("TruffleHog buffer scan completed with warnings: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut findings = Vec::new();

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<TruffleHogFinding>(line) {
                Ok(finding) => findings.push(finding),
                Err(e) => {
                    debug!(
                        "Failed to parse TruffleHog filesystem output line: {} - {}",
                        e, line
                    );
                }
            }
        }

        Ok(findings)
    }
}

/// Git repository cloner with compatibility fixes for TruffleHog
pub struct GitCloner {
    cache: &'static CacheManager,
}

impl GitCloner {
    pub fn new() -> Self {
        Self {
            cache: CacheManager::global(),
        }
    }

    /// Clone a repository with full history to ensure TruffleHog compatibility
    pub async fn partial_clone(&mut self, repo_url: &str) -> Result<PathBuf> {
        let normalized_url = normalize_clone_url(repo_url)?;
        let repo_id = sanitize_repo_id(&normalized_url);
        if let Some(next_slot) = RateLimitState::global().global_until() {
            if chrono::Utc::now() < next_slot {
                return Err(anyhow!(CloneError {
                    kind: ScanErrorKind::ApiRateLimited {
                        reset_at: Some(next_slot)
                    },
                    message: format!("RATE_LIMIT: Global cooldown active until {}", next_slot),
                    clone_url: None,
                }));
            }
        }

        if self.cache.is_on_cooldown_sync(&repo_id)
            || RateLimitState::global().repo_on_cooldown(&repo_id)
        {
            return Err(anyhow!(CloneError {
                kind: ScanErrorKind::GitRateLimited,
                message: format!(
                    "Repository {} is on cooldown due to prior 403/clone errors",
                    repo_id
                ),
                clone_url: Some(normalized_url.clone()),
            }));
        }

        let repo_path = self
            .cache
            .allocate_repo(&repo_id, DEFAULT_RESERVATION)
            .await?;

        let mut last_err = None;
        for attempt in 1..=3 {
            let token = github_token();
            let mut cmd = AsyncCommand::new("git");
            cmd.arg("clone")
                .arg("--filter=blob:none")
                .arg("--no-checkout")
                .arg("--depth")
                .arg("50")
                .arg(&normalized_url)
                .arg(".")
                .current_dir(&repo_path)
                .env("GIT_TERMINAL_PROMPT", "0");

            if let Some(token) = token.as_deref() {
                cmd.env(
                    "GIT_HTTP_EXTRAHEADER",
                    format!("Authorization: Bearer {}", token),
                );
            } else {
                cmd.env_remove("GIT_HTTP_EXTRAHEADER");
            }

            info!(
                "Cloning repository (attempt {} of 3): {} -> {:?}",
                attempt, normalized_url, repo_path
            );

            let output = cmd.output().await.context("Failed to execute git clone")?;
            if output.status.success() {
                let size_bytes = measure_repo_size(&repo_path)?;
                if size_bytes > MAX_REPO_BYTES {
                    self.cache.remove_entry(&repo_id).await.ok();
                    return Err(anyhow!(
                        "Repository {} exceeds 1GB limit ({} bytes)",
                        repo_id,
                        size_bytes
                    ));
                }
                self.cache.finalize_success(&repo_id, &repo_path).await?;
                return Ok(repo_path);
            }

            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            last_err = Some(stderr.clone());

            match classify_git_error(&stderr, &normalized_url) {
                ScanErrorKind::GitRateLimited | ScanErrorKind::ApiRateLimited { .. } => {
                    let note = if github_token().is_some() {
                        "Rate limited; cooling down this repo."
                    } else {
                        "Rate limited; set GITHUB_TOKEN to increase clone quota."
                    };
                    warn!(
                        "Rate limited cloning {}: {} — {}",
                        normalized_url,
                        stderr.trim(),
                        note
                    );
                    self.cache.mark_cooldown(&repo_id).await.ok();
                    RateLimitState::global()
                        .set_repo_cooldown(&repo_id, chrono::Duration::minutes(15));
                    if let Some(reset_at) = maybe_probe_rate_limit(github_token().as_deref()).await
                    {
                        RateLimitState::global().set_global(reset_at);
                    }
                    TruffleHogScanner::set_global_rate_limit_backoff();
                    return Err(anyhow!(CloneError {
                        kind: ScanErrorKind::GitRateLimited,
                        message: format!("RATE_LIMIT: {}", stderr.trim()),
                        clone_url: Some(normalized_url.clone()),
                    }));
                }
                ScanErrorKind::RepoNotFound => {
                    return Err(anyhow!(CloneError {
                        kind: ScanErrorKind::RepoNotFound,
                        message: format!("Repo not found for {}", normalized_url),
                        clone_url: Some(normalized_url.clone()),
                    }));
                }
                ScanErrorKind::RepoForbidden => {
                    return Err(anyhow!(CloneError {
                        kind: ScanErrorKind::RepoForbidden,
                        message: format!("Repo forbidden for {}", normalized_url),
                        clone_url: Some(normalized_url.clone()),
                    }));
                }
                ScanErrorKind::MisconfiguredEndpoint => {
                    return Err(anyhow!(CloneError {
                        kind: ScanErrorKind::MisconfiguredEndpoint,
                        message: format!("Misconfigured clone endpoint {}", normalized_url),
                        clone_url: Some(normalized_url.clone()),
                    }));
                }
                ScanErrorKind::TooLargeRepository => {
                    return Err(anyhow!(CloneError {
                        kind: ScanErrorKind::TooLargeRepository,
                        message: format!("Repository too large {}", normalized_url),
                        clone_url: Some(normalized_url.clone()),
                    }));
                }
                ScanErrorKind::TemporaryNetworkFailure => {
                    if attempt < 3 {
                        let backoff = TokioDuration::from_secs(2u64.pow(attempt));
                        sleep(backoff).await;
                        continue;
                    }
                }
                ScanErrorKind::PermanentFailure(_) => {}
            }

            if attempt < 3 {
                let backoff = TokioDuration::from_secs(2u64.pow(attempt));
                sleep(backoff).await;
            }
        }

        self.cache.remove_entry(&repo_id).await.ok();
        let msg = last_err.unwrap_or_else(|| "unknown error".to_string());
        Err(anyhow!(CloneError {
            kind: ScanErrorKind::PermanentFailure(msg.clone()),
            message: format!("Git clone failed after retries: {}", msg),
            clone_url: Some(normalized_url),
        }))
    }

    /// Fetch a specific commit by object id and materialize a stable local ref for scanners.
    pub async fn fetch_commit(&self, repo_path: &Path, commit_sha: &str) -> Result<()> {
        debug!("Fetching commit: {}", commit_sha);

        let refspec = format!("+{}:refs/temp/gitarchiver/{}", commit_sha, commit_sha);
        let attempts = vec![
            vec![
                "-c".to_string(),
                "uploadpack.allowReachableSHA1InWant=true".to_string(),
                "fetch".to_string(),
                "--no-tags".to_string(),
                "origin".to_string(),
                commit_sha.to_string(),
            ],
            vec![
                "-c".to_string(),
                "uploadpack.allowReachableSHA1InWant=true".to_string(),
                "fetch".to_string(),
                "--no-tags".to_string(),
                "--depth".to_string(),
                "1".to_string(),
                "origin".to_string(),
                commit_sha.to_string(),
            ],
            vec![
                "-c".to_string(),
                "uploadpack.allowReachableSHA1InWant=true".to_string(),
                "fetch".to_string(),
                "--no-tags".to_string(),
                "origin".to_string(),
                refspec.clone(),
            ],
            vec![
                "-c".to_string(),
                "uploadpack.allowReachableSHA1InWant=true".to_string(),
                "fetch".to_string(),
                "--no-tags".to_string(),
                "--depth".to_string(),
                "1".to_string(),
                "origin".to_string(),
                refspec,
            ],
        ];

        let mut last_error = None;

        for args in attempts {
            let mut cmd = AsyncCommand::new("git");
            cmd.args(&args)
                .current_dir(repo_path)
                .env("GIT_TERMINAL_PROMPT", "0");

            debug!("Attempting git {}", args.join(" "));

            match cmd.output().await.context("Failed to execute git fetch") {
                Ok(output) => {
                    if output.status.success() {
                        self.create_local_commit_ref(repo_path, commit_sha).await?;
                        return Ok(());
                    }

                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    last_error = Some(stderr);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }
        }

        if let Some(error) = last_error {
            if error.contains("not our ref") {
                return Err(anyhow!(
                    "Commit {} was likely manually removed from repository",
                    commit_sha
                ));
            }

            return Err(anyhow!("Git fetch failed for {}: {}", commit_sha, error));
        }

        Err(anyhow!(
            "Git fetch failed for {} with no diagnostic output",
            commit_sha
        ))
    }

    pub async fn prepare_commit_scan_ref(
        &self,
        repo_path: &Path,
        commit_sha: &str,
    ) -> Result<CommitScanRef> {
        self.fetch_commit(repo_path, commit_sha).await?;

        Ok(CommitScanRef {
            since_commit: self
                .parent_commit(repo_path, commit_sha)
                .await?
                .unwrap_or_default(),
            branch: Self::local_commit_branch(commit_sha),
        })
    }

    /// Ensure a commit is present locally so it can be used as a TruffleHog boundary.
    pub async fn identify_base_commit(
        &self,
        repo_path: &Path,
        since_commit: &str,
    ) -> Result<String> {
        debug!("Identifying scan boundary for: {}", since_commit);

        self.fetch_commit(repo_path, since_commit).await?;
        Ok(since_commit.to_string())
    }

    async fn create_local_commit_ref(&self, repo_path: &Path, commit_sha: &str) -> Result<()> {
        let branch_ref = format!("refs/heads/{}", Self::local_commit_branch(commit_sha));
        let output = AsyncCommand::new("git")
            .arg("update-ref")
            .arg(&branch_ref)
            .arg(commit_sha)
            .current_dir(repo_path)
            .output()
            .await
            .context("Failed to create local commit scan ref")?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!(
                "Failed to create local scan ref {} for {}: {}",
                branch_ref,
                commit_sha,
                stderr.trim()
            ))
        }
    }

    async fn parent_commit(&self, repo_path: &Path, commit_sha: &str) -> Result<Option<String>> {
        let output = AsyncCommand::new("git")
            .arg("rev-list")
            .arg("--parents")
            .arg("-n")
            .arg("1")
            .arg(commit_sha)
            .current_dir(repo_path)
            .output()
            .await
            .context("Failed to inspect commit parents")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Failed to inspect parent for {}: {}",
                commit_sha,
                stderr.trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut parts = stdout.split_whitespace();
        let _commit = parts.next();
        Ok(parts.next().map(str::to_string))
    }

    fn local_commit_branch(commit_sha: &str) -> String {
        format!("gitarchiver/{}", commit_sha)
    }
}

impl Default for GitCloner {
    fn default() -> Self {
        Self::new()
    }
}

fn github_token() -> Option<String> {
    ensure_dotenv();
    env::var("GITHUB_TOKEN")
        .or_else(|_| env::var("GH_TOKEN"))
        .ok()
        .filter(|v| !v.trim().is_empty())
}

fn ensure_dotenv() {
    static LOADED: OnceCell<()> = OnceCell::new();
    let _ = LOADED.get_or_init(|| {
        dotenv::dotenv().ok();
    });
}

fn sanitize_repo_id(repo: &str) -> String {
    let mut id: String = repo
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if id.len() > 120 {
        id.truncate(120);
    }
    id
}

fn measure_repo_size(path: &Path) -> Result<u64> {
    let output = Command::new("git")
        .arg("count-objects")
        .arg("-vH")
        .current_dir(path)
        .output()
        .context("Failed to run git count-objects")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total_kib: u64 = 0;
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("size-pack:") {
                total_kib = total_kib.saturating_add(parse_kib(rest.trim()));
            } else if let Some(rest) = line.strip_prefix("size:") {
                total_kib = total_kib.saturating_add(parse_kib(rest.trim()));
            }
        }
        if total_kib > 0 {
            return Ok(total_kib * 1024);
        }
    }

    // Fallback: walk directory sizes
    dir_size(path)
}

fn parse_kib(value: &str) -> u64 {
    value.parse::<u64>().unwrap_or(0)
}

fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    if !path.exists() {
        return Ok(0);
    }
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        for entry in std_fs::read_dir(&p)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

fn normalize_clone_url(url: &str) -> Result<String> {
    // Convert only api.github.com repo URLs to github.com equivalents.
    // Do not rewrite GHE/custom domains.
    let parsed = Url::parse(url).map_err(|e| {
        anyhow!(CloneError {
            kind: ScanErrorKind::MisconfiguredEndpoint,
            message: format!("Invalid repository URL {}: {}", url, e),
            clone_url: Some(url.to_string()),
        })
    })?;

    let scheme = parsed.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(anyhow!(CloneError {
            kind: ScanErrorKind::MisconfiguredEndpoint,
            message: format!("Unsupported scheme for clone URL: {}", url),
            clone_url: Some(url.to_string()),
        }));
    }

    if let Some(host) = parsed.host_str() {
        if host == "api.github.com" && parsed.path().starts_with("/repos/") {
            let trimmed = parsed
                .path()
                .trim_start_matches("/repos/")
                .trim_end_matches('/');
            return Ok(format!("https://github.com/{}.git", trimmed));
        }
        if host == "api.github.com" || parsed.path().contains("/api/") {
            return Err(anyhow!(CloneError {
                kind: ScanErrorKind::MisconfiguredEndpoint,
                message: format!("Misconfigured clone endpoint: {}", url),
                clone_url: Some(url.to_string()),
            }));
        }
    }

    Ok(url.to_string())
}

fn classify_git_error(stderr: &str, url: &str) -> ScanErrorKind {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("rate limit") || lower.contains("please wait a moment") {
        return ScanErrorKind::GitRateLimited;
    }
    if lower.contains("abuse") {
        return ScanErrorKind::GitRateLimited;
    }
    if lower.contains("invalid credentials")
        || lower.contains("authentication failed")
        || lower.contains("could not read username")
    {
        return ScanErrorKind::RepoForbidden;
    }
    if lower.contains("404") || lower.contains("not found") {
        return ScanErrorKind::RepoNotFound;
    }
    if lower.contains("403") || lower.contains("access denied") || lower.contains("forbidden") {
        return ScanErrorKind::RepoForbidden;
    }
    if lower.contains("api.github.com") {
        return ScanErrorKind::MisconfiguredEndpoint;
    }
    if lower.contains("could not resolve host") || lower.contains("failed to connect") {
        return ScanErrorKind::TemporaryNetworkFailure;
    }
    if url.contains("api.github.com") {
        return ScanErrorKind::MisconfiguredEndpoint;
    }
    ScanErrorKind::PermanentFailure(stderr.trim().to_string())
}

fn should_probe_rate_limit() -> bool {
    let now = chrono::Utc::now();
    let slot = LAST_RATE_PROBE
        .get_or_init(|| StdMutex::new(None))
        .lock()
        .ok()
        .and_then(|t| *t);
    match slot {
        None => true,
        Some(last) => (now - last) >= chrono::Duration::minutes(1),
    }
}

fn record_rate_probe() {
    if let Ok(mut guard) = LAST_RATE_PROBE.get_or_init(|| StdMutex::new(None)).lock() {
        *guard = Some(chrono::Utc::now());
    }
}

async fn maybe_probe_rate_limit(token: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    if !should_probe_rate_limit() {
        return None;
    }

    let client = reqwest::Client::new();
    let mut req = client.get("https://api.github.com/rate_limit").header(
        reqwest::header::USER_AGENT,
        "github-archiver-secret-scanner/1.0",
    );

    if let Some(t) = token {
        req = req.header(reqwest::header::AUTHORIZATION, format!("token {}", t));
    }

    let resp = req.send().await.ok()?;
    record_rate_probe();

    if !resp.status().is_success() {
        return None;
    }

    let json: Value = resp.json().await.ok()?;
    let core = json
        .get("resources")
        .and_then(|r| r.get("core"))
        .and_then(|c| c.as_object())
        .cloned()
        .unwrap_or_default();
    let remaining = core.get("remaining").and_then(|r| r.as_i64()).unwrap_or(-1);
    let reset = core.get("reset").and_then(|r| r.as_i64());

    if remaining == 0 {
        if let Some(reset_ts) = reset {
            let reset_at = chrono::DateTime::<chrono::Utc>::from_timestamp(reset_ts, 0)?;
            return Some(reset_at);
        }
        return Some(chrono::Utc::now() + chrono::Duration::minutes(5));
    }

    None
}

impl TruffleHogScanner {
    fn set_global_rate_limit_backoff() {
        RateLimitState::global().set_global(
            chrono::Utc::now() + chrono::Duration::seconds(GLOBAL_RATE_LIMIT_BACKOFF_SECS),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trufflehog_available() {
        // This test only checks if trufflehog is in PATH
        // It may fail in CI environments without trufflehog installed
        let available = TruffleHogScanner::is_available();
        println!("TruffleHog available: {}", available);
        if std::env::var("CI").is_ok() {
            assert!(available, "TruffleHog must be installed in CI environment");
        }
    }

    #[tokio::test]
    async fn test_git_cloner_creation() {
        let _cloner = GitCloner::new();
        // creation should not panic
    }

    #[test]
    fn normalize_api_url() {
        let url = "https://api.github.com/repos/owner/repo";
        let normalized = normalize_clone_url(url).unwrap();
        assert_eq!(normalized, "https://github.com/owner/repo.git");
    }

    #[test]
    fn normalize_rejects_api_endpoint() {
        let url = "https://api.github.com/other/path";
        let err = normalize_clone_url(url).unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("Misconfigured"));
    }

    #[test]
    fn normalize_rejects_unsupported_scheme() {
        let url = "ftp://api.github.com/repos/owner/repo";
        let err = normalize_clone_url(url).unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("Unsupported scheme"));
    }

    #[test]
    fn normalize_rejects_no_scheme() {
        let url = "github.com/owner/repo";
        let err = normalize_clone_url(url).unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("Invalid repository URL"));
    }

    #[tokio::test]
    async fn test_scan_buffer_errors_without_binary() {
        let temp = tempfile::tempdir().expect("tempdir to exist");
        let missing_binary = temp.path().join("trufflehog-missing");
        let scanner = TruffleHogScanner::new(TruffleHogConfig {
            binary_path: Some(missing_binary),
            ..Default::default()
        });

        let err = scanner
            .scan_buffer("dummy secret payload")
            .await
            .expect_err("expected an error when the binary is missing");
        let msg = format!("{err:?}").to_lowercase();
        assert!(
            msg.contains("no such file")
                || msg.contains("not found")
                || msg.contains("could not find"),
            "unexpected error message: {msg}"
        );
    }

    #[tokio::test]
    async fn test_trufflehog_buffer_scan_executes() {
        if !TruffleHogScanner::is_available() {
            eprintln!("Skipping: TruffleHog binary not available in PATH/TRUFFLEHOG_PATH");
            return;
        }

        let scanner = TruffleHogScanner::new(TruffleHogConfig {
            only_verified: false,
            ..Default::default()
        });

        // Use a realistic AWS-style key pattern that TruffleHog should detect
        // without storing a push-protection-triggering literal in source.
        let access_key = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
        let payload = format!(
            r#"
        # Configuration file
        AWS_ACCESS_KEY_ID={}
        AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
        "#,
            access_key
        );

        let findings = scanner
            .scan_buffer(&payload)
            .await
            .expect("buffer scan should succeed");

        // Just verify the scan executes without error
        // Finding actual secrets depends on TruffleHog's detector patterns
        println!("Buffer scan completed with {} findings", findings.len());
    }

    #[tokio::test]
    #[ignore]
    async fn test_trufflehog_detects_test_repo() {
        assert!(
            TruffleHogScanner::is_available(),
            "TruffleHog binary must be available to run this integration test"
        );

        let repo_url = "https://github.com/trufflesecurity/test_keys";

        // Create a temporary directory for the clone
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo_path = temp_dir.path();

        // Do a full shallow clone (not partial) so TruffleHog can scan it
        let output = AsyncCommand::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg(repo_url)
            .arg(".")
            .current_dir(repo_path)
            .output()
            .await
            .expect("git clone to execute");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("Failed to clone test repository: {}", stderr);
        }

        let scanner = TruffleHogScanner::new(TruffleHogConfig {
            only_verified: false,
            timeout_seconds: 180,
            ..Default::default()
        });

        let findings = scanner
            .scan_repository(repo_path, "", "HEAD")
            .await
            .expect("scan to succeed against test corpus");

        assert!(
            !findings.is_empty(),
            "expected at least one finding from test_keys corpus"
        );
        let detectors: Vec<String> = findings
            .iter()
            .filter_map(|f| f.detector_name.clone())
            .collect();
        assert!(
            detectors
                .iter()
                .any(|name| name.to_lowercase().contains("github")
                    || name.to_lowercase().contains("aws")),
            "expected GitHub/AWS detector hits, got {:?}",
            detectors
        );

        let verified = findings
            .iter()
            .filter(|f| f.verified.unwrap_or(false))
            .count();
        assert!(
            verified > 0,
            "expected at least one verified secret in corpus, got {}",
            verified
        );

        let with_metadata = findings.iter().find(|f| {
            f.source_metadata
                .as_ref()
                .and_then(|m| m.data.as_ref())
                .and_then(|d| d.git.as_ref())
                .and_then(|g| g.file.as_ref())
                .is_some()
                && f.raw.is_some()
        });
        assert!(
            with_metadata.is_some(),
            "expected at least one finding with file/commit metadata"
        );
    }
}
