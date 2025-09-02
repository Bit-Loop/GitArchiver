// Web API module placeholder
// This will contain the Axum web server, routes, and handlers

pub mod routes;
pub mod handlers;
pub mod middleware;
pub mod server;
pub mod state;
pub mod scanner_handlers;
pub mod api_keys;
pub mod api_key_handlers;

// Re-export main components
pub use server::*;
pub use state::*;
