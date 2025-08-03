// Authentication middleware implementation
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::auth::{jwt, users::UserManager};

/// Authentication middleware that checks for valid JWT tokens
pub async fn auth_middleware(
    State(user_manager): State<Arc<UserManager>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)> {
    // Extract Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Missing Authorization header",
                    "message": "Authorization header is required"
                })),
            )
        })?;

    // Check for Bearer token format
    if !auth_header.starts_with("Bearer ") {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Invalid Authorization format",
                "message": "Authorization header must start with 'Bearer '"
            })),
        ));
    }

    // Extract the token
    let token = &auth_header[7..]; // Remove "Bearer " prefix

    // Verify the JWT token
    let claims = jwt::verify_token(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Invalid token",
                "message": "JWT token is invalid or expired"
            })),
        )
    })?;

    // Get user information
    let user = user_manager
        .get_user(&claims.sub)
        .await
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "User not found",
                    "message": "User associated with token not found"
                })),
            )
        })?;

    // Add user info to request extensions for use in handlers
    request.extensions_mut().insert(user);

    // Continue to the next middleware/handler
    Ok(next.run(request).await)
}

/// Optional authentication middleware that doesn't fail if no token is provided
pub async fn optional_auth_middleware(
    State(user_manager): State<Arc<UserManager>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    // Try to extract and verify token, but don't fail if it's missing
    if let Some(auth_header) = headers.get("Authorization").and_then(|h| h.to_str().ok()) {
        if auth_header.starts_with("Bearer ") {
            let token = &auth_header[7..];
            if let Ok(claims) = jwt::verify_token(token) {
                if let Some(user) = user_manager.get_user(&claims.sub).await {
                    request.extensions_mut().insert(user);
                }
            }
        }
    }

    next.run(request).await
}
