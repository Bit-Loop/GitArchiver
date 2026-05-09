/// Integration Tests for GitHub Events API Monitoring System
/// Tests end-to-end event flow, rate limiting, token rotation, and webhooks
use github_archiver::realtime::{
    metrics::MetricsCollector,
    token_pool::{SelectionStrategy, TokenPool},
    webhook::WebhookManager,
    AdaptiveRateLimiter, GitHubEventMonitor,
};
use std::sync::Arc;
use tokio::time::Duration;

/// Test: Basic event monitoring without authentication
#[tokio::test]
async fn test_unauthenticated_event_monitoring() {
    let monitor = GitHubEventMonitor::new("").await.unwrap();

    // Monitor should be created successfully
    assert!(!monitor.is_running().await);
}

/// Test: Rate limiter enforces limits correctly
#[tokio::test]
async fn test_rate_limiter_enforcement() {
    let rate_limiter = AdaptiveRateLimiter::new(5, false); // 5 req/min, no auto-adjust

    let start = std::time::Instant::now();

    // Make 7 requests rapidly - first 5 should go through, then need to wait
    for i in 0..7 {
        let _ = rate_limiter.wait_if_needed().await;
        if i < 5 {
            // First 5 should be instant
            assert!(start.elapsed().as_millis() < 100);
        }
    }

    let elapsed = start.elapsed();

    // After 5 requests hit the limit, should wait for sliding window
    // With 5 req/min, the 6th request should wait up to 60 seconds
    // We expect at least some delay (a few seconds)
    assert!(
        elapsed.as_secs() >= 1,
        "Expected at least 1 second delay after hitting limit, got {} ms",
        elapsed.as_millis()
    );
}

/// Test: Rate limiter auto-adjusts on rate limit hit
#[tokio::test]
async fn test_rate_limiter_auto_adjust() {
    let rate_limiter = AdaptiveRateLimiter::new(10, true); // 10 req/min with auto-adjust

    let initial_rate = rate_limiter.get_status().await.requests_per_minute;
    assert_eq!(initial_rate, 10);

    // Simulate rate limit hit
    rate_limiter.handle_rate_limit_response(Some(60)).await;

    let adjusted_rate = rate_limiter.get_status().await.requests_per_minute;
    assert_eq!(adjusted_rate, 8); // Should be reduced by 20%
}
/// Test: Token pool round-robin selection
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
    assert_eq!(t4.id, "token1"); // Wraps around
}

/// Test: Token pool handles unhealthy tokens
#[tokio::test]
async fn test_token_pool_health_tracking() {
    let pool = TokenPool::new();

    pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;
    pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;

    // Mark token1 as unhealthy
    pool.mark_failure("token1", false).await;
    pool.mark_failure("token1", false).await;
    pool.mark_failure("token1", false).await;

    let stats = pool.get_stats().await;
    assert_eq!(stats.healthy_tokens, 1); // Only token2 healthy
    assert_eq!(stats.available_tokens, 1);

    // Next token should skip token1
    let token = pool.get_next_token().await.unwrap();
    assert_eq!(token.id, "token2");
}

/// Test: Token pool least-used selection strategy
#[tokio::test]
async fn test_token_pool_least_used_strategy() {
    let pool = TokenPool::with_strategy(SelectionStrategy::LeastUsed);

    pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;
    pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;

    // Use token1 multiple times
    pool.mark_success("token1", Some(5000), None).await;
    pool.mark_success("token1", Some(4999), None).await;
    pool.mark_success("token1", Some(4998), None).await;

    // Next should be token2 (least used)
    let token = pool.get_next_token().await.unwrap();
    assert_eq!(token.id, "token2");
}

/// Test: Webhook manager adds and removes endpoints
#[tokio::test]
async fn test_webhook_manager() {
    let manager = WebhookManager::new();

    let id1 = manager
        .add_endpoint(
            "https://example.com/webhook1".to_string(),
            Some("secret123".to_string()),
            vec!["secret_detected".to_string()],
        )
        .await;

    let id2 = manager
        .add_endpoint(
            "https://example.com/webhook2".to_string(),
            None,
            vec!["high_severity".to_string()],
        )
        .await;

    let endpoints = manager.get_endpoints().await;
    assert_eq!(endpoints.len(), 2);

    manager.remove_endpoint(id1).await.unwrap();

    let endpoints = manager.get_endpoints().await;
    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0].id, id2);
}

/// Test: Webhook endpoint auto-disables after failures
#[tokio::test]
async fn test_webhook_auto_disable() {
    use github_archiver::realtime::webhook::WebhookEndpoint;

    let mut webhook = WebhookEndpoint::new(
        "https://example.com/webhook".to_string(),
        None,
        vec!["secret_detected".to_string()],
    );

    assert!(webhook.active);

    // Fail 5 times
    for _ in 0..5 {
        webhook.mark_failure();
    }

    assert!(!webhook.active);
    assert_eq!(webhook.consecutive_failures, 5);
}

/// Test: Metrics collector tracks API requests
#[tokio::test]
async fn test_metrics_collector_api_tracking() {
    let collector = MetricsCollector::new();

    collector.record_api_request(true, 100.0).await;
    collector.record_api_request(true, 200.0).await;
    collector.record_api_request(false, 150.0).await;
    collector.record_api_request(true, 120.0).await;

    let metrics = collector.get_metrics().await;

    assert_eq!(metrics.api_requests, 4);
    assert_eq!(metrics.api_success, 3);
    assert_eq!(metrics.api_failures, 1);
    assert_eq!(metrics.success_rate(), 75.0);

    // Average should be (100 + 200 + 150 + 120) / 4 = 142.5
    assert!((metrics.avg_fetch_time_ms - 142.5).abs() < 0.1);
}

/// Test: Metrics collector tracks events
#[tokio::test]
async fn test_metrics_collector_event_tracking() {
    let collector = MetricsCollector::new();

    collector.record_events_fetched(30, "PushEvent").await;
    collector.record_events_fetched(20, "IssuesEvent").await;
    collector.record_events_fetched(15, "PushEvent").await;
    collector.record_events_stored(60, 500.0).await;
    collector.record_duplicate(5).await;

    let metrics = collector.get_metrics().await;

    assert_eq!(metrics.events_fetched, 65);
    assert_eq!(metrics.events_stored, 60);
    assert_eq!(metrics.events_duplicate, 5);
    assert_eq!(metrics.storage_success_rate(), 92.307_692_307_692_3);

    // Check event type breakdown
    assert_eq!(metrics.event_types.get("PushEvent"), Some(&45));
    assert_eq!(metrics.event_types.get("IssuesEvent"), Some(&20));
}

/// Test: Metrics collector health status
#[tokio::test]
async fn test_metrics_health_status() {
    use github_archiver::realtime::metrics::HealthStatus;

    let collector = MetricsCollector::new();

    // Healthy: 100% success
    for _ in 0..100 {
        collector.record_api_request(true, 100.0).await;
    }

    let report = collector.get_report().await;
    assert_eq!(report.health_status, HealthStatus::Healthy);

    // Reset and test degraded
    collector.reset().await;

    // Degraded: 94% success (6% error)
    for _ in 0..94 {
        collector.record_api_request(true, 100.0).await;
    }
    for _ in 0..6 {
        collector.record_api_request(false, 100.0).await;
    }

    let report = collector.get_report().await;
    assert_eq!(report.health_status, HealthStatus::Degraded);

    // Reset and test unhealthy
    collector.reset().await;

    // Unhealthy: 85% success (15% error)
    for _ in 0..85 {
        collector.record_api_request(true, 100.0).await;
    }
    for _ in 0..15 {
        collector.record_api_request(false, 100.0).await;
    }

    let report = collector.get_report().await;
    assert_eq!(report.health_status, HealthStatus::Unhealthy);
}

/// Test: End-to-end event flow (mock)
#[tokio::test]
async fn test_end_to_end_event_flow() {
    let collector = MetricsCollector::new();

    // Simulate fetching events
    collector.record_api_request(true, 150.0).await;
    collector.record_events_fetched(30, "PushEvent").await;

    // Simulate storing events
    collector.record_events_stored(30, 200.0).await;

    // Simulate detecting secrets
    collector.record_secret_detected("critical").await;
    collector.record_secret_detected("high").await;

    // Simulate sending webhooks
    collector.record_webhook(true).await;
    collector.record_webhook(true).await;

    let metrics = collector.get_metrics().await;

    assert_eq!(metrics.api_requests, 1);
    assert_eq!(metrics.events_fetched, 30);
    assert_eq!(metrics.events_stored, 30);
    assert_eq!(metrics.secrets_detected, 2);
    assert_eq!(metrics.critical_severity_secrets, 1);
    assert_eq!(metrics.high_severity_secrets, 1);
    assert_eq!(metrics.webhooks_sent, 2);
    assert_eq!(metrics.webhooks_success, 2);
}

/// Test: Concurrent token access (stress test)
#[tokio::test]
async fn test_concurrent_token_access() {
    let pool = Arc::new(TokenPool::new());

    pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;
    pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;
    pool.add_token("token3".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;

    let mut handles = vec![];

    // Spawn 100 concurrent tasks
    for _ in 0..100 {
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
    assert_eq!(stats.total_requests, 100);
    assert_eq!(stats.total_successes, 100);
}

/// Test: Time series data tracking
#[tokio::test]
async fn test_time_series_tracking() {
    let collector = MetricsCollector::new();

    // Add time series points
    for i in 0..5 {
        collector.add_time_series_point("events", i * 10).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let metrics = collector.get_metrics().await;
    assert_eq!(metrics.events_per_minute.len(), 5);

    // Values should be 0, 10, 20, 30, 40
    assert_eq!(metrics.events_per_minute[0].value, 0);
    assert_eq!(metrics.events_per_minute[4].value, 40);
}

/// Test: Token pool statistics accuracy
#[tokio::test]
async fn test_token_pool_statistics() {
    let pool = TokenPool::new();

    pool.add_token("token1".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;
    pool.add_token("token2".to_string(), "ghp_REDACTED_EXAMPLE".to_string())
        .await;

    // Use tokens with different outcomes
    pool.mark_success("token1", Some(4999), Some(1000000000))
        .await;
    pool.mark_success("token1", Some(4998), Some(1000000000))
        .await;
    pool.mark_failure("token1", false).await;

    pool.mark_success("token2", Some(5000), Some(1000000000))
        .await;
    pool.mark_success("token2", Some(4999), Some(1000000000))
        .await;
    pool.mark_success("token2", Some(4998), Some(1000000000))
        .await;

    let stats = pool.get_stats().await;

    assert_eq!(stats.total_tokens, 2);
    assert_eq!(stats.total_requests, 6);
    assert_eq!(stats.total_successes, 5);
    assert_eq!(stats.total_failures, 1);

    // Overall success rate: 5/6 = 83.33%
    assert!((stats.overall_success_rate - 83.33333333333334).abs() < 0.1);
}
