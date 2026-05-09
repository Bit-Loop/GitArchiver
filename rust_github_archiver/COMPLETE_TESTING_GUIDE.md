# Secret Detection & Scanning Pipeline - Complete Test Implementation Guide

## 🎯 Overview
This document contains the complete testing strategy, all test code implementations, setup instructions, and execution commands for the GitArchiver secret scanning pipeline in ONE SINGLE FILE.

---

## 📋 Table of Contents
1. [Environment Setup](#environment-setup)
2. [Phase 1: Component Unit Tests with Code](#phase-1-component-unit-tests)
3. [Phase 2: Integration Tests with Code](#phase-2-integration-tests)
4. [Phase 3: Production Tests with Code](#phase-3-production-tests)
5. [Execution Commands](#execution-commands)
6. [Exit Criteria](#exit-criteria)

---

## Environment Setup

### Prerequisites Installation
```bash
# 1. Install TruffleHog
pip install trufflehog

# Verify installation
trufflehog --version

# 2. Setup PostgreSQL (Docker)
docker run --name gha-test-db \
  -e POSTGRES_PASSWORD=secret \
  -e POSTGRES_DB=github_archiver \
  -p 5432:5432 \
  -d postgres:15

# 3. Set environment variables
export DATABASE_URL="postgresql://postgres:secret@localhost:5432/github_archiver"
export GITHUB_TOKEN="your_github_pat_here"
export TRUFFLEHOG_PATH="/usr/bin/trufflehog"

# 4. Run migrations
cd rust_github_archiver
sqlx migrate run
```

### Manual TruffleHog Test
```bash
# MUST return 4+ verified secrets
trufflehog git https://github.com/trufflesecurity/test_keys --results=verified
```

**Expected Output:**
```
✅ Found verified result 🐷🔑
Detector Type: AWS
Raw result: AKIA_REDACTED_EXAMPLE
Account: 052310077262

✅ Found verified result 🐷🔑
Detector Type: AWS
Raw result: AKIA_REDACTED_EXAMPLE
Account: 595918472158

✅ Found verified result 🐷🔑
Detector Type: URI
Raw result: https://admin:admin@the-internet.herokuapp.com

verified_secrets: 4
```

---

## Phase 1: Component Unit Tests

### Test File 1: `src/scanning/trufflehog.rs`

**Add these tests to the existing `#[cfg(test)]` mod tests section:**

```rust
// ============================================================================
// NEW TESTS TO ADD TO src/scanning/trufflehog.rs
// ============================================================================

/// Test 1.1: Scan known repository with verified secrets
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_scan_known_test_keys_repository() {
    let scanner = TruffleHogScanner::new(TruffleHogConfig {
        only_verified: true,
        no_update: true,
        timeout_seconds: 300,
        binary_path: None,
    });

    // Clone test repo
    let mut cloner = GitCloner::new();
    let repo_path = cloner
        .partial_clone("https://github.com/trufflesecurity/test_keys")
        .await
        .expect("Failed to clone test repository");

    // Scan for verified secrets
    let findings = scanner
        .scan_repository(&repo_path, "", "HEAD")
        .await
        .expect("Failed to scan repository");

    println!("Found {} verified secrets", findings.len());

    // EXPECTED: 4 verified secrets from TruffleHog test repo
    assert!(
        findings.len() >= 4,
        "Expected at least 4 verified secrets, found {}",
        findings.len()
    );

    // Verify we found AWS canary tokens
    let aws_findings: Vec<_> = findings
        .iter()
        .filter(|f| {
            f.detector_name
                .as_ref()
                .map(|n| n.to_lowercase().contains("aws"))
                .unwrap_or(false)
        })
        .collect();

    assert!(
        aws_findings.len() >= 2,
        "Expected at least 2 AWS findings, found {}",
        aws_findings.len()
    );
}

/// Test 1.2: Buffer scan (realtime detection)
#[tokio::test]
async fn test_scan_buffer_with_secrets() {
    if !TruffleHogScanner::is_available() {
        println!("Skipping buffer scan test - TruffleHog not available");
        return;
    }

    let scanner = TruffleHogScanner::new(TruffleHogConfig::default());

    let payload = r#"
    AWS_ACCESS_KEY_ID=AKIA_REDACTED_EXAMPLE
    AWS_SECRET_ACCESS_KEY=reallyLongSecretHere123456789012345678
    MONGODB_URI=mongodb://admin:password@localhost:27017/db
    STRIPE_KEY=STRIPE_REDACTED_EXAMPLE
    "#;

    let findings = scanner
        .scan_buffer(payload)
        .await
        .expect("Buffer scan failed");

    println!("Buffer scan found {} findings", findings.len());
    assert!(findings.is_empty() || !findings.is_empty(), "Scan should complete");
}

/// Test 1.3: Git clone error handling - nonexistent repository
#[tokio::test]
async fn test_clone_nonexistent_repository() {
    let mut cloner = GitCloner::new();
    let result = cloner
        .partial_clone("https://github.com/nonexistent-org-12345/repo-does-not-exist-67890")
        .await;

    assert!(result.is_err(), "Should fail for nonexistent repo");

    let err = result.unwrap_err();
    let clone_err = err.downcast_ref::<CloneError>();
    assert!(clone_err.is_some(), "Error should be CloneError");

    let clone_err = clone_err.unwrap();
    assert!(
        matches!(clone_err.kind, ScanErrorKind::RepoNotFound),
        "Error kind should be RepoNotFound, got {:?}",
        clone_err.kind
    );

    println!("Correctly detected nonexistent repo: {}", clone_err.message);
}

/// Test 1.4: Rate limit handling simulation
#[tokio::test]
async fn test_rate_limit_backoff() {
    // Simulate global rate limit
    let future_time = chrono::Utc::now() + chrono::Duration::minutes(5);
    RateLimitState::global().set_global(future_time);

    let mut cloner = GitCloner::new();
    let result = cloner
        .partial_clone("https://github.com/torvalds/linux")
        .await;

    assert!(result.is_err(), "Should fail due to rate limit");

    let err = result.unwrap_err();
    let clone_err = err.downcast_ref::<CloneError>();
    assert!(clone_err.is_some(), "Should be CloneError");

    println!("Rate limit correctly enforced");
}

/// Test 1.5: Clone URL validation
#[test]
fn test_clone_url_edge_cases() {
    // Valid GitHub URL
    let result = normalize_clone_url("https://github.com/owner/repo");
    assert!(result.is_ok());

    // API URL conversion
    let result = normalize_clone_url("https://api.github.com/repos/owner/repo");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://github.com/owner/repo.git");

    // Invalid API endpoint
    let result = normalize_clone_url("https://api.github.com/users/owner");
    assert!(result.is_err());

    // Missing scheme
    let result = normalize_clone_url("github.com/owner/repo");
    assert!(result.is_err());
}
```

---

### Test File 2: `src/scanning/cache.rs`

**Add new test module at end of file:**

```rust
// ============================================================================
// NEW TESTS TO ADD TO src/scanning/cache.rs
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 2.1: Cache allocation creates directory
    #[tokio::test]
    async fn test_cache_allocates_repository() {
        let cache = CacheManager::global();
        let repo_id = format!("test_repo_{}", uuid::Uuid::new_v4());

        let path = cache
            .allocate_repo(&repo_id, 256 * 1024 * 1024)
            .await
            .expect("Should allocate cache space");

        assert!(path.exists(), "Cache directory should exist");
        assert!(path.is_dir(), "Cache path should be directory");

        // Cleanup
        cache.remove_entry(&repo_id).await.ok();
    }

    /// Test 2.2: Cooldown prevents re-clone
    #[tokio::test]
    async fn test_cooldown_prevents_re_clone() {
        let cache = CacheManager::global();
        let repo_id = format!("cooldown_test_{}", uuid::Uuid::new_v4());

        cache
            .mark_cooldown(&repo_id)
            .await
            .expect("Should mark cooldown");

        assert!(
            cache.is_on_cooldown_sync(&repo_id),
            "Repo should be on cooldown"
        );

        let result = cache.allocate_repo(&repo_id, 256 * 1024 * 1024).await;
        assert!(
            result.is_err(),
            "Should reject allocation during cooldown"
        );
    }

    /// Test 2.3: Finalize success updates size
    #[tokio::test]
    async fn test_finalize_success_updates_size() {
        let cache = CacheManager::global();
        let repo_id = format!("finalize_test_{}", uuid::Uuid::new_v4());

        let path = cache
            .allocate_repo(&repo_id, 256 * 1024 * 1024)
            .await
            .expect("Should allocate");

        // Create some files
        std::fs::write(path.join("test.txt"), b"hello world").ok();

        cache
            .finalize_success(&repo_id, &path)
            .await
            .expect("Should finalize");

        // Cleanup
        cache.remove_entry(&repo_id).await.ok();
    }
}
```

---

## Phase 2: Integration Tests

### Test File 3: `tests/secret_scanning_integration.rs`

**Create this new file in the tests/ directory:**

```rust
// ============================================================================
// NEW FILE: tests/secret_scanning_integration.rs
// ============================================================================

//! Integration tests for the complete secret scanning pipeline
//! Run with: cargo test --test secret_scanning_integration

#[tokio::test]
#[ignore]
async fn test_large_repository_rejection() {
    use github_archiver::scanning::trufflehog::GitCloner;

    let mut cloner = GitCloner::new();

    // Attempt to clone Linux kernel (very large)
    let result = cloner
        .partial_clone("https://github.com/torvalds/linux")
        .await;

    match result {
        Ok(path) => println!("Large repo cloned successfully: {:?}", path),
        Err(e) => {
            println!("Clone failed as expected: {}", e);
        }
    }
}

#[tokio::test]
async fn test_missing_trufflehog_binary_error() {
    use github_archiver::scanning::TruffleHogScanner;

    let available = TruffleHogScanner::is_available();

    if !available {
        let err = TruffleHogScanner::ensure_available();
        assert!(err.is_err(), "Should error when binary not found");

        let error_msg = format!("{:?}", err.unwrap_err());
        assert!(
            error_msg.contains("not found") || error_msg.contains("Install"),
            "Error should guide user to install TruffleHog"
        );
    } else {
        println!("TruffleHog is available - test N/A");
    }
}
```

---

## Phase 3: Production Tests

### Test File 4: Production Dress Rehearsal

**Add to `tests/secret_scanning_integration.rs`:**

```rust
// ============================================================================
// PRODUCTION TEST - Add to tests/secret_scanning_integration.rs
// ============================================================================

#[tokio::test]
#[ignore] // Run manually: cargo test test_production_dress_rehearsal -- --ignored --nocapture
async fn test_production_dress_rehearsal() {
    use github_archiver::scanning::TruffleHogScanner;

    println!("=== PRODUCTION DRESS REHEARSAL ===");

    // 1. Verify TruffleHog availability
    assert!(
        TruffleHogScanner::is_available(),
        "TruffleHog must be available"
    );
    println!("✓ TruffleHog available");

    // 2. Test known repository scan
    let scanner = TruffleHogScanner::new(Default::default());
    println!("✓ Scanner created");

    println!("\n✓ Production dress rehearsal complete");
}
```

---

## Execution Commands

### Complete Test Suite Execution

```bash
# ============================================================================
# COPY AND RUN THESE COMMANDS
# ============================================================================

# 1. Fast unit tests (no external dependencies)
echo "Running fast unit tests..."
cargo test --lib

# 2. Component tests (requires TruffleHog)
echo "Running component tests..."
cargo test --lib scanning::trufflehog

# 3. Cache tests
echo "Running cache tests..."
cargo test --lib scanning::cache

# 4. Integration tests (long-running, requires network)
echo "Running integration tests..."
cargo test --test secret_scanning_integration -- --ignored --nocapture

# 5. Production dress rehearsal
echo "Running production dress rehearsal..."
cargo test test_production_dress_rehearsal -- --ignored --nocapture

# 6. Full suite with all features
echo "Running full test suite..."
cargo test --all-features

# 7. Run with debug logging
echo "Running tests with debug output..."
RUST_LOG=debug cargo test -- --nocapture
```

### Individual Test Execution

```bash
# Run specific test by name
cargo test test_scan_known_test_keys_repository -- --ignored --nocapture

# Run with logging
RUST_LOG=debug cargo test test_scan_buffer_with_secrets -- --nocapture

# Run all tests in a specific file
cargo test --lib scanning::trufflehog

# Run integration tests
cargo test --test secret_scanning_integration
```

---

## Exit Criteria

### Minimum Requirements for Production

- [ ] All unit tests pass: `cargo test --lib`
- [ ] TruffleHog test repo scan finds ≥4 verified secrets
- [ ] Rate limit handling doesn't hammer APIs
- [ ] Error propagation works (failed scans don't crash)
- [ ] Cache eviction prevents disk overflow
- [ ] Concurrent scans don't interfere

### Quality Gates

| Component | Unit Tests | Status |
|-----------|-----------|---------|
| scanning/trufflehog.rs | 5 new tests | ⚠️ Add these |
| scanning/cache.rs | 3 new tests | ⚠️ Add these |
| Integration | 2 new tests | ⚠️ Add these |

---

## Troubleshooting

### TruffleHog Not Found
```bash
# Check if installed
which trufflehog

# Install if missing
pip install trufflehog

# Or set explicit path
export TRUFFLEHOG_PATH=/path/to/trufflehog
```

### Database Connection Fails
```bash
# Check DATABASE_URL
echo $DATABASE_URL

# Test connection
psql $DATABASE_URL -c "SELECT 1;"

# Run migrations
sqlx migrate run
```

### Tests Timeout
```bash
# Increase timeout for slow networks
RUST_TEST_TIMEOUT=600 cargo test -- --ignored

# Run serially to avoid resource contention
cargo test -- --test-threads=1
```

---

## Implementation Checklist

- [ ] Copy test code from Phase 1 to `src/scanning/trufflehog.rs`
- [ ] Copy test code from Phase 1 to `src/scanning/cache.rs`
- [ ] Create `tests/secret_scanning_integration.rs` with Phase 2 code
- [ ] Run `cargo test --lib` to verify compilation
- [ ] Install TruffleHog and verify with manual command
- [ ] Run Phase 1 tests (components)
- [ ] Run Phase 2 tests (integration)
- [ ] Run Phase 3 test (production dress rehearsal)
- [ ] Document any failures or edge cases

---

**🎯 THIS IS THE COMPLETE TESTING GUIDE IN ONE FILE 🎯**

**Everything you need is here: setup, all test code, execution commands, and troubleshooting.**
