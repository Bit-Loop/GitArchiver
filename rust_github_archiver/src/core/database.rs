use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool, Row}; // Re-added PgPoolOptions for pool creation
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use super::config::Config;

/// Database statistics for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatistics {
    pub total_events: i64,
    pub database_size: String,
    pub table_count: i64,
    pub tables: Vec<(String, i64, String)>, // (name, row_count, size)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub status: String,
    pub connection_pool: HashMap<String, String>,
    pub schema_version: String,
    pub uptime_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub total_events: i64,
    pub data_integrity_score: f64,
    pub processing_efficiency: f64,
    pub storage_utilization: f64,
    pub cache_hit_ratio: f64,
    pub error_rate: f64,
    pub data_freshness_hours: f64,
}

/// Validated and converted event ready for database insertion
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorData {
    pub id: Option<i64>,
    pub login: Option<String>,
    pub display_login: Option<String>,
    pub gravatar_id: Option<String>,
    pub url: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoData {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub url: Option<String>,
    pub repository_url: Option<String>,
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
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing database connection...");
        
        let database_url = format!(
            "postgres://{}:{}@{}:{}/{}",
            config.database.user, config.database.password, 
            config.database.host, config.database.port, config.database.name
        );
        
        let pool = PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .connect(&database_url)
            .await
            .context("Failed to create database connection pool")?;
        
        let db = Self { pool, config };
        
        // Test the connection
        db.verify_connection().await?;
        
        // Initialize schema if needed
        db.initialize_schema().await?;
        
        info!("Database initialized successfully");
        
        Ok(db)
    }

    /// Get database configuration - useful for debugging and connection management
    pub fn get_config(&self) -> &Config {
        &self.config
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
        
        for (i, command) in schema_commands.iter().enumerate() {
            if !command.trim().is_empty() {
                match sqlx::query(&command)
                    .execute(&self.pool)
                    .await
                {
                    Ok(_) => {
                        debug!("Successfully executed schema command {}", i + 1);
                    }
                    Err(e) => {
                        error!("Failed to execute schema command {}: {} - Command: {}", i + 1, e, command);
                        // Don't fail completely - some commands might be optional
                        warn!("Continuing with schema initialization despite error");
                    }
                }
            }
        }
        
        info!("Database schema initialization completed");
        Ok(())
    }

    /// Get database schema creation commands
    fn get_schema_commands(&self) -> Vec<String> {
        vec![
            // Enable UUID extension first
            "CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"".to_string(),
            
            // Create events table
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id BIGINT PRIMARY KEY,
                type VARCHAR(255) NOT NULL,
                public BOOLEAN DEFAULT false,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL,
                actor_id BIGINT,
                actor_login VARCHAR(255),
                actor_display_login VARCHAR(255),
                actor_gravatar_id VARCHAR(255),
                actor_url TEXT,
                actor_avatar_url TEXT,
                repo_id BIGINT,
                repo_name VARCHAR(255),
                repo_url TEXT,
                repository_url TEXT,
                org_id BIGINT,
                org_login VARCHAR(255),
                org_node_id VARCHAR(255),
                org_gravatar_id VARCHAR(255),
                org_url TEXT,
                org_avatar_url TEXT,
                org_html_url TEXT,
                org_type VARCHAR(50),
                org_site_admin BOOLEAN,
                payload JSONB,
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            )
            "#.to_string(),

            // Create indexes for better performance
            "CREATE INDEX IF NOT EXISTS idx_events_type ON events(type)".to_string(),
            "CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at)".to_string(),
            "CREATE INDEX IF NOT EXISTS idx_events_actor_login ON events(actor_login)".to_string(),
            "CREATE INDEX IF NOT EXISTS idx_events_repo_name ON events(repo_name)".to_string(),
            "CREATE INDEX IF NOT EXISTS idx_events_processed_at ON events(processed_at)".to_string(),
            "CREATE INDEX IF NOT EXISTS idx_events_payload_gin ON events USING gin(payload)".to_string(),

            // Create processing metadata table
            r#"
            CREATE TABLE IF NOT EXISTS processing_metadata (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                filename VARCHAR(255) NOT NULL,
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                event_count INTEGER DEFAULT 0,
                file_size_bytes BIGINT DEFAULT 0,
                processing_duration_ms INTEGER DEFAULT 0,
                status VARCHAR(50) DEFAULT 'completed',
                error_message TEXT,
                UNIQUE(filename)
            )
            "#.to_string(),

            // Create health monitoring table
            r#"
            CREATE TABLE IF NOT EXISTS system_health (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                metric_name VARCHAR(255) NOT NULL,
                metric_value DECIMAL,
                metadata JSONB
            )
            "#.to_string(),
        ]
    }

    /// Store a batch of events in the database with optimized bulk insert
    pub async fn store_events(&self, events: &[ValidatedEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        info!("Storing {} events in database", events.len());
        
        // Use a transaction for batch insert
        let mut tx = self.pool.begin().await?;
        
        for event in events {
            let query = r#"
                INSERT INTO events (
                    id, type, public, created_at,
                    actor_id, actor_login, actor_display_login, actor_gravatar_id,
                    actor_url, actor_avatar_url,
                    repo_id, repo_name, repo_url, repository_url,
                    org_id, org_login, org_node_id, org_gravatar_id,
                    org_url, org_avatar_url, org_html_url, org_type, org_site_admin,
                    payload
                ) VALUES (
                    $1, $2, $3, $4,
                    $5, $6, $7, $8,
                    $9, $10,
                    $11, $12, $13, $14,
                    $15, $16, $17, $18,
                    $19, $20, $21, $22, $23,
                    $24
                )
                ON CONFLICT (id) DO UPDATE SET
                    type = EXCLUDED.type,
                    public = EXCLUDED.public,
                    created_at = EXCLUDED.created_at,
                    payload = EXCLUDED.payload,
                    processed_at = NOW()
            "#;

            sqlx::query(query)
                .bind(event.id)
                .bind(&event.event_type)
                .bind(event.public)
                .bind(event.created_at)
                .bind(event.actor.id)
                .bind(&event.actor.login)
                .bind(&event.actor.display_login)
                .bind(&event.actor.gravatar_id)
                .bind(&event.actor.url)
                .bind(&event.actor.avatar_url)
                .bind(event.repo.id)
                .bind(&event.repo.name)
                .bind(&event.repo.url)
                .bind(&event.repo.repository_url)
                .bind(event.org.as_ref().and_then(|o| o.id))
                .bind(event.org.as_ref().and_then(|o| o.login.as_ref()))
                .bind(event.org.as_ref().and_then(|o| o.node_id.as_ref()))
                .bind(event.org.as_ref().and_then(|o| o.gravatar_id.as_ref()))
                .bind(event.org.as_ref().and_then(|o| o.url.as_ref()))
                .bind(event.org.as_ref().and_then(|o| o.avatar_url.as_ref()))
                .bind(event.org.as_ref().and_then(|o| o.html_url.as_ref()))
                .bind(event.org.as_ref().and_then(|o| o.org_type.as_ref()))
                .bind(event.org.as_ref().and_then(|o| o.site_admin))
                .bind(&event.payload)
                .execute(&mut *tx)
                .await
                .context("Failed to insert event")?;
        }
        
        tx.commit().await?;
        info!("Successfully stored {} events", events.len());
        Ok(())
    }

    /// Validate and convert a raw JSON event
    pub fn validate_and_convert_event(&self, event: Value) -> Option<ValidatedEvent> {
        // Extract basic fields
        let id = event.get("id")?.as_i64()?;
        let event_type = event.get("type")?.as_str()?.to_string();
        let public = event.get("public").and_then(|v| v.as_bool()).unwrap_or(false);
        
        // Parse timestamp
        let created_at = event.get("created_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))?;

        // Extract actor data
        let actor_obj = event.get("actor");
        let actor = ActorData {
            id: actor_obj.and_then(|a| a.get("id")).and_then(|v| v.as_i64()),
            login: actor_obj.and_then(|a| a.get("login")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            display_login: actor_obj.and_then(|a| a.get("display_login")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            gravatar_id: actor_obj.and_then(|a| a.get("gravatar_id")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            url: actor_obj.and_then(|a| a.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            avatar_url: actor_obj.and_then(|a| a.get("avatar_url")).and_then(|v| v.as_str()).map(|s| s.to_string()),
        };

        // Extract repo data
        let repo_obj = event.get("repo");
        let repo = RepoData {
            id: repo_obj.and_then(|r| r.get("id")).and_then(|v| v.as_i64()),
            name: repo_obj.and_then(|r| r.get("name")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            url: repo_obj.and_then(|r| r.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            repository_url: repo_obj.and_then(|r| r.get("repository_url")).and_then(|v| v.as_str()).map(|s| s.to_string()),
        };

        // Extract org data (optional)
        let org = event.get("org").map(|org_obj| OrgData {
            id: org_obj.get("id").and_then(|v| v.as_i64()),
            login: org_obj.get("login").and_then(|v| v.as_str()).map(|s| s.to_string()),
            node_id: org_obj.get("node_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            gravatar_id: org_obj.get("gravatar_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            url: org_obj.get("url").and_then(|v| v.as_str()).map(|s| s.to_string()),
            avatar_url: org_obj.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
            html_url: org_obj.get("html_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
            org_type: org_obj.get("type").and_then(|v| v.as_str()).map(|s| s.to_string()),
            site_admin: org_obj.get("site_admin").and_then(|v| v.as_bool()),
        });

        let payload = event.get("payload").cloned().unwrap_or(Value::Null);

        Some(ValidatedEvent {
            id,
            event_type,
            created_at,
            public,
            actor,
            repo,
            org,
            payload,
        })
    }

    /// Get database statistics
    pub async fn get_statistics(&self) -> Result<DatabaseStatistics> {
        // Get total events count
        let total_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
            .fetch_one(&self.pool)
            .await
            .context("Failed to get total events count")?;

        // Get database size
        let db_size: String = sqlx::query_scalar(
            "SELECT pg_size_pretty(pg_database_size(current_database()))"
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get database size")?;

        // Get table statistics
        let table_stats = sqlx::query(
            r#"
            SELECT 
                schemaname || '.' || tablename as table_name,
                n_tup_ins + n_tup_upd + n_tup_del as row_count,
                pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size
            FROM pg_stat_user_tables 
            ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC
            LIMIT 10
            "#
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get table statistics")?;

        let tables: Vec<(String, i64, String)> = table_stats
            .iter()
            .map(|row| (
                row.get::<String, _>("table_name"),
                row.get::<i64, _>("row_count"),
                row.get::<String, _>("size"),
            ))
            .collect();

        Ok(DatabaseStatistics {
            total_events,
            database_size: db_size,
            table_count: tables.len() as i64,
            tables,
        })
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
            "created_at": "2023-01-01T00:00:00Z",
            "public": true,
            "actor": {
                "id": 12345,
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
