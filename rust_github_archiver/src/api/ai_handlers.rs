use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::ai::{LocalOpenAiTriageClient, LocalOpenAiTriageConfig, RedactedTriageInput};
use crate::api::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct TriageRunRequest {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub detection_id: Option<Uuid>,
    pub secret_hash: String,
    pub detector_name: String,
    pub severity: String,
    pub category: String,
    pub repository: Option<String>,
    pub file_path: Option<String>,
    pub line_number: Option<i32>,
    pub verified: Option<bool>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersistedTriageResult {
    pub id: Uuid,
    pub detection_id: Option<Uuid>,
    pub secret_hash: String,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub redacted_input: Value,
    pub result: Value,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub async fn run_ai_triage(
    State(app_state): State<AppState>,
    Json(request): Json<TriageRunRequest>,
) -> impl IntoResponse {
    if !ai_triage_enabled() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "AI triage is disabled; set ENABLE_AI_TRIAGE=true to enable local triage"
            })),
        )
            .into_response();
    }

    let provider = request
        .provider
        .clone()
        .unwrap_or_else(|| "local-openai".to_string());
    if provider != "local-openai" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "only provider local-openai is supported"
            })),
        )
            .into_response();
    }

    let config = match triage_config(&request) {
        Ok(config) => config,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    let input = RedactedTriageInput {
        detection_id: request.detection_id,
        secret_hash: request.secret_hash.clone(),
        detector_name: request.detector_name.clone(),
        severity: request.severity.clone(),
        category: request.category.clone(),
        repository: request.repository.clone(),
        file_path: request.file_path.clone(),
        line_number: request.line_number,
        verified: request.verified.unwrap_or(false),
        source: request.source.clone(),
    };

    let client = LocalOpenAiTriageClient::new(config);
    let output = match client.triage(&input).await {
        Ok(output) => output,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    let redacted_input = json!(input);
    let result = json!(output);
    let insert_result = sqlx::query(
        r#"
        INSERT INTO ai_triage_results (
            id, detection_id, secret_hash, provider, model, base_url,
            redacted_input, result, status, completed_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, 'completed', $9
        )
        "#,
    )
    .bind(output.id)
    .bind(input.detection_id)
    .bind(&input.secret_hash)
    .bind(&output.provider)
    .bind(&output.model)
    .bind(&output.base_url)
    .bind(&redacted_input)
    .bind(&result)
    .bind(output.completed_at)
    .execute(app_state.database.pool())
    .await;

    match insert_result {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "job_id": output.id,
                "status": "completed",
                "result": output,
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_ai_triage(
    State(app_state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
    match fetch_triage_result(&app_state, job_id).await {
        Ok(Some(result)) => (StatusCode::OK, Json(json!(result))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "triage job not found" })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

async fn fetch_triage_result(
    app_state: &AppState,
    job_id: Uuid,
) -> anyhow::Result<Option<PersistedTriageResult>> {
    let row = sqlx::query(
        r#"
        SELECT
            id, detection_id, secret_hash, provider, model, base_url,
            redacted_input, result, status, error_message, created_at, completed_at
        FROM ai_triage_results
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(app_state.database.pool())
    .await?;

    Ok(row.map(|row| PersistedTriageResult {
        id: row.get("id"),
        detection_id: row.get("detection_id"),
        secret_hash: row.get("secret_hash"),
        provider: row.get("provider"),
        model: row.get("model"),
        base_url: row.get("base_url"),
        redacted_input: row.get("redacted_input"),
        result: row.get("result"),
        status: row.get("status"),
        error_message: row.get("error_message"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    }))
}

fn triage_config(request: &TriageRunRequest) -> anyhow::Result<LocalOpenAiTriageConfig> {
    let env_config = LocalOpenAiTriageConfig::from_env();
    let base_url = request
        .base_url
        .clone()
        .or_else(|| {
            env_config
                .as_ref()
                .ok()
                .map(|config| config.base_url.clone())
        })
        .unwrap_or_else(|| "http://127.0.0.1:11434/v1".to_string());
    let model = request
        .model
        .clone()
        .or_else(|| env_config.as_ref().ok().map(|config| config.model.clone()))
        .ok_or_else(|| anyhow::anyhow!("AI triage requires request.model or AI_TRIAGE_MODEL"))?;
    let api_key = env_config.ok().and_then(|config| config.api_key);

    Ok(LocalOpenAiTriageConfig {
        base_url,
        model,
        api_key,
    })
}

fn ai_triage_enabled() -> bool {
    std::env::var("ENABLE_AI_TRIAGE")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn request_with_model() -> TriageRunRequest {
        TriageRunRequest {
            provider: Some("local-openai".to_string()),
            base_url: Some("http://127.0.0.1:11434/v1".to_string()),
            model: Some("local-triage".to_string()),
            detection_id: None,
            secret_hash: "hash-only".to_string(),
            detector_name: "GitHub Token".to_string(),
            severity: "high".to_string(),
            category: "token".to_string(),
            repository: Some("owner/repo".to_string()),
            file_path: Some(".env".to_string()),
            line_number: Some(10),
            verified: Some(true),
            source: Some("manual".to_string()),
        }
    }

    #[test]
    fn ai_triage_enabled_accepts_only_explicit_truthy_values() {
        let _guard = env_lock();

        std::env::remove_var("ENABLE_AI_TRIAGE");
        assert!(!ai_triage_enabled());

        for value in ["1", "true", "TRUE", "yes"] {
            std::env::set_var("ENABLE_AI_TRIAGE", value);
            assert!(ai_triage_enabled(), "{value} should enable triage");
        }

        for value in ["0", "false", "no", ""] {
            std::env::set_var("ENABLE_AI_TRIAGE", value);
            assert!(!ai_triage_enabled(), "{value} should not enable triage");
        }

        std::env::remove_var("ENABLE_AI_TRIAGE");
    }

    #[test]
    fn triage_config_prefers_request_base_url_and_model() {
        let _guard = env_lock();
        std::env::remove_var("AI_TRIAGE_BASE_URL");
        std::env::remove_var("AI_TRIAGE_MODEL");
        std::env::remove_var("AI_TRIAGE_API_KEY");

        let config = triage_config(&request_with_model()).expect("config");

        assert_eq!(config.base_url, "http://127.0.0.1:11434/v1");
        assert_eq!(config.model, "local-triage");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn triage_config_requires_model_from_request_or_environment() {
        let _guard = env_lock();
        std::env::remove_var("AI_TRIAGE_BASE_URL");
        std::env::remove_var("AI_TRIAGE_MODEL");
        std::env::remove_var("AI_TRIAGE_API_KEY");
        let mut request = request_with_model();
        request.model = None;

        let error = triage_config(&request).expect_err("model should be required");

        assert!(error.to_string().contains("AI_TRIAGE_MODEL"));
    }

    #[test]
    fn triage_request_contains_only_redacted_secret_identity() {
        let request = request_with_model();
        let encoded = serde_json::to_value(RedactedTriageInput {
            detection_id: request.detection_id,
            secret_hash: request.secret_hash,
            detector_name: request.detector_name,
            severity: request.severity,
            category: request.category,
            repository: request.repository,
            file_path: request.file_path,
            line_number: request.line_number,
            verified: request.verified.unwrap_or(false),
            source: request.source,
        })
        .expect("redacted input json");

        assert_eq!(encoded["secret_hash"], "hash-only");
        assert!(encoded.get("matched_text").is_none());
        assert!(encoded.get("raw").is_none());
    }
}
