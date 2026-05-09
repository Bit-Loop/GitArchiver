// Prevents additional console window on Windows in release, DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::{Client, Method};
use serde_json::{json, Value};
use tauri::State;
use tracing::info;

#[derive(Clone)]
struct AppState {
    api_base_url: String,
    api_token: Option<String>,
    client: Client,
}

impl AppState {
    fn from_env() -> Self {
        let api_base_url = std::env::var("GITARCHIVER_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
            .trim_end_matches('/')
            .to_string();
        let api_token = std::env::var("GITARCHIVER_API_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Self {
            api_base_url,
            api_token,
            client: Client::new(),
        }
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let url = format!("{}{}", self.api_base_url, path);
        let mut request = self.client.request(method, url);
        if let Some(token) = &self.api_token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .map_err(|error| format!("API request failed: {}", error))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| format!("API response read failed: {}", error))?;

        if !status.is_success() {
            return Err(format!("API returned {}: {}", status, text));
        }

        if text.trim().is_empty() {
            return Ok(json!({}));
        }

        serde_json::from_str(&text).map_err(|error| format!("API JSON decode failed: {}", error))
    }

    async fn get_json(&self, path: &str) -> Result<Value, String> {
        self.request_json(Method::GET, path, None).await
    }

    async fn post_json(&self, path: &str, body: Value) -> Result<Value, String> {
        self.request_json(Method::POST, path, Some(body)).await
    }

    async fn scan_results(
        &self,
        repository: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let mut path = format!("/api/scanner/results?limit={}", limit);
        if let Some(repository) = repository {
            path.push_str("&repository=");
            path.push_str(&percent_encode(repository));
        }

        let response = self.get_json(&path).await?;
        let detections = response
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        Ok(detections
            .into_iter()
            .map(detection_to_scan_result)
            .collect())
    }
}

#[tauri::command]
async fn initialize_hunter(state: State<'_, AppState>) -> Result<String, String> {
    state.get_json("/api/health").await?;
    Ok(format!(
        "Connected to GitArchiver API at {}",
        state.api_base_url
    ))
}

#[tauri::command]
async fn start_hunting(state: State<'_, AppState>) -> Result<String, String> {
    state.post_json("/api/start-scraper", json!({})).await?;
    Ok("Scraper start requested through API".to_string())
}

#[tauri::command]
async fn stop_hunting(state: State<'_, AppState>) -> Result<String, String> {
    state.post_json("/api/stop-scraper", json!({})).await?;
    Ok("Scraper stop requested through API".to_string())
}

#[tauri::command]
async fn scan_repository(
    repository: String,
    scan_type: Option<String>,
    secret_types: Option<Vec<String>>,
    exclude_patterns: Option<Vec<String>>,
    include_private: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<Value>, String> {
    state
        .post_json(
            "/api/scanner/scan",
            json!({
                "repository": repository,
                "scan_type": scan_type,
                "secret_types": secret_types,
                "exclude_patterns": exclude_patterns,
                "include_private": include_private,
            }),
        )
        .await?;

    state.scan_results(Some(&repository), 100).await
}

#[tauri::command]
async fn get_recent_scan_results(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<Value>, String> {
    state.scan_results(None, limit.unwrap_or(50).min(500)).await
}

#[tauri::command]
async fn get_dashboard_data(state: State<'_, AppState>) -> Result<Value, String> {
    let statistics = state
        .get_json("/api/scanner/statistics")
        .await
        .unwrap_or_else(|_| json!({}));
    let metrics = state
        .get_json("/api/system/metrics")
        .await
        .unwrap_or_else(|_| json!({}));
    let results = state
        .get_json("/api/scanner/results?limit=10")
        .await
        .unwrap_or_else(|_| json!({ "results": [] }));

    let severity = statistics
        .get("severity_breakdown")
        .or_else(|| statistics.get("secrets_by_severity"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let total_secrets = statistics
        .get("total_secrets")
        .or_else(|| statistics.get("secrets_found"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let recent_scans = results
        .get("results")
        .or_else(|| results.get("scans"))
        .cloned()
        .unwrap_or_else(|| json!([]));

    Ok(json!({
        "total_secrets": total_secrets,
        "secrets_by_severity": {
            "critical": severity.get("critical").and_then(Value::as_i64).unwrap_or(0),
            "high": severity.get("high").and_then(Value::as_i64).unwrap_or(0),
            "medium": severity.get("medium").and_then(Value::as_i64).unwrap_or(0),
            "low": severity.get("low").and_then(Value::as_i64).unwrap_or(0),
        },
        "recent_scans": recent_scans,
        "system_metrics": {
            "cpu_usage": metrics.pointer("/cpu/usage_percent").and_then(Value::as_f64).unwrap_or(0.0),
            "memory_usage": metrics.pointer("/memory/usage_percent").and_then(Value::as_f64).unwrap_or(0.0),
            "disk_usage": metrics.pointer("/disk/usage_percent").and_then(Value::as_f64).unwrap_or(0.0),
            "network_requests": metrics.pointer("/network/requests").and_then(Value::as_i64).unwrap_or(0),
        },
        "threat_timeline": [],
    }))
}

#[tauri::command]
async fn validate_secret(
    secret_id: String,
    is_valid: bool,
    reason: Option<String>,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    state
        .post_json(
            "/api/scanner/validation",
            json!({
                "secret_id": secret_id,
                "is_valid": is_valid,
                "reason": reason,
            }),
        )
        .await
}

#[tauri::command]
async fn export_secrets(
    format: Option<String>,
    output_path: Option<String>,
    secret_ids: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let format = format.unwrap_or_else(|| "json".to_string());
    let path = format!("/api/scanner/export?format={}", format);
    let mut response = state.get_json(&path).await?;
    if let Some(output_path) = output_path {
        response["output_path"] = json!(output_path);
    }
    if let Some(secret_ids) = secret_ids {
        response["secret_ids"] = json!(secret_ids);
    }
    Ok(response)
}

#[tauri::command]
async fn get_performance_report(
    time_range: Option<String>,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let metrics = state
        .get_json("/api/monitoring/metrics")
        .await
        .unwrap_or_else(|_| json!({}));
    let statistics = state
        .get_json("/api/scanner/statistics")
        .await
        .unwrap_or_else(|_| json!({}));

    Ok(json!({
        "time_range": time_range.unwrap_or_else(|| "24h".to_string()),
        "scan_performance": {
            "total_scans": statistics.get("total_scans").and_then(Value::as_i64).unwrap_or(0),
            "avg_scan_time": statistics.get("avg_scan_duration_ms").and_then(Value::as_f64).unwrap_or(0.0),
            "successful_scans": statistics.get("successful_scans").and_then(Value::as_i64).unwrap_or(0),
            "failed_scans": statistics.get("failed_scans").and_then(Value::as_i64).unwrap_or(0),
            "scans_per_hour": statistics.get("scan_rate_per_hour").and_then(Value::as_f64).unwrap_or(0.0),
        },
        "resource_usage": {
            "cpu_usage_history": [metrics.pointer("/system/cpu_usage").and_then(Value::as_f64).unwrap_or(0.0)],
            "memory_usage_history": [metrics.pointer("/system/memory_usage").and_then(Value::as_f64).unwrap_or(0.0)],
            "disk_io_history": [metrics.pointer("/system/disk_usage").and_then(Value::as_f64).unwrap_or(0.0)],
            "network_io_history": [metrics.pointer("/system/network_requests").and_then(Value::as_f64).unwrap_or(0.0)],
            "timestamps": [Utc::now().to_rfc3339()],
        },
        "detection_metrics": {
            "secrets_detected": statistics.get("total_secrets").and_then(Value::as_i64).unwrap_or(0),
            "false_positives": statistics.get("false_positives").and_then(Value::as_i64).unwrap_or(0),
            "accuracy_rate": statistics.get("accuracy_rate").and_then(Value::as_f64).unwrap_or(0.0),
            "detection_types": statistics.get("category_breakdown").cloned().unwrap_or_else(|| json!({})),
        },
        "optimization_suggestions": [],
    }))
}

#[tauri::command]
async fn optimize_system(state: State<'_, AppState>) -> Result<Value, String> {
    state.post_json("/api/metrics/reset", json!({})).await
}

#[tauri::command]
async fn configure_webhooks(
    action: String,
    webhook: Option<Value>,
    webhook_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let path = match action.as_str() {
        "create" => "/api/webhooks/add",
        "delete" => "/api/webhooks/remove",
        "enable" | "disable" | "test" => "/api/webhooks/update",
        _ => return Err(format!("Unsupported webhook action: {}", action)),
    };

    state
        .post_json(
            path,
            json!({
                "action": action,
                "webhook": webhook,
                "webhook_id": webhook_id,
            }),
        )
        .await
}

fn detection_to_scan_result(detection: Value) -> Value {
    let detector_name = detection
        .get("detector_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let verified = detection
        .get("verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    json!({
        "id": detection.get("detection_id").or_else(|| detection.get("id")).cloned().unwrap_or_else(|| json!("")),
        "repository": detection.get("repository").cloned().unwrap_or_else(|| json!("")),
        "file_path": detection.get("file_path").cloned().unwrap_or_else(|| json!("unknown")),
        "line_number": detection.get("line_number").and_then(Value::as_i64).unwrap_or(0),
        "secret_type": detection.get("category").and_then(Value::as_str).unwrap_or(detector_name),
        "severity": detection.get("severity").and_then(Value::as_str).unwrap_or("low"),
        "content_preview": detection.get("matched_text_preview").cloned().unwrap_or_else(|| json!("[redacted]")),
        "confidence": if verified { 1.0 } else { 0.7 },
        "timestamp": detection.get("detected_at").cloned().unwrap_or_else(|| json!(Utc::now().to_rfc3339())),
        "status": if verified { "validated" } else { "new" },
    })
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                vec![byte as char]
            }
            _ => format!("%{:02X}", byte).chars().collect(),
        })
        .collect()
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("github_secret_hunter_tauri=info")
        .init();

    let app_state = AppState::from_env();
    info!("Starting Tauri shell for {}", app_state.api_base_url);

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            initialize_hunter,
            start_hunting,
            stop_hunting,
            scan_repository,
            get_dashboard_data,
            validate_secret,
            export_secrets,
            get_performance_report,
            optimize_system,
            get_recent_scan_results,
            configure_webhooks
        ])
        .run(tauri::generate_context!())
        .context("error while running tauri application")?;

    Ok(())
}
