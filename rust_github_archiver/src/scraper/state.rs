use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScraperState {
    Stopped,
    Running,
    Paused,
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ScraperStatus {
    pub state: ScraperState,
    pub last_updated: DateTime<Utc>,
    pub events_processed: u64,
    pub files_processed: u64,
    pub current_file: Option<String>,
    pub processing_rate: f64,
    pub error_count: u64,
    pub start_time: Option<DateTime<Utc>>,
}

impl Default for ScraperStatus {
    fn default() -> Self {
        Self {
            state: ScraperState::Stopped,
            last_updated: Utc::now(),
            events_processed: 0,
            files_processed: 0,
            current_file: None,
            processing_rate: 0.0,
            error_count: 0,
            start_time: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScraperManager {
    status: Arc<Mutex<ScraperStatus>>,
}

impl ScraperManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ScraperStatus::default())),
        }
    }

    pub fn start(&self) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        match status.state {
            ScraperState::Running => Err("Scraper is already running".to_string()),
            _ => {
                status.state = ScraperState::Running;
                status.start_time = Some(Utc::now());
                status.last_updated = Utc::now();
                Ok(())
            }
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        status.state = ScraperState::Stopped;
        status.start_time = None;
        status.current_file = None;
        status.last_updated = Utc::now();
        Ok(())
    }

    pub fn pause(&self) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        match status.state {
            ScraperState::Running => {
                status.state = ScraperState::Paused;
                status.last_updated = Utc::now();
                Ok(())
            }
            ScraperState::Paused => Ok(()),
            ScraperState::Stopped => Err("Scraper is not running".to_string()),
            ScraperState::Error(ref err) => Err(format!("Scraper is in an error state: {}", err)),
        }
    }

    pub fn resume(&self) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        match status.state {
            ScraperState::Paused => {
                status.state = ScraperState::Running;
                status.last_updated = Utc::now();
                Ok(())
            }
            ScraperState::Running => Ok(()),
            ScraperState::Stopped => Err("Scraper is not paused".to_string()),
            ScraperState::Error(ref err) => Err(format!("Scraper is in an error state: {}", err)),
        }
    }

    pub fn restart(&self) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        // Reset counters and restart
        status.state = ScraperState::Running;
        status.start_time = Some(Utc::now());
        status.events_processed = 0;
        status.files_processed = 0;
        status.current_file = None;
        status.processing_rate = 0.0;
        status.error_count = 0;
        status.last_updated = Utc::now();
        Ok(())
    }

    pub fn get_status(&self) -> Result<ScraperStatus, String> {
        let status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(status.clone())
    }

    pub fn update_progress(
        &self,
        events: u64,
        files: u64,
        current_file: Option<String>,
    ) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        status.events_processed = events;
        status.files_processed = files;
        status.current_file = current_file;
        status.last_updated = Utc::now();

        // Calculate processing rate (files per minute, not per second)
        if let Some(start_time) = status.start_time {
            let duration_minutes = (Utc::now() - start_time).num_seconds() as f64 / 60.0;
            if duration_minutes > 0.0 {
                status.processing_rate = files as f64 / duration_minutes;
            }
        }

        Ok(())
    }

    pub fn add_error(&self) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        status.error_count += 1;
        status.last_updated = Utc::now();
        Ok(())
    }

    pub fn enter_error(&self, message: impl Into<String>) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        status.error_count += 1;
        status.state = ScraperState::Error(message.into());
        status.last_updated = Utc::now();
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        if let Ok(status) = self.status.lock() {
            matches!(status.state, ScraperState::Running)
        } else {
            false
        }
    }
}

impl Default for ScraperManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(manager: &ScraperManager) -> ScraperState {
        manager
            .get_status()
            .expect("status lock should be available")
            .state
    }

    #[test]
    fn scraper_fsm_accepts_valid_lifecycle_transitions() {
        let manager = ScraperManager::new();

        assert_eq!(state(&manager), ScraperState::Stopped);
        assert!(!manager.is_running());

        manager.start().expect("stopped -> running");
        assert_eq!(state(&manager), ScraperState::Running);
        assert!(manager.is_running());
        assert!(manager.get_status().expect("status").start_time.is_some());

        manager.pause().expect("running -> paused");
        assert_eq!(state(&manager), ScraperState::Paused);
        assert!(!manager.is_running());

        manager.resume().expect("paused -> running");
        assert_eq!(state(&manager), ScraperState::Running);

        manager.stop().expect("running -> stopped");
        assert_eq!(state(&manager), ScraperState::Stopped);
        assert!(manager.get_status().expect("status").start_time.is_none());
    }

    #[test]
    fn scraper_fsm_rejects_invalid_lifecycle_transitions() {
        let manager = ScraperManager::new();

        assert_eq!(manager.pause().unwrap_err(), "Scraper is not running");
        assert_eq!(manager.resume().unwrap_err(), "Scraper is not paused");

        manager.start().expect("start");
        assert_eq!(manager.start().unwrap_err(), "Scraper is already running");

        manager.enter_error("boom").expect("enter error");
        assert!(manager.pause().unwrap_err().contains("error state"));
        assert!(manager.resume().unwrap_err().contains("error state"));
        assert!(!manager.is_running());
    }

    #[test]
    fn restart_resets_operational_counters_from_any_state() {
        let manager = ScraperManager::new();

        manager.start().expect("start");
        manager
            .update_progress(10, 20, Some("2026-05-09-10.json.gz".to_string()))
            .expect("progress");
        manager.add_error().expect("error");

        manager.restart().expect("restart");
        let status = manager.get_status().expect("status");

        assert_eq!(status.state, ScraperState::Running);
        assert_eq!(status.events_processed, 0);
        assert_eq!(status.files_processed, 0);
        assert_eq!(status.error_count, 0);
        assert!(status.current_file.is_none());
        assert_eq!(status.processing_rate, 0.0);
        assert!(status.start_time.is_some());
    }

    #[test]
    fn progress_and_error_updates_do_not_mutate_lifecycle_state() {
        let manager = ScraperManager::new();

        manager.start().expect("start");
        manager
            .update_progress(7, 3, Some("file.json.gz".to_string()))
            .expect("progress");
        manager.add_error().expect("error");

        let status = manager.get_status().expect("status");
        assert_eq!(status.state, ScraperState::Running);
        assert_eq!(status.events_processed, 7);
        assert_eq!(status.files_processed, 3);
        assert_eq!(status.current_file.as_deref(), Some("file.json.gz"));
        assert_eq!(status.error_count, 1);
    }
}
