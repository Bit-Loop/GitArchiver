use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::collections::HashMap;
use tracing::{error, info, warn};

use super::config::Config;

/// Database health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub is_connected: bool,
    pub connection_count: i64,
    pub active_queries: i64,
    pub cache_hit_ratio: f64,
    pub error_message: Option<String>,
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
    pub async fn new(config: Config) -> Result<Self> {
        let connection_string = &config.database.connection_string();
        let max_attempts = 3;

        for attempt in 1..=max_attempts {
            match PgPoolOptions::new()
                .min_connections(config.database.min_connections)
                .max_connections(config.database.max_connections)
                .acquire_timeout(std::time::Duration::from_secs(config.database.command_timeout))
                .connect(connection_string)
                .await
            {
                Ok(pool) => {
                    let db = Database { pool, config };
                    
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

    /// Initialize database schema if needed
    async fn initialize_schema(&self) -> Result<()> {
        let schema_commands = self.get_schema_commands();
        
        for command in schema_commands {
            if !command.trim().is_empty() {
                sqlx::query(&command)
                    .execute(&self.pool)
                    .await
                    .context("Failed to execute schema command")?;
            }
        }
        
        info!("Database schema initialized");
        Ok(())
    }

    /// Comprehensive database health check
    pub async fn health_check(&self) -> DatabaseHealth {
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
            Err(e) => DatabaseHealth {
                is_connected: false,
                connection_count: 0,
                active_queries: 0,
                cache_hit_ratio: 0.0,
                error_message: Some(e.to_string()),
            },
        }
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
                count(*) as active_connections,
                count(CASE WHEN state = 'active' THEN 1 END) as active_queries
            FROM pg_stat_activity 
            WHERE datname = current_database()
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get connection statistics")?;

        // Get cache hit ratio
        let cache_stats = sqlx::query(
            r#"
            SELECT 
                round(
                    100 * sum(blks_hit) / nullif(sum(blks_hit + blks_read), 0), 2
                ) as cache_hit_ratio
            FROM pg_stat_database 
            WHERE datname = current_database()
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get cache statistics")?;

        Ok(DatabaseHealth {
            is_connected: true,
            connection_count: stats.get::<i64, _>("active_connections"),
            active_queries: stats.get::<i64, _>("active_queries"),
            cache_hit_ratio: cache_stats
                .get::<Option<f64>, _>("cache_hit_ratio")
                .unwrap_or(0.0),
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

        let mut tx = self.pool.begin().await.context("Failed to start transaction")?;

        for event in validated_events {
            match self.insert_single_event(&mut tx, &insert_sql, &event, filename).await {
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

        Ok(())
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
                SUM(event_count) as total_processed_events,
                AVG(event_count) as avg_events_per_file,
                SUM(file_size) / (1024*1024*1024.0) as total_gb_processed
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
                COALESCE(SUM(event_count), 0) as events_processed_24h
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
        integrity_map.insert("null_ids".to_string(), integrity_issues.get::<i64, _>("null_ids"));
        integrity_map.insert("invalid_types".to_string(), integrity_issues.get::<i64, _>("invalid_types"));
        integrity_map.insert("null_timestamps".to_string(), integrity_issues.get::<i64, _>("null_timestamps"));
        integrity_map.insert("null_payloads".to_string(), integrity_issues.get::<i64, _>("null_payloads"));

        let mut processing_map = HashMap::new();
        processing_map.insert("total_files".to_string(), processing_stats.get::<Option<i64>, _>("total_files").unwrap_or(0) as f64);
        processing_map.insert("total_processed_events".to_string(), processing_stats.get::<Option<i64>, _>("total_processed_events").unwrap_or(0) as f64);
        processing_map.insert("avg_events_per_file".to_string(), processing_stats.get::<Option<f64>, _>("avg_events_per_file").unwrap_or(0.0));
        processing_map.insert("total_gb_processed".to_string(), processing_stats.get::<Option<f64>, _>("total_gb_processed").unwrap_or(0.0));

        let mut recent_map = HashMap::new();
        recent_map.insert("files_processed_24h".to_string(), recent_activity.get::<i64, _>("files_processed_24h"));
        recent_map.insert("events_processed_24h".to_string(), recent_activity.get::<i64, _>("events_processed_24h"));

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
            "SELECT etag, file_size FROM processed_files WHERE filename = $1"
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
                    let stored_size: Option<i64> = row.get("file_size");
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
            INSERT INTO processed_files (filename, etag, file_size, event_count)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (filename) 
            DO UPDATE SET 
                etag = $2, 
                file_size = $3, 
                event_count = $4, 
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

    // Private helper methods

    fn validate_and_convert_event(&self, event: serde_json::Value) -> Option<ValidatedEvent> {
        // Extract basic event data
        let id = event.get("id")?.as_i64()?;
        let event_type = event.get("type")?.as_str()?.to_string();
        let created_at = self.parse_datetime(event.get("created_at")?.as_str()?)?;
        let public = event.get("public").and_then(|v| v.as_bool()).unwrap_or(true);

        // Extract actor data
        let actor_obj = event.get("actor").unwrap_or(&serde_json::Value::Null);
        let actor = ActorData {
            id: actor_obj.get("id").and_then(|v| v.as_i64()),
            login: actor_obj.get("login").and_then(|v| v.as_str()).map(String::from),
            display_login: actor_obj.get("display_login").and_then(|v| v.as_str()).map(String::from),
            gravatar_id: actor_obj.get("gravatar_id").and_then(|v| v.as_str()).map(String::from),
            url: actor_obj.get("url").and_then(|v| v.as_str()).map(String::from),
            avatar_url: actor_obj.get("avatar_url").and_then(|v| v.as_str()).map(String::from),
            node_id: actor_obj.get("node_id").and_then(|v| v.as_str()).map(String::from),
            html_url: actor_obj.get("html_url").and_then(|v| v.as_str()).map(String::from),
            followers_url: actor_obj.get("followers_url").and_then(|v| v.as_str()).map(String::from),
            following_url: actor_obj.get("following_url").and_then(|v| v.as_str()).map(String::from),
            gists_url: actor_obj.get("gists_url").and_then(|v| v.as_str()).map(String::from),
            starred_url: actor_obj.get("starred_url").and_then(|v| v.as_str()).map(String::from),
            subscriptions_url: actor_obj.get("subscriptions_url").and_then(|v| v.as_str()).map(String::from),
            organizations_url: actor_obj.get("organizations_url").and_then(|v| v.as_str()).map(String::from),
            repos_url: actor_obj.get("repos_url").and_then(|v| v.as_str()).map(String::from),
            events_url: actor_obj.get("events_url").and_then(|v| v.as_str()).map(String::from),
            received_events_url: actor_obj.get("received_events_url").and_then(|v| v.as_str()).map(String::from),
            actor_type: actor_obj.get("type").and_then(|v| v.as_str()).map(String::from),
            user_view_type: actor_obj.get("user_view_type").and_then(|v| v.as_str()).map(String::from),
            site_admin: actor_obj.get("site_admin").and_then(|v| v.as_bool()),
        };

        // Extract repo data
        let repo_obj = event.get("repo").unwrap_or(&serde_json::Value::Null);
        let repo_owner = repo_obj.get("owner").unwrap_or(&serde_json::Value::Null);
        let repo_license = repo_obj.get("license").unwrap_or(&serde_json::Value::Null);
        
        let repo = RepoData {
            id: repo_obj.get("id").and_then(|v| v.as_i64()),
            name: repo_obj.get("name").and_then(|v| v.as_str()).map(String::from),
            url: repo_obj.get("url").and_then(|v| v.as_str()).map(String::from),
            full_name: repo_obj.get("full_name").and_then(|v| v.as_str()).map(String::from),
            owner_login: repo_owner.get("login").and_then(|v| v.as_str()).map(String::from),
            owner_id: repo_owner.get("id").and_then(|v| v.as_i64()),
            owner_node_id: repo_owner.get("node_id").and_then(|v| v.as_str()).map(String::from),
            owner_avatar_url: repo_owner.get("avatar_url").and_then(|v| v.as_str()).map(String::from),
            owner_gravatar_id: repo_owner.get("gravatar_id").and_then(|v| v.as_str()).map(String::from),
            owner_url: repo_owner.get("url").and_then(|v| v.as_str()).map(String::from),
            owner_html_url: repo_owner.get("html_url").and_then(|v| v.as_str()).map(String::from),
            owner_type: repo_owner.get("type").and_then(|v| v.as_str()).map(String::from),
            owner_site_admin: repo_owner.get("site_admin").and_then(|v| v.as_bool()),
            node_id: repo_obj.get("node_id").and_then(|v| v.as_str()).map(String::from),
            html_url: repo_obj.get("html_url").and_then(|v| v.as_str()).map(String::from),
            description: repo_obj.get("description").and_then(|v| v.as_str()).map(String::from),
            fork: repo_obj.get("fork").and_then(|v| v.as_bool()),
            language: repo_obj.get("language").and_then(|v| v.as_str()).map(String::from),
            stargazers_count: repo_obj.get("stargazers_count").and_then(|v| v.as_i64()),
            watchers_count: repo_obj.get("watchers_count").and_then(|v| v.as_i64()),
            forks_count: repo_obj.get("forks_count").and_then(|v| v.as_i64()),
            open_issues_count: repo_obj.get("open_issues_count").and_then(|v| v.as_i64()),
            size: repo_obj.get("size").and_then(|v| v.as_i64()),
            default_branch: repo_obj.get("default_branch").and_then(|v| v.as_str()).map(String::from),
            topics: repo_obj.get("topics")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|t| t.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            license_key: repo_license.get("key").and_then(|v| v.as_str()).map(String::from),
            license_name: repo_license.get("name").and_then(|v| v.as_str()).map(String::from),
            created_at: repo_obj.get("created_at").and_then(|v| v.as_str()).and_then(|s| self.parse_datetime(s)),
            updated_at: repo_obj.get("updated_at").and_then(|v| v.as_str()).and_then(|s| self.parse_datetime(s)),
            pushed_at: repo_obj.get("pushed_at").and_then(|v| v.as_str()).and_then(|s| self.parse_datetime(s)),
        };

        // Extract org data (optional)
        let org = event.get("org").map(|org_obj| OrgData {
            id: org_obj.get("id").and_then(|v| v.as_i64()),
            login: org_obj.get("login").and_then(|v| v.as_str()).map(String::from),
            node_id: org_obj.get("node_id").and_then(|v| v.as_str()).map(String::from),
            gravatar_id: org_obj.get("gravatar_id").and_then(|v| v.as_str()).map(String::from),
            url: org_obj.get("url").and_then(|v| v.as_str()).map(String::from),
            avatar_url: org_obj.get("avatar_url").and_then(|v| v.as_str()).map(String::from),
            html_url: org_obj.get("html_url").and_then(|v| v.as_str()).map(String::from),
            org_type: org_obj.get("type").and_then(|v| v.as_str()).map(String::from),
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
            payload: event.get("payload").unwrap_or(&serde_json::Value::Null).clone(),
            raw_event: event.clone(),
            api_source: "github_archive".to_string(),
        })
    }

    fn parse_datetime(&self, date_str: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(date_str)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    }

    fn calculate_quality_score(&self, total_events: i64, integrity_issues: &HashMap<String, i64>) -> f64 {
        if total_events == 0 {
            return 0.0;
        }

        let total_issues: i64 = integrity_issues.values().sum();
        let clean_percentage = ((total_events - total_issues) as f64 / total_events as f64) * 100.0;
        
        clean_percentage.clamp(0.0, 100.0)
    }

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
        
        // Split by semicolon and filter out empty commands
        schema_sql
            .split(';')
            .map(|cmd| cmd.trim().to_string())
            .filter(|cmd| !cmd.is_empty() && !cmd.starts_with("--"))
            .collect()
    }
    
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
                etag VARCHAR(255),
                last_modified TIMESTAMP WITH TIME ZONE,
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                event_count INTEGER DEFAULT 0,
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
            
            -- Performance indexes
            CREATE INDEX IF NOT EXISTS idx_github_events_created_at ON github_events (event_created_at);
            CREATE INDEX IF NOT EXISTS idx_github_events_type ON github_events (event_type);
            CREATE INDEX IF NOT EXISTS idx_github_events_actor_id ON github_events (actor_id);
            CREATE INDEX IF NOT EXISTS idx_github_events_repo_id ON github_events (repo_id);
            CREATE INDEX IF NOT EXISTS idx_github_events_actor_login ON github_events (actor_login);
            CREATE INDEX IF NOT EXISTS idx_github_events_repo_name ON github_events (repo_name);
            CREATE INDEX IF NOT EXISTS idx_github_events_payload ON github_events USING GIN (payload);
            CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories (language);
            CREATE INDEX IF NOT EXISTS idx_repositories_stars ON repositories (stargazers_count DESC);
        "#.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_validate_event() {
        let config = Config::default();
        let db = Database::new(config).await.unwrap();

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
}
