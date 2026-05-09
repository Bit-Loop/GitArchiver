use crate::core::config::ResourceConfig;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

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
        ResourceConfig::default().into()
    }
}

impl From<&ResourceConfig> for ResourceLimits {
    fn from(config: &ResourceConfig) -> Self {
        Self {
            memory_limit_gb: config.memory_limit_gb,
            disk_limit_gb: config.disk_limit_gb,
            cpu_limit_percent: config.cpu_limit_percent,
            memory_warning_threshold: config.memory_warning_threshold,
            disk_warning_threshold: config.disk_warning_threshold,
            cpu_warning_threshold: config.cpu_warning_threshold,
            emergency_cleanup_threshold: config.emergency_cleanup_threshold,
            monitoring_interval_seconds: config.monitoring_interval_seconds,
        }
    }
}

impl From<ResourceConfig> for ResourceLimits {
    fn from(config: ResourceConfig) -> Self {
        Self::from(&config)
    }
}

pub struct ResourceMonitor {
    limits: ResourceLimits,
    emergency_mode: bool,
    last_cpu_measurement: Option<(Instant, f64)>,
}

impl ResourceMonitor {
    pub fn new(limits: ResourceLimits) -> Self {
        tracing::info!(
            "Resource monitor initialized: memory_limit={:.1} GB, disk_limit={:.1} GB, cpu_limit={:.0}%",
            limits.memory_limit_gb, limits.disk_limit_gb, limits.cpu_limit_percent
        );

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

        let mut emergency_conditions = Vec::with_capacity(3);

        if memory_status.percent > (self.limits.emergency_cleanup_threshold * 100.0) {
            emergency_conditions.push("memory".to_string());
        }
        if disk_status.percent > (self.limits.emergency_cleanup_threshold * 100.0) {
            emergency_conditions.push("disk".to_string());
        }
        if cpu_status.percent
            > (self.limits.cpu_limit_percent * self.limits.emergency_cleanup_threshold)
        {
            emergency_conditions.push("cpu".to_string());
        }

        self.emergency_mode = !emergency_conditions.is_empty();

        if self.emergency_mode {
            tracing::warn!(
                "Emergency resource thresholds exceeded: memory={:.1}% ({:.2}/{:.2} GB), disk={:.1}% ({:.2}/{:.2} GB), cpu={:.1}% (limit {}%)",
                memory_status.percent,
                memory_status.used_gb,
                memory_status.limit_gb,
                disk_status.percent,
                disk_status.used_gb,
                disk_status.limit_gb,
                cpu_status.percent,
                self.limits.cpu_limit_percent
            );
        }

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
        let used_kb =
            memory_info.total - memory_info.free - memory_info.cached - memory_info.buffers;
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
        let cpu_percent = if let Some((last_time, last_percent)) = self.last_cpu_measurement {
            if last_time.elapsed() < Duration::from_secs(1) {
                // Return cached value if measured recently
                last_percent
            } else {
                self.measure_cpu_usage().await?
            }
        } else {
            self.measure_cpu_usage().await?
        };

        let warning =
            cpu_percent > (self.limits.cpu_limit_percent * self.limits.cpu_warning_threshold);

        Ok(CpuStatus {
            percent: (cpu_percent * 10.0).round() / 10.0,
            limit_percent: self.limits.cpu_limit_percent,
            warning,
        })
    }

    async fn measure_cpu_usage(&mut self) -> Result<f64> {
        let cpu_count = num_cpus::get().max(1) as f64;
        let load_average = sys_info::loadavg()?.one as f64;
        let cpu_percent = ((load_average / cpu_count) * 100.0).clamp(0.0, 100.0);

        self.last_cpu_measurement = Some((Instant::now(), cpu_percent));

        Ok(cpu_percent)
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
        match self.clear_caches().await {
            Ok(cache_actions) => cleanup_actions.extend(cache_actions),
            Err(error) => cleanup_actions.push(format!("Cache cleanup failed: {}", error)),
        }

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
                                if extension == "log" && std::fs::remove_file(entry.path()).is_ok()
                                {
                                    count += 1;
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

    async fn clear_caches(&self) -> Result<Vec<String>> {
        let cache_manager = crate::scanning::cache::CacheManager::global();
        let retained_removed = cache_manager.enforce_retention().await?;
        let cleared_entries = cache_manager.clear_all().await?;

        let mut actions = Vec::new();
        actions.push(format!(
            "Pruned {} expired repository cache entries",
            retained_removed
        ));
        actions.push(format!(
            "Cleared {} repository cache entries",
            cleared_entries
        ));

        Ok(actions)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_limits_from_config_preserve_threshold_contract() {
        let config = ResourceConfig {
            memory_limit_gb: 12.0,
            disk_limit_gb: 50.0,
            cpu_limit_percent: 75.0,
            memory_warning_threshold: 0.7,
            disk_warning_threshold: 0.8,
            cpu_warning_threshold: 0.6,
            emergency_cleanup_threshold: 0.95,
            monitoring_interval_seconds: 15,
        };

        let limits = ResourceLimits::from(&config);

        assert_eq!(limits.memory_limit_gb, 12.0);
        assert_eq!(limits.disk_limit_gb, 50.0);
        assert_eq!(limits.cpu_limit_percent, 75.0);
        assert_eq!(limits.memory_warning_threshold, 0.7);
        assert_eq!(limits.disk_warning_threshold, 0.8);
        assert_eq!(limits.cpu_warning_threshold, 0.6);
        assert_eq!(limits.emergency_cleanup_threshold, 0.95);
        assert_eq!(limits.monitoring_interval_seconds, 15);
    }

    #[test]
    fn resource_monitor_starts_outside_emergency_mode() {
        let monitor = ResourceMonitor::new(ResourceLimits {
            memory_limit_gb: 1.0,
            disk_limit_gb: 1.0,
            cpu_limit_percent: 1.0,
            memory_warning_threshold: 0.5,
            disk_warning_threshold: 0.5,
            cpu_warning_threshold: 0.5,
            emergency_cleanup_threshold: 0.9,
            monitoring_interval_seconds: 1,
        });

        assert!(!monitor.is_emergency_mode());
    }

    #[tokio::test]
    async fn cpu_measurement_is_bounded_and_cached() {
        let mut monitor = ResourceMonitor::new(ResourceLimits::default());

        let first = monitor
            .measure_cpu_usage()
            .await
            .expect("measure cpu usage");
        let status = monitor.get_cpu_status().await.expect("cached cpu status");

        assert!((0.0..=100.0).contains(&first));
        assert_eq!(status.percent, (first * 10.0).round() / 10.0);
    }
}
