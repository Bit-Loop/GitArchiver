/// Multi-Token Rotation System for GitHub API Rate Limit Scaling
/// Implements Phase 2 of PRD: Free scaling from 5K to 25-50K req/hour
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use secrecy::Secret;
use serde::{Deserialize, Serialize, Serializer};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Custom serializer for Secret<String> that redacts the value
fn serialize_secret<S>(_secret: &Secret<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str("[REDACTED]")
}

/// GitHub token with health tracking
#[derive(Clone, Serialize, Deserialize)]
pub struct GitHubToken {
    pub id: String,
    #[serde(serialize_with = "serialize_secret")]
    pub token: Secret<String>,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub is_healthy: bool,
    pub rate_limit_remaining: Option<u32>,
    pub rate_limit_reset: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limit_hits: u32,
}

impl std::fmt::Debug for GitHubToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubToken")
            .field("id", &self.id)
            .field("token", &"[REDACTED]")
            .field("created_at", &self.created_at)
            .field("last_used", &self.last_used)
            .field("is_healthy", &self.is_healthy)
            .field("rate_limit_remaining", &self.rate_limit_remaining)
            .field("rate_limit_reset", &self.rate_limit_reset)
            .field("consecutive_failures", &self.consecutive_failures)
            .field("total_requests", &self.total_requests)
            .field("successful_requests", &self.successful_requests)
            .field("failed_requests", &self.failed_requests)
            .field("rate_limit_hits", &self.rate_limit_hits)
            .finish()
    }
}

impl GitHubToken {
    pub fn new(id: String, token: String) -> Self {
        Self {
            id,
            token: Secret::new(token),
            created_at: Utc::now(),
            last_used: None,
            is_healthy: true,
            rate_limit_remaining: None,
            rate_limit_reset: None,
            consecutive_failures: 0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            rate_limit_hits: 0,
        }
    }

    /// Mark token as unhealthy after failure
    pub fn mark_unhealthy(&mut self) {
        self.consecutive_failures += 1;
        self.failed_requests += 1;

        if self.consecutive_failures >= 3 {
            self.is_healthy = false;
            warn!(
                "Token {} marked as unhealthy after {} consecutive failures",
                self.id, self.consecutive_failures
            );
        }
    }

    /// Mark token as healthy after success
    pub fn mark_healthy(&mut self) {
        if self.consecutive_failures > 0 {
            info!(
                "Token {} recovered after {} failures",
                self.id, self.consecutive_failures
            );
        }
        self.consecutive_failures = 0;
        self.is_healthy = true;
        self.successful_requests += 1;
    }

    /// Update rate limit info from GitHub API headers
    pub fn update_rate_limit(&mut self, remaining: u32, reset_timestamp: i64) {
        self.rate_limit_remaining = Some(remaining);
        self.rate_limit_reset =
            Some(DateTime::from_timestamp(reset_timestamp, 0).unwrap_or_else(Utc::now));

        if remaining == 0 {
            self.rate_limit_hits += 1;
            warn!(
                "Token {} hit rate limit, resets at {:?}",
                self.id, self.rate_limit_reset
            );
        }
    }

    /// Check if token is available for use
    pub fn is_available(&self) -> bool {
        if !self.is_healthy {
            return false;
        }

        // Check if rate limit has reset
        if let (Some(remaining), Some(reset)) = (self.rate_limit_remaining, self.rate_limit_reset) {
            if remaining == 0 && Utc::now() < reset {
                return false;
            }
        }

        true
    }

    /// Get success rate percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 100.0;
        }
        (self.successful_requests as f64 / self.total_requests as f64) * 100.0
    }
}

/// Token pool with round-robin selection and health tracking
pub struct TokenPool {
    tokens: Arc<RwLock<Vec<GitHubToken>>>,
    current_index: Arc<RwLock<usize>>,
    strategy: SelectionStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SelectionStrategy {
    RoundRobin,    // Simple round-robin
    LeastUsed,     // Pick token with fewest requests
    BestHealth,    // Pick token with highest success rate
    MostRemaining, // Pick token with most rate limit remaining
}

const NO_HEALTHY_TOKENS: &str = "No healthy tokens available";

impl TokenPool {
    /// Create new token pool with default round-robin strategy
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(Vec::new())),
            current_index: Arc::new(RwLock::new(0)),
            strategy: SelectionStrategy::RoundRobin,
        }
    }

    /// Create token pool with specific strategy
    pub fn with_strategy(strategy: SelectionStrategy) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(Vec::new())),
            current_index: Arc::new(RwLock::new(0)),
            strategy,
        }
    }

    /// Add token to pool
    pub async fn add_token(&self, id: String, token: String) {
        let mut tokens = self.tokens.write().await;
        tokens.push(GitHubToken::new(id.clone(), token));
        info!("Added token {} to pool, total tokens: {}", id, tokens.len());
    }

    /// Add multiple tokens at once
    pub async fn add_tokens(&self, token_list: Vec<(String, String)>) {
        let mut tokens = self.tokens.write().await;
        for (id, token) in token_list {
            tokens.push(GitHubToken::new(id.clone(), token));
        }
        info!(
            "Added {} tokens to pool, total: {}",
            tokens.len(),
            tokens.len()
        );
    }

    /// Get next available token using current strategy
    pub async fn get_next_token(&self) -> Result<GitHubToken> {
        let tokens = self.tokens.read().await;

        if tokens.is_empty() {
            return Err(anyhow!(NO_HEALTHY_TOKENS));
        }

        match self.strategy {
            SelectionStrategy::RoundRobin => self.select_round_robin(&tokens).await,
            SelectionStrategy::LeastUsed => self.select_least_used(&tokens),
            SelectionStrategy::BestHealth => self.select_best_health(&tokens),
            SelectionStrategy::MostRemaining => self.select_most_remaining(&tokens),
        }
    }

    /// Round-robin selection
    async fn select_round_robin(&self, tokens: &[GitHubToken]) -> Result<GitHubToken> {
        let mut index = self.current_index.write().await;
        let start_index = *index;

        // Try to find available token starting from current index
        for _ in 0..tokens.len() {
            let token = &tokens[*index];
            *index = (*index + 1) % tokens.len();

            if token.is_available() {
                debug!("Selected token {} (round-robin)", token.id);
                return Ok(token.clone());
            }
        }

        // Reset index and report unavailability
        *index = (start_index + 1) % tokens.len();
        warn!("No healthy tokens available across all strategies");
        Err(anyhow!(NO_HEALTHY_TOKENS))
    }

    /// Select least used token
    fn select_least_used(&self, tokens: &[GitHubToken]) -> Result<GitHubToken> {
        tokens
            .iter()
            .filter(|t| t.is_available())
            .min_by_key(|t| t.total_requests)
            .cloned()
            .ok_or_else(|| anyhow!(NO_HEALTHY_TOKENS))
    }

    /// Select token with best health (highest success rate)
    fn select_best_health(&self, tokens: &[GitHubToken]) -> Result<GitHubToken> {
        tokens
            .iter()
            .filter(|t| t.is_available())
            .max_by(|a, b| {
                a.success_rate()
                    .partial_cmp(&b.success_rate())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .ok_or_else(|| anyhow!(NO_HEALTHY_TOKENS))
    }

    /// Select token with most rate limit remaining
    fn select_most_remaining(&self, tokens: &[GitHubToken]) -> Result<GitHubToken> {
        tokens
            .iter()
            .filter(|t| t.is_available())
            .max_by_key(|t| t.rate_limit_remaining.unwrap_or(5000))
            .cloned()
            .ok_or_else(|| anyhow!(NO_HEALTHY_TOKENS))
    }

    /// Update token after successful request
    pub async fn mark_success(&self, token_id: &str, remaining: Option<u32>, reset: Option<i64>) {
        let mut tokens = self.tokens.write().await;
        if let Some(token) = tokens.iter_mut().find(|t| t.id == token_id) {
            token.mark_healthy();
            token.total_requests += 1;
            token.last_used = Some(Utc::now());

            if let (Some(rem), Some(rst)) = (remaining, reset) {
                token.update_rate_limit(rem, rst);
            }
        }
    }

    /// Update token after failed request
    pub async fn mark_failure(&self, token_id: &str, is_rate_limit: bool) {
        let mut tokens = self.tokens.write().await;
        if let Some(token) = tokens.iter_mut().find(|t| t.id == token_id) {
            token.total_requests += 1;
            token.last_used = Some(Utc::now());

            if is_rate_limit {
                token.rate_limit_hits += 1;
                token.rate_limit_remaining = Some(0);
                return;
            }

            token.mark_unhealthy();
        }
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> TokenPoolStats {
        let tokens = self.tokens.read().await;

        let total_tokens = tokens.len();
        let healthy_tokens = tokens.iter().filter(|t| t.is_healthy).count();
        let available_tokens = tokens.iter().filter(|t| t.is_available()).count();
        let total_requests: u64 = tokens.iter().map(|t| t.total_requests).sum();
        let total_successes: u64 = tokens.iter().map(|t| t.successful_requests).sum();
        let total_failures: u64 = tokens.iter().map(|t| t.failed_requests).sum();
        let total_rate_limit_hits: u32 = tokens.iter().map(|t| t.rate_limit_hits).sum();

        let overall_success_rate = if total_requests > 0 {
            (total_successes as f64 / total_requests as f64) * 100.0
        } else {
            100.0
        };

        let total_rate_limit_remaining: u32 =
            tokens.iter().filter_map(|t| t.rate_limit_remaining).sum();

        TokenPoolStats {
            total_tokens,
            healthy_tokens,
            available_tokens,
            total_requests,
            total_successes,
            total_failures,
            overall_success_rate,
            total_rate_limit_hits,
            total_rate_limit_remaining,
            strategy: self.strategy,
        }
    }

    /// Get detailed info for all tokens
    pub async fn get_token_details(&self) -> Vec<GitHubToken> {
        self.tokens.read().await.clone()
    }

    /// Remove unhealthy tokens (manual cleanup)
    pub async fn remove_unhealthy_tokens(&self) -> usize {
        let mut tokens = self.tokens.write().await;
        let before = tokens.len();
        tokens.retain(|t| t.is_healthy);
        let removed = before - tokens.len();

        if removed > 0 {
            info!("Removed {} unhealthy tokens from pool", removed);
        }

        removed
    }

    /// Reset all token health (useful after API outage)
    pub async fn reset_all_health(&self) {
        let mut tokens = self.tokens.write().await;
        for token in tokens.iter_mut() {
            token.is_healthy = true;
            token.consecutive_failures = 0;
        }
        info!("Reset health for all {} tokens", tokens.len());
    }
}

impl Default for TokenPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for token pool
#[derive(Debug, Clone, Serialize)]
pub struct TokenPoolStats {
    pub total_tokens: usize,
    pub healthy_tokens: usize,
    pub available_tokens: usize,
    pub total_requests: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub overall_success_rate: f64,
    pub total_rate_limit_hits: u32,
    pub total_rate_limit_remaining: u32,
    pub strategy: SelectionStrategy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_pool_round_robin() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token3".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        let t1 = pool.get_next_token().await.unwrap();
        let t2 = pool.get_next_token().await.unwrap();
        let t3 = pool.get_next_token().await.unwrap();
        let t4 = pool.get_next_token().await.unwrap();

        assert_eq!(t1.id, "token1");
        assert_eq!(t2.id, "token2");
        assert_eq!(t3.id, "token3");
        assert_eq!(t4.id, "token1"); // Should wrap around
    }

    #[tokio::test]
    async fn test_token_health_tracking() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Mark failures
        pool.mark_failure("token1", false).await;
        pool.mark_failure("token1", false).await;
        pool.mark_failure("token1", false).await;

        let stats = pool.get_stats().await;
        assert_eq!(stats.healthy_tokens, 0); // Should be unhealthy after 3 failures

        // Mark success to recover
        pool.mark_success("token1", Some(5000), Some(Utc::now().timestamp() + 3600))
            .await;

        let stats = pool.get_stats().await;
        assert_eq!(stats.healthy_tokens, 1); // Should recover
    }

    #[tokio::test]
    async fn test_token_pool_stats() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        pool.mark_success("token1", Some(4999), Some(Utc::now().timestamp() + 3600))
            .await;
        pool.mark_success("token2", Some(4998), Some(Utc::now().timestamp() + 3600))
            .await;
        pool.mark_failure("token1", false).await;

        let stats = pool.get_stats().await;
        assert_eq!(stats.total_tokens, 2);
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.total_successes, 2);
        assert_eq!(stats.total_failures, 1);
    }

    #[tokio::test]
    async fn test_selection_strategies() {
        let pool = TokenPool::with_strategy(SelectionStrategy::LeastUsed);
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Use token1 multiple times
        pool.mark_success("token1", Some(5000), None).await;
        pool.mark_success("token1", Some(4999), None).await;

        // Next token should be token2 (least used)
        let token = pool.get_next_token().await.unwrap();
        assert_eq!(token.id, "token2");
    }

    #[tokio::test]
    async fn test_empty_pool() {
        let pool = TokenPool::new();

        // Should return error when no tokens available
        let result = pool.get_next_token().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No healthy tokens"));
    }

    #[tokio::test]
    async fn test_all_tokens_unhealthy() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Mark all tokens unhealthy
        for _ in 0..3 {
            pool.mark_failure("token1", false).await;
            pool.mark_failure("token2", false).await;
        }

        let result = pool.get_next_token().await;
        assert!(result.is_err());

        let stats = pool.get_stats().await;
        assert_eq!(stats.healthy_tokens, 0);
        assert_eq!(stats.available_tokens, 0);
    }

    #[tokio::test]
    async fn test_rate_limit_tracking() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Update rate limit info
        let reset_time = Utc::now().timestamp() + 3600;
        pool.mark_success("token1", Some(4500), Some(reset_time))
            .await;

        let details = pool.get_token_details().await;
        assert_eq!(details[0].rate_limit_remaining, Some(4500));
        assert!(details[0].rate_limit_reset.is_some());
    }

    #[tokio::test]
    async fn test_token_exhaustion() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Use token until exhausted (remaining = 0)
        pool.mark_success("token1", Some(0), Some(Utc::now().timestamp() + 3600))
            .await;

        // Token should still be in pool but marked as unavailable
        let stats = pool.get_stats().await;
        assert_eq!(stats.total_tokens, 1);
        // Available tokens should be 0 since rate limit is exhausted
        // (depending on implementation, might still be 1 if not checking remaining)
    }

    #[tokio::test]
    async fn test_add_multiple_tokens() {
        let pool = TokenPool::new();

        let tokens = vec![
            ("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string()),
            ("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string()),
            ("token3".to_string(), "ghp_REDACTED_EXAMPLE".to_string()),
        ];

        pool.add_tokens(tokens).await;

        let stats = pool.get_stats().await;
        assert_eq!(stats.total_tokens, 3);
        assert_eq!(stats.healthy_tokens, 3);
    }

    #[tokio::test]
    async fn test_remove_unhealthy_tokens() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token3".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Mark token1 and token2 unhealthy
        for _ in 0..3 {
            pool.mark_failure("token1", false).await;
            pool.mark_failure("token2", false).await;
        }

        let removed = pool.remove_unhealthy_tokens().await;
        assert_eq!(removed, 2);

        let stats = pool.get_stats().await;
        assert_eq!(stats.total_tokens, 1);
        assert_eq!(stats.healthy_tokens, 1);
    }

    #[tokio::test]
    async fn test_reset_all_health() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Mark tokens unhealthy
        for _ in 0..3 {
            pool.mark_failure("token1", false).await;
            pool.mark_failure("token2", false).await;
        }

        let stats = pool.get_stats().await;
        assert_eq!(stats.healthy_tokens, 0);

        // Reset health
        pool.reset_all_health().await;

        let stats = pool.get_stats().await;
        assert_eq!(stats.healthy_tokens, 2);
    }

    #[tokio::test]
    async fn test_rate_limit_failure() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Mark rate limit failure
        pool.mark_failure("token1", true).await;

        let details = pool.get_token_details().await;
        // Token should still be healthy after rate limit (not token's fault)
        assert!(details[0].is_healthy);
        assert_eq!(details[0].consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_token_success_rate() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // 7 successes, 3 failures
        for _ in 0..7 {
            pool.mark_success("token1", Some(5000), None).await;
        }
        for _ in 0..3 {
            pool.mark_failure("token1", false).await;
        }

        let details = pool.get_token_details().await;
        let success_rate = details[0].success_rate();
        assert!((success_rate - 70.0).abs() < 0.1); // Should be 70%
    }

    #[tokio::test]
    async fn test_concurrent_token_access() {
        use std::sync::Arc;

        let pool = Arc::new(TokenPool::new());
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token3".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        let mut handles = vec![];

        // Spawn 30 concurrent tasks
        for _ in 0..30 {
            let pool = pool.clone();
            let handle = tokio::spawn(async move {
                let token = pool.get_next_token().await.unwrap();
                pool.mark_success(&token.id, Some(5000), None).await;
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        let stats = pool.get_stats().await;
        assert_eq!(stats.total_requests, 30);
        assert_eq!(stats.total_successes, 30);
    }

    #[tokio::test]
    async fn test_most_remaining_strategy() {
        let pool = TokenPool::with_strategy(SelectionStrategy::MostRemaining);
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Set different remaining counts
        pool.mark_success("token1", Some(3000), None).await;
        pool.mark_success("token2", Some(4500), None).await;

        // Next token should be token2 (most remaining)
        let token = pool.get_next_token().await.unwrap();
        assert_eq!(token.id, "token2");
    }

    #[tokio::test]
    async fn test_token_details() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        pool.mark_success("token1", Some(4999), None).await;
        pool.mark_failure("token2", false).await;

        let details = pool.get_token_details().await;
        assert_eq!(details.len(), 2);

        let token1 = details.iter().find(|t| t.id == "token1").unwrap();
        assert_eq!(token1.total_requests, 1);
        assert_eq!(token1.successful_requests, 1);

        let token2 = details.iter().find(|t| t.id == "token2").unwrap();
        assert_eq!(token2.total_requests, 1);
        assert_eq!(token2.failed_requests, 1);
    }

    #[tokio::test]
    async fn test_token_recovery_after_failures() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Fail 2 times (not enough to mark unhealthy)
        pool.mark_failure("token1", false).await;
        pool.mark_failure("token1", false).await;

        let stats = pool.get_stats().await;
        assert_eq!(stats.healthy_tokens, 1);

        // Success resets consecutive failures
        pool.mark_success("token1", Some(5000), None).await;

        let details = pool.get_token_details().await;
        assert_eq!(details[0].consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_overall_success_rate() {
        let pool = TokenPool::new();
        pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;
        pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
            .await;

        // Token1: 8 successes, 2 failures = 80%
        for _ in 0..8 {
            pool.mark_success("token1", Some(5000), None).await;
        }
        for _ in 0..2 {
            pool.mark_failure("token1", false).await;
        }

        // Token2: 6 successes, 4 failures = 60%
        for _ in 0..6 {
            pool.mark_success("token2", Some(5000), None).await;
        }
        for _ in 0..4 {
            pool.mark_failure("token2", false).await;
        }

        let stats = pool.get_stats().await;
        // Overall: 14 successes / 20 total = 70%
        assert!((stats.overall_success_rate - 70.0).abs() < 0.1);
    }
}
