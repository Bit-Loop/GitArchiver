// Simple web server starter for GitHub Archiver
// This creates a minimal web server without BigQuery dependencies

use github_archiver::api::ApiServer;
use github_archiver::core::Config;
use github_archiver::logging;
use github_archiver::metrics;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Initialize structured logging (JSON in production, pretty in development)
    logging::init_logging();

    // Initialize Prometheus metrics
    metrics::init_metrics();

    // Load configuration from environment
    let mut config = Config::default();

    // Override ports from environment
    if let Ok(web_port) = env::var("WEB_PORT") {
        config.web.port = web_port.parse().unwrap_or(config.web.port);
    }
    if let Ok(db_port) = env::var("DB_PORT") {
        config.database.port = db_port.parse().unwrap_or(config.database.port);
    }

    println!("🚀 Starting GitHub Archiver Web Server");
    println!("   Web Port: {}", config.web.port);
    println!("   Database Port: {}", config.database.port);

    // Start the API server
    let server = ApiServer::new(config).await?;
    server.start().await?;

    Ok(())
}
