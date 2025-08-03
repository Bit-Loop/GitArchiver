// API server implementation
use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

use crate::core::Config;
use crate::api::routes::create_routes;
use crate::api::state::AppState;

#[derive(Clone)]
pub struct ApiServer {
    app_state: AppState,
}

impl ApiServer {
    pub fn new(config: Config) -> Self {
        Self { 
            app_state: AppState::new(config)
        }
    }

    pub async fn start(&self) -> Result<()> {
        let app = self.create_app();
        
        let addr = SocketAddr::from(([0, 0, 0, 0], self.app_state.config.web.port));
        info!("Server listening on {}", addr);
        
        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }

    pub fn create_app(&self) -> Router {
        create_routes(self.app_state.clone())
    }
}
