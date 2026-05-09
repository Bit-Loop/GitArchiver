// API server implementation
use crate::core::Database;
use crate::shutdown::ShutdownCoordinator;
use anyhow::{Context, Result};
use axum::Router;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::api::routes::create_routes;
use crate::api::state::AppState;
use crate::core::Config;

#[derive(Clone)]
pub struct ApiServer {
    app_state: AppState,
}

impl ApiServer {
    pub async fn new(config: Config) -> Result<Self> {
        // Initialize database first (async) then pass into state
        let database = Arc::new(Database::new(&config).await?);
        let app_state = AppState::new(config, database);
        app_state.start_background_workers();
        Ok(Self { app_state })
    }
    pub async fn start(&self) -> Result<()> {
        let app = self.create_app();
        let bind_host = self.app_state.config.web.host.as_str();
        let bind_addr = format!("{}:{}", bind_host, self.app_state.config.web.port);
        let listener = TcpListener::bind(&bind_addr).await.with_context(|| {
            format!(
                "failed to bind API server to {}; check WEB_HOST/WEB_PORT and stop any process already using the port",
                bind_addr
            )
        })?;

        let addr = listener.local_addr()?;
        let display_host = match bind_host {
            "0.0.0.0" | "::" => "localhost",
            host => host,
        };
        info!("🚀 Server listening on {}", addr);
        info!(
            "📊 Health checks: http://{}:{}/health",
            display_host,
            addr.port()
        );
        info!(
            "📈 Metrics: http://{}:{}/metrics",
            display_host,
            addr.port()
        );

        // Create shutdown coordinator for tracking active tasks
        let coordinator = Arc::new(ShutdownCoordinator::new());
        let coordinator_clone = coordinator.clone();
        let app_state = self.app_state.clone();

        // Create graceful shutdown signal handler using our shutdown coordinator
        let shutdown_signal = async move {
            use crate::shutdown::shutdown_signal;
            shutdown_signal().await;

            info!("Shutdown signal received, waiting for active tasks...");

            if let Err(error) = app_state.scraper_runtime.stop().await {
                warn!("Failed to stop scraper runtime during shutdown: {}", error);
            }

            // Wait for active tasks to complete (max 30 seconds)
            if let Err(e) = coordinator_clone
                .wait_for_tasks(std::time::Duration::from_secs(30))
                .await
            {
                warn!("Active tasks did not finish before shutdown: {}", e);
            }
        };

        info!("✅ Server ready. Graceful shutdown enabled (Ctrl+C or SIGTERM)");
        info!("🔒 Security: Rate limiting, CORS, and security headers active");

        // Start server with graceful shutdown
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await?;

        info!("👋 Server stopped gracefully");
        Ok(())
    }

    pub fn create_app(&self) -> Router {
        create_routes(self.app_state.clone())
    }
}
