// Axum API boundary: thin handlers, shared request services, and runtime control.

pub mod ai_handlers;
pub mod api_key_handlers;
pub mod api_keys;
pub mod audit_handlers;
pub mod auth_middleware;
pub mod extended_handlers;
pub mod handlers;
pub mod health_handlers;
pub mod maintenance_handlers;
pub mod middleware;
pub mod monitoring_handlers;
pub mod realtime_handlers;
pub mod research_handlers;
pub mod routes;
pub mod scanner_handlers;
pub mod scanner_service;
pub mod scraper_control;
pub mod server;
pub mod state;
pub mod status_service;

// Re-export main components
pub use handlers::*;
pub use health_handlers::*;
pub use routes::create_routes;
pub use server::*;
pub use state::*;
