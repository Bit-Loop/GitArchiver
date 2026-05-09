//! Monitoring API Handlers
//!
//! Provides comprehensive monitoring endpoints with REAL data from scanning service

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use chrono::{DateTime, Duration, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::SeekFrom;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time;
use tracing::{error, info};

use crate::api::state::AppState;
use crate::scanning::ScanFilter;

// Global WebSocket connection counter
static WS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

const SEVERITY_SERIES: [&str; 4] = ["Critical", "High", "Medium", "Low"];
const CATEGORY_SERIES: [&str; 8] = [
    "API Keys",
    "Access Tokens",
    "Passwords",
    "Certificates",
    "Private Keys",
    "Database URLs",
    "URLs",
    "Other",
];
const MAX_LOG_READ_BYTES: u64 = 2 * 1024 * 1024;

static GITHUB_TOKEN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"gh[psuor]_[A-Za-z0-9_]{20,}").expect("valid GitHub token regex"));
static SECRET_ASSIGNMENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)\b(github_token|authorization|api[_-]?key|token|password|secret)(["'\s:=]+)([^"',\s}\]]+)"#,
    )
    .expect("valid secret assignment regex")
});

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionOverview {
    // Secret counts
    pub total_secrets: u64,
    pub critical_secrets: u64,
    pub high_secrets: u64,
    pub medium_secrets: u64,
    pub low_secrets: u64,
    pub verified_secrets: u64,
    pub false_positives: u64,

    // Scan statistics
    pub total_scans: u64,
    pub repositories_scanned: u64,
    pub files_scanned: u64,
    pub active_scans: u64,
    pub failed_scans: u64,
    pub scan_success_rate: f64,
    pub scan_rate_per_minute: f64,
    pub repos_per_minute: f64,
    pub avg_scan_duration_ms: u64,
    pub last_scan_time: Option<DateTime<Utc>>,

    // Distributions
    pub severity_distribution: HashMap<String, u64>,
    pub category_distribution: HashMap<String, u64>,

    // Top repositories by risk
    pub top_repositories: Vec<RepositoryRisk>,

    // Recent detections
    pub recent_detections: Vec<RecentDetection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryRisk {
    pub repository: String,
    pub total_secrets: u64,
    pub critical_count: u64,
    pub high_count: u64,
    pub risk_score: f64,
    pub last_scanned: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentDetection {
    pub id: String,
    pub repository: String,
    pub file_path: Option<String>,
    pub detector: String,
    pub severity: String,
    pub category: String,
    pub detected_at: DateTime<Utc>,
    pub verified: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetectionTrends {
    pub period: String,
    pub data_points: Vec<TrendDataPoint>,
    pub growth_rate: f64,
    pub severity_trends: HashMap<String, Vec<u64>>,
    pub category_trends: HashMap<String, Vec<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendDataPoint {
    pub timestamp: DateTime<Utc>,
    pub count: u64,
    pub critical: u64,
    pub high: u64,
    pub medium: u64,
    pub low: u64,
}

#[derive(Debug, Deserialize)]
pub struct TrendsQueryParams {
    pub period: Option<String>, // "24h", "7d", "30d", "90d"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemLogs {
    pub logs: Vec<LogEntry>,
    pub total_count: u64,
    pub page: u64,
    pub page_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub category: String,
    pub message: String,
    pub source: String,
    pub trace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogsQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub level: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RealTimeMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub active_scans: u64,
    pub secrets_per_minute: u64,
    pub websocket_connections: u64,
    pub database_connections: u64,
    pub timestamp: DateTime<Utc>,
    // Additional metrics for frontend display
    pub requests_per_second: f64,
    pub avg_response_time_ms: f64,
    pub error_rate_percent: f64,
    pub db_queries_per_second: f64,
    pub cache_hit_rate_percent: f64,
    pub events_processed_total: u64,
    pub critical_alerts: u64,
    pub medium_alerts: u64,
}

// ============================================================================
// API Handlers
// ============================================================================

/// Get comprehensive detection overview from real scanning data
pub async fn get_detection_overview(
    State(app_state): State<AppState>,
) -> Result<Json<DetectionOverview>, StatusCode> {
    info!("Fetching detection overview from persistent dashboard tables");

    let dashboard = app_state
        .persistence
        .secret_dashboard_data(15, 25)
        .await
        .map_err(|e| {
            error!("Failed to load dashboard overview: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let overview = dashboard.overview;

    let mut severity_distribution: HashMap<String, u64> = HashMap::new();
    for (label, count) in overview.severity_counts.iter() {
        severity_distribution.insert(label.clone(), *count as u64);
    }

    let mut category_distribution: HashMap<String, u64> = HashMap::new();
    for (label, count) in overview.category_counts.iter() {
        category_distribution.insert(label.clone(), *count as u64);
    }

    let top_repositories: Vec<RepositoryRisk> = dashboard
        .top_repositories
        .into_iter()
        .map(|repo| RepositoryRisk {
            repository: repo.repository,
            total_secrets: repo.total_secrets as u64,
            critical_count: repo.critical_count as u64,
            high_count: repo.high_count as u64,
            risk_score: repo.risk_score,
            last_scanned: repo.last_detected,
        })
        .collect();

    let recent_detections: Vec<RecentDetection> = dashboard
        .recent_detections
        .into_iter()
        .map(|detection| RecentDetection {
            id: detection.detection_id.to_string(),
            repository: detection.repository,
            file_path: detection.file_path,
            detector: detection.detector_name,
            severity: detection.severity,
            category: detection.category,
            detected_at: detection.detected_at,
            verified: detection.verified,
        })
        .collect();

    let scanner_stats = app_state.scanning_service.get_statistics().await;
    let active_metrics = app_state.scanning_service.get_active_scan_metrics().await;
    let recent_history = app_state
        .scanning_service
        .get_scan_results(ScanFilter {
            date_from: Some(Utc::now() - Duration::minutes(15)),
            date_to: None,
            repository: None,
            severity: None,
            category: None,
            detector: None,
            verified_only: None,
            limit: Some(500),
            offset: None,
        })
        .await;

    if severity_distribution.is_empty() {
        severity_distribution = scanner_stats
            .severity_distribution
            .iter()
            .map(|(label, count)| (label.clone(), *count as u64))
            .collect();
    }

    if category_distribution.is_empty() {
        category_distribution = scanner_stats
            .category_distribution
            .iter()
            .map(|(label, count)| (label.clone(), *count as u64))
            .collect();
    }

    let total_from_severity: u64 = severity_distribution.values().sum();
    let total_secrets = if overview.total_secrets > 0 {
        overview.total_secrets as u64
    } else if total_from_severity > 0 {
        total_from_severity
    } else {
        scanner_stats.total_findings as u64
    };

    let verified_secrets = if overview.verified_secrets > 0 {
        overview.verified_secrets as u64
    } else {
        scanner_stats.verified_findings as u64
    };

    let false_positives = if overview.false_positives > 0 {
        overview.false_positives as u64
    } else {
        scanner_stats.false_positives as u64
    };

    let total_scans = if overview.total_scans > 0 {
        overview.total_scans as u64
    } else {
        scanner_stats.total_scans as u64
    };

    let repositories_scanned = if overview.repositories_scanned > 0 {
        overview.repositories_scanned as u64
    } else {
        let repo_set: HashSet<_> = recent_history
            .iter()
            .map(|s| s.repository.clone())
            .collect();
        repo_set.len() as u64
    };

    let files_scanned = if overview.files_scanned > 0 {
        overview.files_scanned as u64
    } else {
        recent_history
            .iter()
            .map(|s| s.results.files_scanned as u64)
            .sum()
    };

    let avg_from_overview = overview.avg_scan_duration_ms.unwrap_or(0).max(0) as u64;
    let avg_from_history = if !recent_history.is_empty() {
        recent_history.iter().map(|s| s.duration_ms).sum::<u64>() / recent_history.len() as u64
    } else {
        0
    };
    let avg_scan_duration_ms = if avg_from_overview > 0 {
        avg_from_overview
    } else if scanner_stats.avg_scan_time_ms > 0 {
        scanner_stats.avg_scan_time_ms
    } else {
        avg_from_history
    };

    let recent_scan_rate = if !recent_history.is_empty() {
        recent_history.len() as f64 / 15.0
    } else {
        0.0
    };

    let recent_repo_rate = if !recent_history.is_empty() {
        let repo_set: HashSet<_> = recent_history
            .iter()
            .map(|s| s.repository.clone())
            .collect();
        repo_set.len() as f64 / 15.0
    } else {
        0.0
    };

    let scan_rate_per_minute = if overview.scan_rate_per_minute > 0.0 {
        overview.scan_rate_per_minute
    } else if recent_scan_rate > 0.0 {
        recent_scan_rate
    } else {
        0.0
    };

    let repos_per_minute = if overview.repos_per_minute > 0.0 {
        overview.repos_per_minute
    } else {
        recent_repo_rate
    };

    let scan_success_rate = if overview.total_scans > 0 {
        overview.scan_success_rate
    } else if scanner_stats.total_scans > 0 {
        scanner_stats.success_rate
    } else {
        0.0
    };

    let last_scan_time = overview
        .last_scan_time
        .or_else(|| recent_history.iter().map(|s| s.completed_at).max());

    let active_scans = if overview.active_scans > 0 {
        overview.active_scans as u64
    } else {
        active_metrics.active_scans as u64
    };

    let failed_scans = if overview.failed_scans > 0 {
        overview.failed_scans as u64
    } else {
        recent_history
            .iter()
            .filter(|scan| matches!(scan.status, crate::scanning::ScanStatus::Failed))
            .count() as u64
    };

    let overview_response = DetectionOverview {
        total_secrets,
        critical_secrets: *severity_distribution.get("Critical").unwrap_or(&0),
        high_secrets: *severity_distribution.get("High").unwrap_or(&0),
        medium_secrets: *severity_distribution.get("Medium").unwrap_or(&0),
        low_secrets: *severity_distribution.get("Low").unwrap_or(&0),
        verified_secrets,
        false_positives,
        total_scans,
        repositories_scanned,
        files_scanned,
        active_scans,
        failed_scans,
        scan_success_rate,
        scan_rate_per_minute,
        repos_per_minute,
        avg_scan_duration_ms,
        last_scan_time,
        severity_distribution,
        category_distribution,
        top_repositories,
        recent_detections,
    };

    Ok(Json(overview_response))
}

/// Get detection trends over time from real scanning history
pub async fn get_detection_trends(
    State(app_state): State<AppState>,
    Query(params): Query<TrendsQueryParams>,
) -> Result<Json<DetectionTrends>, StatusCode> {
    let period = params.period.as_deref().unwrap_or("7d");
    info!("Fetching detection trends for period: {}", period);

    // Calculate time window
    let (start_time, granularity_hours, num_buckets): (DateTime<Utc>, i64, usize) = match period {
        "24h" => (Utc::now() - Duration::hours(24), 1, 24),
        "7d" => (Utc::now() - Duration::days(7), 6, 28),
        "30d" => (Utc::now() - Duration::days(30), 24, 30),
        "90d" => (Utc::now() - Duration::days(90), 72, 30),
        _ => (Utc::now() - Duration::days(7), 6, 28),
    };

    let samples = app_state
        .persistence
        .secret_trend_samples(start_time)
        .await
        .map_err(|e| {
            error!("Failed to load trend samples: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut data_points: Vec<TrendDataPoint> = Vec::with_capacity(num_buckets);
    let mut severity_trends: HashMap<String, Vec<u64>> = SEVERITY_SERIES
        .iter()
        .map(|label| (label.to_string(), Vec::with_capacity(num_buckets)))
        .collect();
    let mut category_trends: HashMap<String, Vec<u64>> = CATEGORY_SERIES
        .iter()
        .map(|label| (label.to_string(), Vec::with_capacity(num_buckets)))
        .collect();

    for bucket_index in 0..num_buckets {
        let bucket_start = start_time + Duration::hours(granularity_hours * bucket_index as i64);
        let bucket_end = bucket_start + Duration::hours(granularity_hours);

        let mut severity_counts: HashMap<String, u64> = SEVERITY_SERIES
            .iter()
            .map(|label| (label.to_string(), 0))
            .collect();
        let mut category_counts: HashMap<String, u64> = CATEGORY_SERIES
            .iter()
            .map(|label| (label.to_string(), 0))
            .collect();

        for sample in samples
            .iter()
            .filter(|s| s.detected_at >= bucket_start && s.detected_at < bucket_end)
        {
            *severity_counts.entry(sample.severity.clone()).or_insert(0) += 1;
            *category_counts.entry(sample.category.clone()).or_insert(0) += 1;
        }

        let critical = *severity_counts.get("Critical").unwrap_or(&0);
        let high = *severity_counts.get("High").unwrap_or(&0);
        let medium = *severity_counts.get("Medium").unwrap_or(&0);
        let low = *severity_counts.get("Low").unwrap_or(&0);
        let total = critical + high + medium + low;

        data_points.push(TrendDataPoint {
            timestamp: bucket_start,
            count: total,
            critical,
            high,
            medium,
            low,
        });

        for label in SEVERITY_SERIES {
            let value = *severity_counts.get(label).unwrap_or(&0);
            if let Some(series) = severity_trends.get_mut(label) {
                series.push(value);
            }
        }

        for label in CATEGORY_SERIES {
            let value = *category_counts.get(label).unwrap_or(&0);
            if let Some(series) = category_trends.get_mut(label) {
                series.push(value);
            }
        }
    }

    let growth_rate = if data_points.len() >= 2 {
        let mid_point = data_points.len() / 2;
        let first_half_total: u64 = data_points[..mid_point].iter().map(|d| d.count).sum();
        let second_half_total: u64 = data_points[mid_point..].iter().map(|d| d.count).sum();

        if first_half_total > 0 {
            ((second_half_total as f64 - first_half_total as f64) / first_half_total as f64) * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    let trends = DetectionTrends {
        period: period.to_string(),
        data_points,
        growth_rate,
        severity_trends,
        category_trends,
    };

    Ok(Json(trends))
}

async fn read_log_tail(path: &Path) -> std::io::Result<String> {
    let metadata = tokio::fs::metadata(path).await?;
    let mut file = File::open(path).await?;

    if metadata.len() > MAX_LOG_READ_BYTES {
        file.seek(SeekFrom::Start(metadata.len() - MAX_LOG_READ_BYTES))
            .await?;
    }

    let mut content = String::new();
    file.read_to_string(&mut content).await?;

    if metadata.len() > MAX_LOG_READ_BYTES {
        if let Some(first_newline) = content.find('\n') {
            content = content[first_newline + 1..].to_string();
        }
    }

    Ok(content)
}

fn redact_log_line(line: &str) -> String {
    let redacted = GITHUB_TOKEN_RE.replace_all(line, "<redacted>");
    SECRET_ASSIGNMENT_RE
        .replace_all(&redacted, "$1$2<redacted>")
        .to_string()
}

/// Get system logs generated from configured log files, scan events, and database activity.
pub async fn get_system_logs(
    State(app_state): State<AppState>,
    Query(params): Query<LogsQueryParams>,
) -> Result<Json<SystemLogs>, StatusCode> {
    info!("Fetching system logs");

    let page = params.page.unwrap_or(1) as usize;
    let page_size = params.page_size.unwrap_or(100) as usize;

    let mut logs: Vec<LogEntry> = Vec::new();
    let configured_paths = [
        app_state.config.logging.api_log_path(),
        app_state.config.logging.main_log_path(),
        app_state.config.logging.audit_log_path(),
    ];
    let mut selected_log = None;

    for path in &configured_paths {
        match read_log_tail(path).await {
            Ok(content) if !content.trim().is_empty() => {
                selected_log = Some((path.clone(), content));
                break;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                error!(
                    "Failed to read configured log file {}: {}",
                    path.display(),
                    error
                );
            }
        }
    }

    if let Some((log_path, log_content)) = selected_log {
        let log_lines: Vec<&str> = log_content.lines().collect();
        let total_lines = log_lines.len();
        let start_idx = total_lines.saturating_sub(page_size * page);
        let end_idx = total_lines.saturating_sub(page_size * (page - 1));

        for (idx, line) in log_lines[start_idx..end_idx].iter().rev().enumerate() {
            let message = redact_log_line(line);
            // Parse log line (basic parsing for structured logs)
            let level = if message.contains("ERROR") || message.contains("ERRO") {
                "ERROR"
            } else if message.contains("WARN") {
                "WARN"
            } else if message.contains("INFO") {
                "INFO"
            } else if message.contains("DEBUG") {
                "DEBUG"
            } else {
                "INFO"
            };

            // Extract timestamp if present (ISO 8601 format)
            let timestamp = if let Some(ts_end) = line.find("Z ") {
                if ts_end > 20 {
                    let ts_start = ts_end.saturating_sub(27);
                    &line[ts_start..ts_end + 1]
                } else {
                    ""
                }
            } else {
                ""
            };

            let timestamp_parsed = if !timestamp.is_empty() {
                chrono::DateTime::parse_from_rfc3339(timestamp)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(chrono::Utc::now)
            } else {
                chrono::Utc::now()
            };

            logs.push(LogEntry {
                id: format!("log_{}", idx),
                timestamp: timestamp_parsed,
                level: level.to_string(),
                category: "System".to_string(),
                message,
                source: log_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("ConfiguredLog")
                    .to_string(),
                trace_id: None,
            });
        }
    } else {
        let configured = configured_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        logs.push(LogEntry {
            id: "info_1".to_string(),
            timestamp: chrono::Utc::now(),
            level: "INFO".to_string(),
            category: "System".to_string(),
            message: format!(
                "No configured log files have entries yet. Checked: {}.",
                configured
            ),
            source: "LogSystem".to_string(),
            trace_id: None,
        });
    }

    // Also get recent scans to generate log entries (keep existing functionality)
    let scan_filter = ScanFilter {
        repository: None,
        severity: None,
        category: None,
        detector: None,
        verified_only: None,
        date_from: None,
        date_to: None,
        limit: Some(50), // Reduced from 500 to prioritize file logs
        offset: None,
    };
    let scans = app_state
        .scanning_service
        .get_scan_results(scan_filter)
        .await;

    // Generate log entries from scan events
    for scan in scans.iter().take(20) {
        // Only take top 20 scan logs
        let status_str = match scan.status {
            crate::scanning::ScanStatus::Completed => "Completed",
            crate::scanning::ScanStatus::Failed => "Failed",
            crate::scanning::ScanStatus::Running => "Running",
            crate::scanning::ScanStatus::Cancelled => "Cancelled",
            _ => "Unknown",
        };

        let level = if matches!(scan.status, crate::scanning::ScanStatus::Failed) {
            "ERROR"
        } else {
            "INFO"
        };

        logs.push(LogEntry {
            id: format!("scan_{}", scan.repository),
            timestamp: scan.completed_at,
            level: level.to_string(),
            category: "Scan".to_string(),
            message: format!(
                "Scan {} for repository: {} - Found {} secrets in {}ms",
                status_str,
                scan.repository,
                scan.results.findings.len(),
                scan.duration_ms
            ),
            source: "ScanningService".to_string(),
            trace_id: Some(scan.repository.clone()),
        });

        // Add entries for each critical/high severity secret found
        for secret in scan.results.findings.iter().filter(|s| {
            matches!(
                s.severity,
                crate::secrets::SecretSeverity::Critical | crate::secrets::SecretSeverity::High
            )
        }) {
            let filename = secret
                .filename
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            logs.push(LogEntry {
                id: format!("secret_{}_{}", scan.repository, filename),
                timestamp: scan.completed_at, // Use scan time
                level: if matches!(secret.severity, crate::secrets::SecretSeverity::Critical) {
                    "ERROR"
                } else {
                    "WARN"
                }
                .to_string(),
                category: "Detection".to_string(),
                message: format!(
                    "{} severity {} detected in {} - {}",
                    secret.severity,
                    secret.category,
                    filename,
                    if secret.verified {
                        "VERIFIED"
                    } else {
                        "Unverified"
                    }
                ),
                source: "SecretScanner".to_string(),
                trace_id: Some(scan.repository.clone()),
            });
        }
    }

    // Sort by timestamp descending
    logs.sort_by_key(|entry| std::cmp::Reverse(entry.timestamp));

    // Apply filters
    if let Some(level) = &params.level {
        logs.retain(|log| log.level == *level);
    }
    if let Some(category) = &params.category {
        logs.retain(|log| log.category == *category);
    }
    if let Some(search) = &params.search {
        let search_lower = search.to_lowercase();
        logs.retain(|log| log.message.to_lowercase().contains(&search_lower));
    }

    let total_count = logs.len() as u64;

    // Paginate
    let start = (page.saturating_sub(1) * page_size).min(logs.len());
    let end = (start + page_size).min(logs.len());
    let paginated_logs = logs[start..end].to_vec();

    Ok(Json(SystemLogs {
        logs: paginated_logs,
        total_count,
        page: page as u64,
        page_size: page_size as u64,
    }))
}

/// Export logs to CSV format
pub async fn export_logs(
    State(app_state): State<AppState>,
    Query(params): Query<LogsQueryParams>,
) -> Response {
    info!("Exporting logs to CSV");

    // Get logs using the same logic as get_system_logs
    let logs_result = get_system_logs(
        State(app_state),
        Query(LogsQueryParams {
            page: Some(1),
            page_size: Some(10000), // Export all logs up to 10000
            level: params.level,
            category: params.category,
            search: params.search,
        }),
    )
    .await;

    match logs_result {
        Ok(Json(system_logs)) => {
            let mut csv = String::from("Timestamp,Level,Category,Message,Source,TraceID\n");

            for log in system_logs.logs {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    log.timestamp.to_rfc3339(),
                    log.level,
                    log.category,
                    log.message.replace(",", ";"), // Escape commas
                    log.source,
                    log.trace_id.unwrap_or_default()
                ));
            }

            (
                StatusCode::OK,
                [
                    ("Content-Type", "text/csv"),
                    ("Content-Disposition", "attachment; filename=logs.csv"),
                ],
                csv,
            )
                .into_response()
        }
        Err(status) => (status, "Failed to export logs").into_response(),
    }
}

/// Get real-time metrics from resource monitor and scanning service (internal)
async fn get_metrics_internal(app_state: &AppState) -> RealTimeMetrics {
    // Get resource status from tokio::sync::Mutex (Send-safe)
    let resource_status = {
        let mut monitor = app_state.resource_monitor.lock().await;
        monitor.get_resource_status().await.ok()
    };

    // Get database health
    let db_health = Some(app_state.persistence.health_status().await);

    // Get scanning statistics (not currently used but available)
    let _stats = app_state.scanning_service.get_statistics().await;

    // Get active scans count
    let active_scans = app_state.scanning_service.get_active_scans_count().await as u64;
    let collector_metrics = app_state.metrics_collector.get_metrics().await;
    let collector_uptime_seconds = app_state.metrics_collector.uptime_seconds().max(1) as f64;

    // Calculate secrets per minute from recent activity
    let one_minute_ago = Utc::now() - Duration::minutes(1);
    let recent_filter = ScanFilter {
        repository: None,
        severity: None,
        category: None,
        detector: None,
        verified_only: None,
        date_from: Some(one_minute_ago),
        date_to: None,
        limit: Some(100),
        offset: None,
    };
    let recent_scans = app_state
        .scanning_service
        .get_scan_results(recent_filter)
        .await;
    let secrets_per_minute = recent_scans
        .iter()
        .map(|s| s.results.findings.len())
        .sum::<usize>() as u64;

    // Get WebSocket connection count
    let websocket_connections = WS_CONNECTIONS.load(Ordering::Relaxed) as u64;

    // Calculate events processed total (sum of all secrets found)
    let all_filter = ScanFilter {
        repository: None,
        severity: None,
        category: None,
        detector: None,
        verified_only: None,
        date_from: None,
        date_to: None,
        limit: Some(1000),
        offset: None,
    };
    let all_scans = app_state
        .scanning_service
        .get_scan_results(all_filter)
        .await;
    let events_processed_total = all_scans
        .iter()
        .map(|s| s.results.findings.len())
        .sum::<usize>() as u64;

    RealTimeMetrics {
        cpu_usage: resource_status
            .as_ref()
            .map(|r| r.cpu.percent)
            .unwrap_or(0.0),
        memory_usage: resource_status
            .as_ref()
            .map(|r| r.memory.percent)
            .unwrap_or(0.0),
        disk_usage: resource_status
            .as_ref()
            .map(|r| r.disk.percent)
            .unwrap_or(0.0),
        active_scans,
        secrets_per_minute,
        websocket_connections,
        database_connections: db_health
            .as_ref()
            .map(|h| h.connection_count.max(0) as u64)
            .unwrap_or(0),
        timestamp: Utc::now(),
        requests_per_second: collector_metrics.api_requests as f64 / collector_uptime_seconds,
        avg_response_time_ms: collector_metrics.avg_fetch_time_ms,
        error_rate_percent: collector_metrics.error_rate(),
        db_queries_per_second: db_health
            .as_ref()
            .map(|h| h.active_queries.max(0) as f64)
            .unwrap_or(0.0),
        cache_hit_rate_percent: db_health.as_ref().map(|h| h.cache_hit_ratio).unwrap_or(0.0),
        events_processed_total,
        critical_alerts: collector_metrics.critical_severity_secrets,
        medium_alerts: collector_metrics.high_severity_secrets,
    }
}

/// Get real-time metrics HTTP handler
pub async fn get_realtime_metrics(State(app_state): State<AppState>) -> Json<RealTimeMetrics> {
    let metrics = get_metrics_internal(&app_state).await;
    Json(metrics)
}

/// WebSocket handler for real-time monitoring updates
pub async fn realtime_websocket(
    ws: WebSocketUpgrade,
    State(app_state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, app_state))
}

async fn handle_websocket(mut socket: WebSocket, app_state: AppState) {
    // Increment connection counter
    WS_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
    info!(
        "WebSocket client connected for real-time monitoring (total: {})",
        WS_CONNECTIONS.load(Ordering::Relaxed)
    );

    // Send metrics every second
    let mut interval = time::interval(time::Duration::from_secs(1));

    loop {
        interval.tick().await;

        // Get real-time metrics
        let metrics = get_metrics_internal(&app_state).await;

        let json_str = match serde_json::to_string(&metrics) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize metrics: {}", e);
                continue;
            }
        };

        if socket.send(Message::Text(json_str)).await.is_err() {
            info!("WebSocket client disconnected");
            break;
        }
    }

    // Decrement connection counter when client disconnects
    WS_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
    info!(
        "WebSocket connection closed (remaining: {})",
        WS_CONNECTIONS.load(Ordering::Relaxed)
    );
}
