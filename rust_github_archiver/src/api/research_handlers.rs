use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::api::state::AppState;
use crate::auth::User;

#[derive(Debug, Clone, Deserialize)]
pub struct ResearchListQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResearchCandidate {
    pub source_type: String,
    pub source_detection_id: Option<Uuid>,
    pub source_event_id: Option<i64>,
    pub title: String,
    pub repository: Option<String>,
    pub severity: Option<String>,
    pub playbook: String,
    pub created_at: DateTime<Utc>,
    pub raw_evidence: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResearchFinding {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub source_type: String,
    pub source_detection_id: Option<Uuid>,
    pub source_event_id: Option<i64>,
    pub program_name: Option<String>,
    pub scope_asset: Option<String>,
    pub scope_status: String,
    pub playbook: Option<String>,
    pub severity: Option<String>,
    pub repository: Option<String>,
    pub raw_evidence: Value,
    pub derived_metadata: Value,
    pub notes: Option<String>,
    pub readiness_score: i32,
    pub readiness_blockers: Value,
    pub ai_outputs: Value,
    pub export_history: Value,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateResearchFindingRequest {
    pub source_type: Option<String>,
    pub source_detection_id: Option<Uuid>,
    pub source_event_id: Option<i64>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub program_name: Option<String>,
    pub scope_asset: Option<String>,
    pub scope_status: Option<String>,
    pub playbook: Option<String>,
    pub severity: Option<String>,
    pub repository: Option<String>,
    pub raw_evidence: Option<Value>,
    pub derived_metadata: Option<Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateResearchFindingRequest {
    pub title: Option<String>,
    pub status: Option<String>,
    pub program_name: Option<String>,
    pub scope_asset: Option<String>,
    pub scope_status: Option<String>,
    pub playbook: Option<String>,
    pub severity: Option<String>,
    pub repository: Option<String>,
    pub raw_evidence: Option<Value>,
    pub derived_metadata: Option<Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadinessScore {
    pub score: i32,
    pub blockers: Vec<String>,
    pub components: HashMap<String, i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResearchAiAssistRequest {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub include_full_evidence: Option<bool>,
    pub confirmed_full_evidence: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportResearchFindingRequest {
    pub format: Option<String>,
    pub redacted: Option<bool>,
}

pub async fn list_research_candidates(
    State(app_state): State<AppState>,
    Query(query): Query<ResearchListQuery>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    let limit = clamp_limit(query.limit, 30, 100);
    match fetch_research_candidates(&app_state, limit).await {
        Ok(candidates) => {
            (StatusCode::OK, Json(json!({ "candidates": candidates }))).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn list_research_findings(
    State(app_state): State<AppState>,
    Query(query): Query<ResearchListQuery>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    let limit = clamp_limit(query.limit, 50, 200);
    match fetch_research_findings(&app_state, limit).await {
        Ok(findings) => (StatusCode::OK, Json(json!({ "findings": findings }))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_research_finding(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    match fetch_research_finding(&app_state, id).await {
        Ok(Some(finding)) => (StatusCode::OK, Json(json!(finding))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "research finding not found" })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn create_research_finding(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateResearchFindingRequest>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    match insert_research_finding(&app_state, &user.username, request).await {
        Ok(finding) => (StatusCode::CREATED, Json(json!(finding))).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn update_research_finding(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateResearchFindingRequest>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    match update_research_finding_record(&app_state, &user.username, id, request).await {
        Ok(Some(finding)) => (StatusCode::OK, Json(json!(finding))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "research finding not found" })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn score_research_finding(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    let Some(finding) = (match fetch_research_finding(&app_state, id).await {
        Ok(finding) => finding,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    }) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "research finding not found" })),
        )
            .into_response();
    };

    let score = calculate_readiness_score(&finding);
    match persist_readiness_score(&app_state, &user.username, id, &score).await {
        Ok(updated) => (
            StatusCode::OK,
            Json(json!({
                "score": score,
                "finding": updated,
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

pub async fn export_research_finding(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    Json(request): Json<ExportResearchFindingRequest>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    let Some(finding) = (match fetch_research_finding(&app_state, id).await {
        Ok(finding) => finding,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    }) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "research finding not found" })),
        )
            .into_response();
    };

    let format = request.format.unwrap_or_else(|| "markdown".to_string());
    let redacted = request.redacted.unwrap_or(true);
    let export = match format.as_str() {
        "markdown" | "md" => json!({
            "format": "markdown",
            "content_type": "text/markdown",
            "content": build_markdown_report(&finding, redacted),
        }),
        "json" => {
            let payload = if redacted {
                redacted_finding_value(&finding)
            } else {
                json!(finding)
            };
            json!({
                "format": "json",
                "content_type": "application/json",
                "content": payload,
            })
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "format must be markdown or json" })),
            )
                .into_response();
        }
    };

    let history_item = json!({
        "exported_at": Utc::now(),
        "exported_by": user.username,
        "format": format,
        "redacted": redacted,
    });
    let _ = append_export_history(&app_state, id, history_item).await;

    (StatusCode::OK, Json(export)).into_response()
}

pub async fn run_research_ai_assist(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    Json(request): Json<ResearchAiAssistRequest>,
) -> impl IntoResponse {
    if !research_mode_enabled() {
        return research_disabled_response();
    }

    let Some(finding) = (match fetch_research_finding(&app_state, id).await {
        Ok(finding) => finding,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    }) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "research finding not found" })),
        )
            .into_response();
    };

    let provider = normalize_provider(request.provider.as_deref().unwrap_or("local-openai"));
    let include_full_evidence = request.include_full_evidence.unwrap_or(false);
    let is_external = provider_is_external(&provider);
    let evidence = build_ai_evidence(&finding, include_full_evidence);

    if is_external && !external_ai_research_enabled() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "external research AI is disabled; set ENABLE_EXTERNAL_AI_RESEARCH=true"
            })),
        )
            .into_response();
    }

    if needs_full_evidence_confirmation(
        &provider,
        include_full_evidence,
        request.confirmed_full_evidence.unwrap_or(false),
    ) {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "confirmation_required": true,
                "reason": "external AI full-evidence sends require explicit per-job confirmation",
                "provider": provider,
                "evidence_preview": evidence,
            })),
        )
            .into_response();
    }

    let config = match resolve_ai_provider_config(&provider, &request) {
        Ok(config) => config,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    let prompt = request.prompt.unwrap_or_else(|| {
        "Improve the bug bounty report draft. Focus on reproducibility, impact clarity, scope fit, and missing evidence. Do not invent facts.".to_string()
    });
    let system_prompt = "You are a security research report assistant. Work only from supplied evidence, call out uncertainty, and do not suggest unauthorized live testing.";
    let user_prompt = format!(
        "Operator prompt:\n{}\n\nEvidence bundle:\n{}",
        prompt,
        serde_json::to_string_pretty(&evidence).unwrap_or_else(|_| "{}".to_string())
    );

    let result = match call_research_ai(&config, system_prompt, &user_prompt).await {
        Ok(content) => content,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    let output = json!({
        "id": Uuid::new_v4(),
        "created_at": Utc::now(),
        "created_by": user.username,
        "provider": provider,
        "model": config.model,
        "base_url": config.base_url,
        "full_evidence_sent": include_full_evidence,
        "external_provider": is_external,
        "prompt_hash": sha256_hex(&user_prompt),
        "result": result,
    });

    match append_ai_output(&app_state, id, &user.username, output.clone()).await {
        Ok(updated) => (
            StatusCode::OK,
            Json(json!({
                "ai_output": output,
                "finding": updated,
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

async fn fetch_research_candidates(
    app_state: &AppState,
    limit: i64,
) -> anyhow::Result<Vec<ResearchCandidate>> {
    let detection_rows = sqlx::query(
        r#"
        SELECT
            detection_id, event_id, repository, file_path, detector_name,
            severity, category, detected_at, verified, source,
            matched_text_preview, line_number
        FROM secret_detections detections
        WHERE NOT EXISTS (
            SELECT 1 FROM research_findings findings
            WHERE findings.source_detection_id = detections.detection_id
        )
        ORDER BY detected_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(app_state.database.pool())
    .await?;

    let mut candidates = Vec::new();
    for row in detection_rows {
        let detector_name: String = row.get("detector_name");
        let repository: String = row.get("repository");
        let severity: String = row.get("severity");
        candidates.push(ResearchCandidate {
            source_type: "detection".to_string(),
            source_detection_id: Some(row.get("detection_id")),
            source_event_id: row.get("event_id"),
            title: format!("{detector_name} in {repository}"),
            repository: Some(repository.clone()),
            severity: Some(severity.clone()),
            playbook: playbook_for_detection(&detector_name, row.get::<String, _>("category")),
            created_at: row.get("detected_at"),
            raw_evidence: json!({
                "detection_id": row.get::<Uuid, _>("detection_id"),
                "event_id": row.get::<Option<i64>, _>("event_id"),
                "repository": repository,
                "file_path": row.get::<Option<String>, _>("file_path"),
                "detector_name": detector_name,
                "severity": severity,
                "category": row.get::<String, _>("category"),
                "verified": row.get::<bool, _>("verified"),
                "source": row.get::<String, _>("source"),
                "matched_text_preview": row.get::<String, _>("matched_text_preview"),
                "line_number": row.get::<Option<i32>, _>("line_number"),
            }),
        });
    }

    if candidates.len() < limit as usize {
        let remaining = limit - candidates.len() as i64;
        let event_rows = sqlx::query(
            r#"
            SELECT
                events.event_id,
                events.event_type,
                events.event_created_at,
                COALESCE(
                    events.repo_full_name,
                    NULLIF(CONCAT_WS('/', events.repo_owner_login, events.repo_name), ''),
                    events.repo_name,
                    'unknown'
                ) AS repository,
                events.actor_login,
                events.payload,
                queue.status AS queue_status,
                queue.error_message
            FROM github_events events
            LEFT JOIN pending_push_scans queue ON queue.event_id = events.event_id
            WHERE events.event_type IN ('PushEvent', 'CreateEvent', 'ReleaseEvent', 'PullRequestEvent')
              AND NOT EXISTS (
                  SELECT 1 FROM research_findings findings
                  WHERE findings.source_event_id = events.event_id
              )
            ORDER BY events.event_created_at DESC
            LIMIT $1
            "#,
        )
        .bind(remaining)
        .fetch_all(app_state.database.pool())
        .await?;

        for row in event_rows {
            let event_type: String = row.get("event_type");
            let repository: String = row.get("repository");
            let queue_status: Option<String> = row.get("queue_status");
            candidates.push(ResearchCandidate {
                source_type: "event".to_string(),
                source_detection_id: None,
                source_event_id: Some(row.get("event_id")),
                title: format!("{event_type} in {repository}"),
                repository: Some(repository.clone()),
                severity: Some(event_severity(queue_status.as_deref()).to_string()),
                playbook: "repo-event-anomaly".to_string(),
                created_at: row.get("event_created_at"),
                raw_evidence: json!({
                    "event_id": row.get::<i64, _>("event_id"),
                    "event_type": event_type,
                    "repository": repository,
                    "actor_login": row.get::<Option<String>, _>("actor_login"),
                    "queue_status": queue_status,
                    "queue_error": row.get::<Option<String>, _>("error_message"),
                    "payload": row.get::<Option<Value>, _>("payload").unwrap_or_else(|| json!({})),
                }),
            });
        }
    }

    Ok(candidates)
}

async fn fetch_research_findings(
    app_state: &AppState,
    limit: i64,
) -> anyhow::Result<Vec<ResearchFinding>> {
    let rows = sqlx::query(
        r#"
        SELECT *
        FROM research_findings
        ORDER BY updated_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(app_state.database.pool())
    .await?;

    Ok(rows.into_iter().map(row_to_research_finding).collect())
}

async fn fetch_research_finding(
    app_state: &AppState,
    id: Uuid,
) -> anyhow::Result<Option<ResearchFinding>> {
    let row = sqlx::query("SELECT * FROM research_findings WHERE id = $1")
        .bind(id)
        .fetch_optional(app_state.database.pool())
        .await?;

    Ok(row.map(row_to_research_finding))
}

async fn insert_research_finding(
    app_state: &AppState,
    username: &str,
    request: CreateResearchFindingRequest,
) -> anyhow::Result<ResearchFinding> {
    let raw_evidence = match request.raw_evidence {
        Some(value) => value,
        None => hydrate_source_evidence(
            app_state,
            request.source_detection_id,
            request.source_event_id,
        )
        .await?
        .unwrap_or_else(|| json!({})),
    };

    let source_type = request.source_type.unwrap_or_else(|| {
        if request.source_detection_id.is_some() {
            "detection".to_string()
        } else if request.source_event_id.is_some() {
            "event".to_string()
        } else {
            "manual".to_string()
        }
    });
    let repository = request.repository.or_else(|| {
        raw_evidence
            .get("repository")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let severity = request.severity.or_else(|| {
        raw_evidence
            .get("severity")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let playbook = request
        .playbook
        .or_else(|| {
            raw_evidence
                .get("playbook")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| playbook_for_evidence(&raw_evidence));
    let title = request.title.unwrap_or_else(|| {
        let detector = raw_evidence
            .get("detector_name")
            .or_else(|| raw_evidence.get("event_type"))
            .and_then(Value::as_str)
            .unwrap_or("Research candidate");
        let repo = repository.as_deref().unwrap_or("unknown asset");
        format!("{detector} in {repo}")
    });

    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO research_findings (
            id, title, status, source_type, source_detection_id, source_event_id,
            program_name, scope_asset, scope_status, playbook, severity, repository,
            raw_evidence, derived_metadata, notes, created_by, updated_by
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9, $10, $11, $12,
            $13, $14, $15, $16, $17
        )
        "#,
    )
    .bind(id)
    .bind(title)
    .bind(request.status.unwrap_or_else(|| "draft".to_string()))
    .bind(source_type)
    .bind(request.source_detection_id)
    .bind(request.source_event_id)
    .bind(request.program_name)
    .bind(request.scope_asset)
    .bind(
        request
            .scope_status
            .unwrap_or_else(|| "unknown".to_string()),
    )
    .bind(playbook)
    .bind(severity)
    .bind(repository)
    .bind(raw_evidence)
    .bind(request.derived_metadata.unwrap_or_else(|| json!({})))
    .bind(request.notes)
    .bind(username)
    .bind(username)
    .execute(app_state.database.pool())
    .await?;

    fetch_research_finding(app_state, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("research finding insert did not return a row"))
}

async fn update_research_finding_record(
    app_state: &AppState,
    username: &str,
    id: Uuid,
    request: UpdateResearchFindingRequest,
) -> anyhow::Result<Option<ResearchFinding>> {
    let Some(existing) = fetch_research_finding(app_state, id).await? else {
        return Ok(None);
    };

    sqlx::query(
        r#"
        UPDATE research_findings
        SET
            title = $2,
            status = $3,
            program_name = $4,
            scope_asset = $5,
            scope_status = $6,
            playbook = $7,
            severity = $8,
            repository = $9,
            raw_evidence = $10,
            derived_metadata = $11,
            notes = $12,
            updated_by = $13,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(request.title.unwrap_or(existing.title))
    .bind(request.status.unwrap_or(existing.status))
    .bind(request.program_name.or(existing.program_name))
    .bind(request.scope_asset.or(existing.scope_asset))
    .bind(request.scope_status.unwrap_or(existing.scope_status))
    .bind(request.playbook.or(existing.playbook))
    .bind(request.severity.or(existing.severity))
    .bind(request.repository.or(existing.repository))
    .bind(request.raw_evidence.unwrap_or(existing.raw_evidence))
    .bind(
        request
            .derived_metadata
            .unwrap_or(existing.derived_metadata),
    )
    .bind(request.notes.or(existing.notes))
    .bind(username)
    .execute(app_state.database.pool())
    .await?;

    fetch_research_finding(app_state, id).await
}

async fn hydrate_source_evidence(
    app_state: &AppState,
    detection_id: Option<Uuid>,
    event_id: Option<i64>,
) -> anyhow::Result<Option<Value>> {
    if let Some(detection_id) = detection_id {
        let row = sqlx::query(
            r#"
            SELECT
                detection_id, event_id, repository, file_path, detector_name,
                severity, category, detected_at, verified, source,
                matched_text_preview, line_number, metadata
            FROM secret_detections
            WHERE detection_id = $1
            "#,
        )
        .bind(detection_id)
        .fetch_optional(app_state.database.pool())
        .await?;

        return Ok(row.map(|row| {
            json!({
                "detection_id": row.get::<Uuid, _>("detection_id"),
                "event_id": row.get::<Option<i64>, _>("event_id"),
                "repository": row.get::<String, _>("repository"),
                "file_path": row.get::<Option<String>, _>("file_path"),
                "detector_name": row.get::<String, _>("detector_name"),
                "severity": row.get::<String, _>("severity"),
                "category": row.get::<String, _>("category"),
                "detected_at": row.get::<DateTime<Utc>, _>("detected_at"),
                "verified": row.get::<bool, _>("verified"),
                "source": row.get::<String, _>("source"),
                "matched_text_preview": row.get::<String, _>("matched_text_preview"),
                "line_number": row.get::<Option<i32>, _>("line_number"),
                "metadata": row.get::<Option<Value>, _>("metadata").unwrap_or_else(|| json!({})),
            })
        }));
    }

    if let Some(event_id) = event_id {
        let row = sqlx::query(
            r#"
            SELECT
                events.event_id,
                events.event_type,
                events.event_created_at,
                COALESCE(
                    events.repo_full_name,
                    NULLIF(CONCAT_WS('/', events.repo_owner_login, events.repo_name), ''),
                    events.repo_name,
                    'unknown'
                ) AS repository,
                events.actor_login,
                events.payload,
                events.raw_event,
                queue.status AS queue_status,
                queue.error_message
            FROM github_events events
            LEFT JOIN pending_push_scans queue ON queue.event_id = events.event_id
            WHERE events.event_id = $1
            "#,
        )
        .bind(event_id)
        .fetch_optional(app_state.database.pool())
        .await?;

        return Ok(row.map(|row| {
            json!({
                "event_id": row.get::<i64, _>("event_id"),
                "event_type": row.get::<String, _>("event_type"),
                "event_created_at": row.get::<DateTime<Utc>, _>("event_created_at"),
                "repository": row.get::<String, _>("repository"),
                "actor_login": row.get::<Option<String>, _>("actor_login"),
                "queue_status": row.get::<Option<String>, _>("queue_status"),
                "queue_error": row.get::<Option<String>, _>("error_message"),
                "payload": row.get::<Option<Value>, _>("payload").unwrap_or_else(|| json!({})),
                "raw_event": row.get::<Option<Value>, _>("raw_event").unwrap_or_else(|| json!({})),
            })
        }));
    }

    Ok(None)
}

async fn persist_readiness_score(
    app_state: &AppState,
    username: &str,
    id: Uuid,
    score: &ReadinessScore,
) -> anyhow::Result<ResearchFinding> {
    sqlx::query(
        r#"
        UPDATE research_findings
        SET readiness_score = $2,
            readiness_blockers = $3,
            updated_by = $4,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(score.score)
    .bind(json!(score.blockers))
    .bind(username)
    .execute(app_state.database.pool())
    .await?;

    fetch_research_finding(app_state, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("research finding disappeared during score update"))
}

async fn append_ai_output(
    app_state: &AppState,
    id: Uuid,
    username: &str,
    output: Value,
) -> anyhow::Result<ResearchFinding> {
    let Some(existing) = fetch_research_finding(app_state, id).await? else {
        return Err(anyhow::anyhow!("research finding not found"));
    };
    let updated_outputs = append_json_array(existing.ai_outputs, output);
    sqlx::query(
        r#"
        UPDATE research_findings
        SET ai_outputs = $2,
            updated_by = $3,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(updated_outputs)
    .bind(username)
    .execute(app_state.database.pool())
    .await?;

    fetch_research_finding(app_state, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("research finding disappeared during AI update"))
}

async fn append_export_history(
    app_state: &AppState,
    id: Uuid,
    history_item: Value,
) -> anyhow::Result<()> {
    let Some(existing) = fetch_research_finding(app_state, id).await? else {
        return Ok(());
    };
    let updated_history = append_json_array(existing.export_history, history_item);
    sqlx::query(
        r#"
        UPDATE research_findings
        SET export_history = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(updated_history)
    .execute(app_state.database.pool())
    .await?;
    Ok(())
}

fn row_to_research_finding(row: sqlx::postgres::PgRow) -> ResearchFinding {
    ResearchFinding {
        id: row.get("id"),
        title: row.get("title"),
        status: row.get("status"),
        source_type: row.get("source_type"),
        source_detection_id: row.get("source_detection_id"),
        source_event_id: row.get("source_event_id"),
        program_name: row.get("program_name"),
        scope_asset: row.get("scope_asset"),
        scope_status: row.get("scope_status"),
        playbook: row.get("playbook"),
        severity: row.get("severity"),
        repository: row.get("repository"),
        raw_evidence: row.get("raw_evidence"),
        derived_metadata: row.get("derived_metadata"),
        notes: row.get("notes"),
        readiness_score: row.get("readiness_score"),
        readiness_blockers: row.get("readiness_blockers"),
        ai_outputs: row.get("ai_outputs"),
        export_history: row.get("export_history"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn calculate_readiness_score(finding: &ResearchFinding) -> ReadinessScore {
    let mut components = HashMap::new();
    let mut blockers = Vec::new();

    let reproducibility = if finding.source_detection_id.is_some()
        || finding.source_event_id.is_some()
        || text_has_repro_steps(finding.notes.as_deref())
    {
        20
    } else {
        blockers.push("Missing exact source event/detection or reproduction steps".to_string());
        5
    };
    components.insert("reproducibility".to_string(), reproducibility);

    let scope_fit = if !is_blank(finding.scope_asset.as_deref())
        && finding.scope_status.eq_ignore_ascii_case("in_scope")
    {
        20
    } else if !is_blank(finding.scope_asset.as_deref()) {
        blockers.push("Scope asset is present but not marked in_scope".to_string());
        10
    } else {
        blockers.push("Missing program scope asset".to_string());
        0
    };
    components.insert("scope_fit".to_string(), scope_fit);

    let evidence_quality = if !finding
        .raw_evidence
        .as_object()
        .is_none_or(|map| map.is_empty())
        && !is_blank(finding.repository.as_deref())
    {
        20
    } else {
        blockers.push("Missing repository or evidence bundle".to_string());
        5
    };
    components.insert("evidence_quality".to_string(), evidence_quality);

    let impact = if text_has_impact(finding.notes.as_deref())
        || finding.derived_metadata.get("impact").is_some()
    {
        20
    } else {
        blockers.push("Missing concrete impact statement".to_string());
        5
    };
    components.insert("impact_clarity".to_string(), impact);

    let dedupe = if finding.derived_metadata.get("duplicate_of").is_some() {
        blockers.push("Potential duplicate is recorded".to_string());
        0
    } else {
        10
    };
    components.insert("dedupe_risk".to_string(), dedupe);

    let safety = if finding
        .notes
        .as_deref()
        .is_some_and(contains_likely_secret_material)
    {
        blockers.push("Report notes appear to contain raw secret material".to_string());
        0
    } else {
        10
    };
    components.insert("safety_redaction".to_string(), safety);

    let score = components.values().sum::<i32>().clamp(0, 100);
    ReadinessScore {
        score,
        blockers,
        components,
    }
}

fn build_markdown_report(finding: &ResearchFinding, redacted: bool) -> String {
    let blockers = finding
        .readiness_blockers
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "- None recorded".to_string());
    let notes = finding.notes.as_deref().unwrap_or("No notes recorded.");
    let notes = if redacted {
        redact_text(notes)
    } else {
        notes.to_string()
    };
    let evidence = if redacted {
        redact_value(&finding.raw_evidence)
    } else {
        finding.raw_evidence.clone()
    };
    let evidence_text =
        serde_json::to_string_pretty(&evidence).unwrap_or_else(|_| "{}".to_string());

    format!(
        "# {}\n\n\
         ## Summary\n\
         - Status: {}\n\
         - Program: {}\n\
         - Scope asset: {}\n\
         - Scope status: {}\n\
         - Repository: {}\n\
         - Severity: {}\n\
         - Playbook: {}\n\
         - Readiness score: {}/100\n\n\
         ## Blockers\n{}\n\n\
         ## Notes\n{}\n\n\
         ## Evidence\n```json\n{}\n```\n",
        finding.title,
        finding.status,
        finding.program_name.as_deref().unwrap_or("not set"),
        finding.scope_asset.as_deref().unwrap_or("not set"),
        finding.scope_status,
        finding.repository.as_deref().unwrap_or("not set"),
        finding.severity.as_deref().unwrap_or("not set"),
        finding.playbook.as_deref().unwrap_or("not set"),
        finding.readiness_score,
        blockers,
        notes,
        evidence_text
    )
}

fn redacted_finding_value(finding: &ResearchFinding) -> Value {
    let mut value = json!(finding);
    if let Some(obj) = value.as_object_mut() {
        if let Some(raw) = obj.get("raw_evidence").cloned() {
            obj.insert("raw_evidence".to_string(), redact_value(&raw));
        }
        if let Some(notes) = obj.get("notes").and_then(Value::as_str) {
            obj.insert("notes".to_string(), json!(redact_text(notes)));
        }
    }
    value
}

fn build_ai_evidence(finding: &ResearchFinding, include_full_evidence: bool) -> Value {
    let raw_evidence = if include_full_evidence {
        finding.raw_evidence.clone()
    } else {
        redact_value(&finding.raw_evidence)
    };
    json!({
        "finding_id": finding.id,
        "title": finding.title,
        "status": finding.status,
        "program_name": finding.program_name,
        "scope_asset": finding.scope_asset,
        "scope_status": finding.scope_status,
        "playbook": finding.playbook,
        "severity": finding.severity,
        "repository": finding.repository,
        "notes": if include_full_evidence {
            finding.notes.clone()
        } else {
            finding.notes.as_deref().map(redact_text)
        },
        "readiness_score": finding.readiness_score,
        "readiness_blockers": finding.readiness_blockers,
        "raw_evidence": raw_evidence,
    })
}

#[derive(Debug, Clone)]
struct ResearchAiProviderConfig {
    provider: String,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

fn resolve_ai_provider_config(
    provider: &str,
    request: &ResearchAiAssistRequest,
) -> anyhow::Result<ResearchAiProviderConfig> {
    let model = request
        .model
        .clone()
        .or_else(|| std::env::var("RESEARCH_AI_MODEL").ok())
        .or_else(|| std::env::var("AI_TRIAGE_MODEL").ok())
        .ok_or_else(|| {
            anyhow::anyhow!("research AI requires request.model or RESEARCH_AI_MODEL")
        })?;

    match provider {
        "local-openai" => {
            let base_url = request
                .base_url
                .clone()
                .or_else(|| std::env::var("RESEARCH_AI_BASE_URL").ok())
                .or_else(|| std::env::var("AI_TRIAGE_BASE_URL").ok())
                .unwrap_or_else(|| "http://127.0.0.1:11434/v1".to_string());
            validate_local_ai_base_url(&base_url)?;
            Ok(ResearchAiProviderConfig {
                provider: provider.to_string(),
                base_url,
                model,
                api_key: std::env::var("RESEARCH_AI_API_KEY")
                    .ok()
                    .or_else(|| std::env::var("AI_TRIAGE_API_KEY").ok()),
            })
        }
        "openai" => {
            reject_request_base_url_for_external_provider(request)?;
            Ok(ResearchAiProviderConfig {
                provider: provider.to_string(),
                base_url: std::env::var("RESEARCH_OPENAI_BASE_URL")
                    .ok()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                model,
                api_key: std::env::var("RESEARCH_OPENAI_API_KEY")
                    .ok()
                    .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                    .or_else(|| std::env::var("RESEARCH_AI_API_KEY").ok()),
            })
        }
        "anthropic" => {
            reject_request_base_url_for_external_provider(request)?;
            Ok(ResearchAiProviderConfig {
                provider: provider.to_string(),
                base_url: std::env::var("RESEARCH_ANTHROPIC_BASE_URL")
                    .ok()
                    .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
                model,
                api_key: std::env::var("RESEARCH_ANTHROPIC_API_KEY")
                    .ok()
                    .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                    .or_else(|| std::env::var("RESEARCH_AI_API_KEY").ok()),
            })
        }
        _ => Err(anyhow::anyhow!(
            "provider must be local-openai, openai, or anthropic"
        )),
    }
}

fn reject_request_base_url_for_external_provider(
    request: &ResearchAiAssistRequest,
) -> anyhow::Result<()> {
    if request.base_url.is_some() {
        return Err(anyhow::anyhow!(
            "external research AI providers use configured base URLs only"
        ));
    }
    Ok(())
}

fn validate_local_ai_base_url(base_url: &str) -> anyhow::Result<()> {
    let parsed = reqwest::Url::parse(base_url)
        .map_err(|_| anyhow::anyhow!("local research AI base_url must be a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(anyhow::anyhow!(
            "local research AI base_url must use http or https"
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("local research AI base_url must include a host"))?;
    if host.eq_ignore_ascii_case("localhost") {
        return Ok(());
    }
    let ip = host
        .parse::<std::net::IpAddr>()
        .map_err(|_| anyhow::anyhow!("local research AI base_url must point to loopback"))?;
    if ip.is_loopback() {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "local research AI base_url must point to loopback"
    ))
}

async fn call_research_ai(
    config: &ResearchAiProviderConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> anyhow::Result<String> {
    match config.provider.as_str() {
        "anthropic" => call_anthropic(config, system_prompt, user_prompt).await,
        "local-openai" | "openai" => {
            call_openai_compatible(config, system_prompt, user_prompt).await
        }
        _ => Err(anyhow::anyhow!("unsupported provider {}", config.provider)),
    }
}

async fn call_openai_compatible(
    config: &ResearchAiProviderConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> anyhow::Result<String> {
    if provider_is_external(&config.provider) && config.api_key.is_none() {
        return Err(anyhow::anyhow!(
            "{} requires an API key environment variable",
            config.provider
        ));
    }

    let endpoint = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut request = client.post(endpoint).json(&json!({
        "model": config.model,
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]
    }));
    if let Some(api_key) = &config.api_key {
        request = request.bearer_auth(api_key);
    }
    let response: Value = request.send().await?.error_for_status()?.json().await?;
    response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow::anyhow!("OpenAI-compatible response did not include message content")
        })
}

async fn call_anthropic(
    config: &ResearchAiProviderConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> anyhow::Result<String> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("anthropic requires an API key environment variable"))?;
    let endpoint = format!("{}/v1/messages", config.base_url.trim_end_matches('/'));
    let response: Value = reqwest::Client::new()
        .post(endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": config.model,
            "max_tokens": 1200,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": user_prompt}
            ]
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    response
        .pointer("/content/0/text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Anthropic response did not include text content"))
}

fn research_mode_enabled() -> bool {
    std::env::var("ENABLE_RESEARCH_MODE")
        .map(|value| !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "no"))
        .unwrap_or(true)
}

fn external_ai_research_enabled() -> bool {
    std::env::var("ENABLE_EXTERNAL_AI_RESEARCH")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

fn research_disabled_response() -> axum::response::Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": "research mode is disabled; set ENABLE_RESEARCH_MODE=true"
        })),
    )
        .into_response()
}

fn clamp_limit(limit: Option<i64>, default: i64, max: i64) -> i64 {
    limit.unwrap_or(default).clamp(1, max)
}

fn playbook_for_detection(detector: &str, category: String) -> String {
    let text = format!("{detector} {category}").to_ascii_lowercase();
    if text.contains("webhook") {
        "webhook-exposure".to_string()
    } else if text.contains("token")
        || text.contains("key")
        || text.contains("secret")
        || text.contains("password")
    {
        "secret-leak".to_string()
    } else {
        "api-auth-review".to_string()
    }
}

fn playbook_for_evidence(evidence: &Value) -> String {
    let detector = evidence
        .get("detector_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let category = evidence
        .get("category")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !detector.is_empty() || !category.is_empty() {
        return playbook_for_detection(detector, category);
    }
    "repo-event-anomaly".to_string()
}

fn event_severity(queue_status: Option<&str>) -> &'static str {
    match queue_status {
        Some("failed") => "medium",
        Some("processing") => "low",
        _ => "informational",
    }
}

fn provider_is_external(provider: &str) -> bool {
    matches!(provider, "openai" | "anthropic")
}

fn normalize_provider(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "gpt" | "chatgpt" | "openai" => "openai".to_string(),
        "claude" | "anthropic" => "anthropic".to_string(),
        _ => "local-openai".to_string(),
    }
}

fn needs_full_evidence_confirmation(
    provider: &str,
    include_full_evidence: bool,
    confirmed_full_evidence: bool,
) -> bool {
    provider_is_external(provider) && include_full_evidence && !confirmed_full_evidence
}

fn append_json_array(existing: Value, item: Value) -> Value {
    let mut items = existing.as_array().cloned().unwrap_or_default();
    items.push(item);
    Value::Array(items)
}

fn text_has_repro_steps(text: Option<&str>) -> bool {
    let Some(text) = text else {
        return false;
    };
    let text = text.to_ascii_lowercase();
    text.contains("step") || text.contains("repro") || text.contains("request")
}

fn text_has_impact(text: Option<&str>) -> bool {
    let Some(text) = text else {
        return false;
    };
    let text = text.to_ascii_lowercase();
    text.contains("impact") || text.contains("access") || text.contains("leak")
}

fn is_blank(value: Option<&str>) -> bool {
    value.map(str::trim).unwrap_or_default().is_empty()
}

fn contains_likely_secret_material(text: &str) -> bool {
    secret_regex().is_match(text)
}

fn redact_text(text: &str) -> String {
    secret_regex()
        .replace_all(text, "[REDACTED_SECRET]")
        .to_string()
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_text(text)),
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if key.to_ascii_lowercase().contains("secret")
                        || key.to_ascii_lowercase().contains("token")
                        || key.to_ascii_lowercase().contains("password")
                    {
                        (key.clone(), json!("[REDACTED_SECRET]"))
                    } else {
                        (key.clone(), redact_value(value))
                    }
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn secret_regex() -> &'static Regex {
    static SECRET_REGEX: OnceLock<Regex> = OnceLock::new();
    SECRET_REGEX.get_or_init(|| {
        Regex::new(
            r#"(?ix)
            gh[pousr]_[A-Za-z0-9_]{20,} |
            github_pat_[A-Za-z0-9_]{20,} |
            sk-[A-Za-z0-9_-]{20,} |
            xox[baprs]-[A-Za-z0-9-]{20,} |
            AKIA[0-9A-Z]{16} |
            (?:(?:secret|token|password|api[_-]?key)\s*[:=]\s*)["']?[^"'\s]{12,}
            "#,
        )
        .expect("secret redaction regex compiles")
    })
}

fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding_fixture() -> ResearchFinding {
        ResearchFinding {
            id: Uuid::new_v4(),
            title: "GitHub token in owner/repo".to_string(),
            status: "draft".to_string(),
            source_type: "detection".to_string(),
            source_detection_id: Some(Uuid::new_v4()),
            source_event_id: None,
            program_name: Some("Example Program".to_string()),
            scope_asset: Some("owner/repo".to_string()),
            scope_status: "in_scope".to_string(),
            playbook: Some("secret-leak".to_string()),
            severity: Some("high".to_string()),
            repository: Some("owner/repo".to_string()),
            raw_evidence: json!({"token": "ghp_abcdefghijklmnopqrstuvwxyz123456"}),
            derived_metadata: json!({"impact": "Token could expose private repository data"}),
            notes: Some("Steps: inspect commit. Impact: token leak enables access.".to_string()),
            readiness_score: 0,
            readiness_blockers: json!([]),
            ai_outputs: json!([]),
            export_history: json!([]),
            created_by: "operator".to_string(),
            updated_by: "operator".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn readiness_score_rewards_complete_evidence_and_scope() {
        let score = calculate_readiness_score(&finding_fixture());
        assert_eq!(score.score, 100);
        assert!(score.blockers.is_empty());
    }

    #[test]
    fn readiness_score_reports_missing_submit_blockers() {
        let mut finding = finding_fixture();
        finding.scope_asset = None;
        finding.scope_status = "unknown".to_string();
        finding.notes = Some("needs review".to_string());
        finding.derived_metadata = json!({});

        let score = calculate_readiness_score(&finding);
        assert!(score.score < 80);
        assert!(score
            .blockers
            .iter()
            .any(|blocker| blocker.contains("scope asset")));
        assert!(score
            .blockers
            .iter()
            .any(|blocker| blocker.contains("impact")));
    }

    #[test]
    fn redaction_removes_secret_material_from_text_and_json() {
        let text = "token = ghp_abcdefghijklmnopqrstuvwxyz123456";
        assert!(!redact_text(text).contains("ghp_"));

        let value = redact_value(&json!({
            "secret_value": "super-sensitive-value",
            "nested": {"message": text}
        }));
        assert_eq!(value["secret_value"], "[REDACTED_SECRET]");
        assert!(!value["nested"]["message"]
            .as_str()
            .unwrap()
            .contains("ghp_"));
    }

    #[test]
    fn external_full_evidence_requires_confirmation() {
        assert!(needs_full_evidence_confirmation("openai", true, false));
        assert!(!needs_full_evidence_confirmation("openai", true, true));
        assert!(!needs_full_evidence_confirmation(
            "local-openai",
            true,
            false
        ));
        assert!(!needs_full_evidence_confirmation("openai", false, false));
    }

    #[test]
    fn provider_aliases_are_normalized() {
        assert_eq!(normalize_provider("gpt"), "openai");
        assert_eq!(normalize_provider("Claude"), "anthropic");
        assert_eq!(normalize_provider("local"), "local-openai");
    }

    #[test]
    fn local_ai_base_url_must_be_loopback() {
        assert!(validate_local_ai_base_url("http://127.0.0.1:11434/v1").is_ok());
        assert!(validate_local_ai_base_url("http://localhost:1234/v1").is_ok());
        assert!(validate_local_ai_base_url("http://10.0.0.10:11434/v1").is_err());
        assert!(validate_local_ai_base_url("file:///tmp/model.sock").is_err());
    }

    #[test]
    fn external_provider_rejects_request_level_base_url() {
        let request = ResearchAiAssistRequest {
            provider: Some("openai".to_string()),
            base_url: Some("http://127.0.0.1:1".to_string()),
            model: Some("gpt-test".to_string()),
            prompt: None,
            include_full_evidence: None,
            confirmed_full_evidence: None,
        };
        assert!(reject_request_base_url_for_external_provider(&request).is_err());
    }
}
