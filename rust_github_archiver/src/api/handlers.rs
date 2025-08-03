// API handlers placeholder
// API handlers implementation
use axum::{extract::{Extension, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;
use std::sync::Arc;

use crate::auth::{User, UserManager, create_token};
use crate::api::state::AppState;

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
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
        }
    }
}

pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": Utc::now().to_rfc3339(),
        "service": "github-archiver-rust",
        "version": "2.0.0"
    }))
}

pub async fn login(
    State(app_state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<Value>)> {
    // Authenticate user
    let user = app_state.user_manager
        .authenticate(&payload.username, &payload.password)
        .await
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Authentication failed",
                    "message": "Invalid username or password"
                })),
            )
        })?;

    // Update last login time
    if let Err(e) = app_state.user_manager.update_last_login(&user.username).await {
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

    Ok(Json(LoginResponse {
        token,
        user: user.into(),
        expires_at,
    }))
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

pub async fn logout() -> Json<Value> {
    // In a stateless JWT system, logout is handled client-side by discarding the token
    // Server-side logout would require token blacklisting, which we're not implementing here
    Json(json!({
        "message": "Logged out successfully",
        "timestamp": Utc::now().to_rfc3339()
    }))
}

pub async fn user_info(Extension(user): Extension<User>) -> Json<UserInfo> {
    Json(user.into())
}

// Scraper control handlers
pub async fn start_scraper(State(app_state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Initialize main scraper if not already done
    if let Err(e) = app_state.initialize_main_scraper().await {
        eprintln!("Failed to initialize main scraper: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    match app_state.scraper_manager.start() {
        Ok(()) => {
            // Start the main scraper
            if let Ok(mut scraper_opt) = app_state.main_scraper.lock() {
                if let Some(ref mut scraper) = *scraper_opt {
                    if let Err(e) = scraper.start().await {
                        eprintln!("Failed to start main scraper: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }

            Ok(Json(json!({
                "message": "Scraper started successfully",
                "status": "running",
                "timestamp": Utc::now().to_rfc3339()
            })))
        }
        Err(e) => {
            eprintln!("Failed to start scraper: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn stop_scraper(State(app_state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match app_state.scraper_manager.stop() {
        Ok(()) => {
            // Stop the main scraper
            if let Ok(mut scraper_opt) = app_state.main_scraper.lock() {
                if let Some(ref mut scraper) = *scraper_opt {
                    if let Err(e) = scraper.stop().await {
                        eprintln!("Failed to stop main scraper: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }

            Ok(Json(json!({
                "message": "Scraper stopped successfully",
                "status": "stopped",
                "timestamp": Utc::now().to_rfc3339()
            })))
        }
        Err(e) => {
            eprintln!("Failed to stop scraper: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn pause_scraper(State(app_state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match app_state.scraper_manager.pause() {
        Ok(()) => {
            // Pause the main scraper
            if let Ok(mut scraper_opt) = app_state.main_scraper.lock() {
                if let Some(ref mut scraper) = *scraper_opt {
                    if let Err(e) = scraper.pause().await {
                        eprintln!("Failed to pause main scraper: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }

            Ok(Json(json!({
                "message": "Scraper paused successfully",
                "status": "paused",
                "timestamp": Utc::now().to_rfc3339()
            })))
        }
        Err(e) => Ok(Json(json!({
            "error": e,
            "timestamp": Utc::now().to_rfc3339()
        })))
    }
}

pub async fn resume_scraper(State(app_state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match app_state.scraper_manager.resume() {
        Ok(()) => {
            // Resume the main scraper
            if let Ok(mut scraper_opt) = app_state.main_scraper.lock() {
                if let Some(ref mut scraper) = *scraper_opt {
                    if let Err(e) = scraper.resume().await {
                        eprintln!("Failed to resume main scraper: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }

            Ok(Json(json!({
                "message": "Scraper resumed successfully",
                "status": "running",
                "timestamp": Utc::now().to_rfc3339()
            })))
        }
        Err(e) => Ok(Json(json!({
            "error": e,
            "timestamp": Utc::now().to_rfc3339()
        })))
    }
}

pub async fn restart_scraper(State(app_state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match app_state.scraper_manager.restart() {
        Ok(()) => {
            // Restart the main scraper
            if let Ok(mut scraper_opt) = app_state.main_scraper.lock() {
                if let Some(ref mut scraper) = *scraper_opt {
                    if let Err(e) = scraper.restart().await {
                        eprintln!("Failed to restart main scraper: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }

            Ok(Json(json!({
                "message": "Scraper restarted successfully",
                "status": "running",
                "timestamp": Utc::now().to_rfc3339()
            })))
        }
        Err(e) => {
            eprintln!("Failed to restart scraper: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn scraper_status(State(app_state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match app_state.scraper_manager.get_status() {
        Ok(status) => Ok(Json(json!({
            "status": match status.state {
                crate::scraper::ScraperState::Stopped => "stopped",
                crate::scraper::ScraperState::Running => "running",
                crate::scraper::ScraperState::Paused => "paused",
                crate::scraper::ScraperState::Error(_) => "error",
            },
            "scraper_running": matches!(status.state, crate::scraper::ScraperState::Running),
            "last_updated": status.last_updated.to_rfc3339(),
            "events_processed": status.events_processed,
            "files_processed": status.files_processed,
            "current_file": status.current_file,
            "processing_rate": status.processing_rate,
            "error_count": status.error_count
        }))),
        Err(e) => {
            eprintln!("Failed to get scraper status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn system_status(State(app_state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Get comprehensive status if main scraper is available
    match app_state.get_comprehensive_status().await {
        Ok(status) => {
            let mut response = json!({
                "status": if status.running { "running" } else { "stopped" },
                "scraper_running": status.running,
                "api_healthy": true,
                "timestamp": Utc::now().to_rfc3339(),
                "uptime_seconds": status.uptime_seconds,
                "total_files_processed": status.total_files_processed,
                "total_events_processed": status.total_events_processed,
                "total_errors": status.total_errors
            });

            // Add database health if available
            if let Some(db_health) = status.database_health {
                response["database_connected"] = json!(db_health.is_connected);
                response["database_connections"] = json!(db_health.connection_count);
            } else {
                response["database_connected"] = json!(false);
            }

            // Add resource status if available
            if let Some(resource_status) = status.resource_status {
                response["memory_usage"] = json!(format!("{:.1} GB", resource_status.memory.used_gb));
                response["cpu_usage"] = json!(format!("{:.1}%", resource_status.cpu.percent));
                response["emergency_mode"] = json!(resource_status.emergency_mode);
            } else {
                response["memory_usage"] = json!("0 MB");
                response["cpu_usage"] = json!("0%");
                response["emergency_mode"] = json!(false);
            }

            // Add quality metrics if available
            if let Some(quality_metrics) = status.quality_metrics {
                response["data_quality"] = json!({
                    "total_events": quality_metrics.total_events,
                    "unique_actors": quality_metrics.unique_actors,
                    "unique_repos": quality_metrics.unique_repos,
                    "quality_score": quality_metrics.quality_score
                });
            }

            Ok(Json(response))
        }
        Err(e) => {
            eprintln!("Failed to get comprehensive status: {}", e);
            
            // Fallback to basic status
            let scraper_running = app_state.scraper_manager.is_running();
            
            Ok(Json(json!({
                "status": "healthy",
                "scraper_running": scraper_running,
                "database_connected": false,
                "api_healthy": true,
                "timestamp": Utc::now().to_rfc3339(),
                "uptime": "0d 0h 0m",
                "memory_usage": "0 MB",
                "cpu_usage": "0%",
                "error": "Failed to get comprehensive status"
            })))
        }
    }
}
