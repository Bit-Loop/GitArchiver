// API routes implementation
use axum::{Router, routing::{get, post}, middleware, response::Html};

use crate::auth::{auth_middleware, optional_auth_middleware};
use crate::api::handlers::{
    health_check, login, logout, user_info, auth_status,
    start_scraper, stop_scraper, pause_scraper, resume_scraper, 
    restart_scraper, system_status_simple, scraper_status_simple,
    system_metrics, database_status, scraper_control, auth_verify,
    // Database management handlers
    database_start, database_stop, database_restart, database_stats,
    // User management handlers
    change_password, list_users, create_user, delete_user,
    // Scanner handlers
    scan_repository, batch_scan_repositories, get_scan_results,
    get_scan_statistics, get_detectors, export_scan_results,
    schedule_scan, get_scheduled_scans
};
use crate::api::api_key_handlers::{
    create_api_key, list_api_keys, get_api_key, deactivate_api_key,
    delete_api_key, regenerate_api_key, get_api_key_statistics,
    get_api_key_types, validate_api_key_handler
};
use crate::api::state::AppState;

// Handler to serve dashboard.html
async fn serve_dashboard() -> Html<String> {
    // Try multiple possible locations for the dashboard file
    let possible_paths = [
        "/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/dashboard.html",
        "dashboard.html",
        "./dashboard.html", 
        "../dashboard.html"
    ];
    
    for path in &possible_paths {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                tracing::info!("Successfully loaded dashboard from: {}", path);
                return Html(content);
            }
            Err(e) => {
                tracing::debug!("Failed to load dashboard from {}: {}", path, e);
            }
        }
    }
    
    // Get current working directory for debugging
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    
    tracing::error!("Dashboard file not found in any location. Current working directory: {}", cwd);
    
    Html(format!(
        r#"<html><body>
        <h1>Dashboard not found</h1>
        <p>dashboard.html file is missing</p>
        <p>Current working directory: {}</p>
        <p>Checked paths:</p>
        <ul>
            <li>/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/dashboard.html</li>
            <li>dashboard.html</li>
            <li>./dashboard.html</li>
            <li>../dashboard.html</li>
        </ul>
        </body></html>"#, cwd))
}

pub fn create_routes(app_state: AppState) -> Router {
    // Create protected routes with auth middleware
    let protected_routes = Router::new()
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/user", get(user_info))
        .route("/api/start-scraper", post(start_scraper))
        .route("/api/stop-scraper", post(stop_scraper))
        .route("/api/pause-scraper", post(pause_scraper))
        .route("/api/resume-scraper", post(resume_scraper))
        .route("/api/restart-scraper", post(restart_scraper))
        // Database management endpoints (protected)
        .route("/api/database/start", post(database_start))
        .route("/api/database/stop", post(database_stop))
        .route("/api/database/restart", post(database_restart))
        // User management endpoints
        .route("/api/auth/change-password", post(change_password))
        .route("/api/auth/users", get(list_users))
        .route("/api/auth/users", post(create_user))
        .route("/api/auth/users/:id", axum::routing::delete(delete_user))
        // API Keys management endpoints
        .route("/api/keys", post(create_api_key))
        .route("/api/keys", get(list_api_keys))
        .route("/api/keys/:id", get(get_api_key))
        .route("/api/keys/:id/deactivate", post(deactivate_api_key))
        .route("/api/keys/:id/delete", axum::routing::delete(delete_api_key))
        .route("/api/keys/:id/regenerate", post(regenerate_api_key))
        .route("/api/keys/statistics", get(get_api_key_statistics))
        .route("/api/keys/types", get(get_api_key_types))
        .route("/api/keys/validate", post(validate_api_key_handler))
        // Scanner endpoints (protected)
        .route("/api/scanner/scan", post(scan_repository))
        .route("/api/scanner/batch-scan", post(batch_scan_repositories))
        .route("/api/scanner/export", get(export_scan_results))
        .route("/api/scanner/schedule", post(schedule_scan))
        .route("/api/scanner/schedules", get(get_scheduled_scans))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state.user_manager.clone(), 
            auth_middleware
        ));

    // Create auth status route with optional auth middleware  
    let auth_status_route = Router::new()
        .route("/api/auth/status", get(auth_status))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state.user_manager.clone(), 
            optional_auth_middleware
        ));

    // Create public routes
    Router::new()
        // Public routes
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        .route("/api/auth/login", post(login))
        .route("/api/auth/verify", get(auth_verify))
        // Status endpoints (public)
        .route("/api/status", get(system_status_simple))
        .route("/api/system/status", get(system_status_simple))
        .route("/api/system/metrics", get(system_metrics))
        .route("/api/scraper/status", get(scraper_status_simple))
        .route("/api/scraper/control", post(scraper_control))
        .route("/api/database/status", get(database_status))
        .route("/api/database/stats", get(database_stats))
        // Scanner public endpoints
        .route("/api/scanner/results", get(get_scan_results))
        .route("/api/scanner/statistics", get(get_scan_statistics))
        .route("/api/scanner/detectors", get(get_detectors))
        // Dashboard routes (public access)
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/dashboard.html", get(serve_dashboard))
        .route("/favicon.ico", get(|| async { axum::response::Response::builder()
            .status(204)
            .body(axum::body::Body::empty())
            .unwrap() }))
        .with_state(app_state)
        // Merge the protected routes
        .merge(auth_status_route)
        .merge(protected_routes)
}
