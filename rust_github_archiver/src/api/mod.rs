// Web API module placeholder
// This will contain the Axum web server, routes, and handlers

pub mod routes;
pub mod handlers;
pub mod middleware;
pub mod server;
pub mod state;

// Re-export main components
pub use server::*;
pub use state::*;
