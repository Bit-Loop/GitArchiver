use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use reqwest::Client;
use tokio::time;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};
use flate2::read::GzDecoder;
use std::io::Read;
use tracing::{info, warn, error, debug};

use crate::core::{Config, ResourceMonitor, ResourceLimits};
use crate::scraper::{ScraperManager, ScraperState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveFile {
    pub filename: String,
    pub url: String,
    pub last_modified: Option<String>,
    pub size: u64,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    pub filename: String,
    pub status: String,
    pub events_processed: u64,
    pub file_size: u64,
    pub processing_time: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScrapingStats {
    pub start_time: Option<f64>,
    pub files_processed: u64,
    pub events_processed: u64,
    pub errors_encountered: u64,
    pub last_activity: f64,
    pub processing_rate: f64,
}

impl Default for ScrapingStats {
    fn default() -> Self {
        Self {
            start_time: None,
            files_processed: 0,
            events_processed: 0,
            errors_encountered: 0,
            last_activity: 0.0,
            processing_rate: 0.0,
        }
    }
}

pub struct ArchiveScraper {
    config: Config,
    client: Client,
    stats: Arc<Mutex<ScrapingStats>>,
    resource_monitor: Arc<Mutex<ResourceMonitor>>,
    scraper_manager: Arc<ScraperManager>,
    shutdown_requested: Arc<Mutex<bool>>,
}

impl ArchiveScraper {
    pub fn new(config: Config, scraper_manager: Arc<ScraperManager>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("Failed to create HTTP client");

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

        let resource_monitor = Arc::new(Mutex::new(ResourceMonitor::new(resource_limits)));

        Self {
            config,
            client,
            stats: Arc::new(Mutex::new(ScrapingStats::default())),
            resource_monitor,
            scraper_manager,
            shutdown_requested: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing archive scraper...");
        
        // Create download directory
        tokio::fs::create_dir_all(&self.config.download.download_dir).await?;
        
        info!("Archive scraper initialized successfully");
        Ok(())
    }

    pub async fn get_available_files(&self) -> Result<Vec<ArchiveFile>> {
        info!("Fetching available archive files...");
        
        let response = self.client
            .get("https://data.gharchive.org/?list-type=2")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch file list: HTTP {}", response.status()));
        }

        let content = response.text().await?;
        
        // Parse XML response (simplified - in production you'd use a proper XML parser)
        let mut files = Vec::new();
        
        // This is a simplified XML parsing - you'd use quick-xml or similar in production
        for line in content.lines() {
            if line.contains("<Key>") && line.contains(".json.gz</Key>") {
                let start = line.find("<Key>").unwrap() + 5;
                let end = line.find("</Key>").unwrap();
                let filename = &line[start..end];
                
                // Extract size if available
                let size = if let Some(size_line) = content.lines()
                    .skip_while(|l| !l.contains(&format!("<Key>{}</Key>", filename)))
                    .find(|l| l.contains("<Size>")) {
                    if let (Some(start), Some(end)) = (size_line.find("<Size>"), size_line.find("</Size>")) {
                        size_line[start + 6..end].parse().unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                };

                files.push(ArchiveFile {
                    filename: filename.to_string(),
                    url: format!("https://data.gharchive.org/{}", filename),
                    last_modified: None, // Would extract from XML in production
                    size,
                    etag: None, // Would extract from XML in production
                });
            }
        }

        files.sort_by(|a, b| a.filename.cmp(&b.filename));
        info!("Found {} archive files", files.len());
        
        Ok(files)
    }

    pub async fn process_file(&self, file_info: &ArchiveFile) -> Result<ProcessingResult> {
        let start_time = Instant::now();
        
        debug!("Processing file: {}", file_info.filename);
        
        // Download file
        let response = self.client.get(&file_info.url).send().await?;
        
        if !response.status().is_success() {
            return Ok(ProcessingResult {
                filename: file_info.filename.clone(),
                status: "failed".to_string(),
                events_processed: 0,
                file_size: file_info.size,
                processing_time: start_time.elapsed().as_secs_f64(),
                error: Some(format!("HTTP {}", response.status())),
            });
        }

        let compressed_data = response.bytes().await?;
        
        // Decompress data
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut decompressed_data = String::new();
        decoder.read_to_string(&mut decompressed_data)?;
        
        // Process events
        let mut events_processed = 0u64;
        let lines: Vec<&str> = decompressed_data.lines().collect();
        
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(_event) => {
                    // In production, you'd process and store the event
                    events_processed += 1;
                    
                    // Update stats periodically
                    if events_processed % 1000 == 0 {
                        if let Ok(mut stats) = self.stats.lock() {
                            stats.events_processed += 1000;
                            stats.last_activity = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs_f64();
                        }
                        
                        // Update scraper manager progress
                        let _ = self.scraper_manager.update_progress(
                            events_processed,
                            1, // files_processed
                            Some(file_info.filename.clone())
                        );
                    }
                }
                Err(e) => {
                    warn!("Invalid JSON in {}: {}", file_info.filename, e);
                    if let Ok(mut stats) = self.stats.lock() {
                        stats.errors_encountered += 1;
                    }
                    let _ = self.scraper_manager.add_error();
                }
            }
            
            // Check for shutdown request
            if let Ok(shutdown) = self.shutdown_requested.lock() {
                if *shutdown {
                    info!("Shutdown requested, stopping file processing");
                    break;
                }
            }
        }
        
        // Update final stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.files_processed += 1;
            stats.last_activity = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
        }

        let processing_time = start_time.elapsed().as_secs_f64();
        
        info!("Successfully processed {}: {} events in {:.2}s", 
              file_info.filename, events_processed, processing_time);

        Ok(ProcessingResult {
            filename: file_info.filename.clone(),
            status: "success".to_string(),
            events_processed,
            file_size: file_info.size,
            processing_time,
            error: None,
        })
    }

    pub async fn run_continuous_scraping(&self) -> Result<()> {
        info!("Starting continuous scraping...");
        
        // Initialize stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.start_time = Some(SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64());
        }

        // Main scraping loop
        loop {
            // Check if scraper should be running
            if !self.scraper_manager.is_running() {
                debug!("Scraper not running, waiting...");
                time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            // Check for shutdown
            if let Ok(shutdown) = self.shutdown_requested.lock() {
                if *shutdown {
                    info!("Shutdown requested, stopping scraping");
                    break;
                }
            }

            // Check resource status
            if let Ok(mut monitor) = self.resource_monitor.lock() {
                match monitor.get_resource_status().await {
                    Ok(status) => {
                        if status.emergency_mode {
                            warn!("Emergency mode activated: {:?}", status.emergency_conditions);
                            if let Err(e) = monitor.emergency_cleanup().await {
                                error!("Emergency cleanup failed: {}", e);
                            }
                            // Pause for a while to let system recover
                            time::sleep(Duration::from_secs(60)).await;
                            continue;
                        }
                    }
                    Err(e) => {
                        error!("Resource monitoring error: {}", e);
                    }
                }
            }

            // Get available files
            match self.get_available_files().await {
                Ok(available_files) => {
                    info!("Processing {} files", available_files.len());
                    
                    // Process files in batches
                    let batch_size = 10; // Configurable
                    let max_concurrent = 3; // Configurable

                    for batch in available_files.chunks(batch_size) {
                        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
                        let mut tasks = Vec::new();

                        for file_info in batch {
                            let semaphore = Arc::clone(&semaphore);
                            let scraper = self;
                            let file_info = file_info.clone();
                            
                            let task = tokio::spawn(async move {
                                let _permit = semaphore.acquire().await.unwrap();
                                scraper.process_file(&file_info).await
                            });
                            
                            tasks.push(task);
                        }

                        // Wait for batch to complete
                        let results = futures::future::join_all(tasks).await;
                        
                        let mut successful = 0;
                        for result in results {
                            match result {
                                Ok(Ok(process_result)) => {
                                    if process_result.status == "success" {
                                        successful += 1;
                                    }
                                }
                                Ok(Err(e)) => {
                                    error!("File processing error: {}", e);
                                }
                                Err(e) => {
                                    error!("Task join error: {}", e);
                                }
                            }
                        }

                        info!("Batch completed: {}/{} files processed successfully", 
                              successful, batch.len());

                        // Brief pause between batches
                        time::sleep(Duration::from_secs(2)).await;
                        
                        // Check if we should stop
                        if !self.scraper_manager.is_running() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get available files: {}", e);
                    if let Ok(mut stats) = self.stats.lock() {
                        stats.errors_encountered += 1;
                    }
                }
            }

            // Pause before next iteration
            time::sleep(Duration::from_secs(300)).await; // 5 minutes
        }

        info!("Continuous scraping stopped");
        Ok(())
    }

    pub async fn get_stats(&self) -> Result<ScrapingStats> {
        if let Ok(stats) = self.stats.lock() {
            Ok(stats.clone())
        } else {
            Err(anyhow!("Failed to acquire stats lock"))
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down archive scraper...");
        
        if let Ok(mut shutdown) = self.shutdown_requested.lock() {
            *shutdown = true;
        }
        
        info!("Archive scraper shutdown complete");
        Ok(())
    }
}
