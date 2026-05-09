# Authenticated Load Test - Quick Start

## 🎯 What You Have Now

**3 New Files Created**:
1. ✅ **authenticated-load-test.js** - The k6 test script (290 lines)
2. ✅ **setup_load_test_users.sh** - Creates test user accounts
3. ✅ **verify_audit_logs.sh** - Verifies audit logs after test
4. ✅ **AUTHENTICATED_LOAD_TEST_GUIDE.md** - Complete documentation

## 🚀 Quick Run (5 Steps)

### 1. Ensure Server is Running
```bash
# Check if server is up
curl http://localhost:3000/ping

# If not running:
cd /home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver
WEB_PORT=3000 RUST_LOG=info ./target/release/examples/api_server > api_server.log 2>&1 &
```

### 2. Create Test Users
```bash
cd /home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver
./setup_load_test_users.sh
```

### 3. Verify Login Works
```bash
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"loadtest_user1","password":"LoadTest123!"}'
```

**Expected**: Should return a JWT token

### 4. Run the Test
```bash
BASE_URL=http://localhost:3000 k6 run tests/load/authenticated-load-test.js
```

**Duration**: 5 minutes  
**Expected**: ~10,000-16,000 iterations, ~40,000-64,000 audit logs

### 5. Verify Results
```bash
./verify_audit_logs.sh
```

**Expected**: Should show 4× the number of iterations in audit logs

---

## 📊 What Gets Tested

Each iteration performs:
1. **Login** → Audit log: `UserLogin`
2. **Start Scraper** → Audit log: `ScraperStart`
3. **Check Status** → No audit log (read-only)
4. **Stop Scraper** → Audit log: `ScraperStop`
5. **Logout** → Audit log: `UserLogout`

**Result**: 4 audit logs per iteration

---

## ✅ Success Criteria

**Test passes if**:
- ✅ Login p95 < 1000ms
- ✅ Scraper operations p95 < 2000ms
- ✅ Overall p95 < 3000ms
- ✅ Failure rate < 10%
- ✅ Audit logs match iterations × 4 (±5%)
- ✅ No server crashes

---

## 🔧 Troubleshooting

### Login Fails (401)
→ Test users not created or wrong password  
→ Run `./setup_load_test_users.sh` again

### High Failure Rate (>50%)
→ Rate limiting triggered  
→ Expected behavior, similar to public endpoint test

### No Audit Logs
→ Check `api_server.log` for errors  
→ Verify audit_logs table exists in database

### Server Crashes
→ Check logs: `tail -100 api_server.log`  
→ May need to increase database connection pool

---

## 📈 Expected Performance

Based on public endpoint baseline:

| Metric | Public | Authenticated (Expected) |
|--------|--------|--------------------------|
| **p95** | 0.63ms | 500-1500ms |
| **p99** | 0.82ms | 1000-3000ms |
| **Throughput** | 53.7 req/s | 30-50 req/s |
| **Audit overhead** | N/A | 50-200ms per write |

---

## 📁 Full Documentation

See **AUTHENTICATED_LOAD_TEST_GUIDE.md** for:
- Detailed step-by-step instructions
- Troubleshooting guide
- Performance analysis methods
- Database metrics
- Cleanup procedures

---

## 🎯 Why This Matters

**Public endpoint test** showed the server is fast (sub-millisecond).  
**Authenticated test** validates audit logging doesn't slow it down.

**This completes Session 7**: Audit integration + performance validation under load! 🎉

---

**Ready to run?** Just follow the 5 steps above! ⬆️
