use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window for rate limiting
    pub window: Duration,
    /// Burst size (allows brief spikes)
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 60,
            window: Duration::from_secs(60),
            burst_size: 10,
        }
    }
}

/// Rate limiter using token bucket algorithm
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
}

impl TokenBucket {
    fn new(max_tokens: u32, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens as f64,
            last_refill: Instant::now(),
            max_tokens: max_tokens as f64,
            refill_rate,
        }
    }

    fn try_consume(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }

    fn remaining(&self) -> u32 {
        self.tokens as u32
    }
}

/// Rate limiter that tracks multiple clients
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check if request is allowed for given identifier
    pub async fn check_rate_limit(&self, identifier: &str) -> Result<RateLimitInfo, StatusCode> {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets.entry(identifier.to_string()).or_insert_with(|| {
            let refill_rate = self.config.max_requests as f64 / self.config.window.as_secs_f64();
            TokenBucket::new(
                self.config.max_requests + self.config.burst_size,
                refill_rate,
            )
        });

        if bucket.try_consume() {
            Ok(RateLimitInfo {
                limit: self.config.max_requests,
                remaining: bucket.remaining(),
                reset: Instant::now() + self.config.window,
            })
        } else {
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }

    /// Clean up old entries periodically
    pub async fn cleanup(&self) {
        let mut buckets = self.buckets.write().await;
        let cutoff = Instant::now() - Duration::from_secs(3600); // 1 hour

        buckets.retain(|_, bucket| bucket.last_refill > cutoff);
    }
}

/// Rate limit information to include in response headers
#[derive(Debug)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset: Instant,
}

/// Extract client identifier from request
fn get_client_identifier(req: &Request) -> String {
    let trust_proxy_headers = std::env::var("TRUST_PROXY_HEADERS")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if trust_proxy_headers {
        // Proxy headers are accepted only when the deployment explicitly enables trusted proxy mode.
        if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                if let Some(ip) = forwarded_str.split(',').next() {
                    return ip.trim().to_string();
                }
            }
        }

        if let Some(real_ip) = req.headers().get("X-Real-IP") {
            if let Ok(ip_str) = real_ip.to_str() {
                return ip_str.to_string();
            }
        }
    }

    // Fall back to a stable bucket when proxy headers are unavailable.
    "unknown".to_string()
}

/// Axum middleware for rate limiting
pub async fn rate_limit_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    // Get rate limiter from request extensions
    let rate_limiter = req
        .extensions()
        .get::<Arc<RateLimiter>>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        .clone();

    let identifier = get_client_identifier(&req);

    match rate_limiter.check_rate_limit(&identifier).await {
        Ok(info) => {
            let mut response = next.run(req).await;

            // Add rate limit headers. Values are numeric and should always be valid header values.
            if let Ok(value) = HeaderValue::from_str(&info.limit.to_string()) {
                response.headers_mut().insert("X-RateLimit-Limit", value);
            }
            if let Ok(value) = HeaderValue::from_str(&info.remaining.to_string()) {
                response
                    .headers_mut()
                    .insert("X-RateLimit-Remaining", value);
            }
            let reset_seconds = info
                .reset
                .checked_duration_since(Instant::now())
                .unwrap_or_default()
                .as_secs();
            if let Ok(value) = HeaderValue::from_str(&reset_seconds.to_string()) {
                response.headers_mut().insert("X-RateLimit-Reset", value);
            }

            Ok(response)
        }
        Err(status) => {
            // Return 429 Too Many Requests
            Err(status)
        }
    }
}

/// Per-endpoint rate limiting configuration
pub struct EndpointRateLimits {
    /// Default rate limit for all endpoints
    pub default: RateLimitConfig,
    /// Specific rate limits for certain endpoints
    pub endpoints: HashMap<String, RateLimitConfig>,
}

impl EndpointRateLimits {
    pub fn new() -> Self {
        let mut endpoints = HashMap::new();

        // API endpoints - stricter limits
        endpoints.insert(
            "/api/v1/events".to_string(),
            RateLimitConfig {
                max_requests: 1000,
                window: Duration::from_secs(60),
                burst_size: 100,
            },
        );

        // Auth endpoints - very strict
        endpoints.insert(
            "/api/v1/auth/login".to_string(),
            RateLimitConfig {
                max_requests: 5,
                window: Duration::from_secs(60),
                burst_size: 2,
            },
        );

        // Admin endpoints - moderate limits
        endpoints.insert(
            "/api/v1/admin".to_string(),
            RateLimitConfig {
                max_requests: 100,
                window: Duration::from_secs(60),
                burst_size: 20,
            },
        );

        Self {
            default: RateLimitConfig::default(),
            endpoints,
        }
    }

    pub fn get_config(&self, path: &str) -> RateLimitConfig {
        self.endpoints
            .get(path)
            .cloned()
            .unwrap_or_else(|| self.default.clone())
    }
}

impl Default for EndpointRateLimits {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    #[test]
    fn test_token_bucket_consume() {
        let mut bucket = TokenBucket::new(10, 1.0);

        // Should be able to consume tokens
        assert!(bucket.try_consume());
        assert_eq!(bucket.remaining(), 9);
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10, 10.0); // 10 tokens per second

        // Consume all tokens
        for _ in 0..10 {
            bucket.try_consume();
        }
        assert_eq!(bucket.remaining(), 0);

        // Should not allow more
        assert!(!bucket.try_consume());

        // Wait for refill (in real test, would use tokio::time::sleep)
        std::thread::sleep(Duration::from_millis(200));
        bucket.refill();

        // Should have refilled ~2 tokens
        assert!(bucket.remaining() >= 1);
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let config = RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
            burst_size: 0,
        };

        let limiter = RateLimiter::new(config);

        // Should allow first 5 requests
        for _ in 0..5 {
            assert!(limiter.check_rate_limit("test-client").await.is_ok());
        }

        // Should deny 6th request
        assert!(limiter.check_rate_limit("test-client").await.is_err());

        // Different client should be allowed
        assert!(limiter.check_rate_limit("other-client").await.is_ok());
    }

    #[test]
    fn client_identifier_ignores_forwarded_headers_by_default() {
        let _guard = env_lock();
        let request = Request::builder()
            .header("X-Forwarded-For", "203.0.113.5")
            .body(axum::body::Body::empty())
            .expect("request");

        std::env::remove_var("TRUST_PROXY_HEADERS");

        assert_eq!(get_client_identifier(&request), "unknown");
    }

    #[test]
    fn client_identifier_uses_forwarded_headers_only_in_trusted_proxy_mode() {
        let _guard = env_lock();
        let request = Request::builder()
            .header("X-Forwarded-For", "203.0.113.5, 10.0.0.1")
            .body(axum::body::Body::empty())
            .expect("request");

        std::env::set_var("TRUST_PROXY_HEADERS", "true");

        assert_eq!(get_client_identifier(&request), "203.0.113.5");
        std::env::remove_var("TRUST_PROXY_HEADERS");
    }
}
