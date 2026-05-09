// Audit log API handlers
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::state::AppState;
use crate::audit::{
    AuditAction, AuditLog, AuditLogFilters, AuditStatistics, AuditStatus, ResourceType,
};

/// Query parameters for listing audit logs
#[derive(Debug, Deserialize)]
pub struct ListAuditLogsQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    50
}

/// Response for listing audit logs
#[derive(Debug, Serialize)]
pub struct ListAuditLogsResponse {
    pub logs: Vec<AuditLog>,
    pub page: i64,
    pub page_size: i64,
    pub total_count: Option<i64>,
}

/// GET /api/audit/logs - List audit logs with filters
pub async fn list_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<ListAuditLogsQuery>,
) -> Result<Json<ListAuditLogsResponse>, StatusCode> {
    // Parse filters
    let action = query
        .action
        .as_ref()
        .and_then(|a| serde_json::from_str::<AuditAction>(a).ok());

    let resource_type = query
        .resource_type
        .as_ref()
        .and_then(|r| serde_json::from_str::<ResourceType>(r).ok());

    let status = query
        .status
        .as_ref()
        .and_then(|s| serde_json::from_str::<AuditStatus>(s).ok());

    let filters = AuditLogFilters {
        user_id: query.user_id,
        username: query.username,
        action,
        resource_type,
        status,
        start_date: query.start_date,
        end_date: query.end_date,
    };

    let offset = (query.page - 1) * query.page_size;

    let logs = state
        .audit_logger
        .query(filters, query.page_size, offset)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query audit logs");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(ListAuditLogsResponse {
        logs,
        page: query.page,
        page_size: query.page_size,
        total_count: None, // Could add a count query if needed
    }))
}

/// GET /api/audit/logs/:id - Get specific audit log
pub async fn get_audit_log(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<AuditLog>, StatusCode> {
    let log = state
        .audit_logger
        .get_by_id(id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, log_id = id, "Failed to get audit log");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(log))
}

/// Query parameters for audit statistics
#[derive(Debug, Deserialize)]
pub struct AuditStatsQuery {
    #[serde(default = "default_stats_days")]
    pub days: i32,
}

fn default_stats_days() -> i32 {
    30
}

/// GET /api/audit/stats - Get audit statistics
pub async fn get_audit_statistics(
    State(state): State<AppState>,
    Query(query): Query<AuditStatsQuery>,
) -> Result<Json<AuditStatistics>, StatusCode> {
    let stats = state
        .audit_logger
        .get_statistics(query.days)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get audit statistics");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(stats))
}

/// Query parameters for audit log export
#[derive(Debug, Deserialize)]
pub struct ExportAuditLogsQuery {
    pub format: String, // "json" or "csv"
    #[serde(default = "default_export_limit")]
    pub limit: i64,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

fn default_export_limit() -> i64 {
    10000
}

/// GET /api/audit/export - Export audit logs
pub async fn export_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<ExportAuditLogsQuery>,
) -> Result<String, StatusCode> {
    // Parse filters
    let action = query
        .action
        .as_ref()
        .and_then(|a| serde_json::from_str::<AuditAction>(a).ok());

    let resource_type = query
        .resource_type
        .as_ref()
        .and_then(|r| serde_json::from_str::<ResourceType>(r).ok());

    let status = query
        .status
        .as_ref()
        .and_then(|s| serde_json::from_str::<AuditStatus>(s).ok());

    let filters = AuditLogFilters {
        user_id: query.user_id,
        username: query.username,
        action,
        resource_type,
        status,
        start_date: query.start_date,
        end_date: query.end_date,
    };

    match query.format.as_str() {
        "json" => state
            .audit_logger
            .export_json(filters, Some(query.limit))
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to export audit logs as JSON");
                StatusCode::INTERNAL_SERVER_ERROR
            }),
        "csv" => state
            .audit_logger
            .export_csv(filters, Some(query.limit))
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to export audit logs as CSV");
                StatusCode::INTERNAL_SERVER_ERROR
            }),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

/// POST /api/audit/cleanup - Clean up old audit logs (admin only)
#[derive(Debug, Deserialize)]
pub struct CleanupAuditLogsRequest {
    pub retention_days: i32,
}

#[derive(Debug, Serialize)]
pub struct CleanupAuditLogsResponse {
    pub deleted_count: i64,
}

pub async fn cleanup_audit_logs(
    State(state): State<AppState>,
    Json(request): Json<CleanupAuditLogsRequest>,
) -> Result<Json<CleanupAuditLogsResponse>, StatusCode> {
    if request.retention_days < 7 {
        return Err(StatusCode::BAD_REQUEST); // Minimum 7 days retention
    }

    let deleted_count = state
        .audit_logger
        .cleanup(request.retention_days)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to cleanup audit logs");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(CleanupAuditLogsResponse { deleted_count }))
}
