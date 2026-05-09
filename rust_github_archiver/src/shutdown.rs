/*!
 * Graceful Shutdown Handler
 *
 * Handles graceful shutdown of the application, ensuring:
 * - In-flight requests complete
 * - Database connections close properly
 * - Background tasks finish
 * - Resources are cleaned up
 */

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{info, warn};

#[derive(Clone)]
pub struct ShutdownCoordinator {
    shutdown_requested: Arc<RwLock<bool>>,
    active_tasks: Arc<RwLock<usize>>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            shutdown_requested: Arc::new(RwLock::new(false)),
            active_tasks: Arc::new(RwLock::new(0)),
        }
    }

    /// Request shutdown
    pub async fn request_shutdown(&self) {
        info!("Shutdown requested");
        *self.shutdown_requested.write().await = true;
    }

    /// Check if shutdown has been requested
    pub async fn is_shutdown_requested(&self) -> bool {
        *self.shutdown_requested.read().await
    }

    /// Register an active task
    pub async fn register_task(&self) {
        *self.active_tasks.write().await += 1;
    }

    /// Unregister an active task
    pub async fn unregister_task(&self) {
        *self.active_tasks.write().await -= 1;
    }

    /// Get number of active tasks
    pub async fn active_tasks(&self) -> usize {
        *self.active_tasks.read().await
    }

    /// Wait for all active tasks to complete with timeout
    pub async fn wait_for_tasks(&self, max_wait: Duration) -> Result<(), String> {
        info!(
            "Waiting for {} active tasks to complete",
            self.active_tasks().await
        );

        match timeout(max_wait, async {
            loop {
                let active = self.active_tasks().await;
                if active == 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        {
            Ok(_) => {
                info!("All tasks completed successfully");
                Ok(())
            }
            Err(_) => {
                let remaining = self.active_tasks().await;
                warn!(
                    "Timeout waiting for tasks. {} tasks still active",
                    remaining
                );
                Err(format!("{} tasks did not complete in time", remaining))
            }
        }
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Graceful shutdown signal handler
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received SIGTERM signal");
        },
    }
}

/// Perform graceful shutdown
pub async fn perform_shutdown(
    coordinator: Arc<ShutdownCoordinator>,
    pool: Option<sqlx::PgPool>,
) -> Result<(), String> {
    info!("🛑 Starting graceful shutdown...");

    // Step 1: Request shutdown (stops new tasks from starting)
    coordinator.request_shutdown().await;
    info!("✓ Shutdown requested");

    // Step 2: Wait for active tasks to complete (max 30 seconds)
    match coordinator.wait_for_tasks(Duration::from_secs(30)).await {
        Ok(_) => info!("✓ All tasks completed"),
        Err(e) => warn!("⚠ Task completion warning: {}", e),
    }

    // Step 3: Close database connections
    if let Some(pool) = pool {
        info!("Closing database connections...");
        pool.close().await;
        info!("✓ Database connections closed");
    }

    info!("✓ Graceful shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_coordinator() {
        let coordinator = ShutdownCoordinator::new();

        assert!(!coordinator.is_shutdown_requested().await);
        assert_eq!(coordinator.active_tasks().await, 0);

        coordinator.register_task().await;
        assert_eq!(coordinator.active_tasks().await, 1);

        coordinator.register_task().await;
        assert_eq!(coordinator.active_tasks().await, 2);

        coordinator.unregister_task().await;
        assert_eq!(coordinator.active_tasks().await, 1);

        coordinator.request_shutdown().await;
        assert!(coordinator.is_shutdown_requested().await);
    }

    #[tokio::test]
    async fn test_wait_for_tasks_completes() {
        let coordinator = ShutdownCoordinator::new();

        coordinator.register_task().await;

        // Spawn task that unregisters after 100ms
        let coord_clone = coordinator.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            coord_clone.unregister_task().await;
        });

        let result = coordinator.wait_for_tasks(Duration::from_secs(1)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wait_for_tasks_timeout() {
        let coordinator = ShutdownCoordinator::new();

        // Register a task that never completes
        coordinator.register_task().await;

        let result = coordinator.wait_for_tasks(Duration::from_millis(100)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("did not complete"));
    }
}
