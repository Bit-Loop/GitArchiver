use crate::audit::AuditLogger;
use crate::auth::UserManager;
use crate::core::{Config, Database, PersistenceService, ResourceLimits, ResourceMonitor};
use crate::operator::ScraperRuntimeService;
use crate::rate_limiter::{RateLimitConfig, RateLimiter};
use crate::realtime::metrics::MetricsCollector;
use crate::realtime::token_pool::TokenPool;
use crate::realtime::webhook::WebhookManager;
use crate::realtime::GitHubEventMonitor;
use crate::scanning::{domain::ScanInitiator, ScanConfig, ScanType, ScanningService};
use crate::scraper::ScraperManager;
use crate::security::{CorsConfig, SecurityConfig};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub scraper_manager: Arc<ScraperManager>,
    pub scraper_runtime: Arc<ScraperRuntimeService>,
    pub event_monitor: Arc<tokio::sync::Mutex<Option<Arc<GitHubEventMonitor>>>>,
    pub user_manager: Arc<UserManager>,
    pub resource_monitor: Arc<tokio::sync::Mutex<ResourceMonitor>>,
    pub scanning_service: Arc<ScanningService>,
    pub database: Arc<Database>,
    pub persistence: Arc<PersistenceService>,
    pub token_pool: Arc<TokenPool>,
    pub webhook_manager: Arc<WebhookManager>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub rate_limiter: Arc<RateLimiter>,
    pub security_config: SecurityConfig,
    pub cors_config: CorsConfig,
    pub audit_logger: Arc<AuditLogger>,
}

impl AppState {
    /// Create a new application state. The database is constructed externally so that
    /// initialization that requires async (Database::new) happens before this sync constructor.
    pub fn new(config: Config, database: Arc<Database>) -> Self {
        let resource_limits: ResourceLimits = (&config.resources).into();

        // Configure rate limiting
        let rate_limit_config = RateLimitConfig {
            max_requests: 120,
            window: Duration::from_secs(60), // 1 minute window
            burst_size: 20,
        };
        let rate_limiter = Arc::new(RateLimiter::new(rate_limit_config));

        // Configure security headers
        let security_config = SecurityConfig::default();

        let allowed_origins = config
            .web
            .cors_origins
            .iter()
            .map(|origin| origin.trim().to_string())
            .filter(|origin| !origin.is_empty())
            .collect::<Vec<_>>();

        let cors_config = CorsConfig {
            allowed_origins: if allowed_origins.is_empty() {
                CorsConfig::default().allowed_origins
            } else {
                allowed_origins
            },
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "PATCH".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec![
                "Authorization".to_string(),
                "Content-Type".to_string(),
                "Accept".to_string(),
                "Origin".to_string(),
                "X-Requested-With".to_string(),
            ],
            exposed_headers: vec![
                "X-RateLimit-Limit".to_string(),
                "X-RateLimit-Remaining".to_string(),
                "X-RateLimit-Reset".to_string(),
            ],
            allow_credentials: true,
            max_age: 3600, // 1 hour cache for preflight requests
        };

        // Initialize audit logger with database connection
        let audit_logger = Arc::new(AuditLogger::new(database.pool().clone()));
        let persistence = Arc::new(PersistenceService::new(database.clone()));

        let scan_concurrency = usize::max(
            1,
            usize::min(config.download.max_concurrent_downloads as usize, 32),
        );
        let scanning_service =
            Arc::new(ScanningService::new(scan_concurrency).with_persistence(persistence.clone()));
        let scraper_manager = Arc::new(ScraperManager::new());
        let resource_monitor = Arc::new(tokio::sync::Mutex::new(ResourceMonitor::new(
            resource_limits,
        )));
        let scraper_runtime = Arc::new(ScraperRuntimeService::new(
            config.clone(),
            scraper_manager.clone(),
            resource_monitor.clone(),
            scanning_service.clone(),
        ));

        Self {
            config: config.clone(),
            scraper_manager,
            scraper_runtime,
            event_monitor: Arc::new(tokio::sync::Mutex::new(None)),
            user_manager: Arc::new(
                UserManager::from_admin_password(&config.security.admin_password)
                    .expect("validated ADMIN_PASSWORD should create the initial admin user"),
            ),
            resource_monitor,
            scanning_service,
            database,
            persistence,
            token_pool: Arc::new(TokenPool::new()),
            webhook_manager: Arc::new(WebhookManager::new()),
            metrics_collector: Arc::new(MetricsCollector::new()),
            rate_limiter,
            security_config,
            cors_config,
            audit_logger,
        }
    }

    pub fn start_background_workers(&self) {
        self.spawn_scanner_worker();
    }

    fn spawn_scanner_worker(&self) {
        let persistence = self.persistence.clone();
        let scanning_service = self.scanning_service.clone();
        let scraper_manager = self.scraper_manager.clone();
        let worker_id = format!("scanner-worker-{}", Uuid::new_v4());

        tokio::spawn(async move {
            let mut idle_delay = Duration::from_secs(5);
            loop {
                if !scraper_manager.is_running() {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    idle_delay = Duration::from_secs(5);
                    continue;
                }
                match persistence.claim_pending_push_events(25, &worker_id).await {
                    Ok(events) if events.is_empty() => {
                        tokio::time::sleep(idle_delay).await;
                        idle_delay = (idle_delay * 2).min(Duration::from_secs(60));
                    }
                    Ok(events) => {
                        idle_delay = Duration::from_secs(5);
                        Self::launch_scans_for_events(
                            &scanning_service,
                            &persistence,
                            events,
                            &worker_id,
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            worker_id = %worker_id,
                            error = ?e,
                            "Scanner worker failed to claim events"
                        );
                        tokio::time::sleep(Duration::from_secs(15)).await;
                    }
                }
            }
        });
    }

    async fn launch_scans_for_events(
        scanning_service: &Arc<ScanningService>,
        persistence: &Arc<PersistenceService>,
        events: Vec<crate::core::database::EventScanTarget>,
        worker_id: &str,
    ) {
        if events.is_empty() {
            return;
        }

        let mut grouped: HashMap<String, Vec<crate::core::database::EventScanTarget>> =
            HashMap::new();
        for event in events {
            grouped
                .entry(event.repository_full_name.clone())
                .or_default()
                .push(event);
        }

        for (repository, event_targets) in grouped {
            let config = ScanConfig::default();
            let event_ids: Vec<i64> = event_targets.iter().map(|e| e.event_id).collect();

            if let Err(e) = scanning_service
                .start_scan(
                    repository.clone(),
                    ScanType::Incremental,
                    config.clone(),
                    ScanInitiator::worker(worker_id),
                    event_targets,
                )
                .await
            {
                tracing::error!(
                    "Failed to start scan for {} from worker {}: {}",
                    repository,
                    worker_id,
                    e
                );
                let error_message = e.to_string();
                let lifecycle_blocked =
                    error_message.contains("paused") || error_message.contains("shutting down");
                let db_result = if lifecycle_blocked {
                    persistence.release_push_events(&event_ids).await
                } else {
                    persistence
                        .mark_push_events_failed(&event_ids, Some(error_message.as_str()))
                        .await
                };

                if let Err(db_err) = db_result {
                    tracing::warn!(
                        "Failed to update push event state after scan rejection: {}",
                        db_err
                    );
                }
            }
        }
    }

    pub async fn initialize_scraper_runtime(&self) -> Result<()> {
        self.scraper_runtime.start().await
    }

    pub async fn get_comprehensive_status(&self) -> Result<crate::scraper::MainScraperStatus> {
        // Get resource status
        let resource_status = {
            let mut monitor = self.resource_monitor.lock().await;
            monitor.get_resource_status().await.ok()
        };
        let database_health = { Some(self.persistence.health_status().await) };

        if let Ok(status) = self.scraper_manager.get_status() {
            let uptime_seconds = status
                .start_time
                .map(|start_time| (Utc::now() - start_time).num_seconds().max(0) as f64)
                .unwrap_or(0.0);

            let last_activity = Some(status.last_updated.timestamp() as f64);

            return Ok(crate::scraper::MainScraperStatus {
                running: matches!(status.state, crate::scraper::ScraperState::Running),
                uptime_seconds,
                total_files_processed: status.files_processed,
                total_events_processed: status.events_processed,
                total_errors: status.error_count,
                last_activity,
                resource_status,
                database_health,
                quality_metrics: None,
            });
        }

        Ok(crate::scraper::MainScraperStatus {
            running: false,
            uptime_seconds: 0.0,
            total_files_processed: 0,
            total_events_processed: 0,
            total_errors: 0,
            last_activity: None,
            resource_status,
            database_health,
            quality_metrics: None,
        })
    }

    pub async fn perform_emergency_cleanup(&self) -> Result<()> {
        let monitor = self.resource_monitor.lock().await;
        if monitor.is_emergency_mode() {
            monitor.emergency_cleanup().await?;
            tracing::info!("Emergency cleanup completed");
        }
        Ok(())
    }
}
