# Testing Guide - GitHub Events API Monitoring System

## Table of Contents
1. [Testing Overview](#testing-overview)
2. [Unit Tests](#unit-tests)
3. [Integration Tests](#integration-tests)
4. [Edge-Case Fuzz Tests](#edge-case-fuzz-tests)
5. [Performance Tests](#performance-tests)
6. [Security Tests](#security-tests)
7. [End-to-End Tests](#end-to-end-tests)
8. [CI/CD Integration](#cicd-integration)
9. [Test Coverage](#test-coverage)

---

## Testing Overview

### Test Pyramid
```
           ┌─────────────┐
           │  E2E Tests  │  (10%)
           ├─────────────┤
           │ Integration │  (20%)
           │    Tests    │
           ├─────────────┤
           │ Unit Tests  │  (70%)
           └─────────────┘
```

### Test Categories
- **Unit Tests**: Individual components (rate limiter, token pool, etc.)
- **Integration Tests**: Component interactions (API → Database, Webhooks)
- **Edge-Case Fuzz Tests**: Generated inputs for scanner spans, patch extraction, parsers, and token filtering
- **Performance Tests**: Load, stress, and scalability
- **Security Tests**: Authentication, authorization, input validation
- **E2E Tests**: Full user workflows

---

## Unit Tests

### Running Unit Tests
```bash
# Run all unit tests
cargo test --lib

# Run specific module tests
cargo test --lib realtime::rate_limiter
cargo test --lib realtime::token_pool
cargo test --lib realtime::metrics

# Run with output
cargo test --lib -- --nocapture

# Run with coverage
cargo tarpaulin --lib
```

### Key Unit Test Suites

#### 1. Rate Limiter Tests
```bash
cargo test --lib realtime::rate_limiter
```

**Tests**:
- ✅ Sliding window algorithm accuracy
- ✅ Auto-adjust on rate limit (50% reduction)
- ✅ Manual rate updates
- ✅ Pause/resume functionality
- ✅ Statistics tracking

**Example**:
```rust
#[tokio::test]
async fn test_rate_limiter_sliding_window() {
    let limiter = AdaptiveRateLimiter::new(10, false);
    // Should allow 10 requests in 60 seconds
    for _ in 0..10 {
        limiter.wait_if_needed().await;
    }
    // Next request should wait ~6 seconds
}
```

#### 2. Token Pool Tests
```bash
cargo test --lib realtime::token_pool
```

**Tests**:
- ✅ Round-robin selection
- ✅ Least-used selection
- ✅ Best-health selection
- ✅ Most-remaining selection
- ✅ Health tracking (3 failures = unhealthy)
- ✅ Auto-recovery on success
- ✅ Rate limit tracking

**Example**:
```rust
#[tokio::test]
async fn test_token_pool_health_tracking() {
    let pool = TokenPool::new();
    pool.add_token("token1".into(), "ghp_REDACTED_EXAMPLE".into()).await;
    
    // 3 failures = unhealthy
    pool.mark_failure("token1", false).await;
    pool.mark_failure("token1", false).await;
    pool.mark_failure("token1", false).await;
    
    let stats = pool.get_stats().await;
    assert_eq!(stats.healthy_tokens, 0);
}
```

#### 3. Webhook Tests
```bash
cargo test --lib realtime::webhook
```

**Tests**:
- ✅ Webhook endpoint CRUD
- ✅ Event filtering
- ✅ Auto-disable after 5 failures
- ✅ HMAC signature generation
- ✅ Retry logic (exponential backoff)

#### 4. Metrics Tests
```bash
cargo test --lib realtime::metrics
```

**Tests**:
- ✅ API request tracking
- ✅ Event counting
- ✅ Success rate calculation
- ✅ Health status (Healthy/Degraded/Unhealthy)
- ✅ Time series data (last 60 minutes)

---

## Integration Tests

### Running Integration Tests
```bash
# Run all integration tests
cargo test --test integration_tests

# Run specific test
cargo test --test integration_tests test_end_to_end_event_flow

# Run with database (requires PostgreSQL)
DATABASE_URL=postgresql://... cargo test --test integration_tests
```

### Key Integration Test Suites

#### 1. Event Flow Tests
**File**: `tests/integration_tests.rs`

```bash
cargo test --test integration_tests test_end_to_end_event_flow
```

**Flow**:
1. Fetch events from GitHub API (mock)
2. Parse and validate JSON
3. Store in PostgreSQL
4. Detect secrets
5. Send webhooks
6. Update metrics

**Assertions**:
- ✅ Events fetched successfully
- ✅ Database storage (>99% success)
- ✅ Secret detection triggers
- ✅ Webhooks sent
- ✅ Metrics updated

#### 2. Token Rotation Tests
```bash
cargo test --test integration_tests test_token_pool
```

**Scenarios**:
- Multiple tokens with round-robin
- Token health tracking
- Auto-switch on rate limit
- Performance comparison (least-used vs round-robin)

#### 3. Database Integration Tests
```bash
cargo test --test integration_tests test_database
```

**Tests**:
- Event upsert (INSERT ON CONFLICT)
- Duplicate detection
- Batch insertion (30 events)
- Query performance (<100ms)

#### 4. API Endpoint Tests
```bash
cargo test --test integration_tests test_api_endpoints
```

**Endpoints Tested**:
- `POST /api/realtime/start`
- `GET /api/realtime/status`
- `POST /api/tokens/add`
- `GET /api/webhooks`
- `GET /api/metrics`

---

## Edge-Case Fuzz Tests

The crate includes bounded property tests that run under the normal Rust test
suite and in GitHub Actions. They focus on production parser and scanner
invariants rather than long-running corpus fuzzing.

```bash
# Run the edge-case fuzz/property suite with default case count
cargo test --test edge_case_fuzz --locked

# Increase generated cases locally when changing detector logic
PROPTEST_CASES=512 cargo test --test edge_case_fuzz --locked
```

**Covered invariants**:
- Scanner findings always use valid UTF-8 byte spans into the scanned text.
- Matched text, entropy, hashes, and filenames stay internally consistent.
- Patch scanning only inspects added lines and skips diff metadata.
- GitHub token filtering trims usable tokens and rejects sample/example values.
- Secret category and severity parsers tolerate arbitrary labels.

---

## Performance Tests

### Running Performance Tests
```bash
# Benchmark rate limiter
cargo bench rate_limiter

# Load test with Apache Bench
ab -n 1000 -c 10 http://localhost:8081/api/health

# Load test with wrk
wrk -t4 -c100 -d30s http://localhost:8081/api/metrics
```

### Performance Benchmarks

#### 1. Rate Limiter Performance
```bash
cargo bench rate_limiter
```

**Metrics**:
- Sliding window algorithm: <1ms per request
- 10,000 requests/sec throughput
- Memory usage: <10MB for 10,000 requests

#### 2. Database Performance
```bash
cargo bench database
```

**Metrics**:
- Event insertion: <5ms (single)
- Batch insertion: <50ms (30 events)
- Query by ID: <2ms
- Query by date range: <100ms (10k events)

#### 3. API Latency
```bash
# Test with wrk
wrk -t4 -c100 -d30s http://localhost:8081/api/health
```

**Target Metrics**:
- P50 latency: <10ms
- P95 latency: <50ms
- P99 latency: <100ms
- Throughput: >1000 req/sec

#### 4. Memory & CPU
```bash
# Monitor during load test
./scripts/load_test.sh
```

**Target Metrics**:
- Memory: <200MB steady state
- CPU: <10% (2 cores)
- No memory leaks (24hr test)

---

## Security Tests

### Running Security Tests
```bash
# SQL injection tests
cargo test security::sql_injection

# XSS tests
cargo test security::xss

# Authentication tests
cargo test security::auth
```

### Security Test Suites

#### 1. SQL Injection Protection
```rust
#[test]
fn test_sql_injection_protection() {
    // Test parameterized queries prevent injection
    let malicious_input = "'; DROP TABLE github_events; --";
    // Should be safely escaped
}
```

#### 2. HMAC Signature Validation
```rust
#[test]
fn test_webhook_hmac_signature() {
    // Verify HMAC-SHA256 signatures
    let payload = r#"{"event": "secret_detected"}"#;
    let secret = "webhook_secret";
    let signature = generate_hmac(payload, secret);
    assert!(verify_hmac(payload, secret, &signature));
}
```

#### 3. Rate Limit Enforcement
```rust
#[test]
fn test_rate_limit_prevents_abuse() {
    // Ensure rate limiter blocks excessive requests
}
```

#### 4. Input Validation
```rust
#[test]
fn test_input_validation() {
    // Test all API endpoints reject invalid input
    // - Invalid JSON
    // - Missing required fields
    // - Out-of-range values
}
```

---

## End-to-End Tests

### Running E2E Tests
```bash
# Start test environment
docker-compose -f docker-compose.test.yml up -d

# Run E2E tests
cargo test --test e2e

# Cleanup
docker-compose -f docker-compose.test.yml down
```

### E2E Test Scenarios

#### 1. Complete Monitoring Workflow
```bash
cargo test --test e2e test_complete_workflow
```

**Steps**:
1. Start server
2. Configure tokens (add 3 tokens)
3. Start event monitoring
4. Verify events being fetched
5. Check database has events
6. Verify metrics updated
7. Stop monitoring
8. Cleanup

**Assertions**:
- ✅ Server starts successfully
- ✅ Tokens added to pool
- ✅ Events fetched every 12 seconds
- ✅ Database has 30+ events
- ✅ Metrics show success rate >95%

#### 2. Webhook Alert Flow
```bash
cargo test --test e2e test_webhook_alert_flow
```

**Steps**:
1. Add webhook endpoint (mock server)
2. Start monitoring
3. Inject event with secret
4. Verify webhook received alert
5. Check webhook stats

**Assertions**:
- ✅ Webhook endpoint created
- ✅ Secret detected in event
- ✅ Webhook called with correct payload
- ✅ HMAC signature valid

#### 3. Auto-Scale with Token Rotation
```bash
cargo test --test e2e test_auto_scale
```

**Steps**:
1. Start with single token
2. Hit rate limit (429)
3. Add 4 more tokens
4. Verify rate increases
5. Monitor for 5 minutes

**Assertions**:
- ✅ Initial rate: 60 req/hour
- ✅ After multi-token: 300 req/hour (5 tokens)
- ✅ No rate limit errors after scaling

---

## CI/CD Integration

### GitHub Actions Workflow
```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:15.14
        env:
          POSTGRES_DB: github_archiver_test
          POSTGRES_USER: test
          POSTGRES_PASSWORD: test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Cache cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run unit tests
        run: cargo test --lib
      
      - name: Run integration tests
        env:
          DATABASE_URL: postgresql://test:test@localhost/github_archiver_test
        run: cargo test --test integration_tests
      
      - name: Run benchmarks
        run: cargo bench --no-run
      
      - name: Generate coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          file: ./cobertura.xml
```

### Pre-commit Hooks
```bash
# Install pre-commit
pip install pre-commit

# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: cargo-test
        name: Cargo Test
        entry: cargo test --lib
        language: system
        pass_filenames: false
      
      - id: cargo-fmt
        name: Cargo Format
        entry: cargo fmt -- --check
        language: system
        pass_filenames: false
      
      - id: cargo-clippy
        name: Cargo Clippy
        entry: cargo clippy -- -D warnings
        language: system
        pass_filenames: false
```

---

## Test Coverage

### Generate Coverage Report
```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate HTML report
cargo tarpaulin --out Html

# Open report
open tarpaulin-report.html
```

### Coverage Targets
- **Overall**: >80%
- **Critical modules**: >90%
  - `realtime::rate_limiter`: 95%
  - `realtime::token_pool`: 92%
  - `api::handlers`: 88%
  - `core::database`: 85%

### Coverage by Module
```bash
cargo tarpaulin --out Stdout --verbose
```

**Example Output**:
```
|| Tested/Total Lines:
|| src/realtime/rate_limiter.rs: 310/325 (95.38%)
|| src/realtime/token_pool.rs: 380/412 (92.23%)
|| src/realtime/webhook.rs: 290/340 (85.29%)
|| src/realtime/metrics.rs: 260/300 (86.67%)
|| src/api/realtime_handlers.rs: 220/250 (88.00%)
|| 
|| Total: 1460/1627 (89.74%)
```

---

## Test Data

### Mock GitHub Events
```json
// tests/fixtures/github_events.json
[
  {
    "id": "12345",
    "type": "PushEvent",
    "created_at": "2025-10-06T12:00:00Z",
    "actor": {
      "id": 1,
      "login": "testuser",
      "url": "https://api.github.com/users/testuser"
    },
    "repo": {
      "id": 1,
      "name": "test/repo",
      "url": "https://api.github.com/repos/test/repo"
    },
    "payload": {
      "commits": [
        {
          "sha": "abc123",
          "message": "Add feature",
          "author": {
            "email": "test@example.com",
            "name": "Test User"
          }
        }
      ]
    }
  }
]
```

### Test Database Seeding
```sql
-- tests/fixtures/seed.sql
INSERT INTO github_events (event_id, event_type, event_created_at, ...)
VALUES ('test1', 'PushEvent', '2025-10-06 12:00:00', ...);
```

---

## Troubleshooting Tests

### Common Issues

#### 1. Database Connection Errors
```bash
# Ensure PostgreSQL is running
sudo systemctl status postgresql

# Check connection
psql -U test -d github_archiver_test -c "SELECT 1"

# Set DATABASE_URL
export DATABASE_URL=postgresql://test:test@localhost/github_archiver_test
```

#### 2. Flaky Tests
```bash
# Run tests multiple times
for i in {1..10}; do cargo test test_name; done

# Add retries to flaky tests
#[tokio::test]
#[retry(times = 3)]
async fn flaky_test() { ... }
```

#### 3. Timeout Errors
```bash
# Increase timeout
RUST_TEST_TIMEOUT=300 cargo test

# Or in test
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn slow_test() { ... }
```

---

## Best Practices

### 1. Test Naming
```rust
// ✅ Good
#[test]
fn test_rate_limiter_enforces_60_req_per_minute() { }

// ❌ Bad
#[test]
fn test1() { }
```

### 2. Arrange-Act-Assert Pattern
```rust
#[test]
fn test_example() {
    // Arrange
    let pool = TokenPool::new();
    
    // Act
    pool.add_token("token1".into(), "ghp_REDACTED_EXAMPLE".into()).await;
    
    // Assert
    assert_eq!(pool.get_stats().await.total_tokens, 1);
}
```

### 3. Test Isolation
```rust
// Each test should be independent
#[tokio::test]
async fn test_isolated() {
    let pool = TokenPool::new(); // Fresh instance
    // ...
}
```

### 4. Mocking External Services
```rust
use mockall::predicate::*;
use mockall::mock;

mock! {
    GitHubApi {}
    
    impl GitHubApi {
        async fn fetch_events() -> Result<Vec<Event>>;
    }
}
```

---

## Test Checklist

- [ ] All unit tests pass (`cargo test --lib`)
- [ ] All integration tests pass (`cargo test --test integration_tests`)
- [ ] All benchmarks compile (`cargo bench --no-run`)
- [ ] Coverage >80% (`cargo tarpaulin`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Security tests pass
- [ ] Performance benchmarks meet targets
- [ ] E2E tests pass
- [ ] CI/CD pipeline green

---

## Resources

- **Rust Testing**: https://doc.rust-lang.org/book/ch11-00-testing.html
- **Tokio Testing**: https://tokio.rs/tokio/topics/testing
- **Mockall**: https://docs.rs/mockall/
- **Tarpaulin**: https://github.com/xd009642/tarpaulin
- **Criterion**: https://bheisler.github.io/criterion.rs/book/

---

**Last Updated**: October 6, 2025  
**Maintained By**: GitHub Copilot AI Assistant
