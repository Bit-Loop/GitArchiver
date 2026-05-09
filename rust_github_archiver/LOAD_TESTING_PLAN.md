# Load Testing Plan & Setup

**Date**: October 13, 2025  
**Status**: Test Suite Created - Compilation Errors Need Resolution  
**Priority**: HIGH

---

## 🎯 Objective

Validate that the GitHub Archiver API can handle the target performance requirements:
- **Target Throughput**: 10,000 events/second
- **p95 Latency**: < 500ms under normal load
- **p99 Latency**: < 1s under normal load
- **Error Rate**: < 1% under normal load
- **Availability**: 99.9% uptime

---

## 📦 Test Suite Created

### 1. Smoke Test (`tests/load/smoke-test.js`)
**Purpose**: Quick validation that the system is working  
**Duration**: 30 seconds  
**Virtual Users**: 10  
**Endpoints**: Health checks (`/health/live`)  
**Success Criteria**:
- All requests return 200 OK
- p95 latency < 200ms
- p99 latency < 500ms
- Error rate < 1%

**Command**:
```bash
cd rust_github_archiver
/var/lib/snapd/snap/bin/k6 run tests/load/smoke-test.js
```

### 2. Load Test (`tests/load/load-test-simple.js`)
**Purpose**: Measure performance under expected production load  
**Duration**: 5 minutes  
**Virtual Users**: 0 → 100 (ramp 1m) → 100 (hold 3m) → 0 (ramp 1m)  
**Endpoints**: Mixed workload
- 60% health checks (`/health/live`)
- 25% readiness checks (`/health/ready`)
- 10% full health (`/health`)
- 5% metrics (`/metrics`)

**Success Criteria**:
- p95 latency < 500ms
- p99 latency < 1s
- Error rate < 5%
- Throughput > 100 req/s

**Command**:
```bash
cd rust_github_archiver
/var/lib/snapd/snap/bin/k6 run tests/load/load-test-simple.js
```

### 3. Stress Test (`tests/load/stress-test.js`)
**Purpose**: Find system limits and identify bottlenecks  
**Duration**: 10 minutes  
**Virtual Users**: 0 → 500 (ramp 2m) → 500 (hold 6m) → 0 (ramp 2m)  
**Endpoints**: Aggressive mixed workload
- 50% health checks
- 30% readiness checks
- 15% full health
- 5% metrics

**Success Criteria**:
- p95 latency < 1s
- p99 latency < 2s
- Error rate < 10% (acceptable under stress)
- Identify breaking point
- System remains stable

**Command**:
```bash
cd rust_github_archiver
/var/lib/snapd/snap/bin/k6 run tests/load/stress-test.js
```

### 4. Comprehensive Test (`tests/load/load-test.js`)
**Purpose**: Full 46-minute test with multiple stages  
**Duration**: 46 minutes  
**Stages**:
- Warm-up: 2m → 10 VUs
- Normal load: 5m ramp to 100, hold 10m
- Peak load: 5m ramp to 500, hold 10m
- Spike: 2m ramp to 1000, hold 3m
- Cool down: 5m ramp to 100, 2m to 0

**Command**:
```bash
cd rust_github_archiver
/var/lib/snapd/snap/bin/k6 run tests/load/load-test.js
```

---

## 🛠️ Setup Requirements

### ✅ Completed
- [x] k6 installed (v1.3.0 via snap)
- [x] Smoke test script created
- [x] Load test script created
- [x] Stress test script created
- [x] Test scripts include custom metrics and reporting

### ❌ Blocked
- [ ] **Compilation errors must be fixed first**
- [ ] API server must be built and started
- [ ] Database must be running
- [ ] Migrations must be applied

---

## 🚨 Current Blockers

### Critical Compilation Errors

#### 1. **audit.rs** - Login Handler Integration (FIXED)
- **Error**: Type mismatch in audit logging calls
- **Fix Applied**: Changed `user.id` to `Some(user.id)` and `user.id.map()` to `Some(user.id.to_string())`
- **Status**: ✅ FIXED

#### 2. **health.rs** - sysinfo API Changes
- **Error**: `SystemExt` and `DiskExt` traits not found
- **Issue**: sysinfo v0.30+ removed these traits (methods now directly on types)
- **Fix Needed**: Update all `SystemExt` and `DiskExt` imports and calls
- **Files**: `src/health.rs`
- **Priority**: HIGH

#### 3. **security.rs** - Borrow Checker Issue
- **Error**: Cannot move out of borrowed `req`
- **Issue**: Request used after borrow in middleware
- **Fix Needed**: Clone necessary parts before moving
- **Files**: `src/security.rs` line 202
- **Priority**: MEDIUM

#### 4. **circuit_breaker.rs** - Result Type Mismatches in Tests
- **Error**: Expected `Result<T, String>`, found `Result<T, anyhow::Error>`
- **Issue**: Test expectations don't match actual return types
- **Fix Needed**: Update test type annotations
- **Files**: `src/circuit_breaker.rs` lines 229, 246, 272, 280, 286, 303
- **Priority**: LOW (tests only)

#### 5. **Unused Imports** (Warnings)
- Multiple files with unused imports
- Can be cleaned up but not blocking
- **Priority**: LOW

---

## 📋 Pre-Test Checklist

Before running load tests, ensure:

1. **Fix Compilation Errors**
   ```bash
   cd rust_github_archiver
   cargo build --release
   ```

2. **Start Database**
   ```bash
   # Ensure PostgreSQL is running
   sudo systemctl start postgresql
   # Or if using Docker:
   docker-compose up -d postgres
   ```

3. **Apply Migrations**
   ```bash
   sqlx migrate run
   ```

4. **Start API Server**
   ```bash
   ./run.sh
   # Or:
   cargo run --release
   ```

5. **Verify Health**
   ```bash
   curl http://localhost:8081/health/live
   # Expected: {"status":"ok"}
   ```

---

## 🎬 Running the Tests

### Sequential Approach (Recommended)

```bash
# 1. Smoke test (30s) - verify basics
/var/lib/snapd/snap/bin/k6 run tests/load/smoke-test.js

# 2. Load test (5m) - measure normal performance
/var/lib/snapd/snap/bin/k6 run tests/load/load-test-simple.js

# 3. Stress test (10m) - find limits
/var/lib/snapd/snap/bin/k6 run tests/load/stress-test.js

# 4. Comprehensive test (46m) - full validation (optional)
/var/lib/snapd/snap/bin/k6 run tests/load/load-test.js
```

### With Custom Configuration

```bash
# Override base URL
BASE_URL=http://api.example.com:8081 k6 run tests/load/smoke-test.js

# With API authentication
API_TOKEN=your_token_here BASE_URL=https://api.example.com k6 run tests/load/load-test-simple.js
```

---

## 📊 Expected Output

Each test will display:

```
=== SMOKE TEST RESULTS ===

Total Requests: 300
Requests/sec: 10.00

Response Times:
  Avg: 45.23ms
  Min: 12.45ms
  Med: 42.10ms
  Max: 98.67ms
  p(95): 78.34ms
  p(99): 92.11ms

Success Rate: 100.00%

=========================
```

---

## 📈 Performance Metrics to Capture

### Primary Metrics
- **Throughput**: Requests per second (target: > 100 req/s normal, > 1000 req/s peak)
- **Latency**: p50, p95, p99 response times
- **Error Rate**: Percentage of failed requests
- **Success Rate**: Percentage of successful requests

### Secondary Metrics
- **HTTP Request Duration**: Full request lifecycle timing
- **Connection Time**: Time to establish connection
- **TLS Handshake**: SSL/TLS negotiation time (if HTTPS)
- **Time to First Byte (TTFB)**: Server processing time
- **Data Transfer**: Upload/download throughput

### System Metrics (to monitor separately)
- **CPU Usage**: % per core
- **Memory Usage**: RSS, heap, allocations
- **Network I/O**: Bytes in/out, connections
- **Disk I/O**: Read/write operations
- **Database Connections**: Active, idle, max

---

## 🐛 Troubleshooting

### k6 not found
```bash
# Add snap bin to PATH
export PATH=$PATH:/var/lib/snapd/snap/bin

# Or use full path
/var/lib/snapd/snap/bin/k6 version
```

### Server not responding
```bash
# Check if server is running
curl http://localhost:8081/health/live

# Check logs
tail -f logs/service.log

# Check port is open
netstat -tlnp | grep 8081
```

### High error rates
- Check server logs for errors
- Verify database is running and responsive
- Check system resources (CPU, memory, disk)
- Reduce VU count if overwhelming system
- Increase timeouts if network is slow

### OOM (Out of Memory)
- Reduce VU count
- Add resource limits to k6
- Check for memory leaks in application
- Monitor with `htop` or `top`

---

## 📝 Next Steps

1. **Fix compilation errors** (2-3 hours)
   - Update sysinfo usage
   - Fix security middleware borrow
   - Fix circuit breaker tests

2. **Build and start server** (5 minutes)
   ```bash
   cargo build --release
   ./run.sh
   ```

3. **Run smoke test** (1 minute)
   - Verify basic functionality

4. **Run load test** (5 minutes)
   - Measure baseline performance

5. **Run stress test** (10 minutes)
   - Find breaking points

6. **Analyze results** (30 minutes)
   - Document baselines
   - Identify bottlenecks
   - Create optimization plan

7. **Update PROGRESS.md** (15 minutes)
   - Add performance baselines
   - Update Phase 5 completion

**Total Estimated Time**: 4-5 hours

---

## 🎯 Success Criteria

Load testing is **COMPLETE** when:
- [x] k6 installed and verified
- [x] Test scripts created (smoke, load, stress)
- [ ] Compilation errors fixed
- [ ] Server builds successfully
- [ ] Smoke test passes (100% success)
- [ ] Load test completes with < 5% errors
- [ ] Stress test identifies system limits
- [ ] Performance baselines documented
- [ ] Bottlenecks identified
- [ ] Optimization recommendations created

**Current Status**: 3/10 complete (30%)

---

**Created**: October 13, 2025  
**Last Updated**: October 13, 2025  
**Next Session**: Fix compilation errors, then execute tests
