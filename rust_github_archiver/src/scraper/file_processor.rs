use std::path::Path;
use std::time::Instant;
use std::collections::HashMap;
use flate2::read::GzDecoder;
use std::io::Read;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub batch_size: usize,
    pub max_memory_usage_mb: usize,
    pub enable_validation: bool,
    pub save_raw_data: bool,
    pub extract_metadata: bool,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            batch_size: 500,
            max_memory_usage_mb: 512,
            enable_validation: true,
            save_raw_data: false,
            extract_metadata: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessingResult {
    pub filename: String,
    pub total_events: u64,
    pub valid_events: u64,
    pub invalid_events: u64,
    pub processing_time_seconds: f64,
    pub file_size_bytes: u64,
    pub compression_ratio: f64,
    pub event_types: HashMap<String, u64>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubEvent {
    pub id: String,
    pub event_type: String,
    pub actor: Option<Value>,
    pub repo: Option<Value>,
    pub payload: Option<Value>,
    pub public: Option<bool>,
    pub created_at: Option<String>,
    pub org: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventBatch {
    pub events: Vec<GitHubEvent>,
    pub batch_id: String,
    pub source_file: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct FileProcessor {
    config: ProcessingConfig,
}

impl FileProcessor {
    pub fn new(config: ProcessingConfig) -> Self {
        Self { config }
    }

    pub async fn process_archive_file(&self, file_path: &Path) -> Result<ProcessingResult> {
        let start_time = Instant::now();
        let filename = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!("Processing archive file: {}", filename);

        // Read and decompress file
        let compressed_data = tokio::fs::read(file_path).await?;
        let file_size_bytes = compressed_data.len() as u64;
        
        let decompressed_data = self.decompress_gzip(&compressed_data)?;
        let compression_ratio = compressed_data.len() as f64 / decompressed_data.len() as f64;

        debug!("Decompressed {} -> {} bytes (ratio: {:.2})", 
               compressed_data.len(), decompressed_data.len(), compression_ratio);

        // Process events
        let (events, errors) = self.parse_events(&decompressed_data)?;
        let total_events = events.len() as u64;
        let valid_events = events.iter().filter(|e| self.validate_event(e)).count() as u64;
        let invalid_events = total_events - valid_events;

        // Count event types
        let mut event_types = HashMap::new();
        for event in &events {
            *event_types.entry(event.event_type.clone()).or_insert(0) += 1;
        }

        let processing_time = start_time.elapsed().as_secs_f64();

        info!("Processed {}: {} events ({} valid, {} invalid) in {:.2}s",
              filename, total_events, valid_events, invalid_events, processing_time);

        Ok(ProcessingResult {
            filename,
            total_events,
            valid_events,
            invalid_events,
            processing_time_seconds: processing_time,
            file_size_bytes,
            compression_ratio,
            event_types,
            errors,
        })
    }

    fn decompress_gzip(&self, compressed_data: &[u8]) -> Result<String> {
        let mut decoder = GzDecoder::new(compressed_data);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed)?;
        Ok(decompressed)
    }

    fn parse_events(&self, data: &str) -> Result<(Vec<GitHubEvent>, Vec<String>)> {
        let mut events = Vec::new();
        let mut errors = Vec::new();
        let mut line_number = 0;

        for line in data.lines() {
            line_number += 1;
            
            if line.trim().is_empty() {
                continue;
            }

            match self.parse_event_line(line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    let error_msg = format!("Line {}: {}", line_number, e);
                    errors.push(error_msg);
                    
                    if errors.len() > 100 {
                        errors.push("... (truncated, too many errors)".to_string());
                        break;
                    }
                }
            }

            // Memory usage check
            if events.len() % 10000 == 0 {
                debug!("Parsed {} events so far", events.len());
            }
        }

        Ok((events, errors))
    }

    fn parse_event_line(&self, line: &str) -> Result<GitHubEvent> {
        let json_value: Value = serde_json::from_str(line)?;
        
        let event = GitHubEvent {
            id: json_value.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            event_type: json_value.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            actor: json_value.get("actor").cloned(),
            repo: json_value.get("repo").cloned(),
            payload: json_value.get("payload").cloned(),
            public: json_value.get("public").and_then(|v| v.as_bool()),
            created_at: json_value.get("created_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            org: json_value.get("org").cloned(),
        };

        Ok(event)
    }

    fn validate_event(&self, event: &GitHubEvent) -> bool {
        if !self.config.enable_validation {
            return true;
        }

        // Basic validation rules
        if event.id.is_empty() {
            return false;
        }

        if event.event_type.is_empty() || event.event_type == "unknown" {
            return false;
        }

        // Validate created_at format
        if let Some(created_at) = &event.created_at {
            if chrono::DateTime::parse_from_rfc3339(created_at).is_err() {
                return false;
            }
        }

        true
    }

    pub async fn process_events_batch(
        &self,
        events: Vec<GitHubEvent>,
        source_file: &str,
    ) -> Result<EventBatch> {
        let batch_id = uuid::Uuid::new_v4().to_string();
        
        Ok(EventBatch {
            events,
            batch_id,
            source_file: source_file.to_string(),
            created_at: chrono::Utc::now(),
        })
    }

    pub fn extract_repository_info(&self, event: &GitHubEvent) -> Option<RepositoryInfo> {
        event.repo.as_ref().and_then(|repo| {
            let name = repo.get("name").and_then(|v| v.as_str())?;
            let id = repo.get("id").and_then(|v| v.as_u64())?;
            let url = repo.get("url").and_then(|v| v.as_str())?;

            Some(RepositoryInfo {
                id,
                name: name.to_string(),
                url: url.to_string(),
                full_name: name.to_string(), // GitHub repos have full name same as name in this context
            })
        })
    }

    pub fn extract_actor_info(&self, event: &GitHubEvent) -> Option<ActorInfo> {
        event.actor.as_ref().and_then(|actor| {
            let id = actor.get("id").and_then(|v| v.as_u64())?;
            let login = actor.get("login").and_then(|v| v.as_str())?;
            let display_login = actor.get("display_login").and_then(|v| v.as_str());
            let gravatar_id = actor.get("gravatar_id").and_then(|v| v.as_str());
            let url = actor.get("url").and_then(|v| v.as_str());
            let avatar_url = actor.get("avatar_url").and_then(|v| v.as_str());

            Some(ActorInfo {
                id,
                login: login.to_string(),
                display_login: display_login.map(|s| s.to_string()),
                gravatar_id: gravatar_id.map(|s| s.to_string()),
                url: url.map(|s| s.to_string()),
                avatar_url: avatar_url.map(|s| s.to_string()),
            })
        })
    }

    pub fn get_config(&self) -> &ProcessingConfig {
        &self.config
    }

    pub async fn validate_archive_integrity(&self, file_path: &Path) -> Result<bool> {
        // Check if file exists and is readable
        if !file_path.exists() {
            return Ok(false);
        }

        // Try to read and decompress the file
        match tokio::fs::read(file_path).await {
            Ok(data) => {
                match self.decompress_gzip(&data) {
                    Ok(decompressed) => {
                        // Try to parse at least one event
                        for line in decompressed.lines().take(10) {
                            if line.trim().is_empty() {
                                continue;
                            }
                            
                            if serde_json::from_str::<Value>(line).is_ok() {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RepositoryInfo {
    pub id: u64,
    pub name: String,
    pub url: String,
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActorInfo {
    pub id: u64,
    pub login: String,
    pub display_login: Option<String>,
    pub gravatar_id: Option<String>,
    pub url: Option<String>,
    pub avatar_url: Option<String>,
}

impl Default for FileProcessor {
    fn default() -> Self {
        Self::new(ProcessingConfig::default())
    }
}
