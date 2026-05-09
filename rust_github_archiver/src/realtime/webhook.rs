/// Webhook System for Real-Time Alerts
/// Sends notifications when secrets are detected in GitHub events
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use reqwest::{redirect::Policy, Client};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

use crate::realtime::RealTimeSecretAlert;

/// Webhook endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub url: String,
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    pub events: Vec<String>, // Event types to trigger on: ["secret_detected", "high_severity", etc.]
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub last_triggered: Option<DateTime<Utc>>,
    pub total_triggers: u64,
    pub successful_triggers: u64,
    pub failed_triggers: u64,
    pub consecutive_failures: u32,
}

impl WebhookEndpoint {
    pub fn new(url: String, secret: Option<String>, events: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            url,
            secret,
            events,
            active: true,
            created_at: Utc::now(),
            last_triggered: None,
            total_triggers: 0,
            successful_triggers: 0,
            failed_triggers: 0,
            consecutive_failures: 0,
        }
    }

    /// Check if webhook should trigger for this event type
    pub fn should_trigger(&self, event_type: &str) -> bool {
        self.active && (self.events.is_empty() || self.events.contains(&event_type.to_string()))
    }

    /// Mark successful webhook delivery
    pub fn mark_success(&mut self) {
        self.total_triggers += 1;
        self.successful_triggers += 1;
        self.consecutive_failures = 0;
        self.last_triggered = Some(Utc::now());
    }

    /// Mark failed webhook delivery
    pub fn mark_failure(&mut self) {
        self.total_triggers += 1;
        self.failed_triggers += 1;
        self.consecutive_failures += 1;
        self.last_triggered = Some(Utc::now());

        // Auto-disable after 5 consecutive failures
        if self.consecutive_failures >= 5 {
            self.active = false;
            warn!(
                "Webhook {} auto-disabled after {} consecutive failures",
                self.id, self.consecutive_failures
            );
        }
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_triggers == 0 {
            return 100.0;
        }
        (self.successful_triggers as f64 / self.total_triggers as f64) * 100.0
    }
}

/// Webhook payload for secret alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub alert_type: String,
    pub timestamp: DateTime<Utc>,
    pub alert: RealTimeSecretAlert,
    pub metadata: WebhookMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookMetadata {
    pub webhook_id: Uuid,
    pub delivery_id: Uuid,
    pub retry_count: u32,
}

/// Webhook manager for sending alerts
pub struct WebhookManager {
    endpoints: Arc<RwLock<Vec<WebhookEndpoint>>>,
    client: Client,
    max_retries: u32,
}

impl WebhookManager {
    pub fn new() -> Self {
        Self {
            endpoints: Arc::new(RwLock::new(Vec::new())),
            client: Client::builder()
                .redirect(Policy::none())
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("webhook HTTP client should build"),
            max_retries: 3,
        }
    }

    fn validate_webhook_url(url: &str) -> Result<()> {
        let parsed = Url::parse(url).map_err(|e| anyhow!("Invalid webhook URL: {}", e))?;

        if parsed.scheme() != "https" {
            return Err(anyhow!("Webhook URL must use https"));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(anyhow!("Webhook URL must not contain embedded credentials"));
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow!("Webhook URL must include a host"))?;
        let host_lower = host.to_ascii_lowercase();
        if host_lower == "localhost" || host_lower.ends_with(".localhost") {
            return Err(anyhow!("Webhook URL host is not allowed"));
        }

        if let Ok(ip) = host.parse::<IpAddr>() {
            if is_blocked_webhook_ip(ip) {
                return Err(anyhow!("Webhook URL points to a private or local address"));
            }
        }

        Ok(())
    }

    fn redacted_url_for_log(url: &str) -> String {
        match Url::parse(url) {
            Ok(mut parsed) => {
                parsed.set_query(None);
                parsed.set_fragment(None);
                parsed.to_string()
            }
            Err(_) => "<invalid-url>".to_string(),
        }
    }

    /// Add webhook endpoint
    pub async fn add_endpoint(
        &self,
        url: String,
        secret: Option<String>,
        events: Vec<String>,
    ) -> Result<Uuid> {
        Self::validate_webhook_url(&url)?;

        let mut endpoints = self.endpoints.write().await;
        let webhook = WebhookEndpoint::new(url.clone(), secret, events);
        let id = webhook.id;
        endpoints.push(webhook);
        info!(
            "Added webhook endpoint: {} ({})",
            Self::redacted_url_for_log(&url),
            id
        );
        Ok(id)
    }

    /// Remove webhook endpoint
    pub async fn remove_endpoint(&self, id: Uuid) -> Result<()> {
        let mut endpoints = self.endpoints.write().await;
        let before = endpoints.len();
        endpoints.retain(|w| w.id != id);

        if endpoints.len() == before {
            return Err(anyhow!("Webhook {} not found", id));
        }

        info!("Removed webhook endpoint: {}", id);
        Ok(())
    }

    /// Update webhook endpoint
    pub async fn update_endpoint(
        &self,
        id: Uuid,
        url: Option<String>,
        secret: Option<String>,
        events: Option<Vec<String>>,
        active: Option<bool>,
    ) -> Result<()> {
        let mut endpoints = self.endpoints.write().await;
        let webhook = endpoints
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or_else(|| anyhow!("Webhook {} not found", id))?;

        if let Some(url) = url {
            Self::validate_webhook_url(&url)?;
            webhook.url = url;
        }
        if let Some(secret) = secret {
            webhook.secret = Some(secret);
        }
        if let Some(events) = events {
            webhook.events = events;
        }
        if let Some(active) = active {
            webhook.active = active;
        }

        info!("Updated webhook endpoint: {}", id);
        Ok(())
    }

    /// Get all webhook endpoints
    pub async fn get_endpoints(&self) -> Vec<WebhookEndpoint> {
        self.endpoints.read().await.clone()
    }

    /// Send alert to all matching webhooks
    pub async fn send_alert(&self, alert: RealTimeSecretAlert, alert_type: &str) {
        let endpoints = self.endpoints.read().await;
        let active_webhooks: Vec<_> = endpoints
            .iter()
            .filter(|w| w.should_trigger(alert_type))
            .cloned()
            .collect();

        drop(endpoints); // Release read lock

        if active_webhooks.is_empty() {
            debug!("No active webhooks for alert type: {}", alert_type);
            return;
        }

        info!("Sending alert to {} webhooks", active_webhooks.len());

        // Send to all webhooks in parallel
        let futures: Vec<_> = active_webhooks
            .into_iter()
            .map(|webhook| {
                let alert = alert.clone();
                let alert_type = alert_type.to_string();
                let manager = self.clone();
                tokio::spawn(
                    async move { manager.send_to_webhook(webhook, alert, &alert_type).await },
                )
            })
            .collect();

        // Wait for all webhooks to complete
        for future in futures {
            if let Err(e) = future.await {
                error!("Webhook task failed: {}", e);
            }
        }
    }

    /// Send alert to specific webhook with retry logic
    async fn send_to_webhook(
        &self,
        webhook: WebhookEndpoint,
        alert: RealTimeSecretAlert,
        alert_type: &str,
    ) {
        let delivery_id = Uuid::new_v4();

        for retry in 0..=self.max_retries {
            let payload = WebhookPayload {
                alert_type: alert_type.to_string(),
                timestamp: Utc::now(),
                alert: alert.redacted_for_delivery(),
                metadata: WebhookMetadata {
                    webhook_id: webhook.id,
                    delivery_id,
                    retry_count: retry,
                },
            };

            match self.deliver_webhook(&webhook, &payload).await {
                Ok(_) => {
                    debug!(
                        "Webhook {} delivered successfully (delivery: {})",
                        webhook.id, delivery_id
                    );
                    self.mark_webhook_success(webhook.id).await;
                    return;
                }
                Err(e) => {
                    warn!(
                        "Webhook {} delivery failed (attempt {}/{}): {}",
                        webhook.id,
                        retry + 1,
                        self.max_retries + 1,
                        e
                    );

                    if retry < self.max_retries {
                        // Exponential backoff: 1s, 2s, 4s
                        let delay = std::time::Duration::from_secs(2u64.pow(retry));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        error!(
            "Webhook {} failed after {} retries",
            webhook.id, self.max_retries
        );
        self.mark_webhook_failure(webhook.id).await;
    }

    /// Actually deliver webhook via HTTP POST
    async fn deliver_webhook(
        &self,
        webhook: &WebhookEndpoint,
        payload: &WebhookPayload,
    ) -> Result<()> {
        let mut request = self
            .client
            .post(&webhook.url)
            .json(payload)
            .header("Content-Type", "application/json")
            .header(
                "X-Webhook-Delivery",
                payload.metadata.delivery_id.to_string(),
            )
            .header("X-Webhook-Event", &payload.alert_type);

        // Add HMAC signature if secret is configured
        if let Some(secret) = &webhook.secret {
            let signature = self.generate_signature(payload, secret)?;
            request = request.header("X-Hub-Signature-256", format!("sha256={}", signature));
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Webhook returned error status: {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Generate HMAC-SHA256 signature for webhook payload
    fn generate_signature(&self, payload: &WebhookPayload, secret: &str) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let payload_json = serde_json::to_string(payload)?;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| anyhow!("Invalid secret: {}", e))?;
        mac.update(payload_json.as_bytes());

        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }

    /// Mark webhook as successful
    async fn mark_webhook_success(&self, id: Uuid) {
        let mut endpoints = self.endpoints.write().await;
        if let Some(webhook) = endpoints.iter_mut().find(|w| w.id == id) {
            webhook.mark_success();
        }
    }

    /// Mark webhook as failed
    async fn mark_webhook_failure(&self, id: Uuid) {
        let mut endpoints = self.endpoints.write().await;
        if let Some(webhook) = endpoints.iter_mut().find(|w| w.id == id) {
            webhook.mark_failure();
        }
    }

    /// Get webhook statistics
    pub async fn get_stats(&self) -> WebhookStats {
        let endpoints = self.endpoints.read().await;

        let total_webhooks = endpoints.len();
        let active_webhooks = endpoints.iter().filter(|w| w.active).count();
        let total_triggers: u64 = endpoints.iter().map(|w| w.total_triggers).sum();
        let successful_triggers: u64 = endpoints.iter().map(|w| w.successful_triggers).sum();
        let failed_triggers: u64 = endpoints.iter().map(|w| w.failed_triggers).sum();

        let overall_success_rate = if total_triggers > 0 {
            (successful_triggers as f64 / total_triggers as f64) * 100.0
        } else {
            100.0
        };

        WebhookStats {
            total_webhooks,
            active_webhooks,
            total_triggers,
            successful_triggers,
            failed_triggers,
            overall_success_rate,
        }
    }
}

impl Clone for WebhookManager {
    fn clone(&self) -> Self {
        Self {
            endpoints: self.endpoints.clone(),
            client: Client::builder()
                .redirect(Policy::none())
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("webhook HTTP client should build"),
            max_retries: self.max_retries,
        }
    }
}

fn is_blocked_webhook_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookStats {
    pub total_webhooks: usize,
    pub active_webhooks: usize,
    pub total_triggers: u64,
    pub successful_triggers: u64,
    pub failed_triggers: u64,
    pub overall_success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webhook_manager() {
        let manager = WebhookManager::new();

        let id = manager
            .add_endpoint(
                "https://example.com/webhook".to_string(),
                Some("secret123".to_string()),
                vec!["secret_detected".to_string()],
            )
            .await
            .expect("webhook endpoint");

        let endpoints = manager.get_endpoints().await;
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].id, id);
        assert!(endpoints[0].active);

        manager.remove_endpoint(id).await.unwrap();
        let endpoints = manager.get_endpoints().await;
        assert_eq!(endpoints.len(), 0);
    }

    #[tokio::test]
    async fn test_webhook_should_trigger() {
        let webhook = WebhookEndpoint::new(
            "https://example.com/webhook".to_string(),
            None,
            vec!["secret_detected".to_string(), "high_severity".to_string()],
        );

        assert!(webhook.should_trigger("secret_detected"));
        assert!(webhook.should_trigger("high_severity"));
        assert!(!webhook.should_trigger("low_severity"));
    }

    #[tokio::test]
    async fn test_webhook_auto_disable() {
        let mut webhook =
            WebhookEndpoint::new("https://example.com/webhook".to_string(), None, vec![]);

        assert!(webhook.active);

        for _ in 0..5 {
            webhook.mark_failure();
        }

        assert!(!webhook.active);
        assert_eq!(webhook.consecutive_failures, 5);
    }

    // --- EXPANDED COMPREHENSIVE TESTS ---

    #[tokio::test]
    async fn test_webhook_endpoint_creation() {
        let endpoint = WebhookEndpoint::new(
            "https://example.com/webhook".to_string(),
            Some("secret123".to_string()),
            vec!["secret_detected".to_string()],
        );

        assert!(!endpoint.url.is_empty());
        assert!(endpoint.secret.is_some());
        assert_eq!(endpoint.secret.unwrap(), "secret123");
        assert_eq!(endpoint.events.len(), 1);
        assert!(endpoint.active);
        assert_eq!(endpoint.total_triggers, 0);
        assert_eq!(endpoint.consecutive_failures, 0);
    }

    #[test]
    fn webhook_endpoint_serialization_redacts_secret() {
        let endpoint = WebhookEndpoint::new(
            "https://example.com/webhook?token=secret".to_string(),
            Some("secret123".to_string()),
            vec!["secret_detected".to_string()],
        );

        let serialized = serde_json::to_string(&endpoint).expect("serialize endpoint");

        assert!(!serialized.contains("secret123"));
        assert!(!serialized.contains("\"secret\""));
    }

    #[test]
    fn webhook_url_validation_rejects_ssrf_targets() {
        assert!(WebhookManager::validate_webhook_url("http://example.com/hook").is_err());
        assert!(WebhookManager::validate_webhook_url("https://127.0.0.1/hook").is_err());
        assert!(WebhookManager::validate_webhook_url("https://10.0.0.1/hook").is_err());
        assert!(WebhookManager::validate_webhook_url("https://localhost/hook").is_err());
        assert!(
            WebhookManager::validate_webhook_url("https://user:pass@example.com/hook").is_err()
        );
        assert!(WebhookManager::validate_webhook_url("https://example.com/hook").is_ok());
    }

    #[tokio::test]
    async fn test_webhook_mark_success() {
        let mut webhook =
            WebhookEndpoint::new("https://example.com/webhook".to_string(), None, vec![]);

        webhook.mark_success();

        assert_eq!(webhook.total_triggers, 1);
        assert_eq!(webhook.successful_triggers, 1);
        assert_eq!(webhook.failed_triggers, 0);
        assert_eq!(webhook.consecutive_failures, 0);
        assert!(webhook.last_triggered.is_some());
    }

    #[tokio::test]
    async fn test_webhook_mark_failure() {
        let mut webhook =
            WebhookEndpoint::new("https://example.com/webhook".to_string(), None, vec![]);

        webhook.mark_failure();

        assert_eq!(webhook.total_triggers, 1);
        assert_eq!(webhook.successful_triggers, 0);
        assert_eq!(webhook.failed_triggers, 1);
        assert_eq!(webhook.consecutive_failures, 1);
        assert!(webhook.active, "Should still be active after 1 failure");
    }

    #[tokio::test]
    async fn test_webhook_success_rate() {
        let mut webhook =
            WebhookEndpoint::new("https://example.com/webhook".to_string(), None, vec![]);

        // Initially 100%
        assert_eq!(webhook.success_rate(), 100.0);

        // 3 successes
        webhook.mark_success();
        webhook.mark_success();
        webhook.mark_success();

        // 1 failure
        webhook.mark_failure();

        // 3/4 = 75%
        assert!((webhook.success_rate() - 75.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_webhook_consecutive_failures_reset() {
        let mut webhook =
            WebhookEndpoint::new("https://example.com/webhook".to_string(), None, vec![]);

        webhook.mark_failure();
        webhook.mark_failure();
        assert_eq!(webhook.consecutive_failures, 2);

        webhook.mark_success();
        assert_eq!(
            webhook.consecutive_failures, 0,
            "Success should reset consecutive failures"
        );
    }

    #[tokio::test]
    async fn test_webhook_should_trigger_empty_events() {
        let webhook = WebhookEndpoint::new(
            "https://example.com/webhook".to_string(),
            None,
            vec![], // Empty events means trigger on all
        );

        assert!(webhook.should_trigger("any_event"));
        assert!(webhook.should_trigger("secret_detected"));
        assert!(webhook.should_trigger("high_severity"));
    }

    #[tokio::test]
    async fn test_webhook_should_not_trigger_when_inactive() {
        let mut webhook = WebhookEndpoint::new(
            "https://example.com/webhook".to_string(),
            None,
            vec!["secret_detected".to_string()],
        );

        webhook.active = false;

        assert!(!webhook.should_trigger("secret_detected"));
        assert!(!webhook.should_trigger("high_severity"));
    }

    #[tokio::test]
    async fn test_webhook_manager_new() {
        let manager = WebhookManager::new();
        let endpoints = manager.get_endpoints().await;
        assert_eq!(endpoints.len(), 0, "New manager should have no endpoints");
    }

    #[tokio::test]
    async fn test_webhook_manager_default() {
        let manager = WebhookManager::default();
        let endpoints = manager.get_endpoints().await;
        assert_eq!(
            endpoints.len(),
            0,
            "Default manager should have no endpoints"
        );
    }

    #[tokio::test]
    async fn test_webhook_manager_add_multiple_endpoints() {
        let manager = WebhookManager::new();

        let id1 = manager
            .add_endpoint(
                "https://example.com/webhook1".to_string(),
                None,
                vec!["secret_detected".to_string()],
            )
            .await
            .expect("webhook endpoint");

        let id2 = manager
            .add_endpoint(
                "https://example.com/webhook2".to_string(),
                Some("secret".to_string()),
                vec!["high_severity".to_string()],
            )
            .await
            .expect("webhook endpoint");

        let endpoints = manager.get_endpoints().await;
        assert_eq!(endpoints.len(), 2);
        assert!(endpoints.iter().any(|e| e.id == id1));
        assert!(endpoints.iter().any(|e| e.id == id2));
    }

    #[tokio::test]
    async fn test_webhook_manager_remove_nonexistent() {
        let manager = WebhookManager::new();
        let fake_id = Uuid::new_v4();

        let result = manager.remove_endpoint(fake_id).await;
        assert!(
            result.is_err(),
            "Removing nonexistent endpoint should return error"
        );
    }

    #[tokio::test]
    async fn test_webhook_manager_update_endpoint() {
        let manager = WebhookManager::new();

        let id = manager
            .add_endpoint(
                "https://example.com/webhook".to_string(),
                None,
                vec!["secret_detected".to_string()],
            )
            .await
            .expect("webhook endpoint");

        let result = manager
            .update_endpoint(
                id,
                Some("https://example.com/new-webhook".to_string()),
                None,
                Some(vec!["high_severity".to_string()]),
                None, // Keep active status
            )
            .await;

        assert!(result.is_ok());

        let endpoints = manager.get_endpoints().await;
        assert_eq!(endpoints[0].url, "https://example.com/new-webhook");
        assert_eq!(endpoints[0].events, vec!["high_severity".to_string()]);
    }

    #[tokio::test]
    async fn test_webhook_manager_update_partial_fields() {
        let manager = WebhookManager::new();

        let id = manager
            .add_endpoint(
                "https://example.com/webhook".to_string(),
                Some("old_secret".to_string()),
                vec!["secret_detected".to_string()],
            )
            .await
            .expect("webhook endpoint");

        // Update only the secret
        let result = manager
            .update_endpoint(
                id,
                None, // Keep URL same
                Some("new_secret".to_string()),
                None, // Keep events same
                None, // Keep active same
            )
            .await;

        assert!(result.is_ok());

        let endpoints = manager.get_endpoints().await;
        assert_eq!(endpoints[0].url, "https://example.com/webhook");
        assert_eq!(endpoints[0].secret, Some("new_secret".to_string()));
        assert_eq!(endpoints[0].events, vec!["secret_detected".to_string()]);
    }

    #[tokio::test]
    async fn test_webhook_manager_get_stats() {
        let manager = WebhookManager::new();

        manager
            .add_endpoint("https://example.com/webhook1".to_string(), None, vec![])
            .await
            .expect("webhook endpoint");

        manager
            .add_endpoint("https://example.com/webhook2".to_string(), None, vec![])
            .await
            .expect("webhook endpoint");

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_webhooks, 2);
        assert_eq!(stats.active_webhooks, 2);
        assert_eq!(stats.total_triggers, 0);
    }

    #[tokio::test]
    async fn test_webhook_payload_serialization() {
        use crate::realtime::{AlertSeverity, RealTimeSecretAlert, RealTimeSecretMatch};
        use crate::secrets::SecretSeverity;

        let secret_match = RealTimeSecretMatch {
            detector_name: "GitHub PAT".to_string(),
            matched_text: "ghp_REDACTED_EXAMPLE".to_string(),
            line_number: Some(42),
            filename: "config.py".to_string(),
            severity: SecretSeverity::High,
        };

        let alert = RealTimeSecretAlert {
            event_id: "12345".to_string(),
            repository: "owner/repo".to_string(),
            commit_sha: "abc123def456".to_string(),
            secrets_found: vec![secret_match],
            alert_severity: AlertSeverity::High,
            detection_time: Utc::now(),
        };

        let payload = WebhookPayload {
            alert_type: "secret_detected".to_string(),
            timestamp: Utc::now(),
            alert: alert.clone(),
            metadata: WebhookMetadata {
                webhook_id: Uuid::new_v4(),
                delivery_id: Uuid::new_v4(),
                retry_count: 0,
            },
        };

        let serialized = serde_json::to_string(&payload);
        assert!(
            serialized.is_ok(),
            "WebhookPayload should serialize to JSON"
        );

        let json = serialized.unwrap();
        assert!(json.contains("secret_detected"));
        assert!(json.contains("owner/repo"));
        assert!(json.contains("GitHub PAT"));
    }

    #[tokio::test]
    async fn test_webhook_concurrent_add() {
        use std::sync::Arc;

        let manager = Arc::new(WebhookManager::new());
        let mut handles = vec![];

        for i in 0..10 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                manager_clone
                    .add_endpoint(format!("https://example.com/webhook{}", i), None, vec![])
                    .await
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap().expect("webhook endpoint");
        }

        let endpoints = manager.get_endpoints().await;
        assert_eq!(endpoints.len(), 10, "All concurrent adds should succeed");
    }

    #[tokio::test]
    async fn test_webhook_manager_clone() {
        let manager1 = WebhookManager::new();

        let id = manager1
            .add_endpoint("https://example.com/webhook".to_string(), None, vec![])
            .await
            .expect("webhook endpoint");

        let manager2 = manager1.clone();
        let endpoints = manager2.get_endpoints().await;

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].id, id);
    }

    #[tokio::test]
    async fn test_webhook_auto_disable_exact_threshold() {
        let mut webhook =
            WebhookEndpoint::new("https://example.com/webhook".to_string(), None, vec![]);

        // 4 failures - should still be active
        for _ in 0..4 {
            webhook.mark_failure();
        }
        assert!(webhook.active, "Should be active after 4 failures");
        assert_eq!(webhook.consecutive_failures, 4);

        // 5th failure - should auto-disable
        webhook.mark_failure();
        assert!(!webhook.active, "Should be disabled after 5 failures");
        assert_eq!(webhook.consecutive_failures, 5);
    }

    #[tokio::test]
    async fn test_webhook_success_after_failures() {
        let mut webhook =
            WebhookEndpoint::new("https://example.com/webhook".to_string(), None, vec![]);

        // 3 failures
        webhook.mark_failure();
        webhook.mark_failure();
        webhook.mark_failure();

        // 2 successes
        webhook.mark_success();
        webhook.mark_success();

        assert_eq!(webhook.total_triggers, 5);
        assert_eq!(webhook.successful_triggers, 2);
        assert_eq!(webhook.failed_triggers, 3);
        assert_eq!(webhook.consecutive_failures, 0);
        assert!((webhook.success_rate() - 40.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_webhook_stats_with_mixed_activity() {
        let manager = WebhookManager::new();

        // Add 3 webhooks
        let id1 = manager
            .add_endpoint("https://example.com/1".to_string(), None, vec![])
            .await
            .expect("webhook endpoint");
        let id2 = manager
            .add_endpoint("https://example.com/2".to_string(), None, vec![])
            .await
            .expect("webhook endpoint");
        manager
            .add_endpoint("https://example.com/3".to_string(), None, vec![])
            .await
            .expect("webhook endpoint");

        assert!(
            WebhookManager::validate_webhook_url("http://127.0.0.1/hook").is_err(),
            "plain HTTP private endpoints must be rejected"
        );

        // Manually mark some triggers (would normally happen during send_alert)
        {
            let mut endpoints = manager.endpoints.write().await;
            if let Some(e) = endpoints.iter_mut().find(|e| e.id == id1) {
                e.mark_success();
                e.mark_success();
            }
            if let Some(e) = endpoints.iter_mut().find(|e| e.id == id2) {
                e.mark_failure();
            }
        }

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_webhooks, 3);
        assert_eq!(stats.active_webhooks, 3);
        assert_eq!(stats.total_triggers, 3);
        assert_eq!(stats.successful_triggers, 2);
        assert_eq!(stats.failed_triggers, 1);
    }
}
