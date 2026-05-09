use anyhow::Result;
use chrono::{DateTime, Utc};
use lru::LruCache;
use md5;
use rayon::prelude::*;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::secrets::{SecretMatch, SecretSeverity};

/// High-performance secret processing engine with parallel processing
#[derive(Clone)]
pub struct PerformanceEngine {
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    deduplication_store: Arc<RwLock<HashSet<String>>>,
    metrics_collector: MetricsCollector,
    cache_policy: CachePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub data: String,
    pub expiry: chrono::DateTime<chrono::Utc>,
    pub access_count: u64,
}

#[derive(Debug, Clone, Copy)]
struct CachePolicy {
    max_entries: usize,
    ttl_hours: i64,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            max_entries: std::env::var("PERFORMANCE_CACHE_MAX_ENTRIES")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(10_000),
            ttl_hours: std::env::var("PERFORMANCE_CACHE_TTL_HOURS")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(24),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    pub secrets_processed: Arc<Mutex<u64>>,
    pub cache_hits: Arc<Mutex<u64>>,
    pub cache_misses: Arc<Mutex<u64>>,
    pub processing_time_ms: Arc<Mutex<Vec<u64>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProcessingRequest {
    pub id: Uuid,
    pub secrets: Vec<SecretMatch>,
    pub processing_options: ProcessingOptions,
    pub priority: ProcessingPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingOptions {
    pub deduplicate: bool,
    pub validate_secrets: bool,
    pub ai_triage: bool,
    pub parallel_workers: Option<usize>,
    pub cache_results: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProcessingResult {
    pub request_id: Uuid,
    pub processed_count: usize,
    pub duplicates_removed: usize,
    pub secrets_validated: usize,
    pub processing_time_ms: u64,
    pub results: Vec<ProcessedSecret>,
    pub metrics: ProcessingMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedSecret {
    pub secret_hash: String,
    pub secret_type: String,
    pub severity: String,
    pub repository: String,
    pub file_path: String,
    pub detection_time: DateTime<Utc>,
    pub validation_result: Option<crate::secrets::ValidationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingMetrics {
    pub total_processed: usize,
    pub cache_hit_rate: f64,
    pub average_processing_time_ms: f64,
    pub throughput_per_second: f64,
    pub memory_usage_mb: f64,
}

/// Database schema for efficient secret storage
pub struct SecretDatabase {
    connection: Connection,
}

impl SecretDatabase {
    /// Create new database with optimized schema
    pub fn new(db_path: &str) -> Result<Self> {
        let connection = Connection::open(db_path)?;
        let db = Self { connection };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Initialize optimized database schema
    fn initialize_schema(&self) -> Result<()> {
        // Events table with partitioning support
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT UNIQUE NOT NULL,
                event_type TEXT NOT NULL,
                repository_name TEXT NOT NULL,
                actor_login TEXT NOT NULL,
                created_at DATETIME NOT NULL,
                payload_hash TEXT NOT NULL,
                processed BOOLEAN DEFAULT FALSE
            )",
            [],
        )?;

        // Commits table with relationships
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS commits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                commit_sha TEXT UNIQUE NOT NULL,
                repository_name TEXT NOT NULL,
                event_id INTEGER,
                author_email TEXT,
                author_name TEXT,
                message TEXT,
                is_dangling BOOLEAN DEFAULT FALSE,
                created_at DATETIME NOT NULL,
                processed_at DATETIME,
                FOREIGN KEY(event_id) REFERENCES events(id)
            )",
            [],
        )?;

        // Secrets table with advanced indexing
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS secrets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                secret_hash TEXT UNIQUE NOT NULL,
                commit_id INTEGER,
                detector_name TEXT NOT NULL,
                matched_text_hash TEXT NOT NULL,
                filename TEXT,
                line_number INTEGER,
                entropy REAL,
                severity TEXT NOT NULL,
                category TEXT NOT NULL,
                context_hash TEXT,
                verified BOOLEAN DEFAULT FALSE,
                validation_status TEXT,
                validation_method TEXT,
                created_at DATETIME NOT NULL,
                updated_at DATETIME,
                FOREIGN KEY(commit_id) REFERENCES commits(id)
            )",
            [],
        )?;

        // AI Triage results table
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS triage_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                secret_id INTEGER UNIQUE,
                impact_score REAL NOT NULL,
                bounty_potential REAL NOT NULL,
                revocation_priority TEXT NOT NULL,
                analysis TEXT,
                suggested_actions TEXT, -- JSON array
                risk_factors TEXT,      -- JSON array
                confidence REAL NOT NULL,
                created_at DATETIME NOT NULL,
                FOREIGN KEY(secret_id) REFERENCES secrets(id)
            )",
            [],
        )?;

        // Repository metadata cache
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS repositories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                organization TEXT,
                is_public BOOLEAN,
                star_count INTEGER,
                contributor_count INTEGER,
                last_activity DATETIME,
                risk_score REAL,
                created_at DATETIME NOT NULL,
                updated_at DATETIME
            )",
            [],
        )?;

        self.create_indexes()?;

        // Performance optimization: Create materialized views
        self.connection.execute(
            "CREATE VIEW IF NOT EXISTS high_priority_secrets AS
            SELECT 
                s.*,
                tr.impact_score,
                tr.bounty_potential,
                tr.revocation_priority,
                c.repository_name,
                c.is_dangling
            FROM secrets s
            LEFT JOIN triage_results tr ON s.id = tr.secret_id
            LEFT JOIN commits c ON s.commit_id = c.id
            WHERE s.severity IN ('Critical', 'High')
            AND (tr.impact_score > 0.7 OR s.verified = TRUE)
            ORDER BY tr.impact_score DESC, s.created_at DESC",
            [],
        )?;

        info!("Database schema initialized successfully");
        Ok(())
    }

    fn create_indexes(&self) -> Result<()> {
        let statements = [
            // Events
            "CREATE INDEX IF NOT EXISTS idx_events_repository_created_at ON events(repository_name, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_events_type_created_at ON events(event_type, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_events_processed_created_at ON events(processed, created_at)",
            // Commits
            "CREATE INDEX IF NOT EXISTS idx_commits_repository_created_at ON commits(repository_name, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_commits_is_dangling_created_at ON commits(is_dangling, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_commits_sha ON commits(commit_sha)",
            // Secrets
            "CREATE INDEX IF NOT EXISTS idx_secrets_detector_severity ON secrets(detector_name, severity)",
            "CREATE INDEX IF NOT EXISTS idx_secrets_verified_status ON secrets(verified, validation_status)",
            "CREATE INDEX IF NOT EXISTS idx_secrets_hash ON secrets(secret_hash)",
            "CREATE INDEX IF NOT EXISTS idx_secrets_matched_text_hash ON secrets(matched_text_hash)",
            // Triage results
            "CREATE INDEX IF NOT EXISTS idx_triage_impact_score ON triage_results(impact_score DESC)",
            "CREATE INDEX IF NOT EXISTS idx_triage_bounty_potential ON triage_results(bounty_potential DESC)",
            "CREATE INDEX IF NOT EXISTS idx_triage_revocation_priority ON triage_results(revocation_priority, created_at)",
            // Repository metadata
            "CREATE INDEX IF NOT EXISTS idx_repositories_organization_name ON repositories(organization, name)",
            "CREATE INDEX IF NOT EXISTS idx_repositories_risk_score ON repositories(risk_score DESC)",
            "CREATE INDEX IF NOT EXISTS idx_repositories_last_activity ON repositories(last_activity DESC)",
        ];

        for stmt in statements {
            self.connection.execute(stmt, [])?;
        }

        Ok(())
    }

    /// Bulk insert secrets with optimized performance
    pub fn bulk_insert_secrets(&self, secrets: &[SecretMatch]) -> Result<()> {
        let tx = self.connection.unchecked_transaction()?;

        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO secrets 
                (secret_hash, detector_name, matched_text_hash, filename, line_number, 
                 entropy, severity, category, context_hash, verified, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
            )?;

            for secret in secrets {
                let matched_text_hash = format!("{:x}", md5::compute(&secret.matched_text));
                let context_hash = format!("{:x}", md5::compute(&secret.context));

                stmt.execute(params![
                    secret.hash,
                    secret.detector_name,
                    matched_text_hash,
                    secret.filename,
                    secret.line_number,
                    secret.entropy,
                    format!("{:?}", secret.severity),
                    format!("{:?}", secret.category),
                    context_hash,
                    secret.verified,
                ])?;
            }
        }

        tx.commit()?;
        info!("Bulk inserted {} secrets", secrets.len());
        Ok(())
    }

    /// Query secrets with advanced filtering
    pub fn query_secrets(&self, filters: &SecretQueryFilters) -> Result<Vec<SecretRecord>> {
        let mut query = "SELECT * FROM secrets WHERE 1=1".to_string();
        let mut params = Vec::new();

        if let Some(severity) = &filters.min_severity {
            query.push_str(" AND severity IN ");
            match severity {
                SecretSeverity::Critical => query.push_str("('Critical')"),
                SecretSeverity::High => query.push_str("('Critical', 'High')"),
                SecretSeverity::Medium => query.push_str("('Critical', 'High', 'Medium')"),
                SecretSeverity::Low => query.push_str("('Critical', 'High', 'Medium', 'Low')"),
            }
        }

        if let Some(detector) = &filters.detector_name {
            query.push_str(" AND detector_name = ?");
            params.push(detector.clone());
        }

        if filters.verified_only {
            query.push_str(" AND verified = TRUE");
        }

        if let Some(days) = filters.last_n_days {
            query.push_str(" AND created_at >= datetime('now', '-? days')");
            params.push(days.to_string());
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filters.limit {
            query.push_str(" LIMIT ?");
            params.push(limit.to_string());
        }

        let mut stmt = self.connection.prepare(&query)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(SecretRecord {
                id: row.get(0)?,
                secret_hash: row.get(1)?,
                detector_name: row.get(2)?,
                filename: row.get(3)?,
                line_number: row.get(4)?,
                entropy: row.get(5)?,
                severity: row.get(6)?,
                category: row.get(7)?,
                verified: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }
}

#[derive(Debug, Clone)]
pub struct SecretQueryFilters {
    pub min_severity: Option<SecretSeverity>,
    pub detector_name: Option<String>,
    pub verified_only: bool,
    pub last_n_days: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRecord {
    pub id: i64,
    pub secret_hash: String,
    pub detector_name: String,
    pub filename: Option<String>,
    pub line_number: Option<u32>,
    pub entropy: f64,
    pub severity: String,
    pub category: String,
    pub verified: bool,
    pub created_at: String,
}

impl PerformanceEngine {
    /// Create new performance engine
    pub fn new() -> Self {
        let cache_policy = CachePolicy::default();
        let cache_size =
            NonZeroUsize::new(cache_policy.max_entries).expect("Cache size must be non-zero");
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
            deduplication_store: Arc::new(RwLock::new(HashSet::new())),
            metrics_collector: MetricsCollector::new(),
            cache_policy,
        }
    }

    /// Process secrets in parallel batches
    pub async fn process_secrets_parallel(
        &self,
        request: BatchProcessingRequest,
    ) -> Result<BatchProcessingResult> {
        let start_time = std::time::Instant::now();
        let request_id = request.id;
        let original_count = request.secrets.len();

        info!("Starting parallel processing of {} secrets", original_count);

        // Deduplicate if requested
        let secrets = if request.processing_options.deduplicate {
            self.deduplicate_secrets(request.secrets).await?
        } else {
            request.secrets
        };

        let duplicates_removed = original_count - secrets.len();

        // Determine number of workers
        let num_workers = request
            .processing_options
            .parallel_workers
            .unwrap_or_else(num_cpus::get);

        // Split work into chunks for parallel processing
        let chunk_size = (secrets.len() / num_workers).max(1);
        let chunks: Vec<Vec<SecretMatch>> = secrets
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // Process chunks in parallel using Rayon (without database access)
        let results: Result<Vec<Vec<ProcessedSecret>>, anyhow::Error> = chunks
            .into_par_iter()
            .map(|chunk| {
                let mut results = Vec::new();

                for secret in chunk {
                    // Simple processing without database or cache for parallel execution
                    let processed_secret = ProcessedSecret {
                        secret_hash: secret.hash.clone(),
                        secret_type: secret.category.to_string(),
                        severity: secret.severity.to_string(),
                        repository: secret
                            .filename
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        file_path: secret
                            .filename
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        detection_time: chrono::Utc::now(),
                        validation_result: None, // Skip validation in parallel mode
                    };

                    results.push(processed_secret);
                }

                Ok(results)
            })
            .collect();

        let results = results?;

        // Flatten results
        let processed_secrets: Vec<ProcessedSecret> = results.into_iter().flatten().collect();

        let processing_time = start_time.elapsed().as_millis() as u64;

        // Update metrics
        {
            let mut processed_count = self
                .metrics_collector
                .secrets_processed
                .lock()
                .expect("Failed to lock secrets_processed mutex");
            *processed_count += processed_secrets.len() as u64;
        }

        {
            let mut processing_times = self
                .metrics_collector
                .processing_time_ms
                .lock()
                .expect("Failed to lock processing_time_ms mutex");
            processing_times.push(processing_time);
        }

        let metrics = self.collect_metrics().await?;

        Ok(BatchProcessingResult {
            request_id,
            processed_count: processed_secrets.len(),
            duplicates_removed,
            secrets_validated: processed_secrets
                .iter()
                .filter(|s| s.validation_result.is_some())
                .count(),
            processing_time_ms: processing_time,
            results: processed_secrets,
            metrics,
        })
    }

    /// Process a chunk of secrets (single-threaded)
    #[allow(dead_code)] // Used in specific processing scenarios
    fn process_secret_chunk(
        &self,
        secrets: Vec<SecretMatch>,
        options: &ProcessingOptions,
    ) -> Result<Vec<ProcessedSecret>> {
        let mut results = Vec::new();

        for secret in secrets {
            let start_time = std::time::Instant::now();

            // Check cache first
            let cache_key = format!("secret_{}", secret.hash);
            let cached_result = if options.cache_results {
                self.get_from_cache(&cache_key)
            } else {
                None
            };

            let processed_secret = if let Some(cached) = cached_result {
                // Cache hit
                let mut cache_hits = self.metrics_collector.cache_hits.lock().unwrap();
                *cache_hits += 1;

                // Deserialize cached result
                serde_json::from_str(&cached.data)?
            } else {
                // Cache miss - process secret
                let mut cache_misses = self.metrics_collector.cache_misses.lock().unwrap();
                *cache_misses += 1;

                let _validation_requested = options.validate_secrets;
                let _triage_requested = options.ai_triage;
                let validation_result = None;

                let _processing_time = start_time.elapsed().as_millis() as u64;

                let processed = ProcessedSecret {
                    secret_hash: format!("{:x}", md5::compute(&secret.matched_text)),
                    secret_type: secret.detector_name.clone(),
                    severity: format!("{:?}", secret.severity),
                    repository: "unknown".to_string(),
                    file_path: secret
                        .filename
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    detection_time: Utc::now(),
                    validation_result,
                };

                // Cache the result
                if options.cache_results {
                    let serialized = serde_json::to_string(&processed)?;
                    self.cache_result(&cache_key, serialized);
                }

                processed
            };

            results.push(processed_secret);
        }

        Ok(results)
    }

    /// Process a chunk of secrets (thread-safe version for parallel processing)
    /// Process secret chunk with thread safety (alternative to single-threaded processing)
    #[allow(dead_code)] // Alternative processing method for thread-safe scenarios
    fn process_secret_chunk_thread_safe(
        &self,
        secrets: Vec<SecretMatch>,
        options: &ProcessingOptions,
    ) -> Result<Vec<ProcessedSecret>> {
        let mut results = Vec::new();

        for secret in secrets {
            // Check cache first
            let cache_key = format!("secret_{}", secret.hash);
            let cached_result = if options.cache_results {
                self.get_from_cache(&cache_key)
            } else {
                None
            };

            let processed_secret = if let Some(cached) = cached_result {
                // Cache hit
                let mut cache_hits = self.metrics_collector.cache_hits.lock().unwrap();
                *cache_hits += 1;

                // Deserialize cached result
                serde_json::from_str(&cached.data)?
            } else {
                // Cache miss - process secret
                let mut cache_misses = self.metrics_collector.cache_misses.lock().unwrap();
                *cache_misses += 1;

                let _validation_requested = options.validate_secrets;
                let _triage_requested = options.ai_triage;
                let validation_result = None;

                let processed = ProcessedSecret {
                    secret_hash: format!("{:x}", md5::compute(&secret.matched_text)),
                    secret_type: secret.detector_name.clone(),
                    severity: format!("{:?}", secret.severity),
                    repository: "unknown".to_string(),
                    file_path: secret
                        .filename
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    detection_time: Utc::now(),
                    validation_result,
                };

                // Cache the result
                if options.cache_results {
                    let serialized = serde_json::to_string(&processed)?;
                    self.cache_result(&cache_key, serialized);
                }

                processed
            };

            results.push(processed_secret);
        }

        Ok(results)
    }

    /// Deduplicate secrets based on hash
    async fn deduplicate_secrets(&self, secrets: Vec<SecretMatch>) -> Result<Vec<SecretMatch>> {
        let dedup_store = self.deduplication_store.read().await;
        let mut unique_secrets = Vec::new();
        let mut new_hashes = HashSet::new();

        for secret in secrets {
            if !dedup_store.contains(&secret.hash) && !new_hashes.contains(&secret.hash) {
                new_hashes.insert(secret.hash.clone());
                unique_secrets.push(secret);
            }
        }

        // Update deduplication store
        drop(dedup_store);
        let mut dedup_store = self.deduplication_store.write().await;
        dedup_store.extend(new_hashes);

        Ok(unique_secrets)
    }

    /// Get item from cache
    #[allow(dead_code)] // Core cache functionality
    fn get_from_cache(&self, key: &str) -> Option<CacheEntry> {
        let mut cache = self
            .cache
            .lock()
            .expect("Failed to lock cache mutex in get_from_cache");
        let cached = cache.get(key).cloned();
        if cached
            .as_ref()
            .is_some_and(|entry| entry.expiry <= chrono::Utc::now())
        {
            cache.pop(key);
            return None;
        }
        cached
    }

    /// Cache a result
    #[allow(dead_code)] // Core cache functionality
    fn cache_result(&self, key: &str, data: String) {
        let entry = CacheEntry {
            data,
            expiry: chrono::Utc::now() + chrono::Duration::hours(self.cache_policy.ttl_hours),
            access_count: 1,
        };

        let mut cache = self
            .cache
            .lock()
            .expect("Failed to lock cache mutex in cache_result");
        cache.put(key.to_string(), entry);
    }

    /// Collect performance metrics
    async fn collect_metrics(&self) -> Result<ProcessingMetrics> {
        let secrets_processed = *self
            .metrics_collector
            .secrets_processed
            .lock()
            .expect("Failed to lock secrets_processed mutex in collect_metrics");
        let cache_hits = *self
            .metrics_collector
            .cache_hits
            .lock()
            .expect("Failed to lock cache_hits mutex in collect_metrics");
        let cache_misses = *self
            .metrics_collector
            .cache_misses
            .lock()
            .expect("Failed to lock cache_misses mutex in collect_metrics");
        let processing_times = self
            .metrics_collector
            .processing_time_ms
            .lock()
            .expect("Failed to lock processing_time_ms mutex in collect_metrics");

        let total_cache_requests = cache_hits + cache_misses;
        let cache_hit_rate = if total_cache_requests > 0 {
            cache_hits as f64 / total_cache_requests as f64
        } else {
            0.0
        };

        let average_processing_time = if !processing_times.is_empty() {
            processing_times.iter().sum::<u64>() as f64 / processing_times.len() as f64
        } else {
            0.0
        };

        let throughput_per_second = if average_processing_time > 0.0 {
            1000.0 / average_processing_time
        } else {
            0.0
        };

        // Estimate memory usage (rough approximation)
        let cache_size = self
            .cache
            .lock()
            .expect("Failed to lock cache mutex for size check")
            .len();
        let memory_usage_mb = (cache_size * 1024) as f64 / (1024.0 * 1024.0); // Rough estimate

        Ok(ProcessingMetrics {
            total_processed: secrets_processed as usize,
            cache_hit_rate,
            average_processing_time_ms: average_processing_time,
            throughput_per_second,
            memory_usage_mb,
        })
    }

    /// Optimize database queries
    pub async fn optimize_database(&self, db_path: &str) -> Result<()> {
        let db = SecretDatabase::new(db_path)?;

        // Run VACUUM to reclaim space
        db.connection.execute("VACUUM", [])?;

        // Analyze tables for query optimization
        db.connection.execute("ANALYZE", [])?;

        // Update statistics
        db.connection.execute("PRAGMA optimize", [])?;

        info!("Database optimization completed");
        Ok(())
    }

    /// Generate performance report
    pub async fn generate_performance_report(&self) -> Result<PerformanceReport> {
        let metrics = self.collect_metrics().await?;

        Ok(PerformanceReport {
            timestamp: chrono::Utc::now(),
            metrics: metrics.clone(),
            recommendations: self.generate_recommendations(&metrics),
        })
    }

    fn generate_recommendations(&self, metrics: &ProcessingMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();

        if metrics.cache_hit_rate < 0.5 {
            recommendations.push("Consider increasing cache size to improve hit rate".to_string());
        }

        if metrics.throughput_per_second < 10.0 {
            recommendations.push("Consider increasing parallel worker count".to_string());
        }

        if metrics.memory_usage_mb > 1000.0 {
            recommendations
                .push("High memory usage detected - consider cache eviction tuning".to_string());
        }

        if metrics.average_processing_time_ms > 1000.0 {
            recommendations
                .push("High processing time - consider optimizing secret validation".to_string());
        }

        recommendations
    }
}

impl Default for PerformanceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metrics: ProcessingMetrics,
    pub recommendations: Vec<String>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            secrets_processed: Arc::new(Mutex::new(0)),
            cache_hits: Arc::new(Mutex::new(0)),
            cache_misses: Arc::new(Mutex::new(0)),
            processing_time_ms: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{SecretCategory, SecretMatch, SecretSeverity};

    fn create_test_secret(id: &str) -> SecretMatch {
        SecretMatch {
            detector_name: "Test Detector".to_string(),
            matched_text: format!("secret_{}", id),
            start_position: 0,
            end_position: 10,
            line_number: Some(1),
            filename: Some("test.env".to_string()),
            entropy: 4.5,
            severity: SecretSeverity::High,
            category: SecretCategory::ApiKey,
            context: "test context".to_string(),
            verified: false,
            hash: format!("hash_{}", id),
        }
    }

    #[tokio::test]
    async fn test_performance_engine_creation() {
        let engine = PerformanceEngine::new();
        let metrics = engine.collect_metrics().await.unwrap();
        assert_eq!(metrics.total_processed, 0);
    }

    #[tokio::test]
    async fn test_parallel_processing() {
        let engine = PerformanceEngine::new();

        let secrets = vec![
            create_test_secret("1"),
            create_test_secret("2"),
            create_test_secret("3"),
        ];

        let request = BatchProcessingRequest {
            id: Uuid::new_v4(),
            secrets,
            processing_options: ProcessingOptions {
                deduplicate: true,
                validate_secrets: false,
                ai_triage: false,
                parallel_workers: Some(2),
                cache_results: true,
            },
            priority: ProcessingPriority::Normal,
        };

        let result = engine.process_secrets_parallel(request).await.unwrap();
        assert_eq!(result.processed_count, 3);
        assert_eq!(result.duplicates_removed, 0);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let engine = PerformanceEngine::new();

        let secrets = vec![
            create_test_secret("1"),
            create_test_secret("2"),
            create_test_secret("1"), // Duplicate
        ];

        let deduplicated = engine.deduplicate_secrets(secrets).await.unwrap();
        assert_eq!(deduplicated.len(), 2);
    }

    #[test]
    fn test_database_creation() {
        let _db = SecretDatabase::new(":memory:").expect("Failed to create in-memory database");
        // Database creation verified by successful construction
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let engine = PerformanceEngine::new();

        // Simulate some processing
        {
            let mut processed = engine.metrics_collector.secrets_processed.lock().unwrap();
            *processed = 100;
        }
        {
            let mut hits = engine.metrics_collector.cache_hits.lock().unwrap();
            *hits = 80;
        }
        {
            let mut misses = engine.metrics_collector.cache_misses.lock().unwrap();
            *misses = 20;
        }

        let metrics = engine.collect_metrics().await.unwrap();
        assert_eq!(metrics.total_processed, 100);
        assert_eq!(metrics.cache_hit_rate, 0.8);
    }

    #[tokio::test]
    async fn test_performance_report() {
        let engine = PerformanceEngine::new();
        let report = engine.generate_performance_report().await.unwrap();

        assert!(!report.recommendations.is_empty());
        assert_eq!(report.metrics.total_processed, 0);
    }

    #[test]
    fn test_expired_cache_entries_are_evicted_on_read() {
        let engine = PerformanceEngine::new();
        {
            let mut cache = engine.cache.lock().unwrap();
            cache.put(
                "secret_hash".to_string(),
                CacheEntry {
                    data: "{}".to_string(),
                    expiry: Utc::now() - chrono::Duration::hours(1),
                    access_count: 1,
                },
            );
        }

        assert!(engine.get_from_cache("secret_hash").is_none());
        assert_eq!(engine.cache.lock().unwrap().len(), 0);
    }
}
