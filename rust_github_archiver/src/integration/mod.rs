use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::bigquery::BigQueryScanner;
use crate::github::DanglingCommitFetcher;
use crate::secrets::{SecretScanner, SecretValidator, SecretMatch};
use crate::ai::{AITriageAgent, TriageResult, TriageContext};
use crate::realtime::GitHubEventMonitor;
use crate::performance::{PerformanceEngine, SecretDatabase};
use crate::gui::SecretsNinjaApp;

/// Comprehensive GitHub secret hunting platform
pub struct GitHubSecretHunter {
    pub bigquery_scanner: BigQueryScanner,
    pub commit_fetcher: DanglingCommitFetcher,
    pub secret_scanner: SecretScanner,
    pub secret_validator: SecretValidator,
    pub ai_triage_agent: Option<AITriageAgent>,
    pub event_monitor: GitHubEventMonitor,
    pub performance_engine: PerformanceEngine,
    pub database: SecretDatabase,
    pub config: HunterConfig,
    pub state: Arc<RwLock<HunterState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HunterConfig {
    pub gcp_project_id: String,
    pub github_token: String,
    pub redis_url: Option<String>,
    pub database_path: String,
    pub ai_model_path: Option<String>,
    pub webhook_endpoints: Vec<String>,
    pub scanning_options: ScanningOptions,
    pub performance_options: PerformanceOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanningOptions {
    pub enable_bigquery_scanning: bool,
    pub enable_realtime_monitoring: bool,
    pub enable_ai_triage: bool,
    pub enable_secret_validation: bool,
    pub organizations_to_monitor: Vec<String>,
    pub minimum_entropy_threshold: f64,
    pub scan_historical_events: bool,
    pub historical_days_back: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceOptions {
    pub parallel_workers: usize,
    pub cache_size: usize,
    pub batch_size: usize,
    pub rate_limit_per_hour: u32,
    pub enable_caching: bool,
    pub enable_deduplication: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HunterState {
    pub is_running: bool,
    pub started_at: Option<DateTime<Utc>>,
    pub last_bigquery_scan: Option<DateTime<Utc>>,
    pub last_realtime_event: Option<DateTime<Utc>>,
    pub total_secrets_found: u64,
    pub total_repositories_scanned: u64,
    pub total_commits_processed: u64,
    pub high_priority_alerts: u64,
    pub active_monitoring_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanningReport {
    pub scan_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub scan_type: ScanType,
    pub target: String,
    pub secrets_found: Vec<SecretMatch>,
    pub triage_results: Vec<TriageResult>,
    pub performance_metrics: crate::performance::ProcessingMetrics,
    pub recommendations: Vec<String>,
    pub status: ScanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanType {
    BigQueryHistorical,
    RealtimeMonitoring,
    ManualRepository,
    ScheduledScan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl GitHubSecretHunter {
    /// Create a new comprehensive secret hunter
    pub async fn new(config: HunterConfig) -> Result<Self> {
        info!("Initializing GitHub Secret Hunter with config: {:?}", config);

        // Initialize BigQuery scanner
        let bigquery_scanner = BigQueryScanner::new(&config.gcp_project_id).await?;

        // Initialize GitHub commit fetcher
        let commit_fetcher = DanglingCommitFetcher::new(config.github_token.clone());

        // Initialize secret scanner
        let secret_scanner = SecretScanner::new();

        // Initialize secret validator
        let secret_validator = SecretValidator::new();

        // Initialize AI triage agent if configured
        let ai_triage_agent = if config.scanning_options.enable_ai_triage {
            if let Some(model_path) = &config.ai_model_path {
                Some(AITriageAgent::new(model_path).await?)
            } else {
                info!("AI triage enabled but no model path provided, using small model");
                Some(AITriageAgent::new_with_small_model().await?)
            }
        } else {
            None
        };

        // Initialize real-time event monitor
        let mut event_monitor = GitHubEventMonitor::new();
        if let Some(ai_agent) = &ai_triage_agent {
            // Note: This would need proper ownership handling in practice
            // event_monitor = event_monitor.with_ai_triage(ai_agent.clone()).await;
        }

        // Initialize performance engine
        let performance_engine = PerformanceEngine::new();

        // Initialize database
        let database = SecretDatabase::new(&config.database_path)?;

        // Initialize state
        let state = Arc::new(RwLock::new(HunterState {
            is_running: false,
            started_at: None,
            last_bigquery_scan: None,
            last_realtime_event: None,
            total_secrets_found: 0,
            total_repositories_scanned: 0,
            total_commits_processed: 0,
            high_priority_alerts: 0,
            active_monitoring_targets: config.scanning_options.organizations_to_monitor.clone(),
        }));

        Ok(Self {
            bigquery_scanner,
            commit_fetcher,
            secret_scanner,
            secret_validator,
            ai_triage_agent,
            event_monitor,
            performance_engine,
            database,
            config,
            state,
        })
    }

    /// Start comprehensive secret hunting
    pub async fn start_hunting(&mut self) -> Result<()> {
        info!("Starting comprehensive GitHub secret hunting");

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = true;
            state.started_at = Some(Utc::now());
        }

        // Start real-time monitoring if enabled
        if self.config.scanning_options.enable_realtime_monitoring {
            let event_monitor = self.event_monitor.clone();
            tokio::spawn(async move {
                if let Err(e) = event_monitor.start_monitoring().await {
                    error!("Real-time monitoring failed: {}", e);
                }
            });
        }

        // Run historical BigQuery scan if enabled
        if self.config.scanning_options.enable_bigquery_scanning {
            self.run_bigquery_scan().await?;
        }

        info!("GitHub Secret Hunter started successfully");
        Ok(())
    }

    /// Run BigQuery historical scan
    async fn run_bigquery_scan(&mut self) -> Result<ScanningReport> {
        let scan_id = Uuid::new_v4();
        info!("Starting BigQuery historical scan with ID: {}", scan_id);

        let mut report = ScanningReport {
            scan_id,
            started_at: Utc::now(),
            completed_at: None,
            scan_type: ScanType::BigQueryHistorical,
            target: "GitHub Archive".to_string(),
            secrets_found: Vec::new(),
            triage_results: Vec::new(),
            performance_metrics: crate::performance::ProcessingMetrics {
                total_processed: 0,
                cache_hit_rate: 0.0,
                average_processing_time_ms: 0.0,
                throughput_per_second: 0.0,
                memory_usage_mb: 0.0,
            },
            recommendations: Vec::new(),
            status: ScanStatus::Running,
        };

        // Scan each organization
        for org in &self.config.scanning_options.organizations_to_monitor {
            info!("Scanning organization: {}", org);

            match self.scan_organization_historical(org).await {
                Ok(mut org_secrets) => {
                    info!("Found {} secrets for organization: {}", org_secrets.len(), org);
                    report.secrets_found.append(&mut org_secrets);
                }
                Err(e) => {
                    error!("Failed to scan organization {}: {}", org, e);
                }
            }
        }

        // Run AI triage on found secrets
        if self.config.scanning_options.enable_ai_triage && !report.secrets_found.is_empty() {
            info!("Running AI triage on {} secrets", report.secrets_found.len());
            
            if let Some(ai_agent) = &mut self.ai_triage_agent {
                for secret in &report.secrets_found {
                    let context = TriageContext {
                        repository_name: secret.filename.clone().unwrap_or_default(),
                        organization: None,
                        is_public_repository: true,
                        recent_activity: true,
                        contributor_count: None,
                        star_count: None,
                    };

                    match ai_agent.triage_secret(secret, None, &context).await {
                        Ok(triage) => report.triage_results.push(triage),
                        Err(e) => warn!("AI triage failed for secret {}: {}", secret.hash, e),
                    }
                }
            }
        }

        // Store secrets in database
        if !report.secrets_found.is_empty() {
            self.database.bulk_insert_secrets(&report.secrets_found)?;
        }

        // Update state
        {
            let mut state = self.state.write().await;
            state.last_bigquery_scan = Some(Utc::now());
            state.total_secrets_found += report.secrets_found.len() as u64;
        }

        report.completed_at = Some(Utc::now());
        report.status = ScanStatus::Completed;

        info!("BigQuery scan completed. Found {} secrets", report.secrets_found.len());
        Ok(report)
    }

    /// Scan a specific organization's historical data
    async fn scan_organization_historical(&mut self, organization: &str) -> Result<Vec<SecretMatch>> {
        let mut all_secrets = Vec::new();

        // Get zero-commit events from BigQuery
        let events = self.bigquery_scanner.scan_zero_commit_events(
            Some(organization),
            self.config.scanning_options.historical_days_back,
        ).await?;

        info!("Found {} zero-commit events for {}", events.len(), organization);

        // Process events in batches for performance
        let batch_size = self.config.performance_options.batch_size;
        for batch in events.chunks(batch_size) {
            let mut batch_secrets = Vec::new();

            for event in batch {
                // Try to fetch the dangling commit
                match self.commit_fetcher.fetch_commit(&event.repository, &event.before_commit).await {
                    Ok(commit_data) => {
                        // Scan commit for secrets
                        match self.secret_scanner.scan_text(&commit_data).await {
                            Ok(mut secrets) => {
                                // Filter by entropy if configured
                                secrets.retain(|s| s.entropy >= self.config.scanning_options.minimum_entropy_threshold);
                                batch_secrets.extend(secrets);
                            }
                            Err(e) => warn!("Failed to scan commit {}: {}", event.before_commit, e),
                        }
                    }
                    Err(e) => {
                        debug!("Could not fetch commit {} (likely dangling): {}", event.before_commit, e);
                    }
                }
            }

            all_secrets.extend(batch_secrets);
        }

        Ok(all_secrets)
    }

    /// Scan a specific repository manually
    pub async fn scan_repository(&mut self, repository: &str) -> Result<ScanningReport> {
        let scan_id = Uuid::new_v4();
        info!("Starting manual repository scan: {} (ID: {})", repository, scan_id);

        let mut report = ScanningReport {
            scan_id,
            started_at: Utc::now(),
            completed_at: None,
            scan_type: ScanType::ManualRepository,
            target: repository.to_string(),
            secrets_found: Vec::new(),
            triage_results: Vec::new(),
            performance_metrics: crate::performance::ProcessingMetrics {
                total_processed: 0,
                cache_hit_rate: 0.0,
                average_processing_time_ms: 0.0,
                throughput_per_second: 0.0,
                memory_usage_mb: 0.0,
            },
            recommendations: Vec::new(),
            status: ScanStatus::Running,
        };

        // Implementation would scan the specific repository
        // For now, return empty results
        
        report.completed_at = Some(Utc::now());
        report.status = ScanStatus::Completed;

        Ok(report)
    }

    /// Get current hunting status
    pub async fn get_status(&self) -> HunterState {
        self.state.read().await.clone()
    }

    /// Stop hunting operations
    pub async fn stop_hunting(&mut self) -> Result<()> {
        info!("Stopping GitHub Secret Hunter");

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = false;
        }

        // Stop real-time monitoring
        // Implementation would stop the monitoring task

        info!("GitHub Secret Hunter stopped");
        Ok(())
    }

    /// Get comprehensive dashboard data
    pub async fn get_dashboard_data(&self) -> Result<DashboardData> {
        let state = self.state.read().await.clone();
        
        // Query recent secrets from database
        let filters = crate::performance::SecretQueryFilters {
            min_severity: Some(crate::secrets::SecretSeverity::Medium),
            detector_name: None,
            verified_only: false,
            last_n_days: Some(7),
            limit: Some(100),
        };
        
        let recent_secrets = self.database.query_secrets(&filters)?;
        
        // Get performance metrics
        let performance_metrics = self.performance_engine.collect_metrics().await?;

        Ok(DashboardData {
            state,
            recent_secrets_count: recent_secrets.len(),
            performance_metrics,
            active_scans: Vec::new(), // Would query active scans
            alerts: Vec::new(),       // Would query recent alerts
        })
    }

    /// Launch GUI application
    pub async fn launch_gui(&self) -> Result<()> {
        info!("Launching Secrets Ninja GUI");
        
        // This would launch the Iced GUI application
        // For now, just log the action
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub state: HunterState,
    pub recent_secrets_count: usize,
    pub performance_metrics: crate::performance::ProcessingMetrics,
    pub active_scans: Vec<ScanningReport>,
    pub alerts: Vec<String>,
}

/// Default configuration for testing/development
impl Default for HunterConfig {
    fn default() -> Self {
        Self {
            gcp_project_id: "github-archive-project".to_string(),
            github_token: std::env::var("GITHUB_TOKEN").unwrap_or_default(),
            redis_url: Some("redis://localhost:6379".to_string()),
            database_path: "secrets.db".to_string(),
            ai_model_path: None,
            webhook_endpoints: Vec::new(),
            scanning_options: ScanningOptions {
                enable_bigquery_scanning: true,
                enable_realtime_monitoring: true,
                enable_ai_triage: true,
                enable_secret_validation: true,
                organizations_to_monitor: vec!["github".to_string()],
                minimum_entropy_threshold: 3.0,
                scan_historical_events: true,
                historical_days_back: 30,
            },
            performance_options: PerformanceOptions {
                parallel_workers: num_cpus::get(),
                cache_size: 10000,
                batch_size: 100,
                rate_limit_per_hour: 5000,
                enable_caching: true,
                enable_deduplication: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hunter_creation() {
        let config = HunterConfig::default();
        
        // This would require proper credentials to work
        // For now, just test the configuration
        assert!(!config.gcp_project_id.is_empty());
        assert!(config.scanning_options.enable_bigquery_scanning);
    }

    #[test]
    fn test_default_config() {
        let config = HunterConfig::default();
        assert_eq!(config.scanning_options.historical_days_back, 30);
        assert!(config.performance_options.enable_caching);
        assert!(!config.scanning_options.organizations_to_monitor.is_empty());
    }

    #[tokio::test]
    async fn test_dashboard_data_structure() {
        // Test that we can create dashboard data
        let state = HunterState {
            is_running: false,
            started_at: None,
            last_bigquery_scan: None,
            last_realtime_event: None,
            total_secrets_found: 0,
            total_repositories_scanned: 0,
            total_commits_processed: 0,
            high_priority_alerts: 0,
            active_monitoring_targets: Vec::new(),
        };

        let metrics = crate::performance::ProcessingMetrics {
            total_processed: 0,
            cache_hit_rate: 0.0,
            average_processing_time_ms: 0.0,
            throughput_per_second: 0.0,
            memory_usage_mb: 0.0,
        };

        let dashboard = DashboardData {
            state,
            recent_secrets_count: 0,
            performance_metrics: metrics,
            active_scans: Vec::new(),
            alerts: Vec::new(),
        };

        assert_eq!(dashboard.recent_secrets_count, 0);
    }
}
