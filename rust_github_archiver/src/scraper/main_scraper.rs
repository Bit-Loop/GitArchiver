use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;
use anyhow::Result;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};

use crate::core::{Config, DatabaseManager, ResourceMonitor, ResourceLimits};
use crate::scraper::{
    ScraperManager, ArchiveScraper, FileProcessor, Downloader,
    DownloadConfig, ProcessingConfig, ScrapingStats
};

#[derive(Debug, Clone, Serialize)]
pub struct MainScraperStatus {
    pub running: bool,
    pub uptime_seconds: f64,
    pub total_files_processed: u64,
    pub total_events_processed: u64,
    pub total_errors: u64,
    pub last_activity: Option<f64>,
    pub resource_status: Option<crate::core::ResourceStatus>,
    pub database_health: Option<crate::core::DatabaseHealth>,
    pub quality_metrics: Option<crate::core::QualityMetrics>,
}

pub struct MainScraper {
    config: Config,
    scraper_manager: Arc<ScraperManager>,
    archive_scraper: Option<ArchiveScraper>,
    file_processor: FileProcessor,
    downloader: Downloader,
    database_manager: Option<DatabaseManager>,
    resource_monitor: Option<ResourceMonitor>,
    start_time: Option<SystemTime>,
    shutdown_requested: bool,
}

impl MainScraper {
    pub fn new(config: Config) -> Result<Self> {
        let scraper_manager = Arc::new(ScraperManager::new());
        
        let download_config = DownloadConfig {
            max_concurrent_downloads: 6,
            chunk_size: 4096,
            request_timeout_seconds: 180,
            max_retries: 3,
            retry_delay_seconds: 2.0,
        };

        let processing_config = ProcessingConfig {
            batch_size: 500,
            max_memory_usage_mb: 512,
            enable_validation: true,
            save_raw_data: false,
            extract_metadata: true,
        };

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

        Ok(Self {
            config: config.clone(),
            scraper_manager: scraper_manager.clone(),
            archive_scraper: Some(ArchiveScraper::new(config.clone(), scraper_manager)),
            file_processor: FileProcessor::new(processing_config),
            downloader: Downloader::new(download_config)?,
            database_manager: Some(DatabaseManager::new(config)),
            resource_monitor: Some(ResourceMonitor::new(resource_limits)),
            start_time: None,
            shutdown_requested: false,
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing main scraper...");

        // Initialize database
        if let Some(ref mut db) = self.database_manager {
            db.connect().await?;
            info!("Database connection established");
        }

        // Initialize archive scraper
        if let Some(ref scraper) = self.archive_scraper {
            scraper.initialize().await?;
            info!("Archive scraper initialized");
        }

        // Set start time
        self.start_time = Some(SystemTime::now());

        info!("Main scraper initialization complete");
        Ok(())
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting main scraper...");

        // Start the scraper state
        self.scraper_manager.start()?;
        
        // Start the main processing loop
        self.run_main_loop().await?;

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping main scraper...");

        self.shutdown_requested = true;
        self.scraper_manager.stop()?;

        // Shutdown archive scraper
        if let Some(ref scraper) = self.archive_scraper {
            scraper.shutdown().await?;
        }

        // Disconnect database
        if let Some(ref mut db) = self.database_manager {
            db.disconnect().await?;
        }

        info!("Main scraper stopped");
        Ok(())
    }

    pub async fn pause(&mut self) -> Result<()> {
        info!("Pausing main scraper...");
        self.scraper_manager.pause()?;
        Ok(())
    }

    pub async fn resume(&mut self) -> Result<()> {
        info!("Resuming main scraper...");
        self.scraper_manager.resume()?;
        Ok(())
    }

    pub async fn restart(&mut self) -> Result<()> {
        info!("Restarting main scraper...");
        
        self.scraper_manager.restart()?;
        self.start_time = Some(SystemTime::now());
        
        info!("Main scraper restarted");
        Ok(())
    }

    pub async fn get_comprehensive_status(&mut self) -> Result<MainScraperStatus> {
        let scraper_status = self.scraper_manager.get_status()?;
        let running = self.scraper_manager.is_running();
        
        let uptime_seconds = if let Some(start_time) = self.start_time {
            start_time.elapsed().unwrap_or(Duration::ZERO).as_secs_f64()
        } else {
            0.0
        };

        // Get resource status
        let resource_status = if let Some(ref mut monitor) = self.resource_monitor {
            monitor.get_resource_status().await.ok()
        } else {
            None
        };

        // Get database health
        let database_health = if let Some(ref db) = self.database_manager {
            db.get_health_status().await.ok()
        } else {
            None
        };

        // Get quality metrics
        let quality_metrics = if let Some(ref db) = self.database_manager {
            db.get_quality_metrics().await.ok()
        } else {
            None
        };

        let last_activity = if scraper_status.last_updated != chrono::DateTime::<chrono::Utc>::MIN_UTC {
            Some(scraper_status.last_updated.timestamp() as f64)
        } else {
            None
        };

        Ok(MainScraperStatus {
            running,
            uptime_seconds,
            total_files_processed: scraper_status.files_processed,
            total_events_processed: scraper_status.events_processed,
            total_errors: scraper_status.error_count,
            last_activity,
            resource_status,
            database_health,
            quality_metrics,
        })
    }

    async fn run_main_loop(&mut self) -> Result<()> {
        info!("Starting main processing loop...");

        while !self.shutdown_requested {
            // Check if scraper should be running
            if !self.scraper_manager.is_running() {
                debug!("Scraper not running, waiting...");
                time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            // Check resource status
            if let Some(ref mut monitor) = self.resource_monitor {
                match monitor.get_resource_status().await {
                    Ok(status) => {
                        if status.emergency_mode {
                            warn!("Emergency mode activated: {:?}", status.emergency_conditions);
                            
                            // Pause scraper during emergency
                            let _ = self.scraper_manager.pause();
                            
                            // Perform cleanup
                            if let Err(e) = monitor.emergency_cleanup().await {
                                error!("Emergency cleanup failed: {}", e);
                            }
                            
                            // Wait for system to recover
                            time::sleep(Duration::from_secs(60)).await;
                            
                            // Resume scraper
                            let _ = self.scraper_manager.resume();
                            continue;
                        }
                    }
                    Err(e) => {
                        error!("Resource monitoring error: {}", e);
                    }
                }
            }

            // Run archive scraping
            if let Some(ref scraper) = self.archive_scraper {
                match scraper.run_continuous_scraping().await {
                    Ok(()) => {
                        debug!("Archive scraping cycle completed");
                    }
                    Err(e) => {
                        error!("Archive scraping error: {}", e);
                        
                        // Add error to stats
                        let _ = self.scraper_manager.add_error();
                        
                        // Wait before retrying
                        time::sleep(Duration::from_secs(30)).await;
                    }
                }
            }

            // Brief pause between cycles
            time::sleep(Duration::from_secs(10)).await;
        }

        info!("Main processing loop stopped");
        Ok(())
    }

    pub async fn process_single_file(&self, filename: &str) -> Result<crate::scraper::FileProcessingResult> {
        info!("Processing single file: {}", filename);

        let file_path = self.config.download.download_dir.join(filename);
        
        if !file_path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", filename));
        }

        let result = self.file_processor.process_archive_file(&file_path).await?;
        
        // Mark file as processed in database
        if let Some(ref db) = self.database_manager {
            db.mark_file_processed(
                filename,
                None, // etag
                result.file_size_bytes,
                result.valid_events,
                result.processing_time_seconds,
            ).await?;
        }

        info!("Successfully processed file: {} ({} events)", filename, result.valid_events);
        Ok(result)
    }

    pub async fn download_file(&self, url: &str, filename: &str) -> Result<crate::scraper::DownloadResult> {
        info!("Downloading file: {} -> {}", url, filename);

        let local_path = self.config.download.download_dir.join(filename);
        let result = self.downloader.download_file(url, &local_path, None).await?;

        info!("Download completed: {:?}", result.status);
        Ok(result)
    }

    pub async fn get_available_files(&self) -> Result<Vec<crate::scraper::ArchiveFile>> {
        if let Some(ref scraper) = self.archive_scraper {
            scraper.get_available_files().await
        } else {
            Err(anyhow::anyhow!("Archive scraper not initialized"))
        }
    }

    pub async fn cleanup_old_files(&self) -> Result<u64> {
        info!("Cleaning up old files...");

        let mut cleaned = 0u64;
        let download_dir = &self.config.download.download_dir;
        
        if !download_dir.exists() {
            return Ok(0);
        }

        let cutoff_time = SystemTime::now() - Duration::from_secs(7 * 24 * 3600); // 7 days

        let mut entries = tokio::fs::read_dir(download_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff_time {
                        if let Some(extension) = entry.path().extension() {
                            if extension == "gz" {
                                if tokio::fs::remove_file(entry.path()).await.is_ok() {
                                    cleaned += 1;
                                    debug!("Cleaned old file: {:?}", entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }

        info!("Cleaned {} old files", cleaned);
        Ok(cleaned)
    }

    pub fn get_scraper_manager(&self) -> Arc<ScraperManager> {
        self.scraper_manager.clone()
    }

    pub fn is_running(&self) -> bool {
        self.scraper_manager.is_running()
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.stop().await
    }
}
