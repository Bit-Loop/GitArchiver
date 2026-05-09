use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::time::Instant;
use tracing::{debug, info};

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
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!("Processing archive file: {}", filename);

        // Read and decompress file
        let compressed_data = tokio::fs::read(file_path).await?;
        let file_size_bytes = compressed_data.len() as u64;

        let decompressed_data = self.decompress_gzip(&compressed_data)?;
        let compression_ratio = if decompressed_data.is_empty() {
            0.0
        } else {
            compressed_data.len() as f64 / decompressed_data.len() as f64
        };

        debug!(
            "Decompressed {} -> {} bytes (ratio: {:.2})",
            compressed_data.len(),
            decompressed_data.len(),
            compression_ratio
        );

        // Process events
        let (events, errors) = self.parse_events(&decompressed_data)?;
        let total_events = events.len() as u64;
        let valid_events = events.iter().filter(|e| self.validate_event(e)).count() as u64;
        let invalid_events = total_events - valid_events;

        // Count event types - pre-allocate based on typical event type count (~15-20 types)
        let mut event_types = HashMap::with_capacity(20);
        for event in &events {
            *event_types.entry(event.event_type.clone()).or_insert(0) += 1;
        }

        let processing_time = start_time.elapsed().as_secs_f64();

        info!(
            "Processed {}: {} events ({} valid, {} invalid) in {:.2}s",
            filename, total_events, valid_events, invalid_events, processing_time
        );

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
        // Pre-allocate based on average hourly events (~3000-5000 events per file)
        let estimated_events = data.len() / 300; // Rough estimate: ~300 bytes per JSON line
        let mut events = Vec::with_capacity(estimated_events.min(10000));
        let mut errors = Vec::with_capacity(100); // Max 100 errors before truncation
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
        if !json_value.is_object() {
            return Err(anyhow!("event line must be a JSON object"));
        }

        let event = GitHubEvent {
            id: json_value
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            event_type: json_value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            actor: json_value.get("actor").cloned(),
            repo: json_value.get("repo").cloned(),
            payload: json_value.get("payload").cloned(),
            public: json_value.get("public").and_then(|v| v.as_bool()),
            created_at: json_value
                .get("created_at")
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

        if !event.actor.as_ref().is_some_and(Value::is_object) {
            return false;
        }

        if !event.repo.as_ref().is_some_and(Value::is_object) {
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

                            if let Ok(event) = self.parse_event_line(line) {
                                if self.validate_event(&event) {
                                    return Ok(true);
                                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use serde_json::json;
    use std::io::Write;
    use tempfile::tempdir;

    fn gzip(data: &str) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data.as_bytes()).expect("write gzip");
        encoder.finish().expect("finish gzip")
    }

    fn sample_event(id: &str, event_type: &str) -> String {
        json!({
            "id": id,
            "type": event_type,
            "actor": {
                "id": 7,
                "login": "octocat",
                "display_login": "octocat",
                "url": "https://api.github.com/users/octocat"
            },
            "repo": {
                "id": 42,
                "name": "owner/repo",
                "url": "https://api.github.com/repos/owner/repo"
            },
            "public": true,
            "created_at": "2026-05-09T12:00:00Z"
        })
        .to_string()
    }

    #[tokio::test]
    async fn process_archive_file_counts_valid_invalid_and_event_types() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("events.json.gz");
        let payload = format!(
            "{}\n{}\n{}\n",
            sample_event("1", "PushEvent"),
            r#"{"id":"","type":"PushEvent","created_at":"2026-05-09T12:00:00Z"}"#,
            "not-json"
        );
        tokio::fs::write(&file, gzip(&payload))
            .await
            .expect("write");

        let processor = FileProcessor::default();
        let result = processor
            .process_archive_file(&file)
            .await
            .expect("process archive");

        assert_eq!(result.filename, "events.json.gz");
        assert_eq!(result.total_events, 2);
        assert_eq!(result.valid_events, 1);
        assert_eq!(result.invalid_events, 1);
        assert_eq!(result.event_types.get("PushEvent"), Some(&2));
        assert_eq!(result.errors.len(), 1);
    }

    #[tokio::test]
    async fn process_archive_file_handles_empty_decompressed_archive() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("empty.json.gz");
        tokio::fs::write(&file, gzip("")).await.expect("write");

        let processor = FileProcessor::default();
        let result = processor
            .process_archive_file(&file)
            .await
            .expect("process archive");

        assert_eq!(result.total_events, 0);
        assert_eq!(result.valid_events, 0);
        assert_eq!(result.invalid_events, 0);
        assert_eq!(result.compression_ratio, 0.0);
    }

    #[test]
    fn parse_event_line_extracts_repository_and_actor_metadata() {
        let processor = FileProcessor::default();
        let event = processor
            .parse_event_line(&sample_event("99", "CreateEvent"))
            .expect("event");

        let repository = processor
            .extract_repository_info(&event)
            .expect("repository");
        let actor = processor.extract_actor_info(&event).expect("actor");

        assert_eq!(event.id, "99");
        assert_eq!(event.event_type, "CreateEvent");
        assert_eq!(repository.id, 42);
        assert_eq!(repository.full_name, "owner/repo");
        assert_eq!(actor.id, 7);
        assert_eq!(actor.login, "octocat");
    }

    #[test]
    fn parse_event_line_rejects_non_object_json() {
        let processor = FileProcessor::default();

        let err = processor
            .parse_event_line(r#""not an event object""#)
            .expect_err("scalar JSON should not be accepted as an event");

        assert!(err.to_string().contains("JSON object"));
    }

    #[test]
    fn validation_requires_actor_and_repo_objects() {
        let processor = FileProcessor::default();
        let mut event = processor
            .parse_event_line(&sample_event("99", "CreateEvent"))
            .expect("event");

        event.actor = None;
        assert!(!processor.validate_event(&event));

        event.actor = Some(json!({"id": 7, "login": "octocat"}));
        event.repo = Some(json!("owner/repo"));
        assert!(!processor.validate_event(&event));
    }

    #[test]
    fn validation_can_be_disabled_for_partial_events() {
        let processor = FileProcessor::new(ProcessingConfig {
            enable_validation: false,
            ..ProcessingConfig::default()
        });
        let event = GitHubEvent {
            id: String::new(),
            event_type: "unknown".to_string(),
            actor: None,
            repo: None,
            payload: None,
            public: None,
            created_at: Some("not-a-date".to_string()),
            org: None,
        };

        assert!(processor.validate_event(&event));
    }

    #[tokio::test]
    async fn validate_archive_integrity_rejects_missing_invalid_and_empty_archives() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing.json.gz");
        let invalid = dir.path().join("invalid.json.gz");
        let empty = dir.path().join("empty.json.gz");
        tokio::fs::write(&invalid, b"not gzip")
            .await
            .expect("write");
        tokio::fs::write(&empty, gzip("\n\n")).await.expect("write");

        let processor = FileProcessor::default();

        assert!(!processor
            .validate_archive_integrity(&missing)
            .await
            .expect("missing check"));
        assert!(!processor
            .validate_archive_integrity(&invalid)
            .await
            .expect("invalid check"));
        assert!(!processor
            .validate_archive_integrity(&empty)
            .await
            .expect("empty check"));
    }

    #[tokio::test]
    async fn validate_archive_integrity_requires_valid_github_event_shape() {
        let dir = tempdir().expect("tempdir");
        let scalar = dir.path().join("scalar.json.gz");
        let partial = dir.path().join("partial.json.gz");
        let valid = dir.path().join("valid.json.gz");
        tokio::fs::write(&scalar, gzip("\"not an event\"\n"))
            .await
            .expect("write scalar");
        tokio::fs::write(
            &partial,
            gzip(r#"{"id":"1","type":"PushEvent","created_at":"2026-05-09T12:00:00Z"}"#),
        )
        .await
        .expect("write partial");
        tokio::fs::write(&valid, gzip(&sample_event("1", "PushEvent")))
            .await
            .expect("write valid");

        let processor = FileProcessor::default();

        assert!(!processor
            .validate_archive_integrity(&scalar)
            .await
            .expect("scalar check"));
        assert!(!processor
            .validate_archive_integrity(&partial)
            .await
            .expect("partial check"));
        assert!(processor
            .validate_archive_integrity(&valid)
            .await
            .expect("valid check"));
    }
}
