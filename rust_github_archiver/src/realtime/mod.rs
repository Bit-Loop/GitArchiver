use anyhow::{anyhow, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::github::DanglingCommitFetcher;
use crate::secrets::SecretScanner;
use crate::ai::AITriageAgent;

/// Real-time GitHub event monitor
pub struct GitHubEventMonitor {
    client: Client,
    secret_scanner: SecretScanner,
    commit_fetcher: DanglingCommitFetcher,
    ai_agent: Option<AITriageAgent>,
    last_event_id: Arc<RwLock<Option<String>>>,
    webhook_endpoints: Arc<RwLock<Vec<WebhookEndpoint>>>,
    processing_queue: Arc<RwLock<Vec<GitHubEvent>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubEvent {
    pub id: String,
    pub event_type: String,
    pub created_at: DateTime<Utc>,
    pub actor: Actor,
    pub repo: Repository,
    pub payload: serde_json::Value,
    pub public: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEventPayload {
    pub push_id: u64,
    pub size: u32,
    pub distinct_size: u32,
    pub r#ref: String,
    pub head: String,
    pub before: String,
    pub commits: Vec<Commit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub author: CommitAuthor,
    pub message: String,
    pub distinct: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitAuthor {
    pub email: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeSecretAlert {
    pub event_id: String,
    pub repository: String,
    pub commit_sha: String,
    pub secrets_found: Vec<RealTimeSecretMatch>,
    pub alert_severity: AlertSeverity,
    pub detection_time: DateTime<Utc>,
    pub triage_result: Option<crate::ai::TriageResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeSecretMatch {
    pub detector_name: String,
    pub matched_text: String,
    pub line_number: Option<u32>,
    pub filename: String,
    pub severity: crate::secrets::SecretSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,  // Immediate action required
    High,      // Action required within hours
    Medium,    // Action required within days
    Low,       // Monitor
}

impl GitHubEventMonitor {
    /// Create a new real-time monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            secret_scanner: SecretScanner::new(),
            commit_fetcher: DanglingCommitFetcher::new("github_token".to_string()),
            ai_agent: None,
            last_event_id: Arc::new(RwLock::new(None)),
            webhook_endpoints: Arc::new(RwLock::new(Vec::new())),
            processing_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize with AI triage capabilities
    pub async fn with_ai_triage(mut self, ai_agent: AITriageAgent) -> Self {
        self.ai_agent = Some(ai_agent);
        self
    }

    /// Start monitoring GitHub Events API
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting GitHub Events API monitoring");

        let mut poll_interval = interval(Duration::from_secs(10)); // Poll every 10 seconds

        loop {
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
                    // Implement exponential backoff on errors
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }

    /// Poll GitHub Events API for new events
    async fn poll_events(&self) -> Result<Vec<GitHubEvent>> {
        let url = "https://api.github.com/events";
        
        let mut request_builder = self.client.get(url);
        
        // Add conditional request based on last event ID
        if let Some(last_id) = self.last_event_id.read().await.as_ref() {
            // GitHub Events API doesn't support If-Modified-Since, so we filter client-side
            debug!("Polling for events after ID: {}", last_id);
        }

        let response = request_builder
            .header("User-Agent", "GitHubArchiver/2.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("GitHub API returned status: {}", response.status()));
        }

        let events: Vec<GitHubEvent> = response.json().await?;
        
        // Filter for new events only
        let last_id = self.last_event_id.read().await.clone();
        let new_events = if let Some(last_id) = last_id {
            events.into_iter()
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

    /// Process incoming events for secret detection
    async fn process_events(&self, events: Vec<GitHubEvent>) -> Result<()> {
        // Add events to processing queue
        {
            let mut queue = self.processing_queue.write().await;
            queue.extend(events);
        }

        // Process events from queue
        self.process_queue().await?;

        Ok(())
    }

    /// Process events from the queue
    async fn process_queue(&self) -> Result<()> {
        let events = {
            let mut queue = self.processing_queue.write().await;
            let events = queue.clone();
            queue.clear();
            events
        };

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
        let payload: PushEventPayload = serde_json::from_value(event.payload)?;
        
        info!("Processing PushEvent for repo: {} (before: {})", 
              event.repo.name, payload.before);

        // Check if this is a zero-commit push (before hash with no corresponding commit)
        if payload.before != "0000000000000000000000000000000000000000" {
            match self.check_for_dangling_commit(&event.repo.name, &payload.before).await {
                Ok(Some(commit_data)) => {
                    info!("Found dangling commit: {} in {}", payload.before, event.repo.name);
                    
                    // Scan the commit for secrets
                    let secrets = self.scan_commit_for_secrets(&commit_data).await?;
                    
                    if !secrets.is_empty() {
                        let alert = self.create_secret_alert(
                            &event,
                            &payload.before,
                            secrets,
                        ).await?;
                        
                        self.send_alert(alert).await?;
                    }
                }
                Ok(None) => {
                    debug!("Commit {} exists, not dangling", payload.before);
                }
                Err(e) => {
                    warn!("Error checking commit {}: {}", payload.before, e);
                }
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
            let secrets = self.secret_scanner.scan_text(&combined_text).await?;
            
            if !secrets.is_empty() {
                info!("Found {} secrets in PR metadata", secrets.len());
                
                let alert_secrets: Vec<RealTimeSecretMatch> = secrets.into_iter()
                    .map(|s| RealTimeSecretMatch {
                        detector_name: s.detector_name,
                        matched_text: s.matched_text,
                        line_number: None,
                        filename: "PR_METADATA".to_string(),
                        severity: s.severity,
                    })
                    .collect();

                let alert = RealTimeSecretAlert {
                    event_id: event.id.clone(),
                    repository: event.repo.name.clone(),
                    commit_sha: "PR_METADATA".to_string(),
                    secrets_found: alert_secrets,
                    alert_severity: AlertSeverity::Medium,
                    detection_time: Utc::now(),
                    triage_result: None,
                };

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
                let secrets = self.secret_scanner.scan_text(body).await?;
                
                if !secrets.is_empty() {
                    info!("Found {} secrets in issue comment", secrets.len());
                    
                    let alert_secrets: Vec<RealTimeSecretMatch> = secrets.into_iter()
                        .map(|s| RealTimeSecretMatch {
                            detector_name: s.detector_name,
                            matched_text: s.matched_text,
                            line_number: None,
                            filename: "ISSUE_COMMENT".to_string(),
                            severity: s.severity,
                        })
                        .collect();

                    let alert = RealTimeSecretAlert {
                        event_id: event.id.clone(),
                        repository: event.repo.name.clone(),
                        commit_sha: "ISSUE_COMMENT".to_string(),
                        secrets_found: alert_secrets,
                        alert_severity: AlertSeverity::Low,
                        detection_time: Utc::now(),
                        triage_result: None,
                    };

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
            let name = release_data.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let body = release_data.get("body").and_then(|v| v.as_str()).unwrap_or("");
            
            let combined_text = format!("{}\n{}", name, body);
            let secrets = self.secret_scanner.scan_text(&combined_text).await?;
            
            if !secrets.is_empty() {
                info!("Found {} secrets in release", secrets.len());
                
                let alert_secrets: Vec<RealTimeSecretMatch> = secrets.into_iter()
                    .map(|s| RealTimeSecretMatch {
                        detector_name: s.detector_name,
                        matched_text: s.matched_text,
                        line_number: None,
                        filename: "RELEASE_METADATA".to_string(),
                        severity: s.severity,
                    })
                    .collect();

                let alert = RealTimeSecretAlert {
                    event_id: event.id.clone(),
                    repository: event.repo.name.clone(),
                    commit_sha: "RELEASE_METADATA".to_string(),
                    secrets_found: alert_secrets,
                    alert_severity: AlertSeverity::Medium,
                    detection_time: Utc::now(),
                    triage_result: None,
                };

                self.send_alert(alert).await?;
            }
        }

        Ok(())
    }

    /// Check if a commit is dangling (not accessible via API)
    async fn check_for_dangling_commit(&self, repo_name: &str, commit_sha: &str) -> Result<Option<String>> {
        // Try to fetch the commit - if it fails with 404, it's likely dangling
        match self.commit_fetcher.fetch_commit(repo_name, commit_sha).await {
            Ok(commit_data) => Ok(Some(commit_data)),
            Err(e) => {
                if e.to_string().contains("404") {
                    // This is likely a dangling commit
                    info!("Potential dangling commit found: {} in {}", commit_sha, repo_name);
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Scan commit data for secrets
    async fn scan_commit_for_secrets(&self, commit_data: &str) -> Result<Vec<crate::secrets::SecretMatch>> {
        self.secret_scanner.scan_text(commit_data).await
    }

    /// Create a secret alert
    async fn create_secret_alert(
        &self,
        event: &GitHubEvent,
        commit_sha: &str,
        secrets: Vec<crate::secrets::SecretMatch>,
    ) -> Result<RealTimeSecretAlert> {
        let alert_secrets: Vec<RealTimeSecretMatch> = secrets.iter()
            .map(|s| RealTimeSecretMatch {
                detector_name: s.detector_name.clone(),
                matched_text: s.matched_text.clone(),
                line_number: s.line_number,
                filename: s.filename.clone().unwrap_or("UNKNOWN".to_string()),
                severity: s.severity.clone(),
            })
            .collect();

        // Determine alert severity based on secret severities
        let alert_severity = if secrets.iter().any(|s| matches!(s.severity, crate::secrets::SecretSeverity::Critical)) {
            AlertSeverity::Critical
        } else if secrets.iter().any(|s| matches!(s.severity, crate::secrets::SecretSeverity::High)) {
            AlertSeverity::High
        } else if secrets.iter().any(|s| matches!(s.severity, crate::secrets::SecretSeverity::Medium)) {
            AlertSeverity::Medium
        } else {
            AlertSeverity::Low
        };

        // Use AI triage if available
        let triage_result = if let Some(ai_agent) = &self.ai_agent {
            if let Some(secret) = secrets.first() {
                let context = crate::ai::TriageContext {
                    repository_name: event.repo.name.clone(),
                    organization: Some(event.actor.login.clone()),
                    is_public_repository: event.public,
                    recent_activity: true,
                    contributor_count: None,
                    star_count: None,
                };
                
                // Note: This would need a mutable reference in practice
                // ai_agent.triage_secret(secret, None, &context).await.ok()
                None
            } else {
                None
            }
        } else {
            None
        };

        Ok(RealTimeSecretAlert {
            event_id: event.id.clone(),
            repository: event.repo.name.clone(),
            commit_sha: commit_sha.to_string(),
            secrets_found: alert_secrets,
            alert_severity,
            detection_time: Utc::now(),
            triage_result,
        })
    }

    /// Send alert to configured endpoints
    async fn send_alert(&self, alert: RealTimeSecretAlert) -> Result<()> {
        info!("Sending alert for {} secrets in repo: {}", 
              alert.secrets_found.len(), alert.repository);

        // Log the alert
        match alert.alert_severity {
            AlertSeverity::Critical => {
                error!("🚨 CRITICAL SECRET ALERT: {} in {}", 
                       alert.secrets_found.len(), alert.repository);
            }
            AlertSeverity::High => {
                warn!("⚠️ HIGH PRIORITY SECRET ALERT: {} in {}", 
                      alert.secrets_found.len(), alert.repository);
            }
            AlertSeverity::Medium => {
                info!("⚡ MEDIUM PRIORITY SECRET ALERT: {} in {}", 
                      alert.secrets_found.len(), alert.repository);
            }
            AlertSeverity::Low => {
                debug!("📝 LOW PRIORITY SECRET ALERT: {} in {}", 
                       alert.secrets_found.len(), alert.repository);
            }
        }

        // Send to webhook endpoints
        let endpoints = self.webhook_endpoints.read().await;
        for endpoint in endpoints.iter().filter(|e| e.active) {
            match self.send_webhook(&alert, endpoint).await {
                Ok(_) => debug!("Sent alert to webhook: {}", endpoint.url),
                Err(e) => error!("Failed to send webhook to {}: {}", endpoint.url, e),
            }
        }

        Ok(())
    }

    /// Send webhook notification
    async fn send_webhook(&self, alert: &RealTimeSecretAlert, endpoint: &WebhookEndpoint) -> Result<()> {
        let payload = serde_json::to_value(alert)?;
        
        let mut request = self.client.post(&endpoint.url)
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
    fn generate_webhook_signature(&self, payload: &serde_json::Value, secret: &str) -> Result<String> {
        use sha2::{Sha256, Digest};
        use hex;

        let payload_str = serde_json::to_string(payload)?;
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hasher.update(payload_str.as_bytes());
        let result = hasher.finalize();
        
        Ok(format!("sha256={}", hex::encode(result)))
    }

    /// Add webhook endpoint
    pub async fn add_webhook_endpoint(&self, url: String, secret: Option<String>, events: Vec<String>) -> Result<Uuid> {
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
    info!("Received incoming webhook: {:?}", payload);
    // Process the incoming webhook
    Ok(StatusCode::OK)
}

/// List configured webhooks
async fn list_webhooks() -> Json<Vec<WebhookEndpoint>> {
    // This would query the actual webhook storage
    Json(vec![])
}

/// Add new webhook endpoint
async fn add_webhook(
    Json(request): Json<HashMap<String, serde_json::Value>>,
) -> Result<Json<WebhookEndpoint>, StatusCode> {
    // This would add the webhook to storage
    let endpoint = WebhookEndpoint {
        id: Uuid::new_v4(),
        url: request.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        secret: request.get("secret").and_then(|v| v.as_str()).map(|s| s.to_string()),
        events: vec!["push".to_string()],
        active: true,
        created_at: Utc::now(),
    };
    
    Ok(Json(endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_monitor_creation() {
        let monitor = GitHubEventMonitor::new();
        assert_eq!(monitor.processing_queue.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_webhook_endpoint_management() {
        let monitor = GitHubEventMonitor::new();
        
        let id = monitor.add_webhook_endpoint(
            "https://example.com/webhook".to_string(),
            Some("secret".to_string()),
            vec!["push".to_string()]
        ).await.unwrap();
        
        assert_eq!(monitor.webhook_endpoints.read().await.len(), 1);
        
        monitor.remove_webhook_endpoint(id).await.unwrap();
        assert_eq!(monitor.webhook_endpoints.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_webhook_signature_generation() {
        let monitor = GitHubEventMonitor::new();
        let payload = serde_json::json!({"test": "data"});
        let secret = "my_secret";
        
        let signature = monitor.generate_webhook_signature(&payload, secret).unwrap();
        assert!(signature.starts_with("sha256="));
        assert!(signature.len() > 10);
    }
}
