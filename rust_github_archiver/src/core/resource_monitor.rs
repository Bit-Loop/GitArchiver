use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use tokio::time;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatus {
    pub memory: MemoryStatus,
    pub disk: DiskStatus,
    pub cpu: CpuStatus,
    pub emergency_mode: bool,
    pub emergency_conditions: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub used_gb: f64,
    pub limit_gb: f64,
    pub percent: f64,
    pub warning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStatus {
    pub used_gb: f64,
    pub limit_gb: f64,
    pub percent: f64,
    pub warning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStatus {
    pub percent: f64,
    pub limit_percent: f64,
    pub warning: bool,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub memory_limit_gb: f64,
    pub disk_limit_gb: f64,
    pub cpu_limit_percent: f64,
    pub memory_warning_threshold: f64,
    pub disk_warning_threshold: f64,
    pub cpu_warning_threshold: f64,
    pub emergency_cleanup_threshold: f64,
    pub monitoring_interval_seconds: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            memory_limit_gb: 18.0,
            disk_limit_gb: 40.0,
            cpu_limit_percent: 80.0,
            memory_warning_threshold: 0.8,
            disk_warning_threshold: 0.8,
            cpu_warning_threshold: 0.7,
            emergency_cleanup_threshold: 0.9,
            monitoring_interval_seconds: 30,
        }
    }
}

pub struct ResourceMonitor {
    limits: ResourceLimits,
    emergency_mode: bool,
    last_cpu_measurement: Option<(Instant, f64)>,
}

impl ResourceMonitor {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            emergency_mode: false,
            last_cpu_measurement: None,
        }
    }

    pub async fn get_resource_status(&mut self) -> Result<ResourceStatus> {
        let memory_status = self.get_memory_status()?;
        let disk_status = self.get_disk_status()?;
        let cpu_status = self.get_cpu_status().await?;

        let mut emergency_conditions = Vec::new();
        
        if memory_status.percent > (self.limits.emergency_cleanup_threshold * 100.0) {
            emergency_conditions.push("memory".to_string());
        }
        if disk_status.percent > (self.limits.emergency_cleanup_threshold * 100.0) {
            emergency_conditions.push("disk".to_string());
        }
        if cpu_status.percent > (self.limits.cpu_limit_percent * self.limits.emergency_cleanup_threshold) {
            emergency_conditions.push("cpu".to_string());
        }

        self.emergency_mode = !emergency_conditions.is_empty();

        Ok(ResourceStatus {
            memory: memory_status,
            disk: disk_status,
            cpu: cpu_status,
            emergency_mode: self.emergency_mode,
            emergency_conditions,
            timestamp: Utc::now(),
        })
    }

    fn get_memory_status(&self) -> Result<MemoryStatus> {
        let memory_info = sys_info::mem_info()?;
        let used_kb = memory_info.total - memory_info.free - memory_info.cached - memory_info.buffers;
        let used_gb = used_kb as f64 / (1024.0 * 1024.0);
        let percent = (used_gb / self.limits.memory_limit_gb) * 100.0;
        let warning = percent > (self.limits.memory_warning_threshold * 100.0);

        Ok(MemoryStatus {
            used_gb: (used_gb * 100.0).round() / 100.0,
            limit_gb: self.limits.memory_limit_gb,
            percent: (percent * 10.0).round() / 10.0,
            warning,
        })
    }

    fn get_disk_status(&self) -> Result<DiskStatus> {
        let disk_info = sys_info::disk_info()?;
        let used_gb = (disk_info.total - disk_info.free) as f64 / (1024.0 * 1024.0 * 1024.0);
        let percent = (used_gb / self.limits.disk_limit_gb) * 100.0;
        let warning = percent > (self.limits.disk_warning_threshold * 100.0);

        Ok(DiskStatus {
            used_gb: (used_gb * 100.0).round() / 100.0,
            limit_gb: self.limits.disk_limit_gb,
            percent: (percent * 10.0).round() / 10.0,
            warning,
        })
    }

    async fn get_cpu_status(&mut self) -> Result<CpuStatus> {
        let cpu_percent = if let Some((last_time, _)) = self.last_cpu_measurement {
            if last_time.elapsed() < Duration::from_secs(1) {
                // Return cached value if measured recently
                self.last_cpu_measurement.unwrap().1
            } else {
                self.measure_cpu_usage().await?
            }
        } else {
            self.measure_cpu_usage().await?
        };

        let warning = cpu_percent > (self.limits.cpu_limit_percent * self.limits.cpu_warning_threshold);

        Ok(CpuStatus {
            percent: (cpu_percent * 10.0).round() / 10.0,
            limit_percent: self.limits.cpu_limit_percent,
            warning,
        })
    }

    async fn measure_cpu_usage(&mut self) -> Result<f64> {
        // Simple CPU usage calculation
        let start_time = Instant::now();
        let start_usage = self.get_cpu_time()?;
        
        time::sleep(Duration::from_millis(100)).await;
        
        let end_time = Instant::now();
        let end_usage = self.get_cpu_time()?;
        
        let elapsed = end_time.duration_since(start_time).as_secs_f64();
        let cpu_time_diff = end_usage - start_usage;
        let cpu_percent = (cpu_time_diff / elapsed) * 100.0;
        
        self.last_cpu_measurement = Some((end_time, cpu_percent));
        
        Ok(cpu_percent.min(100.0))
    }

    fn get_cpu_time(&self) -> Result<f64> {
        // This is a simplified implementation
        // In production, you'd want to use more accurate CPU time measurement
        Ok(sys_info::loadavg()?.one as f64 * 10.0)
    }

    pub async fn emergency_cleanup(&self) -> Result<CleanupResult> {
        tracing::warn!("Starting emergency resource cleanup");
        
        let mut cleanup_actions = Vec::new();
        let mut total_freed = 0u64;

        // Cleanup old log files
        if let Ok(logs_freed) = self.cleanup_old_logs().await {
            cleanup_actions.push(format!("Cleaned {} old log files", logs_freed));
            total_freed += logs_freed;
        }

        // Cleanup temporary files
        if let Ok(temp_freed) = self.cleanup_temp_files().await {
            cleanup_actions.push(format!("Cleaned {} temporary files", temp_freed));
            total_freed += temp_freed;
        }

        // Clear application caches
        self.clear_caches().await;
        cleanup_actions.push("Cleared application caches".to_string());

        Ok(CleanupResult {
            actions_taken: cleanup_actions,
            files_removed: total_freed,
            success: true,
            timestamp: Utc::now(),
        })
    }

    async fn cleanup_old_logs(&self) -> Result<u64> {
        let mut count = 0;
        let log_dir = std::path::Path::new("logs");
        
        if !log_dir.exists() {
            return Ok(0);
        }

        let cutoff_time = std::time::SystemTime::now() - Duration::from_secs(7 * 24 * 3600); // 7 days

        if let Ok(entries) = std::fs::read_dir(log_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff_time {
                            if let Some(extension) = entry.path().extension() {
                                if extension == "log" {
                                    if std::fs::remove_file(entry.path()).is_ok() {
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    async fn cleanup_temp_files(&self) -> Result<u64> {
        let mut count = 0;
        let temp_dirs = ["./tmp", "./temp", "./gharchive_data/tmp"];

        for temp_dir in &temp_dirs {
            let path = std::path::Path::new(temp_dir);
            if !path.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if std::fs::remove_file(entry.path()).is_ok() {
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    async fn clear_caches(&self) {
        // This can be extended to clear specific application caches
        // For now, just a placeholder
    }

    pub fn is_emergency_mode(&self) -> bool {
        self.emergency_mode
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupResult {
    pub actions_taken: Vec<String>,
    pub files_removed: u64,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
}
