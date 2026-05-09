// API routes implementation
use axum::{
    middleware,
    response::Html,
    routing::{get, post},
    Extension, Router,
};
use std::path::PathBuf;
use tower_http::services::ServeDir;

use crate::api::ai_handlers::{get_ai_triage, run_ai_triage};
use crate::api::api_key_handlers::{
    create_api_key, deactivate_api_key, delete_api_key, get_api_key, get_api_key_statistics,
    get_api_key_types, list_api_keys, regenerate_api_key, validate_api_key_handler,
};
use crate::api::audit_handlers::{
    cleanup_audit_logs, export_audit_logs, get_audit_log, get_audit_statistics, list_audit_logs,
};
use crate::api::extended_handlers::{
    add_tokens, add_webhook, get_health as get_extended_health, get_metrics, get_metrics_report,
    get_token_details, get_token_pool_stats, get_webhook_stats, get_webhooks,
    remove_unhealthy_tokens, remove_webhook, reset_metrics, reset_token_health, update_webhook,
};
use crate::api::handlers::{
    auth_status,
    auth_verify,
    batch_scan_repositories,
    // User management handlers
    change_password,
    create_user,
    database_restart,
    // Database management handlers
    database_start,
    database_stats,
    database_status,
    database_stop,
    delete_user,
    export_scan_results,
    get_app_config,
    get_detectors,
    get_scan_results,
    get_scan_statistics,
    get_scanner_metrics,
    get_scheduled_scans,
    health_check,
    list_users,
    login,
    logout,
    pause_scraper,
    ping,
    restart_scraper,
    resume_scraper,
    // Scanner handlers
    scan_repository,
    schedule_scan,
    scraper_control,
    scraper_status_simple,
    start_scraper,
    stop_scraper,
    system_metrics,
    system_status_simple,
    user_info,
};
use crate::api::health_handlers::{detailed_health_handler, liveness_handler, readiness_handler};
use crate::api::maintenance_handlers::repair_scan_state;
use crate::api::middleware::cors_middleware as api_cors_middleware;
use crate::api::monitoring_handlers::{
    export_logs, get_detection_overview, get_detection_trends, get_realtime_metrics,
    get_system_logs, realtime_websocket,
};
use crate::api::realtime_handlers::{
    get_event_monitor_status, get_recent_event_samples, pause_event_monitor,
    reset_rate_limiter_stats, resume_event_monitor, search_events, start_event_monitor,
    stop_event_monitor, update_rate_limit,
};
use crate::api::state::AppState;
use crate::auth::{
    admin_auth_middleware, auth_middleware, operator_auth_middleware, optional_auth_middleware,
};
use crate::logging;
use crate::metrics;
use crate::rate_limiter;
use crate::security;

async fn serve_dashboard() -> Html<String> {
    for path in dashboard_file_candidates() {
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                tracing::debug!("Loaded dashboard from {}", path.display());
                return Html(content);
            }
            Err(e) => {
                tracing::debug!("Failed to load dashboard from {}: {}", path.display(), e);
            }
        }
    }

    tracing::error!("dashboard.html file not found");

    Html(
        r#"<html><body>
        <h1>Dashboard not found</h1>
        <p>The dashboard asset is not available in this deployment.</p>
        </body></html>"#
            .to_string(),
    )
}

fn dashboard_file_candidates() -> Vec<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    vec![
        manifest_dir.join("dashboard.html"),
        PathBuf::from("dashboard.html"),
        PathBuf::from("./dashboard.html"),
        PathBuf::from("../dashboard.html"),
    ]
}

fn resolve_dashboard_assets_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("dashboard_assets"),
        PathBuf::from("dashboard_assets"),
        PathBuf::from("./dashboard_assets"),
        PathBuf::from("../dashboard_assets"),
    ];

    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("dashboard_assets"))
}

pub fn create_routes(app_state: AppState) -> Router {
    let read_only_routes = Router::new()
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/user", get(user_info))
        .route("/api/auth/change-password", post(change_password))
        .route("/api/scanner/export", get(export_scan_results))
        .route("/api/v1/scanner/export", get(export_scan_results))
        .route("/api/scanner/schedules", get(get_scheduled_scans))
        .route("/api/v1/scanner/schedules", get(get_scheduled_scans))
        .route("/api/monitoring/overview", get(get_detection_overview))
        .route("/api/v1/monitoring/overview", get(get_detection_overview))
        .route("/api/monitoring/trends", get(get_detection_trends))
        .route("/api/v1/monitoring/trends", get(get_detection_trends))
        .route("/api/monitoring/logs", get(get_system_logs))
        .route("/api/v1/monitoring/logs", get(get_system_logs))
        .route("/api/monitoring/logs/export", get(export_logs))
        .route("/api/v1/monitoring/logs/export", get(export_logs))
        .route("/api/metrics", get(get_metrics))
        .route("/api/metrics/report", get(get_metrics_report))
        .route("/api/health/extended", get(get_extended_health))
        .route("/api/ai/triage/:job_id", get(get_ai_triage))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state.user_manager.clone(),
            auth_middleware,
        ));

    let operator_routes = Router::new()
        .route("/api/start-scraper", post(start_scraper))
        .route("/api/stop-scraper", post(stop_scraper))
        .route("/api/pause-scraper", post(pause_scraper))
        .route("/api/resume-scraper", post(resume_scraper))
        .route("/api/restart-scraper", post(restart_scraper))
        .route("/api/scraper/control", post(scraper_control))
        .route("/api/v1/scraper/control", post(scraper_control))
        .route("/api/scanner/scan", post(scan_repository))
        .route("/api/v1/scanner/scan", post(scan_repository))
        .route("/api/scanner/batch-scan", post(batch_scan_repositories))
        .route("/api/v1/scanner/batch-scan", post(batch_scan_repositories))
        .route("/api/scanner/metrics", get(get_scanner_metrics))
        .route("/api/v1/scanner/metrics", get(get_scanner_metrics))
        .route("/api/scanner/schedule", post(schedule_scan))
        .route("/api/v1/scanner/schedule", post(schedule_scan))
        .route("/api/tokens/stats", get(get_token_pool_stats))
        .route("/api/tokens/details", get(get_token_details))
        .route("/api/webhooks/list", get(get_webhooks))
        .route("/api/webhooks/stats", get(get_webhook_stats))
        .route("/api/realtime/start", post(start_event_monitor))
        .route("/api/realtime/stop", post(stop_event_monitor))
        .route("/api/realtime/pause", post(pause_event_monitor))
        .route("/api/realtime/resume", post(resume_event_monitor))
        .route("/api/realtime/config", post(update_rate_limit))
        .route("/api/realtime/stats/reset", post(reset_rate_limiter_stats))
        .route("/api/ai/triage", post(run_ai_triage))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state.user_manager.clone(),
            operator_auth_middleware,
        ));

    let admin_routes = Router::new()
        .route("/api/database/start", post(database_start))
        .route("/api/database/stop", post(database_stop))
        .route("/api/database/restart", post(database_restart))
        .route("/api/auth/users", get(list_users))
        .route("/api/auth/users", post(create_user))
        .route("/api/auth/users/:id", axum::routing::delete(delete_user))
        .route("/api/keys", post(create_api_key))
        .route("/api/keys", get(list_api_keys))
        .route("/api/keys/:id", get(get_api_key))
        .route("/api/keys/:id/deactivate", post(deactivate_api_key))
        .route(
            "/api/keys/:id/delete",
            axum::routing::delete(delete_api_key),
        )
        .route("/api/keys/:id/regenerate", post(regenerate_api_key))
        .route("/api/keys/statistics", get(get_api_key_statistics))
        .route("/api/keys/types", get(get_api_key_types))
        .route("/api/keys/validate", post(validate_api_key_handler))
        .route("/api/tokens/add", post(add_tokens))
        .route(
            "/api/tokens/remove-unhealthy",
            post(remove_unhealthy_tokens),
        )
        .route("/api/tokens/reset-health", post(reset_token_health))
        .route("/api/webhooks/add", post(add_webhook))
        .route("/api/webhooks/remove", post(remove_webhook))
        .route("/api/webhooks/update", post(update_webhook))
        .route("/api/metrics/reset", post(reset_metrics))
        .route("/api/audit/logs", get(list_audit_logs))
        .route("/api/audit/logs/:id", get(get_audit_log))
        .route("/api/audit/stats", get(get_audit_statistics))
        .route("/api/audit/export", get(export_audit_logs))
        .route("/api/audit/cleanup", post(cleanup_audit_logs))
        .route("/api/admin/repair/scan-state", post(repair_scan_state))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state.user_manager.clone(),
            admin_auth_middleware,
        ));

    // Create auth status route with optional auth middleware
    let auth_status_route = Router::new()
        .route("/api/auth/status", get(auth_status))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state.user_manager.clone(),
            optional_auth_middleware,
        ));

    // Create public routes
    Router::new()
        // Simple test endpoint
        .route("/ping", get(ping))
        // Public routes
        .route("/api/health", get(health_check))
        .route("/api/auth/login", post(login))
        .route("/api/auth/verify", get(auth_verify))
        // Status endpoints (public)
        .route("/api/status", get(system_status_simple))
        .route("/api/v1/status", get(system_status_simple))
        .route("/api/system/status", get(system_status_simple))
        .route("/api/v1/system/status", get(system_status_simple))
        .route("/api/system/metrics", get(system_metrics))
        .route("/api/v1/system/metrics", get(system_metrics))
        .route("/api/scraper/status", get(scraper_status_simple))
        .route("/api/v1/scraper/status", get(scraper_status_simple))
        .route("/api/events/search", get(search_events))
        // Prometheus metrics endpoint (public)
        .route("/metrics", get(metrics::metrics_handler))
        // Kubernetes health check endpoints (public)
        .route("/health", get(detailed_health_handler))
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
        .route("/api/database/status", get(database_status))
        .route("/api/v1/database/status", get(database_status))
        .route("/api/database/stats", get(database_stats))
        .route("/api/v1/database/stats", get(database_stats))
        // Scanner public endpoints
        .route("/api/scanner/results", get(get_scan_results))
        .route("/api/v1/scanner/results", get(get_scan_results))
        .route("/api/scanner/statistics", get(get_scan_statistics))
        .route("/api/v1/scanner/statistics", get(get_scan_statistics))
        .route("/api/scanner/detectors", get(get_detectors))
        .route("/api/v1/scanner/detectors", get(get_detectors))
        // Monitoring public endpoints
        .route("/api/monitoring/metrics", get(get_realtime_metrics))
        .route("/api/v1/monitoring/metrics", get(get_realtime_metrics))
        .route("/api/monitoring/ws", get(realtime_websocket))
        .route("/api/v1/monitoring/ws", get(realtime_websocket))
        // GitHub Events realtime monitoring endpoints
        .route("/api/realtime/status", get(get_event_monitor_status))
        .route("/api/realtime/events", get(get_recent_event_samples))
        .route("/api/config", get(get_app_config))
        // Dashboard routes (public access)
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/dashboard.html", get(serve_dashboard))
        .nest_service(
            "/dashboard-assets",
            ServeDir::new(resolve_dashboard_assets_dir()),
        )
        .route(
            "/favicon.ico",
            get(|| async {
                axum::response::Response::builder()
                    .status(204)
                    .body(axum::body::Body::empty())
                    .expect("Failed to build empty response for favicon")
            }),
        )
        .with_state(app_state.clone())
        // Merge the protected routes
        .merge(auth_status_route)
        .merge(read_only_routes)
        .merge(operator_routes)
        .merge(admin_routes)
        // Add security headers middleware (extension then middleware)
        .layer(middleware::from_fn(security::security_headers_middleware))
        .layer(Extension(app_state.security_config.clone()))
        // Add CORS middleware (extension then middleware)
        .layer(middleware::from_fn(api_cors_middleware))
        .layer(Extension(app_state.cors_config.clone()))
        // Add rate limiting middleware (extension then middleware)
        .layer(middleware::from_fn(rate_limiter::rate_limit_middleware))
        .layer(Extension(app_state.rate_limiter.clone()))
        // Add request size and timeout middleware (no config needed, use constants)
        .layer(middleware::from_fn(security::request_size_limit_middleware))
        .layer(middleware::from_fn(security::request_timeout_middleware))
        // Add structured logging middleware for all requests (should be last to log everything)
        .layer(middleware::from_fn(logging::middleware::log_request))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_assets_resolution_returns_existing_dir_or_stable_fallback() {
        let path = resolve_dashboard_assets_dir();

        assert!(
            path.as_path() == std::path::Path::new("dashboard_assets") || path.is_dir(),
            "unexpected dashboard assets path: {}",
            path.display()
        );
    }

    #[test]
    fn dashboard_file_resolution_does_not_use_machine_specific_paths() {
        let candidates = dashboard_file_candidates();

        assert!(candidates
            .iter()
            .any(|path| path.ends_with("dashboard.html")));
        let source = include_str!("routes.rs");
        let machine_path_marker = ["Documents", "GITHUB", "GitArchiver"].join("/");
        assert!(
            !source.contains(&machine_path_marker),
            "dashboard resolution should not embed developer machine paths"
        );
    }
}
