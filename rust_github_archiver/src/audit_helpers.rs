// Helper functions for easy audit logging from handlers
use axum::http::HeaderMap;
use std::collections::HashMap;

use crate::audit::{
    extract_ip_from_headers, extract_user_agent, AuditAction, AuditLogEntry, AuditLogger,
    AuditStatus, ResourceType,
};

/// Log a successful user action
#[allow(clippy::too_many_arguments)]
pub async fn log_success(
    logger: &AuditLogger,
    user_id: Option<i64>,
    username: &str,
    action: AuditAction,
    resource_type: ResourceType,
    resource_id: Option<String>,
    headers: &HeaderMap,
    details: HashMap<String, serde_json::Value>,
) -> anyhow::Result<i64> {
    let entry = AuditLogEntry {
        user_id,
        username: username.to_string(),
        action,
        resource_type,
        resource_id,
        ip_address: extract_ip_from_headers(headers),
        user_agent: extract_user_agent(headers),
        status: AuditStatus::Success,
        details,
        error_message: None,
    };

    logger.log(entry).await
}

/// Log a failed user action
#[allow(clippy::too_many_arguments)]
pub async fn log_failure(
    logger: &AuditLogger,
    user_id: Option<i64>,
    username: &str,
    action: AuditAction,
    resource_type: ResourceType,
    resource_id: Option<String>,
    headers: &HeaderMap,
    error: &str,
    details: HashMap<String, serde_json::Value>,
) -> anyhow::Result<i64> {
    let entry = AuditLogEntry {
        user_id,
        username: username.to_string(),
        action,
        resource_type,
        resource_id,
        ip_address: extract_ip_from_headers(headers),
        user_agent: extract_user_agent(headers),
        status: AuditStatus::Failure,
        details,
        error_message: Some(error.to_string()),
    };

    logger.log(entry).await
}

/// Log a security event (unauthorized access, suspicious activity)
pub async fn log_security_event(
    logger: &AuditLogger,
    username: &str,
    action: AuditAction,
    headers: &HeaderMap,
    description: &str,
) -> anyhow::Result<i64> {
    let mut details = HashMap::new();
    details.insert("description".to_string(), serde_json::json!(description));

    let entry = AuditLogEntry {
        user_id: None,
        username: username.to_string(),
        action,
        resource_type: ResourceType::System,
        resource_id: None,
        ip_address: extract_ip_from_headers(headers),
        user_agent: extract_user_agent(headers),
        status: AuditStatus::Warning,
        details,
        error_message: Some(description.to_string()),
    };

    logger.log(entry).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let mut details = HashMap::new();
        details.insert("key".to_string(), serde_json::json!("value"));

        let entry = AuditLogEntry {
            user_id: Some(1),
            username: "testuser".to_string(),
            action: AuditAction::UserCreated,
            resource_type: ResourceType::User,
            resource_id: Some("123".to_string()),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("test-agent".to_string()),
            status: AuditStatus::Success,
            details,
            error_message: None,
        };

        assert_eq!(entry.username, "testuser");
        assert_eq!(entry.user_id, Some(1));
        assert_eq!(entry.status, AuditStatus::Success);
    }
}
