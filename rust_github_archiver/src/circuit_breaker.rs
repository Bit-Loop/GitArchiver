/*!
 * Circuit Breaker Pattern Implementation
 *
 * Prevents cascading failures by stopping requests to failing services.
 * States: Closed (normal) -> Open (failing) -> Half-Open (testing) -> Closed
 */

use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Circuit is open, rejecting requests
    HalfOpen, // Testing if service has recovered
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,      // Number of failures before opening
    pub success_threshold: u32,      // Number of successes to close from half-open
    pub timeout: Duration,           // How long to wait before trying half-open
    pub half_open_max_requests: u32, // Max requests to allow in half-open state
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            half_open_max_requests: 3,
        }
    }
}

#[derive(Clone)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    config: CircuitBreakerConfig,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    half_open_requests: Arc<RwLock<u32>>,
    name: String,
}

impl CircuitBreaker {
    pub fn new(name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            config,
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            half_open_requests: Arc::new(RwLock::new(0)),
            name,
        }
    }

    pub fn with_defaults(name: String) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, f: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        // Check if we can make the request
        if !self.can_request().await {
            return Err(anyhow!("Circuit breaker is OPEN for '{}'", self.name));
        }

        // Execute the function
        match f.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(anyhow!("Request failed: {}", e))
            }
        }
    }

    async fn can_request(&self) -> bool {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.config.timeout {
                        info!("Circuit breaker '{}' transitioning to HALF_OPEN", self.name);
                        *state = CircuitState::HalfOpen;
                        *self.half_open_requests.write().await = 0;
                        *self.success_count.write().await = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                let mut requests = self.half_open_requests.write().await;
                if *requests < self.config.half_open_max_requests {
                    *requests += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    async fn on_success(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => {
                // Reset failure count on success
                *self.failure_count.write().await = 0;
            }
            CircuitState::HalfOpen => {
                let mut success_count = self.success_count.write().await;
                *success_count += 1;

                if *success_count >= self.config.success_threshold {
                    info!("Circuit breaker '{}' CLOSED after recovery", self.name);
                    *state = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    *success_count = 0;
                }
            }
            CircuitState::Open => {
                // Should not happen, but handle gracefully
                warn!("Success recorded in OPEN state for '{}'", self.name);
            }
        }
    }

    async fn on_failure(&self) {
        let mut state = self.state.write().await;
        *self.last_failure_time.write().await = Some(Instant::now());

        match *state {
            CircuitState::Closed => {
                let mut failure_count = self.failure_count.write().await;
                *failure_count += 1;

                if *failure_count >= self.config.failure_threshold {
                    warn!(
                        "Circuit breaker '{}' OPENED after {} failures",
                        self.name, *failure_count
                    );
                    *state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                warn!(
                    "Circuit breaker '{}' reopening after failure in HALF_OPEN state",
                    self.name
                );
                *state = CircuitState::Open;
                *self.success_count.write().await = 0;
            }
            CircuitState::Open => {
                // Already open, just update timestamp
            }
        }
    }

    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    pub async fn get_stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            name: self.name.clone(),
            state: self.get_state().await,
            failure_count: *self.failure_count.read().await,
            success_count: *self.success_count.read().await,
            last_failure: self.last_failure_time.read().await.map(|t| t.elapsed()),
        }
    }

    /// Manually reset the circuit breaker to closed state
    pub async fn reset(&self) {
        info!("Manually resetting circuit breaker '{}'", self.name);
        *self.state.write().await = CircuitState::Closed;
        *self.failure_count.write().await = 0;
        *self.success_count.write().await = 0;
        *self.last_failure_time.write().await = None;
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub name: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(1),
            half_open_max_requests: 2,
        };
        let cb = CircuitBreaker::new("test".to_string(), config);

        assert_eq!(cb.get_state().await, CircuitState::Closed);

        for _ in 0..3 {
            let result: Result<()> = cb.call(async { Err(anyhow::anyhow!("error")) }).await;
            assert!(result.is_err());
        }

        assert_eq!(cb.get_state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_rejects_when_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test".to_string(), config);

        for _ in 0..2 {
            let _: Result<()> = cb.call(async { Err(anyhow::anyhow!("error")) }).await;
        }

        assert_eq!(cb.get_state().await, CircuitState::Open);

        let result: Result<String> = cb
            .call(async { Ok::<String, anyhow::Error>("success".to_string()) })
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circuit breaker is OPEN"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            half_open_max_requests: 3,
        };
        let cb = CircuitBreaker::new("test".to_string(), config);

        for _ in 0..2 {
            let _: Result<()> = cb.call(async { Err(anyhow::anyhow!("error")) }).await;
        }
        assert_eq!(cb.get_state().await, CircuitState::Open);

        tokio::time::sleep(Duration::from_millis(150)).await;

        let result: Result<String> = cb
            .call(async { Ok::<String, anyhow::Error>("success".to_string()) })
            .await;
        assert!(result.is_ok());

        let result: Result<String> = cb
            .call(async { Ok::<String, anyhow::Error>("success".to_string()) })
            .await;
        assert!(result.is_ok());
        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test".to_string(), config);

        for _ in 0..2 {
            let _: Result<()> = cb.call(async { Err(anyhow::anyhow!("error")) }).await;
        }
        assert_eq!(cb.get_state().await, CircuitState::Open);

        cb.reset().await;
        assert_eq!(cb.get_state().await, CircuitState::Closed);

        let stats = cb.get_stats().await;
        assert_eq!(stats.failure_count, 0);
    }
}
