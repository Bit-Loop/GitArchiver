use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::api::state::AppState;
use crate::core::DatabaseHealth;
use crate::scraper::{MainScraperStatus, ScraperState};

#[derive(Debug, Serialize)]
pub struct SystemStatusResponse {
    pub status: String,
    pub timestamp: String,
    pub hostname: String,
    pub platform: String,
    pub load_average: f64,
    pub scraper: ScraperStatusResponse,
    pub database: DatabaseHealth,
    pub ready: bool,
}

#[derive(Debug, Serialize)]
pub struct ScraperStatusResponse {
    pub status: String,
    pub events_processed: u64,
    pub files_processed: u64,
    pub current_file: Option<String>,
    pub processing_rate: f64,
    pub error_count: u64,
    pub is_running: bool,
    pub last_updated: String,
    pub uptime_seconds: Option<i64>,
    pub database_connected: bool,
    pub ready: bool,
}

pub async fn build_system_status(app_state: &AppState) -> Result<SystemStatusResponse> {
    let comprehensive_status = app_state.get_comprehensive_status().await?;
    let scraper_status = build_scraper_status(app_state, &comprehensive_status)?;
    let database = comprehensive_status
        .database_health
        .clone()
        .unwrap_or(DatabaseHealth {
            is_connected: false,
            connection_count: 0,
            active_queries: 0,
            cache_hit_ratio: 0.0,
            error_message: Some("Database health unavailable".to_string()),
        });

    let ready = scraper_status.ready && database.is_connected;
    let status = if !database.is_connected || scraper_status.status == "error" {
        "degraded"
    } else if scraper_status.status == "paused" {
        "paused"
    } else if scraper_status.status == "running" {
        "running"
    } else {
        "idle"
    };

    Ok(SystemStatusResponse {
        status: status.to_string(),
        timestamp: Utc::now().to_rfc3339(),
        hostname: sys_info::hostname().unwrap_or_else(|_| "unknown".to_string()),
        platform: format!(
            "{} {}",
            sys_info::os_type().unwrap_or_else(|_| "unknown".to_string()),
            sys_info::os_release().unwrap_or_else(|_| "unknown".to_string())
        ),
        load_average: sys_info::loadavg().map(|la| la.one).unwrap_or(0.0),
        scraper: scraper_status,
        database,
        ready,
    })
}

pub fn build_scraper_status(
    app_state: &AppState,
    comprehensive_status: &MainScraperStatus,
) -> Result<ScraperStatusResponse> {
    let manager_status = app_state
        .scraper_manager
        .get_status()
        .map_err(anyhow::Error::msg)?;

    let status = scraper_state_label(&manager_status.state);

    let database_connected = comprehensive_status
        .database_health
        .as_ref()
        .map(|health| health.is_connected)
        .unwrap_or(false);
    let ready = scraper_ready(&manager_status.state, database_connected);

    Ok(ScraperStatusResponse {
        status: status.to_string(),
        events_processed: manager_status.events_processed,
        files_processed: manager_status.files_processed,
        current_file: manager_status.current_file,
        processing_rate: manager_status.processing_rate,
        error_count: manager_status.error_count,
        is_running: app_state.scraper_manager.is_running(),
        last_updated: manager_status.last_updated.to_rfc3339(),
        uptime_seconds: manager_status
            .start_time
            .map(|started_at| (Utc::now() - started_at).num_seconds().max(0)),
        database_connected,
        ready,
    })
}

fn scraper_state_label(state: &ScraperState) -> &'static str {
    match state {
        ScraperState::Running => "running",
        ScraperState::Stopped => "stopped",
        ScraperState::Paused => "paused",
        ScraperState::Error(_) => "error",
    }
}

fn scraper_ready(state: &ScraperState, database_connected: bool) -> bool {
    matches!(state, ScraperState::Running | ScraperState::Paused) && database_connected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scraper_state_labels_are_stable_api_values() {
        assert_eq!(scraper_state_label(&ScraperState::Stopped), "stopped");
        assert_eq!(scraper_state_label(&ScraperState::Running), "running");
        assert_eq!(scraper_state_label(&ScraperState::Paused), "paused");
        assert_eq!(
            scraper_state_label(&ScraperState::Error("boom".to_string())),
            "error"
        );
    }

    #[test]
    fn scraper_readiness_requires_running_or_paused_state_and_database() {
        assert!(scraper_ready(&ScraperState::Running, true));
        assert!(scraper_ready(&ScraperState::Paused, true));
        assert!(!scraper_ready(&ScraperState::Stopped, true));
        assert!(!scraper_ready(
            &ScraperState::Error("boom".to_string()),
            true
        ));
        assert!(!scraper_ready(&ScraperState::Running, false));
    }
}
