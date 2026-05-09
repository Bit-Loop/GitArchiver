use anyhow::{anyhow, Result};
use async_compression::tokio::bufread::GzipDecoder;
use futures::TryStreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time;
use tokio_util::io::StreamReader;
use tracing::{debug, error, info, warn};

use crate::core::{Config, ResourceLimits, ResourceMonitor};
use crate::scraper::ScraperManager;

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

/// Persisted index of processed GHArchive files to avoid reprocessing.
#[derive(Debug)]
struct ProcessedIndex {
    path: PathBuf,
    files: HashSet<String>,
}

impl ProcessedIndex {
    fn load(path: PathBuf) -> Result<Self> {
        let files = if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashSet::new()
        };

        Ok(Self { path, files })
    }

    fn contains(&self, filename: &str) -> bool {
        self.files.contains(filename)
    }

    fn insert(&mut self, filename: &str) -> bool {
        self.files.insert(filename.to_string())
    }

    async fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let data = serde_json::to_string(&self.files)?;
        tokio::fs::write(&self.path, data).await?;
        Ok(())
    }
}

pub struct ArchiveScraper {
    config: Config,
    client: Client,
    stats: Arc<AsyncMutex<ScrapingStats>>,
    resource_monitor: Arc<AsyncMutex<ResourceMonitor>>,
    scraper_manager: Arc<ScraperManager>,
    shutdown_requested: Arc<AsyncMutex<bool>>,
    processed_index: Arc<AsyncMutex<ProcessedIndex>>,
}

impl ArchiveScraper {
    pub fn new(config: Config, scraper_manager: Arc<ScraperManager>) -> Result<Self> {
        let resource_limits: ResourceLimits = (&config.resources).into();
        let resource_monitor = Arc::new(AsyncMutex::new(ResourceMonitor::new(resource_limits)));
        Self::with_resource_monitor(config, scraper_manager, resource_monitor)
    }

    pub fn with_resource_monitor(
        config: Config,
        scraper_manager: Arc<ScraperManager>,
        resource_monitor: Arc<AsyncMutex<ResourceMonitor>>,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;

        let processed_index_path = config.download.download_dir.join("processed_files.json");
        let processed_index =
            ProcessedIndex::load(processed_index_path).unwrap_or_else(|_| ProcessedIndex {
                path: PathBuf::from("./processed_files.json"),
                files: HashSet::new(),
            });
        let processed_index = Arc::new(AsyncMutex::new(processed_index));

        Ok(Self {
            config,
            client,
            stats: Arc::new(AsyncMutex::new(ScrapingStats::default())),
            resource_monitor,
            scraper_manager,
            shutdown_requested: Arc::new(AsyncMutex::new(false)),
            processed_index,
        })
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

        let response = self
            .client
            .get(&self.config.download.s3_list_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch file list: HTTP {}",
                response.status()
            ));
        }

        let content = response.text().await?;

        let mut files = parse_archive_listing(&content, &self.config.download.base_url);
        let original_len = files.len();

        // Optionally restrict to the most recent N hours to avoid scanning huge backlogs
        if let Some(recent_hours) = self.config.download.recent_hours {
            let keep = recent_hours as usize;
            if files.len() > keep {
                files = files.into_iter().rev().take(keep).collect();
                files.reverse(); // maintain ascending order after taking most recent
            }
            info!(
                "Applying recent_hours cap: keeping {}/{} most recent files",
                files.len(),
                original_len
            );
        }

        info!("Found {} archive files", files.len());

        Ok(files)
    }

    async fn filter_unprocessed_files(
        &self,
        available_files: Vec<ArchiveFile>,
    ) -> Result<Vec<ArchiveFile>> {
        let index = self.processed_index.lock().await;
        let unprocessed = available_files
            .into_iter()
            .filter(|file| !index.contains(&file.filename))
            .collect();
        Ok(unprocessed)
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

        // Stream and decompress to avoid loading entire file in memory
        let stream = response.bytes_stream().map_err(std::io::Error::other);
        let stream_reader = StreamReader::new(stream);
        let decoder = GzipDecoder::new(BufReader::new(stream_reader));
        let mut lines = BufReader::new(decoder).lines();

        let mut events_processed = 0u64;

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(_event) => {
                    events_processed += 1;

                    // Update stats periodically
                    if events_processed.is_multiple_of(1000) {
                        {
                            let mut stats = self.stats.lock().await;
                            stats.events_processed += 1000;
                            stats.last_activity = unix_timestamp_seconds();
                        }

                        // Update scraper manager progress
                        let _ = self.scraper_manager.update_progress(
                            events_processed,
                            1, // files_processed
                            Some(file_info.filename.clone()),
                        );
                    }
                }
                Err(e) => {
                    warn!("Invalid JSON in {}: {}", file_info.filename, e);
                    {
                        let mut stats = self.stats.lock().await;
                        stats.errors_encountered += 1;
                    }
                    let _ = self.scraper_manager.add_error();
                }
            }

            // Check for shutdown request
            let shutdown = self.shutdown_requested.lock().await;
            if *shutdown {
                info!("Shutdown requested, stopping file processing");
                break;
            }
        }

        // Update final stats
        {
            let mut stats = self.stats.lock().await;
            stats.files_processed += 1;
            stats.last_activity = unix_timestamp_seconds();
            // Add any remaining events that were not captured in the 1000-event updates
            stats.events_processed += events_processed % 1000;
        }

        let processing_time = start_time.elapsed().as_secs_f64();

        info!(
            "Successfully processed {}: {} events in {:.2}s",
            file_info.filename, events_processed, processing_time
        );

        // Persist processed file marker so we do not reprocess on the next loop
        if events_processed > 0 {
            let mut index = self.processed_index.lock().await;
            if index.insert(&file_info.filename) {
                if let Err(err) = index.persist().await {
                    warn!("Failed to persist processed index: {}", err);
                }
            }
        }

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
        {
            let mut stats = self.stats.lock().await;
            stats.start_time = Some(unix_timestamp_seconds());
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
            {
                let shutdown = self.shutdown_requested.lock().await;
                if *shutdown {
                    info!("Shutdown requested, stopping scraping");
                    break;
                }
            }

            // Check resource status
            let mut monitor = self.resource_monitor.lock().await;
            match monitor.get_resource_status().await {
                Ok(status) => {
                    if status.emergency_mode {
                        warn!(
                            "Emergency mode activated: {:?}",
                            status.emergency_conditions
                        );
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

            // Get available files
            match self.get_available_files().await {
                Ok(available_files) => {
                    let unprocessed_files = self.filter_unprocessed_files(available_files).await?;

                    if unprocessed_files.is_empty() {
                        info!("No new archive files to process; sleeping");
                        time::sleep(Duration::from_secs(60)).await;
                        continue;
                    }

                    // Optionally cap how many files we process per cycle
                    let target_files: Vec<ArchiveFile> =
                        if let Some(cap) = self.config.download.max_files_per_cycle {
                            let cap = cap as usize;
                            unprocessed_files.into_iter().take(cap).collect()
                        } else {
                            unprocessed_files
                        };

                    info!("Processing {} new files", target_files.len());

                    // Process files in batches, honoring configured limits
                    let batch_size = std::cmp::max(1, self.config.download.batch_size as usize);
                    let max_concurrent =
                        (self.config.download.max_concurrent_downloads as usize).clamp(1, 32);

                    for batch in target_files.chunks(batch_size) {
                        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
                        let mut tasks = Vec::with_capacity(batch.len());

                        for file_info in batch {
                            let semaphore = Arc::clone(&semaphore);
                            let client = self.client.clone();
                            let config = self.config.clone();
                            let stats = Arc::clone(&self.stats);
                            let resource_monitor = Arc::clone(&self.resource_monitor);
                            let scraper_manager = Arc::clone(&self.scraper_manager);
                            let shutdown_requested = Arc::clone(&self.shutdown_requested);
                            let processed_index = Arc::clone(&self.processed_index);
                            let file_info = file_info.clone();

                            let task = tokio::spawn(async move {
                                let _permit = semaphore
                                    .acquire()
                                    .await
                                    .map_err(|_| anyhow!("archive processing semaphore closed"))?;

                                // Create a temporary scraper instance for this task
                                let temp_scraper = ArchiveScraper {
                                    config,
                                    client,
                                    stats,
                                    resource_monitor,
                                    scraper_manager,
                                    shutdown_requested,
                                    processed_index,
                                };

                                temp_scraper.process_file(&file_info).await
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

                        info!(
                            "Batch completed: {}/{} files processed successfully",
                            successful,
                            batch.len()
                        );

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
                    {
                        let mut stats = self.stats.lock().await;
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
        let stats = self.stats.lock().await;
        Ok(stats.clone())
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down archive scraper...");

        {
            let mut shutdown = self.shutdown_requested.lock().await;
            *shutdown = true;
        }

        info!("Archive scraper shutdown complete");
        Ok(())
    }
}

fn unix_timestamp_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn parse_archive_listing(content: &str, base_url: &str) -> Vec<ArchiveFile> {
    let estimated_files = content.matches("<Contents>").count().max(1000);
    let mut files = Vec::with_capacity(estimated_files);
    let mut remaining = content;

    while let Some(start) = remaining.find("<Contents>") {
        let entry_start = start + "<Contents>".len();
        let after_start = &remaining[entry_start..];
        let Some(end) = after_start.find("</Contents>") else {
            warn!("Malformed XML archive listing: missing </Contents> closing tag");
            break;
        };

        let entry = &after_start[..end];
        if let Some(file) = parse_archive_entry(entry, base_url) {
            files.push(file);
        }

        remaining = &after_start[end + "</Contents>".len()..];
    }

    if files.is_empty() {
        for line in content.lines() {
            if let Some(file) = parse_archive_entry(line, base_url) {
                files.push(file);
            }
        }
    }

    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    files
}

fn parse_archive_entry(entry: &str, base_url: &str) -> Option<ArchiveFile> {
    let filename = extract_xml_tag(entry, "Key")?;
    if !filename.ends_with(".json.gz") {
        return None;
    }

    let size = extract_xml_tag(entry, "Size")
        .and_then(|size| size.parse::<u64>().ok())
        .unwrap_or(0);
    let last_modified = extract_xml_tag(entry, "LastModified");
    let etag = extract_xml_tag(entry, "ETag").map(|etag| etag.trim_matches('"').to_string());

    Some(ArchiveFile {
        url: archive_file_url(base_url, &filename),
        filename,
        last_modified,
        size,
        etag,
    })
}

fn extract_xml_tag(entry: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = entry.find(&open)? + open.len();
    let end = entry[start..].find(&close)? + start;
    let value = entry[start..end].trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn archive_file_url(base_url: &str, filename: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        filename.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scraper::ScraperManager;
    use tempfile::tempdir;

    fn archive_file(filename: &str) -> ArchiveFile {
        ArchiveFile {
            filename: filename.to_string(),
            url: format!("https://data.gharchive.org/{filename}"),
            last_modified: None,
            size: 10,
            etag: None,
        }
    }

    #[test]
    fn parse_archive_listing_uses_configured_base_url_and_s3_metadata() {
        let xml = r#"
            <ListBucketResult>
              <Contents>
                <Key>2026-05-09-00.json.gz</Key>
                <LastModified>2026-05-09T00:05:00.000Z</LastModified>
                <ETag>"abc123"</ETag>
                <Size>1234</Size>
              </Contents>
              <Contents>
                <Key>readme.txt</Key>
                <Size>4</Size>
              </Contents>
              <Contents>
                <Key>2026-05-09-01.json.gz</Key>
                <LastModified>2026-05-09T01:05:00.000Z</LastModified>
                <ETag>"def456"</ETag>
                <Size>5678</Size>
              </Contents>
            </ListBucketResult>
        "#;

        let files = parse_archive_listing(xml, "https://mirror.example/archives/");

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].filename, "2026-05-09-00.json.gz");
        assert_eq!(
            files[0].url,
            "https://mirror.example/archives/2026-05-09-00.json.gz"
        );
        assert_eq!(
            files[0].last_modified.as_deref(),
            Some("2026-05-09T00:05:00.000Z")
        );
        assert_eq!(files[0].etag.as_deref(), Some("abc123"));
        assert_eq!(files[0].size, 1234);
        assert_eq!(files[1].size, 5678);
    }

    #[test]
    fn archive_file_url_normalizes_slashes() {
        assert_eq!(
            archive_file_url("https://data.gharchive.org/", "/2026-05-09-00.json.gz"),
            "https://data.gharchive.org/2026-05-09-00.json.gz"
        );
    }

    #[tokio::test]
    async fn processed_index_persists_and_reloads_processed_files() {
        let dir = tempdir().expect("tempdir");
        let index_path = dir.path().join("processed.json");
        let mut index = ProcessedIndex::load(index_path.clone()).expect("load empty index");

        assert!(!index.contains("2026-05-09-12.json.gz"));
        assert!(index.insert("2026-05-09-12.json.gz"));
        assert!(!index.insert("2026-05-09-12.json.gz"));
        index.persist().await.expect("persist index");

        let reloaded = ProcessedIndex::load(index_path).expect("reload index");
        assert!(reloaded.contains("2026-05-09-12.json.gz"));
    }

    #[tokio::test]
    async fn filter_unprocessed_files_removes_known_archive_names() {
        let dir = tempdir().expect("tempdir");
        let mut config = Config::default();
        config.download.download_dir = dir.path().to_path_buf();
        let scraper =
            ArchiveScraper::new(config, Arc::new(ScraperManager::new())).expect("archive scraper");

        {
            let mut index = scraper.processed_index.lock().await;
            assert!(index.insert("processed.json.gz"));
            index.persist().await.expect("persist");
        }

        let files = vec![
            archive_file("processed.json.gz"),
            archive_file("new.json.gz"),
            archive_file("also-new.json.gz"),
        ];
        let unprocessed = scraper
            .filter_unprocessed_files(files)
            .await
            .expect("filter");
        let names: Vec<_> = unprocessed.into_iter().map(|file| file.filename).collect();

        assert_eq!(names, vec!["new.json.gz", "also-new.json.gz"]);
    }

    #[tokio::test]
    async fn shutdown_sets_stop_flag_without_requiring_network() {
        let dir = tempdir().expect("tempdir");
        let mut config = Config::default();
        config.download.download_dir = dir.path().to_path_buf();
        let scraper =
            ArchiveScraper::new(config, Arc::new(ScraperManager::new())).expect("archive scraper");

        scraper.shutdown().await.expect("shutdown");

        assert!(*scraper.shutdown_requested.lock().await);
    }

    #[test]
    fn unix_timestamp_seconds_is_epoch_based() {
        assert!(unix_timestamp_seconds() > 1_700_000_000.0);
    }
}
