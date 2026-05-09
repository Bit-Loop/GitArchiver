use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Pool, Postgres, Row};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::core::database::DatabaseHealth;
use crate::core::Config;
use crate::scraper::GitHubEvent;

#[derive(Debug, Clone, Serialize)]
pub struct QualityMetrics {
    pub total_events: u64,
    pub unique_actors: u64,
    pub unique_repos: u64,
    pub event_types: u64,
    pub quality_score: f64,
    pub integrity_issues: HashMap<String, u64>,
    pub processing_stats: HashMap<String, serde_json::Value>,
    pub recent_activity: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessedFile {
    pub filename: String,
    pub etag: Option<String>,
    pub size_bytes: u64,
    pub events_count: u64,
    pub processed_at: DateTime<Utc>,
    pub processing_time_seconds: f64,
}

pub struct DatabaseManager {
    pool: Option<Pool<Postgres>>,
    config: Config,
    connection_attempts: u32,
    max_connection_attempts: u32,
}

async fn fetch_active_query_count(pool: &Pool<Postgres>) -> Result<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*)::BIGINT as count
        FROM pg_stat_activity
        WHERE datname = current_database()
          AND state = 'active'
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(row.get("count"))
}

async fn fetch_cache_hit_ratio(pool: &Pool<Postgres>) -> Result<f64> {
    let row = sqlx::query(
        r#"
        SELECT COALESCE(
            (
                SUM(heap_blks_hit)::DOUBLE PRECISION
                / NULLIF(SUM(heap_blks_hit + heap_blks_read)::DOUBLE PRECISION, 0.0)
            ) * 100.0,
            0.0
        ) as ratio
        FROM pg_statio_user_tables
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(row.get("ratio"))
}

fn calculate_event_completeness_score(
    total_events: i64,
    events_with_actors: i64,
    events_with_repos: i64,
) -> f64 {
    if total_events <= 0 {
        return 0.0;
    }

    let complete_fields = events_with_actors.saturating_add(events_with_repos);
    let required_fields = total_events.saturating_mul(2);

    ((complete_fields as f64 / required_fields as f64) * 100.0).clamp(0.0, 100.0)
}

impl DatabaseManager {
    pub fn new(config: Config) -> Self {
        Self {
            pool: None,
            config,
            connection_attempts: 0,
            max_connection_attempts: 3,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to database...");

        let connection_string = self.build_connection_string();

        for attempt in 0..self.max_connection_attempts {
            match self.connect_attempt(&connection_string).await {
                Ok(pool) => {
                    self.pool = Some(pool);
                    self.verify_connection().await?;
                    self.initialize_schema().await?;
                    info!("Database connected successfully (attempt {})", attempt + 1);
                    return Ok(());
                }
                Err(e) => {
                    self.connection_attempts += 1;
                    error!("Database connection attempt {} failed: {}", attempt + 1, e);

                    if attempt < self.max_connection_attempts - 1 {
                        let delay = Duration::from_secs(2 * (attempt + 1) as u64);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(anyhow!(
            "Failed to connect to database after {} attempts",
            self.max_connection_attempts
        ))
    }

    async fn connect_attempt(&self, connection_string: &str) -> Result<Pool<Postgres>> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(self.config.database.min_connections)
            .max_connections(self.config.database.max_connections)
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect(connection_string)
            .await?;

        Ok(pool)
    }

    fn build_connection_string(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.config.database.user,
            self.config.database.password,
            self.config.database.host,
            self.config.database.port,
            self.config.database.name
        )
    }

    async fn verify_connection(&self) -> Result<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow!("No database connection"))?;

        let row = sqlx::query("SELECT 1 as test").fetch_one(pool).await?;

        let test_value: i32 = row.get("test");
        if test_value != 1 {
            return Err(anyhow!("Database connection verification failed"));
        }

        debug!("Database connection verified");
        Ok(())
    }

    async fn initialize_schema(&self) -> Result<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow!("No database connection"))?;

        info!("Initializing database schema...");

        // Create events table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id BIGSERIAL PRIMARY KEY,
                github_id VARCHAR(255) UNIQUE NOT NULL,
                event_type VARCHAR(100) NOT NULL,
                actor_id BIGINT,
                actor_login VARCHAR(255),
                repo_id BIGINT,
                repo_name VARCHAR(512),
                repo_url VARCHAR(512),
                payload JSONB,
                public BOOLEAN DEFAULT true,
                created_at TIMESTAMP WITH TIME ZONE,
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                source_file VARCHAR(255),
                raw_data JSONB
            )
        "#,
        )
        .execute(pool)
        .await?;

        // Create repositories table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS repositories (
                id BIGSERIAL PRIMARY KEY,
                github_id BIGINT UNIQUE NOT NULL,
                name VARCHAR(512) NOT NULL,
                full_name VARCHAR(512) NOT NULL,
                url VARCHAR(512),
                description TEXT,
                private BOOLEAN DEFAULT false,
                fork BOOLEAN DEFAULT false,
                created_at TIMESTAMP WITH TIME ZONE,
                updated_at TIMESTAMP WITH TIME ZONE,
                pushed_at TIMESTAMP WITH TIME ZONE,
                size_kb BIGINT DEFAULT 0,
                stargazers_count INTEGER DEFAULT 0,
                watchers_count INTEGER DEFAULT 0,
                language VARCHAR(100),
                forks_count INTEGER DEFAULT 0,
                archived BOOLEAN DEFAULT false,
                disabled BOOLEAN DEFAULT false,
                open_issues_count INTEGER DEFAULT 0,
                license VARCHAR(255),
                default_branch VARCHAR(255) DEFAULT 'main',
                first_seen TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                last_seen TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            )
        "#,
        )
        .execute(pool)
        .await?;

        // Create actors table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS actors (
                id BIGSERIAL PRIMARY KEY,
                github_id BIGINT UNIQUE NOT NULL,
                login VARCHAR(255) NOT NULL,
                display_login VARCHAR(255),
                gravatar_id VARCHAR(255),
                url VARCHAR(512),
                avatar_url VARCHAR(512),
                account_type VARCHAR(50) DEFAULT 'User',
                site_admin BOOLEAN DEFAULT false,
                first_seen TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                last_seen TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                event_count BIGINT DEFAULT 0
            )
        "#,
        )
        .execute(pool)
        .await?;

        // Create processed_files table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS processed_files (
                id BIGSERIAL PRIMARY KEY,
                filename VARCHAR(255) UNIQUE NOT NULL,
                etag VARCHAR(255),
                size_bytes BIGINT NOT NULL,
                events_count BIGINT DEFAULT 0,
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                processing_time_seconds DOUBLE PRECISION DEFAULT 0.0,
                status VARCHAR(50) DEFAULT 'completed',
                error_message TEXT
            )
        "#,
        )
        .execute(pool)
        .await?;

        // Create indexes for performance
        let indexes = vec![
            "CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type)",
            "CREATE INDEX IF NOT EXISTS idx_events_actor_id ON events(actor_id)",
            "CREATE INDEX IF NOT EXISTS idx_events_repo_id ON events(repo_id)",
            "CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at)",
            "CREATE INDEX IF NOT EXISTS idx_events_processed_at ON events(processed_at)",
            "CREATE INDEX IF NOT EXISTS idx_repositories_name ON repositories(name)",
            "CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories(language)",
            "CREATE INDEX IF NOT EXISTS idx_actors_login ON actors(login)",
            "CREATE INDEX IF NOT EXISTS idx_processed_files_filename ON processed_files(filename)",
        ];

        for index_sql in indexes {
            if let Err(e) = sqlx::query(index_sql).execute(pool).await {
                warn!("Failed to create index: {}", e);
            }
        }

        info!("Database schema initialized successfully");
        Ok(())
    }

    pub async fn insert_events_batch(
        &self,
        events: &[GitHubEvent],
        source_file: &str,
    ) -> Result<u64> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow!("No database connection"))?;

        if events.is_empty() {
            return Ok(0);
        }

        debug!(
            "Inserting batch of {} events from {}",
            events.len(),
            source_file
        );

        let mut tx = pool.begin().await?;
        let mut inserted_count = 0u64;

        for event in events {
            // Parse created_at timestamp
            let created_at = event
                .created_at
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            // Extract actor information
            let (actor_id, actor_login) = if let Some(actor) = &event.actor {
                let id = actor.get("id").and_then(|v| v.as_u64()).map(|id| id as i64);
                let login = actor
                    .get("login")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                (id, login)
            } else {
                (None, None)
            };

            // Extract repository information
            let (repo_id, repo_name, repo_url) = if let Some(repo) = &event.repo {
                let id = repo.get("id").and_then(|v| v.as_u64()).map(|id| id as i64);
                let name = repo
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let url = repo
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                (id, name, url)
            } else {
                (None, None, None)
            };

            // Insert event
            match sqlx::query(
                r#"
                INSERT INTO events (
                    github_id, event_type, actor_id, actor_login, repo_id, repo_name, repo_url,
                    payload, public, created_at, source_file, raw_data
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (github_id) DO NOTHING
            "#,
            )
            .bind(&event.id)
            .bind(&event.event_type)
            .bind(actor_id)
            .bind(actor_login)
            .bind(repo_id)
            .bind(repo_name)
            .bind(repo_url)
            .bind(event.payload.as_ref())
            .bind(event.public)
            .bind(created_at)
            .bind(source_file)
            .bind(serde_json::to_value(event)?)
            .execute(&mut *tx)
            .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        inserted_count += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to insert event {}: {}", event.id, e);
                }
            }
        }

        tx.commit().await?;
        debug!("Successfully inserted {} events", inserted_count);

        Ok(inserted_count)
    }

    pub async fn mark_file_processed(
        &self,
        filename: &str,
        etag: Option<&str>,
        size_bytes: u64,
        events_count: u64,
        processing_time: f64,
    ) -> Result<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow!("No database connection"))?;

        sqlx::query(
            r#"
            INSERT INTO processed_files (
                filename, etag, size_bytes, events_count, processing_time_seconds
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (filename) DO UPDATE SET
                etag = EXCLUDED.etag,
                size_bytes = EXCLUDED.size_bytes,
                events_count = EXCLUDED.events_count,
                processed_at = NOW(),
                processing_time_seconds = EXCLUDED.processing_time_seconds,
                status = 'completed'
        "#,
        )
        .bind(filename)
        .bind(etag)
        .bind(size_bytes as i64)
        .bind(events_count as i64)
        .bind(processing_time)
        .execute(pool)
        .await?;

        debug!("Marked file {} as processed", filename);
        Ok(())
    }

    pub async fn is_file_processed(&self, filename: &str, etag: Option<&str>) -> Result<bool> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow!("No database connection"))?;

        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM processed_files WHERE filename = $1 AND (etag = $2 OR $2 IS NULL)"
        )
        .bind(filename)
        .bind(etag)
        .fetch_one(pool)
        .await?;

        let count: i64 = row.get("count");
        Ok(count > 0)
    }

    pub async fn get_health_status(&self) -> Result<DatabaseHealth> {
        let Some(pool) = self.pool.as_ref() else {
            return Ok(DatabaseHealth {
                is_connected: false,
                connection_count: 0,
                active_queries: 0,
                cache_hit_ratio: 0.0,
                error_message: Some("No database connection".to_string()),
            });
        };

        let connection_count = i64::from(pool.size());

        if let Err(error) = sqlx::query("SELECT 1").fetch_one(pool).await {
            return Ok(DatabaseHealth {
                is_connected: false,
                connection_count,
                active_queries: 0,
                cache_hit_ratio: 0.0,
                error_message: Some(error.to_string()),
            });
        }

        let active_queries = match fetch_active_query_count(pool).await {
            Ok(count) => count,
            Err(error) => {
                warn!("Failed to collect active query count: {}", error);
                0
            }
        };
        let cache_hit_ratio = match fetch_cache_hit_ratio(pool).await {
            Ok(ratio) => ratio,
            Err(error) => {
                warn!("Failed to collect cache hit ratio: {}", error);
                0.0
            }
        };

        Ok(DatabaseHealth {
            is_connected: true,
            connection_count,
            active_queries,
            cache_hit_ratio,
            error_message: None,
        })
    }

    pub async fn get_quality_metrics(&self) -> Result<QualityMetrics> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow!("No database connection"))?;

        // Get total events
        let total_events_row = sqlx::query("SELECT COUNT(*) as count FROM events")
            .fetch_one(pool)
            .await?;
        let total_events: i64 = total_events_row.get("count");

        // Get unique actors
        let unique_actors_row = sqlx::query(
            "SELECT COUNT(DISTINCT actor_id) as count FROM events WHERE actor_id IS NOT NULL",
        )
        .fetch_one(pool)
        .await?;
        let unique_actors: i64 = unique_actors_row.get("count");

        // Get unique repositories
        let unique_repos_row = sqlx::query(
            "SELECT COUNT(DISTINCT repo_id) as count FROM events WHERE repo_id IS NOT NULL",
        )
        .fetch_one(pool)
        .await?;
        let unique_repos: i64 = unique_repos_row.get("count");

        // Get event types
        let event_types_row = sqlx::query("SELECT COUNT(DISTINCT event_type) as count FROM events")
            .fetch_one(pool)
            .await?;
        let event_types: i64 = event_types_row.get("count");

        let events_with_actors_row =
            sqlx::query("SELECT COUNT(*) as count FROM events WHERE actor_id IS NOT NULL")
                .fetch_one(pool)
                .await?;
        let events_with_actors: i64 = events_with_actors_row.get("count");

        let events_with_repos_row =
            sqlx::query("SELECT COUNT(*) as count FROM events WHERE repo_id IS NOT NULL")
                .fetch_one(pool)
                .await?;
        let events_with_repos: i64 = events_with_repos_row.get("count");

        let quality_score =
            calculate_event_completeness_score(total_events, events_with_actors, events_with_repos);

        let mut integrity_issues = HashMap::new();
        let null_actors = total_events.saturating_sub(events_with_actors);
        integrity_issues.insert("null_actors".to_string(), null_actors as u64);

        let null_repos = total_events.saturating_sub(events_with_repos);
        integrity_issues.insert("null_repos".to_string(), null_repos as u64);

        let processing_stats_row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_files_processed,
                COALESCE(SUM(events_count), 0)::BIGINT as total_events_processed,
                COALESCE(AVG(processing_time_seconds), 0.0)::DOUBLE PRECISION as avg_processing_time_seconds
            FROM processed_files
            "#,
        )
        .fetch_one(pool)
        .await?;
        let total_files_processed: i64 = processing_stats_row.get("total_files_processed");
        let total_events_processed: i64 = processing_stats_row.get("total_events_processed");
        let avg_processing_time_seconds: f64 =
            processing_stats_row.get("avg_processing_time_seconds");

        let mut processing_stats = HashMap::new();
        processing_stats.insert(
            "total_files_processed".to_string(),
            serde_json::json!(total_files_processed),
        );
        processing_stats.insert(
            "total_events_processed".to_string(),
            serde_json::json!(total_events_processed),
        );
        processing_stats.insert(
            "avg_processing_time_seconds".to_string(),
            serde_json::json!(avg_processing_time_seconds),
        );

        // Recent activity (last 24 hours)
        let mut recent_activity = HashMap::new();
        let recent_events_row = sqlx::query(
            "SELECT COUNT(*) as count FROM events WHERE processed_at > NOW() - INTERVAL '24 hours'",
        )
        .fetch_one(pool)
        .await?;
        let recent_events: i64 = recent_events_row.get("count");
        recent_activity.insert("events_24h".to_string(), recent_events as u64);

        Ok(QualityMetrics {
            total_events: total_events as u64,
            unique_actors: unique_actors as u64,
            unique_repos: unique_repos as u64,
            event_types: event_types as u64,
            quality_score,
            integrity_issues,
            processing_stats,
            recent_activity,
        })
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(pool) = &self.pool {
            pool.close().await;
            self.pool = None;
            info!("Database connection closed");
        }
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.pool.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_completeness_score_uses_present_event_fields_not_distinct_counts() {
        assert_eq!(calculate_event_completeness_score(0, 0, 0), 0.0);
        assert_eq!(calculate_event_completeness_score(10, 10, 10), 100.0);
        assert_eq!(calculate_event_completeness_score(10, 10, 5), 75.0);
        assert_eq!(calculate_event_completeness_score(10, 50, 50), 100.0);
    }

    #[test]
    fn disconnected_manager_reports_not_connected() {
        let manager = DatabaseManager::new(Config::default());

        assert!(!manager.is_connected());
    }

    #[tokio::test]
    async fn health_status_without_pool_is_explicitly_unhealthy() {
        let manager = DatabaseManager::new(Config::default());

        let status = manager.get_health_status().await.expect("health status");

        assert!(!status.is_connected);
        assert_eq!(status.connection_count, 0);
        assert_eq!(status.active_queries, 0);
        assert_eq!(status.cache_hit_ratio, 0.0);
        assert_eq!(
            status.error_message.as_deref(),
            Some("No database connection")
        );
    }
}
