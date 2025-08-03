// API routes implementation
use axum::{Router, routing::{get, post}, middleware, response::Html};
use std::sync::Arc;

use crate::auth::{UserManager, auth_middleware, optional_auth_middleware};
use crate::api::handlers::{
    health_check, login, logout, user_info, auth_status,
    start_scraper, stop_scraper, pause_scraper, resume_scraper, 
    restart_scraper, scraper_status, system_status
};
use crate::api::state::AppState;

// Handler to serve dashboard.html
async fn serve_dashboard() -> Html<String> {
    match tokio::fs::read_to_string("dashboard.html").await {
        Ok(content) => Html(content),
        Err(_) => Html("<html><body><h1>Dashboard not found</h1><p>dashboard.html file is missing</p></body></html>".to_string()),
    }
}

pub fn create_routes(app_state: AppState) -> Router {
    // Create protected routes that require authentication
    let protected_routes = Router::new()
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/user", get(user_info))
        // Scraper control endpoints (protected)
        .route("/api/start-scraper", post(start_scraper))
        .route("/api/stop-scraper", post(stop_scraper))
        .route("/api/pause-scraper", post(pause_scraper))
        .route("/api/resume-scraper", post(resume_scraper))
        .route("/api/restart-scraper", post(restart_scraper))
        .layer(middleware::from_fn_with_state(app_state.user_manager.clone(), auth_middleware))
        .with_state(app_state.clone());

    // Create auth status route with optional authentication
    let auth_status_route = Router::new()
        .route("/api/auth/status", get(auth_status))
        .layer(middleware::from_fn_with_state(app_state.user_manager.clone(), optional_auth_middleware))
        .with_state(app_state.clone());

    // Combine public and protected routes
    Router::new()
        // Public routes
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        .route("/api/auth/login", post(login))
        // Status endpoints (public)
        .route("/api/status", get(system_status))
        .route("/api/scraper/status", get(scraper_status))
        // Dashboard routes (public access)
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/dashboard.html", get(serve_dashboard))
        // Merge auth status route with optional auth
        .merge(auth_status_route)
        // Merge protected routes
        .merge(protected_routes)
        // Add app state that includes user manager and scraper manager
        .with_state(app_state)
}
