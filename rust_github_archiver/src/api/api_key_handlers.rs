// API key management handlers
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use serde::Deserialize; // Removed unused Serialize
use tracing::{error, info, warn};

use crate::api::api_keys::{ApiKeyManager, CreateApiKeyRequest}; // Removed unused ApiKeyType
use crate::api::state::AppState;
use crate::auth::User;

#[derive(Debug, Deserialize)]
pub struct ListApiKeysQuery {
    pub key_type: Option<String>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

/// Create a new API key
pub async fn create_api_key(
    State(_state): State<AppState>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!(
        "Creating API key '{}' for user '{}'",
        request.name, user.username
    );

    // Validate request
    if request.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    match ApiKeyManager::create_api_key(request, user.username) {
        Ok(api_key) => {
            info!("Successfully created API key '{}'", api_key.name);
            Ok(Json(serde_json::json!({
                "success": true,
                "data": api_key,
                "message": "API key created successfully"
            })))
        }
        Err(e) => {
            error!("Failed to create API key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List all API keys
pub async fn list_api_keys(
    State(_state): State<AppState>,
    Extension(_user): Extension<User>,
    Query(query): Query<ListApiKeysQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match ApiKeyManager::list_api_keys() {
        Ok(mut keys) => {
            // Filter by type if specified
            if let Some(key_type) = &query.key_type {
                keys.retain(|k| k.key_type.to_string().to_lowercase() == key_type.to_lowercase());
            }

            // Filter by active status if specified
            if let Some(active_only) = query.active_only {
                if active_only {
                    keys.retain(|k| k.is_active);
                }
            }

            Ok(Json(serde_json::json!({
                "success": true,
                "data": keys,
                "count": keys.len()
            })))
        }
        Err(e) => {
            error!("Failed to list API keys: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get a specific API key by ID
pub async fn get_api_key(
    State(_state): State<AppState>,
    Extension(_user): Extension<User>,
    Path(key_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match ApiKeyManager::get_api_key(&key_id) {
        Ok(Some(api_key)) => Ok(Json(serde_json::json!({
            "success": true,
            "data": api_key
        }))),
        Ok(None) => {
            warn!("API key not found: {}", key_id);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to get API key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Deactivate an API key
pub async fn deactivate_api_key(
    State(_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(key_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("User '{}' deactivating API key '{}'", user.username, key_id);

    match ApiKeyManager::deactivate_api_key(&key_id) {
        Ok(true) => {
            info!("Successfully deactivated API key '{}'", key_id);
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "API key deactivated successfully"
            })))
        }
        Ok(false) => {
            warn!("API key not found for deactivation: {}", key_id);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to deactivate API key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Delete an API key
pub async fn delete_api_key(
    State(_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(key_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("User '{}' deleting API key '{}'", user.username, key_id);

    match ApiKeyManager::delete_api_key(&key_id) {
        Ok(true) => {
            info!("Successfully deleted API key '{}'", key_id);
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "API key deleted successfully"
            })))
        }
        Ok(false) => {
            warn!("API key not found for deletion: {}", key_id);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to delete API key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Regenerate an API key
pub async fn regenerate_api_key(
    State(_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(key_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("User '{}' regenerating API key '{}'", user.username, key_id);

    match ApiKeyManager::regenerate_api_key(&key_id) {
        Ok(Some(new_key)) => {
            info!("Successfully regenerated API key '{}'", key_id);
            Ok(Json(serde_json::json!({
                "success": true,
                "data": {
                    "new_key": new_key
                },
                "message": "API key regenerated successfully"
            })))
        }
        Ok(None) => {
            warn!("API key not found for regeneration: {}", key_id);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to regenerate API key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get API key statistics
pub async fn get_api_key_statistics(
    State(_state): State<AppState>,
    Extension(_user): Extension<User>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match ApiKeyManager::get_statistics() {
        Ok(stats) => Ok(Json(serde_json::json!({
            "success": true,
            "data": stats
        }))),
        Err(e) => {
            error!("Failed to get API key statistics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get all available API key types
pub async fn get_api_key_types(
    State(_state): State<AppState>,
    Extension(_user): Extension<User>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let types = vec![
        serde_json::json!({
            "value": "GitHub",
            "label": "GitHub API",
            "description": "Access to GitHub repositories and metadata"
        }),
        serde_json::json!({
            "value": "AWS",
            "label": "AWS Services",
            "description": "Access to AWS cloud services"
        }),
        serde_json::json!({
            "value": "Database",
            "label": "Database Access",
            "description": "Direct database read/write access"
        }),
        serde_json::json!({
            "value": "Webhook",
            "label": "Webhook Integration",
            "description": "Webhook endpoint authentication"
        }),
        serde_json::json!({
            "value": "Scanner",
            "label": "Security Scanner",
            "description": "Secret and vulnerability scanning"
        }),
        serde_json::json!({
            "value": "Admin",
            "label": "Administrative",
            "description": "Full system administration access"
        }),
        serde_json::json!({
            "value": "ReadOnly",
            "label": "Read Only",
            "description": "Read-only access to data and reports"
        }),
    ];

    Ok(Json(serde_json::json!({
        "success": true,
        "data": types
    })))
}

/// Validate an API key (for authentication middleware)
pub async fn validate_api_key_handler(
    State(_state): State<AppState>,
    Extension(_user): Extension<User>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key_value = request
        .get("key")
        .and_then(|k| k.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    match ApiKeyManager::validate_api_key(key_value) {
        Ok(Some(api_key)) => {
            // Update last used timestamp
            if let Err(e) = ApiKeyManager::update_last_used(key_value) {
                warn!("Failed to update last used timestamp: {}", e);
            }

            Ok(Json(serde_json::json!({
                "success": true,
                "data": {
                    "valid": true,
                    "key_id": api_key.id,
                    "key_type": api_key.key_type,
                    "permissions": api_key.permissions
                }
            })))
        }
        Ok(None) => Ok(Json(serde_json::json!({
            "success": true,
            "data": {
                "valid": false
            }
        }))),
        Err(e) => {
            error!("Failed to validate API key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
