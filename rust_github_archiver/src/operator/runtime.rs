use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tracing::{error, warn};

use crate::core::{Config, ResourceMonitor};
use crate::scanning::ScanningService;
use crate::scraper::{ArchiveScraper, ScraperManager, ScraperState};

#[async_trait]
pub trait ScraperRuntimeLauncher: Send + Sync {
    async fn run(
        &self,
        config: Config,
        scraper_manager: Arc<ScraperManager>,
        resource_monitor: Arc<tokio::sync::Mutex<ResourceMonitor>>,
    ) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct ArchiveScraperRuntimeLauncher;

#[async_trait]
impl ScraperRuntimeLauncher for ArchiveScraperRuntimeLauncher {
    async fn run(
        &self,
        config: Config,
        scraper_manager: Arc<ScraperManager>,
        resource_monitor: Arc<tokio::sync::Mutex<ResourceMonitor>>,
    ) -> Result<()> {
        let scraper =
            ArchiveScraper::with_resource_monitor(config, scraper_manager, resource_monitor)?;
        scraper.initialize().await?;
        scraper.run_continuous_scraping().await
    }
}

#[derive(Clone)]
pub struct ScraperRuntimeService {
    config: Config,
    scraper_manager: Arc<ScraperManager>,
    resource_monitor: Arc<tokio::sync::Mutex<ResourceMonitor>>,
    scanning_service: Arc<ScanningService>,
    launcher: Arc<dyn ScraperRuntimeLauncher>,
    runtime_task: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
}

impl ScraperRuntimeService {
    pub fn new(
        config: Config,
        scraper_manager: Arc<ScraperManager>,
        resource_monitor: Arc<tokio::sync::Mutex<ResourceMonitor>>,
        scanning_service: Arc<ScanningService>,
    ) -> Self {
        Self::with_launcher(
            config,
            scraper_manager,
            resource_monitor,
            scanning_service,
            Arc::new(ArchiveScraperRuntimeLauncher),
        )
    }

    pub fn with_launcher(
        config: Config,
        scraper_manager: Arc<ScraperManager>,
        resource_monitor: Arc<tokio::sync::Mutex<ResourceMonitor>>,
        scanning_service: Arc<ScanningService>,
        launcher: Arc<dyn ScraperRuntimeLauncher>,
    ) -> Self {
        Self {
            config,
            scraper_manager,
            resource_monitor,
            scanning_service,
            launcher,
            runtime_task: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        self.scanning_service.resume_execution().await;
        self.activate_runtime_state(RuntimeAction::Start)?;
        self.ensure_runtime_task().await
    }

    pub async fn pause(&self) -> Result<()> {
        self.scanning_service.pause_execution().await;
        self.scraper_manager.pause().map_err(anyhow::Error::msg)?;
        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        self.scanning_service.resume_execution().await;
        self.activate_runtime_state(RuntimeAction::Resume)?;
        self.ensure_runtime_task().await
    }

    pub async fn restart(&self) -> Result<()> {
        self.stop().await?;
        self.scanning_service.resume_execution().await;
        self.scraper_manager.restart().map_err(anyhow::Error::msg)?;
        self.ensure_runtime_task().await
    }

    pub async fn stop(&self) -> Result<()> {
        self.scanning_service.request_shutdown().await;
        self.scraper_manager.stop().map_err(anyhow::Error::msg)?;
        if let Err(error) = self
            .scanning_service
            .wait_for_active_scans(Duration::from_secs(30))
            .await
        {
            warn!("Timed out waiting for scans to drain before runtime stop: {error}");
        }
        self.wait_for_runtime_exit(Duration::from_secs(30)).await
    }

    async fn ensure_runtime_task(&self) -> Result<()> {
        self.reap_finished_task().await;

        let mut task_guard = self.runtime_task.lock().await;
        if task_guard.as_ref().is_some_and(|task| !task.is_finished()) {
            return Ok(());
        }

        let config = self.config.clone();
        let scraper_manager = self.scraper_manager.clone();
        let resource_monitor = self.resource_monitor.clone();
        let launcher = self.launcher.clone();

        *task_guard = Some(tokio::spawn(async move {
            if let Err(error) = launcher
                .run(config, scraper_manager.clone(), resource_monitor)
                .await
            {
                error!("Scraper runtime task exited with error: {error}");
                let _ = scraper_manager.stop();
            }
        }));

        Ok(())
    }

    async fn reap_finished_task(&self) {
        let finished_task = {
            let mut task_guard = self.runtime_task.lock().await;
            if task_guard.as_ref().is_some_and(|task| task.is_finished()) {
                task_guard.take()
            } else {
                None
            }
        };

        if let Some(task) = finished_task {
            if let Err(error) = task.await {
                warn!("Scraper runtime task finished unsuccessfully: {error}");
            }
        }
    }

    async fn wait_for_runtime_exit(&self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;

        loop {
            let finished = {
                let task_guard = self.runtime_task.lock().await;
                task_guard
                    .as_ref()
                    .map(|task| task.is_finished())
                    .unwrap_or(true)
            };

            if finished || Instant::now() >= deadline {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let task = {
            let mut task_guard = self.runtime_task.lock().await;
            task_guard.take()
        };

        if let Some(task) = task {
            if task.is_finished() {
                task.await
                    .map_err(|error| anyhow!("Scraper runtime join failed: {error}"))?;
            } else {
                warn!("Scraper runtime did not stop in time; aborting task");
                task.abort();
                let _ = task.await;
            }
        }

        Ok(())
    }

    fn activate_runtime_state(&self, action: RuntimeAction) -> Result<()> {
        let status = self
            .scraper_manager
            .get_status()
            .map_err(anyhow::Error::msg)?;

        match action {
            RuntimeAction::Start => match status.state {
                ScraperState::Paused => self.scraper_manager.resume(),
                ScraperState::Running => Ok(()),
                _ => self.scraper_manager.start(),
            },
            RuntimeAction::Resume => match status.state {
                ScraperState::Paused => self.scraper_manager.resume(),
                ScraperState::Running => Ok(()),
                ScraperState::Stopped => self.scraper_manager.start(),
                ScraperState::Error(_) => self.scraper_manager.restart(),
            },
        }
        .map_err(anyhow::Error::msg)
    }
}

enum RuntimeAction {
    Start,
    Resume,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::core::ResourceLimits;
    use crate::secrets::{SecretCategory, SecretMatch, SecretSeverity};

    #[derive(Debug, Default)]
    struct TestLauncher;

    #[async_trait]
    impl ScraperRuntimeLauncher for TestLauncher {
        async fn run(
            &self,
            _config: Config,
            scraper_manager: Arc<ScraperManager>,
            _resource_monitor: Arc<tokio::sync::Mutex<ResourceMonitor>>,
        ) -> Result<()> {
            loop {
                let status = scraper_manager.get_status().map_err(anyhow::Error::msg)?;
                match status.state {
                    ScraperState::Stopped => return Ok(()),
                    ScraperState::Running | ScraperState::Paused | ScraperState::Error(_) => {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn runtime_service_starts_and_stops_without_compatibility_flag() {
        let config = Config::default();
        let scraper_manager = Arc::new(ScraperManager::new());
        let scanning_service = Arc::new(ScanningService::new(1));
        let resource_monitor = Arc::new(tokio::sync::Mutex::new(ResourceMonitor::new(
            (&config.resources).into(),
        )));

        let runtime = ScraperRuntimeService::with_launcher(
            config,
            scraper_manager.clone(),
            resource_monitor,
            scanning_service,
            Arc::new(TestLauncher),
        );

        runtime.start().await.expect("runtime should start");
        assert!(scraper_manager.is_running());

        runtime.pause().await.expect("runtime should pause");
        assert_eq!(
            scraper_manager
                .get_status()
                .expect("status should be available")
                .state,
            ScraperState::Paused
        );

        runtime.resume().await.expect("runtime should resume");
        assert!(scraper_manager.is_running());

        runtime.stop().await.expect("runtime should stop");
        assert_eq!(
            scraper_manager
                .get_status()
                .expect("status should be available")
                .state,
            ScraperState::Stopped
        );
    }

    #[tokio::test]
    async fn operator_workflow_smoke_covers_login_runtime_and_findings_review() {
        let config = Config::default();
        let scraper_manager = Arc::new(ScraperManager::new());
        let scanning_service = Arc::new(ScanningService::new(1));
        let resource_monitor = Arc::new(tokio::sync::Mutex::new(ResourceMonitor::new(
            ResourceLimits::from(&config.resources),
        )));

        let runtime = ScraperRuntimeService::with_launcher(
            config,
            scraper_manager.clone(),
            resource_monitor,
            scanning_service.clone(),
            Arc::new(TestLauncher),
        );

        let admin_password = "RootSeed123!";
        let user_manager =
            crate::auth::UserManager::from_admin_password(admin_password).expect("manager");
        let admin = user_manager
            .authenticate("admin", admin_password)
            .await
            .expect("configured admin login should work");
        assert_eq!(admin.canonical_role(), "admin");

        runtime.start().await.expect("runtime should start");
        assert!(scraper_manager.is_running());

        let queue_metrics = scanning_service.get_active_scan_metrics().await;
        assert_eq!(queue_metrics.active_scans, 0);

        scanning_service
            .record_realtime_detection(
                "owner/repo",
                vec![SecretMatch {
                    detector_name: "Smoke Detector".to_string(),
                    matched_text: "ghp_REDACTED_EXAMPLE".to_string(),
                    start_position: 0,
                    end_position: 10,
                    line_number: Some(3),
                    filename: Some(".env".to_string()),
                    entropy: 5.1,
                    severity: SecretSeverity::High,
                    category: SecretCategory::ApiKey,
                    context: "Event 7 @ smoke".to_string(),
                    verified: true,
                    hash: "smoke-hash".to_string(),
                }],
                Utc::now(),
                "operator-smoke",
            )
            .await
            .expect("smoke detection should be recorded");

        let findings = scanning_service
            .get_scan_results(crate::scanning::ScanFilter {
                repository: Some("owner/repo".to_string()),
                severity: None,
                category: None,
                detector: None,
                verified_only: None,
                date_from: None,
                date_to: None,
                limit: Some(10),
                offset: Some(0),
            })
            .await;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].repository, "owner/repo");

        runtime.stop().await.expect("runtime should stop");
    }
}
