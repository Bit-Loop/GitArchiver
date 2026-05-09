use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::api::state::AppState;
use crate::core::database::ScanStateRepairRequest;

pub async fn repair_scan_state(
    State(app_state): State<AppState>,
    Json(request): Json<ScanStateRepairRequest>,
) -> impl IntoResponse {
    match app_state.database.repair_scan_state(request).await {
        Ok(report) => (StatusCode::OK, Json(json!(report))).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": error.to_string(),
            })),
        )
            .into_response(),
    }
}
