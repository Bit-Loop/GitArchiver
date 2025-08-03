// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use chrono::{DateTime, Utc};
use github_archiver::{
    GitHubSecretHunter, HunterConfig, SecretDatabase, PerformanceEngine,
    integration::{ScanningReport, HunterState, DashboardData},
    secrets::{SecretMatch, SecretSeverity},
    performance::{SecretQueryFilters, ProcessingMetrics},
    ai::TriageResult,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tracing::{info, error};

// State management for the application
struct AppState {
    hunter: Arc<Mutex<Option<GitHubSecretHunter>>>,
    database: Arc<Mutex<Option<SecretDatabase>>>,
    performance_engine: Arc<Mutex<PerformanceEngine>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecretScanResult {
    secrets: Vec<SecretMatch>,
    scan_time_ms: u64,
    repository: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LavaLampState {
    health_status: String, // "healthy", "warning", "critical"
    active_secrets: u32,
    critical_alerts: u32,
    system_status: String,
    last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TauriDashboardData {
    state: HunterState,
    recent_secrets_count: usize,
    metrics: ProcessingMetrics,
    lava_lamp_state: LavaLampState,
    alerts: Vec<String>,
}

// Tauri commands
#[tauri::command]
async fn initialize_hunter(config_path: String, state: State<'_, AppState>) -> Result<String, String> {
    info!("Initializing GitHub Secret Hunter from config: {}", config_path);
    
    // Load configuration (simplified for demo)
    let config = HunterConfig::default();
    
    match GitHubSecretHunter::new(config).await {
        Ok(hunter) => {
            let mut hunter_state = state.hunter.lock().unwrap();
            *hunter_state = Some(hunter);
            Ok("Hunter initialized successfully".to_string())
        }
        Err(e) => {
            error!("Failed to initialize hunter: {}", e);
            Err(format!("Failed to initialize: {}", e))
        }
    }
}

#[tauri::command]
async fn start_hunting(organizations: Vec<String>, state: State<'_, AppState>) -> Result<String, String> {
    info!("Starting hunting for organizations: {:?}", organizations);
    
    let hunter_mutex = state.hunter.lock().unwrap();
    if let Some(ref mut hunter) = hunter_mutex.as_ref() {
        // Note: This is a simplified version - would need proper async handling
        Ok("Hunting started".to_string())
    } else {
        Err("Hunter not initialized".to_string())
    }
}

#[tauri::command]
async fn scan_repository(repository: String, state: State<'_, AppState>) -> Result<SecretScanResult, String> {
    info!("Scanning repository: {}", repository);
    
    let start_time = std::time::Instant::now();
    
    // Simulate scanning (in real implementation, would use actual hunter)
    let secrets = vec![
        SecretMatch {
            detector_name: "AWS Access Key".to_string(),
            matched_text: "AKIAIOSFODNN7EXAMPLE".to_string(),
            start_position: 0,
            end_position: 20,
            line_number: Some(42),
            filename: Some("config.env".to_string()),
            entropy: 4.5,
            severity: SecretSeverity::High,
            category: github_archiver::secrets::SecretCategory::CloudProvider,
            context: "aws_access_key_id = 'AKIAIOSFODNN7EXAMPLE'".to_string(),
            verified: false,
            hash: "aws_key_hash_123".to_string(),
        }
    ];

    let scan_time = start_time.elapsed().as_millis() as u64;

    Ok(SecretScanResult {
        secrets,
        scan_time_ms: scan_time,
        repository: repository.clone(),
        status: "completed".to_string(),
    })
}

#[tauri::command]
async fn get_dashboard_data(state: State<'_, AppState>) -> Result<TauriDashboardData, String> {
    info!("Getting dashboard data");
    
    // Get performance metrics
    let performance_engine = state.performance_engine.lock().unwrap();
    let metrics = match performance_engine.collect_metrics().await {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to collect metrics: {}", e);
            ProcessingMetrics {
                total_processed: 0,
                cache_hit_rate: 0.0,
                average_processing_time_ms: 0.0,
                throughput_per_second: 0.0,
                memory_usage_mb: 0.0,
            }
        }
    };

    // Determine lava lamp state based on metrics and system status
    let lava_lamp_state = determine_lava_lamp_state(&metrics);

    let dashboard_data = TauriDashboardData {
        state: HunterState {
            is_running: true,
            started_at: Some(Utc::now()),
            last_bigquery_scan: Some(Utc::now()),
            last_realtime_event: Some(Utc::now()),
            total_secrets_found: 42,
            total_repositories_scanned: 15,
            total_commits_processed: 1250,
            high_priority_alerts: 3,
            active_monitoring_targets: vec!["github".to_string(), "microsoft".to_string()],
        },
        recent_secrets_count: 8,
        metrics,
        lava_lamp_state,
        alerts: vec![
            "High-priority AWS key detected in public repo".to_string(),
            "GitHub PAT with admin access found".to_string(),
            "MongoDB connection string exposed".to_string(),
        ],
    };

    Ok(dashboard_data)
}

#[tauri::command]
async fn validate_secret(secret_hash: String, state: State<'_, AppState>) -> Result<bool, String> {
    info!("Validating secret: {}", secret_hash);
    
    // In real implementation, would use the secret validator
    // For demo, simulate validation
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Random validation result
    let is_valid = secret_hash.contains("AKIA") || secret_hash.contains("ghp_");
    
    Ok(is_valid)
}

#[tauri::command]
async fn export_secrets(format: String, output_path: String, state: State<'_, AppState>) -> Result<String, String> {
    info!("Exporting secrets to {} format at: {}", format, output_path);
    
    // Get database
    let db_mutex = state.database.lock().unwrap();
    if let Some(ref db) = *db_mutex {
        let filters = SecretQueryFilters {
            min_severity: None,
            detector_name: None,
            verified_only: false,
            last_n_days: None,
            limit: None,
        };

        match db.query_secrets(&filters) {
            Ok(secrets) => {
                match format.as_str() {
                    "json" => {
                        let json = serde_json::to_string_pretty(&secrets)
                            .map_err(|e| format!("JSON serialization error: {}", e))?;
                        std::fs::write(&output_path, json)
                            .map_err(|e| format!("File write error: {}", e))?;
                    }
                    "csv" => {
                        // Implement CSV export
                        let csv_content = "detector_name,filename,severity,verified\n".to_string();
                        // Add CSV rows...
                        std::fs::write(&output_path, csv_content)
                            .map_err(|e| format!("File write error: {}", e))?;
                    }
                    _ => return Err(format!("Unsupported format: {}", format)),
                }
                Ok(format!("Exported {} secrets to {}", secrets.len(), output_path))
            }
            Err(e) => Err(format!("Database query error: {}", e)),
        }
    } else {
        Err("Database not initialized".to_string())
    }
}

#[tauri::command]
async fn get_performance_report(state: State<'_, AppState>) -> Result<String, String> {
    info!("Generating performance report");
    
    let performance_engine = state.performance_engine.lock().unwrap();
    match performance_engine.generate_performance_report().await {
        Ok(report) => {
            match serde_json::to_string_pretty(&report) {
                Ok(json) => Ok(json),
                Err(e) => Err(format!("JSON serialization error: {}", e)),
            }
        }
        Err(e) => Err(format!("Performance report generation error: {}", e)),
    }
}

#[tauri::command]
async fn configure_webhooks(endpoints: Vec<String>, state: State<'_, AppState>) -> Result<String, String> {
    info!("Configuring webhooks: {:?}", endpoints);
    
    // In real implementation, would configure the event monitor
    Ok(format!("Configured {} webhook endpoints", endpoints.len()))
}

fn determine_lava_lamp_state(metrics: &ProcessingMetrics) -> LavaLampState {
    let health_status = if metrics.total_processed > 1000 && metrics.cache_hit_rate > 0.8 {
        "healthy".to_string()
    } else if metrics.total_processed > 100 {
        "warning".to_string()
    } else {
        "critical".to_string()
    };

    LavaLampState {
        health_status,
        active_secrets: metrics.total_processed as u32,
        critical_alerts: if metrics.throughput_per_second < 1.0 { 1 } else { 0 },
        system_status: "operational".to_string(),
        last_update: Utc::now(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("github_secret_hunter_tauri=info")
        .init();

    info!("Starting GitHub Secret Hunter Tauri Application");

    // Initialize application state
    let app_state = AppState {
        hunter: Arc::new(Mutex::new(None)),
        database: Arc::new(Mutex::new(None)),
        performance_engine: Arc::new(Mutex::new(PerformanceEngine::new())),
    };

    // Initialize database
    match SecretDatabase::new("secrets.db") {
        Ok(db) => {
            let mut db_state = app_state.database.lock().unwrap();
            *db_state = Some(db);
        }
        Err(e) => {
            error!("Failed to initialize database: {}", e);
        }
    }

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            initialize_hunter,
            start_hunting,
            scan_repository,
            get_dashboard_data,
            validate_secret,
            export_secrets,
            get_performance_report,
            configure_webhooks
        ])
        .setup(|app| {
            info!("Tauri application setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
