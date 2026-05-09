use axum::{
    extract::{Extension, Query, State},
    http::HeaderMap,
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use tracing::{error, info};
use uuid::Uuid;

use crate::api::scanner_service::{
    create_scan_schedule, start_batch_scan, start_repository_scan, BatchScanRequest,
    BatchScanResponse, ScanRepositoryRequest, ScanResponse, ScannerServiceError,
    ScheduleScanRequest, ScheduledScanResponse,
};
use crate::api::state::AppState;
use crate::auth::User;
use crate::core::database::SecretDetectionFilter;
use crate::scanning::TruffleHogScanner;
use crate::secrets::SecretScanner;

#[derive(Debug, Deserialize, Serialize)]
pub struct ScanFiltersQuery {
    pub severity: Option<String>,
    pub category: Option<String>,
    pub repository: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ScannerMetricsResponse {
    pub status: String,
    pub active_scans: usize,
    pub files_processed: u64,
    pub events_processed: u64,
    pub processing_rate_per_minute: f64,
    pub secrets_found: u64,
    pub queue_pending: i64,
    pub queue_processing: i64,
    pub queue_failed: i64,
    pub queue_failed_forbidden: i64,
    pub queue_failed_not_found: i64,
    pub queue_completed_last_hour: i64,
    pub oldest_pending_age_seconds: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub trufflehog_available: bool,
    pub scraper_running: bool,
    pub ready: bool,
    pub issues: Vec<String>,
}

/// Return aggregated scanner metrics for the dashboard
pub async fn get_scanner_metrics(
    State(app_state): State<AppState>,
) -> Json<ScannerMetricsResponse> {
    let active_metrics = app_state.scanning_service.get_active_scan_metrics().await;
    let queue_stats = app_state
        .persistence
        .scan_queue_stats()
        .await
        .unwrap_or_default();
    let trufflehog_available = TruffleHogScanner::is_available();
    let scraper_running = app_state.scraper_manager.is_running();

    let mut issues = Vec::new();
    if !trufflehog_available {
        issues.push(
            "TruffleHog binary not found (set TRUFFLEHOG_PATH or install it in the environment)"
                .to_string(),
        );
    }
    if !scraper_running {
        issues.push("Scanner service is stopped; start or resume it to process events".to_string());
    }
    if queue_stats.pending_events > 0 && active_metrics.active_scans == 0 {
        issues.push(format!(
            "{} queued push events waiting for a scan",
            queue_stats.pending_events
        ));
    }
    if queue_stats.failed_events > 0 {
        issues.push(format!(
            "{} failed events in queue",
            queue_stats.failed_events
        ));
    }
    if queue_stats.failed_forbidden > 0 {
        issues.push(format!(
            "{} repos forbidden/invalid credentials",
            queue_stats.failed_forbidden
        ));
    }
    if queue_stats.failed_not_found > 0 {
        issues.push(format!(
            "{} repos not found/deleted",
            queue_stats.failed_not_found
        ));
    }

    let status = if active_metrics.active_scans > 0 || queue_stats.processing_events > 0 {
        "running"
    } else if queue_stats.pending_events > 0 {
        "queued"
    } else if queue_stats.failed_events > 0 {
        "attention"
    } else {
        "idle"
    };

    let ready = trufflehog_available && scraper_running;

    Json(ScannerMetricsResponse {
        status: status.to_string(),
        active_scans: active_metrics.active_scans,
        files_processed: active_metrics.files_processed,
        events_processed: active_metrics.events_processed,
        processing_rate_per_minute: active_metrics.processing_rate,
        secrets_found: active_metrics.findings_found,
        queue_pending: queue_stats.pending_events,
        queue_processing: queue_stats.processing_events,
        queue_failed: queue_stats.failed_events,
        queue_failed_forbidden: queue_stats.failed_forbidden,
        queue_failed_not_found: queue_stats.failed_not_found,
        queue_completed_last_hour: queue_stats.completed_last_hour,
        oldest_pending_age_seconds: queue_stats.oldest_pending_age_seconds,
        timestamp: Utc::now(),
        trufflehog_available,
        scraper_running,
        ready,
        issues,
    })
}

/// Start a repository scan
pub async fn scan_repository(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Extension(user): Extension<User>,
    Json(request): Json<ScanRepositoryRequest>,
) -> Result<Json<ScanResponse>, (StatusCode, Json<Value>)> {
    start_repository_scan(&app_state, request, &user.username)
        .await
        .map(|response| {
            let mut details = HashMap::new();
            details.insert("repository".to_string(), json!(response.repository));
            details.insert("status".to_string(), json!(response.status));
            details.insert(
                "scan_duration_ms".to_string(),
                json!(response.scan_duration_ms),
            );
            let resource_id = Some(response.scan_id.clone());
            let audit_logger = app_state.audit_logger.clone();
            let username = user.username.clone();
            let headers = headers.clone();
            tokio::spawn(async move {
                let _ = crate::audit_helpers::log_success(
                    &audit_logger,
                    None,
                    &username,
                    crate::audit::AuditAction::ScanLaunched,
                    crate::audit::ResourceType::Scan,
                    resource_id,
                    &headers,
                    details,
                )
                .await;
            });
            Json(response)
        })
        .map_err(|error: ScannerServiceError| {
            error!("Repository scan request failed: {}", error);
            (
                error.status_code(),
                Json(json!({
                    "error": error.to_string()
                })),
            )
        })
}

/// Start a batch scan of multiple repositories
pub async fn batch_scan_repositories(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Extension(user): Extension<User>,
    Json(request): Json<BatchScanRequest>,
) -> Result<Json<BatchScanResponse>, (StatusCode, Json<Value>)> {
    start_batch_scan(&app_state, request, &user.username)
        .await
        .map(|response| {
            let mut details = HashMap::new();
            details.insert(
                "total_repositories".to_string(),
                json!(response.total_repositories),
            );
            details.insert(
                "completed_repositories".to_string(),
                json!(response.completed_repositories),
            );
            details.insert("status".to_string(), json!(response.status));
            let resource_id = Some(response.batch_id.clone());
            let audit_logger = app_state.audit_logger.clone();
            let username = user.username.clone();
            let headers = headers.clone();
            tokio::spawn(async move {
                let _ = crate::audit_helpers::log_success(
                    &audit_logger,
                    None,
                    &username,
                    crate::audit::AuditAction::ScanLaunched,
                    crate::audit::ResourceType::Scan,
                    resource_id,
                    &headers,
                    details,
                )
                .await;
            });
            Json(response)
        })
        .map_err(|error: ScannerServiceError| {
            error!("Batch scan request failed: {}", error);
            (
                error.status_code(),
                Json(json!({
                    "error": error.to_string()
                })),
            )
        })
}

/// Get scan results with filtering
pub async fn get_scan_results(
    State(app_state): State<AppState>,
    Query(filters): Query<ScanFiltersQuery>,
) -> Json<Value> {
    info!("Fetching scan results with filters: {:?}", filters);

    let limit = filters.limit.unwrap_or(50).min(500) as i64;
    let offset = filters.offset.unwrap_or(0) as i64;

    let filter = SecretDetectionFilter {
        repository: filters.repository.clone(),
        severity: filters.severity.clone(),
        category: filters.category.clone(),
        source: None,
        verified: None,
        date_from: parse_datetime_filter(filters.date_from.clone()),
        date_to: parse_datetime_filter(filters.date_to.clone()),
        limit: Some(limit),
        offset: Some(offset),
    };

    match app_state.persistence.secret_detections(filter).await {
        Ok(detections) => {
            let total_loaded = detections.len() as u32;
            let has_more = total_loaded as i64 == limit;

            Json(json!({
                "results": detections,
                "total_count": total_loaded,
                "page_size": limit,
                "has_more": has_more,
                "filters_applied": filters
            }))
        }
        Err(e) => {
            error!("Failed to load scan results: {}", e);
            Json(json!({
                "error": "Failed to load scan results",
                "details": e.to_string()
            }))
        }
    }
}

/// Get scanning statistics and metrics
pub async fn get_scan_statistics(State(app_state): State<AppState>) -> Json<Value> {
    info!("Fetching scan statistics");

    let stats = app_state.scanning_service.get_statistics().await;

    Json(json!({
        "total_scans": stats.total_scans,
        "repositories_scanned": stats.repositories_scanned,
        "total_secrets_found": stats.total_findings,
        "verified_secrets": stats.verified_findings,
        "false_positives": stats.false_positives,
        "scan_stats": {
            "avg_scan_time_ms": stats.avg_scan_time_ms,
            "avg_secrets_per_repo": stats.avg_secrets_per_repo,
            "success_rate": stats.success_rate,
        },
        "severity_distribution": stats.severity_distribution,
        "category_distribution": stats.category_distribution,
        "detector_performance": stats.detector_performance,
        "recent_activity": stats.recent_activity,
    }))
}

/// Get available secret detectors
pub async fn get_detectors() -> Json<Value> {
    info!("Fetching available secret detectors");

    let scanner = SecretScanner::new();
    let detectors = scanner.detectors();
    let mut categories: HashSet<String> = HashSet::new();
    for detector in &detectors {
        categories.insert(detector.category.to_string());
    }

    Json(json!({
        "detectors": detectors,
        "total_count": detectors.len(),
        "enabled_count": detectors.len(),
        "categories": categories.into_iter().collect::<Vec<_>>()
    }))
}

/// Export scan results in various formats
pub async fn export_scan_results(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Extension(user): Extension<User>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let format = params
        .get("format")
        .cloned()
        .unwrap_or_else(|| "json".to_string())
        .to_ascii_lowercase();
    let repository_filter = params.get("repository").cloned();

    info!(
        "Exporting scan results in {} format for user: {} (repo filter: {:?})",
        format, user.username, repository_filter
    );

    // Build filter for detection query
    let filter = crate::core::database::SecretDetectionFilter {
        repository: repository_filter,
        severity: params.get("severity").cloned(),
        category: params.get("category").cloned(),
        source: params.get("source").cloned(),
        verified: params.get("verified").and_then(|v| v.parse().ok()),
        date_from: None,
        date_to: None,
        limit: Some(10000), // Export up to 10k detections
        offset: None,
    };

    // Fetch secret detections from database
    let detections = match app_state.persistence.secret_detections(filter).await {
        Ok(results) => results,
        Err(e) => {
            error!("Failed to fetch secret detections for export: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match format.as_str() {
        "json" | "csv" => {
            let export_data = json!({
                "export_id": Uuid::new_v4().to_string(),
                "exported_at": Utc::now(),
                "exported_by": user.username,
                "requested_format": format,
                "content_type": "application/json",
                "total_secrets": detections.len(),
                "filters": params,
                "detections": detections
            });
            let mut details = HashMap::new();
            details.insert("format".to_string(), json!(format));
            details.insert("total_secrets".to_string(), json!(detections.len()));
            details.insert("filters".to_string(), json!(params));
            let _ = crate::audit_helpers::log_success(
                &app_state.audit_logger,
                None,
                &user.username,
                crate::audit::AuditAction::ScanExported,
                crate::audit::ResourceType::Scan,
                export_data
                    .get("export_id")
                    .and_then(|value| value.as_str())
                    .map(std::string::ToString::to_string),
                &headers,
                details,
            )
            .await;
            Ok(Json(export_data))
        }
        "pdf" => Err(StatusCode::NOT_IMPLEMENTED),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

/// Schedule a recurring scan
pub async fn schedule_scan(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Extension(user): Extension<User>,
    Json(request): Json<ScheduleScanRequest>,
) -> Result<Json<ScheduledScanResponse>, (StatusCode, Json<Value>)> {
    create_scan_schedule(&app_state, request, &user.username)
        .await
        .map(|response| {
            let mut details = HashMap::new();
            details.insert("name".to_string(), json!(response.name));
            details.insert("schedule".to_string(), json!(response.schedule));
            details.insert("repositories".to_string(), json!(response.repositories));
            let resource_id = Some(response.schedule_id.clone());
            let audit_logger = app_state.audit_logger.clone();
            let username = user.username.clone();
            let headers = headers.clone();
            tokio::spawn(async move {
                let _ = crate::audit_helpers::log_success(
                    &audit_logger,
                    None,
                    &username,
                    crate::audit::AuditAction::ScanScheduled,
                    crate::audit::ResourceType::Scan,
                    resource_id,
                    &headers,
                    details,
                )
                .await;
            });
            Json(response)
        })
        .map_err(|error: ScannerServiceError| {
            error!("Schedule creation failed: {}", error);
            (
                error.status_code(),
                Json(json!({
                    "error": error.to_string()
                })),
            )
        })
}

/// Get scheduled scans
pub async fn get_scheduled_scans(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>,
) -> Json<Value> {
    info!("Fetching scheduled scans for user: {}", user.username);

    let schedules = app_state.scanning_service.get_schedules().await;

    Json(json!({
        "schedules": schedules,
        "total_count": schedules.len(),
        "requested_by": user.username
    }))
}

fn parse_datetime_filter(value: Option<String>) -> Option<DateTime<Utc>> {
    value
        .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
        .map(|dt| dt.with_timezone(&Utc))
}
