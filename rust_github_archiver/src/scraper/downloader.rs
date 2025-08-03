use std::path::Path;
use std::time::{Duration, Instant};
use reqwest::Client;
use tokio::fs::{File, create_dir_all};
use tokio::io::AsyncWriteExt;
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use tracing::{info, warn, error, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub max_concurrent_downloads: usize,
    pub chunk_size: usize,
    pub request_timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_delay_seconds: f64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 6,
            chunk_size: 4096,
            request_timeout_seconds: 180,
            max_retries: 3,
            retry_delay_seconds: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadResult {
    pub url: String,
    pub local_path: String,
    pub size_bytes: u64,
    pub duration_seconds: f64,
    pub status: DownloadStatus,
    pub error: Option<String>,
    pub retries_used: u32,
}

#[derive(Debug, Clone, Serialize)]
pub enum DownloadStatus {
    Success,
    Failed,
    Skipped,
}

pub struct Downloader {
    client: Client,
    config: DownloadConfig,
}

impl Downloader {
    pub fn new(config: DownloadConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .build()?;

        Ok(Self { client, config })
    }

    pub async fn download_file(
        &self,
        url: &str,
        local_path: &Path,
        expected_size: Option<u64>,
    ) -> Result<DownloadResult> {
        let start_time = Instant::now();
        let mut retries_used = 0;

        // Ensure parent directory exists
        if let Some(parent) = local_path.parent() {
            create_dir_all(parent).await?;
        }

        // Check if file already exists and has correct size
        if local_path.exists() {
            if let Ok(metadata) = tokio::fs::metadata(local_path).await {
                if let Some(expected) = expected_size {
                    if metadata.len() == expected {
                        debug!("File already exists with correct size: {}", local_path.display());
                        return Ok(DownloadResult {
                            url: url.to_string(),
                            local_path: local_path.to_string_lossy().to_string(),
                            size_bytes: metadata.len(),
                            duration_seconds: start_time.elapsed().as_secs_f64(),
                            status: DownloadStatus::Skipped,
                            error: None,
                            retries_used: 0,
                        });
                    }
                }
            }
        }

        let mut last_error = None;

        // Retry loop
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                retries_used += 1;
                let delay = Duration::from_secs_f64(self.config.retry_delay_seconds * attempt as f64);
                warn!("Retrying download attempt {} after {:?}: {}", attempt, delay, url);
                tokio::time::sleep(delay).await;
            }

            match self.download_attempt(url, local_path).await {
                Ok(size) => {
                    info!("Successfully downloaded {} ({} bytes) in {:.2}s", 
                          url, size, start_time.elapsed().as_secs_f64());
                    
                    return Ok(DownloadResult {
                        url: url.to_string(),
                        local_path: local_path.to_string_lossy().to_string(),
                        size_bytes: size,
                        duration_seconds: start_time.elapsed().as_secs_f64(),
                        status: DownloadStatus::Success,
                        error: None,
                        retries_used,
                    });
                }
                Err(e) => {
                    error!("Download attempt {} failed for {}: {}", attempt + 1, url, e);
                    last_error = Some(e);
                    
                    // Clean up partial file
                    if local_path.exists() {
                        let _ = tokio::fs::remove_file(local_path).await;
                    }
                }
            }
        }

        // All retries failed
        let error_msg = last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Unknown error".to_string());

        Ok(DownloadResult {
            url: url.to_string(),
            local_path: local_path.to_string_lossy().to_string(),
            size_bytes: 0,
            duration_seconds: start_time.elapsed().as_secs_f64(),
            status: DownloadStatus::Failed,
            error: Some(error_msg),
            retries_used,
        })
    }

    async fn download_attempt(&self, url: &str, local_path: &Path) -> Result<u64> {
        debug!("Starting download: {} -> {}", url, local_path.display());

        // Send request
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP error: {}", response.status()));
        }

        // Get content length
        let content_length = response.content_length();

        // Create file
        let mut file = File::create(local_path).await?;
        let mut bytes_written = 0u64;

        // Stream download
        let mut stream = response.bytes_stream();
        use futures::StreamExt;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;
            bytes_written += chunk.len() as u64;

            // Optional: Progress reporting for large files
            if let Some(total) = content_length {
                if total > 10_000_000 && bytes_written % 1_000_000 == 0 {
                    debug!("Downloaded {}/{} MB", bytes_written / 1_000_000, total / 1_000_000);
                }
            }
        }

        // Ensure file is flushed
        file.flush().await?;
        drop(file);

        // Verify download size if known
        if let Some(expected) = content_length {
            if bytes_written != expected {
                return Err(anyhow!(
                    "Download size mismatch: got {} bytes, expected {}",
                    bytes_written,
                    expected
                ));
            }
        }

        Ok(bytes_written)
    }

    pub async fn download_multiple(
        &self,
        downloads: Vec<(String, std::path::PathBuf)>,
    ) -> Vec<DownloadResult> {
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrent_downloads));
        let mut tasks = Vec::new();

        for (url, path) in downloads {
            let semaphore = semaphore.clone();
            let downloader = self;
            
            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                downloader.download_file(&url, &path, None).await
            });
            
            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    error!("Download task error: {}", e);
                    // Create a failed result
                    results.push(DownloadResult {
                        url: "unknown".to_string(),
                        local_path: "unknown".to_string(),
                        size_bytes: 0,
                        duration_seconds: 0.0,
                        status: DownloadStatus::Failed,
                        error: Some(e.to_string()),
                        retries_used: 0,
                    });
                }
                Err(e) => {
                    error!("Task join error: {}", e);
                    results.push(DownloadResult {
                        url: "unknown".to_string(),
                        local_path: "unknown".to_string(),
                        size_bytes: 0,
                        duration_seconds: 0.0,
                        status: DownloadStatus::Failed,
                        error: Some(e.to_string()),
                        retries_used: 0,
                    });
                }
            }
        }

        results
    }

    pub fn get_config(&self) -> &DownloadConfig {
        &self.config
    }

    pub async fn estimate_download_time(&self, url: &str) -> Result<Duration> {
        // Send HEAD request to get content length
        let response = self.client.head(url).send().await?;
        
        if let Some(content_length) = response.content_length() {
            // Rough estimate: assume 1 MB/s download speed
            let estimated_seconds = content_length / 1_000_000;
            Ok(Duration::from_secs(estimated_seconds.max(1)))
        } else {
            // Default estimate for unknown size
            Ok(Duration::from_secs(30))
        }
    }

    pub async fn check_url_availability(&self, url: &str) -> Result<bool> {
        match self.client.head(url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}
