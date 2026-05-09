// Health check handlers for production Kubernetes deployment
use crate::api::state::AppState;
use crate::health::{HealthChecker, HealthResponse};
use axum::{extract::State, http::StatusCode, Json};
use tracing::{error, info};

/// Liveness probe - checks if application is running
/// Returns 200 if the application process is alive
pub async fn liveness_handler(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    info!("Liveness handler called");
    let checker = HealthChecker::new(state.database.pool().clone());
    match checker.liveness().await {
        Ok(response) => {
            info!("Liveness check successful");
            Ok(Json(response))
        }
        Err(e) => {
            error!("Liveness check failed: {:?}", e);
            Err(e)
        }
    }
}

/// Readiness probe - checks if application is ready to serve requests
/// Returns 200 if ready, 503 if not ready (e.g., database unavailable)
pub async fn readiness_handler(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let checker = HealthChecker::new(state.database.pool().clone());

    match checker.readiness().await {
        Ok(response) => {
            if response.status == crate::health::HealthStatus::Unhealthy {
                Err(StatusCode::SERVICE_UNAVAILABLE)
            } else {
                Ok(Json(response))
            }
        }
        Err(e) => Err(e),
    }
}

/// Detailed health check - provides comprehensive system health information
/// Includes database, memory, disk status with degradation indicators
pub async fn detailed_health_handler(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let checker = HealthChecker::new(state.database.pool().clone());
    match checker.readiness().await {
        Ok(response) => Ok(Json(response)),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
