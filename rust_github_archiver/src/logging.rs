use std::env;
/// Structured logging configuration for production environments
///
/// Phase 3.3: Provides JSON-formatted logging with correlation IDs, structured fields,
/// and environment-based configuration.
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize structured logging for the application
///
/// # Logging Formats
/// - **Production**: JSON format for machine parsing and log aggregation
/// - **Development**: Pretty-printed format for human readability
///
/// # Configuration via Environment Variables
/// - `LOG_LEVEL`: Set log level (trace, debug, info, warn, error). Default: info
/// - `LOG_FORMAT`: Set format (json, pretty). Default: json in production, pretty in dev
/// - `RUST_LOG`: Override specific module logging (e.g., RUST_LOG=github_archiver=debug)
pub fn init_logging() {
    // Determine log level from environment, default to info
    let log_level = env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .to_lowercase();

    // Determine if we're in production mode
    let environment = env::var("ENVIRONMENT")
        .unwrap_or_else(|_| "development".to_string())
        .to_lowercase();

    let is_production = environment == "production" || environment == "prod";

    // Determine log format from environment
    let log_format = env::var("LOG_FORMAT")
        .unwrap_or_else(|_| {
            if is_production {
                "json".to_string()
            } else {
                "pretty".to_string()
            }
        })
        .to_lowercase();

    // Build the env filter with fallback to RUST_LOG or our default
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("github_archiver={}", log_level)))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Configure logging based on format
    match log_format.as_str() {
        "json" => {
            // JSON format for production - machine parseable
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_current_span(true)
                        .with_span_list(true)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .init();

            tracing::info!(
                environment = %environment,
                log_level = %log_level,
                log_format = "json",
                "Structured JSON logging initialized"
            );
        }
        _ => {
            // Pretty format for development - human readable
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .pretty()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .init();

            tracing::info!(
                environment = %environment,
                log_level = %log_level,
                log_format = "pretty",
                "Pretty-printed logging initialized"
            );
        }
    }
}

/// Logging middleware for HTTP requests
///
/// Automatically logs all incoming HTTP requests with:
/// - Request ID (correlation ID)
/// - Method, path, query parameters
/// - Response status and duration
/// - User information (if authenticated)
///
/// # Example Log Output (JSON format)
/// ```json
/// {
///   "timestamp": "2025-10-13T12:34:56.789Z",
///   "level": "INFO",
///   "target": "github_archiver::logging",
///   "fields": {
///     "message": "HTTP request completed",
///     "request_id": "req-a1b2c3d4",
///     "method": "GET",
///     "path": "/api/health",
///     "status": 200,
///     "duration_ms": 15,
///     "user": "admin"
///   }
/// }
/// ```
pub mod middleware {
    use crate::metrics;
    use axum::{extract::Request, middleware::Next, response::Response};
    use std::time::Instant;
    use tracing::{debug, info, trace, warn};
    use uuid::Uuid;

    fn is_high_volume_path(path: &str) -> bool {
        path == "/api/scanner/metrics"
            || path == "/api/monitoring/metrics"
            || path == "/api/monitoring/overview"
            || path == "/api/monitoring/trends"
            || path == "/api/system/metrics"
            || path == "/api/system/status"
            || path == "/health"
    }

    /// HTTP request logging and metrics middleware
    ///
    /// Adds structured logging for all HTTP requests with correlation IDs
    /// and records Prometheus metrics for monitoring
    pub async fn log_request(request: Request, next: Next) -> Response {
        let start = Instant::now();

        // Generate a unique request ID for correlation
        let request_id = Uuid::new_v4().to_string();

        // Extract request information
        let method = request.method().to_string();
        let path = request.uri().path().to_string();
        let query = request.uri().query().unwrap_or("").to_string();

        let quiet_path = is_high_volume_path(&path);

        // Log incoming request unless we intentionally suppress high-volume paths
        if !quiet_path {
            info!(
                request_id = %request_id,
                method = %method,
                path = %path,
                query = %query,
                "HTTP request received"
            );
        } else {
            trace!(
                request_id = %request_id,
                method = %method,
                path = %path,
                query = %query,
                "HTTP request received (quiet)"
            );
        }

        // Process the request
        let response = next.run(request).await;

        // Calculate duration
        let duration = start.elapsed();
        let duration_ms = duration.as_millis();
        let duration_secs = duration.as_secs_f64();

        // Extract response status
        let status = response.status().as_u16();

        // Record Prometheus metrics
        metrics::record_http_request(&method, &path, status, duration_secs);

        // Log completed request with appropriate level
        if status >= 500 {
            warn!(
                request_id = %request_id,
                method = %method,
                path = %path,
                status = status,
                duration_ms = duration_ms,
                "HTTP request failed with server error"
            );
        } else if status >= 400 {
            warn!(
                request_id = %request_id,
                method = %method,
                path = %path,
                status = status,
                duration_ms = duration_ms,
                "HTTP request failed with client error"
            );
        } else if quiet_path {
            debug!(
                request_id = %request_id,
                method = %method,
                path = %path,
                status = status,
                duration_ms = duration_ms,
                "HTTP request completed"
            );
        } else {
            info!(
                request_id = %request_id,
                method = %method,
                path = %path,
                status = status,
                duration_ms = duration_ms,
                "HTTP request completed successfully"
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_logging_initialization() {
        // This test ensures the module links and compiles as part of the suite.
        super::init_logging();
    }
}
