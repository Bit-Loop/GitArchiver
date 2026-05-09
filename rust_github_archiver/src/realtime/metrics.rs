/// Monitoring and Metrics System
/// Implements comprehensive monitoring for GitHub Events API system
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// System metrics collector
pub struct MetricsCollector {
    metrics: Arc<RwLock<SystemMetrics>>,
    start_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    // Event metrics
    pub events_fetched: u64,
    pub events_stored: u64,
    pub events_failed: u64,
    pub events_duplicate: u64,

    // API metrics
    pub api_requests: u64,
    pub api_success: u64,
    pub api_failures: u64,
    pub api_rate_limit_hits: u64,
    pub api_304_not_modified: u64,

    // Performance metrics
    pub avg_fetch_time_ms: f64,
    pub avg_storage_time_ms: f64,
    pub p95_fetch_time_ms: f64,
    pub p99_fetch_time_ms: f64,

    // Secret detection metrics
    pub secrets_detected: u64,
    pub high_severity_secrets: u64,
    pub critical_severity_secrets: u64,

    // Webhook metrics
    pub webhooks_sent: u64,
    pub webhooks_success: u64,
    pub webhooks_failed: u64,

    // Error metrics
    pub database_errors: u64,
    pub network_errors: u64,
    pub parsing_errors: u64,

    // Resource metrics
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub disk_usage_mb: f64,

    // Time series data (last 60 minutes)
    pub events_per_minute: Vec<TimeSeriesPoint>,
    pub requests_per_minute: Vec<TimeSeriesPoint>,
    pub errors_per_minute: Vec<TimeSeriesPoint>,

    // Event type breakdown
    pub event_types: HashMap<String, u64>,

    // Last update
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: u64,
}

impl SystemMetrics {
    pub fn new() -> Self {
        Self {
            events_fetched: 0,
            events_stored: 0,
            events_failed: 0,
            events_duplicate: 0,
            api_requests: 0,
            api_success: 0,
            api_failures: 0,
            api_rate_limit_hits: 0,
            api_304_not_modified: 0,
            avg_fetch_time_ms: 0.0,
            avg_storage_time_ms: 0.0,
            p95_fetch_time_ms: 0.0,
            p99_fetch_time_ms: 0.0,
            secrets_detected: 0,
            high_severity_secrets: 0,
            critical_severity_secrets: 0,
            webhooks_sent: 0,
            webhooks_success: 0,
            webhooks_failed: 0,
            database_errors: 0,
            network_errors: 0,
            parsing_errors: 0,
            memory_usage_mb: 0.0,
            cpu_usage_percent: 0.0,
            disk_usage_mb: 0.0,
            events_per_minute: Vec::new(),
            requests_per_minute: Vec::new(),
            errors_per_minute: Vec::new(),
            event_types: HashMap::new(),
            last_updated: Utc::now(),
        }
    }

    /// Calculate overall success rate
    pub fn success_rate(&self) -> f64 {
        if self.api_requests == 0 {
            return 100.0;
        }
        (self.api_success as f64 / self.api_requests as f64) * 100.0
    }

    /// Calculate storage success rate
    pub fn storage_success_rate(&self) -> f64 {
        if self.events_fetched == 0 {
            return 100.0;
        }
        (self.events_stored as f64 / self.events_fetched as f64) * 100.0
    }

    /// Calculate error rate
    pub fn error_rate(&self) -> f64 {
        if self.api_requests == 0 {
            return 0.0;
        }
        (self.api_failures as f64 / self.api_requests as f64) * 100.0
    }

    /// Calculate events per second (last minute average)
    pub fn events_per_second(&self) -> f64 {
        if self.events_per_minute.is_empty() {
            return 0.0;
        }

        let last_minute: u64 = self.events_per_minute.iter().map(|p| p.value).sum();
        last_minute as f64 / 60.0
    }

    /// Get top event types
    pub fn top_event_types(&self, limit: usize) -> Vec<(String, u64)> {
        let mut types: Vec<_> = self
            .event_types
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        types.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        types.truncate(limit);
        types
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(SystemMetrics::new())),
            start_time: Utc::now(),
        }
    }

    /// Record API request
    pub async fn record_api_request(&self, success: bool, duration_ms: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.api_requests += 1;

        if success {
            metrics.api_success += 1;
        } else {
            metrics.api_failures += 1;
        }

        // Update average fetch time (simple moving average)
        let total = metrics.api_requests as f64;
        metrics.avg_fetch_time_ms =
            (metrics.avg_fetch_time_ms * (total - 1.0) + duration_ms) / total;

        metrics.last_updated = Utc::now();
    }

    /// Record rate limit hit
    pub async fn record_rate_limit_hit(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.api_rate_limit_hits += 1;
        metrics.last_updated = Utc::now();
    }

    /// Record 304 Not Modified response
    pub async fn record_not_modified(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.api_304_not_modified += 1;
        metrics.last_updated = Utc::now();
    }

    /// Record events fetched
    pub async fn record_events_fetched(&self, count: u64, event_type: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.events_fetched += count;

        // Update event type breakdown
        *metrics
            .event_types
            .entry(event_type.to_string())
            .or_insert(0) += count;

        metrics.last_updated = Utc::now();
    }

    /// Record events stored
    pub async fn record_events_stored(&self, count: u64, duration_ms: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.events_stored += count;

        // Update average storage time
        let total = metrics.events_stored as f64;
        if total > 0.0 {
            metrics.avg_storage_time_ms =
                (metrics.avg_storage_time_ms * (total - count as f64) + duration_ms) / total;
        }

        metrics.last_updated = Utc::now();
    }

    /// Record duplicate events
    pub async fn record_duplicate(&self, count: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.events_duplicate += count;
        metrics.last_updated = Utc::now();
    }

    /// Record secret detected
    pub async fn record_secret_detected(&self, severity: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.secrets_detected += 1;

        match severity.to_lowercase().as_str() {
            "critical" => metrics.critical_severity_secrets += 1,
            "high" => metrics.high_severity_secrets += 1,
            _ => {}
        }

        metrics.last_updated = Utc::now();
    }

    /// Record webhook sent
    pub async fn record_webhook(&self, success: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.webhooks_sent += 1;

        if success {
            metrics.webhooks_success += 1;
        } else {
            metrics.webhooks_failed += 1;
        }

        metrics.last_updated = Utc::now();
    }

    /// Record error
    pub async fn record_error(&self, error_type: &str) {
        let mut metrics = self.metrics.write().await;

        match error_type.to_lowercase().as_str() {
            "database" => metrics.database_errors += 1,
            "network" => metrics.network_errors += 1,
            "parsing" => metrics.parsing_errors += 1,
            _ => {}
        }

        metrics.last_updated = Utc::now();
    }

    /// Update resource metrics
    pub async fn update_resource_metrics(&self, memory_mb: f64, cpu_percent: f64, disk_mb: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.memory_usage_mb = memory_mb;
        metrics.cpu_usage_percent = cpu_percent;
        metrics.disk_usage_mb = disk_mb;
        metrics.last_updated = Utc::now();
    }

    /// Add time series point
    pub async fn add_time_series_point(&self, metric_type: &str, value: u64) {
        let mut metrics = self.metrics.write().await;
        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            value,
        };

        let series = match metric_type {
            "events" => &mut metrics.events_per_minute,
            "requests" => &mut metrics.requests_per_minute,
            "errors" => &mut metrics.errors_per_minute,
            _ => return,
        };

        series.push(point);

        // Keep only last 60 points (60 minutes)
        if series.len() > 60 {
            series.remove(0);
        }
    }

    /// Get current metrics snapshot
    pub async fn get_metrics(&self) -> SystemMetrics {
        self.metrics.read().await.clone()
    }

    /// Get uptime
    pub fn uptime(&self) -> Duration {
        Utc::now() - self.start_time
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> i64 {
        self.uptime().num_seconds()
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = SystemMetrics::new();
        info!("Metrics reset");
    }

    /// Get comprehensive report
    pub async fn get_report(&self) -> MetricsReport {
        let metrics = self.get_metrics().await;
        let health_status = self.calculate_health_status(&metrics);

        MetricsReport {
            uptime_seconds: self.uptime_seconds(),
            uptime_human: format_duration(self.uptime()),
            metrics,
            health_status,
        }
    }

    /// Calculate system health status
    fn calculate_health_status(&self, metrics: &SystemMetrics) -> HealthStatus {
        let error_rate = metrics.error_rate();
        let storage_rate = metrics.storage_success_rate();

        if error_rate > 10.0 || storage_rate < 90.0 {
            HealthStatus::Unhealthy
        } else if error_rate > 5.0 || storage_rate < 95.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            start_time: self.start_time,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsReport {
    pub uptime_seconds: i64,
    pub uptime_human: String,
    pub metrics: SystemMetrics,
    pub health_status: HealthStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Format duration in human-readable form
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.num_seconds();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::new();

        collector.record_api_request(true, 100.0).await;
        collector.record_api_request(true, 200.0).await;
        collector.record_api_request(false, 150.0).await;

        let metrics = collector.get_metrics().await;
        assert_eq!(metrics.api_requests, 3);
        assert_eq!(metrics.api_success, 2);
        assert_eq!(metrics.api_failures, 1);
        assert_eq!(metrics.success_rate(), 66.66666666666666);
    }

    #[tokio::test]
    async fn test_events_tracking() {
        let collector = MetricsCollector::new();

        collector.record_events_fetched(30, "PushEvent").await;
        collector.record_events_fetched(20, "IssuesEvent").await;
        collector.record_events_stored(45, 500.0).await;
        collector.record_duplicate(5).await;

        let metrics = collector.get_metrics().await;
        assert_eq!(metrics.events_fetched, 50);
        assert_eq!(metrics.events_stored, 45);
        assert_eq!(metrics.events_duplicate, 5);
        assert_eq!(metrics.event_types.get("PushEvent"), Some(&30));
        assert_eq!(metrics.event_types.get("IssuesEvent"), Some(&20));
    }

    #[tokio::test]
    async fn test_health_status() {
        let collector = MetricsCollector::new();

        // Healthy status
        collector.record_api_request(true, 100.0).await;
        let report = collector.get_report().await;
        assert_eq!(report.health_status, HealthStatus::Healthy);

        // Degraded status (6% error rate)
        for _ in 0..94 {
            collector.record_api_request(true, 100.0).await;
        }
        for _ in 0..6 {
            collector.record_api_request(false, 100.0).await;
        }

        let report = collector.get_report().await;
        assert_eq!(report.health_status, HealthStatus::Degraded);
    }
}
