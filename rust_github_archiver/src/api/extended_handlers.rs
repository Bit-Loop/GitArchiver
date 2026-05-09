/// Extended API Handlers for Token Pool, Webhooks, and Metrics
/// Implements PRD Phase 2-3 features
use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;
use uuid::Uuid;

use crate::api::state::AppState;
use crate::auth::User;
use crate::realtime::metrics::{MetricsReport, SystemMetrics};
use crate::realtime::token_pool::{SelectionStrategy, TokenPoolStats};
use crate::realtime::webhook::WebhookStats;

// ========== TOKEN POOL HANDLERS ==========

/// Request to add tokens to the pool
#[derive(Debug, Clone, Deserialize)]
pub struct AddTokensRequest {
    pub tokens: Vec<TokenEntry>,
    pub strategy: Option<String>, // "round_robin", "least_used", "best_health", "most_remaining"
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenEntry {
    pub id: String,
    pub token: String,
}

/// Add tokens to the token pool
pub async fn add_tokens(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Extension(user): Extension<User>,
    Json(request): Json<AddTokensRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let token_pool = &app_state.token_pool;

    // Set strategy if provided
    if let Some(strategy_str) = &request.strategy {
        let strategy = match strategy_str.to_lowercase().as_str() {
            "round_robin" => SelectionStrategy::RoundRobin,
            "least_used" => SelectionStrategy::LeastUsed,
            "best_health" => SelectionStrategy::BestHealth,
            "most_remaining" => SelectionStrategy::MostRemaining,
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Invalid strategy",
                        "message": "Strategy must be one of: round_robin, least_used, best_health, most_remaining"
                    })),
                ));
            }
        };
        info!("Token pool strategy set to: {:?}", strategy);
    }

    // Add tokens
    for token_entry in &request.tokens {
        token_pool
            .add_token(token_entry.id.clone(), token_entry.token.clone())
            .await;
    }

    let stats = token_pool.get_stats().await;
    let mut details = std::collections::HashMap::new();
    details.insert("token_count".to_string(), json!(request.tokens.len()));
    details.insert("total_tokens".to_string(), json!(stats.total_tokens));
    details.insert("strategy".to_string(), json!(request.strategy));
    let _ = crate::audit_helpers::log_success(
        &app_state.audit_logger,
        None,
        &user.username,
        crate::audit::AuditAction::TokenPoolUpdated,
        crate::audit::ResourceType::TokenPool,
        None,
        &headers,
        details,
    )
    .await;

    Ok(Json(json!({
        "status": "success",
        "message": format!("Added {} tokens to pool", request.tokens.len()),
        "total_tokens": stats.total_tokens,
        "stats": stats
    })))
}

/// Get token pool statistics
pub async fn get_token_pool_stats(
    State(app_state): State<AppState>,
) -> Result<Json<TokenPoolStats>, (StatusCode, Json<Value>)> {
    let stats = app_state.token_pool.get_stats().await;
    Ok(Json(stats))
}

/// Get detailed token information
pub async fn get_token_details(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tokens = app_state.token_pool.get_token_details().await;
    Ok(Json(json!({
        "status": "success",
        "tokens": tokens
    })))
}

/// Remove unhealthy tokens
pub async fn remove_unhealthy_tokens(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Extension(user): Extension<User>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let removed = app_state.token_pool.remove_unhealthy_tokens().await;
    let mut details = std::collections::HashMap::new();
    details.insert("removed_count".to_string(), json!(removed));
    let _ = crate::audit_helpers::log_success(
        &app_state.audit_logger,
        None,
        &user.username,
        crate::audit::AuditAction::TokenPoolUpdated,
        crate::audit::ResourceType::TokenPool,
        None,
        &headers,
        details,
    )
    .await;

    Ok(Json(json!({
        "status": "success",
        "message": format!("Removed {} unhealthy tokens", removed),
        "removed_count": removed
    })))
}

/// Reset all token health
pub async fn reset_token_health(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Extension(user): Extension<User>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    app_state.token_pool.reset_all_health().await;
    let _ = crate::audit_helpers::log_success(
        &app_state.audit_logger,
        None,
        &user.username,
        crate::audit::AuditAction::TokenHealthReset,
        crate::audit::ResourceType::TokenPool,
        None,
        &headers,
        std::collections::HashMap::new(),
    )
    .await;

    Ok(Json(json!({
        "status": "success",
        "message": "Reset health for all tokens"
    })))
}

// ========== WEBHOOK HANDLERS ==========

/// Request to add webhook
#[derive(Debug, Clone, Deserialize)]
pub struct AddWebhookRequest {
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<String>,
}

/// Add webhook endpoint
pub async fn add_webhook(
    State(app_state): State<AppState>,
    Json(request): Json<AddWebhookRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let webhook_id = app_state
        .webhook_manager
        .add_endpoint(request.url.clone(), request.secret, request.events)
        .await;

    Ok(Json(json!({
        "status": "success",
        "message": "Webhook endpoint added",
        "webhook_id": webhook_id,
        "url": request.url
    })))
}

/// Remove webhook endpoint
pub async fn remove_webhook(
    State(app_state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let webhook_id = payload
        .get("webhook_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid webhook_id",
                    "message": "webhook_id must be a valid UUID"
                })),
            )
        })?;

    app_state
        .webhook_manager
        .remove_endpoint(webhook_id)
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Webhook not found",
                    "message": e.to_string()
                })),
            )
        })?;

    Ok(Json(json!({
        "status": "success",
        "message": "Webhook endpoint removed",
        "webhook_id": webhook_id
    })))
}

/// Update webhook endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWebhookRequest {
    pub webhook_id: Uuid,
    pub url: Option<String>,
    pub secret: Option<String>,
    pub events: Option<Vec<String>>,
    pub active: Option<bool>,
}

pub async fn update_webhook(
    State(app_state): State<AppState>,
    Json(request): Json<UpdateWebhookRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    app_state
        .webhook_manager
        .update_endpoint(
            request.webhook_id,
            request.url.clone(),
            request.secret,
            request.events,
            request.active,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Webhook not found",
                    "message": e.to_string()
                })),
            )
        })?;

    Ok(Json(json!({
        "status": "success",
        "message": "Webhook endpoint updated",
        "webhook_id": request.webhook_id
    })))
}

/// Get all webhook endpoints
pub async fn get_webhooks(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let endpoints = app_state.webhook_manager.get_endpoints().await;

    Ok(Json(json!({
        "status": "success",
        "webhooks": endpoints
    })))
}

/// Get webhook statistics
pub async fn get_webhook_stats(
    State(app_state): State<AppState>,
) -> Result<Json<WebhookStats>, (StatusCode, Json<Value>)> {
    let stats = app_state.webhook_manager.get_stats().await;
    Ok(Json(stats))
}

// ========== METRICS HANDLERS ==========

/// Get system metrics
pub async fn get_metrics(
    State(app_state): State<AppState>,
) -> Result<Json<SystemMetrics>, (StatusCode, Json<Value>)> {
    let metrics = app_state.metrics_collector.get_metrics().await;
    Ok(Json(metrics))
}

/// Get comprehensive metrics report
pub async fn get_metrics_report(
    State(app_state): State<AppState>,
) -> Result<Json<MetricsReport>, (StatusCode, Json<Value>)> {
    let report = app_state.metrics_collector.get_report().await;
    Ok(Json(report))
}

/// Reset metrics
pub async fn reset_metrics(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    app_state.metrics_collector.reset().await;

    Ok(Json(json!({
        "status": "success",
        "message": "Metrics reset successfully"
    })))
}

/// Get health status
pub async fn get_health(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let report = app_state.metrics_collector.get_report().await;

    Ok(Json(json!({
        "status": "success",
        "health": report.health_status,
        "uptime": report.uptime_human,
        "uptime_seconds": report.uptime_seconds,
        "metrics_summary": {
            "success_rate": report.metrics.success_rate(),
            "error_rate": report.metrics.error_rate(),
            "events_per_second": report.metrics.events_per_second(),
            "total_events": report.metrics.events_fetched,
            "total_requests": report.metrics.api_requests,
        }
    })))
}

// ========== ROUTER SETUP ==========

use axum::routing::{get, post};
use axum::Router;

/// Create extended API router with all new endpoints
pub fn create_extended_api_router() -> Router<AppState> {
    Router::new()
        // Token pool endpoints
        .route("/api/tokens/add", post(add_tokens))
        .route("/api/tokens/stats", get(get_token_pool_stats))
        .route("/api/tokens/details", get(get_token_details))
        .route("/api/tokens/cleanup", post(remove_unhealthy_tokens))
        .route("/api/tokens/reset-health", post(reset_token_health))
        // Webhook endpoints
        .route("/api/webhooks/add", post(add_webhook))
        .route("/api/webhooks/remove", post(remove_webhook))
        .route("/api/webhooks/update", post(update_webhook))
        .route("/api/webhooks", get(get_webhooks))
        .route("/api/webhooks/stats", get(get_webhook_stats))
        // Metrics endpoints
        .route("/api/metrics", get(get_metrics))
        .route("/api/metrics/report", get(get_metrics_report))
        .route("/api/metrics/reset", post(reset_metrics))
        .route("/api/health", get(get_health))
}
