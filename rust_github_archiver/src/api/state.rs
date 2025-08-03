use crate::core::Config;
use crate::scraper::{ScraperManager, MainScraper};
use crate::auth::UserManager;
use std::sync::{Arc, Mutex};
use anyhow::Result;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub scraper_manager: Arc<ScraperManager>,
    pub main_scraper: Arc<Mutex<Option<MainScraper>>>,
    pub user_manager: Arc<UserManager>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: config.clone(),
            scraper_manager: Arc::new(ScraperManager::new()),
            main_scraper: Arc::new(Mutex::new(None)),
            user_manager: Arc::new(UserManager::new()),
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
        if let Ok(mut scraper_opt) = self.main_scraper.lock() {
            if let Some(ref mut scraper) = *scraper_opt {
                return scraper.get_comprehensive_status().await;
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
            resource_status: None,
            database_health: None,
            quality_metrics: None,
        })
    }
}
