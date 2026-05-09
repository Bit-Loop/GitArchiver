use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::api::state::AppState;
use crate::core::database::EventPreview;
use crate::realtime::{GitHubEventMonitor, RateLimitStatus};

/// Request body for starting the monitor
#[derive(Debug, Clone, Deserialize)]
pub struct StartMonitorRequest {
    pub requests_per_minute: Option<u32>,
    pub auto_adjust: Option<bool>,
    pub github_token: Option<String>,
}

/// Request body for updating rate limit configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub auto_adjust: bool,
}

/// Response for monitor status
#[derive(Debug, Clone, Serialize)]
pub struct MonitorStatus {
    pub running: bool,
    pub events_processed: u64,
    pub last_event_id: Option<String>,
    pub rate_limit: RateLimitStatus,
    pub lifetime_events: i64,
}

/// Query params for event preview endpoint
#[derive(Debug, Default, Deserialize)]
pub struct EventPreviewParams {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct EventSearchParams {
    pub q: String,
    pub limit: Option<i64>,
}

/// Start GitHub Events API monitoring
pub async fn start_event_monitor(
    State(app_state): State<AppState>,
    Json(request): Json<StartMonitorRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("API: Starting GitHub Events monitor");

    // Check if already running (quick lock check)
    {
        let monitor_lock = app_state.event_monitor.lock().await;
        if let Some(monitor) = monitor_lock.as_ref() {
            if monitor.is_running().await {
                return Ok(Json(json!({
                    "status": "info",
                    "message": "Event monitor is already running",
                    "running": true
                })));
            }
        }
    } // Lock released

    // Get GitHub token - priority: request body > config > empty (unauthenticated)
    let github_token = request
        .github_token
        .or_else(|| app_state.config.github_token.clone())
        .unwrap_or_else(|| {
            info!("No GitHub token provided - using unauthenticated requests (60 req/hour limit)");
            String::new()
        });

    // Get rate limit configuration
    let requests_per_minute = request.requests_per_minute.unwrap_or(5);
    let auto_adjust = request.auto_adjust.unwrap_or(false);

    // Validate rate limit
    if !(1..=60).contains(&requests_per_minute) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid rate limit",
                "message": "Requests per minute must be between 1 and 60"
            })),
        ));
    }

    // Initialize monitor if not exists
    let monitor = GitHubEventMonitor::new(&github_token)
        .await
        .map_err(|e| {
            error!("Failed to create event monitor: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to create event monitor",
                    "message": e.to_string()
                })),
            )
        })?
        .with_persistence(app_state.persistence.clone())
        .with_rate_limit(requests_per_minute, auto_adjust)
        .with_scanning_service(app_state.scanning_service.clone())
        .with_metrics_collector(app_state.metrics_collector.clone());

    info!(
        "✅ Monitor configured: {} req/min, auto-adjust: {}",
        requests_per_minute, auto_adjust
    );

    // Wrap in Arc for shared ownership
    let monitor_arc = Arc::new(monitor);

    // Store monitor in state
    {
        let mut monitor_lock = app_state.event_monitor.lock().await;
        *monitor_lock = Some(monitor_arc.clone());
    } // Lock released immediately

    // Start monitoring in a background task with its own shared monitor handle.
    tokio::spawn(async move {
        info!("🚀 Starting event monitor background task");
        match monitor_arc.start_monitoring().await {
            Ok(_) => info!("✅ Event monitor loop completed"),
            Err(e) => error!("❌ Event monitor failed: {}", e),
        }
    });

    info!("✅ GitHub Events monitor started in background");

    Ok(Json(json!({
        "status": "success",
        "message": "GitHub Events monitor started",
        "running": true
    })))
}

/// Stop GitHub Events API monitoring
pub async fn stop_event_monitor(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("API: Stopping GitHub Events monitor");

    let monitor_lock = app_state.event_monitor.lock().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor.stop_monitoring().await;
        drop(monitor_lock); // Release lock before logging

        info!("✅ GitHub Events monitor stopped successfully");

        Ok(Json(json!({
            "status": "success",
            "message": "GitHub Events monitor stopped",
            "running": false
        })))
    } else {
        drop(monitor_lock);
        Ok(Json(json!({
            "status": "info",
            "message": "Event monitor was not running",
            "running": false
        })))
    }
}

/// Get GitHub Events monitor status
pub async fn get_event_monitor_status(State(app_state): State<AppState>) -> Json<MonitorStatus> {
    let monitor_lock = app_state.event_monitor.lock().await;

    let lifetime_events = match app_state.persistence.total_event_count().await {
        Ok(count) => count,
        Err(e) => {
            warn!("Failed to fetch lifetime event count: {}", e);
            0
        }
    };

    if let Some(monitor) = monitor_lock.as_ref() {
        // Get real status from monitor (Arc is cheap to clone)
        let running = monitor.is_running().await;
        let events_processed = monitor.get_events_processed().await;
        let rate_limit = monitor.rate_limiter().get_status().await;

        // Lock is dropped here
        drop(monitor_lock);

        Json(MonitorStatus {
            running,
            events_processed,
            last_event_id: None, // Could expose if needed
            rate_limit,
            lifetime_events,
        })
    } else {
        // Monitor not initialized
        drop(monitor_lock);

        Json(MonitorStatus {
            running: false,
            events_processed: 0,
            last_event_id: None,
            rate_limit: RateLimitStatus {
                requests_per_minute: 5,
                requests_last_minute: 0,
                auto_adjust_enabled: true,
                is_paused: false,
                retry_after_seconds: None,
                pause_remaining_seconds: None,
                total_requests: 0,
                rate_limit_hits: 0,
            },
            lifetime_events,
        })
    }
}

/// Update rate limit configuration
pub async fn update_rate_limit(
    State(app_state): State<AppState>,
    Json(config): Json<RateLimitConfig>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!(
        "API: Updating rate limit to {} req/min, auto_adjust: {}",
        config.requests_per_minute, config.auto_adjust
    );

    // Validate rate limit
    if config.requests_per_minute < 1 || config.requests_per_minute > 60 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid rate limit",
                "message": "Rate limit must be between 1 and 60 requests per minute"
            })),
        ));
    }

    let monitor_lock = app_state.event_monitor.lock().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor
            .rate_limiter()
            .set_rate(config.requests_per_minute)
            .await;
        monitor
            .rate_limiter()
            .set_auto_adjust(config.auto_adjust)
            .await;

        info!("✅ Rate limit configuration updated");

        Ok(Json(json!({
            "status": "success",
            "message": "Rate limit updated",
            "config": {
                "requests_per_minute": config.requests_per_minute,
                "auto_adjust": config.auto_adjust
            }
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Monitor not initialized",
                "message": "Please start the event monitor first"
            })),
        ))
    }
}

/// Pause event monitoring (manual pause)
pub async fn pause_event_monitor(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("API: Pausing GitHub Events monitor");

    let monitor_lock = app_state.event_monitor.lock().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor.stop_monitoring().await;

        Ok(Json(json!({
            "status": "success",
            "message": "Event monitor paused",
            "running": false
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Monitor not initialized",
                "message": "Event monitor is not running"
            })),
        ))
    }
}

/// Resume event monitoring (restart if stopped)
pub async fn resume_event_monitor(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("API: Resuming GitHub Events monitor");

    let monitor_lock = app_state.event_monitor.lock().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        if !monitor.is_running().await {
            monitor.start_monitoring().await.map_err(|e| {
                error!("Failed to resume monitor: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to resume monitor",
                        "message": e.to_string()
                    })),
                )
            })?;
        }

        Ok(Json(json!({
            "status": "success",
            "message": "Event monitor resumed"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Monitor not found",
                "message": "Event monitor has not been started"
            })),
        ))
    }
}

/// Reset rate limiter statistics
pub async fn reset_rate_limiter_stats(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("API: Resetting rate limiter statistics");

    let monitor_lock = app_state.event_monitor.lock().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor.rate_limiter().reset_stats().await;

        Ok(Json(json!({
            "status": "success",
            "message": "Rate limiter statistics reset"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Monitor not initialized",
                "message": "Event monitor is not running"
            })),
        ))
    }
}

/// Return the most recently stored events so the UI can preview persisted data
pub async fn get_recent_event_samples(
    State(app_state): State<AppState>,
    Query(params): Query<EventPreviewParams>,
) -> Result<Json<Vec<EventPreview>>, (StatusCode, Json<Value>)> {
    let limit = params.limit.unwrap_or(15);

    match app_state.persistence.recent_events(limit).await {
        Ok(events) => Ok(Json(events)),
        Err(e) => {
            error!("Failed to load recent events: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "failed_to_fetch_events",
                    "message": e.to_string()
                })),
            ))
        }
    }
}

/// Search persisted events by repo, actor, or event type
pub async fn search_events(
    State(app_state): State<AppState>,
    Query(params): Query<EventSearchParams>,
) -> Result<Json<Vec<EventPreview>>, (StatusCode, Json<Value>)> {
    let limit = params.limit.unwrap_or(25);

    match app_state.persistence.search_events(&params.q, limit).await {
        Ok(events) => Ok(Json(events)),
        Err(e) => {
            error!("Failed to search events: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "failed_to_search_events",
                    "message": e.to_string()
                })),
            ))
        }
    }
}
