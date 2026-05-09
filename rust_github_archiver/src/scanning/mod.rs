// Enhanced scanning service with advanced features
use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant as StdInstant};
use tokio::sync::{watch, RwLock, Semaphore};
use tracing::{error, info, warn}; // Removed unused debug
use uuid::Uuid;

use crate::core::database::EventScanTarget;
use crate::core::PersistenceService;
use crate::secrets::{ScanResult, SecretCategory, SecretMatch, SecretScanner, SecretSeverity};

pub mod cache;
pub mod domain;
pub mod persistence;
pub mod trufflehog;
use self::domain::{ScanFinding, ScanInitiator, SourceEventProvenance};
use self::trufflehog::TruffleHogFinding;
pub use trufflehog::{GitCloner, TruffleHogConfig, TruffleHogScanner};

/// Advanced scanning service with batch processing, scheduling, and filtering
pub struct ScanningService {
    #[allow(dead_code)] // Scanner will be used for actual secret detection
    scanner: SecretScanner,
    active_scans: Arc<RwLock<HashMap<String, ScanJob>>>,
    scan_history: Arc<RwLock<Vec<CompletedScan>>>,
    scan_schedules: Arc<RwLock<HashMap<String, ScanSchedule>>>,
    max_concurrent_scans: usize,
    semaphore: Arc<Semaphore>,
    persistence: Option<Arc<PersistenceService>>,
    execution_state: Arc<RwLock<ScanExecutionState>>,
    execution_signal: Arc<watch::Sender<ScanExecutionState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanJob {
    pub id: String,
    pub repository: String,
    pub scan_type: ScanType,
    pub config: ScanConfig,
    pub status: ScanStatus,
    pub started_at: DateTime<Utc>,
    pub progress: ScanProgress,
    pub created_by: String,
    pub initiator: ScanInitiator,
    pub source_events: Vec<SourceEventProvenance>,
    pub event_targets: Vec<EventScanTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedScan {
    pub id: String,
    pub repository: String,
    pub scan_type: ScanType,
    pub status: ScanStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub results: ScanResults,
    pub created_by: String,
    pub initiator: ScanInitiator,
    pub source_events: Vec<SourceEventProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub secret_types: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub include_extensions: Option<Vec<String>>,
    pub exclude_paths: Vec<String>,
    pub max_file_size_mb: Option<u32>,
    pub timeout_seconds: Option<u32>,
    pub entropy_threshold: Option<f64>,
    pub verify_secrets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanType {
    Full,
    Incremental,
    Targeted,
    Scheduled,
    Manual,
}

impl ScanType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanType::Full => "full",
            ScanType::Incremental => "incremental",
            ScanType::Targeted => "targeted",
            ScanType::Scheduled => "scheduled",
            ScanType::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl ScanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanStatus::Queued => "queued",
            ScanStatus::Running => "running",
            ScanStatus::Completed => "completed",
            ScanStatus::Failed => "failed",
            ScanStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub files_scanned: u32,
    pub total_files: u32,
    pub findings_found: u32,
    pub current_file: Option<String>,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanResults {
    #[serde(alias = "secrets")]
    pub findings: Vec<ScanFinding>,
    pub files_scanned: u32,
    pub total_lines: u32,
    pub scan_duration_ms: u64,
    pub severity_breakdown: HashMap<String, u32>,
    pub category_breakdown: HashMap<String, u32>,
    pub detector_stats: HashMap<String, u32>,
    pub false_positives: u32,
    #[serde(alias = "verified_secrets")]
    pub verified_findings: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSchedule {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub repositories: Vec<String>,
    pub config: ScanConfig,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: DateTime<Utc>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanFilter {
    pub repository: Option<String>,
    pub severity: Option<SecretSeverity>,
    pub category: Option<SecretCategory>,
    pub detector: Option<String>,
    pub verified_only: Option<bool>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStatistics {
    pub total_scans: u32,
    pub repositories_scanned: u32,
    pub total_findings: u32,
    pub verified_findings: u32,
    pub false_positives: u32,
    pub avg_scan_time_ms: u64,
    pub avg_secrets_per_repo: f64,
    pub success_rate: f64,
    pub severity_distribution: HashMap<String, u32>,
    pub category_distribution: HashMap<String, u32>,
    pub detector_performance: HashMap<String, DetectorStats>,
    pub recent_activity: RecentActivity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorStats {
    pub matches: u32,
    pub false_positive_rate: f64,
    pub avg_entropy: f64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentActivity {
    pub scans_last_24h: u32,
    pub findings_last_24h: u32,
    pub top_repositories: Vec<RepositoryStats>,
    pub trending_detectors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryStats {
    pub name: String,
    pub findings: u32,
    pub last_scan: DateTime<Utc>,
    pub risk_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActiveScanMetrics {
    pub active_scans: usize,
    pub files_processed: u64,
    pub events_processed: u64,
    pub processing_rate: f64,
    pub findings_found: u64,
    pub oldest_start: Option<DateTime<Utc>>,
    pub last_activity: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanExecutionState {
    Running,
    Paused,
    ShuttingDown,
}

const EVENT_SCAN_LIMIT: usize = 25;

#[derive(Debug, Clone)]
struct EventCommit {
    event_id: i64,
    repository: String,
    repository_url: Option<String>,
    before_sha: String,
    head_sha: Option<String>,
    reference: Option<String>,
    forced: bool,
    is_zero_commit: bool,
    commit_count: usize,
    created_at: DateTime<Utc>,
}

impl From<EventScanTarget> for EventCommit {
    fn from(value: EventScanTarget) -> Self {
        Self {
            event_id: value.event_id,
            repository: value.repository_full_name.clone(),
            repository_url: value.repository_url.clone(),
            before_sha: value.before_sha,
            head_sha: value.head_sha,
            reference: value.reference,
            forced: value.forced,
            is_zero_commit: value.is_zero_commit,
            commit_count: value.commit_count.max(1) as usize,
            created_at: value.event_created_at,
        }
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            secret_types: vec![
                "AWS Access Key ID".to_string(),
                "GitHub Personal Access Token".to_string(),
                "MongoDB Connection String".to_string(),
                "Generic API Key".to_string(),
            ],
            exclude_patterns: vec![
                "*.log".to_string(),
                "*.tmp".to_string(),
                "node_modules/*".to_string(),
                ".git/*".to_string(),
            ],
            include_extensions: None,
            exclude_paths: vec![
                "target/".to_string(),
                "build/".to_string(),
                "dist/".to_string(),
            ],
            max_file_size_mb: Some(10),
            timeout_seconds: Some(300), // 5 minutes
            entropy_threshold: Some(4.5),
            verify_secrets: true,
        }
    }
}

impl ScanningService {
    fn map_clone_error(
        repo_api: &str,
        clone_err: &crate::scanning::trufflehog::CloneError,
    ) -> anyhow::Error {
        match clone_err.kind {
            crate::scanning::trufflehog::ScanErrorKind::RepoNotFound => {
                warn!(
                    repo_api = %repo_api,
                    clone_url = ?clone_err.clone_url,
                    "Repository not found (deleted/private). Skipping retries."
                );
                anyhow!("Repo not found")
            }
            crate::scanning::trufflehog::ScanErrorKind::RepoForbidden => {
                warn!(
                    repo_api = %repo_api,
                    clone_url = ?clone_err.clone_url,
                    "Repository forbidden or invalid credentials. Skipping retries."
                );
                anyhow!("Repo forbidden")
            }
            crate::scanning::trufflehog::ScanErrorKind::TooLargeRepository => {
                warn!(repo_api = %repo_api, clone_url = ?clone_err.clone_url, "Repository exceeds size guard.");
                anyhow!("Repo too large")
            }
            crate::scanning::trufflehog::ScanErrorKind::MisconfiguredEndpoint => {
                error!(repo_api = %repo_api, clone_url = ?clone_err.clone_url, "Misconfigured clone endpoint");
                anyhow!("Misconfigured endpoint")
            }
            crate::scanning::trufflehog::ScanErrorKind::GitRateLimited
            | crate::scanning::trufflehog::ScanErrorKind::ApiRateLimited { .. } => {
                warn!(repo_api = %repo_api, clone_url = ?clone_err.clone_url, "Repository hit rate limits; leaving events queued");
                anyhow!("Rate limited")
            }
            _ => {
                error!(
                    repo_api = %repo_api,
                    clone_url = ?clone_err.clone_url,
                    "Failed to clone repository (clone url normalized): {}",
                    clone_err.message
                );
                anyhow!("Failed to clone repository: {}", clone_err.message)
            }
        }
    }

    /// Create a new scanning service
    pub fn new(max_concurrent_scans: usize) -> Self {
        let (execution_signal, _execution_receiver) = watch::channel(ScanExecutionState::Running);
        Self {
            scanner: SecretScanner::new(),
            active_scans: Arc::new(RwLock::new(HashMap::new())),
            scan_history: Arc::new(RwLock::new(Vec::new())),
            scan_schedules: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent_scans,
            semaphore: Arc::new(Semaphore::new(max_concurrent_scans)),
            persistence: None,
            execution_state: Arc::new(RwLock::new(ScanExecutionState::Running)),
            execution_signal: Arc::new(execution_signal),
        }
    }

    pub fn with_persistence(mut self, persistence: Arc<PersistenceService>) -> Self {
        self.persistence = Some(persistence);
        self
    }

    pub async fn pause_execution(&self) {
        self.set_execution_state(ScanExecutionState::Paused).await;
    }

    pub async fn resume_execution(&self) {
        self.set_execution_state(ScanExecutionState::Running).await;
    }

    pub async fn request_shutdown(&self) {
        self.set_execution_state(ScanExecutionState::ShuttingDown)
            .await;
    }

    pub async fn execution_state(&self) -> ScanExecutionState {
        *self.execution_state.read().await
    }

    pub async fn wait_for_active_scans(&self, timeout: StdDuration) -> Result<()> {
        let deadline = StdInstant::now() + timeout;

        while StdInstant::now() < deadline {
            if self.get_active_scans_count().await == 0 {
                return Ok(());
            }
            tokio::time::sleep(StdDuration::from_millis(100)).await;
        }

        Err(anyhow!("Timed out waiting for active scans to settle"))
    }

    async fn set_execution_state(&self, state: ScanExecutionState) {
        *self.execution_state.write().await = state;
        let _ = self.execution_signal.send(state);
    }

    /// Record secrets detected by the realtime monitor so that monitoring APIs
    /// can surface accurate statistics.
    pub async fn record_realtime_detection(
        &self,
        repository: &str,
        matches: Vec<SecretMatch>,
        detection_time: DateTime<Utc>,
        source_reference: &str,
    ) -> Result<()> {
        if matches.is_empty() {
            return Ok(());
        }

        let mut severity_breakdown: HashMap<String, u32> = HashMap::new();
        let mut category_breakdown: HashMap<String, u32> = HashMap::new();
        let mut detector_stats: HashMap<String, u32> = HashMap::new();

        for secret in &matches {
            *severity_breakdown
                .entry(secret.severity.to_string())
                .or_insert(0) += 1;
            *category_breakdown
                .entry(secret.category.to_string())
                .or_insert(0) += 1;
            *detector_stats
                .entry(secret.detector_name.clone())
                .or_insert(0) += 1;
        }

        let verified_findings = matches.iter().filter(|finding| finding.verified).count() as u32;
        let files_scanned = matches
            .iter()
            .filter_map(|finding| finding.filename.as_ref())
            .collect::<HashSet<_>>()
            .len()
            .max(1) as u32;
        let total_lines = matches
            .iter()
            .filter_map(|finding| finding.line_number)
            .max()
            .unwrap_or(0) as u32;
        let duration_ms = 0;

        let results = ScanResults {
            findings: matches.clone(),
            files_scanned,
            total_lines,
            scan_duration_ms: duration_ms,
            severity_breakdown,
            category_breakdown,
            detector_stats,
            false_positives: 0,
            verified_findings,
        };

        let completed_scan = CompletedScan {
            id: Uuid::new_v4().to_string(),
            repository: repository.to_string(),
            scan_type: ScanType::Targeted,
            status: ScanStatus::Completed,
            started_at: detection_time,
            completed_at: detection_time,
            duration_ms,
            results,
            created_by: ScanInitiator::realtime(source_reference).created_by_label(),
            initiator: ScanInitiator::realtime(source_reference),
            source_events: Vec::new(),
        };

        let mut history = self.scan_history.write().await;
        history.push(completed_scan);

        if history.len() > 1000 {
            history.remove(0);
        }

        let persisted = history.last().cloned();
        drop(history);

        if let Some(scan) = persisted {
            self.persist_scan_artifacts(&scan, None).await;
        }

        Ok(())
    }

    /// Start a new scan
    pub async fn start_scan(
        &self,
        repository: String,
        scan_type: ScanType,
        config: ScanConfig,
        initiator: ScanInitiator,
        event_targets: Vec<EventScanTarget>,
    ) -> Result<String> {
        match self.execution_state().await {
            ScanExecutionState::Running => {}
            ScanExecutionState::Paused => {
                return Err(anyhow!(
                    "Scanning service is paused; resume it before launching new scans"
                ));
            }
            ScanExecutionState::ShuttingDown => {
                return Err(anyhow!(
                    "Scanning service is shutting down and will not accept new scans"
                ));
            }
        }

        let scan_id = Uuid::new_v4().to_string();
        let source_events: Vec<SourceEventProvenance> = event_targets
            .iter()
            .map(SourceEventProvenance::from)
            .collect();
        let created_by = initiator.created_by_label();

        let scan_job = ScanJob {
            id: scan_id.clone(),
            repository: repository.clone(),
            scan_type,
            config: config.clone(),
            status: ScanStatus::Queued,
            started_at: Utc::now(),
            progress: ScanProgress {
                files_scanned: 0,
                total_files: 0,
                findings_found: 0,
                current_file: None,
                percentage: 0.0,
            },
            created_by,
            initiator,
            source_events,
            event_targets,
        };

        // Add to active scans
        {
            let mut active_scans = self.active_scans.write().await;
            active_scans.insert(scan_id.clone(), scan_job);
        }

        // Start scanning in background
        let service = self.clone();
        let repo = repository.clone();
        let scan_id_clone = scan_id.clone();

        tokio::spawn(async move {
            if let Err(e) = service.execute_scan(scan_id_clone, repo, config).await {
                error!("Scan execution failed: {}", e);
            }
        });

        info!("Started scan {} for repository {}", scan_id, repository);
        Ok(scan_id)
    }

    /// Execute the actual scan
    async fn execute_scan(
        &self,
        scan_id: String,
        repository: String,
        config: ScanConfig,
    ) -> Result<()> {
        // Acquire semaphore permit
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to acquire scan permit: {}", e))?;

        if let Err(error) = self.wait_for_execution_window(&scan_id).await {
            self.handle_scan_cancellation(&scan_id, &error.to_string())
                .await;
            return Err(error);
        }

        // Update status to running
        let event_targets = {
            let mut active_scans = self.active_scans.write().await;
            if let Some(scan) = active_scans.get_mut(&scan_id) {
                scan.status = ScanStatus::Running;
                scan.event_targets.clone()
            } else {
                Vec::new()
            }
        };

        let start_time = Utc::now();

        // Run scanning process
        let (scan_result, completed_events, failed_events) = match self
            .perform_repository_scan(&repository, &config, &scan_id, &event_targets)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                let repo_api = repository.clone();
                let clone_err = e
                    .downcast_ref::<crate::scanning::trufflehog::CloneError>()
                    .cloned();
                if let Some(clone_err) = clone_err {
                    return Err(Self::map_clone_error(&repo_api, &clone_err));
                }
                // If we were rate limited (or similar), keep the events for retry after cooldown.
                let msg = e.to_string();
                let lowered = msg.to_ascii_lowercase();
                if lowered.contains("cancelled") || lowered.contains("shutdown") {
                    self.handle_scan_cancellation(&scan_id, &msg).await;
                    return Err(anyhow!(msg));
                }
                if lowered.contains("rate_limit")
                    || lowered.contains("rate limit")
                    || lowered.contains("rate limited")
                    || lowered.contains("status 403")
                {
                    warn!(
                        "Scan {} hit rate limits for {}. Marking events failed with retry delay.",
                        scan_id, repository
                    );
                }
                self.handle_scan_failure(&scan_id, &msg).await;
                return Err(anyhow!(msg));
            }
        };

        let end_time = Utc::now();
        let duration_ms = (end_time - start_time).num_milliseconds() as u64;

        // Calculate statistics
        let mut severity_breakdown = HashMap::new();
        let mut category_breakdown = HashMap::new();
        let mut detector_stats = HashMap::new();
        let mut verified_findings = 0;

        for finding in &scan_result.matches {
            *severity_breakdown
                .entry(finding.severity.to_string())
                .or_insert(0) += 1;
            *category_breakdown
                .entry(finding.category.to_string())
                .or_insert(0) += 1;
            *detector_stats
                .entry(finding.detector_name.clone())
                .or_insert(0) += 1;

            if finding.verified {
                verified_findings += 1;
            }
        }

        let results = ScanResults {
            findings: scan_result.matches,
            files_scanned: scan_result.files_scanned as u32,
            total_lines: scan_result.total_lines as u32,
            scan_duration_ms: duration_ms,
            severity_breakdown,
            category_breakdown,
            detector_stats: detector_stats
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            false_positives: 0,
            verified_findings,
        };

        // Move from active to completed
        let completed_scan = {
            let mut active_scans = self.active_scans.write().await;
            if let Some(mut scan) = active_scans.remove(&scan_id) {
                scan.status = ScanStatus::Completed;
                CompletedScan {
                    id: scan.id,
                    repository: scan.repository,
                    scan_type: scan.scan_type,
                    status: scan.status,
                    started_at: scan.started_at,
                    completed_at: end_time,
                    duration_ms,
                    results,
                    created_by: scan.created_by,
                    initiator: scan.initiator,
                    source_events: scan.source_events.clone(),
                }
            } else {
                return Err(anyhow!("Scan not found in active scans"));
            }
        };

        // Add to history
        let persisted = {
            let mut history = self.scan_history.write().await;
            history.push(completed_scan);

            // Keep only last 1000 scans
            if history.len() > 1000 {
                history.remove(0);
            }

            history.last().cloned()
        };

        if let Some(scan) = persisted {
            self.persist_scan_artifacts(&scan, None).await;
        }

        if let Some(persistence) = &self.persistence {
            if !completed_events.is_empty() {
                if let Err(e) = persistence
                    .mark_push_events_completed(&completed_events)
                    .await
                {
                    warn!("Failed to mark push events as completed: {}", e);
                }
            }

            if !failed_events.is_empty() {
                if let Err(e) = persistence
                    .mark_push_events_failed(&failed_events, Some("unreachable dangling commit"))
                    .await
                {
                    warn!("Failed to mark push events as failed: {}", e);
                }
            }
        }

        info!("Completed scan {} for repository {}", scan_id, repository);
        Ok(())
    }

    async fn persist_scan_artifacts(&self, scan: &CompletedScan, failure_reason: Option<&str>) {
        let Some(persistence) = &self.persistence else {
            tracing::debug!("Skipping scan persistence because persistence is not configured");
            return;
        };

        if let Err(error) = persistence.persist_scan(scan, failure_reason).await {
            warn!("Failed to persist scan {}: {}", scan.id, error);
        }
    }

    /// Perform the actual repository scanning using TruffleHog
    async fn perform_repository_scan(
        &self,
        repository: &str,
        config: &ScanConfig,
        scan_id: &str,
        event_targets: &[EventScanTarget],
    ) -> Result<(ScanResult, Vec<i64>, Vec<i64>)> {
        info!("Scanning repository: {} with TruffleHog", repository);

        if !TruffleHogScanner::is_available() {
            error!(
                "TruffleHog binary not found. Set TRUFFLEHOG_PATH or install it in github_scraper_env to run real scans"
            );
            return Err(anyhow!(
                "TruffleHog binary not found. Install it and set TRUFFLEHOG_PATH or place the binary in PATH."
            ));
        }

        let normalized_repo = Self::normalize_repository_identifier(repository);

        let event_commits = if event_targets.is_empty() {
            match self
                .fetch_commits_from_event_store(&normalized_repo, EVENT_SCAN_LIMIT)
                .await
            {
                Ok(commits) => commits,
                Err(e) => {
                    warn!(
                        "Failed to read event store for {}: {}. Falling back to direct scan",
                        repository, e
                    );
                    Vec::new()
                }
            }
        } else {
            event_targets
                .iter()
                .cloned()
                .map(EventCommit::from)
                .collect::<Vec<_>>()
        };

        let repo_url = Self::determine_repository_url(repository, &normalized_repo, &event_commits);

        if repo_url.trim().is_empty() {
            return Err(anyhow!(
                "Unable to determine repository URL for {}",
                repository
            ));
        }

        let scanner = TruffleHogScanner::new(TruffleHogConfig {
            only_verified: config.verify_secrets,
            no_update: true,
            timeout_seconds: config.timeout_seconds.unwrap_or(300) as u64,
            binary_path: None,
        });

        let mut cloner = GitCloner::new();
        let repo_path = match cloner.partial_clone(&repo_url).await {
            Ok(path) => {
                info!("Successfully cloned repository to {:?}", path);
                path
            }
            Err(e) => {
                let repo_api = repo_url.clone();
                let clone_err = e
                    .downcast_ref::<crate::scanning::trufflehog::CloneError>()
                    .cloned();
                if let Some(clone_err) = clone_err {
                    return Err(Self::map_clone_error(&repo_api, &clone_err));
                }
                error!("Failed to clone repository {}: {}", repo_url, e);
                return Err(anyhow!("Failed to clone repository: {}", e));
            }
        };

        let mut total_secrets = Vec::new();
        let mut unique_files: HashSet<String> = HashSet::new();
        let mut total_lines: usize = 0;
        let mut successful_units: usize = 0;
        let scan_start = std::time::Instant::now();

        let mut completed_events = Vec::new();
        let mut failed_events = Vec::new();

        if !event_commits.is_empty() {
            info!(
                "Found {} push events in event store for {}",
                event_commits.len(),
                repository
            );
            let total_units = Self::calculate_total_commit_units(&event_commits);
            self.initialize_scan_progress(scan_id, total_units).await;

            let mut attempted_units: usize = 0;

            for target in &event_commits {
                self.wait_for_execution_window(scan_id).await?;
                let event_units = target.commit_count.max(1);
                let mut progress_ref = Self::resolve_branch_reference(target);

                match Self::prepare_event_scan(&cloner, &repo_path, target).await {
                    Ok((base_commit, branch_ref)) => {
                        progress_ref = branch_ref.clone();
                        match scanner
                            .scan_repository(&repo_path, &base_commit, &branch_ref)
                            .await
                        {
                            Ok(findings) => {
                                self.append_findings_to_results(
                                    findings,
                                    Some(target),
                                    &mut total_secrets,
                                    &mut unique_files,
                                    &mut total_lines,
                                );

                                successful_units += event_units;
                                completed_events.push(target.event_id);
                            }
                            Err(e) => {
                                warn!("TruffleHog scan failed for commit {}: {}", branch_ref, e);
                                failed_events.push(target.event_id);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Unable to identify base commit for {} in {}: {}",
                            target.before_sha, repository, e
                        );
                        failed_events.push(target.event_id);
                    }
                }

                attempted_units += event_units;
                let files_examined = unique_files.len().max(successful_units.max(1));

                self.update_scan_progress_for_commit(
                    scan_id,
                    attempted_units.min(total_units),
                    total_units,
                    &progress_ref,
                    files_examined,
                    total_secrets.len(),
                )
                .await;
            }

            if successful_units == 0 {
                return Err(anyhow!(
                    "all event-driven scans failed for {}; events will be retried or surfaced as failed",
                    repository
                ));
            }
        } else {
            warn!(
                "No push events found in event store for {}. Running direct repository scan",
                repository
            );
            self.initialize_scan_progress(scan_id, 1).await;
            self.wait_for_execution_window(scan_id).await?;
            let findings = scanner.scan_repository(&repo_path, "", "HEAD").await?;
            self.append_findings_to_results(
                findings,
                None,
                &mut total_secrets,
                &mut unique_files,
                &mut total_lines,
            );
            let files_examined = unique_files.len().max(1);
            self.update_scan_progress_for_commit(
                scan_id,
                1,
                1,
                "HEAD",
                files_examined,
                total_secrets.len(),
            )
            .await;
            successful_units = 1;
        }

        let final_files_examined = unique_files.len().max(successful_units.max(1));
        self.finalize_scan_progress(scan_id, final_files_examined, total_secrets.len())
            .await;

        let scan_duration_ms = scan_start.elapsed().as_millis() as u64;

        let mut detector_stats = HashMap::new();
        for secret in &total_secrets {
            *detector_stats
                .entry(secret.detector_name.clone())
                .or_insert(0) += 1;
        }

        info!(
            "Scan completed for {}: {} secrets in {}ms",
            repository,
            total_secrets.len(),
            scan_duration_ms
        );

        Ok((
            ScanResult {
                matches: total_secrets,
                files_scanned: final_files_examined,
                total_lines,
                scan_duration_ms,
                detector_stats,
            },
            completed_events,
            failed_events,
        ))
    }

    /// Map TruffleHog detector name to severity and category
    fn map_detector_to_severity_category(detector_name: &str) -> (SecretSeverity, SecretCategory) {
        let lower = detector_name.to_lowercase();

        let mut severity = if lower.contains("private")
            || lower.contains("secret")
            || lower.contains("password")
        {
            SecretSeverity::Critical
        } else if lower.contains("token")
            || lower.contains("key")
            || lower.contains("aws")
            || lower.contains("webhook")
            || lower.contains("slack")
            || lower.contains("discord")
        {
            SecretSeverity::High
        } else if lower.contains("api") {
            SecretSeverity::Medium
        } else {
            SecretSeverity::Low
        };

        let category = if lower.contains("aws")
            || lower.contains("azure")
            || lower.contains("gcp")
            || lower.contains("cloud")
        {
            SecretCategory::CloudProvider
        } else if lower.contains("github")
            || lower.contains("gitlab")
            || lower.contains("bitbucket")
        {
            SecretCategory::Token
        } else if lower.contains("mongo")
            || lower.contains("postgres")
            || lower.contains("mysql")
            || lower.contains("database")
        {
            SecretCategory::Database
        } else if lower.contains("stripe") || lower.contains("paypal") || lower.contains("payment")
        {
            SecretCategory::ApiKey // Payment services use API keys
        } else if lower.contains("slack") || lower.contains("discord") || lower.contains("webhook")
        {
            SecretCategory::Webhook
        } else if lower.contains("private") && lower.contains("key") {
            SecretCategory::PrivateKey
        } else if lower.contains("certificate") || lower.contains("cert") {
            SecretCategory::Certificate
        } else {
            SecretCategory::ApiKey
        };

        if category == SecretCategory::CloudProvider
            && matches!(severity, SecretSeverity::Low | SecretSeverity::Medium)
        {
            severity = SecretSeverity::High;
        }

        (severity, category)
    }

    fn normalize_repository_identifier(repository: &str) -> String {
        if repository.trim().is_empty() {
            return String::new();
        }

        let mut normalized = repository.trim();

        if normalized.ends_with(".git") {
            normalized = &normalized[..normalized.len().saturating_sub(4)];
        }

        let lowered = normalized.to_ascii_lowercase();
        if let Some(pos) = lowered.find("github.com/") {
            let start = pos + "github.com/".len();
            normalized = &normalized[start..];
        } else if let Some(pos) = lowered.find("git@github.com:") {
            let start = pos + "git@github.com:".len();
            normalized = &normalized[start..];
        }

        normalized.trim_matches('/').to_lowercase()
    }

    fn determine_repository_url(
        original: &str,
        normalized_repo: &str,
        commits: &[EventCommit],
    ) -> String {
        if original.starts_with("http") || original.starts_with("git@") {
            return original.to_string();
        }

        if let Some(url) = commits
            .iter()
            .find_map(|commit| commit.repository_url.as_ref().cloned())
        {
            if !url.trim().is_empty() {
                return url;
            }
        }

        if let Some(name) = commits.iter().find_map(|commit| {
            let repo = commit.repository.trim();
            if repo.is_empty() {
                None
            } else {
                Some(repo.to_string())
            }
        }) {
            return format!("https://github.com/{}", name.trim_matches('/'));
        }

        if !normalized_repo.is_empty() {
            return format!("https://github.com/{}", normalized_repo);
        }

        original.to_string()
    }

    fn calculate_total_commit_units(commits: &[EventCommit]) -> usize {
        if commits.is_empty() {
            return 0;
        }

        let unit_sum: usize = commits.iter().map(|commit| commit.commit_count).sum();
        unit_sum.max(commits.len()).max(1)
    }

    fn resolve_branch_reference(commit: &EventCommit) -> String {
        if Self::is_deleted_history_target(commit) {
            return commit.before_sha.clone();
        }

        if let Some(head) = commit.head_sha.as_ref() {
            let trimmed = head.trim();
            if !trimmed.is_empty() && !trimmed.chars().all(|c| c == '0') {
                return trimmed.to_string();
            }
        }

        if let Some(reference) = commit.reference.as_ref() {
            let trimmed = reference.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        commit.before_sha.clone()
    }

    fn is_deleted_history_target(commit: &EventCommit) -> bool {
        commit.is_zero_commit || commit.forced
    }

    async fn prepare_event_scan(
        cloner: &GitCloner,
        repo_path: &std::path::Path,
        target: &EventCommit,
    ) -> Result<(String, String)> {
        if Self::is_deleted_history_target(target) {
            let prepared = cloner
                .prepare_commit_scan_ref(repo_path, &target.before_sha)
                .await?;
            Ok((prepared.since_commit, prepared.branch))
        } else {
            let branch_ref = Self::resolve_branch_reference(target);
            let base_commit = cloner
                .identify_base_commit(repo_path, &target.before_sha)
                .await?;
            Ok((base_commit, branch_ref))
        }
    }

    async fn fetch_commits_from_event_store(
        &self,
        repository: &str,
        limit: usize,
    ) -> Result<Vec<EventCommit>> {
        let persistence = self
            .persistence
            .clone()
            .ok_or_else(|| anyhow!("Persistence is not configured for scanning"))?;

        if repository.is_empty() {
            return Ok(Vec::new());
        }

        let targets = persistence
            .repository_push_events(repository, limit)
            .await?;

        Ok(targets.into_iter().map(EventCommit::from).collect())
    }

    async fn wait_for_execution_window(&self, scan_id: &str) -> Result<()> {
        loop {
            {
                let active_scans = self.active_scans.read().await;
                if let Some(scan) = active_scans.get(scan_id) {
                    if matches!(scan.status, ScanStatus::Cancelled) {
                        return Err(anyhow!("Scan was cancelled by operator request"));
                    }
                }
            }

            match self.execution_state().await {
                ScanExecutionState::Running => return Ok(()),
                ScanExecutionState::ShuttingDown => {
                    return Err(anyhow!("Scan cancelled due to runtime shutdown"));
                }
                ScanExecutionState::Paused => {
                    let mut receiver = self.execution_signal.subscribe();
                    if self.execution_state().await != ScanExecutionState::Paused {
                        continue;
                    }

                    receiver
                        .changed()
                        .await
                        .map_err(|_| anyhow!("Scanning execution control channel closed"))?;
                }
            }
        }
    }

    async fn initialize_scan_progress(&self, scan_id: &str, total_units: usize) {
        let mut active_scans = self.active_scans.write().await;
        if let Some(scan) = active_scans.get_mut(scan_id) {
            scan.progress.total_files = total_units as u32;
            scan.progress.files_scanned = 0;
            scan.progress.findings_found = 0;
            scan.progress.percentage = 0.0;
            scan.progress.current_file = None;
        }
    }

    async fn update_scan_progress_for_commit(
        &self,
        scan_id: &str,
        processed_units: usize,
        total_units: usize,
        current_commit: &str,
        files_scanned: usize,
        findings_found: usize,
    ) {
        let mut active_scans = self.active_scans.write().await;
        if let Some(scan) = active_scans.get_mut(scan_id) {
            scan.progress.total_files = total_units as u32;
            scan.progress.files_scanned = files_scanned as u32;
            scan.progress.findings_found = findings_found as u32;
            scan.progress.percentage = if total_units == 0 {
                0.0
            } else {
                ((processed_units as f32) / (total_units as f32)).min(1.0) * 100.0
            };
            scan.progress.current_file = Some(format!(
                "Commit {} ({} / {})",
                Self::short_sha(current_commit),
                processed_units,
                total_units
            ));
        }
    }

    async fn finalize_scan_progress(
        &self,
        scan_id: &str,
        files_scanned: usize,
        findings_found: usize,
    ) {
        let mut active_scans = self.active_scans.write().await;
        if let Some(scan) = active_scans.get_mut(scan_id) {
            scan.progress.files_scanned = files_scanned as u32;
            scan.progress.findings_found = findings_found as u32;
            scan.progress.percentage = 100.0;
            scan.progress.current_file = None;
        }
    }

    async fn handle_scan_failure(&self, scan_id: &str, error_msg: &str) {
        let failure_record = {
            let mut active_scans = self.active_scans.write().await;
            if let Some(scan) = active_scans.remove(scan_id) {
                let failed_results = ScanResults {
                    files_scanned: scan.progress.files_scanned,
                    ..ScanResults::default()
                };
                let duration_ms = (Utc::now() - scan.started_at).num_milliseconds().max(0) as u64;
                Some((
                    CompletedScan {
                        id: scan.id.clone(),
                        repository: scan.repository.clone(),
                        scan_type: scan.scan_type,
                        status: ScanStatus::Failed,
                        started_at: scan.started_at,
                        completed_at: Utc::now(),
                        duration_ms,
                        results: failed_results,
                        created_by: scan.created_by.clone(),
                        initiator: scan.initiator.clone(),
                        source_events: scan.source_events.clone(),
                    },
                    scan.source_events,
                ))
            } else {
                None
            }
        };

        if let Some((failed_scan, source_events)) = failure_record {
            self.persist_scan_artifacts(&failed_scan, Some(error_msg))
                .await;
            {
                let mut history = self.scan_history.write().await;
                history.push(failed_scan);
                if history.len() > 1000 {
                    history.remove(0);
                }
            }

            if let Some(persistence) = &self.persistence {
                let failed_event_ids: Vec<i64> =
                    source_events.iter().map(|event| event.event_id).collect();
                if !failed_event_ids.is_empty() {
                    if let Err(e) = persistence
                        .mark_push_events_failed(&failed_event_ids, Some(error_msg))
                        .await
                    {
                        warn!("Failed to mark push events as failed: {}", e);
                    }
                }
            }
        }
    }

    async fn handle_scan_cancellation(&self, scan_id: &str, reason: &str) {
        let cancellation_record = {
            let mut active_scans = self.active_scans.write().await;
            if let Some(scan) = active_scans.remove(scan_id) {
                let cancelled_results = ScanResults {
                    files_scanned: scan.progress.files_scanned,
                    ..ScanResults::default()
                };
                let duration_ms = (Utc::now() - scan.started_at).num_milliseconds().max(0) as u64;
                Some((
                    CompletedScan {
                        id: scan.id.clone(),
                        repository: scan.repository.clone(),
                        scan_type: scan.scan_type,
                        status: ScanStatus::Cancelled,
                        started_at: scan.started_at,
                        completed_at: Utc::now(),
                        duration_ms,
                        results: cancelled_results,
                        created_by: scan.created_by.clone(),
                        initiator: scan.initiator.clone(),
                        source_events: scan.source_events.clone(),
                    },
                    scan.source_events,
                ))
            } else {
                None
            }
        };

        if let Some((cancelled_scan, source_events)) = cancellation_record {
            self.persist_scan_artifacts(&cancelled_scan, Some(reason))
                .await;
            {
                let mut history = self.scan_history.write().await;
                history.push(cancelled_scan);
                if history.len() > 1000 {
                    history.remove(0);
                }
            }

            if let Some(persistence) = &self.persistence {
                let event_ids: Vec<i64> =
                    source_events.iter().map(|event| event.event_id).collect();
                if !event_ids.is_empty() {
                    if let Err(error) = persistence.release_push_events(&event_ids).await {
                        warn!(
                            "Failed to release push events after cancellation: {}",
                            error
                        );
                    }
                }
            }
        }
    }

    fn append_findings_to_results(
        &self,
        findings: Vec<TruffleHogFinding>,
        context: Option<&EventCommit>,
        total_secrets: &mut Vec<SecretMatch>,
        unique_files: &mut HashSet<String>,
        total_lines: &mut usize,
    ) {
        for finding in findings {
            let detector_name = finding
                .detector_name
                .unwrap_or_else(|| "Unknown".to_string());
            let raw_value = finding.raw.or(finding.raw_v2).unwrap_or_default();

            let (filename, line_number, commit) =
                if let Some(ref metadata) = finding.source_metadata {
                    if let Some(ref data) = metadata.data {
                        if let Some(ref git_info) = data.git {
                            (
                                git_info.file.clone(),
                                git_info.line.map(|l| l as usize),
                                git_info.commit.clone(),
                            )
                        } else {
                            (None, None, None)
                        }
                    } else {
                        (None, None, None)
                    }
                } else {
                    (None, None, None)
                };

            let (severity, category) = Self::map_detector_to_severity_category(&detector_name);
            let commit_for_context = commit
                .clone()
                .or_else(|| context.map(|c| c.before_sha.clone()))
                .unwrap_or_else(|| "HEAD".to_string());

            let context_string = if let Some(ctx) = context {
                format!(
                    "Event {} @ {} • Commit {} • Ref: {} • Forced: {}",
                    ctx.event_id,
                    ctx.created_at,
                    commit_for_context.as_str(),
                    ctx.reference
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    ctx.forced
                )
            } else {
                format!("Commit: {}", commit_for_context.as_str())
            };

            if let Some(file) = filename.as_ref() {
                unique_files.insert(format!("{}:{}", commit_for_context, file));
            }

            if let Some(line) = line_number {
                *total_lines += line;
            }

            let secret_match = SecretMatch {
                detector_name: detector_name.clone(),
                matched_text: raw_value.clone(),
                start_position: 0,
                end_position: raw_value.len(),
                line_number,
                filename: filename.clone(),
                entropy: 5.0,
                severity,
                category,
                context: context_string,
                verified: finding.verified.unwrap_or(false),
                hash: format!("{:x}", md5::compute(&raw_value)),
            };

            total_secrets.push(secret_match);
        }
    }

    fn short_sha(commit: &str) -> String {
        if commit.len() > 12 {
            commit[..12].to_string()
        } else {
            commit.to_string()
        }
    }

    /// Get scan status
    pub async fn get_scan_status(&self, scan_id: &str) -> Option<ScanJob> {
        let active_scans = self.active_scans.read().await;
        active_scans.get(scan_id).cloned()
    }

    /// Wait for completion of a scan up to the provided timeout
    pub async fn wait_for_scan_completion(
        &self,
        scan_id: &str,
        timeout: StdDuration,
    ) -> Result<CompletedScan> {
        let deadline = StdInstant::now() + timeout;

        loop {
            if let Some(completed) = {
                let history = self.scan_history.read().await;
                history.iter().find(|scan| scan.id == scan_id).cloned()
            } {
                return Ok(completed);
            }

            {
                let active_scans = self.active_scans.read().await;
                if let Some(scan) = active_scans.get(scan_id) {
                    if matches!(scan.status, ScanStatus::Failed | ScanStatus::Cancelled) {
                        return Err(anyhow!(
                            "Scan {} {} before completing",
                            scan_id,
                            scan.status.as_str()
                        ));
                    }
                } else if StdInstant::now() >= deadline {
                    return Err(anyhow!(
                        "Scan {} disappeared from active set before completion",
                        scan_id
                    ));
                }
            }

            if StdInstant::now() >= deadline {
                return Err(anyhow!(
                    "Timed out waiting for scan {} to complete after {:?}",
                    scan_id,
                    timeout
                ));
            }

            tokio::time::sleep(StdDuration::from_millis(300)).await;
        }
    }

    /// Get scan results with filtering
    pub async fn get_scan_results(&self, filter: ScanFilter) -> Vec<CompletedScan> {
        let history = self.scan_history.read().await;

        history
            .iter()
            .filter(|scan| {
                if let Some(ref repo) = filter.repository {
                    if !scan.repository.contains(repo) {
                        return false;
                    }
                }

                if let Some(ref date_from) = filter.date_from {
                    if scan.completed_at < *date_from {
                        return false;
                    }
                }

                if let Some(ref date_to) = filter.date_to {
                    if scan.completed_at > *date_to {
                        return false;
                    }
                }

                true
            })
            .take(filter.limit.unwrap_or(50) as usize)
            .skip(filter.offset.unwrap_or(0) as usize)
            .cloned()
            .collect()
    }

    pub async fn get_active_scan_metrics(&self) -> ActiveScanMetrics {
        let active_scans = self.active_scans.read().await;
        if active_scans.is_empty() {
            return ActiveScanMetrics::default();
        }

        let now = Utc::now();
        let mut metrics = ActiveScanMetrics {
            active_scans: active_scans.len(),
            last_activity: Some(now),
            ..ActiveScanMetrics::default()
        };

        for scan in active_scans.values() {
            metrics.files_processed += scan.progress.files_scanned as u64;
            metrics.events_processed += scan.progress.total_files as u64;
            metrics.findings_found += scan.progress.findings_found as u64;

            metrics.oldest_start = Some(
                metrics
                    .oldest_start
                    .map(|existing| existing.min(scan.started_at))
                    .unwrap_or(scan.started_at),
            );

            let elapsed_minutes = ((now - scan.started_at).num_seconds().max(1) as f64) / 60.0;
            if elapsed_minutes.is_finite() && elapsed_minutes > 0.0 {
                metrics.processing_rate += scan.progress.files_scanned as f64 / elapsed_minutes;
            }
        }

        metrics
    }

    /// Get scanning statistics
    pub async fn get_statistics(&self) -> ScanStatistics {
        let history = self.scan_history.read().await;

        if history.is_empty() {
            return ScanStatistics {
                total_scans: 0,
                repositories_scanned: 0,
                total_findings: 0,
                verified_findings: 0,
                false_positives: 0,
                avg_scan_time_ms: 0,
                avg_secrets_per_repo: 0.0,
                success_rate: 0.0,
                severity_distribution: HashMap::new(),
                category_distribution: HashMap::new(),
                detector_performance: HashMap::new(),
                recent_activity: RecentActivity {
                    scans_last_24h: 0,
                    findings_last_24h: 0,
                    top_repositories: Vec::new(),
                    trending_detectors: Vec::new(),
                },
            };
        }

        let total_scans = history.len() as u32;
        let repositories: HashSet<_> = history.iter().map(|s| &s.repository).collect();
        let repositories_scanned = repositories.len() as u32;

        let total_findings: u32 = history
            .iter()
            .map(|scan| scan.results.findings.len() as u32)
            .sum();
        let verified_findings: u32 = history
            .iter()
            .map(|scan| scan.results.verified_findings)
            .sum();
        let false_positives: u32 = history.iter().map(|s| s.results.false_positives).sum();

        let avg_scan_time_ms =
            history.iter().map(|s| s.duration_ms).sum::<u64>() / total_scans as u64;
        let avg_secrets_per_repo = total_findings as f64 / repositories_scanned as f64;

        let successful_scans = history
            .iter()
            .filter(|s| matches!(s.status, ScanStatus::Completed))
            .count();
        let success_rate = successful_scans as f64 / total_scans as f64;

        // Calculate distributions from completed scan history.
        let mut severity_distribution = HashMap::new();
        let mut category_distribution = HashMap::new();
        let mut detector_totals: HashMap<String, (u32, f64, DateTime<Utc>)> = HashMap::new();

        for scan in history.iter() {
            for finding in &scan.results.findings {
                *severity_distribution
                    .entry(finding.severity.to_string())
                    .or_insert(0) += 1;
                *category_distribution
                    .entry(finding.category.to_string())
                    .or_insert(0) += 1;

                let detector_stats = detector_totals
                    .entry(finding.detector_name.clone())
                    .or_insert((0, 0.0, scan.completed_at));
                detector_stats.0 += 1;
                detector_stats.1 += finding.entropy;
                detector_stats.2 = detector_stats.2.max(scan.completed_at);
            }
        }

        let detector_performance = detector_totals
            .into_iter()
            .map(|(detector, (matches, entropy_sum, last_updated))| {
                (
                    detector,
                    DetectorStats {
                        matches,
                        false_positive_rate: 0.0,
                        avg_entropy: if matches == 0 {
                            0.0
                        } else {
                            entropy_sum / matches as f64
                        },
                        last_updated,
                    },
                )
            })
            .collect();

        // Recent activity (last 24 hours)
        let cutoff = Utc::now() - Duration::hours(24);
        let recent_scans: Vec<_> = history.iter().filter(|s| s.completed_at > cutoff).collect();
        let scans_last_24h = recent_scans.len() as u32;
        let findings_last_24h: u32 = recent_scans
            .iter()
            .map(|scan| scan.results.findings.len() as u32)
            .sum();

        // Top repositories by actual findings, with last scan time from history.
        let mut repo_stats: HashMap<String, (u32, DateTime<Utc>, f64)> = HashMap::new();
        for scan in history.iter() {
            let entry =
                repo_stats
                    .entry(scan.repository.clone())
                    .or_insert((0, scan.completed_at, 0.0));
            entry.0 += scan.results.findings.len() as u32;
            entry.1 = entry.1.max(scan.completed_at);
            entry.2 += scan
                .results
                .findings
                .iter()
                .map(|finding| match finding.severity {
                    SecretSeverity::Critical => 10.0,
                    SecretSeverity::High => 6.0,
                    SecretSeverity::Medium => 3.0,
                    SecretSeverity::Low => 1.0,
                })
                .sum::<f64>();
        }

        let mut top_repositories: Vec<_> = repo_stats
            .into_iter()
            .map(
                |(name, (findings, last_scan, risk_score))| RepositoryStats {
                    name,
                    findings,
                    last_scan,
                    risk_score,
                },
            )
            .collect();
        top_repositories.sort_by_key(|repo| std::cmp::Reverse(repo.findings));
        top_repositories.truncate(10);

        let mut trending_detector_counts: HashMap<String, u32> = HashMap::new();
        for scan in &recent_scans {
            for finding in &scan.results.findings {
                *trending_detector_counts
                    .entry(finding.detector_name.clone())
                    .or_insert(0) += 1;
            }
        }
        let mut trending_detectors: Vec<_> = trending_detector_counts.into_iter().collect();
        trending_detectors.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        let trending_detectors = trending_detectors
            .into_iter()
            .take(10)
            .map(|(detector, _)| detector)
            .collect();

        ScanStatistics {
            total_scans,
            repositories_scanned,
            total_findings,
            verified_findings,
            false_positives,
            avg_scan_time_ms,
            avg_secrets_per_repo,
            success_rate,
            severity_distribution,
            category_distribution,
            detector_performance,
            recent_activity: RecentActivity {
                scans_last_24h,
                findings_last_24h,
                top_repositories,
                trending_detectors,
            },
        }
    }

    /// Get count of currently active (running) scans
    pub async fn get_active_scans_count(&self) -> usize {
        let active_scans = self.active_scans.read().await;
        active_scans.len()
    }

    /// Get count of failed scans from history
    pub async fn get_failed_scans_count(&self) -> usize {
        let history = self.scan_history.read().await;
        history
            .iter()
            .filter(|scan| matches!(scan.status, ScanStatus::Failed))
            .count()
    }

    /// Schedule a recurring scan
    pub async fn create_schedule(
        &self,
        name: String,
        cron_expression: String,
        repositories: Vec<String>,
        config: ScanConfig,
        created_by: String,
    ) -> String {
        let schedule_id = Uuid::new_v4().to_string();

        let schedule = ScanSchedule {
            id: schedule_id.clone(),
            name,
            cron_expression: cron_expression.clone(),
            repositories,
            config,
            enabled: true,
            created_at: Utc::now(),
            last_run: None,
            next_run: self.calculate_next_run(&cron_expression),
            created_by,
        };

        let mut schedules = self.scan_schedules.write().await;
        schedules.insert(schedule_id.clone(), schedule);

        info!("Created scan schedule: {}", schedule_id);
        schedule_id
    }

    /// Calculate next run time from a standard five-field cron expression.
    fn calculate_next_run(&self, cron_expression: &str) -> DateTime<Utc> {
        next_cron_after(cron_expression, Utc::now())
            .unwrap_or_else(|| Utc::now() + Duration::hours(24))
    }

    /// Get all schedules
    pub async fn get_schedules(&self) -> Vec<ScanSchedule> {
        let schedules = self.scan_schedules.read().await;
        schedules.values().cloned().collect()
    }

    /// Cancel a running scan
    pub async fn cancel_scan(&self, scan_id: &str) -> Result<()> {
        let queued_cancel = {
            let mut active_scans = self.active_scans.write().await;
            if let Some(scan) = active_scans.get_mut(scan_id) {
                let queued = matches!(scan.status, ScanStatus::Queued);
                scan.status = ScanStatus::Cancelled;
                queued
            } else {
                return Err(anyhow!("Scan not found or already completed"));
            }
        };

        if queued_cancel {
            self.handle_scan_cancellation(scan_id, "Scan was cancelled by operator request")
                .await;
        }

        info!("Cancelled scan: {}", scan_id);
        Ok(())
    }
}

// Implement Clone for ScanningService (needed for async spawning)
impl Clone for ScanningService {
    fn clone(&self) -> Self {
        Self {
            scanner: SecretScanner::new(), // Create a new scanner instance
            active_scans: self.active_scans.clone(),
            scan_history: self.scan_history.clone(),
            scan_schedules: self.scan_schedules.clone(),
            max_concurrent_scans: self.max_concurrent_scans,
            semaphore: self.semaphore.clone(),
            persistence: self.persistence.clone(),
            execution_state: self.execution_state.clone(),
            execution_signal: self.execution_signal.clone(),
        }
    }
}

fn next_cron_after(cron_expression: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let fields: Vec<&str> = cron_expression.split_whitespace().collect();
    if fields.len() != 5 {
        return None;
    }

    let minutes = parse_cron_field(fields[0], 0, 59, false)?;
    let hours = parse_cron_field(fields[1], 0, 23, false)?;
    let days = parse_cron_field(fields[2], 1, 31, false)?;
    let months = parse_cron_field(fields[3], 1, 12, false)?;
    let weekdays = parse_cron_field(fields[4], 0, 7, true)?;

    let mut candidate = after
        .checked_add_signed(Duration::minutes(1))?
        .with_second(0)?
        .with_nanosecond(0)?;

    for _ in 0..(366 * 24 * 60) {
        let weekday = candidate.weekday().num_days_from_sunday();
        if minutes.contains(&candidate.minute())
            && hours.contains(&candidate.hour())
            && days.contains(&candidate.day())
            && months.contains(&candidate.month())
            && weekdays.contains(&weekday)
        {
            return Some(candidate);
        }

        candidate = candidate.checked_add_signed(Duration::minutes(1))?;
    }

    None
}

fn parse_cron_field(
    field: &str,
    min: u32,
    max: u32,
    normalize_weekday: bool,
) -> Option<HashSet<u32>> {
    let mut values = HashSet::new();

    for segment in field.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            return None;
        }

        let (range_part, step) = if let Some((range, step)) = segment.split_once('/') {
            let step = step.parse::<u32>().ok()?;
            if step == 0 {
                return None;
            }
            (range, step)
        } else {
            (segment, 1)
        };

        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some((start, end)) = range_part.split_once('-') {
            (start.parse::<u32>().ok()?, end.parse::<u32>().ok()?)
        } else {
            let value = range_part.parse::<u32>().ok()?;
            (value, value)
        };

        if start < min || end > max || start > end {
            return None;
        }

        for value in (start..=end).step_by(step as usize) {
            let normalized = if normalize_weekday && value == 7 {
                0
            } else {
                value
            };
            values.insert(normalized);
        }
    }

    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanning::domain::ScanInitiator;
    use crate::scanning::trufflehog::{GitData, GitInfo, SourceMetadata, TruffleHogFinding};
    use chrono::{TimeZone, Utc};
    use std::collections::HashSet;

    #[test]
    fn map_detector_to_severity_category_samples() {
        let lower = "AWS Access Key ID".to_lowercase();
        assert!(
            lower.contains("key"),
            "expected lowered detector name to contain 'key', got {}",
            lower
        );

        let (sev, cat) = ScanningService::map_detector_to_severity_category("AWS Access Key ID");
        assert_eq!(sev, SecretSeverity::High);
        assert_eq!(cat, SecretCategory::CloudProvider);

        let (sev, cat) = ScanningService::map_detector_to_severity_category("GitHub Token");
        assert_eq!(sev, SecretSeverity::High);
        assert_eq!(cat, SecretCategory::Token);

        let (sev, cat) = ScanningService::map_detector_to_severity_category("Private Key");
        assert_eq!(sev, SecretSeverity::Critical);
        assert_eq!(cat, SecretCategory::PrivateKey);

        let (sev, cat) = ScanningService::map_detector_to_severity_category("Slack Webhook");
        assert_eq!(sev, SecretSeverity::High);
        assert_eq!(cat, SecretCategory::Webhook);

        let (sev, cat) = ScanningService::map_detector_to_severity_category("Stripe API Key");
        assert_eq!(sev, SecretSeverity::High);
        assert_eq!(cat, SecretCategory::ApiKey);
    }

    #[test]
    fn append_findings_to_results_maps_metadata() {
        let service = ScanningService::new(1);
        let mut total_secrets = Vec::new();
        let mut unique_files = HashSet::new();
        let mut total_lines = 0usize;

        let event = EventCommit {
            event_id: 42,
            repository: "owner/repo".to_string(),
            repository_url: Some("https://github.com/owner/repo".to_string()),
            before_sha: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
            head_sha: Some("fedcba0987654321fedcba0987654321fedcba09".to_string()),
            reference: Some("refs/heads/main".to_string()),
            forced: false,
            is_zero_commit: false,
            commit_count: 1,
            created_at: Utc::now(),
        };

        let github_token = format!("ghp_{}", "A".repeat(36));
        let finding = TruffleHogFinding {
            detector_name: Some("GitHub Token".to_string()),
            decoder_name: Some("plain".to_string()),
            raw: Some(github_token),
            raw_v2: None,
            source_metadata: Some(SourceMetadata {
                data: Some(GitData {
                    git: Some(GitInfo {
                        commit: Some("abcdef1234567890abcdef1234567890abcdef12".to_string()),
                        file: Some("secrets/keys.txt".to_string()),
                        email: Some("dev@example.com".to_string()),
                        timestamp: Some("2023-01-01T00:00:00Z".to_string()),
                        line: Some(12),
                    }),
                }),
            }),
            extra_data: None,
            verified: Some(true),
        };

        service.append_findings_to_results(
            vec![finding],
            Some(&event),
            &mut total_secrets,
            &mut unique_files,
            &mut total_lines,
        );

        assert_eq!(total_secrets.len(), 1);
        let secret = &total_secrets[0];
        assert_eq!(secret.detector_name, "GitHub Token");
        assert_eq!(secret.filename.as_deref(), Some("secrets/keys.txt"));
        assert_eq!(secret.line_number, Some(12));
        assert_eq!(secret.severity, SecretSeverity::High);
        assert_eq!(secret.category, SecretCategory::Token);
        assert!(secret.context.contains("refs/heads/main"));
        assert!(secret.verified, "expected verified flag to be preserved");
        assert_eq!(total_lines, 12);
        assert!(unique_files.contains("abcdef1234567890abcdef1234567890abcdef12:secrets/keys.txt"));
        assert!(!secret.hash.is_empty(), "hash should be populated");
    }

    #[test]
    fn deleted_history_events_scan_before_commit_ref() {
        let event = EventCommit {
            event_id: 7,
            repository: "owner/repo".to_string(),
            repository_url: Some("https://github.com/owner/repo".to_string()),
            before_sha: "1111111111111111111111111111111111111111".to_string(),
            head_sha: Some("2222222222222222222222222222222222222222".to_string()),
            reference: Some("refs/heads/main".to_string()),
            forced: true,
            is_zero_commit: true,
            commit_count: 1,
            created_at: Utc::now(),
        };

        assert_eq!(
            ScanningService::resolve_branch_reference(&event),
            "1111111111111111111111111111111111111111"
        );
    }

    #[test]
    fn cron_next_run_uses_expression_not_fixed_day_offset() {
        let after = Utc
            .with_ymd_and_hms(2026, 5, 9, 10, 14, 30)
            .single()
            .expect("valid test timestamp");
        let next = next_cron_after("*/15 * * * *", after).expect("next cron time");

        assert_eq!(
            next,
            Utc.with_ymd_and_hms(2026, 5, 9, 10, 15, 0)
                .single()
                .expect("valid expected timestamp")
        );
    }

    #[tokio::test]
    async fn statistics_are_derived_from_history() {
        let service = ScanningService::new(1);
        let completed_at = Utc::now();
        let github_token = format!("ghp_{}", "A".repeat(36));
        let finding = SecretMatch {
            detector_name: "GitHub Token".to_string(),
            matched_text: github_token,
            start_position: 0,
            end_position: 24,
            line_number: Some(8),
            filename: Some("app.env".to_string()),
            entropy: 4.8,
            severity: SecretSeverity::High,
            category: SecretCategory::Token,
            context: "test".to_string(),
            verified: true,
            hash: "hash".to_string(),
        };

        service.scan_history.write().await.push(CompletedScan {
            id: "scan-1".to_string(),
            repository: "owner/repo".to_string(),
            scan_type: ScanType::Manual,
            status: ScanStatus::Completed,
            started_at: completed_at - Duration::seconds(2),
            completed_at,
            duration_ms: 2_000,
            results: ScanResults {
                findings: vec![finding],
                files_scanned: 1,
                total_lines: 8,
                scan_duration_ms: 2_000,
                severity_breakdown: HashMap::new(),
                category_breakdown: HashMap::new(),
                detector_stats: HashMap::new(),
                false_positives: 0,
                verified_findings: 1,
            },
            created_by: "analyst".to_string(),
            initiator: ScanInitiator::manual("analyst"),
            source_events: Vec::new(),
        });

        let stats = service.get_statistics().await;

        assert_eq!(
            stats.recent_activity.trending_detectors,
            vec!["GitHub Token"]
        );
        assert_eq!(stats.recent_activity.top_repositories.len(), 1);
        assert_eq!(
            stats.recent_activity.top_repositories[0].last_scan,
            completed_at
        );
        assert_eq!(stats.recent_activity.top_repositories[0].risk_score, 6.0);
    }

    fn sample_scan_job(scan_id: &str) -> ScanJob {
        ScanJob {
            id: scan_id.to_string(),
            repository: "owner/repo".to_string(),
            scan_type: ScanType::Manual,
            config: ScanConfig::default(),
            status: ScanStatus::Queued,
            started_at: Utc::now(),
            progress: ScanProgress {
                files_scanned: 0,
                total_files: 0,
                findings_found: 0,
                current_file: None,
                percentage: 0.0,
            },
            created_by: "analyst".to_string(),
            initiator: ScanInitiator::manual("analyst"),
            source_events: Vec::new(),
            event_targets: Vec::new(),
        }
    }

    #[tokio::test]
    async fn pause_gate_waits_until_resumed() {
        let service = ScanningService::new(1);
        let scan_id = "scan-1".to_string();

        {
            let mut active_scans = service.active_scans.write().await;
            active_scans.insert(scan_id.clone(), sample_scan_job(&scan_id));
        }

        service.pause_execution().await;

        let waiting_service = service.clone();
        let waiting_scan_id = scan_id.clone();
        let waiter = tokio::spawn(async move {
            waiting_service
                .wait_for_execution_window(&waiting_scan_id)
                .await
        });

        tokio::time::sleep(StdDuration::from_millis(50)).await;
        assert!(!waiter.is_finished(), "waiter should still be paused");

        service.resume_execution().await;
        assert!(waiter.await.expect("task should join").is_ok());
    }

    #[tokio::test]
    async fn shutdown_gate_cancels_active_scan() {
        let service = ScanningService::new(1);
        let scan_id = "scan-2".to_string();

        {
            let mut active_scans = service.active_scans.write().await;
            active_scans.insert(scan_id.clone(), sample_scan_job(&scan_id));
        }

        service.request_shutdown().await;
        let error = service
            .wait_for_execution_window(&scan_id)
            .await
            .expect_err("shutdown should interrupt scan execution");
        assert!(error.to_string().contains("shutdown"));
    }
}
