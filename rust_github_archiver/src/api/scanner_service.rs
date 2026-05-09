use anyhow::anyhow;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration as StdDuration;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api::state::AppState;
use crate::scanning::{domain::ScanInitiator, CompletedScan, ScanConfig, ScanJob, ScanType};

pub type ScannerServiceResult<T> = std::result::Result<T, ScannerServiceError>;

#[derive(Debug)]
pub enum ScannerServiceError {
    Validation(String),
    Operation(anyhow::Error),
}

impl ScannerServiceError {
    fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::Operation(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Display for ScannerServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) => write!(f, "{}", message),
            Self::Operation(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for ScannerServiceError {}

impl From<anyhow::Error> for ScannerServiceError {
    fn from(value: anyhow::Error) -> Self {
        Self::Operation(value)
    }
}

#[derive(Debug, Deserialize)]
pub struct ScanRepositoryRequest {
    pub repository: String,
    pub scan_type: Option<String>,
    pub secret_types: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub include_private: Option<bool>,
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchScanRequest {
    pub repositories: Vec<String>,
    pub scan_config: ScanConfiguration,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScanConfiguration {
    pub scan_type: String,
    pub secret_types: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub include_private: bool,
    pub max_concurrent: Option<u32>,
    pub timeout_seconds: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ScanResponse {
    pub scan_id: String,
    pub repository: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub secrets_found: u32,
    pub files_scanned: u32,
    pub scan_duration_ms: u64,
    pub severity_breakdown: HashMap<String, u32>,
    pub category_breakdown: HashMap<String, u32>,
}

#[derive(Debug, Serialize)]
pub struct BatchScanResponse {
    pub batch_id: String,
    pub total_repositories: u32,
    pub completed_repositories: u32,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub results: Vec<ScanResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ScheduleScanRequest {
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub repositories: Option<Vec<String>>,
    pub scan_config: Option<ScanConfiguration>,
}

#[derive(Debug, Serialize)]
pub struct ScheduledScanResponse {
    pub schedule_id: String,
    pub name: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub schedule: String,
    pub repositories: Vec<String>,
    pub scan_config: ScanConfig,
    pub status: String,
    pub next_run: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
}

pub async fn start_repository_scan(
    app_state: &AppState,
    request: ScanRepositoryRequest,
    username: &str,
) -> ScannerServiceResult<ScanResponse> {
    validate_repository_request(&request)?;

    info!(
        repository = %request.repository,
        user = %username,
        branch = request.branch.as_deref().unwrap_or("default"),
        include_private = request.include_private.unwrap_or(false),
        "Starting repository scan"
    );

    let repository = request.repository.clone();
    let scan_id = app_state
        .scanning_service
        .start_scan(
            repository.clone(),
            parse_scan_type(request.scan_type.as_deref()),
            manual_scan_config_from_request(&request),
            ScanInitiator::manual(username),
            Vec::new(),
        )
        .await?;

    wait_for_scan_response(app_state, &scan_id, &repository, StdDuration::from_secs(30)).await
}

pub async fn start_batch_scan(
    app_state: &AppState,
    request: BatchScanRequest,
    username: &str,
) -> ScannerServiceResult<BatchScanResponse> {
    validate_batch_request(&request)?;

    info!(
        repository_count = request.repositories.len(),
        user = %username,
        max_concurrent = request.scan_config.max_concurrent.unwrap_or(5),
        "Starting batch scan"
    );

    let BatchScanRequest {
        repositories,
        scan_config,
    } = request;

    let batch_id = Uuid::new_v4().to_string();
    let started_at = Utc::now();
    let total_repositories = repositories.len() as u32;
    let max_concurrent = scan_config.max_concurrent.unwrap_or(5).max(1) as usize;
    let scan_type = parse_scan_type(Some(&scan_config.scan_type));
    let internal_config = convert_api_scan_config(&scan_config);
    let timeout = batch_timeout(&scan_config);

    let mut completed_repositories = 0u32;
    let mut results = Vec::with_capacity(repositories.len());
    let mut latest_completion: Option<DateTime<Utc>> = None;

    for repository_chunk in repositories.chunks(max_concurrent) {
        let mut pending = Vec::with_capacity(repository_chunk.len());

        for repository in repository_chunk {
            match app_state
                .scanning_service
                .start_scan(
                    repository.clone(),
                    scan_type.clone(),
                    internal_config.clone(),
                    ScanInitiator::manual(username),
                    Vec::new(),
                )
                .await
            {
                Ok(scan_id) => pending.push((repository.clone(), scan_id)),
                Err(error) => {
                    error!("Failed to start scan for {}: {}", repository, error);
                    return Err(anyhow!("failed to start batch scan for {}", repository).into());
                }
            }
        }

        for (repository, scan_id) in pending {
            match app_state
                .scanning_service
                .wait_for_scan_completion(&scan_id, timeout)
                .await
            {
                Ok(scan) => {
                    latest_completion = Some(
                        latest_completion
                            .map(|current| current.max(scan.completed_at))
                            .unwrap_or(scan.completed_at),
                    );
                    results.push(completed_scan_to_response(&scan));
                    completed_repositories += 1;
                }
                Err(error) => {
                    warn!(
                        "Scan {} for {} still running or failed in batch: {}",
                        scan_id, repository, error
                    );
                    if let Some(job) = app_state.scanning_service.get_scan_status(&scan_id).await {
                        results.push(active_scan_to_response(&job));
                    }
                }
            }
        }
    }

    let status = if completed_repositories == total_repositories {
        "completed"
    } else if completed_repositories == 0 {
        "running"
    } else {
        "partial"
    }
    .to_string();

    Ok(BatchScanResponse {
        batch_id,
        total_repositories,
        completed_repositories,
        status,
        started_at,
        estimated_completion: if completed_repositories == total_repositories {
            latest_completion
        } else {
            None
        },
        results,
    })
}

pub async fn create_scan_schedule(
    app_state: &AppState,
    request: ScheduleScanRequest,
    username: &str,
) -> ScannerServiceResult<ScheduledScanResponse> {
    validate_schedule_request(&request)?;

    info!(user = %username, "Scheduling recurring scan");

    let repositories = request.repositories.unwrap_or_default();
    let cron_expression = request.schedule.unwrap_or_else(|| "0 0 * * *".to_string());
    let name = request
        .name
        .unwrap_or_else(|| default_schedule_name(&cron_expression));
    let config = request
        .scan_config
        .as_ref()
        .map(convert_api_scan_config)
        .unwrap_or_default();

    let schedule_id = app_state
        .scanning_service
        .create_schedule(
            name,
            cron_expression,
            repositories,
            config,
            username.to_string(),
        )
        .await;

    let schedule = app_state
        .scanning_service
        .get_schedules()
        .await
        .into_iter()
        .find(|schedule| schedule.id == schedule_id)
        .ok_or_else(|| anyhow!("created schedule {} could not be reloaded", schedule_id))?;

    Ok(ScheduledScanResponse {
        schedule_id: schedule.id,
        name: schedule.name,
        created_by: schedule.created_by,
        created_at: schedule.created_at,
        schedule: schedule.cron_expression,
        repositories: schedule.repositories,
        scan_config: schedule.config,
        status: if schedule.enabled {
            "active".to_string()
        } else {
            "disabled".to_string()
        },
        next_run: schedule.next_run,
        last_run: schedule.last_run,
    })
}

fn validate_repository_request(request: &ScanRepositoryRequest) -> ScannerServiceResult<()> {
    if request.repository.trim().is_empty() {
        return Err(ScannerServiceError::validation(
            "repository must not be empty",
        ));
    }

    if let Some(scan_type) = request.scan_type.as_deref() {
        validate_scan_type(scan_type)?;
    }

    Ok(())
}

fn validate_batch_request(request: &BatchScanRequest) -> ScannerServiceResult<()> {
    if request.repositories.is_empty() {
        return Err(ScannerServiceError::validation(
            "repositories must contain at least one entry",
        ));
    }

    if request
        .repositories
        .iter()
        .any(|repository| repository.trim().is_empty())
    {
        return Err(ScannerServiceError::validation(
            "repositories must not contain empty values",
        ));
    }

    validate_scan_configuration(&request.scan_config)
}

fn validate_schedule_request(request: &ScheduleScanRequest) -> ScannerServiceResult<()> {
    if let Some(repositories) = request.repositories.as_ref() {
        if repositories
            .iter()
            .any(|repository| repository.trim().is_empty())
        {
            return Err(ScannerServiceError::validation(
                "scheduled repositories must not contain empty values",
            ));
        }
    }

    if let Some(scan_config) = request.scan_config.as_ref() {
        validate_scan_configuration(scan_config)?;
    }

    Ok(())
}

fn validate_scan_configuration(config: &ScanConfiguration) -> ScannerServiceResult<()> {
    validate_scan_type(&config.scan_type)?;

    if let Some(max_concurrent) = config.max_concurrent {
        if max_concurrent == 0 {
            return Err(ScannerServiceError::validation(
                "max_concurrent must be at least 1",
            ));
        }
    }

    if let Some(timeout_seconds) = config.timeout_seconds {
        if timeout_seconds == 0 {
            return Err(ScannerServiceError::validation(
                "timeout_seconds must be at least 1",
            ));
        }
    }

    Ok(())
}

fn validate_scan_type(scan_type: &str) -> ScannerServiceResult<()> {
    match scan_type.trim().to_ascii_lowercase().as_str() {
        "full" | "incremental" | "targeted" | "scheduled" | "manual" | "repository" => Ok(()),
        _ => Err(ScannerServiceError::validation(format!(
            "unsupported scan_type '{}'",
            scan_type
        ))),
    }
}

fn parse_scan_type(input: Option<&str>) -> ScanType {
    match input.unwrap_or("full").to_ascii_lowercase().as_str() {
        "full" => ScanType::Full,
        "incremental" => ScanType::Incremental,
        "targeted" => ScanType::Targeted,
        "scheduled" => ScanType::Scheduled,
        _ => ScanType::Manual,
    }
}

fn manual_scan_config_from_request(request: &ScanRepositoryRequest) -> ScanConfig {
    ScanConfig {
        secret_types: request.secret_types.clone().unwrap_or_else(|| {
            vec![
                "AWS Access Key ID".to_string(),
                "GitHub Personal Access Token".to_string(),
            ]
        }),
        exclude_patterns: request
            .exclude_patterns
            .clone()
            .unwrap_or_else(|| vec!["*.log".to_string(), "node_modules/*".to_string()]),
        include_extensions: None,
        exclude_paths: vec!["target/".to_string(), ".git/".to_string()],
        max_file_size_mb: Some(10),
        timeout_seconds: Some(300),
        entropy_threshold: Some(4.5),
        verify_secrets: true,
    }
}

fn convert_api_scan_config(config: &ScanConfiguration) -> ScanConfig {
    let mut internal = ScanConfig::default();

    if !config.secret_types.is_empty() {
        internal.secret_types = config.secret_types.clone();
    }
    if !config.exclude_patterns.is_empty() {
        internal.exclude_patterns = config.exclude_patterns.clone();
    }
    if let Some(timeout) = config.timeout_seconds {
        internal.timeout_seconds = Some(timeout);
    }

    internal
}

fn batch_timeout(config: &ScanConfiguration) -> StdDuration {
    StdDuration::from_secs(config.timeout_seconds.unwrap_or(30).clamp(1, 30) as u64)
}

async fn wait_for_scan_response(
    app_state: &AppState,
    scan_id: &str,
    repository: &str,
    wait_timeout: StdDuration,
) -> ScannerServiceResult<ScanResponse> {
    match app_state
        .scanning_service
        .wait_for_scan_completion(scan_id, wait_timeout)
        .await
    {
        Ok(completed) => {
            let response = completed_scan_to_response(&completed);
            info!(
                scan_id = %scan_id,
                repository = %repository,
                secrets_found = response.secrets_found,
                files_scanned = response.files_scanned,
                "Repository scan completed"
            );
            Ok(response)
        }
        Err(error) => {
            warn!(
                "Scan {} for {} did not finish within {:?}: {}",
                scan_id, repository, wait_timeout, error
            );
            app_state
                .scanning_service
                .get_scan_status(scan_id)
                .await
                .map(|job| active_scan_to_response(&job))
                .ok_or_else(|| anyhow!("scan {} could not be loaded after timeout", scan_id).into())
        }
    }
}

fn default_schedule_name(cron_expression: &str) -> String {
    format!("Scheduled scan ({})", cron_expression)
}

fn completed_scan_to_response(scan: &CompletedScan) -> ScanResponse {
    ScanResponse {
        scan_id: scan.id.clone(),
        repository: scan.repository.clone(),
        status: scan.status.as_str().to_string(),
        started_at: scan.started_at,
        completed_at: Some(scan.completed_at),
        secrets_found: scan.results.findings.len() as u32,
        files_scanned: scan.results.files_scanned,
        scan_duration_ms: scan.duration_ms,
        severity_breakdown: scan.results.severity_breakdown.clone(),
        category_breakdown: scan.results.category_breakdown.clone(),
    }
}

fn active_scan_to_response(job: &ScanJob) -> ScanResponse {
    let elapsed_ms = (Utc::now() - job.started_at).num_milliseconds().max(0) as u64;

    ScanResponse {
        scan_id: job.id.clone(),
        repository: job.repository.clone(),
        status: job.status.as_str().to_string(),
        started_at: job.started_at,
        completed_at: None,
        secrets_found: job.progress.findings_found,
        files_scanned: job.progress.files_scanned,
        scan_duration_ms: elapsed_ms,
        severity_breakdown: HashMap::new(),
        category_breakdown: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_scan_config() -> ScanConfiguration {
        ScanConfiguration {
            scan_type: "full".to_string(),
            secret_types: vec!["Custom Detector".to_string()],
            exclude_patterns: vec!["vendor/*".to_string()],
            include_private: true,
            max_concurrent: Some(3),
            timeout_seconds: Some(120),
        }
    }

    #[test]
    fn convert_api_scan_config_keeps_verified_scanning_enabled() {
        let internal = convert_api_scan_config(&sample_scan_config());
        assert!(internal.verify_secrets);
    }

    #[test]
    fn convert_api_scan_config_copies_supported_fields() {
        let internal = convert_api_scan_config(&sample_scan_config());
        assert_eq!(internal.secret_types, vec!["Custom Detector".to_string()]);
        assert_eq!(internal.exclude_patterns, vec!["vendor/*".to_string()]);
        assert_eq!(internal.timeout_seconds, Some(120));
    }

    #[test]
    fn manual_scan_config_uses_request_specific_defaults() {
        let request = ScanRepositoryRequest {
            repository: "octo/repo".to_string(),
            scan_type: None,
            secret_types: None,
            exclude_patterns: None,
            include_private: Some(false),
            branch: None,
        };

        let config = manual_scan_config_from_request(&request);
        assert_eq!(
            config.secret_types,
            vec![
                "AWS Access Key ID".to_string(),
                "GitHub Personal Access Token".to_string()
            ]
        );
        assert_eq!(
            config.exclude_patterns,
            vec!["*.log".to_string(), "node_modules/*".to_string()]
        );
        assert_eq!(
            config.exclude_paths,
            vec!["target/".to_string(), ".git/".to_string()]
        );
    }

    #[test]
    fn batch_timeout_respects_short_waits_and_caps_long_waits() {
        let mut config = sample_scan_config();
        config.timeout_seconds = Some(5);

        assert_eq!(batch_timeout(&config), StdDuration::from_secs(5));

        config.timeout_seconds = Some(300);
        assert_eq!(batch_timeout(&config), StdDuration::from_secs(30));
    }

    #[test]
    fn parse_scan_type_defaults_and_fallbacks_are_stable() {
        assert!(matches!(parse_scan_type(None), ScanType::Full));
        assert!(matches!(
            parse_scan_type(Some("incremental")),
            ScanType::Incremental
        ));
        assert!(matches!(parse_scan_type(Some("unknown")), ScanType::Manual));
    }

    #[test]
    fn schedule_name_defaults_from_cron_expression() {
        assert_eq!(
            default_schedule_name("0 0 * * *"),
            "Scheduled scan (0 0 * * *)"
        );
    }

    #[test]
    fn repository_request_validation_rejects_empty_repository_and_bad_scan_type() {
        let empty = ScanRepositoryRequest {
            repository: "   ".to_string(),
            scan_type: None,
            secret_types: None,
            exclude_patterns: None,
            include_private: None,
            branch: None,
        };
        let bad_scan_type = ScanRepositoryRequest {
            repository: "owner/repo".to_string(),
            scan_type: Some("dangerous".to_string()),
            secret_types: None,
            exclude_patterns: None,
            include_private: None,
            branch: None,
        };

        assert!(matches!(
            validate_repository_request(&empty),
            Err(ScannerServiceError::Validation(_))
        ));
        assert!(matches!(
            validate_repository_request(&bad_scan_type),
            Err(ScannerServiceError::Validation(_))
        ));
    }

    #[test]
    fn batch_request_validation_covers_empty_inputs_and_timeout_limits() {
        let mut request = BatchScanRequest {
            repositories: Vec::new(),
            scan_config: sample_scan_config(),
        };
        assert!(validate_batch_request(&request).is_err());

        request.repositories = vec!["owner/repo".to_string(), String::new()];
        assert!(validate_batch_request(&request).is_err());

        request.repositories = vec!["owner/repo".to_string()];
        request.scan_config.timeout_seconds = Some(0);
        assert!(validate_batch_request(&request).is_err());

        request.scan_config.timeout_seconds = Some(1);
        request.scan_config.max_concurrent = Some(0);
        assert!(validate_batch_request(&request).is_err());

        request.scan_config.max_concurrent = Some(1);
        assert!(validate_batch_request(&request).is_ok());
    }

    #[test]
    fn schedule_request_validation_allows_empty_repository_list_but_not_empty_items() {
        let valid = ScheduleScanRequest {
            name: None,
            schedule: Some("*/5 * * * *".to_string()),
            repositories: Some(Vec::new()),
            scan_config: Some(sample_scan_config()),
        };
        let invalid = ScheduleScanRequest {
            repositories: Some(vec!["owner/repo".to_string(), " ".to_string()]),
            ..valid
        };

        assert!(validate_schedule_request(&invalid).is_err());
        assert!(validate_schedule_request(&ScheduleScanRequest {
            repositories: Some(Vec::new()),
            name: None,
            schedule: Some("*/5 * * * *".to_string()),
            scan_config: Some(sample_scan_config()),
        })
        .is_ok());
    }

    #[test]
    fn scanner_service_errors_map_to_public_status_codes() {
        assert_eq!(
            ScannerServiceError::validation("bad input").status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ScannerServiceError::Operation(anyhow!("boom")).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
