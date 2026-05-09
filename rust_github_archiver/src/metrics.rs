/// Prometheus metrics exporter
///
/// Phase 3.4: Provides comprehensive metrics collection and export for monitoring
/// systems like Prometheus, Grafana, etc.
use axum::{http::StatusCode, response::IntoResponse};
use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec_with_registry, register_int_counter_with_registry,
    register_int_gauge_with_registry, Encoder, HistogramOpts, HistogramVec, IntCounter, IntGauge,
    Opts, Registry, TextEncoder,
};

/// Global Prometheus registry
static PROMETHEUS_REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

/// HTTP request counter by endpoint and status code
static HTTP_REQUESTS_TOTAL: Lazy<HistogramVec> = Lazy::new(|| {
    let opts = HistogramOpts::new(
        "http_requests_duration_seconds",
        "HTTP request latency in seconds",
    )
    .buckets(vec![
        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ]);

    register_histogram_vec_with_registry!(
        opts,
        &["method", "endpoint", "status"],
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register HTTP requests histogram")
});

/// Total number of HTTP requests served
static HTTP_REQUESTS_COUNT: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new("http_requests_count_total", "Total number of HTTP requests"),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register HTTP requests counter")
});

/// Active database connections
static DB_CONNECTIONS_ACTIVE: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge_with_registry!(
        Opts::new(
            "db_connections_active",
            "Number of active database connections"
        ),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register database connections gauge")
});

/// Database query count
static DB_QUERIES_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new(
            "db_queries_total",
            "Total number of database queries executed"
        ),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register database queries counter")
});

/// GitHub API rate limiter stats
static GITHUB_API_REQUESTS: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new(
            "github_api_requests_total",
            "Total GitHub API requests made"
        ),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register GitHub API requests counter")
});

static GITHUB_API_RATE_LIMITED: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new(
            "github_api_rate_limited_total",
            "Number of times rate limited by GitHub API"
        ),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register GitHub rate limit counter")
});

/// Token pool stats
static TOKEN_POOL_SIZE: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge_with_registry!(
        Opts::new("token_pool_size", "Number of tokens in the pool"),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register token pool size gauge")
});

static TOKEN_POOL_HEALTHY: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge_with_registry!(
        Opts::new("token_pool_healthy", "Number of healthy tokens in the pool"),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register token pool healthy gauge")
});

/// Secrets scanner stats
static SECRETS_SCANNED: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new(
            "secrets_scanned_total",
            "Total number of commits/files scanned for secrets"
        ),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register secrets scanned counter")
});

static SECRETS_FOUND: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new("secrets_found_total", "Total number of secrets detected"),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register secrets found counter")
});

/// Webhook delivery stats
static WEBHOOK_DELIVERIES_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new(
            "webhook_deliveries_total",
            "Total webhook delivery attempts"
        ),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register webhook deliveries counter")
});

static WEBHOOK_FAILURES_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter_with_registry!(
        Opts::new("webhook_failures_total", "Total webhook delivery failures"),
        PROMETHEUS_REGISTRY
    )
    .expect("Failed to register webhook failures counter")
});

/// Public API for recording metrics from application code
/// Record an HTTP request with method, endpoint, status, and duration
pub fn record_http_request(method: &str, endpoint: &str, status: u16, duration_secs: f64) {
    HTTP_REQUESTS_COUNT.inc();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, endpoint, &status.to_string()])
        .observe(duration_secs);
}

/// Update database connection count
pub fn set_db_connections(count: i64) {
    DB_CONNECTIONS_ACTIVE.set(count);
}

/// Record a database query execution
pub fn record_db_query() {
    DB_QUERIES_TOTAL.inc();
}

/// Record GitHub API request
pub fn record_github_api_request() {
    GITHUB_API_REQUESTS.inc();
}

/// Record GitHub API rate limit hit
pub fn record_github_rate_limit() {
    GITHUB_API_RATE_LIMITED.inc();
}

/// Update token pool stats
pub fn set_token_pool_stats(total: i64, healthy: i64) {
    TOKEN_POOL_SIZE.set(total);
    TOKEN_POOL_HEALTHY.set(healthy);
}

/// Record secrets scanning activity
pub fn record_secrets_scanned(count: u64) {
    SECRETS_SCANNED.inc_by(count);
}

pub fn record_secrets_found(count: u64) {
    SECRETS_FOUND.inc_by(count);
}

/// Record webhook delivery
pub fn record_webhook_delivery(success: bool) {
    WEBHOOK_DELIVERIES_TOTAL.inc();
    if !success {
        WEBHOOK_FAILURES_TOTAL.inc();
    }
}

/// Axum handler to export metrics in Prometheus text format
///
/// This endpoint should be called by Prometheus scraper
/// Configure in prometheus.yml:
/// ```yaml
/// scrape_configs:
///   - job_name: 'github-archiver'
///     static_configs:
///       - targets: ['localhost:8081']
///     metrics_path: '/metrics'
/// ```
pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = PROMETHEUS_REGISTRY.gather();

    let mut buffer = Vec::new();
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4")],
            buffer,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to encode Prometheus metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to encode metrics: {}", e),
            )
                .into_response()
        }
    }
}

/// Initialize metrics with default values
pub fn init_metrics() {
    // Force lazy static initialization
    let _ = *HTTP_REQUESTS_COUNT;
    let _ = *HTTP_REQUESTS_TOTAL;
    let _ = *DB_CONNECTIONS_ACTIVE;
    let _ = *DB_QUERIES_TOTAL;
    let _ = *GITHUB_API_REQUESTS;
    let _ = *GITHUB_API_RATE_LIMITED;
    let _ = *TOKEN_POOL_SIZE;
    let _ = *TOKEN_POOL_HEALTHY;
    let _ = *SECRETS_SCANNED;
    let _ = *SECRETS_FOUND;
    let _ = *WEBHOOK_DELIVERIES_TOTAL;
    let _ = *WEBHOOK_FAILURES_TOTAL;

    tracing::info!("Prometheus metrics initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        init_metrics();
    }

    #[test]
    fn test_record_http_request() {
        record_http_request("GET", "/api/health", 200, 0.015);
        record_http_request("POST", "/api/scan", 201, 0.125);
    }

    #[test]
    fn test_database_metrics() {
        set_db_connections(10);
        record_db_query();
        record_db_query();
    }
}
