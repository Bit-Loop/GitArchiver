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
use std::env;
use tracing::{debug, warn};

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
    let expected_key = load_expected_api_key();
    validate_api_key(&headers, &expected_key)?;
    debug!("API key validated successfully");
    Ok(next.run(request).await)
}

fn validate_api_key(headers: &HeaderMap, expected_key: &str) -> Result<(), ApiKeyError> {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiKeyError::Missing)?;

    if api_key != expected_key {
        warn!("Invalid API key attempt from request");
        return Err(ApiKeyError::Invalid);
    }

    Ok(())
}

fn load_expected_api_key() -> String {
    resolve_expected_api_key(
        env::var("API_KEY").ok(),
        env::var("GITHUB_ARCHIVER_API_KEY").ok(),
    )
}

fn resolve_expected_api_key(
    api_key: Option<String>,
    github_archiver_api_key: Option<String>,
) -> String {
    api_key.or(github_archiver_api_key).unwrap_or_else(|| {
        warn!("No API_KEY environment variable set! Using default key. CHANGE THIS IN PRODUCTION!");
        "changeme-insecure-default-key".to_string()
    })
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

        (status, Json(json!({
            "error": message,
            "hint": "Add X-API-Key header with valid API key. Set API_KEY environment variable on server."
        }))).into_response()
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
    fn validate_api_key_rejects_missing_header() {
        assert_eq!(
            validate_api_key(&HeaderMap::new(), "expected-key"),
            Err(ApiKeyError::Missing)
        );
    }

    #[test]
    fn validate_api_key_rejects_invalid_header() {
        assert_eq!(
            validate_api_key(&headers_with_key("wrong-key"), "expected-key"),
            Err(ApiKeyError::Invalid)
        );
    }

    #[test]
    fn validate_api_key_accepts_valid_header() {
        assert_eq!(
            validate_api_key(&headers_with_key("expected-key"), "expected-key"),
            Ok(())
        );
    }

    #[test]
    fn resolve_expected_api_key_prefers_primary_env_var() {
        let resolved = resolve_expected_api_key(
            Some("primary-key".to_string()),
            Some("fallback-key".to_string()),
        );
        assert_eq!(resolved, "primary-key");
    }

    #[test]
    fn resolve_expected_api_key_uses_fallback_env_var() {
        let resolved = resolve_expected_api_key(None, Some("fallback-key".to_string()));
        assert_eq!(resolved, "fallback-key");
    }

    #[test]
    fn resolve_expected_api_key_uses_default_when_env_is_missing() {
        let resolved = resolve_expected_api_key(None, None);
        assert_eq!(resolved, "changeme-insecure-default-key");
    }
}
