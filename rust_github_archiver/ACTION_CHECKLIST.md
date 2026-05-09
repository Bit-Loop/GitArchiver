# ✅ Action Checklist - GitArchiver Improvements

**Generated:** October 4, 2025  
**Based on:** CODE_ANALYSIS_REPORT.md

---

## 🚨 CRITICAL (Do This Week)

### Day 1: Quick Wins (2 hours)
- [x] ✅ Fix compile warnings in `src/performance/mod.rs`
- [ ] 🔴 Fix XML parsing in `src/scraper/archive_scraper.rs:128-129`
  ```bash
  # Replace unwrap() with proper error handling
  vim +128 src/scraper/archive_scraper.rs
  ```
- [ ] 🔴 Create `.env.example` file
  ```bash
  cat > .env.example << 'EOF'
  # Database Configuration
  DB_HOST=localhost
  DB_PORT=5432
  DB_NAME=github_archiver
  DB_USER=github_archiver
  DB_PASSWORD=github_archiver_password
  DB_MIN_CONNECTIONS=5
  DB_MAX_CONNECTIONS=20
  
  # GitHub API (Required for scraping)
  GITHUB_TOKEN=ghp_REDACTED_EXAMPLE
  GITHUB_USERNAME=your_username
  
  # API Server
  API_HOST=0.0.0.0
  API_PORT=8081
  
  # JWT Secret (Generate with: openssl rand -hex 32)
  JWT_SECRET=replace_with_random_32_byte_hex
  
  # Security
  ADMIN_PASSWORD=change_on_first_login
  
  # Optional: Redis Cache
  REDIS_URL=redis://localhost:6379
  
  # Optional: Monitoring
  PROMETHEUS_ENABLED=false
  GRAFANA_ENABLED=false
  EOF
  ```

### Day 2: Security Hardening (4 hours)
- [ ] 🔴 **Admin Password Security**
  - Add forced password change on first login
  - Generate random initial password on setup
  - Log warning if default password detected
  
- [ ] 🔴 **Rate Limiting**
  ```bash
  # Add to Cargo.toml
  echo 'tower-governor = "0.3"' >> Cargo.toml
  ```
  - Implement on `/api/auth/login` (10 req/min)
  - Implement on `/api/scanner/scan` (5 req/min)
  
- [ ] 🔴 **Request Logging Middleware**
  ```rust
  // Add to src/api/middleware/logging.rs
  - Log all requests with sanitized data (no passwords/tokens)
  - Include: timestamp, IP, endpoint, status, duration
  ```

### Day 3: Error Handling Cleanup (4 hours)
- [ ] 🟡 Replace unwrap() in production code
  - `src/scraper/archive_scraper.rs` (10+ instances)
  - `src/schema/core.rs` (cache handling)
  - `src/cli.rs` (argument parsing)
  - `src/github/dangling_commits.rs` (string operations)
  
- [ ] 🟡 Add better error contexts
  ```rust
  // Instead of:
  let value = operation()?;
  
  // Use:
  let value = operation()
      .context("Failed to perform operation on resource X")?;
  ```

---

## 🔧 HIGH PRIORITY (This Week)

### Day 4: Configuration Management (3 hours)
- [ ] 🟡 Create `config.toml` template
  ```bash
  cat > config.toml.example << 'EOF'
  [database]
  host = "localhost"
  port = 5432
  name = "github_archiver"
  user = "github_archiver"
  password = "${DB_PASSWORD}"  # From environment
  pool_min = 5
  pool_max = 20
  
  [security]
  jwt_secret = "${JWT_SECRET}"
  admin_default_password = "${ADMIN_PASSWORD}"
  session_timeout_minutes = 60
  force_password_change = true
  
  [api]
  host = "0.0.0.0"
  port = 8081
  rate_limit_rpm = 100
  cors_origins = ["http://localhost:3000"]
  enable_https = false
  
  [scraper]
  max_concurrent_downloads = 6
  retry_attempts = 3
  batch_size = 1000
  start_on_boot = false
  
  [monitoring]
  enable_prometheus = false
  enable_grafana = false
  log_level = "info"
  EOF
  ```

- [ ] 🟡 Update `Config::load_from_file()` to use config.toml

### Day 5: Documentation (2 hours)
- [ ] 📝 Update README.md with current architecture
  - Add link to ARCHITECTURE_GUIDE.md
  - Add "Getting Started in 5 Minutes" section
  - Document all environment variables
  - Add troubleshooting section
  
- [ ] 📝 Create DEPLOYMENT.md
  - Docker deployment steps
  - Environment setup checklist
  - Security hardening guide
  - Monitoring setup

### Day 6: Testing & Validation (3 hours)
- [ ] 🧪 Add integration tests
  ```bash
  # Create tests/integration_test.rs
  - Full API flow (login → start scraper → check status)
  - Database connection pooling
  - Error handling for invalid inputs
  ```
  
- [ ] 🧪 Run comprehensive linting
  ```bash
  cargo clippy --all-targets --all-features -- \
    -W clippy::unwrap_used \
    -W clippy::expect_used \
    -W clippy::panic \
    -W clippy::todo
  ```

---

## 📊 MEDIUM PRIORITY (Next 2 Weeks)

### Week 2: Feature Activation
- [ ] ⭐ **Activate Multi-Source System**
  ```bash
  # Add CLI commands
  cargo run -- source add --name "my_csv" --type csv --path /data/file.csv
  cargo run -- source sync --id <uuid>
  cargo run -- source list
  ```
  
- [ ] ⭐ **Activate Scanner**
  ```rust
  // In src/bin/web_server.rs startup
  app_state.activate_scanner().await?;
  ```
  
- [ ] ⭐ **Add Tree Visualization API**
  ```rust
  // In src/api/routes.rs
  .route("/api/tree/:source_id", get(get_tree))
  .route("/api/tree/:tree_id/export", get(export_tree))
  ```

### Week 2: Observability
- [ ] 📊 **Prometheus Metrics**
  ```bash
  # Add dependency
  echo 'prometheus = "0.13"' >> Cargo.toml
  ```
  - Endpoint: GET /metrics
  - Metrics: request_count, request_duration, db_pool_active, scraper_files_processed
  
- [ ] 📊 **Structured Logging**
  ```rust
  // Replace println! with tracing
  use tracing::{info, error, instrument};
  
  #[instrument(skip(self))]
  async fn process_file(&self, filename: &str) -> Result<()> {
      info!(filename = %filename, "Processing file");
      // ...
  }
  ```

- [ ] 📊 **Enhanced Health Check**
  ```json
  {
    "status": "healthy",
    "version": "0.1.0",
    "uptime_seconds": 3600,
    "database": {
      "connected": true,
      "pool_active": 5,
      "pool_idle": 15
    },
    "scraper": {
      "running": true,
      "files_processed": 1234
    },
    "memory_mb": 2048
  }
  ```

---

## 🚀 LONGER TERM (This Month)

### Week 3: Performance Optimization
- [ ] ⚡ Add database indexes
  ```sql
  CREATE INDEX CONCURRENTLY idx_events_created_at 
    ON github_events(created_at DESC);
  CREATE INDEX CONCURRENTLY idx_events_type 
    ON github_events(event_type);
  ```
  
- [ ] ⚡ Tune connection pool
  ```rust
  // Based on load testing results
  max_connections: 50,
  min_connections: 10,
  ```
  
- [ ] ⚡ Add Redis caching (optional)
  - Cache schema introspection results
  - Cache frequently accessed API responses

### Week 4: Code Quality
- [ ] 🧹 Refactor large modules
  - Split `src/schema/mod.rs` (800+ lines)
  - Separate tests into `tests/` directory
  - Extract common HTTP client patterns
  
- [ ] 🧹 Increase test coverage
  ```bash
  # Target: 80% coverage
  cargo tarpaulin --out Html
  ```
  
- [ ] 🧹 Add missing documentation
  ```rust
  // All public APIs should have doc comments
  /// Processes a GitHub Archive file
  ///
  /// # Arguments
  /// * `filename` - The archive filename (e.g., "2024-01-01-0.json.gz")
  ///
  /// # Returns
  /// Processing result with event counts and duration
  pub async fn process_file(&self, filename: &str) -> Result<ProcessingResult>
  ```

---

## 🎯 Quick Reference Commands

### Build & Run
```bash
# Clean build
cargo clean && cargo build --release

# Run web server
cargo run --release --bin web_server

# Run with specific config
CONFIG_FILE=config.toml cargo run --release --bin web_server
```

### Database
```bash
# Connect to database
psql postgresql://github_archiver:github_archiver_password@localhost:5432/github_archiver

# Check table sizes
SELECT 
    relname AS table_name,
    pg_size_pretty(pg_total_relation_size(relid)) AS total_size
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC;

# Vacuum analyze
VACUUM ANALYZE;
```

### Docker
```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Restart database
docker-compose restart postgres

# Clean rebuild
docker-compose down -v && docker-compose up -d
```

### Testing
```bash
# All tests
cargo test --all

# Specific module
cargo test --package github_archiver --lib schema::tests

# With output
cargo test -- --nocapture

# Integration tests only
cargo test --test '*'
```

### API Testing
```bash
# Health check
curl http://localhost:8081/health

# Login
TOKEN=$(curl -s -X POST http://localhost:8081/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' \
  | jq -r '.token')

# Use token
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8081/api/status
```

---

## 📈 Progress Tracking

### Completion Metrics
- [ ] Compile Warnings: 2 → 0 (✅ DONE)
- [ ] Critical Issues: 7 → 0
- [ ] Security Concerns: 5 → 0
- [ ] Test Coverage: ~60% → 80%
- [ ] Documentation: 40% → 90%

### Weekly Goals
**Week 1:** Fix all critical issues + security hardening  
**Week 2:** Activate dormant features + observability  
**Week 3:** Performance optimization + refactoring  
**Week 4:** Documentation + final polish

---

## 🆘 Troubleshooting

### Common Issues

**Database connection fails:**
```bash
# Check if postgres is running
docker-compose ps postgres

# Check credentials
echo $DB_PASSWORD

# Test connection
psql postgresql://github_archiver:github_archiver_password@localhost:5432/github_archiver
```

**API server won't start:**
```bash
# Check if port is in use
lsof -i :8081

# Kill existing process
pkill -f web_server

# Check logs
tail -f logs/api.log
```

**Scraper not processing files:**
```bash
# Check GitHub token
echo $GITHUB_TOKEN

# Verify rate limits
curl -H "Authorization: token $GITHUB_TOKEN" \
  https://api.github.com/rate_limit

# Manual test
cargo run -- hunt --file 2024-01-01-0.json.gz
```

---

## 📞 Getting Help

- **Architecture questions:** See `ARCHITECTURE_GUIDE.md`
- **Bug reports:** Check `CODE_ANALYSIS_REPORT.md`
- **Deployment:** See `DEPLOYMENT.md` (to be created)
- **API reference:** See `docs/api/` (to be created)

---

**Last Updated:** October 4, 2025  
**Next Review:** Weekly
