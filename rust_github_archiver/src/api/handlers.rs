// API handlers implementation
use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::scraper_control::{
    execute_scraper_action, ScraperControlAction, ScraperControlResult,
};
use crate::api::state::AppState;
use crate::api::status_service::{build_scraper_status, build_system_status};
use crate::auth::jwt::verify_token;
use crate::auth::{create_token, User, UserRole};

// Re-export scanner handlers
pub use crate::api::scanner_handlers::*;

// Simple test endpoint
pub async fn ping() -> &'static str {
    "pong"
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
    user: UserInfo,
    expires_at: String,
}

#[derive(Serialize)]
pub struct UserInfo {
    id: String,
    username: String,
    role: String,
}

impl From<User> for UserInfo {
    fn from(user: User) -> Self {
        let role = user.canonical_role();
        Self {
            id: user.id,
            username: user.username,
            role,
        }
    }
}

/// Health check with database, memory, disk, and service readiness details.
pub async fn health_check(State(app_state): State<AppState>) -> Json<Value> {
    use sysinfo::{Disks, System};

    let mut checks = serde_json::Map::new();
    let mut has_warnings = false;
    let mut has_errors = false;

    // Database health check
    let db_health = app_state.persistence.health_status().await;
    if !db_health.is_connected {
        has_errors = true;
    }
    checks.insert(
        "database".to_string(),
        json!({
            "status": if db_health.is_connected { "ok" } else { "error" },
            "connected": db_health.is_connected,
            "connection_count": db_health.connection_count,
            "active_queries": db_health.active_queries,
            "cache_hit_ratio": format!("{:.2}%", db_health.cache_hit_ratio),
            "error": db_health.error_message
        }),
    );

    // Check system memory
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let memory_percent = if total_memory > 0 {
        (used_memory as f64 / total_memory as f64) * 100.0
    } else {
        0.0
    };

    let memory_warning = memory_percent > 80.0;
    let memory_critical = memory_percent > 90.0;
    if memory_critical {
        has_errors = true;
    } else if memory_warning {
        has_warnings = true;
    }

    checks.insert("memory".to_string(), json!({
        "status": if memory_critical { "critical" } else if memory_warning { "warning" } else { "ok" },
        "used_mb": used_memory / 1024 / 1024,
        "total_mb": total_memory / 1024 / 1024,
        "used_percent": format!("{:.1}%", memory_percent)
    }));

    // Check disk space
    let disks = Disks::new_with_refreshed_list();
    let mut disk_status = "ok";
    let mut disk_checks = Vec::new();

    for disk in disks.list() {
        let total_space = disk.total_space();
        let available_space = disk.available_space();
        let used_percent = if total_space > 0 {
            ((total_space - available_space) as f64 / total_space as f64) * 100.0
        } else {
            0.0
        };

        let mount_point = disk.mount_point().to_string_lossy().to_string();
        let this_disk_status = if used_percent > 90.0 {
            has_errors = true;
            "critical"
        } else if used_percent > 80.0 {
            has_warnings = true;
            "warning"
        } else {
            "ok"
        };

        if disk_status == "ok" && this_disk_status != "ok" {
            disk_status = this_disk_status;
        }

        disk_checks.push(json!({
            "mount_point": mount_point,
            "status": this_disk_status,
            "total_gb": total_space / 1024 / 1024 / 1024,
            "available_gb": available_space / 1024 / 1024 / 1024,
            "used_percent": format!("{:.1}%", used_percent)
        }));
    }

    checks.insert(
        "disk".to_string(),
        json!({
            "status": disk_status,
            "disks": disk_checks
        }),
    );

    // Overall status
    let overall_status = if has_errors {
        "unhealthy"
    } else if has_warnings {
        "degraded"
    } else {
        "healthy"
    };

    Json(json!({
        "status": overall_status,
        "timestamp": Utc::now().to_rfc3339(),
        "service": "github-archiver-rust",
        "version": "1.0.0-beta",
        "checks": checks
    }))
}

pub async fn login(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<Value>)> {
    // Authenticate user
    let user = app_state
        .user_manager
        .authenticate(&payload.username, &payload.password)
        .await;

    match user {
        Some(user) => {
            // Update last login time
            if let Err(e) = app_state
                .user_manager
                .update_last_login(&user.username)
                .await
            {
                tracing::warn!("Failed to update last login for {}: {}", user.username, e);
            }

            // Create JWT token
            let token = create_token(&user.username).map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Token creation failed",
                        "message": "Failed to create authentication token"
                    })),
                )
            })?;

            // Calculate expiration time (24 hours from now)
            let expires_at = (Utc::now() + chrono::Duration::hours(24)).to_rfc3339();

            // Audit log: successful login
            let mut details = std::collections::HashMap::new();
            details.insert(
                "ip_address".to_string(),
                serde_json::json!(crate::audit::extract_ip_from_headers(&headers)
                    .unwrap_or_else(|| "unknown".to_string())),
            );
            let _ = crate::audit_helpers::log_success(
                &app_state.audit_logger,
                None, // user_id is String in this codebase, not i64
                &user.username,
                crate::audit::AuditAction::LoginSuccess,
                crate::audit::ResourceType::User,
                Some(user.id.clone()),
                &headers,
                details,
            )
            .await;

            Ok(Json(LoginResponse {
                token,
                user: user.into(),
                expires_at,
            }))
        }
        None => {
            // Audit log: failed login
            let mut details = std::collections::HashMap::new();
            details.insert("username".to_string(), serde_json::json!(payload.username));
            details.insert(
                "ip_address".to_string(),
                serde_json::json!(crate::audit::extract_ip_from_headers(&headers)
                    .unwrap_or_else(|| "unknown".to_string())),
            );
            let _ = crate::audit_helpers::log_failure(
                &app_state.audit_logger,
                None,
                &payload.username,
                crate::audit::AuditAction::LoginFailure,
                crate::audit::ResourceType::User,
                None,
                &headers,
                "Invalid username or password",
                details,
            )
            .await;

            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Authentication failed",
                    "message": "Invalid username or password"
                })),
            ))
        }
    }
}

#[derive(Serialize)]
pub struct AuthStatusResponse {
    authenticated: bool,
    user: Option<String>,
}

pub async fn auth_status(user: Option<Extension<User>>) -> Json<AuthStatusResponse> {
    if let Some(Extension(user)) = user {
        Json(AuthStatusResponse {
            authenticated: true,
            user: Some(user.username.clone()),
        })
    } else {
        Json(AuthStatusResponse {
            authenticated: false,
            user: None,
        })
    }
}

pub async fn logout(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> Json<Value> {
    // In a stateless JWT system, logout is handled client-side by discarding the token
    // Server-side logout would require token blacklisting, which we're not implementing here

    // Log logout for audit trail
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let _ = crate::audit_helpers::log_success(
        &app_state.audit_logger,
        None,
        &username,
        crate::audit::AuditAction::LogoutSuccess,
        crate::audit::ResourceType::User,
        None,
        &headers,
        std::collections::HashMap::new(),
    )
    .await;

    Json(json!({
        "message": "Logged out successfully",
        "timestamp": Utc::now().to_rfc3339()
    }))
}

pub async fn user_info(Extension(user): Extension<User>) -> Json<UserInfo> {
    Json(user.into())
}

fn scraper_action_success_response(result: &ScraperControlResult) -> Json<Value> {
    Json(json!({
        "status": result.status,
        "message": result.message,
        "scraper_running": result.scraper_running,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

fn scraper_action_error_message(action: ScraperControlAction, error: &str) -> String {
    if error.starts_with("Failed to initialize scraper runtime:") {
        error.to_string()
    } else {
        format!("Failed to {} scraper: {}", action.failure_prefix(), error)
    }
}

fn github_token_preview(token: &str) -> String {
    if token.is_empty() {
        String::new()
    } else if token.len() >= 8 {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    } else {
        "****".to_string()
    }
}

fn scraper_action_error_response(
    action: ScraperControlAction,
    error: &str,
    scraper_running: bool,
) -> Json<Value> {
    Json(json!({
        "status": "error",
        "message": scraper_action_error_message(action, error),
        "scraper_running": scraper_running,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

async fn audited_scraper_action(
    app_state: &AppState,
    headers: &HeaderMap,
    user: Option<&Extension<User>>,
    action: ScraperControlAction,
) -> Json<Value> {
    let username = user
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "system".to_string());

    match execute_scraper_action(app_state, action).await {
        Ok(result) => {
            let mut details = std::collections::HashMap::new();
            details.insert("scraper_running".to_string(), json!(result.scraper_running));
            let _ = crate::audit_helpers::log_success(
                &app_state.audit_logger,
                None,
                &username,
                action.audit_action(),
                crate::audit::ResourceType::Scraper,
                None,
                headers,
                details,
            )
            .await;

            scraper_action_success_response(&result)
        }
        Err(error) => {
            let _ = crate::audit_helpers::log_failure(
                &app_state.audit_logger,
                None,
                &username,
                action.audit_action(),
                crate::audit::ResourceType::Scraper,
                None,
                headers,
                &error,
                std::collections::HashMap::new(),
            )
            .await;

            scraper_action_error_response(action, &error, app_state.scraper_manager.is_running())
        }
    }
}

async fn execute_scraper_action_response(
    app_state: &AppState,
    action: ScraperControlAction,
) -> Json<Value> {
    match execute_scraper_action(app_state, action).await {
        Ok(result) => scraper_action_success_response(&result),
        Err(error) => {
            scraper_action_error_response(action, &error, app_state.scraper_manager.is_running())
        }
    }
}

// Scraper control handlers
pub async fn start_scraper(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> Json<Value> {
    audited_scraper_action(
        &app_state,
        &headers,
        user.as_ref(),
        ScraperControlAction::Start,
    )
    .await
}

pub async fn stop_scraper(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> Json<Value> {
    audited_scraper_action(
        &app_state,
        &headers,
        user.as_ref(),
        ScraperControlAction::Stop,
    )
    .await
}

pub async fn pause_scraper(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> Json<Value> {
    audited_scraper_action(
        &app_state,
        &headers,
        user.as_ref(),
        ScraperControlAction::Pause,
    )
    .await
}

pub async fn resume_scraper(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> Json<Value> {
    audited_scraper_action(
        &app_state,
        &headers,
        user.as_ref(),
        ScraperControlAction::Resume,
    )
    .await
}

pub async fn restart_scraper(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> Json<Value> {
    audited_scraper_action(
        &app_state,
        &headers,
        user.as_ref(),
        ScraperControlAction::Restart,
    )
    .await
}

pub async fn system_status_simple(State(app_state): State<AppState>) -> Json<Value> {
    match build_system_status(&app_state).await {
        Ok(status) => Json(json!(status)),
        Err(error) => Json(json!({
            "status": "error",
            "timestamp": Utc::now().to_rfc3339(),
            "error": error.to_string(),
            "ready": false
        })),
    }
}

// Scraper status with real data from AppState
pub async fn scraper_status_simple(State(app_state): State<AppState>) -> Json<Value> {
    match app_state.get_comprehensive_status().await {
        Ok(comprehensive_status) => match build_scraper_status(&app_state, &comprehensive_status) {
            Ok(status) => Json(json!(status)),
            Err(error) => Json(json!({
                "status": "error",
                "message": error.to_string(),
                "is_running": false,
                "ready": false,
                "last_updated": Utc::now().to_rfc3339()
            })),
        },
        Err(error) => Json(json!({
            "status": "error",
            "message": error.to_string(),
            "is_running": false,
            "ready": false,
            "last_updated": Utc::now().to_rfc3339()
        })),
    }
}

pub async fn system_metrics() -> Json<Value> {
    // Get actual system metrics using sys_info
    let mem_info = sys_info::mem_info().unwrap_or(sys_info::MemInfo {
        total: 0,
        free: 0,
        avail: 0,
        buffers: 0,
        cached: 0,
        swap_total: 0,
        swap_free: 0,
    });

    let disk_info = sys_info::disk_info().unwrap_or(sys_info::DiskInfo { total: 0, free: 0 });

    let load = sys_info::loadavg().unwrap_or(sys_info::LoadAvg {
        one: 0.0,
        five: 0.0,
        fifteen: 0.0,
    });

    let memory_usage = if mem_info.total > 0 {
        ((mem_info.total - mem_info.avail) as f64 / mem_info.total as f64) * 100.0
    } else {
        0.0
    };

    let disk_usage = if disk_info.total > 0 {
        ((disk_info.total - disk_info.free) as f64 / disk_info.total as f64) * 100.0
    } else {
        0.0
    };

    Json(json!({
        "cpu_usage": (load.one * 100.0 / num_cpus::get() as f64).min(100.0),
        "memory_usage": memory_usage,
        "disk_usage": disk_usage,
        "load_average": {
            "one": load.one,
            "five": load.five,
            "fifteen": load.fifteen
        },
        "memory_info": {
            "total_mb": mem_info.total / 1024,
            "available_mb": mem_info.avail / 1024,
            "used_mb": (mem_info.total - mem_info.avail) / 1024
        },
        "disk_info": {
            "total_gb": disk_info.total / (1024 * 1024),
            "free_gb": disk_info.free / (1024 * 1024),
            "used_gb": (disk_info.total - disk_info.free) / (1024 * 1024)
        }
    }))
}

pub async fn database_status(State(app_state): State<AppState>) -> Json<Value> {
    // Use the existing database connection from app_state
    let health = app_state.persistence.health_status().await;

    Json(json!({
        "status": if health.is_connected { "connected" } else { "disconnected" },
        "is_connected": health.is_connected,
        "connection_count": health.connection_count,
        "active_queries": health.active_queries,
        "cache_hit_ratio": health.cache_hit_ratio,
        "error_message": health.error_message,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

pub async fn scraper_control(
    State(app_state): State<AppState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("");

    match ScraperControlAction::from_api_action(action) {
        Some(action) => execute_scraper_action_response(&app_state, action).await,
        None => Json(json!({
            "error": "Invalid action. Use start, stop, restart, pause, or resume",
            "timestamp": Utc::now().to_rfc3339()
        })),
    }
}

pub async fn auth_verify(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if let Ok(claims) = verify_token(token) {
                    if let Some(user) = app_state.user_manager.get_user(&claims.sub).await {
                        return Ok(Json(json!({
                            "valid": true,
                            "user": user.username,
                            "role": user.canonical_role()
                        })));
                    }
                }
            }
        }
    }

    // Log unauthorized access attempt
    let _ = crate::audit_helpers::log_security_event(
        &app_state.audit_logger,
        "unknown",
        crate::audit::AuditAction::InvalidToken,
        &headers,
        "Unauthorized access attempt - missing or invalid token",
    )
    .await;

    Err(StatusCode::UNAUTHORIZED)
}

// New endpoint for secrets statistics
pub async fn secrets_stats(State(app_state): State<AppState>) -> Json<Value> {
    match app_state.persistence.secret_overview_metrics().await {
        Ok(metrics) => Json(json!({
            "success": true,
            "total_secrets": metrics.total_secrets,
            "severity_counts": metrics.severity_counts,
            "category_counts": metrics.category_counts,
            "verified_secrets": metrics.verified_secrets,
            "false_positives": metrics.false_positives,
            "repositories_scanned": metrics.repositories_scanned,
            "files_scanned": metrics.files_scanned,
            "total_scans": metrics.total_scans,
            "active_scans": metrics.active_scans,
            "failed_scans": metrics.failed_scans,
            "avg_scan_duration_ms": metrics.avg_scan_duration_ms,
            "last_scan_time": metrics.last_scan_time,
            "scan_success_rate": metrics.scan_success_rate,
            "scan_rate_per_minute": metrics.scan_rate_per_minute,
            "repos_per_minute": metrics.repos_per_minute,
            "timestamp": Utc::now().to_rfc3339()
        })),
        Err(e) => {
            tracing::error!("Failed to load secret overview metrics: {}", e);
            Json(json!({
                "success": false,
                "error": "Failed to load secrets stats",
                "timestamp": Utc::now().to_rfc3339()
            }))
        }
    }
}

// New endpoint for auth verification
pub async fn verify_auth(headers: axum::http::HeaderMap) -> Json<Value> {
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                // Verify JWT token
                match verify_token(token) {
                    Ok(claims) => {
                        return Json(json!({
                            "valid": true,
                            "username": claims.sub,
                            "timestamp": Utc::now().to_rfc3339()
                        }));
                    }
                    Err(e) => {
                        tracing::warn!("Token verification failed: {}", e);
                    }
                }
            }
        }
    }

    Json(json!({
        "valid": false,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

// Emergency cleanup endpoint
pub async fn emergency_cleanup(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
) -> Json<Value> {
    let comprehensive_status = match app_state.get_comprehensive_status().await {
        Ok(status) => status,
        Err(e) => {
            tracing::error!("Failed to get comprehensive status: {}", e);
            return Json(json!({
                "success": false,
                "error": "Failed to access system status",
                "timestamp": Utc::now().to_rfc3339()
            }));
        }
    };

    if let Some(resource_status) = comprehensive_status.resource_status {
        if resource_status.emergency_mode {
            // Perform cleanup through app state
            match app_state.perform_emergency_cleanup().await {
                Ok(()) => {
                    // Log successful emergency cleanup
                    let username = user
                        .as_ref()
                        .map(|u| u.username.clone())
                        .unwrap_or_else(|| "system".to_string());
                    let mut details = std::collections::HashMap::new();
                    details.insert(
                        "emergency_conditions".to_string(),
                        json!(resource_status.emergency_conditions),
                    );
                    let _ = crate::audit_helpers::log_security_event(
                        &app_state.audit_logger,
                        &username,
                        crate::audit::AuditAction::SystemCleanup,
                        &headers,
                        "Emergency cleanup performed due to system resource constraints",
                    )
                    .await;

                    Json(json!({
                        "success": true,
                        "message": "Emergency cleanup performed",
                        "emergency_conditions": resource_status.emergency_conditions,
                        "timestamp": Utc::now().to_rfc3339()
                    }))
                }
                Err(e) => {
                    // Log failure
                    let username = user
                        .as_ref()
                        .map(|u| u.username.clone())
                        .unwrap_or_else(|| "system".to_string());
                    let _ = crate::audit_helpers::log_failure(
                        &app_state.audit_logger,
                        None,
                        &username,
                        crate::audit::AuditAction::SystemCleanup,
                        crate::audit::ResourceType::System,
                        None,
                        &headers,
                        &e.to_string(),
                        std::collections::HashMap::new(),
                    )
                    .await;

                    Json(json!({
                        "success": false,
                        "error": format!("Cleanup failed: {}", e),
                        "timestamp": Utc::now().to_rfc3339()
                    }))
                }
            }
        } else {
            Json(json!({
                "success": true,
                "message": "System is not in emergency mode",
                "timestamp": Utc::now().to_rfc3339()
            }))
        }
    } else {
        Json(json!({
            "success": false,
            "error": "Resource monitoring not available",
            "timestamp": Utc::now().to_rfc3339()
        }))
    }
}

// Database Management Handlers

#[derive(Deserialize)]
pub struct DatabaseControlRequest {
    pub force: Option<bool>,
}

#[derive(Serialize)]
pub struct DatabaseStatsResponse {
    pub total_events: u64,
    pub database_size: String,
    pub table_count: u32,
    pub tables: Vec<TableInfo>,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct TableInfo {
    pub name: String,
    pub row_count: u64,
    pub size: String,
}

pub async fn database_start(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
    Json(request): Json<DatabaseControlRequest>,
) -> impl IntoResponse {
    // Check if database is already running by attempting a connection
    let connection_result = crate::core::database::Database::new(&app_state.config).await;

    match connection_result {
        Ok(_) => {
            // Log database already running
            let username = user
                .as_ref()
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "system".to_string());
            let mut details = std::collections::HashMap::new();
            details.insert("status".to_string(), json!("already_running"));
            details.insert("force".to_string(), json!(request.force.unwrap_or(false)));
            let _ = crate::audit_helpers::log_success(
                &app_state.audit_logger,
                None,
                &username,
                crate::audit::AuditAction::DatabaseStarted,
                crate::audit::ResourceType::Database,
                None,
                &headers,
                details,
            )
            .await;

            Json(json!({
                "success": true,
                "message": "Database is already running and accessible",
                "status": "running",
                "timestamp": Utc::now().to_rfc3339(),
                "force": request.force.unwrap_or(false)
            }))
        }
        Err(e) => {
            // Log database connection failure
            let username = user
                .as_ref()
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "system".to_string());
            let _ = crate::audit_helpers::log_failure(
                &app_state.audit_logger,
                None,
                &username,
                crate::audit::AuditAction::DatabaseStarted,
                crate::audit::ResourceType::Database,
                None,
                &headers,
                &format!("Connection failed: {}", e),
                std::collections::HashMap::new(),
            )
            .await;

            tracing::warn!("Database connection failed: {}", e);
            Json(json!({
                "success": false,
                "message": "Database appears to be down - cannot start database through API (requires system-level access)",
                "error": format!("Connection failed: {}", e),
                "status": "down",
                "timestamp": Utc::now().to_rfc3339(),
                "force": request.force.unwrap_or(false)
            }))
        }
    }
}

pub async fn database_stop(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    user: Option<Extension<User>>,
    Json(request): Json<DatabaseControlRequest>,
) -> impl IntoResponse {
    // Log database stop attempt (not allowed via API)
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "system".to_string());
    let mut details = std::collections::HashMap::new();
    details.insert("operation".to_string(), json!("stop"));
    details.insert("result".to_string(), json!("not_supported"));
    let _ = crate::audit_helpers::log_security_event(
        &app_state.audit_logger,
        &username,
        crate::audit::AuditAction::SuspiciousActivity,
        &headers,
        "Attempted database stop via API (operation not supported for safety)",
    )
    .await;

    // Database stop requires system-level privileges and is not recommended via API
    Json(json!({
        "success": false,
        "message": "Database stop operation not supported via API for safety reasons",
        "status": "operation_not_supported",
        "recommendation": "Use system-level tools (systemctl, pg_ctl) to manage database service",
        "timestamp": Utc::now().to_rfc3339(),
        "force": request.force.unwrap_or(false)
    }))
}

pub async fn database_restart(
    State(_app_state): State<AppState>,
    Json(request): Json<DatabaseControlRequest>,
) -> impl IntoResponse {
    // Database restart requires system-level privileges and is not recommended via API
    Json(json!({
        "success": false,
        "message": "Database restart operation not supported via API for safety reasons",
        "status": "operation_not_supported",
        "recommendation": "Use system-level tools (systemctl, pg_ctl) to manage database service",
        "timestamp": Utc::now().to_rfc3339(),
        "force": request.force.unwrap_or(false)
    }))
}

pub async fn database_stats(State(app_state): State<AppState>) -> impl IntoResponse {
    // Use the existing database connection from app_state instead of creating a new one
    let stats_result = async {
        let db_stats = app_state.persistence.database_statistics().await?;

        // Convert to our response format
        let tables: Vec<TableInfo> = db_stats
            .tables
            .iter()
            .map(|(name, row_count, size)| TableInfo {
                name: name.clone(),
                row_count: (*row_count).max(0) as u64,
                size: size.clone(),
            })
            .collect();

        let stats = DatabaseStatsResponse {
            total_events: db_stats.total_events.max(0) as u64,
            database_size: db_stats.database_size,
            table_count: db_stats.table_count.max(0) as u32,
            tables,
            last_updated: Utc::now().to_rfc3339(),
        };

        Ok::<DatabaseStatsResponse, anyhow::Error>(stats)
    }
    .await;

    match stats_result {
        Ok(stats) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "status": if stats.total_events == 0 { "empty" } else { "ready" },
                "data": stats,
                "timestamp": Utc::now().to_rfc3339()
            })),
        ),
        Err(e) => {
            tracing::error!("Failed to get database stats: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "success": false,
                    "status": "unavailable",
                    "error": "Failed to load database statistics",
                    "details": e.to_string(),
                    "timestamp": Utc::now().to_rfc3339()
                })),
            )
        }
    }
}

// User Management Handlers

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub username: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct UserListResponse {
    pub id: String,
    pub username: String,
    pub role: String,
    pub created_at: String,
    pub last_login: Option<String>,
}

pub async fn change_password(
    State(app_state): State<AppState>,
    Extension(current_user): Extension<User>,
    Json(request): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let user_manager = &app_state.user_manager;

    let current_role = match current_user.parsed_role() {
        Ok(role) => role,
        Err(error) => {
            tracing::warn!(
                username = %current_user.username,
                error = %error,
                "User with invalid role attempted password change"
            );
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "error": "User account has an unsupported role assignment",
                    "timestamp": Utc::now().to_rfc3339()
                })),
            );
        }
    };

    if current_role != UserRole::Admin && request.username != current_user.username {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "Only administrators can change another user's password",
                "timestamp": Utc::now().to_rfc3339()
            })),
        );
    }

    // Verify user exists
    if user_manager.get_user(&request.username).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "User not found",
                "timestamp": Utc::now().to_rfc3339()
            })),
        );
    }

    // Validate password strength
    if request.new_password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Password must be at least 8 characters long",
                "timestamp": Utc::now().to_rfc3339()
            })),
        );
    }

    // Update user's password in the UserManager
    match user_manager
        .update_password(&request.username, &request.new_password)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": format!("Password updated for user: {}", request.username),
                "timestamp": Utc::now().to_rfc3339()
            })),
        ),
        Err(e) => {
            tracing::error!("Failed to update password for {}: {}", request.username, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "Failed to update password",
                    "timestamp": Utc::now().to_rfc3339()
                })),
            )
        }
    }
}

pub async fn list_users(State(app_state): State<AppState>) -> impl IntoResponse {
    let user_manager = &app_state.user_manager;

    // Get all users from the UserManager
    match user_manager.list_all_users().await {
        Ok(users) => {
            let user_responses: Vec<UserListResponse> = users
                .into_iter()
                .map(|user| UserListResponse {
                    id: user.id.clone(),
                    username: user.username.clone(),
                    role: user.canonical_role(),
                    created_at: user.created_at.to_rfc3339(),
                    last_login: user.last_login.map(|dt| dt.to_rfc3339()),
                })
                .collect();

            Json(json!({
                "success": true,
                "users": user_responses,
                "total": user_responses.len(),
                "timestamp": Utc::now().to_rfc3339()
            }))
        }
        Err(e) => {
            tracing::error!("Failed to list users: {}", e);
            Json(json!({
                "success": false,
                "error": "Failed to retrieve users",
                "timestamp": Utc::now().to_rfc3339()
            }))
        }
    }
}

pub async fn create_user(
    State(app_state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> impl IntoResponse {
    let user_manager = &app_state.user_manager;

    // Validate username
    if request.username.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Username cannot be empty",
                "timestamp": Utc::now().to_rfc3339()
            })),
        );
    }

    // Validate password
    if request.password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Password must be at least 6 characters long",
                "timestamp": Utc::now().to_rfc3339()
            })),
        );
    }

    let normalized_role = match UserRole::normalize(&request.role) {
        Ok(role) => role.to_string(),
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": error.to_string(),
                    "timestamp": Utc::now().to_rfc3339()
                })),
            );
        }
    };

    // Create user using UserManager
    match user_manager
        .create_user_public(&request.username, &request.password, &normalized_role)
        .await
    {
        Ok(user) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": format!("User '{}' created successfully", request.username),
                "user": {
                    "id": user.id,
                    "username": user.username,
                    "role": user.role,
                    "created_at": user.created_at.to_rfc3339(),
                },
                "timestamp": Utc::now().to_rfc3339()
            })),
        ),
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": format!("Failed to create user: {}", e),
                    "timestamp": Utc::now().to_rfc3339()
                })),
            )
        }
    }
}

pub async fn delete_user(
    State(app_state): State<AppState>,
    axum::extract::Path(username): axum::extract::Path<String>,
) -> impl IntoResponse {
    let user_manager = &app_state.user_manager;

    // Prevent deletion of admin user
    if username == "admin" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "Cannot delete user",
                "timestamp": Utc::now().to_rfc3339()
            })),
        );
    }

    // Delete user using UserManager
    match user_manager.delete_user(&username).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": format!("User '{}' deleted successfully", username),
                "timestamp": Utc::now().to_rfc3339()
            })),
        ),
        Err(e) => {
            tracing::error!("Failed to delete user: {}", e);
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": format!("Failed to delete user: {}", e),
                    "timestamp": Utc::now().to_rfc3339()
                })),
            )
        }
    }
}

/// Get application configuration - SECURITY: Never exposes full token
/// Returns masked preview only; backend uses stored token for API calls
pub async fn get_app_config(State(app_state): State<AppState>) -> Json<Value> {
    let github_token = app_state.config.github.token.clone();
    let has_token = !github_token.is_empty();

    Json(json!({
        "github": {
            "has_token": has_token,
            "token_preview": github_token_preview(&github_token),
            "rate_limit": if has_token { 5000 } else { 60 }
        },
        "web": {
            "port": app_state.config.web.port,
            "host": app_state.config.web.host
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn user(username: &str, role: &str) -> User {
        User {
            id: format!("{username}-id"),
            username: username.to_string(),
            password_hash: "not-used".to_string(),
            role: role.to_string(),
            created_at: Utc::now(),
            last_login: None,
            is_active: true,
        }
    }

    #[tokio::test]
    async fn auth_status_reports_authenticated_user() {
        let Json(response) = auth_status(Some(Extension(user("analyst", "operator")))).await;

        assert!(response.authenticated);
        assert_eq!(response.user.as_deref(), Some("analyst"));
    }

    #[tokio::test]
    async fn auth_status_reports_anonymous_request() {
        let Json(response) = auth_status(None).await;

        assert!(!response.authenticated);
        assert!(response.user.is_none());
    }

    #[tokio::test]
    async fn user_info_normalizes_legacy_role_names() {
        let Json(response) = user_info(Extension(user("legacy-user", "user"))).await;

        assert_eq!(response.username, "legacy-user");
        assert_eq!(response.role, "operator");
    }

    #[test]
    fn scraper_action_error_messages_are_stable_for_operator_flows() {
        assert_eq!(
            scraper_action_error_message(ScraperControlAction::Pause, "not running"),
            "Failed to pause scraper: not running"
        );
        assert_eq!(
            scraper_action_error_message(
                ScraperControlAction::Start,
                "Failed to initialize scraper runtime: missing db"
            ),
            "Failed to initialize scraper runtime: missing db"
        );
    }

    #[test]
    fn github_token_preview_never_returns_full_secret() {
        assert_eq!(github_token_preview(""), "");
        assert_eq!(github_token_preview("short"), "****");
        assert_eq!(github_token_preview("ghp_REDACTED_EXAMPLE"), "ghp_...cdef");
    }
}
