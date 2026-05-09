// Simple API server starter for load testing
// This bypasses all the hunting/BigQuery dependencies and just starts the API

use anyhow::Result;
use github_archiver::api::server::ApiServer;
use github_archiver::core::Config;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file.
    dotenv::dotenv().ok();

    // Check for debug mode
    let web_debug = std::env::var("WEB_DEBUG").unwrap_or_default() == "1"
        || std::env::args().any(|arg| arg == "--web-debug");

    if web_debug {
        println!("🐛 WEB DEBUG MODE ENABLED");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    println!("✅ Environment variables loaded from .env");

    // Verify GitHub token is loaded
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            println!("✅ GitHub token loaded: <redacted>");
            if web_debug {
                println!("   Token value remains redacted in debug output");
            }
        }
    } else {
        println!("⚠️  No GITHUB_TOKEN found in environment");
    }

    // Show port configuration
    let port = std::env::var("WEB_PORT").unwrap_or_else(|_| "3000".to_string());
    println!("🌐 API Server will start on port: {}", port);

    if web_debug {
        println!("📋 Environment variables:");
        for (key, value) in std::env::vars() {
            if key.starts_with("WEB_") || key.starts_with("GITHUB_") || key.starts_with("RUST_") {
                if key.contains("TOKEN") || key.contains("SECRET") {
                    println!("   {}=<redacted>", key);
                } else {
                    println!("   {}={}", key, value);
                }
            }
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,github_archiver=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::new(None)?;

    if web_debug {
        println!("🔧 Configuration loaded:");
        println!(
            "   GitHub token present: {}",
            !config.github.token.is_empty()
        );
        println!("   Web host: {}", config.web.host);
        println!("   Web port: {}", config.web.port);
        println!("   CORS origins: {:?}", config.web.cors_origins);
    }

    // Create and start API server
    let api_server = ApiServer::new(config).await?;

    if web_debug {
        println!("✨ Starting API server with debug logging...");
        println!("   Dashboard URL: http://localhost:{}/dashboard.html", port);
        println!("   Health check: http://localhost:{}/health", port);
        println!("   API config: http://localhost:{}/api/config", port);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    api_server.start().await?;

    Ok(())
}
