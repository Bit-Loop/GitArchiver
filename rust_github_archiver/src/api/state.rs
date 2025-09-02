use crate::core::{Config, ResourceMonitor, ResourceLimits};
use crate::scraper::{ScraperManager, MainScraper};
use crate::auth::UserManager;
use crate::scanning::ScanningService;
use std::sync::{Arc, Mutex};
use anyhow::Result;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub scraper_manager: Arc<ScraperManager>,
    pub main_scraper: Arc<Mutex<Option<MainScraper>>>,
    pub user_manager: Arc<UserManager>,
    pub resource_monitor: Arc<Mutex<ResourceMonitor>>,
    pub scanning_service: Arc<ScanningService>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let resource_limits = ResourceLimits {
            memory_limit_gb: 18.0,
            disk_limit_gb: 40.0,
            cpu_limit_percent: 80.0,
            memory_warning_threshold: 0.8,
            disk_warning_threshold: 0.8,
            cpu_warning_threshold: 0.7,
            emergency_cleanup_threshold: 0.9,
            monitoring_interval_seconds: 30,
        };

        Self {
            config: config.clone(),
            scraper_manager: Arc::new(ScraperManager::new()),
            main_scraper: Arc::new(Mutex::new(None)),
            user_manager: Arc::new(UserManager::new()),
            resource_monitor: Arc::new(Mutex::new(ResourceMonitor::new(resource_limits))),
            scanning_service: Arc::new(ScanningService::new(5)), // Max 5 concurrent scans
        }
    }

    pub async fn initialize_main_scraper(&self) -> Result<()> {
        let mut main_scraper = MainScraper::new(self.config.clone())?;
        main_scraper.initialize().await?;
        
        if let Ok(mut scraper_opt) = self.main_scraper.lock() {
            *scraper_opt = Some(main_scraper);
        }
        
        Ok(())
    }

    pub async fn get_comprehensive_status(&self) -> Result<crate::scraper::MainScraperStatus> {
        // Get resource status
        let resource_status = if let Ok(mut monitor) = self.resource_monitor.lock() {
            monitor.get_resource_status().await.ok()
        } else {
            None
        };

        if let Ok(mut scraper_opt) = self.main_scraper.lock() {
            if let Some(ref mut scraper) = *scraper_opt {
                let mut status = scraper.get_comprehensive_status().await?;
                status.resource_status = resource_status;
                return Ok(status);
            }
        }
        
        // Return basic status if main scraper not available
        Ok(crate::scraper::MainScraperStatus {
            running: self.scraper_manager.is_running(),
            uptime_seconds: 0.0,
            total_files_processed: 0,
            total_events_processed: 0,
            total_errors: 0,
            last_activity: None,
            resource_status,
            database_health: None,
            quality_metrics: None,
        })
    }

    pub async fn perform_emergency_cleanup(&self) -> Result<()> {
        if let Ok(monitor) = self.resource_monitor.lock() {
            if monitor.is_emergency_mode() {
                monitor.emergency_cleanup().await?;
                tracing::info!("Emergency cleanup completed");
            }
        }
        Ok(())
    }
}
