pub mod metrics;
pub mod rate_limiter;
pub mod token_pool;
pub mod webhook;

use anyhow::{anyhow, Result};
use axum::{
    // Removed unused Query and State
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::{database::PushEventQueueInsert, PersistenceService};
use crate::github::dangling_commits::CommitInfo;
use crate::github::DanglingCommitFetcher;
use crate::realtime::metrics::MetricsCollector;
use crate::scanning::trufflehog::TruffleHogFinding;
use crate::scanning::{ScanningService, TruffleHogConfig, TruffleHogScanner};
use crate::secrets::{
    redacted_preview, SecretCategory, SecretMatch, SecretScanner, SecretSeverity,
};
pub use rate_limiter::{AdaptiveRateLimiter, RateLimitStatus};

/// Real-time GitHub event monitor
pub struct GitHubEventMonitor {
    client: Client,
    github_token: String, // Store token for authenticated requests
    persistence: Option<Arc<PersistenceService>>,
    rate_limiter: AdaptiveRateLimiter,
    secret_scanner: SecretScanner,
    scanning_service: Option<Arc<ScanningService>>,
    metrics_collector: Option<Arc<MetricsCollector>>,
    commit_fetcher: Arc<tokio::sync::Mutex<DanglingCommitFetcher>>,
    organization_filter: Arc<RwLock<Vec<String>>>,
    last_event_id: Arc<RwLock<Option<String>>>,
    webhook_endpoints: Arc<RwLock<Vec<WebhookEndpoint>>>,
    processing_queue: Arc<RwLock<Vec<GitHubEvent>>>,
    events_processed: Arc<RwLock<u64>>,
    running: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub created_at: DateTime<Utc>,
    pub actor: Actor,
    pub repo: Repository,
    pub payload: serde_json::Value,
    pub public: bool,
}

const ZERO_SHA: &str = "0000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub id: u64,
    pub login: String,
    pub display_login: Option<String>,
    pub gravatar_id: Option<String>,
    pub url: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PushEventPayload {
    pub push_id: Option<u64>,
    #[serde(default)]
    pub size: u32,
    #[serde(default)]
    pub distinct_size: u32,
    #[serde(default)]
    pub created: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub forced: bool,
    #[serde(default)]
    pub commits: Vec<Commit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Commit {
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub author: Option<CommitAuthor>,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub distinct: bool,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitAuthor {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

static WEBHOOK_SERVER_ENDPOINTS: OnceCell<Arc<RwLock<Vec<WebhookEndpoint>>>> = OnceCell::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeSecretAlert {
    pub event_id: String,
    pub repository: String,
    pub commit_sha: String,
    pub secrets_found: Vec<RealTimeSecretMatch>,
    pub alert_severity: AlertSeverity,
    pub detection_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeSecretMatch {
    pub detector_name: String,
    pub matched_text: String,
    pub line_number: Option<u32>,
    pub filename: String,
    pub severity: crate::secrets::SecretSeverity,
}

impl RealTimeSecretAlert {
    pub fn redacted_for_delivery(&self) -> Self {
        let mut alert = self.clone();
        for secret in &mut alert.secrets_found {
            secret.matched_text = redacted_preview(&secret.matched_text);
        }
        alert
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical, // Immediate action required
    High,     // Action required within hours
    Medium,   // Action required within days
    Low,      // Monitor
}

impl GitHubEventMonitor {
    /// Create a new real-time monitor
    pub async fn new(github_token: &str) -> Result<Self> {
        let commit_fetcher = DanglingCommitFetcher::new(github_token, None).await?;

        Ok(Self {
            client: Client::new(),
            github_token: github_token.to_string(), // Store for authenticated requests
            persistence: None,
            rate_limiter: AdaptiveRateLimiter::new(5, true), // 5 req/min, auto-adjust enabled
            secret_scanner: SecretScanner::new(),
            scanning_service: None,
            metrics_collector: None,
            commit_fetcher: Arc::new(tokio::sync::Mutex::new(commit_fetcher)),
            organization_filter: Arc::new(RwLock::new(Vec::new())),
            last_event_id: Arc::new(RwLock::new(None)),
            webhook_endpoints: Arc::new(RwLock::new(Vec::new())),
            processing_queue: Arc::new(RwLock::new(Vec::new())),
            events_processed: Arc::new(RwLock::new(0)),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Attach persistence service for event storage and queue updates.
    pub fn with_persistence(mut self, persistence: Arc<PersistenceService>) -> Self {
        self.persistence = Some(persistence);
        self
    }

    /// Configure rate limiter settings
    pub fn with_rate_limit(mut self, requests_per_minute: u32, auto_adjust: bool) -> Self {
        self.rate_limiter = AdaptiveRateLimiter::new(requests_per_minute, auto_adjust);
        self
    }

    /// Restrict processing to repositories owned by the listed organizations/users.
    pub fn with_organizations(self, organizations: Vec<String>) -> Self {
        let normalized = organizations
            .into_iter()
            .map(|org| org.trim().to_ascii_lowercase())
            .filter(|org| !org.is_empty())
            .collect();

        if let Ok(mut filter) = self.organization_filter.try_write() {
            *filter = normalized;
        }

        self
    }

    /// Attach scanning service for persisting detections
    pub fn with_scanning_service(mut self, scanning_service: Arc<ScanningService>) -> Self {
        self.scanning_service = Some(scanning_service);
        self
    }

    /// Attach metrics collector for observability
    pub fn with_metrics_collector(mut self, metrics_collector: Arc<MetricsCollector>) -> Self {
        self.metrics_collector = Some(metrics_collector);
        self
    }

    /// Get rate limiter reference
    pub fn rate_limiter(&self) -> &AdaptiveRateLimiter {
        &self.rate_limiter
    }

    /// Check if monitor is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get events processed count
    pub async fn get_events_processed(&self) -> u64 {
        *self.events_processed.read().await
    }

    /// Start monitoring GitHub Events API
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting GitHub Events API monitoring");
        *self.running.write().await = true;

        let mut poll_interval = interval(Duration::from_secs(1)); // Check every second, rate limiter controls actual rate

        loop {
            // Check if still running
            if !*self.running.read().await {
                info!("Monitoring stopped");
                break;
            }

            poll_interval.tick().await;

            match self.poll_events().await {
                Ok(events) => {
                    if !events.is_empty() {
                        info!("Received {} new events", events.len());
                        self.process_events(events).await?;
                    }
                }
                Err(e) => {
                    error!("Error polling events: {}", e);
                    // Don't stop on errors, just wait and retry
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }

        Ok(())
    }

    /// Stop monitoring
    pub async fn stop_monitoring(&self) {
        info!("Stopping GitHub Events API monitoring");
        *self.running.write().await = false;
    }

    /// Poll GitHub Events API for new events
    async fn poll_events(&self) -> Result<Vec<GitHubEvent>> {
        // WAIT FOR RATE LIMITER - This enforces the rate limit
        info!("⏳ Waiting for rate limiter...");
        self.rate_limiter.wait_if_needed().await?;
        info!("✅ Rate limiter cleared, fetching events from GitHub API...");

        let url = "https://api.github.com/events";

        // Build GitHub API request with authentication
        let mut request_builder = self
            .client
            .get(url)
            .header("User-Agent", "GitHubArchiver/2.0")
            .header("Accept", "application/vnd.github.v3+json");

        // Add authentication if token is available
        // GitHub API uses "token" prefix for personal access tokens
        if !self.github_token.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("token {}", self.github_token));
            debug!("Using authenticated GitHub API request with token prefix");
        } else {
            warn!("No GitHub token - using unauthenticated requests (60 req/hour limit)");
        }

        // Add conditional request based on last event ID
        if let Some(last_id) = self.last_event_id.read().await.as_ref() {
            debug!("Polling for events after ID: {}", last_id);
        } else {
            info!("📡 First poll - fetching initial events");
        }

        let response = request_builder.send().await?;

        info!(
            "📥 Received response from GitHub API: {}",
            response.status()
        );

        // CHECK FOR RATE LIMITING (429)
        if response.status().as_u16() == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());

            error!(
                "🚨 Rate limited by GitHub API (429)! Retry-After: {:?} seconds",
                retry_after
            );

            // Handle rate limit response
            self.rate_limiter
                .handle_rate_limit_response(retry_after)
                .await;

            return Ok(vec![]); // Return empty, will retry after pause
        }

        if !response.status().is_success() {
            error!("❌ GitHub API error: {}", response.status());
            return Err(anyhow!("GitHub API returned status: {}", response.status()));
        }

        let events: Vec<GitHubEvent> = response.json().await?;
        info!("✅ Fetched {} events from GitHub API", events.len());

        // Filter for new events only
        let last_id = self.last_event_id.read().await.clone();
        let new_events = if let Some(last_id) = last_id {
            events
                .into_iter()
                .take_while(|event| event.id != last_id)
                .collect()
        } else {
            events
        };

        // Update last event ID
        if let Some(first_event) = new_events.first() {
            *self.last_event_id.write().await = Some(first_event.id.clone());
        }

        Ok(new_events)
    }

    /// Process incoming events for database storage and secret detection
    async fn process_events(&self, events: Vec<GitHubEvent>) -> Result<()> {
        let events = self.filter_events_by_organization(events).await;

        if events.is_empty() {
            return Ok(());
        }

        // SAVE TO DATABASE FIRST (batch insert for efficiency)
        self.save_events_to_db(&events).await?;

        // Update processed count
        *self.events_processed.write().await += events.len() as u64;

        // Add events to processing queue for secret scanning
        {
            let mut queue = self.processing_queue.write().await;
            queue.extend(events);
        }

        // Process events from queue for secrets
        self.process_queue().await?;

        Ok(())
    }

    async fn filter_events_by_organization(&self, events: Vec<GitHubEvent>) -> Vec<GitHubEvent> {
        let organizations = self.organization_filter.read().await.clone();
        if organizations.is_empty() {
            return events;
        }

        let original_count = events.len();
        let filtered = events
            .into_iter()
            .filter(|event| {
                Self::repo_matches_organization_filter(&event.repo.name, &organizations)
            })
            .collect::<Vec<_>>();

        if filtered.len() != original_count {
            debug!(
                "Filtered GitHub Events API batch by organization: {} -> {} events",
                original_count,
                filtered.len()
            );
        }

        filtered
    }

    fn repo_matches_organization_filter(repo_name: &str, organizations: &[String]) -> bool {
        repo_name
            .split_once('/')
            .map(|(owner, _)| {
                let owner = owner.to_ascii_lowercase();
                organizations.iter().any(|org| org == &owner)
            })
            .unwrap_or(false)
    }

    /// Save events to database using existing Database::insert_events_batch
    async fn save_events_to_db(&self, events: &[GitHubEvent]) -> Result<()> {
        if let Some(persistence) = &self.persistence {
            // Convert GitHubEvent to serde_json::Value for batch insert
            let event_values: Vec<serde_json::Value> = events
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?;

            let rows_inserted = persistence
                .insert_events_batch(event_values, "github_events_api")
                .await?;

            info!(
                "✅ Inserted {} events into database (github_events_api)",
                rows_inserted
            );
        } else {
            warn!("No database configured - events not persisted");
        }

        Ok(())
    }

    /// Process events from the queue
    async fn process_queue(&self) -> Result<()> {
        // Take ownership and swap with empty vec to minimize lock time
        let events = {
            let mut queue = self.processing_queue.write().await;
            std::mem::take(&mut *queue) // Moves events out, leaves empty vec
        };

        if events.is_empty() {
            return Ok(());
        }

        for event in events {
            match self.process_single_event(event).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Error processing event: {}", e);
                    // Continue processing other events
                }
            }
        }

        Ok(())
    }

    /// Process a single GitHub event
    async fn process_single_event(&self, event: GitHubEvent) -> Result<()> {
        match event.event_type.as_str() {
            "PushEvent" => self.process_push_event(event).await,
            "PullRequestEvent" => self.process_pull_request_event(event).await,
            "IssueCommentEvent" => self.process_issue_comment_event(event).await,
            "ReleaseEvent" => self.process_release_event(event).await,
            _ => {
                debug!("Ignoring event type: {}", event.event_type);
                Ok(())
            }
        }
    }

    /// Process push events for zero-commit secrets
    async fn process_push_event(&self, event: GitHubEvent) -> Result<()> {
        let payload: PushEventPayload = match serde_json::from_value(event.payload.clone()) {
            Ok(payload) => payload,
            Err(e) => {
                warn!(
                    "Skipping PushEvent {} due to malformed payload: {}",
                    event.id, e
                );
                return Ok(());
            }
        };

        let before_sha = payload
            .before
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string();
        info!(
            "Processing PushEvent for repo: {} (before: {})",
            event.repo.name, before_sha
        );

        if before_sha.is_empty() {
            debug!(
                "PushEvent {} missing before hash, skipping dangling commit check",
                event.id
            );
            return Ok(());
        }

        let is_zero_commit = Self::is_zero_commit_push(&payload);

        if is_zero_commit {
            self.enqueue_push_event_for_scanner(&event, &payload, &before_sha, true)
                .await;
        } else {
            debug!(
                "PushEvent {} is not a zero-commit force push, skipping queue",
                event.id
            );
        }

        // Only zero-commit events (force pushes) are interesting for dangling detection
        if !is_zero_commit || before_sha == ZERO_SHA {
            return Ok(());
        }

        match self
            .check_for_dangling_commit(&event.repo.name, &before_sha)
            .await
        {
            Ok(None) => {
                info!(
                    "Commit {} in {} is missing from the API and is likely dangling",
                    before_sha, event.repo.name
                );
            }
            Ok(Some(commit_data)) => {
                info!(
                    "Commit {} still reachable for {}, performing realtime scan",
                    before_sha, event.repo.name
                );

                let file_patches: Vec<String> = commit_data
                    .files
                    .iter()
                    .filter_map(|f| f.patch.as_ref())
                    .cloned()
                    .collect();
                let commit_text = format!("{}\n{}", commit_data.message, file_patches.join("\n"));

                let secrets = self
                    .scan_commit_for_secrets(&event.repo.name, &before_sha, &commit_text)
                    .await?;

                if !secrets.is_empty() {
                    let alert = self
                        .create_secret_alert(&event, &before_sha, &secrets)
                        .await?;

                    self.persist_detection(&alert, &secrets).await;
                    self.send_alert(alert).await?;
                }
            }
            Err(e) => {
                warn!("Error checking commit {}: {}", before_sha, e);
            }
        }

        Ok(())
    }

    /// Process pull request events
    async fn process_pull_request_event(&self, event: GitHubEvent) -> Result<()> {
        info!("Processing PullRequestEvent for repo: {}", event.repo.name);

        // Extract PR data from payload
        if let Some(pr_data) = event.payload.get("pull_request") {
            // Scan PR title and body for secrets
            let title = pr_data.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let body = pr_data.get("body").and_then(|v| v.as_str()).unwrap_or("");

            let combined_text = format!("{}\n{}", title, body);
            let secrets = self.secret_scanner.scan_text(&combined_text, None);

            if !secrets.is_empty() {
                info!("Found {} secrets in PR metadata", secrets.len());

                let alert_secrets: Vec<RealTimeSecretMatch> = secrets
                    .iter()
                    .map(|s| RealTimeSecretMatch {
                        detector_name: s.detector_name.clone(),
                        matched_text: s.matched_text.clone(),
                        line_number: None,
                        filename: "PR_METADATA".to_string(),
                        severity: s.severity.clone(),
                    })
                    .collect();

                let alert = RealTimeSecretAlert {
                    event_id: event.id.clone(),
                    repository: event.repo.name.clone(),
                    commit_sha: "PR_METADATA".to_string(),
                    secrets_found: alert_secrets,
                    alert_severity: AlertSeverity::Medium,
                    detection_time: Utc::now(),
                };

                self.persist_detection(&alert, &secrets).await;
                self.send_alert(alert).await?;
            }
        }

        Ok(())
    }

    /// Process issue comment events
    async fn process_issue_comment_event(&self, event: GitHubEvent) -> Result<()> {
        info!("Processing IssueCommentEvent for repo: {}", event.repo.name);

        if let Some(comment_data) = event.payload.get("comment") {
            if let Some(body) = comment_data.get("body").and_then(|v| v.as_str()) {
                let secrets = self.secret_scanner.scan_text(body, None);

                if !secrets.is_empty() {
                    info!("Found {} secrets in issue comment", secrets.len());

                    let alert_secrets: Vec<RealTimeSecretMatch> = secrets
                        .iter()
                        .map(|s| RealTimeSecretMatch {
                            detector_name: s.detector_name.clone(),
                            matched_text: s.matched_text.clone(),
                            line_number: None,
                            filename: "ISSUE_COMMENT".to_string(),
                            severity: s.severity.clone(),
                        })
                        .collect();

                    let alert = RealTimeSecretAlert {
                        event_id: event.id.clone(),
                        repository: event.repo.name.clone(),
                        commit_sha: "ISSUE_COMMENT".to_string(),
                        secrets_found: alert_secrets,
                        alert_severity: AlertSeverity::Low,
                        detection_time: Utc::now(),
                    };

                    self.persist_detection(&alert, &secrets).await;
                    self.send_alert(alert).await?;
                }
            }
        }

        Ok(())
    }

    /// Process release events
    async fn process_release_event(&self, event: GitHubEvent) -> Result<()> {
        info!("Processing ReleaseEvent for repo: {}", event.repo.name);

        if let Some(release_data) = event.payload.get("release") {
            // Scan release name and body
            let name = release_data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let body = release_data
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let combined_text = format!("{}\n{}", name, body);
            let secrets = self.secret_scanner.scan_text(&combined_text, None);

            if !secrets.is_empty() {
                info!("Found {} secrets in release", secrets.len());

                let alert_secrets: Vec<RealTimeSecretMatch> = secrets
                    .iter()
                    .map(|s| RealTimeSecretMatch {
                        detector_name: s.detector_name.clone(),
                        matched_text: s.matched_text.clone(),
                        line_number: None,
                        filename: "RELEASE_METADATA".to_string(),
                        severity: s.severity.clone(),
                    })
                    .collect();

                let alert = RealTimeSecretAlert {
                    event_id: event.id.clone(),
                    repository: event.repo.name.clone(),
                    commit_sha: "RELEASE_METADATA".to_string(),
                    secrets_found: alert_secrets,
                    alert_severity: AlertSeverity::Medium,
                    detection_time: Utc::now(),
                };

                self.persist_detection(&alert, &secrets).await;
                self.send_alert(alert).await?;
            }
        }

        Ok(())
    }

    async fn enqueue_push_event_for_scanner(
        &self,
        event: &GitHubEvent,
        payload: &PushEventPayload,
        before_sha: &str,
        is_zero_commit: bool,
    ) {
        let persistence = match &self.persistence {
            Some(persistence) => persistence,
            None => return,
        };

        let event_id = match event.id.parse::<i64>() {
            Ok(id) => id,
            Err(e) => {
                warn!("Skipping PushEvent {} due to invalid id: {}", event.id, e);
                return;
            }
        };

        if event.repo.name.trim().is_empty() {
            warn!(
                "Skipping PushEvent {} with missing repository name",
                event.id
            );
            return;
        }

        let repository_url = Self::derive_repository_url(&event.repo);
        let commit_span = payload.size.min(i32::MAX as u32) as i32;

        let insert = PushEventQueueInsert {
            event_id,
            repository_full_name: event.repo.name.clone(),
            repository_url,
            before_sha: before_sha.to_string(),
            head_sha: payload.head.clone(),
            ref_name: payload.r#ref.clone(),
            forced_flag: payload.forced,
            commit_span,
            event_created_at: event.created_at,
            is_zero_commit,
            event_payload: event.payload.clone(),
        };

        if let Err(e) = persistence.enqueue_push_event_from_monitor(insert).await {
            warn!(
                event_id = %event.id,
                repository = %event.repo.name,
                error = ?e,
                "Failed to enqueue PushEvent for scanning queue"
            );
        }
    }

    fn derive_repository_url(repo: &Repository) -> Option<String> {
        let trimmed = repo.url.trim();
        if !trimmed.is_empty() {
            if let Some(stripped) = trimmed.strip_prefix("https://api.github.com/repos/") {
                return Some(format!("https://github.com/{}", stripped));
            }
            return Some(trimmed.to_string());
        }

        let fallback = repo.name.trim().trim_matches('/');
        if fallback.is_empty() {
            None
        } else {
            Some(format!("https://github.com/{}", fallback))
        }
    }

    /// Check if a commit is dangling (not accessible via API)
    async fn check_for_dangling_commit(
        &self,
        repo_name: &str,
        commit_sha: &str,
    ) -> Result<Option<CommitInfo>> {
        // Try to fetch the commit - if it fails with 404, it's likely dangling
        let mut fetcher = self.commit_fetcher.lock().await;
        match fetcher.fetch_commit(repo_name, commit_sha).await {
            Ok(commit_data) => Ok(commit_data),
            Err(e) => {
                if e.to_string().contains("404") {
                    info!(
                        "Potential dangling commit found: {} in {}",
                        commit_sha, repo_name
                    );
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Scan commit data for secrets
    async fn scan_commit_for_secrets(
        &self,
        repository: &str,
        commit_sha: &str,
        commit_data: &str,
    ) -> Result<Vec<SecretMatch>> {
        if commit_data.trim().is_empty() {
            return Ok(Vec::new());
        }

        if TruffleHogScanner::is_available() {
            let scanner = TruffleHogScanner::new(TruffleHogConfig {
                only_verified: true,
                no_update: true,
                timeout_seconds: 180,
                binary_path: None,
            });

            match scanner.scan_buffer(commit_data).await {
                Ok(findings) if !findings.is_empty() => {
                    let converted =
                        Self::convert_trufflehog_findings(repository, commit_sha, findings);
                    if !converted.is_empty() {
                        return Ok(converted);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("Realtime TruffleHog scan failed for {}: {}", repository, e);
                }
            }
        }

        Ok(self.secret_scanner.scan_text(commit_data, Some(repository)))
    }

    fn convert_trufflehog_findings(
        repository: &str,
        commit_sha: &str,
        findings: Vec<TruffleHogFinding>,
    ) -> Vec<SecretMatch> {
        findings
            .into_iter()
            .filter_map(|finding| {
                let detector_name = finding
                    .detector_name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());
                let raw_value = finding
                    .raw
                    .or(finding.raw_v2)
                    .filter(|value| !value.is_empty())?;

                let (filename, line_number, commit) = finding
                    .source_metadata
                    .as_ref()
                    .and_then(|data| data.data.as_ref())
                    .and_then(|git| git.git.as_ref())
                    .map(|info| {
                        (
                            info.file.clone(),
                            info.line.map(|line| line as usize),
                            info.commit.clone(),
                        )
                    })
                    .unwrap_or((None, None, None));

                let commit_for_context = commit.clone().unwrap_or_else(|| commit_sha.to_string());

                let context_string =
                    format!("Repository {} • Commit {}", repository, commit_for_context);

                let (severity, category) = Self::map_detector_to_severity_category(&detector_name);

                Some(SecretMatch {
                    detector_name,
                    matched_text: raw_value.clone(),
                    start_position: 0,
                    end_position: raw_value.len(),
                    line_number,
                    filename,
                    entropy: 5.0,
                    severity,
                    category,
                    context: context_string,
                    verified: finding.verified.unwrap_or(false),
                    hash: format!("{:x}", md5::compute(&raw_value)),
                })
            })
            .collect()
    }

    fn map_detector_to_severity_category(detector_name: &str) -> (SecretSeverity, SecretCategory) {
        let lower = detector_name.to_lowercase();

        let severity = if lower.contains("private")
            || lower.contains("secret")
            || lower.contains("password")
        {
            SecretSeverity::Critical
        } else if lower.contains("token") || lower.contains("key") {
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
            SecretCategory::ApiKey
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

        (severity, category)
    }

    fn is_zero_commit_push(payload: &PushEventPayload) -> bool {
        payload.forced
            && !payload.created
            && !payload.deleted
            && payload.size == 0
            && payload.distinct_size == 0
            && payload.commits.is_empty()
            && payload
                .before
                .as_deref()
                .map(|sha| !sha.trim().is_empty() && sha != ZERO_SHA)
                .unwrap_or(false)
            && payload
                .head
                .as_deref()
                .map(|sha| !sha.trim().is_empty() && sha != ZERO_SHA)
                .unwrap_or(false)
    }

    /// Create a secret alert
    async fn create_secret_alert(
        &self,
        event: &GitHubEvent,
        commit_sha: &str,
        secrets: &[SecretMatch],
    ) -> Result<RealTimeSecretAlert> {
        let alert_secrets: Vec<RealTimeSecretMatch> = secrets
            .iter()
            .map(|s| RealTimeSecretMatch {
                detector_name: s.detector_name.clone(),
                matched_text: s.matched_text.clone(),
                line_number: s.line_number.map(|ln| ln as u32),
                filename: s.filename.clone().unwrap_or("UNKNOWN".to_string()),
                severity: s.severity.clone(),
            })
            .collect();

        // Determine alert severity based on secret severities
        let alert_severity = if secrets
            .iter()
            .any(|s| matches!(s.severity, crate::secrets::SecretSeverity::Critical))
        {
            AlertSeverity::Critical
        } else if secrets
            .iter()
            .any(|s| matches!(s.severity, crate::secrets::SecretSeverity::High))
        {
            AlertSeverity::High
        } else if secrets
            .iter()
            .any(|s| matches!(s.severity, crate::secrets::SecretSeverity::Medium))
        {
            AlertSeverity::Medium
        } else {
            AlertSeverity::Low
        };

        Ok(RealTimeSecretAlert {
            event_id: event.id.clone(),
            repository: event.repo.name.clone(),
            commit_sha: commit_sha.to_string(),
            secrets_found: alert_secrets,
            alert_severity,
            detection_time: Utc::now(),
        })
    }

    /// Send alert to configured endpoints
    async fn send_alert(&self, alert: RealTimeSecretAlert) -> Result<()> {
        info!(
            "Sending alert for {} secrets in repo: {}",
            alert.secrets_found.len(),
            alert.repository
        );

        // Log the alert
        match alert.alert_severity {
            AlertSeverity::Critical => {
                error!(
                    "🚨 CRITICAL SECRET ALERT: {} in {}",
                    alert.secrets_found.len(),
                    alert.repository
                );
            }
            AlertSeverity::High => {
                warn!(
                    "⚠️ HIGH PRIORITY SECRET ALERT: {} in {}",
                    alert.secrets_found.len(),
                    alert.repository
                );
            }
            AlertSeverity::Medium => {
                info!(
                    "⚡ MEDIUM PRIORITY SECRET ALERT: {} in {}",
                    alert.secrets_found.len(),
                    alert.repository
                );
            }
            AlertSeverity::Low => {
                debug!(
                    "📝 LOW PRIORITY SECRET ALERT: {} in {}",
                    alert.secrets_found.len(),
                    alert.repository
                );
            }
        }

        // Send to webhook endpoints
        let endpoints = self.webhook_endpoints.read().await;
        let metrics = self.metrics_collector.clone();
        for endpoint in endpoints.iter().filter(|e| e.active) {
            let result = self.send_webhook(&alert, endpoint).await;

            if let Some(ref collector) = metrics {
                collector.record_webhook(result.is_ok()).await;
            }

            match result {
                Ok(_) => debug!("Sent alert to webhook: {}", endpoint.url),
                Err(e) => error!("Failed to send webhook to {}: {}", endpoint.url, e),
            }
        }

        Ok(())
    }

    /// Persist detections into scanning service and update metrics
    async fn persist_detection(&self, alert: &RealTimeSecretAlert, matches: &[SecretMatch]) {
        if matches.is_empty() {
            return;
        }

        if let Some(scanning_service) = &self.scanning_service {
            if let Err(e) = scanning_service
                .record_realtime_detection(
                    &alert.repository,
                    matches.to_vec(),
                    alert.detection_time,
                    &alert.event_id,
                )
                .await
            {
                warn!("Failed to persist realtime detection: {}", e);
            }
        }

        if let Some(metrics) = &self.metrics_collector {
            for secret in matches {
                let severity = secret.severity.to_string();
                metrics.record_secret_detected(&severity).await;
            }
        }
    }

    /// Send webhook notification
    async fn send_webhook(
        &self,
        alert: &RealTimeSecretAlert,
        endpoint: &WebhookEndpoint,
    ) -> Result<()> {
        let payload = serde_json::to_value(alert.redacted_for_delivery())?;

        let mut request = self
            .client
            .post(&endpoint.url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "GitHubArchiver/2.0")
            .json(&payload);

        // Add webhook signature if secret is configured
        if let Some(secret) = &endpoint.secret {
            let signature = self.generate_webhook_signature(&payload, secret)?;
            request = request.header("X-Hub-Signature-256", signature);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Webhook returned status: {}", response.status()));
        }

        Ok(())
    }

    /// Generate webhook signature for security
    #[allow(dead_code)] // Used for webhook validation when feature is enabled
    fn generate_webhook_signature(
        &self,
        payload: &serde_json::Value,
        secret: &str,
    ) -> Result<String> {
        use hex;
        use sha2::{Digest, Sha256};

        let payload_str = serde_json::to_string(payload)?;
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hasher.update(payload_str.as_bytes());
        let result = hasher.finalize();

        Ok(format!("sha256={}", hex::encode(result)))
    }

    /// Add webhook endpoint
    pub async fn add_webhook_endpoint(
        &self,
        url: String,
        secret: Option<String>,
        events: Vec<String>,
    ) -> Result<Uuid> {
        let endpoint = WebhookEndpoint {
            id: Uuid::new_v4(),
            url,
            secret,
            events,
            active: true,
            created_at: Utc::now(),
        };

        let id = endpoint.id;
        self.webhook_endpoints.write().await.push(endpoint);

        Ok(id)
    }

    /// Remove webhook endpoint
    pub async fn remove_webhook_endpoint(&self, id: Uuid) -> Result<()> {
        let mut endpoints = self.webhook_endpoints.write().await;
        endpoints.retain(|e| e.id != id);
        Ok(())
    }

    /// Create webhook server
    pub fn create_webhook_server() -> Router {
        Router::new()
            .route("/webhook", post(handle_incoming_webhook))
            .route("/webhooks", get(list_webhooks))
            .route("/webhooks", post(add_webhook))
    }
}

/// Handle incoming webhook (for receiving alerts from external systems)
async fn handle_incoming_webhook(
    Json(payload): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
    info!(
        "Received incoming webhook: {:?}",
        redact_json_for_log(&payload)
    );
    // Process the incoming webhook
    Ok(StatusCode::OK)
}

fn redact_json_for_log(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_sensitive_json_key(key) {
                        (
                            key.clone(),
                            serde_json::Value::String("<redacted>".to_string()),
                        )
                    } else {
                        (key.clone(), redact_json_for_log(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(redact_json_for_log).collect())
        }
        serde_json::Value::String(text) if looks_like_secret_value(text) => {
            serde_json::Value::String(redacted_preview(text))
        }
        _ => value.clone(),
    }
}

fn is_sensitive_json_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("authorization")
        || key.contains("matched_text")
        || key == "raw"
        || key == "rawv2"
}

fn looks_like_secret_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("ghp_")
        || lower.starts_with("github_pat_")
        || lower.starts_with("sk-")
        || lower.contains("bearer ")
        || lower.contains("authorization:")
}

/// List configured webhooks
async fn list_webhooks() -> Json<Vec<WebhookEndpoint>> {
    let endpoints = webhook_server_endpoints();
    let public_endpoints = endpoints
        .read()
        .await
        .iter()
        .cloned()
        .map(|mut endpoint| {
            endpoint.secret = endpoint.secret.map(|_| "<redacted>".to_string());
            endpoint
        })
        .collect();

    Json(public_endpoints)
}

/// Add new webhook endpoint
async fn add_webhook(
    Json(request): Json<HashMap<String, serde_json::Value>>,
) -> Result<Json<WebhookEndpoint>, StatusCode> {
    let url = request
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let events = request
        .get("events")
        .and_then(|value| value.as_array())
        .map(|events| {
            events
                .iter()
                .filter_map(|event| event.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|events| !events.is_empty())
        .unwrap_or_else(|| vec!["push".to_string()]);

    let endpoint = WebhookEndpoint {
        id: Uuid::new_v4(),
        url: url.to_string(),
        secret: request
            .get("secret")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        events,
        active: true,
        created_at: Utc::now(),
    };

    webhook_server_endpoints()
        .write()
        .await
        .push(endpoint.clone());

    let mut public_endpoint = endpoint;
    public_endpoint.secret = public_endpoint.secret.map(|_| "<redacted>".to_string());
    Ok(Json(public_endpoint))
}

fn webhook_server_endpoints() -> Arc<RwLock<Vec<WebhookEndpoint>>> {
    WEBHOOK_SERVER_ENDPOINTS
        .get_or_init(|| Arc::new(RwLock::new(Vec::new())))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_monitor_creation() {
        let monitor = GitHubEventMonitor::new("fake_token").await.unwrap();
        assert_eq!(monitor.processing_queue.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_webhook_endpoint_management() {
        let monitor = GitHubEventMonitor::new("fake_token").await.unwrap();

        let id = monitor
            .add_webhook_endpoint(
                "https://example.com/webhook".to_string(),
                Some("secret".to_string()),
                vec!["push".to_string()],
            )
            .await
            .unwrap();

        assert_eq!(monitor.webhook_endpoints.read().await.len(), 1);

        monitor.remove_webhook_endpoint(id).await.unwrap();
        assert_eq!(monitor.webhook_endpoints.read().await.len(), 0);
    }

    #[test]
    fn organization_filter_matches_repository_owner_case_insensitively() {
        let organizations = vec!["github".to_string(), "rust-lang".to_string()];

        assert!(GitHubEventMonitor::repo_matches_organization_filter(
            "GitHub/docs",
            &organizations
        ));
        assert!(GitHubEventMonitor::repo_matches_organization_filter(
            "rust-lang/rust",
            &organizations
        ));
        assert!(!GitHubEventMonitor::repo_matches_organization_filter(
            "tokio-rs/tokio",
            &organizations
        ));
        assert!(!GitHubEventMonitor::repo_matches_organization_filter(
            "malformed-repo-name",
            &organizations
        ));
    }

    #[test]
    fn zero_commit_detection_requires_force_push_metadata() {
        let base_payload = PushEventPayload {
            before: Some("1111111111111111111111111111111111111111".to_string()),
            head: Some("2222222222222222222222222222222222222222".to_string()),
            ..PushEventPayload::default()
        };

        assert!(!GitHubEventMonitor::is_zero_commit_push(&base_payload));

        let mut forced_zero_commit = base_payload.clone();
        forced_zero_commit.forced = true;
        assert!(GitHubEventMonitor::is_zero_commit_push(&forced_zero_commit));

        let mut ordinary_push_with_missing_commit_list = base_payload.clone();
        ordinary_push_with_missing_commit_list.size = 2;
        assert!(!GitHubEventMonitor::is_zero_commit_push(
            &ordinary_push_with_missing_commit_list
        ));

        let mut branch_deletion = forced_zero_commit.clone();
        branch_deletion.deleted = true;
        branch_deletion.head = Some(ZERO_SHA.to_string());
        assert!(!GitHubEventMonitor::is_zero_commit_push(&branch_deletion));

        let mut branch_creation = forced_zero_commit;
        branch_creation.created = true;
        branch_creation.before = Some(ZERO_SHA.to_string());
        assert!(!GitHubEventMonitor::is_zero_commit_push(&branch_creation));
    }

    #[tokio::test]
    async fn test_webhook_signature_generation() {
        let monitor = GitHubEventMonitor::new("fake_token").await.unwrap();
        let payload = serde_json::json!({"test": "data"});
        let secret = "my_secret";

        let signature = monitor
            .generate_webhook_signature(&payload, secret)
            .unwrap();
        assert!(signature.starts_with("sha256="));
        assert!(signature.len() > 10);
    }

    #[test]
    fn realtime_alert_delivery_redacts_secret_values() {
        let alert = RealTimeSecretAlert {
            event_id: "1".to_string(),
            repository: "owner/repo".to_string(),
            commit_sha: "abc".to_string(),
            secrets_found: vec![RealTimeSecretMatch {
                detector_name: "GitHub Token".to_string(),
                matched_text: "ghp_REDACTED_EXAMPLE".to_string(),
                line_number: Some(1),
                filename: ".env".to_string(),
                severity: crate::secrets::SecretSeverity::High,
            }],
            alert_severity: AlertSeverity::High,
            detection_time: Utc::now(),
        };

        let redacted = alert.redacted_for_delivery();

        assert_ne!(
            redacted.secrets_found[0].matched_text,
            "ghp_REDACTED_EXAMPLE"
        );
        assert!(redacted.secrets_found[0]
            .matched_text
            .contains("[redacted:"));
    }
}
