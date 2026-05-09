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

use crate::auth::{jwt, users::User, UserManager, UserRole};

async fn authenticate_user(
    user_manager: &UserManager,
    headers: &HeaderMap,
) -> Result<User, (StatusCode, Json<Value>)> {
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

    if !auth_header.starts_with("Bearer ") {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Invalid Authorization format",
                "message": "Authorization header must start with 'Bearer '"
            })),
        ));
    }

    let token = &auth_header[7..];
    let claims = jwt::verify_token(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Invalid token",
                "message": "JWT token is invalid or expired"
            })),
        )
    })?;

    user_manager.get_user(&claims.sub).await.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "User not found",
                "message": "User associated with token not found"
            })),
        )
    })
}

fn require_minimum_role(
    user: &User,
    minimum_role: UserRole,
) -> Result<(), (StatusCode, Json<Value>)> {
    let actual_role = user.parsed_role().map_err(|_| {
        (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Invalid role assignment",
                "message": "User account has an unsupported role assignment"
            })),
        )
    })?;

    if actual_role.allows(minimum_role) {
        return Ok(());
    }

    Err((
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": "Insufficient permissions",
            "message": format!("{} role required for this operation", minimum_role),
            "required_role": minimum_role.canonical_label(),
            "actual_role": actual_role.canonical_label()
        })),
    ))
}

async fn authenticate_with_minimum_role(
    user_manager: Arc<UserManager>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
    minimum_role: UserRole,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let user = authenticate_user(&user_manager, &headers).await?;
    require_minimum_role(&user, minimum_role)?;
    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}

/// Authentication middleware that checks for valid JWT tokens
pub async fn auth_middleware(
    State(user_manager): State<Arc<UserManager>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)> {
    authenticate_with_minimum_role(user_manager, headers, request, next, UserRole::ReadOnly).await
}

/// Authentication middleware for operator actions.
pub async fn operator_auth_middleware(
    State(user_manager): State<Arc<UserManager>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)> {
    authenticate_with_minimum_role(user_manager, headers, request, next, UserRole::Operator).await
}

/// Authentication middleware for admin-only actions.
pub async fn admin_auth_middleware(
    State(user_manager): State<Arc<UserManager>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)> {
    authenticate_with_minimum_role(user_manager, headers, request, next, UserRole::Admin).await
}

/// Optional authentication middleware that doesn't fail if no token is provided
pub async fn optional_auth_middleware(
    State(user_manager): State<Arc<UserManager>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    // Try to extract and verify token, but don't fail if it's missing
    if let Ok(user) = authenticate_user(&user_manager, &headers).await {
        if require_minimum_role(&user, UserRole::ReadOnly).is_ok() {
            request.extensions_mut().insert(user);
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::require_minimum_role;
    use crate::auth::{User, UserRole};
    use axum::http::StatusCode;

    fn user_with_role(role: &str) -> User {
        User {
            id: "1".to_string(),
            username: "tester".to_string(),
            password_hash: "hash".to_string(),
            role: role.to_string(),
            created_at: chrono::Utc::now(),
            last_login: None,
            is_active: true,
        }
    }

    #[test]
    fn require_minimum_role_accepts_legacy_operator_alias() {
        let user = user_with_role("user");
        assert!(require_minimum_role(&user, UserRole::Operator).is_ok());
    }

    #[test]
    fn require_minimum_role_rejects_insufficient_permissions() {
        let user = user_with_role("viewer");
        let error = require_minimum_role(&user, UserRole::Operator).expect_err("must reject");
        assert_eq!(error.0, StatusCode::FORBIDDEN);
    }

    #[test]
    fn require_minimum_role_rejects_invalid_role_assignments() {
        let user = user_with_role("superadmin");
        let error = require_minimum_role(&user, UserRole::ReadOnly).expect_err("must reject");
        assert_eq!(error.0, StatusCode::FORBIDDEN);
    }
}
