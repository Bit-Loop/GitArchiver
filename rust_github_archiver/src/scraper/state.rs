use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
        let mut status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        
        match status.state {
            ScraperState::Running => return Err("Scraper is already running".to_string()),
            _ => {
                status.state = ScraperState::Running;
                status.start_time = Some(Utc::now());
                status.last_updated = Utc::now();
                Ok(())
            }
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        
        status.state = ScraperState::Stopped;
        status.start_time = None;
        status.current_file = None;
        status.last_updated = Utc::now();
        Ok(())
    }

    pub fn pause(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        
        match status.state {
            ScraperState::Running => {
                status.state = ScraperState::Paused;
                status.last_updated = Utc::now();
                Ok(())
            }
            _ => Err("Scraper is not running".to_string())
        }
    }

    pub fn resume(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        
        match status.state {
            ScraperState::Paused => {
                status.state = ScraperState::Running;
                status.last_updated = Utc::now();
                Ok(())
            }
            _ => Err("Scraper is not paused".to_string())
        }
    }

    pub fn restart(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        
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
        let status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok(status.clone())
    }

    pub fn update_progress(&self, events: u64, files: u64, current_file: Option<String>) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        
        status.events_processed = events;
        status.files_processed = files;
        status.current_file = current_file;
        status.last_updated = Utc::now();
        
        // Calculate processing rate (events per second)
        if let Some(start_time) = status.start_time {
            let duration = (Utc::now() - start_time).num_seconds() as f64;
            if duration > 0.0 {
                status.processing_rate = events as f64 / duration;
            }
        }
        
        Ok(())
    }

    pub fn add_error(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| format!("Lock error: {}", e))?;
        status.error_count += 1;
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
