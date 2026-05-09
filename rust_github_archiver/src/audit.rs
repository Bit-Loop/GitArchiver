/*!
 * Audit Logging System
 *
 * Provides comprehensive audit trail for security-sensitive operations.
 * Records who did what, when, from where, and with what result.
 *
 * Used for:
 * - Security compliance (SOC 2, ISO 27001, etc.)
 * - Incident investigation
 * - Forensic analysis
 * - Regulatory requirements
 */

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, PgPool, Row};
use std::collections::HashMap;

/// Types of auditable actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    // User management
    UserCreated,
    UserDeleted,
    UserUpdated,
    PasswordChanged,
    LoginSuccess,
    LoginFailure,
    LogoutSuccess,

    // API key management
    ApiKeyCreated,
    ApiKeyRegenerated,
    ApiKeyDeactivated,
    ApiKeyDeleted,

    // Scraper operations
    ScraperStarted,
    ScraperStopped,
    ScraperPaused,
    ScraperResumed,
    ScraperRestarted,
    ScanLaunched,
    ScanScheduled,
    ScanExported,
    SystemCleanup,
    TokenPoolUpdated,
    TokenHealthReset,

    // Database operations
    DatabaseStarted,
    DatabaseStopped,
    DatabaseRestarted,
    DatabaseBackupCreated,
    DatabaseRestored,

    // System configuration
    ConfigUpdated,
    RateLimitUpdated,
    WebhookAdded,
    WebhookRemoved,
    WebhookUpdated,

    // Security events
    UnauthorizedAccess,
    RateLimitExceeded,
    InvalidToken,
    SuspiciousActivity,
}

/// Resource types for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    User,
    ApiKey,
    Scraper,
    Scan,
    Database,
    Webhook,
    TokenPool,
    Config,
    System,
}

/// Audit event status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuditStatus {
    Success,
    Failure,
    Warning,
}

/// Complete audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<i64>,
    pub username: String,
    pub action: AuditAction,
    pub resource_type: ResourceType,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub status: AuditStatus,
    pub details: serde_json::Value,
    pub error_message: Option<String>,
}

/// Audit log entry for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub user_id: Option<i64>,
    pub username: String,
    pub action: AuditAction,
    pub resource_type: ResourceType,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub status: AuditStatus,
    pub details: HashMap<String, serde_json::Value>,
    pub error_message: Option<String>,
}

/// Audit logger
pub struct AuditLogger {
    pool: PgPool,
}

impl AuditLogger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Log an audit event
    pub async fn log(&self, entry: AuditLogEntry) -> Result<i64> {
        let details_json = serde_json::to_value(&entry.details)?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO audit_logs (
                user_id, username, action, resource_type, resource_id,
                ip_address, user_agent, status, details, error_message
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
        )
        .bind(entry.user_id)
        .bind(&entry.username)
        .bind(serde_json::to_string(&entry.action)?)
        .bind(serde_json::to_string(&entry.resource_type)?)
        .bind(&entry.resource_id)
        .bind(&entry.ip_address)
        .bind(&entry.user_agent)
        .bind(serde_json::to_string(&entry.status)?)
        .bind(details_json)
        .bind(&entry.error_message)
        .fetch_one(&self.pool)
        .await?;

        let id = row.0;

        // Also log to structured logging for real-time monitoring
        tracing::info!(
            user_id = ?entry.user_id,
            username = %entry.username,
            action = ?entry.action,
            resource_type = ?entry.resource_type,
            resource_id = ?entry.resource_id,
            ip_address = ?entry.ip_address,
            status = ?entry.status,
            "Audit event logged"
        );

        Ok(id)
    }

    /// Query audit logs with filters
    pub async fn query(
        &self,
        filters: AuditLogFilters,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let mut query = String::from(
            "SELECT id, timestamp, user_id, username, action, resource_type, 
             resource_id, ip_address, user_agent, status, details, error_message
             FROM audit_logs WHERE 1=1",
        );

        if let Some(ref user_id) = filters.user_id {
            query.push_str(&format!(" AND user_id = {}", user_id));
        }

        if let Some(ref username) = filters.username {
            query.push_str(&format!(" AND username = '{}'", username));
        }

        if let Some(ref action) = filters.action {
            let action_str = serde_json::to_string(action)?;
            query.push_str(&format!(" AND action = '{}'", action_str));
        }

        if let Some(ref resource_type) = filters.resource_type {
            let resource_str = serde_json::to_string(resource_type)?;
            query.push_str(&format!(" AND resource_type = '{}'", resource_str));
        }

        if let Some(ref status) = filters.status {
            let status_str = serde_json::to_string(status)?;
            query.push_str(&format!(" AND status = '{}'", status_str));
        }

        if let Some(ref start_date) = filters.start_date {
            query.push_str(&format!(" AND timestamp >= '{}'", start_date.to_rfc3339()));
        }

        if let Some(ref end_date) = filters.end_date {
            query.push_str(&format!(" AND timestamp <= '{}'", end_date.to_rfc3339()));
        }

        query.push_str(&format!(
            " ORDER BY timestamp DESC LIMIT {} OFFSET {}",
            limit, offset
        ));

        let rows = sqlx::query_as::<_, AuditLogRow>(&query)
            .fetch_all(&self.pool)
            .await?;

        let logs = rows
            .into_iter()
            .map(|row| row.into_audit_log())
            .collect::<Result<Vec<_>>>()?;

        Ok(logs)
    }

    /// Get audit log by ID
    pub async fn get_by_id(&self, id: i64) -> Result<Option<AuditLog>> {
        let row = sqlx::query_as::<_, AuditLogRow>(
            "SELECT id, timestamp, user_id, username, action, resource_type,
             resource_id, ip_address, user_agent, status, details, error_message
             FROM audit_logs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.into_audit_log()?)),
            None => Ok(None),
        }
    }

    /// Get audit statistics
    pub async fn get_statistics(&self, days: i32) -> Result<AuditStatistics> {
        let start_date = Utc::now() - chrono::Duration::days(days as i64);

        let total_events: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM audit_logs WHERE timestamp >= $1")
                .bind(start_date)
                .fetch_one(&self.pool)
                .await?;
        let total_events = total_events.0;

        let failed_events: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM audit_logs 
               WHERE timestamp >= $1 AND status = 'failure'"#,
        )
        .bind(start_date)
        .fetch_one(&self.pool)
        .await?;
        let failed_events = failed_events.0;

        let unique_users: (i64,) =
            sqlx::query_as("SELECT COUNT(DISTINCT user_id) FROM audit_logs WHERE timestamp >= $1")
                .bind(start_date)
                .fetch_one(&self.pool)
                .await?;
        let unique_users = unique_users.0;

        // Top actions
        let top_actions_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT action, COUNT(*) as count 
               FROM audit_logs 
               WHERE timestamp >= $1 
               GROUP BY action 
               ORDER BY count DESC 
               LIMIT 10"#,
        )
        .bind(start_date)
        .fetch_all(&self.pool)
        .await?;

        let top_actions = top_actions_rows;

        // Top users
        let top_users_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT username, COUNT(*) as count 
               FROM audit_logs 
               WHERE timestamp >= $1 
               GROUP BY username 
               ORDER BY count DESC 
               LIMIT 10"#,
        )
        .bind(start_date)
        .fetch_all(&self.pool)
        .await?;

        let top_users = top_users_rows;

        Ok(AuditStatistics {
            period_days: days,
            total_events,
            failed_events,
            success_rate: if total_events > 0 {
                ((total_events - failed_events) as f64 / total_events as f64) * 100.0
            } else {
                0.0
            },
            unique_users,
            top_actions,
            top_users,
        })
    }

    /// Clean up old audit logs based on retention policy
    pub async fn cleanup(&self, retention_days: i32) -> Result<i64> {
        let cutoff_date = Utc::now() - chrono::Duration::days(retention_days as i64);

        let result = sqlx::query("DELETE FROM audit_logs WHERE timestamp < $1")
            .bind(cutoff_date)
            .execute(&self.pool)
            .await?;

        let deleted = result.rows_affected() as i64;

        tracing::info!(
            deleted_count = deleted,
            retention_days = retention_days,
            cutoff_date = %cutoff_date,
            "Audit log cleanup completed"
        );

        Ok(deleted as i64)
    }

    /// Export audit logs to JSON
    pub async fn export_json(
        &self,
        filters: AuditLogFilters,
        limit: Option<i64>,
    ) -> Result<String> {
        let limit = limit.unwrap_or(10000); // Default max 10k records
        let logs = self.query(filters, limit, 0).await?;
        Ok(serde_json::to_string_pretty(&logs)?)
    }

    /// Export audit logs to CSV
    pub async fn export_csv(&self, filters: AuditLogFilters, limit: Option<i64>) -> Result<String> {
        let limit = limit.unwrap_or(10000);
        let logs = self.query(filters, limit, 0).await?;

        let mut csv = String::from("id,timestamp,user_id,username,action,resource_type,resource_id,ip_address,status,error_message\n");

        for log in logs {
            csv.push_str(&format!(
                "{},{},{},{},{:?},{:?},{},{},{:?},{}\n",
                log.id,
                log.timestamp.to_rfc3339(),
                log.user_id.map(|id| id.to_string()).unwrap_or_default(),
                log.username,
                log.action,
                log.resource_type,
                log.resource_id.unwrap_or_default(),
                log.ip_address.unwrap_or_default(),
                log.status,
                log.error_message.unwrap_or_default()
            ));
        }

        Ok(csv)
    }
}

/// Filters for querying audit logs
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AuditLogFilters {
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub action: Option<AuditAction>,
    pub resource_type: Option<ResourceType>,
    pub status: Option<AuditStatus>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub period_days: i32,
    pub total_events: i64,
    pub failed_events: i64,
    pub success_rate: f64,
    pub unique_users: i64,
    pub top_actions: Vec<(String, i64)>,
    pub top_users: Vec<(String, i64)>,
}

struct AuditLogRow {
    id: i64,
    timestamp: DateTime<Utc>,
    user_id: Option<i64>,
    username: String,
    action: String,
    resource_type: String,
    resource_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    status: String,
    details: serde_json::Value,
    error_message: Option<String>,
}

impl<'r> sqlx::FromRow<'r, PgRow> for AuditLogRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            timestamp: row.try_get("timestamp")?,
            user_id: row.try_get("user_id")?,
            username: row.try_get("username")?,
            action: row.try_get("action")?,
            resource_type: row.try_get("resource_type")?,
            resource_id: row.try_get("resource_id")?,
            ip_address: row.try_get("ip_address")?,
            user_agent: row.try_get("user_agent")?,
            status: row.try_get("status")?,
            details: row.try_get("details")?,
            error_message: row.try_get("error_message")?,
        })
    }
}

impl AuditLogRow {
    fn into_audit_log(self) -> Result<AuditLog> {
        Ok(AuditLog {
            id: self.id,
            timestamp: self.timestamp,
            user_id: self.user_id,
            username: self.username,
            action: serde_json::from_str(&self.action)?,
            resource_type: serde_json::from_str(&self.resource_type)?,
            resource_id: self.resource_id,
            ip_address: self.ip_address,
            user_agent: self.user_agent,
            status: serde_json::from_str(&self.status)?,
            details: self.details,
            error_message: self.error_message,
        })
    }
}

/// Helper to extract IP address from request
pub fn extract_ip_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    // Try X-Forwarded-For first (for reverse proxies)
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(ip) = forwarded_str.split(',').next() {
                return Some(ip.trim().to_string());
            }
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = headers.get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            return Some(ip_str.to_string());
        }
    }

    None
}

/// Helper to extract user agent from request
pub fn extract_user_agent(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("User-Agent")
        .and_then(|ua| ua.to_str().ok())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_action_serialization() {
        let action = AuditAction::UserCreated;
        let serialized = serde_json::to_string(&action).unwrap();
        assert_eq!(serialized, "\"user_created\"");
    }

    #[test]
    fn test_audit_status_equality() {
        assert_eq!(AuditStatus::Success, AuditStatus::Success);
        assert_ne!(AuditStatus::Success, AuditStatus::Failure);
    }

    #[test]
    fn test_resource_type_serialization() {
        let resource = ResourceType::User;
        let serialized = serde_json::to_string(&resource).unwrap();
        assert_eq!(serialized, "\"user\"");
    }
}
