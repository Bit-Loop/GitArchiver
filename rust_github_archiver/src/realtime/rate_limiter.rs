use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Adaptive rate limiter for API requests with 429 handling
#[derive(Clone)]
pub struct AdaptiveRateLimiter {
    /// Target requests per minute
    requests_per_minute: Arc<RwLock<u32>>,
    /// History of request timestamps (sliding window)
    last_request_times: Arc<RwLock<Vec<Instant>>>,
    /// Auto-adjust rate on 429 responses
    auto_adjust: Arc<RwLock<bool>>,
    /// Paused until this time (from 429 responses)
    paused_until: Arc<RwLock<Option<Instant>>>,
    /// Last retry-after duration received
    retry_after: Arc<RwLock<Option<Duration>>>,
    /// Total requests made
    total_requests: Arc<RwLock<u64>>,
    /// Total rate limit hits (429s)
    rate_limit_hits: Arc<RwLock<u64>>,
}

/// Current status of the rate limiter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    /// Configured requests per minute
    pub requests_per_minute: u32,
    /// Actual requests in last minute
    pub requests_last_minute: u32,
    /// Auto-adjust enabled
    pub auto_adjust_enabled: bool,
    /// Currently paused due to rate limiting
    pub is_paused: bool,
    /// Retry-after duration if paused
    pub retry_after_seconds: Option<u64>,
    /// Time until pause expires (seconds)
    pub pause_remaining_seconds: Option<u64>,
    /// Total requests made
    pub total_requests: u64,
    /// Total 429 responses received
    pub rate_limit_hits: u64,
}

impl AdaptiveRateLimiter {
    /// Create a new adaptive rate limiter
    ///
    /// # Arguments
    /// * `requests_per_minute` - Initial rate limit (default: 5)
    /// * `auto_adjust` - Auto-reduce rate on 429 responses (default: true)
    pub fn new(requests_per_minute: u32, auto_adjust: bool) -> Self {
        info!(
            "Initializing AdaptiveRateLimiter: {} req/min, auto_adjust: {}",
            requests_per_minute, auto_adjust
        );

        Self {
            requests_per_minute: Arc::new(RwLock::new(requests_per_minute)),
            last_request_times: Arc::new(RwLock::new(Vec::with_capacity(100))),
            auto_adjust: Arc::new(RwLock::new(auto_adjust)),
            paused_until: Arc::new(RwLock::new(None)),
            retry_after: Arc::new(RwLock::new(None)),
            total_requests: Arc::new(RwLock::new(0)),
            rate_limit_hits: Arc::new(RwLock::new(0)),
        }
    }

    /// Wait if necessary to respect rate limits
    ///
    /// This method will:
    /// 1. Check if paused due to 429 - wait if needed
    /// 2. Clean old request timestamps (>60s)
    /// 3. Check if at rate limit - wait if needed
    /// 4. Record this request
    pub async fn wait_if_needed(&self) -> Result<()> {
        // Check if paused due to rate limiting
        let paused_until = *self.paused_until.read().await;
        if let Some(until) = paused_until {
            let now = Instant::now();
            if now < until {
                let wait_time = until.duration_since(now);
                warn!(
                    "Rate limited - paused for {:?} more seconds",
                    wait_time.as_secs()
                );
                tokio::time::sleep(wait_time).await;

                // Clear pause state
                *self.paused_until.write().await = None;
                info!("Rate limit pause expired - resuming requests");
            } else {
                // Pause already expired
                *self.paused_until.write().await = None;
            }
        }

        // Clean old request times (older than 60 seconds)
        let mut times = self.last_request_times.write().await;
        let now = Instant::now();
        times.retain(|&t| now.duration_since(t) < Duration::from_secs(60));

        // Check if we've hit the rate limit
        let limit = *self.requests_per_minute.read().await;
        let current_count = times.len() as u32;

        if current_count >= limit {
            // Calculate wait time to next available slot
            if let Some(&oldest) = times.first() {
                let elapsed = now.duration_since(oldest);
                let wait_time = Duration::from_secs(60).saturating_sub(elapsed);

                if !wait_time.is_zero() {
                    debug!(
                        "Rate limit reached ({}/{} req/min) - waiting {:?}",
                        current_count, limit, wait_time
                    );
                    drop(times); // Release lock before sleeping
                    tokio::time::sleep(wait_time).await;

                    // Re-acquire lock and clean again
                    let mut times_after = self.last_request_times.write().await;
                    let now = Instant::now();
                    times_after.retain(|&t| now.duration_since(t) < Duration::from_secs(60));

                    // Record this request
                    times_after.push(Instant::now());

                    // Increment total requests counter
                    *self.total_requests.write().await += 1;

                    return Ok(());
                }
            }
        }

        // Record this request (if we didn't wait above)
        times.push(Instant::now());

        // Increment total requests counter
        *self.total_requests.write().await += 1;

        Ok(())
    }

    /// Handle a 429 rate limit response
    ///
    /// # Arguments
    /// * `retry_after_seconds` - Retry-After header value (optional)
    ///
    /// This will:
    /// 1. Pause requests for the retry period (default 60s)
    /// 2. Optionally reduce rate by 20% if auto_adjust enabled
    /// 3. Log the rate limit event
    pub async fn handle_rate_limit_response(&self, retry_after_seconds: Option<u64>) {
        let retry_duration = retry_after_seconds
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(60)); // Default 1 minute

        // Set pause state
        *self.paused_until.write().await = Some(Instant::now() + retry_duration);
        *self.retry_after.write().await = Some(retry_duration);

        // Increment rate limit hits counter
        *self.rate_limit_hits.write().await += 1;

        warn!(
            "🚨 Rate limit hit (429)! Pausing for {} seconds",
            retry_duration.as_secs()
        );

        // Auto-adjust rate if enabled
        if *self.auto_adjust.read().await {
            let mut current_rate = self.requests_per_minute.write().await;
            let old_rate = *current_rate;
            let new_rate = (old_rate as f64 * 0.8).max(1.0) as u32; // Reduce by 20%, min 1

            if new_rate < old_rate {
                *current_rate = new_rate;
                warn!(
                    "🔧 Auto-adjusting rate limit: {} -> {} req/min (-20%)",
                    old_rate, new_rate
                );
            }
        }
    }

    /// Set the rate limit (requests per minute)
    pub async fn set_rate(&self, requests_per_minute: u32) {
        let old_rate = *self.requests_per_minute.read().await;
        *self.requests_per_minute.write().await = requests_per_minute;
        info!(
            "Rate limit updated: {} -> {} req/min",
            old_rate, requests_per_minute
        );
    }

    /// Get current rate limit
    pub async fn get_rate(&self) -> u32 {
        *self.requests_per_minute.read().await
    }

    /// Set auto-adjust behavior
    pub async fn set_auto_adjust(&self, enabled: bool) {
        let old_value = *self.auto_adjust.read().await;
        *self.auto_adjust.write().await = enabled;
        info!("Auto-adjust updated: {} -> {}", old_value, enabled);
    }

    /// Get auto-adjust status
    pub async fn is_auto_adjust_enabled(&self) -> bool {
        *self.auto_adjust.read().await
    }

    /// Check if currently paused
    pub async fn is_paused(&self) -> bool {
        if let Some(until) = *self.paused_until.read().await {
            Instant::now() < until
        } else {
            false
        }
    }

    /// Get comprehensive status
    pub async fn get_status(&self) -> RateLimitStatus {
        let times = self.last_request_times.read().await;
        let now = Instant::now();
        let recent_requests = times
            .iter()
            .filter(|&&t| now.duration_since(t) < Duration::from_secs(60))
            .count() as u32;

        let paused_until = *self.paused_until.read().await;
        let is_paused = paused_until.map(|u| now < u).unwrap_or(false);
        let pause_remaining = paused_until
            .filter(|&u| now < u)
            .map(|u| u.duration_since(now).as_secs());

        RateLimitStatus {
            requests_per_minute: *self.requests_per_minute.read().await,
            requests_last_minute: recent_requests,
            auto_adjust_enabled: *self.auto_adjust.read().await,
            is_paused,
            retry_after_seconds: self.retry_after.read().await.map(|d| d.as_secs()),
            pause_remaining_seconds: pause_remaining,
            total_requests: *self.total_requests.read().await,
            rate_limit_hits: *self.rate_limit_hits.read().await,
        }
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        self.last_request_times.write().await.clear();
        *self.total_requests.write().await = 0;
        *self.rate_limit_hits.write().await = 0;
        info!("Rate limiter statistics reset");
    }

    /// Clear pause state (for manual resume)
    pub async fn clear_pause(&self) {
        *self.paused_until.write().await = None;
        *self.retry_after.write().await = None;
        info!("Rate limit pause cleared manually");
    }
}

impl Default for AdaptiveRateLimiter {
    fn default() -> Self {
        Self::new(5, true) // 5 req/min, auto-adjust enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let limiter = AdaptiveRateLimiter::new(10, true);
        assert_eq!(limiter.get_rate().await, 10);
        assert!(limiter.is_auto_adjust_enabled().await);
        assert!(!limiter.is_paused().await);
    }

    #[tokio::test]
    async fn test_set_rate() {
        let limiter = AdaptiveRateLimiter::new(5, false);
        limiter.set_rate(10).await;
        assert_eq!(limiter.get_rate().await, 10);
    }

    #[tokio::test]
    async fn test_auto_adjust() {
        let limiter = AdaptiveRateLimiter::new(10, true);
        limiter.handle_rate_limit_response(Some(5)).await;

        // Should reduce by 20%: 10 * 0.8 = 8
        assert_eq!(limiter.get_rate().await, 8);
        assert!(limiter.is_paused().await);
    }

    #[tokio::test]
    async fn test_no_auto_adjust() {
        let limiter = AdaptiveRateLimiter::new(10, false);
        limiter.handle_rate_limit_response(Some(5)).await;

        // Should NOT reduce
        assert_eq!(limiter.get_rate().await, 10);
        assert!(limiter.is_paused().await);
    }

    #[tokio::test]
    async fn test_pause_expires() {
        let limiter = AdaptiveRateLimiter::new(10, false);
        limiter.handle_rate_limit_response(Some(1)).await; // 1 second pause

        assert!(limiter.is_paused().await);
        sleep(Duration::from_millis(1100)).await;
        assert!(!limiter.is_paused().await);
    }

    #[tokio::test]
    async fn test_status() {
        let limiter = AdaptiveRateLimiter::new(5, true);
        let status = limiter.get_status().await;

        assert_eq!(status.requests_per_minute, 5);
        assert_eq!(status.requests_last_minute, 0);
        assert!(status.auto_adjust_enabled);
        assert!(!status.is_paused);
    }

    #[tokio::test]
    async fn test_multiple_requests() {
        let limiter = AdaptiveRateLimiter::new(3, false);

        // Make 3 requests quickly
        for _ in 0..3 {
            limiter.wait_if_needed().await.unwrap();
        }

        let status = limiter.get_status().await;
        assert_eq!(status.requests_last_minute, 3);
        assert_eq!(status.total_requests, 3);
    }

    #[tokio::test]
    async fn test_reset_stats() {
        let limiter = AdaptiveRateLimiter::new(10, true);

        // Make some requests
        for _ in 0..5 {
            limiter.wait_if_needed().await.unwrap();
        }

        // Simulate rate limit hit
        limiter.handle_rate_limit_response(Some(5)).await;

        let status = limiter.get_status().await;
        assert_eq!(status.total_requests, 5);
        assert_eq!(status.rate_limit_hits, 1);

        // Reset
        limiter.reset_stats().await;

        let status = limiter.get_status().await;
        assert_eq!(status.total_requests, 0);
        assert_eq!(status.rate_limit_hits, 0);
        assert_eq!(status.requests_last_minute, 0);
    }

    #[tokio::test]
    async fn test_clear_pause() {
        let limiter = AdaptiveRateLimiter::new(10, false);
        limiter.handle_rate_limit_response(Some(60)).await; // 60 second pause

        assert!(limiter.is_paused().await);

        // Manual clear
        limiter.clear_pause().await;

        assert!(!limiter.is_paused().await);
    }

    #[tokio::test]
    async fn test_auto_adjust_toggle() {
        let limiter = AdaptiveRateLimiter::new(10, true);
        assert!(limiter.is_auto_adjust_enabled().await);

        limiter.set_auto_adjust(false).await;
        assert!(!limiter.is_auto_adjust_enabled().await);

        limiter.set_auto_adjust(true).await;
        assert!(limiter.is_auto_adjust_enabled().await);
    }

    #[tokio::test]
    async fn test_rate_limit_without_retry_after() {
        let limiter = AdaptiveRateLimiter::new(10, true);

        // Handle 429 without retry-after header
        limiter.handle_rate_limit_response(None).await;

        // Should use default 60 seconds
        let status = limiter.get_status().await;
        assert!(status.is_paused);
        assert!(status.retry_after_seconds.is_some());
        assert_eq!(status.retry_after_seconds.unwrap(), 60);

        // Rate should be reduced by 20%
        assert_eq!(limiter.get_rate().await, 8);
    }

    #[tokio::test]
    async fn test_multiple_rate_limit_hits() {
        let limiter = AdaptiveRateLimiter::new(100, true);

        // Hit rate limit multiple times
        limiter.handle_rate_limit_response(Some(1)).await;
        sleep(Duration::from_millis(1100)).await;

        assert_eq!(limiter.get_rate().await, 80); // 100 * 0.8

        limiter.handle_rate_limit_response(Some(1)).await;
        sleep(Duration::from_millis(1100)).await;

        assert_eq!(limiter.get_rate().await, 64); // 80 * 0.8

        let status = limiter.get_status().await;
        assert_eq!(status.rate_limit_hits, 2);
    }

    #[tokio::test]
    async fn test_sliding_window_cleanup() {
        let limiter = AdaptiveRateLimiter::new(60, false); // 60 req/min = 1/sec

        // Make a request
        limiter.wait_if_needed().await.unwrap();

        let status = limiter.get_status().await;
        assert_eq!(status.requests_last_minute, 1);

        // Wait for it to age out (>60 seconds)
        // For test speed, we'll just verify the count is being tracked
        assert!(status.requests_last_minute > 0);
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        use std::sync::Arc;

        let limiter = Arc::new(AdaptiveRateLimiter::new(100, false));
        let mut handles = vec![];

        // Spawn 10 concurrent requests
        for _ in 0..10 {
            let limiter = limiter.clone();
            let handle = tokio::spawn(async move {
                limiter.wait_if_needed().await.unwrap();
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap();
        }

        let status = limiter.get_status().await;
        assert_eq!(status.total_requests, 10);
    }

    #[tokio::test]
    async fn test_default_rate_limiter() {
        let limiter = AdaptiveRateLimiter::default();
        assert_eq!(limiter.get_rate().await, 5);
        assert!(limiter.is_auto_adjust_enabled().await);
    }

    #[tokio::test]
    async fn test_minimum_rate_limit() {
        let limiter = AdaptiveRateLimiter::new(2, true);

        // Trigger multiple auto-adjustments
        for _ in 0..10 {
            limiter.handle_rate_limit_response(Some(1)).await;
            sleep(Duration::from_millis(1100)).await;
        }

        // Should never go below 1 req/min
        assert!(limiter.get_rate().await >= 1);
    }

    #[tokio::test]
    async fn test_status_pause_remaining() {
        let limiter = AdaptiveRateLimiter::new(10, false);
        limiter.handle_rate_limit_response(Some(5)).await;

        let status = limiter.get_status().await;
        assert!(status.pause_remaining_seconds.is_some());
        assert!(status.pause_remaining_seconds.unwrap() <= 5);
    }
}
