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

use crate::api::api_keys::{ApiKey, ApiKeyManager, ApiKeyType};
use crate::auth::{jwt, users::User, UserManager, UserRole};

fn unauthorized(error: &str, message: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": error,
            "message": message
        })),
    )
}

fn user_from_api_key(api_key: &ApiKey) -> User {
    let role = match &api_key.key_type {
        ApiKeyType::Admin => UserRole::Admin,
        ApiKeyType::ReadOnly => UserRole::ReadOnly,
        ApiKeyType::GitHub
        | ApiKeyType::AWS
        | ApiKeyType::Database
        | ApiKeyType::Webhook
        | ApiKeyType::Scanner => UserRole::Operator,
    };

    User {
        id: api_key.id.clone(),
        username: format!("api-key:{}", api_key.name),
        password_hash: String::new(),
        role: role.canonical_label().to_string(),
        created_at: api_key.created_at,
        last_login: api_key.last_used,
        is_active: api_key.is_active,
        token_version: 0,
    }
}

fn authenticate_api_key(headers: &HeaderMap) -> Result<Option<User>, (StatusCode, Json<Value>)> {
    let Some(api_key_value) = headers.get("X-API-Key").and_then(|h| h.to_str().ok()) else {
        return Ok(None);
    };

    let Some(api_key) = ApiKeyManager::validate_api_key(api_key_value)
        .map_err(|_| unauthorized("Invalid API key", "API key could not be validated"))?
    else {
        return Err(unauthorized(
            "Invalid API key",
            "API key is invalid or expired",
        ));
    };

    if let Err(error) = ApiKeyManager::update_last_used(api_key_value) {
        tracing::warn!(
            key_id = %api_key.id,
            error = %error,
            "Failed to update API key last-used timestamp"
        );
    }

    Ok(Some(user_from_api_key(&api_key)))
}

async fn authenticate_user(
    user_manager: &UserManager,
    headers: &HeaderMap,
) -> Result<User, (StatusCode, Json<Value>)> {
    if let Some(api_key_user) = authenticate_api_key(headers)? {
        return Ok(api_key_user);
    }

    let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());
    let Some(auth_header) = auth_header else {
        return Err(unauthorized(
            "Missing credentials",
            "Authorization bearer token or X-API-Key header is required",
        ));
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(unauthorized(
            "Invalid Authorization format",
            "Authorization header must start with 'Bearer '",
        ));
    }

    let token = &auth_header[7..];
    let claims = jwt::verify_token(token)
        .map_err(|_| unauthorized("Invalid token", "JWT token is invalid or expired"))?;

    let user = user_manager
        .get_user(&claims.sub)
        .await
        .ok_or_else(|| unauthorized("User not found", "User associated with token not found"))?;

    if !user.is_active {
        return Err(unauthorized("User inactive", "User account is disabled"));
    }

    if user.token_version != claims.token_version {
        return Err(unauthorized(
            "Token revoked",
            "JWT token was issued before the account credential version changed",
        ));
    }

    if user.canonical_role() != claims.role {
        return Err(unauthorized(
            "Token role mismatch",
            "JWT token role no longer matches the user account",
        ));
    }

    Ok(user)
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
            token_version: 0,
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
