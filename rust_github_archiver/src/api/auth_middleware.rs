/// API Key Authentication Middleware
/// Provides basic security for sensitive endpoints via X-API-Key header validation
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::{debug, warn};

use crate::api::api_keys::ApiKeyManager;
use crate::api::state::AppState;

/// API key validation middleware
///
/// Checks for X-API-Key header and validates against environment variable.
/// Returns 401 Unauthorized if key is missing or invalid.
///
/// # Usage
/// ```rust,no_run
/// use axum::{middleware, routing::post, Router};
/// use github_archiver::api::auth_middleware::require_api_key;
/// use github_archiver::api::state::AppState;
///
/// async fn add_token_handler() -> &'static str { "ok" }
/// # let app_state: AppState = panic!("provide AppState in application setup");
/// let protected_routes: Router<AppState> = Router::new()
///     .route("/api/tokens/add", post(add_token_handler))
///     .route_layer(middleware::from_fn_with_state(app_state.clone(), require_api_key));
/// ```
pub async fn require_api_key(
    State(_state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ApiKeyError> {
    let api_key = extract_api_key(&headers)?;
    let valid = ApiKeyManager::validate_api_key(api_key)
        .map_err(|_| ApiKeyError::Invalid)?
        .is_some();
    if !valid {
        warn!("Invalid API key attempt from request");
        return Err(ApiKeyError::Invalid);
    }
    if let Err(error) = ApiKeyManager::update_last_used(api_key) {
        warn!("Failed to update API key last-used timestamp: {}", error);
    }
    debug!("API key validated successfully");
    Ok(next.run(request).await)
}

fn extract_api_key(headers: &HeaderMap) -> Result<&str, ApiKeyError> {
    headers
        .get("X-API-Key")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiKeyError::Missing)
}

/// API Key validation errors
#[derive(Debug, PartialEq, Eq)]
pub enum ApiKeyError {
    Missing,
    Invalid,
}

impl IntoResponse for ApiKeyError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiKeyError::Missing => (StatusCode::UNAUTHORIZED, "Missing X-API-Key header"),
            ApiKeyError::Invalid => (StatusCode::UNAUTHORIZED, "Invalid API key"),
        };

        (
            status,
            Json(json!({
                "error": message,
                "hint": "Add an X-API-Key header containing a server-generated active API key."
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn headers_with_key(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-API-Key",
            HeaderValue::from_str(value).expect("header should build"),
        );
        headers
    }

    #[test]
    fn extract_api_key_rejects_missing_header() {
        assert_eq!(
            extract_api_key(&HeaderMap::new()),
            Err(ApiKeyError::Missing)
        );
    }

    #[test]
    fn extract_api_key_accepts_header() {
        assert_eq!(
            extract_api_key(&headers_with_key("expected-key")),
            Ok("expected-key")
        );
    }
}
