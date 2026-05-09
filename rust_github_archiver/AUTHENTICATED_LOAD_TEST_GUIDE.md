# Authenticated Load Test - Complete Guide

## Overview

This authenticated load test validates that audit logging performs well under production-like load conditions. It simulates real users performing authenticated operations that trigger audit log writes.

## What Gets Tested

### Endpoints Hit Per Iteration
1. **POST /api/auth/login** - Creates `UserLogin` audit log
2. **POST /api/scraper/start** - Creates `ScraperStart` audit log
3. **GET /api/scraper/status** - Read-only, no audit log
4. **POST /api/scraper/stop** - Creates `ScraperStop` audit log
5. **POST /api/auth/logout** - Creates `UserLogout` audit log

**Result**: 4 audit log entries per iteration

### Load Profile
- **Duration**: 5 minutes
- **Virtual Users**: 0 → 100 (ramped over 4 minutes)
- **Expected Iterations**: ~10,000-16,000 (depending on response times)
- **Expected Audit Logs**: ~40,000-64,000 entries

### Performance Thresholds
- Login p95 < 1000ms
- Scraper operations p95 < 2000ms
- Overall HTTP p95 < 3000ms
- Failure rate < 10%

## Prerequisites

### 1. Server Running
```bash
# Make sure API server is running on port 3000
ps aux | grep api_server

# If not running:
cd /home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver
WEB_PORT=3000 RUST_LOG=info ./target/release/examples/api_server > api_server.log 2>&1 &
```

### 2. Database Access
Ensure PostgreSQL is running and accessible:
```bash
# Test connection
psql -h localhost -U postgres -d github_archiver -c "SELECT COUNT(*) FROM audit_logs;"
```

### 3. Test Users Created
The test requires 5 test user accounts. These will be created automatically by the setup script.

## Step-by-Step Instructions

### Step 1: Create Test Users

Run the setup script to create test user accounts:

```bash
cd /home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver
./setup_load_test_users.sh
```

**Expected Output**:
```
=== Authenticated Load Test Setup ===
Creating test users...
  - loadtest_user1... ✅ Created
  - loadtest_user2... ✅ Created
  - loadtest_user3... ✅ Created
  - loadtest_user4... ✅ Created
  - loadtest_user5... ✅ Created
```

**Troubleshooting**:
- If API registration endpoint doesn't exist, you'll need to create users manually
- Check `src/api/handlers.rs` for a `register` or `create_user` endpoint
- Alternative: Manually insert users with hashed passwords in the database

### Step 2: Verify Test User Can Login

Test that authentication works before running the full load test:

```bash
# Test login for first user
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"loadtest_user1","password":"LoadTest123!"}'
```

**Expected Response**:
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
  "user": {
    "username": "loadtest_user1",
    "role": "user"
  }
}
```

**Troubleshooting**:
- If login fails, check password hashing in database
- Verify users table has the test users
- Check server logs for authentication errors

### Step 3: Clear Old Audit Logs (Optional)

If you want a clean baseline, clear old audit logs:

```bash
# WARNING: This deletes audit logs! Only use in testing!
psql -h localhost -U postgres -d github_archiver -c "DELETE FROM audit_logs WHERE username LIKE 'loadtest_%';"
```

### Step 4: Run the Authenticated Load Test

Execute the test with k6:

```bash
cd /home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver
BASE_URL=http://localhost:3000 k6 run tests/load/authenticated-load-test.js
```

**What to Watch**:
```
# Monitor server logs in another terminal
tail -f api_server.log

# Monitor database connections
watch -n 1 "psql -h localhost -U postgres -d github_archiver -c \"SELECT count(*) FROM pg_stat_activity WHERE datname='github_archiver';\""
```

**Expected Duration**: ~5 minutes

**Live Output**:
```
running (1m30s), 050/100 VUs, 1500 complete and 0 interrupted iterations
default   [  30% ] 050/100 VUs  1m30s/5m00s
```

### Step 5: Review Test Results

Once complete, k6 will display a summary:

```
=== AUTHENTICATED LOAD TEST RESULTS ===
Total Iterations: 14,200
Expected Audit Logs: 56,800
Requests/sec: 47.33
Duration: 300.0s

Authentication Performance:
  Login p95: 425.50ms
  Login p99: 890.20ms
  Login avg: 185.30ms
  Logout p95: 150.20ms
  Auth failure rate: 0.50%

Scraper Operations:
  Start p95: 1250.40ms
  Start avg: 580.20ms
  Stop p95: 1180.30ms
  Stop avg: 520.15ms
  Operation failure rate: 2.30%

Overall Performance:
  HTTP p95: 1420.50ms
  HTTP p99: 2850.30ms
  HTTP avg: 620.40ms
  HTTP failure rate: 3.50%

Audit Logging:
  Expected audit log writes: 56,800
  Audit writes/sec: 189.33
```

### Step 6: Verify Audit Logs in Database

Run the verification script to confirm audit logs were created:

```bash
./verify_audit_logs.sh
```

**Expected Output**:
```
=== Audit Log Verification ===

1. Recent Audit Logs (last 10 minutes):
   Total: 56,800

2. Audit Logs by Action:
   UserLogin     | 14,200
   ScraperStart  | 14,200
   ScraperStop   | 14,200
   UserLogout    | 14,200

3. Success vs Failure:
   true  | 55,000 | 96.83%
   false | 1,800  | 3.17%

4. Top Users (Load Test Users):
   loadtest_user1 | 11,360
   loadtest_user2 | 11,440
   loadtest_user3 | 11,280
   loadtest_user4 | 11,360
   loadtest_user5 | 11,360
```

### Step 7: Analyze Performance Impact

Compare results with the public endpoint load test:

| Metric | Public Endpoints | Authenticated Endpoints | Overhead |
|--------|------------------|------------------------|----------|
| **p95 Latency** | 0.63ms | ~1,420ms | +1,419ms |
| **p99 Latency** | 0.82ms | ~2,850ms | +2,849ms |
| **Throughput** | 53.7 req/s | 47.3 req/s | -11.9% |
| **Failure Rate** | 63% (rate limits) | 3.5% | Better! |

**Analysis**:
- Authenticated endpoints are slower (expected - DB writes, auth verification)
- Audit logging overhead is part of the authenticated endpoint latency
- Still well within acceptable ranges (p95 < 2s for most operations)

### Step 8: Database Performance Analysis

Check database metrics:

```bash
# Query performance during test
psql -h localhost -U postgres -d github_archiver -c "
  SELECT 
    query,
    calls,
    mean_exec_time,
    max_exec_time
  FROM pg_stat_statements 
  WHERE query LIKE '%audit_logs%'
  ORDER BY mean_exec_time DESC
  LIMIT 10;
"

# Connection pool usage
psql -h localhost -U postgres -d github_archiver -c "
  SELECT 
    state,
    COUNT(*) 
  FROM pg_stat_activity 
  WHERE datname = 'github_archiver'
  GROUP BY state;
"
```

## Success Criteria

### ✅ Test Passes If:
1. **All thresholds met**:
   - Login p95 < 1000ms ✅
   - Scraper ops p95 < 2000ms ✅
   - Overall p95 < 3000ms ✅
   - Failure rate < 10% ✅

2. **Audit logs created**:
   - Count matches iterations × 4 (±5%)
   - All 4 action types present
   - Success rate > 95%

3. **No server crashes**:
   - API server still running after test
   - No panic/fatal errors in logs
   - Database connections stable

4. **Performance acceptable**:
   - p95 latency < 2 seconds for most operations
   - Throughput > 30 req/sec
   - Audit write latency < 100ms

### ⚠️ Investigation Needed If:
- Failure rate > 10% (check server logs)
- Audit log count doesn't match iterations
- p95 latency > 5 seconds (performance issue)
- Database connection errors
- Server crash or OOM

## Cleanup

After testing, you can remove test users and audit logs:

```bash
# Remove test users
psql -h localhost -U postgres -d github_archiver -c "
  DELETE FROM users WHERE username LIKE 'loadtest_%';
"

# Remove test audit logs
psql -h localhost -U postgres -d github_archiver -c "
  DELETE FROM audit_logs WHERE username LIKE 'loadtest_%';
"

# Or keep them for historical analysis
```

## Troubleshooting

### Issue: "Login failed: 401 Unauthorized"
**Cause**: Test users not created or wrong password
**Solution**: 
```bash
# Verify user exists
psql -h localhost -U postgres -d github_archiver -c "SELECT username, created_at FROM users WHERE username LIKE 'loadtest_%';"

# Recreate users
./setup_load_test_users.sh
```

### Issue: "High failure rate (>50%)"
**Cause**: Rate limiting kicking in
**Solution**:
```bash
# Check rate limiter configuration
# Option 1: Increase rate limits temporarily for testing
# Option 2: Whitelist localhost for load testing
# Option 3: Accept higher failure rate in test thresholds
```

### Issue: "No audit logs created"
**Cause**: Audit logging not triggered or database error
**Solution**:
```bash
# Check server logs for audit errors
grep -i "audit" api_server.log | tail -20

# Test audit logging manually
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"loadtest_user1","password":"LoadTest123!"}'

# Check if audit log was created
psql -h localhost -U postgres -d github_archiver -c "SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 1;"
```

### Issue: "Database connection pool exhausted"
**Cause**: Too many concurrent connections
**Solution**:
```bash
# Check current pool configuration
grep -i "max.*pool\|pool.*size" src/database.rs

# Increase pool size if needed (typically 20-50)
# Or reduce VUs in test
```

## Expected Results Documentation

After running the test, document results in:
- `SESSION_7_AUTHENTICATED_LOAD_TEST_RESULTS.md`

Include:
- Full k6 output
- Database verification results
- Performance comparison with public endpoints
- Database metrics during test
- Recommendations for production

## Next Steps

After successful authenticated load test:
1. ✅ Validate audit logging works under load
2. ✅ Measure actual performance impact
3. ✅ Identify any bottlenecks
4. Update production configuration based on findings
5. Set up monitoring for audit log performance in production

---

**Test Created**: October 14, 2025  
**Purpose**: Validate audit logging performance under production-like load  
**Expected Duration**: 5 minutes test + 10 minutes setup/analysis = ~15 minutes total
