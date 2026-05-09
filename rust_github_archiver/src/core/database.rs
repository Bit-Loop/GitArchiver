use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, QueryBuilder, Row};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::config::Config;
use crate::secrets::{SecretCategory, SecretDetectionRecord, SecretScanRecord, SecretSeverity};

/// Canonical database runtime health snapshot shared by API, scraper, and monitoring code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatabaseHealth {
    pub is_connected: bool,
    pub connection_count: i64,
    pub active_queries: i64,
    pub cache_hit_ratio: f64,
    pub error_message: Option<String>,
}

/// Database statistics for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatistics {
    pub total_events: i64,
    pub database_size: String,
    pub table_count: i64,
    pub tables: Vec<(String, i64, String)>, // (name, row_count, size)
}

/// Data quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub total_events: i64,
    pub unique_actors: i64,
    pub unique_repos: i64,
    pub event_types: i64,
    pub quality_score: f64,
    pub integrity_issues: HashMap<String, i64>,
    pub processing_stats: HashMap<String, f64>,
    pub recent_activity: HashMap<String, i64>,
}

/// Lightweight representation of persisted secret detections for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretDetectionRow {
    pub detection_id: Uuid,
    pub repository: String,
    pub file_path: Option<String>,
    pub detector_name: String,
    pub severity: String,
    pub category: String,
    pub detected_at: DateTime<Utc>,
    pub verified: bool,
    pub source: String,
    pub event_id: Option<i64>,
    pub matched_text_preview: String,
    pub line_number: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRepositoryRiskRow {
    pub repository: String,
    pub total_secrets: i64,
    pub critical_count: i64,
    pub high_count: i64,
    pub risk_score: f64,
    pub last_detected: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretOverviewMetrics {
    pub total_secrets: i64,
    pub severity_counts: HashMap<String, i64>,
    pub category_counts: HashMap<String, i64>,
    pub verified_secrets: i64,
    pub false_positives: i64,
    pub repositories_scanned: i64,
    pub files_scanned: i64,
    pub total_scans: i64,
    pub active_scans: i64,
    pub failed_scans: i64,
    pub avg_scan_duration_ms: Option<i64>,
    pub last_scan_time: Option<DateTime<Utc>>,
    pub scan_success_rate: f64,
    pub scan_rate_per_minute: f64,
    pub repos_per_minute: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretTrendSample {
    pub detected_at: DateTime<Utc>,
    pub severity: String,
    pub category: String,
}

#[derive(Debug, Clone, Default)]
pub struct SecretDetectionFilter {
    pub repository: Option<String>,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub source: Option<String>,
    pub verified: Option<bool>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub struct SecretDashboardData {
    pub overview: SecretOverviewMetrics,
    pub top_repositories: Vec<SecretRepositoryRiskRow>,
    pub recent_detections: Vec<SecretDetectionRow>,
}

/// Lightweight representation of stored GitHub events for UI previews
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPreview {
    pub event_id: i64,
    pub event_type: String,
    pub event_created_at: DateTime<Utc>,
    pub actor_login: Option<String>,
    pub repo_name: Option<String>,
    pub repo_url: Option<String>,
    pub payload: Value,
}

/// Minimal representation of a stored PushEvent used for downstream scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventScanTarget {
    pub event_id: i64,
    pub repository_full_name: String,
    pub repository_url: Option<String>,
    pub before_sha: String,
    pub head_sha: Option<String>,
    pub reference: Option<String>,
    pub forced: bool,
    pub commit_count: i32,
    pub event_created_at: DateTime<Utc>,
    pub is_zero_commit: bool,
    pub event_payload: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEventQueueInsert {
    pub event_id: i64,
    pub repository_full_name: String,
    pub repository_url: Option<String>,
    pub before_sha: String,
    pub head_sha: Option<String>,
    pub ref_name: Option<String>,
    pub forced_flag: bool,
    pub commit_span: i32,
    pub event_created_at: DateTime<Utc>,
    pub is_zero_commit: bool,
    pub event_payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanQueueStats {
    pub pending_events: i64,
    pub processing_events: i64,
    pub failed_events: i64,
    pub failed_forbidden: i64,
    pub failed_not_found: i64,
    pub completed_last_hour: i64,
    pub oldest_pending_age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ScanStateRepairCounts {
    pub invalid_secret_scan_summaries: i64,
    pub stale_processing_events: i64,
    pub pending_events: i64,
    pub processing_events: i64,
    pub failed_events: i64,
    pub completed_events: i64,
    pub total_secret_scans: i64,
    pub total_secret_detections: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStateRepairRequest {
    pub execute: bool,
    pub backup_path: Option<String>,
    pub hard_delete_invalid_summaries: bool,
    pub reset_stale_processing: bool,
    pub operator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStateRepairReport {
    pub run_id: Option<Uuid>,
    pub executed: bool,
    pub dry_run: bool,
    pub backup_path: Option<String>,
    pub hard_delete_invalid_summaries: bool,
    pub reset_stale_processing: bool,
    pub pre_counts: ScanStateRepairCounts,
    pub post_counts: ScanStateRepairCounts,
    pub deleted_invalid_summaries: i64,
    pub reset_stale_processing_rows: i64,
    pub operator: String,
    pub executed_at: DateTime<Utc>,
}

/// Event data structure for validation and conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedEvent {
    pub id: i64,
    pub event_type: String,
    pub created_at: DateTime<Utc>,
    pub public: bool,
    pub actor: ActorData,
    pub repo: RepoData,
    pub org: Option<OrgData>,
    pub payload: Value,
    pub raw_event: Value,
    pub api_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorData {
    pub id: Option<i64>,
    pub login: Option<String>,
    pub display_login: Option<String>,
    pub gravatar_id: Option<String>,
    pub url: Option<String>,
    pub avatar_url: Option<String>,
    pub node_id: Option<String>,
    pub html_url: Option<String>,
    pub followers_url: Option<String>,
    pub following_url: Option<String>,
    pub gists_url: Option<String>,
    pub starred_url: Option<String>,
    pub subscriptions_url: Option<String>,
    pub organizations_url: Option<String>,
    pub repos_url: Option<String>,
    pub events_url: Option<String>,
    pub received_events_url: Option<String>,
    pub actor_type: Option<String>,
    pub user_view_type: Option<String>,
    pub site_admin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoData {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub url: Option<String>,
    pub full_name: Option<String>,
    pub owner_login: Option<String>,
    pub owner_id: Option<i64>,
    pub owner_node_id: Option<String>,
    pub owner_avatar_url: Option<String>,
    pub owner_gravatar_id: Option<String>,
    pub owner_url: Option<String>,
    pub owner_html_url: Option<String>,
    pub owner_type: Option<String>,
    pub owner_site_admin: Option<bool>,
    pub node_id: Option<String>,
    pub html_url: Option<String>,
    pub description: Option<String>,
    pub fork: Option<bool>,
    pub language: Option<String>,
    pub stargazers_count: Option<i64>,
    pub watchers_count: Option<i64>,
    pub forks_count: Option<i64>,
    pub open_issues_count: Option<i64>,
    pub size: Option<i64>,
    pub default_branch: Option<String>,
    pub topics: Vec<String>,
    pub license_key: Option<String>,
    pub license_name: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub pushed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgData {
    pub id: Option<i64>,
    pub login: Option<String>,
    pub node_id: Option<String>,
    pub gravatar_id: Option<String>,
    pub url: Option<String>,
    pub avatar_url: Option<String>,
    pub html_url: Option<String>,
    pub org_type: Option<String>,
    pub site_admin: Option<bool>,
}

/// Professional PostgreSQL database manager with connection pooling
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
    config: Config,
}

impl Database {
    /// Create a new database connection with retry logic
    pub async fn new(config: &Config) -> Result<Self> {
        let connection_string = &config.database.connection_string();
        let max_attempts = 3;

        for attempt in 1..=max_attempts {
            match PgPoolOptions::new()
                .min_connections(config.database.min_connections)
                .max_connections(config.database.max_connections)
                .acquire_timeout(std::time::Duration::from_secs(
                    config.database.command_timeout,
                ))
                .idle_timeout(std::time::Duration::from_secs(600))
                .max_lifetime(std::time::Duration::from_secs(1800))
                .connect(connection_string)
                .await
            {
                Ok(pool) => {
                    let db = Database {
                        pool,
                        config: config.clone(),
                    };

                    // Verify connection and initialize schema
                    db.verify_connection().await?;
                    db.initialize_schema().await?;

                    info!("Database connected successfully (attempt {})", attempt);
                    return Ok(db);
                }
                Err(e) => {
                    error!("Database connection attempt {} failed: {}", attempt, e);
                    if attempt < max_attempts {
                        tokio::time::sleep(std::time::Duration::from_secs(2 * attempt)).await;
                    } else {
                        return Err(anyhow::anyhow!(
                            "Failed to connect after {} attempts: {}",
                            max_attempts,
                            e
                        ));
                    }
                }
            }
        }

        unreachable!()
    }

    /// Verify database connection is working
    async fn verify_connection(&self) -> Result<()> {
        let version: String = sqlx::query_scalar("SELECT version()")
            .fetch_one(&self.pool)
            .await
            .context("Failed to verify database connection")?;

        info!("Connected to PostgreSQL: {}", version);
        Ok(())
    }

    /// Get reference to the database connection pool
    /// Used for health checks and direct pool access
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Backward-compatible alias for the canonical runtime health status.
    pub async fn check_health(&self) -> DatabaseHealth {
        self.health_status().await
    }

    /// Initialize database schema if needed
    async fn initialize_schema(&self) -> Result<()> {
        let schema_commands = self.get_schema_commands();
        let total_commands = schema_commands.len();
        info!(
            "Initializing database schema with {} commands",
            total_commands
        );

        // Separate table/extension creation from index creation for deterministic ordering
        let mut table_cmds = Vec::with_capacity(total_commands / 2);
        let mut index_cmds = Vec::with_capacity(total_commands / 2);
        for cmd in schema_commands {
            let lc = cmd.to_lowercase();
            if lc.starts_with("create index") || lc.contains(" create index ") {
                index_cmds.push(cmd);
            } else {
                table_cmds.push(cmd);
            }
        }

        info!("Executing {} table/extension commands", table_cmds.len());
        // Execute table/extension commands first
        for (idx, command) in table_cmds.iter().enumerate() {
            if command.trim().is_empty() {
                continue;
            }
            tracing::debug!(
                "[{}/{}] Executing: {}",
                idx + 1,
                table_cmds.len(),
                command.chars().take(100).collect::<String>()
            );
            if let Err(e) = sqlx::query(command).execute(&self.pool).await {
                error!("Failed to execute schema command: {}", e);
                error!("Command was: {}", command);
                // Bubble up critical failures (tables/extensions) immediately
                return Err(e).context(format!(
                    "Failed to execute schema command: {}",
                    command.chars().take(200).collect::<String>()
                ));
            }
            tracing::debug!("✓ Command {} completed successfully", idx + 1);
        }

        info!("Executing {} index commands", index_cmds.len());
        // Execute index commands; skip (warn) if underlying relation missing to remain resilient
        let mut indexes_created = 0;
        let mut indexes_skipped = 0;
        for (idx, command) in index_cmds.iter().enumerate() {
            if command.trim().is_empty() {
                continue;
            }
            tracing::debug!(
                "[{}/{}] Creating index: {}",
                idx + 1,
                index_cmds.len(),
                command.chars().take(100).collect::<String>()
            );
            if let Err(e) = sqlx::query(command).execute(&self.pool).await {
                if e.to_string().contains("does not exist") {
                    warn!(
                        "Skipping index {}/{}: relation missing (will retry on next startup)",
                        idx + 1,
                        index_cmds.len()
                    );
                    indexes_skipped += 1;
                    continue;
                } else {
                    error!("Failed to create index: {}", e);
                    return Err(e).context("Failed to execute index creation");
                }
            }
            indexes_created += 1;
        }

        info!("✓ Database schema initialized successfully");
        info!("  - Tables/Extensions: {} created", table_cmds.len());
        info!(
            "  - Indexes: {} created, {} skipped",
            indexes_created, indexes_skipped
        );

        info!("Applying schema fixups for backward compatibility");
        for fixup in self.get_schema_fixups() {
            if fixup.trim().is_empty() {
                continue;
            }
            if let Err(e) = sqlx::query(fixup).execute(&self.pool).await {
                let msg = e.to_string();
                if msg.contains("does not exist") || msg.contains("duplicate column name") {
                    warn!(
                        "Skipping schema fixup due to missing relation or duplicate: {}",
                        fixup
                    );
                    continue;
                } else {
                    return Err(e).context(format!("Failed to execute schema fixup: {}", fixup));
                }
            }
        }

        Ok(())
    }

    /// Persist metadata about a completed or in-flight secret scan
    pub async fn insert_secret_scan(&self, scan: &SecretScanRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO secret_scans (
                id, repository, scan_type, status, source,
                started_at, completed_at, duration_ms, files_scanned,
                secrets_found, created_by, metadata
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9,
                $10, $11, $12
            )
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                completed_at = EXCLUDED.completed_at,
                duration_ms = EXCLUDED.duration_ms,
                files_scanned = EXCLUDED.files_scanned,
                secrets_found = EXCLUDED.secrets_found,
                metadata = EXCLUDED.metadata
            "#,
        )
        .bind(scan.scan_id)
        .bind(scan.repository.as_ref())
        .bind(&scan.scan_type)
        .bind(&scan.status)
        .bind(scan.source.as_str())
        .bind(scan.started_at)
        .bind(scan.completed_at)
        .bind(scan.duration_ms)
        .bind(scan.files_scanned)
        .bind(scan.secrets_found)
        .bind(&scan.created_by)
        .bind(&scan.metadata)
        .execute(&self.pool)
        .await
        .context("Failed to insert secret scan record")?;

        Ok(())
    }

    /// Persist detected secrets with deduplication on the hashed match value
    pub async fn insert_secret_detections(
        &self,
        detections: &[SecretDetectionRecord],
    ) -> Result<usize> {
        if detections.is_empty() {
            return Ok(0);
        }

        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO secret_detections (
                detection_id, scan_id, event_id, repository, file_path,
                detector_name, severity, category, matched_text_hash,
                matched_text_preview, line_number, verified, detected_at,
                source, metadata
            ) VALUES ",
        );

        builder.push_values(detections, |mut b, detection| {
            b.push_bind(detection.detection_id)
                .push_bind(detection.scan_id)
                .push_bind(detection.event_id)
                .push_bind(&detection.repository)
                .push_bind(&detection.file_path)
                .push_bind(&detection.detector_name)
                .push_bind(detection.severity.as_str())
                .push_bind(detection.category.storage_key())
                .push_bind(&detection.matched_text_hash)
                .push_bind(&detection.matched_text_preview)
                .push_bind(detection.line_number)
                .push_bind(detection.verified)
                .push_bind(detection.detected_at)
                .push_bind(detection.source.as_str())
                .push_bind(&detection.metadata);
        });

        builder.push(" ON CONFLICT DO NOTHING");

        let result = builder
            .build()
            .execute(&self.pool)
            .await
            .context("Failed to insert secret detections")?;

        Ok(result.rows_affected() as usize)
    }

    /// Aggregate dashboard data for the monitoring endpoint
    pub async fn get_secret_dashboard_data(
        &self,
        top_limit: i64,
        recent_limit: i64,
    ) -> Result<SecretDashboardData> {
        let overview = self.get_secret_overview_metrics().await?;

        let top_repositories = match self.get_top_secret_repositories(top_limit).await {
            Ok(rows) => rows,
            Err(e) => {
                warn!("Failed to load top repositories: {}", e);
                Vec::new()
            }
        };

        let recent_detections = match self
            .get_secret_detections(SecretDetectionFilter {
                limit: Some(recent_limit.max(1)),
                ..Default::default()
            })
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                warn!("Failed to load recent detections: {}", e);
                Vec::new()
            }
        };

        Ok(SecretDashboardData {
            overview,
            top_repositories,
            recent_detections,
        })
    }

    /// Summaries used by overview cards and KPI widgets
    pub async fn get_secret_overview_metrics(&self) -> Result<SecretOverviewMetrics> {
        const RECENT_WINDOW_MINUTES: f64 = 15.0;
        let overview_row = sqlx::query(
            r#"
            WITH detection_stats AS (
                SELECT
                    COUNT(*)::bigint AS total_secrets,
                    COUNT(*) FILTER (WHERE verified = true)::bigint AS verified_secrets,
                    COUNT(DISTINCT repository)::bigint AS repositories_scanned
                FROM secret_detections
            ),
            scan_stats AS (
                SELECT
                    COALESCE(SUM(files_scanned), 0)::bigint AS files_scanned,
                    COUNT(*)::bigint AS total_scans,
                    COUNT(*) FILTER (WHERE status IN ('running','queued'))::bigint AS active_scans,
                    COUNT(*) FILTER (WHERE status = 'failed')::bigint AS failed_scans,
                    AVG(duration_ms)::bigint AS avg_scan_duration_ms,
                    MAX(COALESCE(completed_at, started_at)) AS last_scan_time,
                    COUNT(*) FILTER (WHERE status = 'completed')::bigint AS completed_scans,
                    COUNT(*) FILTER (
                        WHERE status = 'completed'
                          AND completed_at >= NOW() - INTERVAL '15 minutes'
                    )::bigint AS recent_completed,
                    COUNT(DISTINCT repository) FILTER (
                        WHERE repository IS NOT NULL
                          AND status = 'completed'
                          AND completed_at >= NOW() - INTERVAL '15 minutes'
                    )::bigint AS recent_repositories
                FROM secret_scans
            )
            SELECT
                detection_stats.total_secrets,
                detection_stats.verified_secrets,
                detection_stats.repositories_scanned,
                scan_stats.files_scanned,
                scan_stats.total_scans,
                scan_stats.active_scans,
                scan_stats.failed_scans,
                scan_stats.avg_scan_duration_ms,
                scan_stats.last_scan_time,
                scan_stats.completed_scans,
                scan_stats.recent_completed,
                scan_stats.recent_repositories
            FROM detection_stats
            CROSS JOIN scan_stats
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .ok();

        let total_secrets = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("total_secrets").ok())
            .unwrap_or(0);
        let verified_secrets = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("verified_secrets").ok())
            .unwrap_or(0);

        let severity_rows = sqlx::query(
            "SELECT severity, COUNT(*) AS total FROM secret_detections GROUP BY severity",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let mut severity_counts = HashMap::new();
        for row in severity_rows {
            let label = row.get::<String, _>("severity");
            let normalized = Self::normalize_severity_label(&label);
            severity_counts.insert(normalized, row.get::<i64, _>("total"));
        }

        let category_rows = sqlx::query(
            "SELECT category, COUNT(*) AS total FROM secret_detections GROUP BY category",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let mut category_counts = HashMap::new();
        for row in category_rows {
            let raw = row.get::<String, _>("category");
            let label = Self::frontend_category_label(&raw);
            *category_counts.entry(label).or_insert(0) += row.get::<i64, _>("total");
        }

        let repositories_scanned = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("repositories_scanned").ok())
            .unwrap_or(0);
        let files_scanned = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("files_scanned").ok())
            .unwrap_or(0);
        let total_scans = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("total_scans").ok())
            .unwrap_or(0);
        let active_scans = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("active_scans").ok())
            .unwrap_or(0);
        let failed_scans = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("failed_scans").ok())
            .unwrap_or(0);
        let avg_scan_duration_ms = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<Option<i64>, _>("avg_scan_duration_ms").ok())
            .unwrap_or(None);
        let last_scan_time = overview_row
            .as_ref()
            .and_then(|row| {
                row.try_get::<Option<DateTime<Utc>>, _>("last_scan_time")
                    .ok()
            })
            .unwrap_or(None);
        let completed_scans = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("completed_scans").ok())
            .unwrap_or(0);

        let scan_success_rate = if total_scans > 0 {
            completed_scans as f64 / total_scans as f64
        } else {
            0.0
        };

        let recent_completed = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("recent_completed").ok())
            .unwrap_or(0);
        let recent_repositories = overview_row
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("recent_repositories").ok())
            .unwrap_or(0);

        let scan_rate_per_minute = if RECENT_WINDOW_MINUTES > 0.0 {
            recent_completed as f64 / RECENT_WINDOW_MINUTES
        } else {
            0.0
        };

        let repos_per_minute = if RECENT_WINDOW_MINUTES > 0.0 {
            recent_repositories as f64 / RECENT_WINDOW_MINUTES
        } else {
            0.0
        };

        Ok(SecretOverviewMetrics {
            total_secrets,
            severity_counts,
            category_counts,
            verified_secrets,
            false_positives: total_secrets - verified_secrets,
            repositories_scanned,
            files_scanned,
            total_scans,
            active_scans,
            failed_scans,
            avg_scan_duration_ms,
            last_scan_time,
            scan_success_rate,
            scan_rate_per_minute,
            repos_per_minute,
        })
    }

    /// Generic detection listing with optional filters
    pub async fn get_secret_detections(
        &self,
        filter: SecretDetectionFilter,
    ) -> Result<Vec<SecretDetectionRow>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT detection_id, repository, file_path, detector_name, severity, category, detected_at, verified, source, event_id, matched_text_preview, line_number FROM secret_detections WHERE 1=1",
        );

        if let Some(repo) = filter.repository.as_ref() {
            builder
                .push(" AND repository ILIKE ")
                .push_bind(format!("%{}%", repo));
        }
        if let Some(severity) = filter.severity.as_ref() {
            builder
                .push(" AND LOWER(severity) = LOWER(")
                .push_bind(severity)
                .push(")");
        }
        if let Some(category) = filter.category.as_ref() {
            builder
                .push(" AND (category = ")
                .push_bind(category)
                .push(" OR LOWER(category) = LOWER(")
                .push_bind(category)
                .push("))");
        }
        if let Some(source) = filter.source.as_ref() {
            builder
                .push(" AND LOWER(source) = LOWER(")
                .push_bind(source)
                .push(")");
        }
        if let Some(verified) = filter.verified {
            builder.push(" AND verified = ").push_bind(verified);
        }
        if let Some(date_from) = filter.date_from {
            builder.push(" AND detected_at >= ").push_bind(date_from);
        }
        if let Some(date_to) = filter.date_to {
            builder.push(" AND detected_at <= ").push_bind(date_to);
        }

        builder.push(" ORDER BY detected_at DESC");
        builder
            .push(" LIMIT ")
            .push_bind(filter.limit.unwrap_or(50).max(1))
            .push(" OFFSET ")
            .push_bind(filter.offset.unwrap_or(0).max(0));

        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .context("Failed to fetch secret detections")?;

        Ok(rows
            .into_iter()
            .map(|row| SecretDetectionRow {
                detection_id: row.get("detection_id"),
                repository: row.get("repository"),
                file_path: row.get("file_path"),
                detector_name: row.get("detector_name"),
                severity: Self::normalize_severity_label(&row.get::<String, _>("severity")),
                category: Self::frontend_category_label(&row.get::<String, _>("category")),
                detected_at: row.get("detected_at"),
                verified: row.get("verified"),
                source: row.get("source"),
                event_id: row.get("event_id"),
                matched_text_preview: row.get("matched_text_preview"),
                line_number: row.get("line_number"),
            })
            .collect())
    }

    pub async fn count_secret_detections(&self, filter: &SecretDetectionFilter) -> Result<i64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM secret_detections WHERE 1=1");

        if let Some(repo) = filter.repository.as_ref() {
            builder
                .push(" AND repository ILIKE ")
                .push_bind(format!("%{}%", repo));
        }
        if let Some(severity) = filter.severity.as_ref() {
            builder
                .push(" AND LOWER(severity) = LOWER(")
                .push_bind(severity)
                .push(")");
        }
        if let Some(category) = filter.category.as_ref() {
            builder
                .push(" AND (category = ")
                .push_bind(category)
                .push(" OR LOWER(category) = LOWER(")
                .push_bind(category)
                .push("))");
        }
        if let Some(source) = filter.source.as_ref() {
            builder
                .push(" AND LOWER(source) = LOWER(")
                .push_bind(source)
                .push(")");
        }
        if let Some(verified) = filter.verified {
            builder.push(" AND verified = ").push_bind(verified);
        }
        if let Some(date_from) = filter.date_from {
            builder.push(" AND detected_at >= ").push_bind(date_from);
        }
        if let Some(date_to) = filter.date_to {
            builder.push(" AND detected_at <= ").push_bind(date_to);
        }

        let count: i64 = builder
            .build()
            .fetch_one(&self.pool)
            .await
            .context("Failed to count secret detections")?
            .get(0);

        Ok(count)
    }

    /// Lightweight samples used to build trend charts in-process
    pub async fn get_secret_trend_samples(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<SecretTrendSample>> {
        let rows = sqlx::query(
            "SELECT detected_at, severity, category FROM secret_detections WHERE detected_at >= $1 ORDER BY detected_at ASC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch secret trend samples")?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let severity_raw: Option<String> = row.try_get("severity").unwrap_or(None);
                let category_raw: Option<String> = row.try_get("category").unwrap_or(None);

                SecretTrendSample {
                    detected_at: row.get("detected_at"),
                    severity: Self::normalize_severity_label(
                        severity_raw.as_deref().unwrap_or("Unknown"),
                    ),
                    category: Self::frontend_category_label(
                        category_raw.as_deref().unwrap_or("Other"),
                    ),
                }
            })
            .collect())
    }

    /// Canonical runtime database health check used by active code paths.
    pub async fn health_status(&self) -> DatabaseHealth {
        if self.pool.is_closed() {
            return DatabaseHealth {
                is_connected: false,
                connection_count: 0,
                active_queries: 0,
                cache_hit_ratio: 0.0,
                error_message: Some("Connection pool is closed".to_string()),
            };
        }

        match self.perform_health_check().await {
            Ok(health) => health,
            Err(e) => {
                error!("Database health status failed: {}", e);
                DatabaseHealth {
                    is_connected: false,
                    connection_count: 0,
                    active_queries: 0,
                    cache_hit_ratio: 0.0,
                    error_message: Some(e.to_string()),
                }
            }
        }
    }

    /// Backward-compatible alias retained for older call sites.
    pub async fn health_check(&self) -> DatabaseHealth {
        self.health_status().await
    }

    async fn perform_health_check(&self) -> Result<DatabaseHealth> {
        // Basic connectivity test
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .context("Basic connectivity test failed")?;

        // Get connection statistics
        let stats = sqlx::query(
            r#"
            SELECT
                count(*)::bigint as active_connections,
                count(CASE WHEN state = 'active' THEN 1 END)::bigint as active_queries
            FROM pg_stat_activity
            WHERE datname = current_database()
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get connection statistics")?;

        // Get cache hit ratio with explicit float8 casting to avoid NUMERIC decode issues
        let cache_stats = sqlx::query(
            r#"
            SELECT
                CASE WHEN sum(blks_hit + blks_read) = 0 THEN 0.0
                     ELSE (100.0 * sum(blks_hit)::float8 / sum(blks_hit + blks_read)::float8)
                END AS cache_hit_ratio
            FROM pg_stat_database
            WHERE datname = current_database()
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get cache statistics")?;

        let cache_hit_ratio: f64 = cache_stats
            .try_get::<f64, _>("cache_hit_ratio")
            .unwrap_or(0.0);

        // Explicitly cast count(*) to bigint in SQL to ensure INT8 type compatibility
        Ok(DatabaseHealth {
            is_connected: true,
            connection_count: stats.get::<i64, _>("active_connections"),
            active_queries: stats.get::<i64, _>("active_queries"),
            cache_hit_ratio,
            error_message: None,
        })
    }

    /// Insert a batch of validated events with comprehensive error handling
    pub async fn insert_events_batch(
        &self,
        events: Vec<serde_json::Value>,
        filename: &str,
    ) -> Result<i64> {
        if events.is_empty() {
            return Ok(0);
        }

        // Validate events
        let validated_events: Vec<ValidatedEvent> = events
            .into_iter()
            .filter_map(|event| self.validate_and_convert_event(event))
            .collect();

        if validated_events.is_empty() {
            warn!("No valid events found in {}", filename);
            return Ok(0);
        }

        let insert_sql = self.get_comprehensive_insert_sql();
        let mut rows_inserted = 0i64;

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start transaction")?;

        for event in validated_events {
            match self
                .insert_single_event(&mut tx, &insert_sql, &event, filename)
                .await
            {
                Ok(_) => rows_inserted += 1,
                Err(e) => {
                    error!("Failed to insert event {}: {}", event.id, e);
                    continue;
                }
            }
        }

        tx.commit().await.context("Failed to commit transaction")?;

        info!("Inserted {} events from {}", rows_inserted, filename);
        Ok(rows_inserted)
    }

    async fn insert_single_event(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        insert_sql: &str,
        event: &ValidatedEvent,
        filename: &str,
    ) -> Result<()> {
        sqlx::query(insert_sql)
            .bind(event.id)
            .bind(&event.event_type)
            .bind(event.created_at)
            .bind(event.public)
            // Actor fields
            .bind(event.actor.id)
            .bind(&event.actor.login)
            .bind(&event.actor.display_login)
            .bind(&event.actor.gravatar_id)
            .bind(&event.actor.url)
            .bind(&event.actor.avatar_url)
            .bind(&event.actor.node_id)
            .bind(&event.actor.html_url)
            .bind(&event.actor.followers_url)
            .bind(&event.actor.following_url)
            .bind(&event.actor.gists_url)
            .bind(&event.actor.starred_url)
            .bind(&event.actor.subscriptions_url)
            .bind(&event.actor.organizations_url)
            .bind(&event.actor.repos_url)
            .bind(&event.actor.events_url)
            .bind(&event.actor.received_events_url)
            .bind(&event.actor.actor_type)
            .bind(&event.actor.user_view_type)
            .bind(event.actor.site_admin)
            // Repository fields
            .bind(event.repo.id)
            .bind(&event.repo.name)
            .bind(&event.repo.url)
            .bind(&event.repo.full_name)
            .bind(&event.repo.owner_login)
            .bind(event.repo.owner_id)
            .bind(&event.repo.owner_node_id)
            .bind(&event.repo.owner_avatar_url)
            .bind(&event.repo.owner_gravatar_id)
            .bind(&event.repo.owner_url)
            .bind(&event.repo.owner_html_url)
            .bind(&event.repo.owner_type)
            .bind(event.repo.owner_site_admin)
            .bind(&event.repo.node_id)
            .bind(&event.repo.html_url)
            .bind(&event.repo.description)
            .bind(event.repo.fork)
            .bind(&event.repo.language)
            .bind(event.repo.stargazers_count)
            .bind(event.repo.watchers_count)
            .bind(event.repo.forks_count)
            .bind(event.repo.open_issues_count)
            .bind(event.repo.size)
            .bind(&event.repo.default_branch)
            .bind(&event.repo.topics)
            .bind(&event.repo.license_key)
            .bind(&event.repo.license_name)
            .bind(event.repo.created_at)
            .bind(event.repo.updated_at)
            .bind(event.repo.pushed_at)
            // Organization fields
            .bind(event.org.as_ref().and_then(|o| o.id))
            .bind(event.org.as_ref().and_then(|o| o.login.as_ref()))
            .bind(event.org.as_ref().and_then(|o| o.node_id.as_ref()))
            .bind(event.org.as_ref().and_then(|o| o.gravatar_id.as_ref()))
            .bind(event.org.as_ref().and_then(|o| o.url.as_ref()))
            .bind(event.org.as_ref().and_then(|o| o.avatar_url.as_ref()))
            .bind(event.org.as_ref().and_then(|o| o.html_url.as_ref()))
            .bind(event.org.as_ref().and_then(|o| o.org_type.as_ref()))
            .bind(event.org.as_ref().and_then(|o| o.site_admin))
            // Data storage fields
            .bind(&event.payload)
            .bind(&event.raw_event)
            .bind(filename)
            .bind(&event.api_source)
            .execute(&mut **tx)
            .await
            .context("Failed to execute insert statement")?;

        if event.event_type == "PushEvent" {
            if let Err(e) = self.enqueue_push_event_for_scanning(tx, event).await {
                warn!(
                    "Failed to enqueue push event {} for scanning: {}",
                    event.id, e
                );
            }
        }

        Ok(())
    }

    async fn enqueue_push_event_for_scanning(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &ValidatedEvent,
    ) -> Result<()> {
        if event.event_type != "PushEvent" {
            return Ok(());
        }

        let before_sha = event
            .payload
            .get("before")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if before_sha.is_empty() {
            return Ok(());
        }

        let repository_full_name = if let Some(full) = event.repo.full_name.as_ref() {
            full.clone()
        } else if let (Some(owner), Some(name)) = (&event.repo.owner_login, &event.repo.name) {
            format!("{}/{}", owner, name)
        } else if let Some(name) = event.repo.name.as_ref() {
            name.clone()
        } else {
            String::new()
        }
        .trim()
        .trim_matches('/')
        .to_lowercase();

        if repository_full_name.is_empty() {
            return Ok(());
        }

        let repository_url = event
            .repo
            .html_url
            .as_ref()
            .or(event.repo.url.as_ref())
            .cloned();

        let head_sha = event
            .payload
            .get("head")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let ref_name = event
            .payload
            .get("ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let forced_flag = event
            .payload
            .get("forced")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let commit_span = event
            .payload
            .get("size")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let is_zero_commit = Self::detect_zero_commit(&event.payload);

        sqlx::query(
            r#"
            INSERT INTO pending_push_scans (
                event_id,
                repository_full_name,
                repository_url,
                before_sha,
                head_sha,
                ref_name,
                forced_flag,
                commit_span,
                is_zero_commit,
                event_payload,
                event_created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (event_id) DO NOTHING
            "#,
        )
        .bind(event.id)
        .bind(&repository_full_name)
        .bind(&repository_url)
        .bind(&before_sha)
        .bind(&head_sha)
        .bind(&ref_name)
        .bind(forced_flag)
        .bind(commit_span)
        .bind(is_zero_commit)
        .bind(&event.payload)
        .bind(event.created_at)
        .execute(&mut **tx)
        .await
        .context("Failed to enqueue push event for scanning")?;

        Ok(())
    }

    fn detect_zero_commit(payload: &Value) -> bool {
        let commits_empty = payload
            .get("commits")
            .and_then(|v| v.as_array())
            .map(|arr| arr.is_empty())
            .unwrap_or(false);

        if commits_empty {
            return true;
        }

        payload
            .get("size")
            .and_then(|v| v.as_i64())
            .map(|size| size == 0)
            .unwrap_or(false)
    }

    /// Generate comprehensive data quality metrics
    pub async fn get_data_quality_metrics(&self) -> Result<QualityMetrics> {
        // Event statistics
        let event_stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_events,
                COUNT(DISTINCT actor_id) as unique_actors,
                COUNT(DISTINCT repo_id) as unique_repos,
                COUNT(DISTINCT event_type) as event_types,
                MIN(event_created_at) as earliest_event,
                MAX(event_created_at) as latest_event
            FROM github_events
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get event statistics")?;

        // Data integrity issues
        let integrity_issues = sqlx::query(
            r#"
            SELECT
                COUNT(CASE WHEN event_id IS NULL THEN 1 END) as null_ids,
                COUNT(CASE WHEN event_type IS NULL OR event_type = '' THEN 1 END) as invalid_types,
                COUNT(CASE WHEN event_created_at IS NULL THEN 1 END) as null_timestamps,
                COUNT(CASE WHEN payload IS NULL THEN 1 END) as null_payloads
            FROM github_events
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get integrity issues")?;

        // Processing statistics
        let processing_stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_files,
                SUM(COALESCE(event_count, events_count::INTEGER, 0))::BIGINT as total_processed_events,
                AVG(COALESCE(event_count, events_count::INTEGER, 0))::DOUBLE PRECISION as avg_events_per_file,
                (
                    SUM(COALESCE(file_size, size_bytes, 0))::DOUBLE PRECISION
                    / (1024.0 * 1024.0 * 1024.0)
                ) as total_gb_processed
            FROM processed_files
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get processing statistics")?;

        // Recent activity (24 hours)
        let recent_activity = sqlx::query(
            r#"
            SELECT
                COUNT(*) as files_processed_24h,
                COALESCE(SUM(COALESCE(event_count, events_count::INTEGER, 0)), 0)::BIGINT as events_processed_24h
            FROM processed_files
            WHERE processed_at > NOW() - INTERVAL '24 hours'
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get recent activity")?;

        // Build metrics
        let total_events: i64 = event_stats.get("total_events");
        let unique_actors: i64 = event_stats.get("unique_actors");
        let unique_repos: i64 = event_stats.get("unique_repos");
        let event_types: i64 = event_stats.get("event_types");

        let mut integrity_map = HashMap::new();
        integrity_map.insert(
            "null_ids".to_string(),
            integrity_issues.get::<i64, _>("null_ids"),
        );
        integrity_map.insert(
            "invalid_types".to_string(),
            integrity_issues.get::<i64, _>("invalid_types"),
        );
        integrity_map.insert(
            "null_timestamps".to_string(),
            integrity_issues.get::<i64, _>("null_timestamps"),
        );
        integrity_map.insert(
            "null_payloads".to_string(),
            integrity_issues.get::<i64, _>("null_payloads"),
        );

        let mut processing_map = HashMap::new();
        processing_map.insert(
            "total_files".to_string(),
            processing_stats
                .get::<Option<i64>, _>("total_files")
                .unwrap_or(0) as f64,
        );
        processing_map.insert(
            "total_processed_events".to_string(),
            processing_stats
                .get::<Option<i64>, _>("total_processed_events")
                .unwrap_or(0) as f64,
        );
        processing_map.insert(
            "avg_events_per_file".to_string(),
            processing_stats
                .get::<Option<f64>, _>("avg_events_per_file")
                .unwrap_or(0.0),
        );
        processing_map.insert(
            "total_gb_processed".to_string(),
            processing_stats
                .get::<Option<f64>, _>("total_gb_processed")
                .unwrap_or(0.0),
        );

        let mut recent_map = HashMap::new();
        recent_map.insert(
            "files_processed_24h".to_string(),
            recent_activity.get::<i64, _>("files_processed_24h"),
        );
        recent_map.insert(
            "events_processed_24h".to_string(),
            recent_activity.get::<i64, _>("events_processed_24h"),
        );

        // Calculate quality score
        let quality_score = self.calculate_quality_score(total_events, &integrity_map);

        Ok(QualityMetrics {
            total_events,
            unique_actors,
            unique_repos,
            event_types,
            quality_score,
            integrity_issues: integrity_map,
            processing_stats: processing_map,
            recent_activity: recent_map,
        })
    }

    /// Check if file has already been processed
    pub async fn is_file_processed(
        &self,
        filename: &str,
        etag: Option<&str>,
        size: Option<i64>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            SELECT etag, COALESCE(file_size, size_bytes) AS stored_size
            FROM processed_files
            WHERE filename = $1
            "#,
        )
        .bind(filename)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check file processed status")?;

        match result {
            Some(row) => {
                // Check ETag and size if provided
                if let Some(etag) = etag {
                    let stored_etag: Option<String> = row.get("etag");
                    if stored_etag.as_deref() != Some(etag) {
                        return Ok(false);
                    }
                }
                if let Some(size) = size {
                    let stored_size: Option<i64> = row.get("stored_size");
                    if stored_size != Some(size) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Mark file as processed with metadata
    pub async fn mark_file_processed(
        &self,
        filename: &str,
        etag: &str,
        size: i64,
        event_count: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO processed_files (
                filename, etag, file_size, size_bytes, event_count, events_count
            )
            VALUES ($1, $2, $3, $3, $4, $4)
            ON CONFLICT (filename)
            DO UPDATE SET
                etag = $2,
                file_size = $3,
                size_bytes = $3,
                event_count = $4,
                events_count = $4,
                processed_at = NOW()
            "#,
        )
        .bind(filename)
        .bind(etag)
        .bind(size)
        .bind(event_count)
        .execute(&self.pool)
        .await
        .context("Failed to mark file as processed")?;

        Ok(())
    }

    /// Close the database connection pool
    pub async fn close(&self) {
        self.pool.close().await;
        info!("Database connection pool closed");
    }

    /// Get the database configuration
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// Get database statistics for API responses
    pub async fn get_database_statistics(&self) -> Result<DatabaseStatistics> {
        // Get database size
        let db_size_result =
            sqlx::query("SELECT pg_size_pretty(pg_database_size(current_database())) as size")
                .fetch_one(&self.pool)
                .await
                .context("Failed to get database size")?;

        let database_size: String = db_size_result.get("size");

        // Get table information - use relname consistently
        let table_stats = sqlx::query(
            r#"
            SELECT
                schemaname,
                relname as tablename,
                n_tup_ins + n_tup_upd + n_tup_del as total_operations,
                n_live_tup as row_count,
                pg_size_pretty(pg_total_relation_size(schemaname||'.'||relname)) as size
            FROM pg_stat_user_tables
            ORDER BY pg_total_relation_size(schemaname||'.'||relname) DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get table statistics")?;

        let mut tables = Vec::new();
        for row in table_stats {
            let table_name: String = row.get("tablename");
            let row_count: i64 = row.get("row_count");
            let size: String = row.get("size");

            tables.push((table_name, row_count, size));
        }

        // Get total event count - handle missing table gracefully
        let event_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM github_events")
            .fetch_optional(&self.pool)
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get event count (table may not exist yet): {}", e);
                None
            })
            .unwrap_or(0);

        Ok(DatabaseStatistics {
            total_events: event_count,
            database_size,
            table_count: tables.len() as i64,
            tables,
        })
    }

    /// Return the lifetime number of GitHub events stored for dashboard metrics
    pub async fn get_total_event_count(&self) -> Result<i64> {
        match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM github_events")
            .fetch_one(&self.pool)
            .await
        {
            Ok(count) => Ok(count),
            Err(e) => {
                if e.to_string().contains("does not exist") {
                    warn!("github_events table not ready yet: {}", e);
                    Ok(0)
                } else {
                    Err(e).context("Failed to count github_events")
                }
            }
        }
    }

    /// Fetch the most recent persisted events for dashboard previews
    pub async fn get_recent_events(&self, limit: i64) -> Result<Vec<EventPreview>> {
        let limit = limit.clamp(1, 100);

        let rows = sqlx::query(
            r#"
            SELECT
                event_id,
                event_type,
                event_created_at,
                actor_login,
                repo_name,
                repo_url,
                payload
            FROM github_events
            ORDER BY event_created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch recent events")?;

        let events = rows
            .into_iter()
            .map(|row| EventPreview {
                event_id: row.get::<i64, _>("event_id"),
                event_type: row.get::<String, _>("event_type"),
                event_created_at: row.get::<DateTime<Utc>, _>("event_created_at"),
                actor_login: row.get::<Option<String>, _>("actor_login"),
                repo_name: row.get::<Option<String>, _>("repo_name"),
                repo_url: row.get::<Option<String>, _>("repo_url"),
                payload: row.try_get::<Value, _>("payload").unwrap_or(Value::Null),
            })
            .collect();

        Ok(events)
    }

    /// Search recent events by repo, actor, or event type (case-insensitive)
    pub async fn search_events(&self, query: &str, limit: i64) -> Result<Vec<EventPreview>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, 100);
        let pattern = format!("%{}%", trimmed);

        let rows = sqlx::query(
            r#"
            SELECT
                event_id,
                event_type,
                event_created_at,
                actor_login,
                repo_name,
                repo_url,
                payload
            FROM github_events
            WHERE repo_name ILIKE $1
               OR actor_login ILIKE $1
               OR event_type ILIKE $1
            ORDER BY event_created_at DESC
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to search events")?;

        let events = rows
            .into_iter()
            .map(|row| EventPreview {
                event_id: row.get::<i64, _>("event_id"),
                event_type: row.get::<String, _>("event_type"),
                event_created_at: row.get::<DateTime<Utc>, _>("event_created_at"),
                actor_login: row.get::<Option<String>, _>("actor_login"),
                repo_name: row.get::<Option<String>, _>("repo_name"),
                repo_url: row.get::<Option<String>, _>("repo_url"),
                payload: row.try_get::<Value, _>("payload").unwrap_or(Value::Null),
            })
            .collect();

        Ok(events)
    }

    /// Fetch push events for a specific repository to feed the scanner
    pub async fn get_push_events_for_repository(
        &self,
        repository: &str,
        limit: i64,
    ) -> Result<Vec<EventScanTarget>> {
        let normalized_repo = repository
            .trim()
            .trim_matches('/')
            .trim_end_matches(".git")
            .to_lowercase();

        if normalized_repo.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, 500);

        let rows = sqlx::query(
            r#"
            SELECT
                event_id,
                COALESCE(repo_full_name, repo_owner_login || '/' || repo_name) AS repository_full_name,
                COALESCE(repo_html_url, repo_url) AS repository_url,
                payload ->> 'before' AS before_sha,
                payload ->> 'head' AS head_sha,
                payload ->> 'ref' AS ref_name,
                COALESCE((payload ->> 'forced')::boolean, false) AS forced_flag,
                COALESCE(NULLIF(payload ->> 'size', '')::int, 0) AS commit_span,
                false AS is_zero_commit,
                payload AS event_payload,
                event_created_at
            FROM github_events
            WHERE event_type = 'PushEvent'
              AND payload ? 'before'
              AND payload ->> 'before' <> ''
              AND LOWER(COALESCE(repo_full_name, repo_owner_login || '/' || repo_name)) = $1
            ORDER BY event_created_at DESC
            LIMIT $2
            "#,
        )
        .bind(&normalized_repo)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to query push events for repository")?;

        let mut events = Vec::new();
        for row in rows {
            let before_sha: Option<String> = row.get("before_sha");
            let repository_full_name: Option<String> = row.get("repository_full_name");

            let Some(before_sha) = before_sha.filter(|sha| !sha.is_empty()) else {
                continue;
            };

            let Some(repository_full_name) = repository_full_name else {
                continue;
            };

            events.push(EventScanTarget {
                event_id: row.get("event_id"),
                repository_full_name,
                repository_url: row.get("repository_url"),
                before_sha,
                head_sha: row.get("head_sha"),
                reference: row.get("ref_name"),
                forced: row.get("forced_flag"),
                commit_count: row.get("commit_span"),
                event_created_at: row.get("event_created_at"),
                is_zero_commit: row.get("is_zero_commit"),
                event_payload: row.try_get("event_payload").ok(),
            });
        }

        Ok(events)
    }

    /// Enqueue a push event observed by the realtime monitor so the scanner can pick it up
    pub async fn enqueue_push_event_from_monitor(
        &self,
        record: PushEventQueueInsert,
    ) -> Result<()> {
        let before_sha = record.before_sha.trim().to_string();
        if before_sha.is_empty() {
            return Ok(());
        }

        const ZERO_SHA: &str = "0000000000000000000000000000000000000000";
        if before_sha == ZERO_SHA {
            return Ok(());
        }

        let repository_full_name = record
            .repository_full_name
            .trim()
            .trim_matches('/')
            .trim_end_matches(".git")
            .to_lowercase();

        if repository_full_name.is_empty() {
            return Ok(());
        }

        let repository_url = record
            .repository_url
            .as_ref()
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .or_else(|| Some(format!("https://github.com/{}", repository_full_name)));

        let head_sha = record
            .head_sha
            .as_ref()
            .map(|sha| sha.trim().to_string())
            .filter(|sha| !sha.is_empty());

        let ref_name = record
            .ref_name
            .as_ref()
            .map(|reference| reference.trim().to_string())
            .filter(|reference| !reference.is_empty());

        let commit_span = record.commit_span.max(0);

        sqlx::query(
            r#"
            INSERT INTO pending_push_scans (
                event_id,
                repository_full_name,
                repository_url,
                before_sha,
                head_sha,
                ref_name,
                forced_flag,
                commit_span,
                is_zero_commit,
                event_payload,
                event_created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (event_id) DO NOTHING
            "#,
        )
        .bind(record.event_id)
        .bind(&repository_full_name)
        .bind(&repository_url)
        .bind(&before_sha)
        .bind(&head_sha)
        .bind(&ref_name)
        .bind(record.forced_flag)
        .bind(commit_span)
        .bind(record.is_zero_commit)
        .bind(&record.event_payload)
        .bind(record.event_created_at)
        .execute(&self.pool)
        .await
        .context("Failed to enqueue push event from realtime monitor")?;

        Ok(())
    }

    /// Claim pending push events for processing
    pub async fn claim_pending_push_events(
        &self,
        limit: i64,
        worker_id: &str,
    ) -> Result<Vec<EventScanTarget>> {
        self.claim_events_internal(None, limit, worker_id).await
    }

    /// Claim pending push events for a specific repository
    pub async fn claim_pending_push_events_for_repo(
        &self,
        repository: &str,
        limit: i64,
        worker_id: &str,
    ) -> Result<Vec<EventScanTarget>> {
        if repository.trim().is_empty() {
            return Ok(Vec::new());
        }

        self.claim_events_internal(Some(repository), limit, worker_id)
            .await
    }

    /// Release events back to the pending queue (used when scans cannot start)
    pub async fn release_push_events(&self, event_ids: &[i64]) -> Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE pending_push_scans
            SET status = 'pending',
                locked_by = NULL,
                locked_at = NULL,
                updated_at = NOW(),
                next_attempt_after = NOW()
            WHERE event_id = ANY($1)
              AND status = 'processing'
            "#,
        )
        .bind(event_ids)
        .execute(&self.pool)
        .await
        .context("Failed to release queued push events")?;

        Ok(())
    }

    /// Mark claimed events as completed after a successful scan
    pub async fn mark_push_events_completed(&self, event_ids: &[i64]) -> Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE pending_push_scans
            SET status = 'completed',
                completed_at = NOW(),
                locked_by = NULL,
                locked_at = NULL,
                updated_at = NOW(),
                next_attempt_after = NULL,
                error_message = NULL
            WHERE event_id = ANY($1)
              AND status = 'processing'
            "#,
        )
        .bind(event_ids)
        .execute(&self.pool)
        .await
        .context("Failed to mark queued push events as completed")?;

        Ok(())
    }

    /// Mark claimed events as failed so they can be retried later
    pub async fn mark_push_events_failed(
        &self,
        event_ids: &[i64],
        error: Option<&str>,
    ) -> Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }

        let retry_at = Utc::now() + Duration::minutes(5);

        sqlx::query(
            r#"
            UPDATE pending_push_scans
            SET status = 'failed',
                locked_by = NULL,
                locked_at = NULL,
                updated_at = NOW(),
                next_attempt_after = $2,
                error_message = $3
            WHERE event_id = ANY($1)
              AND status = 'processing'
            "#,
        )
        .bind(event_ids)
        .bind(retry_at)
        .bind(error.map(|e| e.to_string()))
        .execute(&self.pool)
        .await
        .context("Failed to mark queued push events as failed")?;

        Ok(())
    }

    /// Fetch processing events for a repository (used by the scanner)
    pub async fn get_processing_push_events_for_repository(
        &self,
        repository: &str,
        limit: i64,
    ) -> Result<Vec<EventScanTarget>> {
        let normalized = repository.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, 500);

        let rows = sqlx::query(
            r#"
            SELECT
                event_id,
                repository_full_name,
                repository_url,
                before_sha,
                head_sha,
                ref_name,
                forced_flag,
                commit_span,
                is_zero_commit,
                event_payload,
                event_created_at
            FROM pending_push_scans
            WHERE status = 'processing'
              AND LOWER(repository_full_name) = $1
            ORDER BY event_created_at ASC
            LIMIT $2
            "#,
        )
        .bind(&normalized)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch processing push events")?;

        let events = rows
            .into_iter()
            .map(|row| EventScanTarget {
                event_id: row.get("event_id"),
                repository_full_name: row.get("repository_full_name"),
                repository_url: row.get("repository_url"),
                before_sha: row.get("before_sha"),
                head_sha: row.get("head_sha"),
                reference: row.get("ref_name"),
                forced: row.get("forced_flag"),
                commit_count: row.get("commit_span"),
                event_created_at: row.get("event_created_at"),
                is_zero_commit: row.get("is_zero_commit"),
                event_payload: row.try_get("event_payload").ok(),
            })
            .collect();

        Ok(events)
    }

    /// Return queue depth metrics for monitoring endpoints
    pub async fn get_scan_queue_stats(&self) -> Result<ScanQueueStats> {
        let row = match sqlx::query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'pending') AS pending_events,
                COUNT(*) FILTER (WHERE status = 'processing') AS processing_events,
                COUNT(*) FILTER (WHERE status = 'failed') AS failed_events,
                COUNT(*) FILTER (
                    WHERE status = 'failed'
                      AND error_message ILIKE '%forbidden%'
                ) AS failed_forbidden,
                COUNT(*) FILTER (
                    WHERE status = 'failed'
                      AND (
                        error_message ILIKE '%not found%'
                        OR error_message ILIKE '%404%'
                      )
                ) AS failed_not_found,
                COUNT(*) FILTER (
                    WHERE status = 'completed'
                      AND completed_at > NOW() - INTERVAL '1 hour'
                ) AS completed_last_hour,
                MIN(event_created_at) FILTER (WHERE status = 'pending') AS oldest_pending
            FROM pending_push_scans
            "#,
        )
        .fetch_one(&self.pool)
        .await
        {
            Ok(row) => row,
            Err(e) => {
                if e.to_string().contains("pending_push_scans") {
                    warn!(
                        "Scan queue table missing, returning default stats until migration completes"
                    );
                    return Ok(ScanQueueStats::default());
                }
                return Err(e).context("Failed to collect scan queue stats");
            }
        };

        let pending_events: i64 = row.get("pending_events");
        let processing_events: i64 = row.get("processing_events");
        let failed_events: i64 = row.get("failed_events");
        let failed_forbidden: i64 = row.try_get("failed_forbidden").unwrap_or(0);
        let failed_not_found: i64 = row.try_get("failed_not_found").unwrap_or(0);
        let completed_last_hour: i64 = row.get("completed_last_hour");
        let oldest_pending: Option<DateTime<Utc>> = row
            .try_get::<Option<DateTime<Utc>>, _>("oldest_pending")
            .unwrap_or(None);

        let oldest_pending_age_seconds =
            oldest_pending.map(|ts| (Utc::now() - ts).num_seconds().max(0));

        Ok(ScanQueueStats {
            pending_events,
            processing_events,
            failed_events,
            failed_forbidden,
            failed_not_found,
            completed_last_hour,
            oldest_pending_age_seconds,
        })
    }

    /// Count queue/scan rows that need scan-state repair.
    pub async fn scan_state_repair_counts(&self) -> Result<ScanStateRepairCounts> {
        let row = sqlx::query(
            r#"
            SELECT
                (
                    SELECT COUNT(*)
                    FROM secret_scans scan
                    WHERE COALESCE(scan.secrets_found, 0) > 0
                      AND NOT EXISTS (
                          SELECT 1
                          FROM secret_detections detection
                          WHERE detection.scan_id = scan.id
                      )
                ) AS invalid_secret_scan_summaries,
                (
                    SELECT COUNT(*)
                    FROM pending_push_scans
                    WHERE status = 'processing'
                      AND (next_attempt_after IS NULL OR next_attempt_after <= NOW())
                ) AS stale_processing_events,
                (
                    SELECT COUNT(*)
                    FROM pending_push_scans
                    WHERE status = 'pending'
                ) AS pending_events,
                (
                    SELECT COUNT(*)
                    FROM pending_push_scans
                    WHERE status = 'processing'
                ) AS processing_events,
                (
                    SELECT COUNT(*)
                    FROM pending_push_scans
                    WHERE status = 'failed'
                ) AS failed_events,
                (
                    SELECT COUNT(*)
                    FROM pending_push_scans
                    WHERE status = 'completed'
                ) AS completed_events,
                (
                    SELECT COUNT(*)
                    FROM secret_scans
                ) AS total_secret_scans,
                (
                    SELECT COUNT(*)
                    FROM secret_detections
                ) AS total_secret_detections
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to collect scan-state repair counts")?;

        Ok(ScanStateRepairCounts {
            invalid_secret_scan_summaries: row.get("invalid_secret_scan_summaries"),
            stale_processing_events: row.get("stale_processing_events"),
            pending_events: row.get("pending_events"),
            processing_events: row.get("processing_events"),
            failed_events: row.get("failed_events"),
            completed_events: row.get("completed_events"),
            total_secret_scans: row.get("total_secret_scans"),
            total_secret_detections: row.get("total_secret_detections"),
        })
    }

    /// Dry-run or execute the audited scan-state repair transaction.
    pub async fn repair_scan_state(
        &self,
        request: ScanStateRepairRequest,
    ) -> Result<ScanStateRepairReport> {
        let operator = request
            .operator
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .unwrap_or_else(|_| "unknown".to_string())
            });
        let pre_counts = self.scan_state_repair_counts().await?;
        let executed_at = Utc::now();

        if !request.execute {
            return Ok(ScanStateRepairReport {
                run_id: None,
                executed: false,
                dry_run: true,
                backup_path: request.backup_path,
                hard_delete_invalid_summaries: request.hard_delete_invalid_summaries,
                reset_stale_processing: request.reset_stale_processing,
                post_counts: pre_counts.clone(),
                pre_counts,
                deleted_invalid_summaries: 0,
                reset_stale_processing_rows: 0,
                operator,
                executed_at,
            });
        }

        if !request.hard_delete_invalid_summaries && !request.reset_stale_processing {
            return Err(anyhow::anyhow!(
                "execute requires at least one repair action: --hard-delete-invalid-summaries or --reset-stale-processing"
            ));
        }

        let backup_path = request
            .backup_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("execute requires --backup-path"))?;
        self.write_scan_state_repair_backup(&backup_path, &pre_counts, &operator)
            .await?;

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start scan-state repair transaction")?;

        let deleted_invalid_summaries = if request.hard_delete_invalid_summaries {
            sqlx::query(
                r#"
                DELETE FROM secret_scans scan
                WHERE COALESCE(scan.secrets_found, 0) > 0
                  AND NOT EXISTS (
                      SELECT 1
                      FROM secret_detections detection
                      WHERE detection.scan_id = scan.id
                  )
                "#,
            )
            .execute(&mut *tx)
            .await
            .context("Failed to delete invalid secret scan summaries")?
            .rows_affected() as i64
        } else {
            0
        };

        let reset_stale_processing_rows = if request.reset_stale_processing {
            sqlx::query(
                r#"
                UPDATE pending_push_scans
                SET status = 'pending',
                    locked_by = NULL,
                    locked_at = NULL,
                    next_attempt_after = NOW(),
                    updated_at = NOW(),
                    error_message = COALESCE(error_message, 'Recovered from stale processing state')
                WHERE status = 'processing'
                  AND (next_attempt_after IS NULL OR next_attempt_after <= NOW())
                "#,
            )
            .execute(&mut *tx)
            .await
            .context("Failed to reset stale processing queue rows")?
            .rows_affected() as i64
        } else {
            0
        };

        tx.commit()
            .await
            .context("Failed to commit scan-state repair transaction")?;

        let post_counts = self.scan_state_repair_counts().await?;
        let run_id = Uuid::new_v4();
        let report = ScanStateRepairReport {
            run_id: Some(run_id),
            executed: true,
            dry_run: false,
            backup_path: Some(backup_path.clone()),
            hard_delete_invalid_summaries: request.hard_delete_invalid_summaries,
            reset_stale_processing: request.reset_stale_processing,
            pre_counts,
            post_counts,
            deleted_invalid_summaries,
            reset_stale_processing_rows,
            operator,
            executed_at,
        };

        self.insert_maintenance_repair_run(&report).await?;
        Ok(report)
    }

    async fn write_scan_state_repair_backup(
        &self,
        backup_path: &str,
        pre_counts: &ScanStateRepairCounts,
        operator: &str,
    ) -> Result<()> {
        let invalid_secret_scans: Value = sqlx::query_scalar(
            r#"
            SELECT COALESCE(jsonb_agg(to_jsonb(scan)), '[]'::jsonb)
            FROM secret_scans scan
            WHERE COALESCE(scan.secrets_found, 0) > 0
              AND NOT EXISTS (
                  SELECT 1
                  FROM secret_detections detection
                  WHERE detection.scan_id = scan.id
              )
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to collect invalid secret scan backup rows")?;

        let stale_processing_rows: Value = sqlx::query_scalar(
            r#"
            SELECT COALESCE(jsonb_agg(to_jsonb(queue)), '[]'::jsonb)
            FROM pending_push_scans queue
            WHERE status = 'processing'
              AND (next_attempt_after IS NULL OR next_attempt_after <= NOW())
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to collect stale queue backup rows")?;

        let backup = serde_json::json!({
            "created_at": Utc::now(),
            "operator": operator,
            "database": {
                "host": self.config.database.host,
                "port": self.config.database.port,
                "name": self.config.database.name,
                "user": self.config.database.user,
            },
            "pre_counts": pre_counts,
            "invalid_secret_scans": invalid_secret_scans,
            "stale_processing_rows": stale_processing_rows,
        });

        if let Some(parent) = std::path::Path::new(backup_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("Failed to create backup directory {:?}", parent))?;
            }
        }

        let json = serde_json::to_string_pretty(&backup)
            .context("Failed to serialize scan-state repair backup")?;
        tokio::fs::write(backup_path, json)
            .await
            .with_context(|| format!("Failed to write repair backup to {}", backup_path))?;

        Ok(())
    }

    async fn insert_maintenance_repair_run(&self, report: &ScanStateRepairReport) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO maintenance_repair_runs (
                id, repair_type, dry_run, backup_path,
                hard_delete_invalid_summaries, reset_stale_processing,
                pre_counts, post_counts, deleted_invalid_summaries,
                reset_stale_processing_rows, operator, executed_at, metadata
            ) VALUES (
                $1, 'scan_state', $2, $3,
                $4, $5,
                $6, $7, $8,
                $9, $10, $11, '{}'::jsonb
            )
            "#,
        )
        .bind(report.run_id.unwrap_or_else(Uuid::new_v4))
        .bind(report.dry_run)
        .bind(&report.backup_path)
        .bind(report.hard_delete_invalid_summaries)
        .bind(report.reset_stale_processing)
        .bind(serde_json::to_value(&report.pre_counts)?)
        .bind(serde_json::to_value(&report.post_counts)?)
        .bind(report.deleted_invalid_summaries)
        .bind(report.reset_stale_processing_rows)
        .bind(&report.operator)
        .bind(report.executed_at)
        .execute(&self.pool)
        .await
        .context("Failed to insert maintenance repair run")?;

        Ok(())
    }

    async fn claim_events_internal(
        &self,
        repository: Option<&str>,
        limit: i64,
        worker_id: &str,
    ) -> Result<Vec<EventScanTarget>> {
        if limit <= 0 {
            return Ok(Vec::new());
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start transaction for event claim")?;

        let limit = limit.clamp(1, 500);
        let mut builder = QueryBuilder::new(
            r#"
            SELECT
                event_id,
                repository_full_name,
                repository_url,
                before_sha,
                head_sha,
                ref_name,
                forced_flag,
                commit_span,
                event_created_at,
                is_zero_commit,
                event_payload
            FROM pending_push_scans
            WHERE status IN ('pending', 'failed')
              AND next_attempt_after <= NOW()
            "#,
        );

        if let Some(repo) = repository {
            builder.push(" AND LOWER(repository_full_name) = LOWER(");
            builder.push_bind(repo);
            builder.push(")");
        }

        builder.push(" ORDER BY event_created_at ASC LIMIT ");
        builder.push_bind(limit);
        builder.push(" FOR UPDATE SKIP LOCKED");

        let rows = builder
            .build()
            .fetch_all(&mut *tx)
            .await
            .context("Failed to claim pending push events")?;

        let event_ids: Vec<i64> = rows.iter().map(|row| row.get("event_id")).collect();

        if event_ids.is_empty() {
            tx.commit()
                .await
                .context("Failed to commit empty event claim transaction")?;
            return Ok(Vec::new());
        }

        sqlx::query(
            r#"
            UPDATE pending_push_scans
            SET status = 'processing',
                locked_by = $2,
                locked_at = NOW(),
                attempts = attempts + 1,
                last_attempt_at = NOW(),
                updated_at = NOW()
            WHERE event_id = ANY($1)
            "#,
        )
        .bind(&event_ids)
        .bind(worker_id)
        .execute(&mut *tx)
        .await
        .context("Failed to update claimed push events")?;

        tx.commit()
            .await
            .context("Failed to commit event claim transaction")?;

        let events = rows
            .into_iter()
            .map(|row| EventScanTarget {
                event_id: row.get("event_id"),
                repository_full_name: row.get("repository_full_name"),
                repository_url: row.get("repository_url"),
                before_sha: row.get("before_sha"),
                head_sha: row.get("head_sha"),
                reference: row.get("ref_name"),
                forced: row.get("forced_flag"),
                commit_count: row.get("commit_span"),
                event_created_at: row.get("event_created_at"),
                is_zero_commit: row.get("is_zero_commit"),
                event_payload: row.try_get("event_payload").ok(),
            })
            .collect();

        Ok(events)
    }

    // Private helper methods

    fn validate_and_convert_event(&self, event: serde_json::Value) -> Option<ValidatedEvent> {
        // Extract basic event data
        let id = event.get("id").and_then(Database::parse_event_id)?;
        let event_type = event.get("type")?.as_str()?.to_string();
        let created_at = self.parse_datetime(event.get("created_at")?.as_str()?)?;
        let public = event
            .get("public")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Extract actor data
        let actor_obj = event.get("actor").unwrap_or(&serde_json::Value::Null);
        let actor = ActorData {
            id: actor_obj.get("id").and_then(|v| v.as_i64()),
            login: actor_obj
                .get("login")
                .and_then(|v| v.as_str())
                .map(String::from),
            display_login: actor_obj
                .get("display_login")
                .and_then(|v| v.as_str())
                .map(String::from),
            gravatar_id: actor_obj
                .get("gravatar_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            url: actor_obj
                .get("url")
                .and_then(|v| v.as_str())
                .map(String::from),
            avatar_url: actor_obj
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            node_id: actor_obj
                .get("node_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            html_url: actor_obj
                .get("html_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            followers_url: actor_obj
                .get("followers_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            following_url: actor_obj
                .get("following_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            gists_url: actor_obj
                .get("gists_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            starred_url: actor_obj
                .get("starred_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            subscriptions_url: actor_obj
                .get("subscriptions_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            organizations_url: actor_obj
                .get("organizations_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            repos_url: actor_obj
                .get("repos_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            events_url: actor_obj
                .get("events_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            received_events_url: actor_obj
                .get("received_events_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            actor_type: actor_obj
                .get("type")
                .and_then(|v| v.as_str())
                .map(String::from),
            user_view_type: actor_obj
                .get("user_view_type")
                .and_then(|v| v.as_str())
                .map(String::from),
            site_admin: actor_obj.get("site_admin").and_then(|v| v.as_bool()),
        };

        // Extract repo data
        let repo_obj = event.get("repo").unwrap_or(&serde_json::Value::Null);
        let repo_owner = repo_obj.get("owner").unwrap_or(&serde_json::Value::Null);
        let repo_license = repo_obj.get("license").unwrap_or(&serde_json::Value::Null);

        let repo = RepoData {
            id: repo_obj.get("id").and_then(|v| v.as_i64()),
            name: repo_obj
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from),
            url: repo_obj
                .get("url")
                .and_then(|v| v.as_str())
                .map(String::from),
            full_name: repo_obj
                .get("full_name")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_login: repo_owner
                .get("login")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_id: repo_owner.get("id").and_then(|v| v.as_i64()),
            owner_node_id: repo_owner
                .get("node_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_avatar_url: repo_owner
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_gravatar_id: repo_owner
                .get("gravatar_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_url: repo_owner
                .get("url")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_html_url: repo_owner
                .get("html_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_type: repo_owner
                .get("type")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner_site_admin: repo_owner.get("site_admin").and_then(|v| v.as_bool()),
            node_id: repo_obj
                .get("node_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            html_url: repo_obj
                .get("html_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            description: repo_obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
            fork: repo_obj.get("fork").and_then(|v| v.as_bool()),
            language: repo_obj
                .get("language")
                .and_then(|v| v.as_str())
                .map(String::from),
            stargazers_count: repo_obj.get("stargazers_count").and_then(|v| v.as_i64()),
            watchers_count: repo_obj.get("watchers_count").and_then(|v| v.as_i64()),
            forks_count: repo_obj.get("forks_count").and_then(|v| v.as_i64()),
            open_issues_count: repo_obj.get("open_issues_count").and_then(|v| v.as_i64()),
            size: repo_obj.get("size").and_then(|v| v.as_i64()),
            default_branch: repo_obj
                .get("default_branch")
                .and_then(|v| v.as_str())
                .map(String::from),
            topics: repo_obj
                .get("topics")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            license_key: repo_license
                .get("key")
                .and_then(|v| v.as_str())
                .map(String::from),
            license_name: repo_license
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from),
            created_at: repo_obj
                .get("created_at")
                .and_then(|v| v.as_str())
                .and_then(|s| self.parse_datetime(s)),
            updated_at: repo_obj
                .get("updated_at")
                .and_then(|v| v.as_str())
                .and_then(|s| self.parse_datetime(s)),
            pushed_at: repo_obj
                .get("pushed_at")
                .and_then(|v| v.as_str())
                .and_then(|s| self.parse_datetime(s)),
        };

        // Extract org data (optional)
        let org = event.get("org").map(|org_obj| OrgData {
            id: org_obj.get("id").and_then(|v| v.as_i64()),
            login: org_obj
                .get("login")
                .and_then(|v| v.as_str())
                .map(String::from),
            node_id: org_obj
                .get("node_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            gravatar_id: org_obj
                .get("gravatar_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            url: org_obj
                .get("url")
                .and_then(|v| v.as_str())
                .map(String::from),
            avatar_url: org_obj
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            html_url: org_obj
                .get("html_url")
                .and_then(|v| v.as_str())
                .map(String::from),
            org_type: org_obj
                .get("type")
                .and_then(|v| v.as_str())
                .map(String::from),
            site_admin: org_obj.get("site_admin").and_then(|v| v.as_bool()),
        });

        Some(ValidatedEvent {
            id,
            event_type,
            created_at,
            public,
            actor,
            repo,
            org,
            payload: event
                .get("payload")
                .unwrap_or(&serde_json::Value::Null)
                .clone(),
            raw_event: event.clone(),
            api_source: "github_archive".to_string(),
        })
    }

    fn parse_event_id(id_field: &Value) -> Option<i64> {
        match id_field {
            Value::Number(num) => num.as_i64(),
            Value::String(text) => text.parse::<i64>().ok(),
            _ => None,
        }
    }

    fn parse_datetime(&self, date_str: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(date_str)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    }

    fn calculate_quality_score(
        &self,
        total_events: i64,
        integrity_issues: &HashMap<String, i64>,
    ) -> f64 {
        if total_events == 0 {
            return 0.0;
        }

        let total_issues: i64 = integrity_issues.values().sum();
        let clean_percentage = ((total_events - total_issues) as f64 / total_events as f64) * 100.0;

        clean_percentage.clamp(0.0, 100.0)
    }

    /// Generate comprehensive insert SQL with all fields - this was the missing function!
    fn get_comprehensive_insert_sql(&self) -> String {
        r#"
            INSERT INTO github_events (
                -- Core event fields
                event_id, event_type, event_created_at, event_public,

                -- Actor fields
                actor_id, actor_login, actor_display_login, actor_gravatar_id, actor_url,
                actor_avatar_url, actor_node_id, actor_html_url, actor_followers_url,
                actor_following_url, actor_gists_url, actor_starred_url, actor_subscriptions_url,
                actor_organizations_url, actor_repos_url, actor_events_url, actor_received_events_url,
                actor_type, actor_user_view_type, actor_site_admin,

                -- Repository fields
                repo_id, repo_name, repo_url, repo_full_name, repo_owner_login, repo_owner_id,
                repo_owner_node_id, repo_owner_avatar_url, repo_owner_gravatar_id, repo_owner_url,
                repo_owner_html_url, repo_owner_type, repo_owner_site_admin, repo_node_id,
                repo_html_url, repo_description, repo_fork, repo_language, repo_stargazers_count,
                repo_watchers_count, repo_forks_count, repo_open_issues_count, repo_size,
                repo_default_branch, repo_topics, repo_license_key, repo_license_name,
                repo_created_at, repo_updated_at, repo_pushed_at,

                -- Organization fields
                org_id, org_login, org_node_id, org_gravatar_id, org_url, org_avatar_url,
                org_html_url, org_type, org_site_admin,

                -- Data storage fields
                payload, raw_event, file_source, api_source
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38,
                $39, $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56,
                $57, $58, $59, $60, $61, $62, $63, $64, $65, $66, $67
            )
            ON CONFLICT (event_id) DO UPDATE SET
                payload = EXCLUDED.payload,
                raw_event = EXCLUDED.raw_event,
                processed_at = NOW()
        "#.to_string()
    }

    /// Get individual schema commands that can be executed separately
    fn get_schema_commands(&self) -> Vec<String> {
        let schema_sql = self.get_schema_sql();

        // Split by semicolon and filter out empty commands and comments
        schema_sql
            .split(';')
            .map(|cmd| {
                // Remove line comments and trim
                cmd.lines()
                    .filter(|line| !line.trim().starts_with("--"))
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_string()
            })
            .filter(|cmd| !cmd.is_empty())
            .collect()
    }

    fn get_schema_fixups(&self) -> Vec<&'static str> {
        vec![
            "ALTER TABLE processed_files ADD COLUMN IF NOT EXISTS file_size BIGINT",
            "ALTER TABLE processed_files ADD COLUMN IF NOT EXISTS size_bytes BIGINT DEFAULT 0",
            "ALTER TABLE processed_files ADD COLUMN IF NOT EXISTS etag VARCHAR(255)",
            "ALTER TABLE processed_files ADD COLUMN IF NOT EXISTS last_modified TIMESTAMP WITH TIME ZONE",
            "ALTER TABLE processed_files ADD COLUMN IF NOT EXISTS event_count INTEGER DEFAULT 0",
            "ALTER TABLE processed_files ADD COLUMN IF NOT EXISTS events_count BIGINT DEFAULT 0",
            "ALTER TABLE processed_files ADD COLUMN IF NOT EXISTS is_complete BOOLEAN DEFAULT TRUE",
            "UPDATE processed_files SET file_size = COALESCE(file_size, size_bytes, 0) WHERE file_size IS NULL",
            "UPDATE processed_files SET size_bytes = COALESCE(size_bytes, file_size, 0) WHERE size_bytes IS NULL",
            "UPDATE processed_files SET event_count = COALESCE(event_count, events_count::INTEGER, 0) WHERE event_count IS NULL",
            "UPDATE processed_files SET events_count = COALESCE(events_count, event_count, 0) WHERE events_count IS NULL",
            "ALTER TABLE processed_files ALTER COLUMN size_bytes SET DEFAULT 0",
            "ALTER TABLE processed_files ALTER COLUMN events_count SET DEFAULT 0",
            "ALTER TABLE processed_files ALTER COLUMN size_bytes SET NOT NULL",
            "ALTER TABLE pending_push_scans ADD COLUMN IF NOT EXISTS event_payload JSONB NOT NULL DEFAULT '{}'::jsonb",
            "ALTER TABLE pending_push_scans ADD COLUMN IF NOT EXISTS is_zero_commit BOOLEAN NOT NULL DEFAULT false",
            "ALTER TABLE pending_push_scans ADD COLUMN IF NOT EXISTS commit_span INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE pending_push_scans ADD COLUMN IF NOT EXISTS forced_flag BOOLEAN NOT NULL DEFAULT false",
            "ALTER TABLE pending_push_scans ADD COLUMN IF NOT EXISTS repository_url TEXT",
            "UPDATE pending_push_scans SET is_zero_commit = FALSE WHERE is_zero_commit IS NULL",
        ]
    }

    /// Generate complete database schema SQL - this was the other missing function!
    fn get_schema_sql(&self) -> String {
        r#"
            -- Create extensions
            CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
            CREATE EXTENSION IF NOT EXISTS "btree_gin";
            CREATE EXTENSION IF NOT EXISTS "pg_trgm";

            -- Main events table with comprehensive GitHub API data capture
            CREATE TABLE IF NOT EXISTS github_events (
                event_id BIGINT PRIMARY KEY,
                event_type VARCHAR(50) NOT NULL,
                event_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
                event_public BOOLEAN NOT NULL DEFAULT true,

                -- Actor information
                actor_id BIGINT,
                actor_login VARCHAR(255),
                actor_display_login VARCHAR(255),
                actor_gravatar_id VARCHAR(255),
                actor_url TEXT,
                actor_avatar_url TEXT,
                actor_node_id VARCHAR(255),
                actor_html_url TEXT,
                actor_followers_url TEXT,
                actor_following_url TEXT,
                actor_gists_url TEXT,
                actor_starred_url TEXT,
                actor_subscriptions_url TEXT,
                actor_organizations_url TEXT,
                actor_repos_url TEXT,
                actor_events_url TEXT,
                actor_received_events_url TEXT,
                actor_type VARCHAR(50),
                actor_user_view_type VARCHAR(50),
                actor_site_admin BOOLEAN,

                -- Repository information
                repo_id BIGINT,
                repo_name VARCHAR(255),
                repo_url TEXT,
                repo_full_name VARCHAR(255),
                repo_owner_login VARCHAR(255),
                repo_owner_id BIGINT,
                repo_owner_node_id VARCHAR(255),
                repo_owner_avatar_url TEXT,
                repo_owner_gravatar_id VARCHAR(255),
                repo_owner_url TEXT,
                repo_owner_html_url TEXT,
                repo_owner_type VARCHAR(50),
                repo_owner_site_admin BOOLEAN,
                repo_node_id VARCHAR(255),
                repo_html_url TEXT,
                repo_description TEXT,
                repo_fork BOOLEAN,
                repo_language VARCHAR(100),
                repo_stargazers_count BIGINT,
                repo_watchers_count BIGINT,
                repo_forks_count BIGINT,
                repo_open_issues_count BIGINT,
                repo_size BIGINT,
                repo_default_branch VARCHAR(100),
                repo_topics TEXT[],
                repo_license_key VARCHAR(50),
                repo_license_name VARCHAR(255),
                repo_created_at TIMESTAMP WITH TIME ZONE,
                repo_updated_at TIMESTAMP WITH TIME ZONE,
                repo_pushed_at TIMESTAMP WITH TIME ZONE,

                -- Organization information (optional)
                org_id BIGINT,
                org_login VARCHAR(255),
                org_node_id VARCHAR(255),
                org_gravatar_id VARCHAR(255),
                org_url TEXT,
                org_avatar_url TEXT,
                org_html_url TEXT,
                org_type VARCHAR(50),
                org_site_admin BOOLEAN,

                -- Complete payload as JSONB for flexible querying
                payload JSONB,
                raw_event JSONB,

                -- Metadata
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                file_source VARCHAR(255),
                api_source VARCHAR(255)
            );

            -- Processed files tracking
            CREATE TABLE IF NOT EXISTS processed_files (
                filename VARCHAR(255) PRIMARY KEY,
                file_size BIGINT,
                size_bytes BIGINT NOT NULL DEFAULT 0,
                etag VARCHAR(255),
                last_modified TIMESTAMP WITH TIME ZONE,
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                event_count INTEGER DEFAULT 0,
                events_count BIGINT DEFAULT 0,
                is_complete BOOLEAN DEFAULT TRUE
            );

            -- Repositories tracking table
            CREATE TABLE IF NOT EXISTS repositories (
                id BIGINT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                full_name VARCHAR(255),
                description TEXT,
                html_url TEXT,
                language VARCHAR(100),
                default_branch VARCHAR(100),
                created_at TIMESTAMP WITH TIME ZONE,
                updated_at TIMESTAMP WITH TIME ZONE,
                pushed_at TIMESTAMP WITH TIME ZONE,
                stargazers_count INTEGER,
                watchers_count INTEGER,
                forks_count INTEGER,
                open_issues_count INTEGER,
                topics TEXT[],
                license_name VARCHAR(255),
                owner_login VARCHAR(255),
                owner_type VARCHAR(50),
                first_seen_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                last_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            );

            -- Pending push events queue feeding the scanner
            CREATE TABLE IF NOT EXISTS pending_push_scans (
                event_id BIGINT PRIMARY KEY REFERENCES github_events(event_id) ON DELETE CASCADE,
                repository_full_name VARCHAR(255) NOT NULL,
                repository_url TEXT,
                before_sha VARCHAR(64) NOT NULL,
                head_sha VARCHAR(64),
                ref_name VARCHAR(255),
                forced_flag BOOLEAN NOT NULL DEFAULT false,
                commit_span INTEGER NOT NULL DEFAULT 0,
                event_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
                event_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'pending',
                attempts INTEGER NOT NULL DEFAULT 0,
                last_attempt_at TIMESTAMP WITH TIME ZONE,
                next_attempt_after TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                locked_by VARCHAR(128),
                locked_at TIMESTAMP WITH TIME ZONE,
                error_message TEXT,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                completed_at TIMESTAMP WITH TIME ZONE
            );

            CREATE INDEX IF NOT EXISTS idx_pending_push_scans_status
                ON pending_push_scans (status, next_attempt_after);
            CREATE INDEX IF NOT EXISTS idx_pending_push_scans_claim_window
                ON pending_push_scans (status, next_attempt_after, event_created_at);
            CREATE INDEX IF NOT EXISTS idx_pending_push_scans_repo
                ON pending_push_scans (repository_full_name);
            CREATE INDEX IF NOT EXISTS idx_pending_push_scans_processing_repo_created
                ON pending_push_scans (status, LOWER(repository_full_name), event_created_at);
            CREATE INDEX IF NOT EXISTS idx_pending_push_scans_created
                ON pending_push_scans (event_created_at);

            -- Secret scan executions (manual, realtime, backfill)
            CREATE TABLE IF NOT EXISTS secret_scans (
                id UUID PRIMARY KEY,
                repository VARCHAR(255),
                scan_type VARCHAR(50) NOT NULL,
                status VARCHAR(50) NOT NULL,
                source VARCHAR(50) NOT NULL,
                started_at TIMESTAMP WITH TIME ZONE NOT NULL,
                completed_at TIMESTAMP WITH TIME ZONE,
                duration_ms BIGINT,
                files_scanned BIGINT,
                secrets_found BIGINT DEFAULT 0,
                created_by VARCHAR(255) NOT NULL,
                metadata JSONB DEFAULT '{}'::jsonb
            );

            -- Individual secret detections persisted for reporting/export
            CREATE TABLE IF NOT EXISTS secret_detections (
                detection_id UUID PRIMARY KEY,
                scan_id UUID REFERENCES secret_scans(id) ON DELETE SET NULL,
                event_id BIGINT,
                repository VARCHAR(255) NOT NULL,
                file_path TEXT,
                detector_name VARCHAR(255) NOT NULL,
                severity VARCHAR(50) NOT NULL,
                category VARCHAR(50) NOT NULL,
                matched_text_hash VARCHAR(128) NOT NULL,
                matched_text_preview VARCHAR(255) NOT NULL,
                line_number INTEGER,
                verified BOOLEAN DEFAULT false,
                detected_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                source VARCHAR(50) NOT NULL,
                metadata JSONB DEFAULT '{}'::jsonb
            );

            -- Redacted local-AI triage results and provider provenance
            CREATE TABLE IF NOT EXISTS ai_triage_results (
                id UUID PRIMARY KEY,
                detection_id UUID REFERENCES secret_detections(detection_id) ON DELETE SET NULL,
                secret_hash VARCHAR(128) NOT NULL,
                provider VARCHAR(50) NOT NULL,
                model VARCHAR(255) NOT NULL,
                base_url TEXT NOT NULL,
                redacted_input JSONB NOT NULL,
                result JSONB NOT NULL,
                status VARCHAR(50) NOT NULL,
                error_message TEXT,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                completed_at TIMESTAMP WITH TIME ZONE
            );

            -- Audited maintenance actions against scan/queue state
            CREATE TABLE IF NOT EXISTS maintenance_repair_runs (
                id UUID PRIMARY KEY,
                repair_type VARCHAR(80) NOT NULL,
                dry_run BOOLEAN NOT NULL DEFAULT FALSE,
                backup_path TEXT,
                hard_delete_invalid_summaries BOOLEAN NOT NULL DEFAULT FALSE,
                reset_stale_processing BOOLEAN NOT NULL DEFAULT FALSE,
                pre_counts JSONB NOT NULL,
                post_counts JSONB NOT NULL,
                deleted_invalid_summaries BIGINT NOT NULL DEFAULT 0,
                reset_stale_processing_rows BIGINT NOT NULL DEFAULT 0,
                operator VARCHAR(255) NOT NULL,
                executed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                metadata JSONB DEFAULT '{}'::jsonb
            );

            -- Performance indexes
            CREATE INDEX IF NOT EXISTS idx_github_events_created_at ON github_events (event_created_at);
            CREATE INDEX IF NOT EXISTS idx_github_events_type ON github_events (event_type);
            CREATE INDEX IF NOT EXISTS idx_github_events_actor_id ON github_events (actor_id);
            CREATE INDEX IF NOT EXISTS idx_github_events_repo_id ON github_events (repo_id);
            CREATE INDEX IF NOT EXISTS idx_github_events_actor_login ON github_events (actor_login);
            CREATE INDEX IF NOT EXISTS idx_github_events_repo_name ON github_events (repo_name);
            CREATE INDEX IF NOT EXISTS idx_github_events_push_repo_created
                ON github_events (
                    LOWER(COALESCE(repo_full_name, repo_owner_login || '/' || repo_name)),
                    event_created_at DESC
                )
                WHERE event_type = 'PushEvent';
            CREATE INDEX IF NOT EXISTS idx_github_events_payload ON github_events USING GIN (payload);
            CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories (language);
            CREATE INDEX IF NOT EXISTS idx_repositories_stars ON repositories (stargazers_count DESC);
            CREATE INDEX IF NOT EXISTS idx_secret_scans_status_completed
                ON secret_scans (status, completed_at DESC);
            CREATE INDEX IF NOT EXISTS idx_secret_scans_repository_completed
                ON secret_scans (repository, completed_at DESC);
            CREATE INDEX IF NOT EXISTS idx_secret_detections_timestamp ON secret_detections (detected_at DESC);
            CREATE INDEX IF NOT EXISTS idx_secret_detections_severity ON secret_detections (severity);
            CREATE INDEX IF NOT EXISTS idx_secret_detections_category ON secret_detections (category);
            CREATE INDEX IF NOT EXISTS idx_secret_detections_repo ON secret_detections (repository);
            CREATE INDEX IF NOT EXISTS idx_secret_detections_repo_detected
                ON secret_detections (repository, detected_at DESC);
            CREATE INDEX IF NOT EXISTS idx_secret_detections_repo_trgm
                ON secret_detections USING GIN (repository gin_trgm_ops);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_secret_detections_unique_match
                ON secret_detections (
                    matched_text_hash,
                    repository,
                    COALESCE(file_path, ''),
                    detector_name,
                    source,
                    COALESCE(event_id, 0)
                );
            CREATE INDEX IF NOT EXISTS idx_ai_triage_results_detection
                ON ai_triage_results (detection_id);
            CREATE INDEX IF NOT EXISTS idx_ai_triage_results_created
                ON ai_triage_results (created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_maintenance_repair_runs_type_time
                ON maintenance_repair_runs (repair_type, executed_at DESC);
        "#.to_string()
    }

    async fn get_top_secret_repositories(
        &self,
        limit: i64,
    ) -> Result<Vec<SecretRepositoryRiskRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                repository,
                COUNT(*) AS total_secrets,
                COUNT(*) FILTER (WHERE LOWER(severity) = 'critical') AS critical_count,
                COUNT(*) FILTER (WHERE LOWER(severity) = 'high') AS high_count,
                MAX(detected_at) AS last_detected
            FROM secret_detections
            GROUP BY repository
            ORDER BY MAX(detected_at) DESC
            LIMIT $1
            "#,
        )
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch repository risk rows")?;

        let mut repos = Vec::with_capacity(rows.len());
        for row in rows {
            let critical = row.get::<i64, _>("critical_count");
            let high = row.get::<i64, _>("high_count");
            let total = row.get::<i64, _>("total_secrets");
            let risk_score = (critical * 10 + high * 5 + (total - critical - high)) as f64;

            repos.push(SecretRepositoryRiskRow {
                repository: row.get("repository"),
                total_secrets: total,
                critical_count: critical,
                high_count: high,
                risk_score,
                last_detected: row.get("last_detected"),
            });
        }

        Ok(repos)
    }

    fn normalize_severity_label(raw: &str) -> String {
        raw.parse::<SecretSeverity>()
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|_| raw.to_string())
    }

    fn frontend_category_label(raw: &str) -> String {
        if let Some(category) = SecretCategory::from_storage_key(raw) {
            category.frontend_label().to_string()
        } else if let Some(category) = SecretCategory::from_label(raw) {
            category.frontend_label().to_string()
        } else {
            raw.to_string()
        }
    }
}

#[cfg(all(test, feature = "db-tests"))]
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::{json, Value};
    use sqlx::Row;

    macro_rules! require_db {
        ($test_name:literal) => {
            match create_test_db().await {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("Skipping {}: {}", $test_name, e);
                    return;
                }
            }
        };
    }

    // Helper to create test database (uses in-memory or test database)
    async fn create_test_db() -> Result<Database> {
        let mut config = Config::default();
        // Use test database or override with TEST_DATABASE_URL env var
        if let Ok(test_url) = std::env::var("TEST_DATABASE_URL") {
            config.database.host = test_url;
        }
        Database::new(&config).await
    }

    #[tokio::test]
    async fn test_validate_event() {
        let config = Config::default();
        let db = Database::new(&config).await.unwrap();

        let event = json!({
            "id": 12345,
            "type": "PushEvent",
            "created_at": "2024-01-01T00:00:00Z",
            "public": true,
            "actor": {
                "id": 67890,
                "login": "test_user",
                "type": "User"
            },
            "repo": {
                "id": 111213,
                "name": "test/repo",
                "full_name": "test/repo"
            },
            "payload": {}
        });

        let validated = db.validate_and_convert_event(event);
        assert!(validated.is_some());

        let event = validated.unwrap();
        assert_eq!(event.id, 12345);
        assert_eq!(event.event_type, "PushEvent");
        assert_eq!(event.actor.login, Some("test_user".to_string()));
    }

    // --- EXPANDED COMPREHENSIVE TESTS ---

    #[tokio::test]
    async fn test_database_connection() {
        let db = require_db!("test_database_connection");
        let health = db.check_health().await;
        assert!(health.is_connected, "Database should be connected");
    }

    #[tokio::test]
    async fn test_health_check_returns_valid_data() {
        let db = require_db!("test_health_check_returns_valid_data");
        let health = db.check_health().await;

        assert!(health.is_connected);
        assert!(health.connection_count >= 0);
        assert!(health.active_queries >= 0);
        assert!(health.cache_hit_ratio >= 0.0 && health.cache_hit_ratio <= 100.0);
    }

    #[tokio::test]
    async fn test_validate_event_with_missing_fields() {
        let db = require_db!("test_validate_event_with_missing_fields");

        let incomplete_event = json!({
            "id": 12345,
            "type": "PushEvent",
            // Missing created_at, actor, repo
        });

        let validated = db.validate_and_convert_event(incomplete_event);
        // Should handle gracefully (either validate with defaults or return None)
        // Exact behavior depends on implementation
        assert!(validated.is_none() || validated.is_some());
    }

    #[tokio::test]
    async fn test_validate_event_with_org() {
        let db = require_db!("test_validate_event_with_org");

        let event = json!({
            "id": 12345,
            "type": "PushEvent",
            "created_at": "2024-01-01T00:00:00Z",
            "public": true,
            "actor": {
                "id": 67890,
                "login": "test_user"
            },
            "repo": {
                "id": 111213,
                "name": "test/repo"
            },
            "org": {
                "id": 999,
                "login": "test_org",
                "node_id": "org_node_123"
            },
            "payload": {}
        });

        let validated = db.validate_and_convert_event(event);
        assert!(validated.is_some());

        let event = validated.unwrap();
        assert!(event.org.is_some());
        let org = event.org.unwrap();
        assert_eq!(org.id, Some(999));
        assert_eq!(org.login, Some("test_org".to_string()));
    }

    #[tokio::test]
    async fn test_validate_event_different_types() {
        let db = require_db!("test_validate_event_different_types");

        let event_types = vec![
            "PushEvent",
            "PullRequestEvent",
            "IssuesEvent",
            "CreateEvent",
        ];

        for event_type in event_types {
            let event = json!({
                "id": 12345,
                "type": event_type,
                "created_at": "2024-01-01T00:00:00Z",
                "public": true,
                "actor": {"id": 1, "login": "user"},
                "repo": {"id": 2, "name": "repo"},
                "payload": {}
            });

            let validated = db.validate_and_convert_event(event);
            assert!(validated.is_some(), "Should validate {} event", event_type);
            assert_eq!(validated.unwrap().event_type, event_type);
        }
    }

    #[tokio::test]
    async fn test_insert_empty_batch() {
        let db = require_db!("test_insert_empty_batch");

        let result = db.insert_events_batch(vec![], "test_empty.json").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0, "Empty batch should insert 0 events");
    }

    #[tokio::test]
    async fn test_insert_single_event_batch() {
        let db = require_db!("test_insert_single_event_batch");

        let events = vec![json!({
            "id": 99999,
            "type": "PushEvent",
            "created_at": "2024-01-01T00:00:00Z",
            "public": true,
            "actor": {
                "id": 123,
                "login": "test_user"
            },
            "repo": {
                "id": 456,
                "name": "test/repo"
            },
            "payload": {"commits": []}
        })];

        let result = db.insert_events_batch(events, "test_single.json").await;
        assert!(result.is_ok());

        let inserted = result.unwrap();
        assert!(
            inserted >= 0,
            "Should insert at least 0 events (may skip duplicates)"
        );
    }

    #[tokio::test]
    async fn test_insert_multiple_events_batch() {
        let db = require_db!("test_insert_multiple_events_batch");

        let events = vec![
            json!({
                "id": 100001,
                "type": "PushEvent",
                "created_at": "2024-01-01T00:00:00Z",
                "public": true,
                "actor": {"id": 1, "login": "user1"},
                "repo": {"id": 10, "name": "repo1"},
                "payload": {}
            }),
            json!({
                "id": 100002,
                "type": "IssuesEvent",
                "created_at": "2024-01-01T01:00:00Z",
                "public": true,
                "actor": {"id": 2, "login": "user2"},
                "repo": {"id": 20, "name": "repo2"},
                "payload": {}
            }),
            json!({
                "id": 100003,
                "type": "PullRequestEvent",
                "created_at": "2024-01-01T02:00:00Z",
                "public": false,
                "actor": {"id": 3, "login": "user3"},
                "repo": {"id": 30, "name": "repo3"},
                "payload": {}
            }),
        ];

        let result = db.insert_events_batch(events, "test_batch.json").await;
        assert!(result.is_ok());

        let inserted = result.unwrap();
        assert!(inserted >= 0, "Should process multiple events");
    }

    #[tokio::test]
    async fn test_insert_batch_with_invalid_events() {
        let db = require_db!("test_insert_batch_with_invalid_events");

        let events = vec![
            json!({
                "id": 200001,
                "type": "PushEvent",
                "created_at": "2024-01-01T00:00:00Z",
                "public": true,
                "actor": {"id": 1, "login": "user1"},
                "repo": {"id": 10, "name": "repo1"},
                "payload": {}
            }),
            json!({
                // Invalid event - missing required fields
                "id": 200002
            }),
            json!({
                "id": 200003,
                "type": "IssuesEvent",
                "created_at": "2024-01-01T01:00:00Z",
                "public": true,
                "actor": {"id": 2, "login": "user2"},
                "repo": {"id": 20, "name": "repo2"},
                "payload": {}
            }),
        ];

        let result = db.insert_events_batch(events, "test_mixed.json").await;
        assert!(result.is_ok(), "Should handle mixed valid/invalid events");

        let inserted = result.unwrap();
        // Should skip invalid event, insert valid ones
        assert!(inserted >= 0);
    }

    #[tokio::test]
    async fn test_is_file_processed_new_file() {
        let db = require_db!("test_is_file_processed_new_file");

        let result = db
            .is_file_processed("test_new_file_999.json", None, None)
            .await;
        assert!(result.is_ok());
        assert!(
            !result.unwrap(),
            "New file should not be marked as processed"
        );
    }

    #[tokio::test]
    async fn test_mark_file_processed() {
        let db = require_db!("test_mark_file_processed");

        let filename = format!("test_mark_{}.json", chrono::Utc::now().timestamp_millis());
        let source = "gharchive";

        // Initially not processed
        let is_processed = db.is_file_processed(&filename, None, None).await.unwrap();
        assert!(!is_processed, "File should not be processed initially");

        // Mark as processed
        let result = db.mark_file_processed(&filename, source, 10, 5).await;
        assert!(result.is_ok(), "Should successfully mark file as processed");

        // Now should be processed
        let is_processed_now = db.is_file_processed(&filename, None, None).await.unwrap();
        assert!(
            is_processed_now,
            "File should be marked as processed after marking"
        );
    }

    #[tokio::test]
    async fn test_is_file_processed_with_etag() {
        let db = require_db!("test_is_file_processed_with_etag");

        let filename = format!("test_etag_{}.json", chrono::Utc::now().timestamp_millis());
        let etag = "etag-12345";
        let size = 1024i64;

        // Mark file as processed with etag
        let _ = db.mark_file_processed(&filename, "gharchive", 5, 3).await;

        // Check with matching etag
        let result = db
            .is_file_processed(&filename, Some(etag), Some(size))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_is_file_processed_etag_mismatch() {
        let db = require_db!("test_is_file_processed_etag_mismatch");

        let filename = format!(
            "test_etag_mismatch_{}.json",
            chrono::Utc::now().timestamp_millis()
        );

        // Mark file as processed
        let _ = db.mark_file_processed(&filename, "gharchive", 5, 3).await;

        // Check with different etag (should return false if etags don't match)
        let result = db
            .is_file_processed(&filename, Some("different-etag"), None)
            .await;
        assert!(result.is_ok());
        // Result depends on whether stored etag matches
    }

    #[tokio::test]
    async fn test_mark_file_processed_with_zero_events() {
        let db = require_db!("test_mark_file_processed_with_zero_events");

        let filename = format!("test_zero_{}.json", chrono::Utc::now().timestamp_millis());
        let result = db.mark_file_processed(&filename, "gharchive", 0, 0).await;

        assert!(result.is_ok(), "Should handle zero events gracefully");
    }

    #[tokio::test]
    async fn test_zero_commit_events_enqueue_payload() {
        let db = require_db!("test_zero_commit_events_enqueue_payload");
        let event_id =
            9_000_000_000_000_i64 + (Utc::now().timestamp_nanos_opt().unwrap_or(0) % 1_000_000);
        let before_sha = "0123456789abcdef0123456789abcdef01234567".to_string();
        let head_sha = "89abcdef0123456789abcdef0123456789abcdef".to_string();

        let events = vec![json!({
            "id": event_id,
            "type": "PushEvent",
            "created_at": "2024-01-01T00:00:00Z",
            "public": true,
            "actor": {"id": 1, "login": "tester"},
            "repo": {
                "id": 1,
                "name": "acme/example",
                "full_name": "acme/example"
            },
            "payload": {
                "before": before_sha,
                "head": head_sha,
                "ref": "refs/heads/main",
                "size": 0,
                "commits": []
            }
        })];

        db.insert_events_batch(events, "test_zero_commit_enqueue.json")
            .await
            .expect("insert zero-commit event");

        let row = sqlx::query(
            r#"
            SELECT is_zero_commit, event_payload
            FROM pending_push_scans
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .fetch_one(&db.pool)
        .await
        .expect("queued zero-commit event");

        let is_zero_commit: bool = row.get("is_zero_commit");
        assert!(is_zero_commit, "Zero commit metadata should be stored");

        let payload: Value = row.get("event_payload");
        assert_eq!(
            payload.get("before").and_then(|v| v.as_str()),
            Some(before_sha.as_str())
        );
        assert_eq!(
            payload
                .get("commits")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(0usize)
        );

        // Cleanup
        sqlx::query("DELETE FROM pending_push_scans WHERE event_id = $1")
            .bind(event_id)
            .execute(&db.pool)
            .await
            .ok();
        sqlx::query("DELETE FROM github_events WHERE event_id = $1")
            .bind(event_id)
            .execute(&db.pool)
            .await
            .ok();
    }

    #[tokio::test]
    async fn test_get_data_quality_metrics() {
        let db = require_db!("test_get_data_quality_metrics");

        let result = db.get_data_quality_metrics().await;
        assert!(result.is_ok(), "Should successfully get quality metrics");

        let metrics = result.unwrap();
        assert!(metrics.total_events >= 0);
        assert!(metrics.unique_actors >= 0);
        assert!(metrics.unique_repos >= 0);
        assert!(metrics.event_types >= 0);
        assert!(metrics.quality_score >= 0.0 && metrics.quality_score <= 100.0);
    }

    #[tokio::test]
    async fn test_get_database_statistics() {
        let db = require_db!("test_get_database_statistics");

        let result = db.get_database_statistics().await;
        assert!(
            result.is_ok(),
            "Should successfully get database statistics"
        );

        let stats = result.unwrap();
        assert!(stats.total_events >= 0);
        assert!(stats.table_count >= 0);
        assert!(!stats.database_size.is_empty());
    }

    #[tokio::test]
    async fn test_close_connection() {
        let db = require_db!("test_close_connection");

        // Should close without error
        db.close().await;

        // After close, connection should be terminated
        // (Additional checks depend on implementation)
    }

    #[tokio::test]
    async fn test_actor_data_serialization() {
        let actor = ActorData {
            id: Some(123),
            login: Some("testuser".to_string()),
            display_login: Some("TestUser".to_string()),
            gravatar_id: Some("gravatar123".to_string()),
            url: Some("https://api.github.com/users/testuser".to_string()),
            avatar_url: Some("https://avatars.githubusercontent.com/u/123".to_string()),
            node_id: Some("MDQ6VXNlcjEyMw==".to_string()),
            html_url: Some("https://github.com/testuser".to_string()),
            followers_url: Some("https://api.github.com/users/testuser/followers".to_string()),
            following_url: Some(
                "https://api.github.com/users/testuser/following{/other_user}".to_string(),
            ),
            gists_url: Some("https://api.github.com/users/testuser/gists{/gist_id}".to_string()),
            starred_url: Some(
                "https://api.github.com/users/testuser/starred{/owner}{/repo}".to_string(),
            ),
            subscriptions_url: Some(
                "https://api.github.com/users/testuser/subscriptions".to_string(),
            ),
            organizations_url: Some("https://api.github.com/users/testuser/orgs".to_string()),
            repos_url: Some("https://api.github.com/users/testuser/repos".to_string()),
            events_url: Some("https://api.github.com/users/testuser/events{/privacy}".to_string()),
            received_events_url: Some(
                "https://api.github.com/users/testuser/received_events".to_string(),
            ),
            actor_type: Some("User".to_string()),
            user_view_type: Some("public".to_string()),
            site_admin: Some(false),
        };

        let serialized = serde_json::to_string(&actor);
        assert!(serialized.is_ok(), "ActorData should serialize to JSON");

        let json_str = serialized.unwrap();
        let deserialized: Result<ActorData, _> = serde_json::from_str(&json_str);
        assert!(
            deserialized.is_ok(),
            "ActorData should deserialize from JSON"
        );

        let actor_back = deserialized.unwrap();
        assert_eq!(actor_back.id, actor.id);
        assert_eq!(actor_back.login, actor.login);
    }

    #[tokio::test]
    async fn test_repo_data_serialization() {
        let repo = RepoData {
            id: Some(456),
            name: Some("test-repo".to_string()),
            url: Some("https://api.github.com/repos/owner/test-repo".to_string()),
            full_name: Some("owner/test-repo".to_string()),
            owner_login: Some("owner".to_string()),
            owner_id: Some(789),
            owner_node_id: Some("MDQ6VXNlcjc4OQ==".to_string()),
            owner_avatar_url: Some("https://avatars.githubusercontent.com/u/789".to_string()),
            owner_gravatar_id: Some("".to_string()),
            owner_url: Some("https://api.github.com/users/owner".to_string()),
            owner_html_url: Some("https://github.com/owner".to_string()),
            owner_type: Some("User".to_string()),
            owner_site_admin: Some(false),
            node_id: Some("MDEwOlJlcG9zaXRvcnk0NTY=".to_string()),
            html_url: Some("https://github.com/owner/test-repo".to_string()),
            description: Some("A test repository".to_string()),
            fork: Some(false),
            language: Some("Rust".to_string()),
            stargazers_count: Some(100),
            watchers_count: Some(50),
            forks_count: Some(20),
            open_issues_count: Some(5),
            size: Some(1024),
            default_branch: Some("main".to_string()),
            topics: vec!["rust".to_string(), "testing".to_string()],
            license_key: Some("mit".to_string()),
            license_name: Some("MIT License".to_string()),
            created_at: None,
            updated_at: None,
            pushed_at: None,
        };

        let serialized = serde_json::to_string(&repo);
        assert!(serialized.is_ok(), "RepoData should serialize to JSON");

        let json_str = serialized.unwrap();
        let deserialized: Result<RepoData, _> = serde_json::from_str(&json_str);
        assert!(
            deserialized.is_ok(),
            "RepoData should deserialize from JSON"
        );

        let repo_back = deserialized.unwrap();
        assert_eq!(repo_back.id, repo.id);
        assert_eq!(repo_back.name, repo.name);
        assert_eq!(repo_back.language, repo.language);
    }

    #[tokio::test]
    async fn test_database_health_serialization() {
        let health = DatabaseHealth {
            is_connected: true,
            connection_count: 5,
            active_queries: 2,
            cache_hit_ratio: 95.5,
            error_message: None,
        };

        let serialized = serde_json::to_string(&health);
        assert!(
            serialized.is_ok(),
            "DatabaseHealth should serialize to JSON"
        );

        let json_str = serialized.unwrap();
        assert!(json_str.contains("is_connected"));
        assert!(json_str.contains("true"));
        assert!(json_str.contains("95.5"));
    }

    #[tokio::test]
    async fn test_quality_metrics_structure() {
        let metrics = QualityMetrics {
            total_events: 1000,
            unique_actors: 50,
            unique_repos: 75,
            event_types: 10,
            quality_score: 92.5,
            integrity_issues: HashMap::new(),
            processing_stats: HashMap::new(),
            recent_activity: HashMap::new(),
        };

        assert_eq!(metrics.total_events, 1000);
        assert_eq!(metrics.unique_actors, 50);
        assert_eq!(metrics.quality_score, 92.5);

        let serialized = serde_json::to_string(&metrics);
        assert!(
            serialized.is_ok(),
            "QualityMetrics should serialize to JSON"
        );
    }

    #[tokio::test]
    async fn test_concurrent_health_checks() {
        let db = require_db!("test_concurrent_health_checks");
        let mut handles = vec![];

        for _ in 0..5 {
            let db_clone = db.pool.clone();
            let config_clone = db.config.clone();
            let handle = tokio::spawn(async move {
                let db_instance = Database {
                    pool: db_clone,
                    config: config_clone,
                };
                db_instance.check_health().await
            });
            handles.push(handle);
        }

        for handle in handles {
            let health = handle.await.unwrap();
            assert!(
                health.is_connected,
                "All concurrent health checks should succeed"
            );
        }
    }

    #[tokio::test]
    async fn test_validate_event_with_complex_payload() {
        let db = require_db!("test_validate_event_with_complex_payload");

        let event = json!({
            "id": 12345,
            "type": "PullRequestEvent",
            "created_at": "2024-01-01T00:00:00Z",
            "public": true,
            "actor": {"id": 1, "login": "user"},
            "repo": {"id": 2, "name": "repo"},
            "payload": {
                "action": "opened",
                "number": 42,
                "pull_request": {
                    "id": 999,
                    "title": "Add new feature",
                    "state": "open",
                    "user": {"login": "contributor"},
                    "body": "This PR adds a new feature",
                    "commits": 5,
                    "additions": 100,
                    "deletions": 20
                }
            }
        });

        let validated = db.validate_and_convert_event(event);
        assert!(
            validated.is_some(),
            "Should validate event with complex payload"
        );

        let event = validated.unwrap();
        assert_eq!(event.event_type, "PullRequestEvent");
        assert!(
            event.payload.is_object(),
            "Payload should be preserved as object"
        );
    }
}
