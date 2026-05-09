/*!
 * Health Check Endpoints
 *
 * Provides comprehensive health and readiness checks for the application.
 * Used by load balancers and orchestrators to determine service health.
 */

use axum::{http::StatusCode, response::Json, routing::get, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub uptime_seconds: u64,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub response_time_ms: u64,
}

#[derive(Clone)]
pub struct HealthChecker {
    pool: PgPool,
    start_time: std::time::Instant,
}

impl HealthChecker {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            start_time: std::time::Instant::now(),
        }
    }

    /// Liveness check - is the application running?
    /// Used by Kubernetes liveness probes
    pub async fn liveness(&self) -> Result<HealthResponse, StatusCode> {
        Ok(HealthResponse {
            status: HealthStatus::Healthy,
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            checks: vec![HealthCheck {
                name: "application".to_string(),
                status: HealthStatus::Healthy,
                message: Some("Application is running".to_string()),
                response_time_ms: 0,
            }],
        })
    }

    /// Readiness check - is the application ready to serve traffic?
    /// Used by Kubernetes readiness probes
    pub async fn readiness(&self) -> Result<HealthResponse, StatusCode> {
        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        // Check database connection
        let db_start = std::time::Instant::now();
        let db_check = match self.check_database().await {
            Ok(()) => HealthCheck {
                name: "database".to_string(),
                status: HealthStatus::Healthy,
                message: Some("Database connection healthy".to_string()),
                response_time_ms: db_start.elapsed().as_millis() as u64,
            },
            Err(e) => {
                overall_status = HealthStatus::Unhealthy;
                HealthCheck {
                    name: "database".to_string(),
                    status: HealthStatus::Unhealthy,
                    message: Some(format!("Database error: {}", e)),
                    response_time_ms: db_start.elapsed().as_millis() as u64,
                }
            }
        };
        checks.push(db_check);

        // Check if database response time is acceptable (< 100ms)
        if let Some(db_check) = checks.last() {
            if db_check.response_time_ms > 100 && db_check.status == HealthStatus::Healthy {
                overall_status = HealthStatus::Degraded;
            }
        }

        let response = HealthResponse {
            status: overall_status,
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            checks,
        };

        match overall_status {
            HealthStatus::Healthy => Ok(response),
            HealthStatus::Degraded => Ok(response),
            HealthStatus::Unhealthy => Err(StatusCode::SERVICE_UNAVAILABLE),
        }
    }

    /// Detailed health check with all components
    pub async fn detailed(&self) -> HealthResponse {
        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        // Database check
        let db_start = std::time::Instant::now();
        let db_check = match self.check_database().await {
            Ok(()) => HealthCheck {
                name: "database".to_string(),
                status: HealthStatus::Healthy,
                message: Some("Database connection healthy".to_string()),
                response_time_ms: db_start.elapsed().as_millis() as u64,
            },
            Err(e) => {
                overall_status = HealthStatus::Unhealthy;
                HealthCheck {
                    name: "database".to_string(),
                    status: HealthStatus::Unhealthy,
                    message: Some(format!("Database error: {}", e)),
                    response_time_ms: db_start.elapsed().as_millis() as u64,
                }
            }
        };
        checks.push(db_check);

        // Memory check
        let memory_start = std::time::Instant::now();
        let memory_check = self.check_memory();
        checks.push(HealthCheck {
            name: "memory".to_string(),
            status: memory_check.0,
            message: Some(memory_check.1),
            response_time_ms: memory_start.elapsed().as_millis() as u64,
        });

        if memory_check.0 != HealthStatus::Healthy && overall_status == HealthStatus::Healthy {
            overall_status = memory_check.0;
        }

        // Disk space check
        let disk_start = std::time::Instant::now();
        let disk_check = self.check_disk_space();
        checks.push(HealthCheck {
            name: "disk_space".to_string(),
            status: disk_check.0,
            message: Some(disk_check.1),
            response_time_ms: disk_start.elapsed().as_millis() as u64,
        });

        if disk_check.0 != HealthStatus::Healthy && overall_status == HealthStatus::Healthy {
            overall_status = disk_check.0;
        }

        HealthResponse {
            status: overall_status,
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            checks,
        }
    }

    async fn check_database(&self) -> Result<(), String> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn check_memory(&self) -> (HealthStatus, String) {
        use sysinfo::System;

        let mut sys = System::new_all();
        sys.refresh_memory();

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

        if usage_percent > 90.0 {
            (
                HealthStatus::Unhealthy,
                format!("Memory usage critical: {:.1}%", usage_percent),
            )
        } else if usage_percent > 80.0 {
            (
                HealthStatus::Degraded,
                format!("Memory usage high: {:.1}%", usage_percent),
            )
        } else {
            (
                HealthStatus::Healthy,
                format!("Memory usage: {:.1}%", usage_percent),
            )
        }
    }

    fn check_disk_space(&self) -> (HealthStatus, String) {
        use sysinfo::Disks;

        let disks = Disks::new_with_refreshed_list();

        // Check the root disk
        if let Some(disk) = disks.first() {
            let total_space = disk.total_space();
            let available_space = disk.available_space();
            let used_percent =
                ((total_space - available_space) as f64 / total_space as f64) * 100.0;

            if used_percent > 95.0 {
                (
                    HealthStatus::Unhealthy,
                    format!("Disk usage critical: {:.1}%", used_percent),
                )
            } else if used_percent > 85.0 {
                (
                    HealthStatus::Degraded,
                    format!("Disk usage high: {:.1}%", used_percent),
                )
            } else {
                (
                    HealthStatus::Healthy,
                    format!("Disk usage: {:.1}%", used_percent),
                )
            }
        } else {
            (
                HealthStatus::Degraded,
                "Could not check disk space".to_string(),
            )
        }
    }
}

/// Handler for liveness probe
pub async fn liveness_handler(
    axum::extract::State(checker): axum::extract::State<Arc<HealthChecker>>,
) -> Result<Json<HealthResponse>, StatusCode> {
    checker.liveness().await.map(Json)
}

/// Handler for readiness probe
pub async fn readiness_handler(
    axum::extract::State(checker): axum::extract::State<Arc<HealthChecker>>,
) -> Result<Json<HealthResponse>, StatusCode> {
    match checker.readiness().await {
        Ok(response) => Ok(Json(response)),
        Err(status) => Err(status),
    }
}

/// Handler for detailed health check
pub async fn health_handler(
    axum::extract::State(checker): axum::extract::State<Arc<HealthChecker>>,
) -> Json<HealthResponse> {
    Json(checker.detailed().await)
}

/// Create health check routes
pub fn health_routes() -> Router<Arc<HealthChecker>> {
    Router::new()
        .route("/health", get(health_handler))
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    fn lazy_checker() -> HealthChecker {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://postgres:postgres@127.0.0.1:1/github_archiver_test")
            .expect("lazy pool");
        HealthChecker::new(pool)
    }

    #[tokio::test]
    async fn liveness_is_healthy_without_database_access() {
        let checker = lazy_checker();

        let response = checker.liveness().await.expect("liveness");

        assert_eq!(response.status, HealthStatus::Healthy);
        assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(response.checks.len(), 1);
        assert_eq!(response.checks[0].name, "application");
    }

    #[test]
    fn health_status_serializes_lowercase_for_api_contract() {
        assert_eq!(
            serde_json::to_string(&HealthStatus::Healthy).expect("serialize"),
            "\"healthy\""
        );
        assert_eq!(
            serde_json::to_string(&HealthStatus::Degraded).expect("serialize"),
            "\"degraded\""
        );
        assert_eq!(
            serde_json::to_string(&HealthStatus::Unhealthy).expect("serialize"),
            "\"unhealthy\""
        );
    }

    #[test]
    fn health_routes_can_be_constructed_for_probe_registration() {
        let _routes: Router<Arc<HealthChecker>> = health_routes();
    }
}
