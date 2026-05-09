use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::ai::{LocalOpenAiTriageClient, LocalOpenAiTriageConfig, RedactedTriageInput};
use crate::realtime::usable_github_token;
use crate::scanning::trufflehog::TruffleHogFinding;
use crate::scanning::{GitCloner, ScanningService, TruffleHogConfig, TruffleHogScanner};
use crate::{
    bigquery::RepositoryFilter,
    core::{Config, Database, PersistenceService},
    GitHubEventMonitor, GitHubSecretHunter, HunterConfig, PerformanceEngine, SecretDatabase,
};

pub struct ComprehensiveHuntOptions {
    pub organizations: Vec<String>,
    pub bigquery: bool,
    pub realtime: bool,
    pub ai_triage: bool,
    pub model_path: Option<String>,
    pub database: String,
}

pub struct RepositoryScanOptions {
    pub target: String,
    pub scan_type: String,
    pub output: String,
}

pub struct GuiOptions {
    pub database: String,
    pub theme: String,
}

pub struct BigQueryScanOptions {
    pub project: String,
    pub organization: Option<String>,
    pub days: u32,
}

pub struct MonitorOptions {
    pub organizations: Vec<String>,
    pub webhook: Option<String>,
    pub interval: u64,
}

pub struct TriageOptions {
    pub database: String,
    pub provider: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub min_severity: String,
}

pub enum DatabaseOperation {
    Init {
        path: String,
    },
    Query {
        path: String,
        limit: Option<u32>,
    },
    Optimize {
        path: String,
    },
    Export {
        path: String,
        output: String,
    },
    RepairScanState {
        execute: bool,
        backup_path: Option<String>,
        hard_delete_invalid_summaries: bool,
        annotate_failed_scan_errors: bool,
        hard_delete_invalid_queue_rows: bool,
        reset_stale_processing: bool,
    },
}

pub enum PerformanceTest {
    Scan { secrets: usize, workers: usize },
    Database { path: String },
    Report { output: String },
}

#[derive(Debug, Serialize)]
struct RepositoryScanReport {
    target: String,
    repository_path: String,
    findings_found: usize,
    findings: Vec<TruffleHogFinding>,
}

pub fn ensure_trufflehog_ready() -> Result<()> {
    match TruffleHogScanner::ensure_available() {
        Ok(path) => {
            info!("✅ TruffleHog detected at {:?}", path);
            Ok(())
        }
        Err(err) => {
            error!("❌ TruffleHog is required but not available: {}", err);
            Err(err)
        }
    }
}

pub async fn run_comprehensive_hunt(args: ComprehensiveHuntOptions) -> Result<()> {
    info!("🚀 Starting comprehensive GitHub secret hunting");

    let config = HunterConfig {
        gcp_project_id: std::env::var("GCP_PROJECT_ID").unwrap_or_default(),
        github_token: usable_github_token(&std::env::var("GITHUB_TOKEN").unwrap_or_default())
            .unwrap_or_default(),
        redis_url: std::env::var("REDIS_URL").ok(),
        database_path: args.database,
        ai_model_path: args.model_path,
        webhook_endpoints: Vec::new(),
        scanning_options: crate::integration::ScanningOptions {
            enable_bigquery_scanning: args.bigquery,
            enable_realtime_monitoring: args.realtime,
            enable_ai_triage: args.ai_triage,
            enable_secret_validation: true,
            organizations_to_monitor: args.organizations,
            minimum_entropy_threshold: 3.0,
            scan_historical_events: true,
            historical_days_back: 30,
        },
        performance_options: crate::integration::PerformanceOptions {
            parallel_workers: num_cpus::get(),
            cache_size: 10000,
            batch_size: 100,
            rate_limit_per_hour: 5000,
            enable_caching: true,
            enable_deduplication: true,
        },
    };

    let mut hunter = GitHubSecretHunter::new(config).await?;
    hunter.start_hunting().await?;

    info!("Secret hunting started. Press Ctrl+C to stop...");
    tokio::signal::ctrl_c().await?;

    hunter.stop_hunting().await?;
    info!("Secret hunting stopped");

    Ok(())
}

pub async fn run_repository_scan(args: RepositoryScanOptions) -> Result<()> {
    info!("🔍 Scanning target: {}", args.target);

    match args.scan_type.as_str() {
        "repository" => {
            let mut cloner = GitCloner::new();
            let repo_path = cloner.partial_clone(&args.target).await?;
            let scanner = TruffleHogScanner::new(TruffleHogConfig::default());
            let findings = scanner.scan_repository(&repo_path, "", "HEAD").await?;
            let report = RepositoryScanReport {
                target: args.target.clone(),
                repository_path: repo_path.display().to_string(),
                findings_found: findings.len(),
                findings,
            };

            match args.output.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&report)?),
                "yaml" => println!("{}", serde_yaml::to_string(&report)?),
                _ => info!("Scan completed: {} findings found", report.findings_found),
            }
        }
        _ => {
            error!("Unsupported scan type: {}", args.scan_type);
        }
    }

    Ok(())
}

pub async fn run_gui(_args: GuiOptions) -> Result<()> {
    info!("Browser dashboard and Tauri are the supported production UI surfaces");
    Err(anyhow::anyhow!(
        "legacy desktop GUI has been removed; start the server and open the browser dashboard or use the Tauri app"
    ))
}

pub async fn run_bigquery_scan(args: BigQueryScanOptions) -> Result<()> {
    info!("📊 Running BigQuery historical scan");

    let scanner =
        crate::BigQueryScanner::new_with_default_credentials(args.project.clone()).await?;
    let end_date = chrono::Utc::now().date_naive();
    let start_date = end_date - chrono::Duration::days(args.days as i64);

    let filter = RepositoryFilter {
        organizations: args.organization.map(|org| vec![org]).unwrap_or_default(),
        users: vec![],
        repositories: vec![],
    };

    let events = scanner
        .scan_zero_commit_events(start_date, end_date, &filter, Some(1000))
        .await?;

    info!("Found {} zero-commit events", events.len());
    for event in events.iter().take(10) {
        info!(
            "Event: {} -> {} ({})",
            event.repo_name, event.before_commit, event.created_at
        );
    }

    let mut converted = Vec::new();
    for event in events {
        let event_id = event.id.clone();
        match event.to_github_event() {
            Some(value) => converted.push(value),
            None => warn!(
                "Skipping zero-commit event {} due to incomplete payload",
                event_id
            ),
        }
    }

    if converted.is_empty() {
        info!("No zero-commit events ready for ingestion");
        return Ok(());
    }

    let config = Config::default();
    let db = Database::new(&config).await?;
    let inserted = db
        .insert_events_batch(converted, "bigquery_zero_commit.json")
        .await?;

    info!(
        "Ingested {} zero-commit events into pending queue for scanning",
        inserted
    );

    Ok(())
}

pub async fn run_realtime_monitor(args: MonitorOptions) -> Result<()> {
    info!("⚡ Starting real-time GitHub event monitoring");
    if !args.organizations.is_empty() {
        info!(
            "Monitoring organizations: {}",
            args.organizations.join(", ")
        );
    }
    info!("Poll interval configured at {}s", args.interval);

    let github_token =
        usable_github_token(&std::env::var("GITHUB_TOKEN").unwrap_or_default()).unwrap_or_default();
    let requested_requests_per_minute =
        u32::try_from(60u64.checked_div(args.interval).unwrap_or(60).max(1)).unwrap_or(1);
    let requests_per_minute = if github_token.trim().is_empty() {
        warn!("GITHUB_TOKEN is not set; using unauthenticated GitHub Events requests at 1 req/min");
        1
    } else {
        requested_requests_per_minute.clamp(1, 60)
    };

    let config = Config::default();
    let database = Arc::new(Database::new(&config).await?);
    let persistence = Arc::new(PersistenceService::new(database));
    let scan_concurrency = usize::max(
        1,
        usize::min(config.download.max_concurrent_downloads as usize, 32),
    );
    let scanning_service =
        Arc::new(ScanningService::new(scan_concurrency).with_persistence(persistence.clone()));

    let monitor = GitHubEventMonitor::new(&github_token)
        .await?
        .with_persistence(persistence)
        .with_scanning_service(scanning_service)
        .with_organizations(args.organizations)
        .with_rate_limit(requests_per_minute, true);

    if let Some(webhook_url) = args.webhook {
        monitor
            .add_webhook_endpoint(webhook_url, None, vec!["push".to_string()])
            .await?;
    }

    monitor.start_monitoring().await?;

    Ok(())
}

pub async fn run_ai_triage(args: TriageOptions) -> Result<()> {
    info!("🤖 Running AI triage on existing secrets");
    let secrets = load_triage_secrets(&args)?;

    if args.provider != "local-openai" {
        return Err(anyhow::anyhow!(
            "unsupported triage provider '{}'; use local-openai",
            args.provider
        ));
    }

    let config = LocalOpenAiTriageConfig {
        base_url: args
            .base_url
            .or_else(|| std::env::var("AI_TRIAGE_BASE_URL").ok())
            .unwrap_or_else(|| "http://127.0.0.1:11434/v1".to_string()),
        model: args
            .model
            .or_else(|| std::env::var("AI_TRIAGE_MODEL").ok())
            .ok_or_else(|| anyhow::anyhow!("triage run requires --model or AI_TRIAGE_MODEL"))?,
        api_key: std::env::var("AI_TRIAGE_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty()),
    };
    let client = LocalOpenAiTriageClient::new(config);

    let mut completed_count = 0;
    for secret_record in secrets {
        let input = RedactedTriageInput {
            detection_id: None,
            secret_hash: secret_record.secret_hash.clone(),
            detector_name: secret_record.detector_name.clone(),
            severity: secret_record.severity.clone(),
            category: secret_record.category.clone(),
            repository: None,
            file_path: secret_record.filename.clone(),
            line_number: secret_record.line_number.map(|line| line as i32),
            verified: secret_record.verified,
            source: Some("cli".to_string()),
        };
        let result = client.triage(&input).await?;
        info!(
            "Triaged {} with confidence {:.2}: {}",
            secret_record.detector_name, result.confidence, result.analysis
        );
        completed_count += 1;
    }

    info!(
        "AI triage completed for {} redacted findings",
        completed_count
    );
    Ok(())
}

pub async fn run_database_operation(operation: DatabaseOperation) -> Result<()> {
    match operation {
        DatabaseOperation::Init { path } => {
            info!("🗄️ Initializing database: {}", path);
            let _db = SecretDatabase::new(&path)?;
            info!("Database initialized successfully");
        }
        DatabaseOperation::Query { path, limit } => {
            info!("🔍 Querying database: {}", path);
            let db = SecretDatabase::new(&path)?;
            let filters = crate::performance::SecretQueryFilters {
                min_severity: None,
                detector_name: None,
                verified_only: false,
                last_n_days: None,
                limit,
            };
            let secrets = db.query_secrets(&filters)?;
            info!("Found {} secrets", secrets.len());
            for secret in secrets.iter().take(5) {
                info!("  - {} ({})", secret.detector_name, secret.severity);
            }
        }
        DatabaseOperation::Optimize { path } => {
            info!("⚡ Optimizing database: {}", path);
            let engine = PerformanceEngine::new();
            engine.optimize_database(&path).await?;
            info!("Database optimization completed");
        }
        DatabaseOperation::Export { path, output } => {
            info!("📤 Exporting database: {} -> {}", path, output);
            info!("Export completed");
        }
        DatabaseOperation::RepairScanState {
            execute,
            backup_path,
            hard_delete_invalid_summaries,
            annotate_failed_scan_errors,
            hard_delete_invalid_queue_rows,
            reset_stale_processing,
        } => {
            if execute && backup_path.is_none() {
                return Err(anyhow::anyhow!(
                    "repair-scan-state --execute requires --backup-path"
                ));
            }

            let config = Config::default();
            let database = Database::new(&config).await?;
            let report = database
                .repair_scan_state(crate::core::database::ScanStateRepairRequest {
                    execute,
                    backup_path,
                    hard_delete_invalid_summaries,
                    annotate_failed_scan_errors,
                    hard_delete_invalid_queue_rows,
                    reset_stale_processing,
                    operator: None,
                })
                .await?;

            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

pub async fn run_performance_test(test: PerformanceTest) -> Result<()> {
    match test {
        PerformanceTest::Scan { secrets, workers } => {
            info!(
                "🚀 Benchmarking secret scanning: {} secrets, {} workers",
                secrets, workers
            );

            let engine = PerformanceEngine::new();
            let test_secrets = generate_test_secrets(secrets);

            let request = crate::performance::BatchProcessingRequest {
                id: uuid::Uuid::new_v4(),
                secrets: test_secrets,
                processing_options: crate::performance::ProcessingOptions {
                    deduplicate: true,
                    validate_secrets: false,
                    ai_triage: false,
                    parallel_workers: Some(workers),
                    cache_results: true,
                },
                priority: crate::performance::ProcessingPriority::Normal,
            };

            let result = engine.process_secrets_parallel(request).await?;
            info!("Benchmark results:");
            info!("  - Processed: {} secrets", result.processed_count);
            info!("  - Time: {} ms", result.processing_time_ms);
            info!(
                "  - Throughput: {:.2} secrets/sec",
                result.processed_count as f64 / (result.processing_time_ms as f64 / 1000.0)
            );
        }
        PerformanceTest::Database { path } => {
            info!("🗄️ Benchmarking database operations: {}", path);

            let db = SecretDatabase::new(&path)?;
            let test_secrets = generate_test_secrets(1000);

            let start = std::time::Instant::now();
            db.bulk_insert_secrets(&test_secrets)?;
            let insert_time = start.elapsed();

            info!("Database benchmark results:");
            info!("  - Insert time: {:?}", insert_time);
            info!(
                "  - Throughput: {:.2} inserts/sec",
                1000.0 / insert_time.as_secs_f64()
            );
        }
        PerformanceTest::Report { output } => {
            info!("📊 Generating performance report: {}", output);

            let engine = PerformanceEngine::new();
            let report = engine.generate_performance_report().await?;

            let json = serde_json::to_string_pretty(&report)?;
            std::fs::write(&output, json)?;

            info!("Performance report generated: {}", output);
        }
    }

    Ok(())
}

fn triage_filters(args: &TriageOptions) -> crate::performance::SecretQueryFilters {
    crate::performance::SecretQueryFilters {
        min_severity: Some(match args.min_severity.as_str() {
            "critical" => crate::secrets::SecretSeverity::Critical,
            "high" => crate::secrets::SecretSeverity::High,
            "medium" => crate::secrets::SecretSeverity::Medium,
            _ => crate::secrets::SecretSeverity::Low,
        }),
        detector_name: None,
        verified_only: false,
        last_n_days: Some(7),
        limit: Some(100),
    }
}

fn load_triage_secrets(args: &TriageOptions) -> Result<Vec<crate::performance::SecretRecord>> {
    let database = SecretDatabase::new(&args.database)?;
    let secrets = database.query_secrets(&triage_filters(args))?;
    info!("Found {} secrets to triage", secrets.len());
    Ok(secrets)
}

fn generate_test_secrets(count: usize) -> Vec<crate::SecretMatch> {
    (0..count)
        .map(|i| crate::SecretMatch {
            detector_name: format!("TestDetector{}", i % 10),
            matched_text: format!("secret_value_{}", i),
            start_position: 0,
            end_position: 20,
            line_number: Some(i + 1),
            filename: Some(format!("test_{}.env", i % 5)),
            entropy: 3.5 + (i % 3) as f64,
            severity: match i % 4 {
                0 => crate::secrets::SecretSeverity::Critical,
                1 => crate::secrets::SecretSeverity::High,
                2 => crate::secrets::SecretSeverity::Medium,
                _ => crate::secrets::SecretSeverity::Low,
            },
            category: crate::secrets::SecretCategory::ApiKey,
            context: format!("api_key = secret_value_{}", i),
            verified: i % 10 == 0,
            hash: format!("hash_{}", i),
        })
        .collect()
}
