# Testing Guide - Monitoring System

## Overview

This guide provides comprehensive testing instructions for the GitHub Archiver Monitoring System.

---

## Quick Test Commands

### 0. Default Engineering Gate

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

### Operator Runtime And Shutdown Smoke Coverage

```bash
# Runtime lifecycle and operator workflow smoke
cargo test operator::runtime::tests::operator_workflow_smoke_covers_login_runtime_and_findings_review --lib

# Deterministic scan pause/resume and shutdown behavior
cargo test scanning::tests::pause_gate_waits_until_resumed --lib
cargo test scanning::tests::shutdown_gate_cancels_active_scan --lib

# Grouped CLI command parsing and dependency gating
cargo test operator::cli::tests:: --lib
```

### 1. Test API Endpoints

```bash
# Set base URL
API_URL="http://localhost:8081"

# Get JWT token (replace with your auth endpoint)
TOKEN=$(curl -s -X POST "$API_URL/api/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"your_password"}' | jq -r '.token')

# Use an admin token for user/database/audit routes.
# Use an operator token for scraper, scanner, and realtime control routes.
# Use a read_only token for dashboard, findings, and log inspection routes.
# Compatibility aliases still load: user -> operator, viewer -> read_only.

# Test Overview Endpoint
curl -H "Authorization: Bearer $TOKEN" "$API_URL/api/monitoring/overview" | jq

# Test Trends Endpoint (24h)
curl -H "Authorization: Bearer $TOKEN" "$API_URL/api/monitoring/trends?period=24h" | jq

# Test Logs Endpoint
curl -H "Authorization: Bearer $TOKEN" "$API_URL/api/monitoring/logs?page=1&page_size=10" | jq

# Test Metrics Endpoint (public)
curl "$API_URL/api/monitoring/metrics" | jq

# Export Logs to CSV
curl -H "Authorization: Bearer $TOKEN" "$API_URL/api/monitoring/logs/export" -o logs.csv
```

### 2. Test WebSocket Connection

Using `websocat`:

```bash
# Install websocat
cargo install websocat

# Connect to WebSocket
websocat ws://localhost:8081/api/monitoring/ws

# You should see JSON messages every second
```

Using `wscat`:

```bash
# Install wscat
npm install -g wscat

# Connect
wscat -c ws://localhost:8081/api/monitoring/ws
```

### 3. Test Dashboard

```bash
# Open dashboard in browser
xdg-open http://localhost:8081/monitoring-dashboard.html

# Or using curl to check it loads
curl -I http://localhost:8081/monitoring-dashboard.html
```

---

## Integration Tests

### Test Suite 1: API Response Validation

```bash
#!/bin/bash

API_URL="http://localhost:8081"
TOKEN="your_jwt_token"

echo "Testing Monitoring API Endpoints..."

# Test 1: Overview endpoint structure
echo "1. Testing Overview..."
OVERVIEW=$(curl -s -H "Authorization: Bearer $TOKEN" "$API_URL/api/monitoring/overview")
echo "$OVERVIEW" | jq -e '.total_secrets' > /dev/null && echo "✓ Overview OK" || echo "✗ Overview FAILED"

# Test 2: Trends endpoint
echo "2. Testing Trends..."
TRENDS=$(curl -s -H "Authorization: Bearer $TOKEN" "$API_URL/api/monitoring/trends?period=24h")
echo "$TRENDS" | jq -e '.total_detections_trend' > /dev/null && echo "✓ Trends OK" || echo "✗ Trends FAILED"

# Test 3: Logs endpoint
echo "3. Testing Logs..."
LOGS=$(curl -s -H "Authorization: Bearer $TOKEN" "$API_URL/api/monitoring/logs")
echo "$LOGS" | jq -e '.logs' > /dev/null && echo "✓ Logs OK" || echo "✗ Logs FAILED"

# Test 4: Metrics endpoint
echo "4. Testing Metrics..."
METRICS=$(curl -s "$API_URL/api/monitoring/metrics")
echo "$METRICS" | jq -e '.cpu_usage' > /dev/null && echo "✓ Metrics OK" || echo "✗ Metrics FAILED"

echo "All tests completed!"
```

### Test Suite 2: Performance Testing

```bash
#!/bin/bash

API_URL="http://localhost:8081"

# Test response times
echo "Performance Testing..."

# Test 1: Overview response time
TIME1=$(curl -w "%{time_total}\n" -o /dev/null -s "$API_URL/api/monitoring/metrics")
echo "Metrics response time: ${TIME1}s"

# Test 2: Concurrent requests
echo "Running 100 concurrent requests..."
seq 1 100 | xargs -P 10 -I {} curl -s "$API_URL/api/monitoring/metrics" > /dev/null
echo "✓ Concurrent test completed"

# Test 3: WebSocket connections
echo "Testing multiple WebSocket connections..."
for i in {1..5}; do
    websocat ws://localhost:8081/api/monitoring/ws > /dev/null 2>&1 &
done
sleep 5
killall websocat 2>/dev/null
echo "✓ WebSocket test completed"
```

---

## Frontend Testing

### Manual Test Checklist

#### Overview Tab
- [ ] Total secrets count displays correctly
- [ ] Critical count shows in red
- [ ] Success rate percentage is accurate
- [ ] Severity chart renders with correct colors
- [ ] Category chart shows all categories
- [ ] Top repositories list populates
- [ ] Recent detections appear with timestamps
- [ ] Cards have hover effects

#### Trends Tab
- [ ] Time period buttons work (24h, 7d, 30d, 90d)
- [ ] Trend direction indicator shows (up/down/stable)
- [ ] Growth rate percentage displays
- [ ] Detections trend chart renders
- [ ] Severity trends chart shows all levels
- [ ] Chart updates when period changes
- [ ] Smooth animations on data updates

#### Logs Tab
- [ ] Log level filter works (ERROR, WARN, INFO, DEBUG)
- [ ] Category filter functions correctly
- [ ] Search box filters logs
- [ ] Pagination controls work
- [ ] Log counts update with filters
- [ ] Export button downloads CSV
- [ ] Color coding by log level works

#### Real-Time Tab
- [ ] CPU usage updates every second
- [ ] Memory usage displays correctly
- [ ] Progress bars animate
- [ ] Active scans counter works
- [ ] WebSocket connections shown
- [ ] Live metrics chart updates
- [ ] Activity feed receives updates
- [ ] Feed auto-scrolls

#### General UI
- [ ] Connection status shows (connected/disconnected)
- [ ] Pulse dot animates when connected
- [ ] Refresh button reloads data
- [ ] Tab switching works smoothly
- [ ] Loading overlay appears during data fetch
- [ ] Toast notifications show for errors/success
- [ ] Responsive on mobile devices
- [ ] All icons load correctly

### Automated UI Tests (Selenium)

```python
# test_dashboard.py
from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
import time

def test_dashboard():
    driver = webdriver.Chrome()
    driver.get("http://localhost:8081/monitoring-dashboard.html")
    
    # Test 1: Page loads
    assert "Monitoring" in driver.title
    
    # Test 2: Connection status
    status = WebDriverWait(driver, 10).until(
        EC.presence_of_element_located((By.ID, "connection-status"))
    )
    assert status.text in ["Connected", "Disconnected"]
    
    # Test 3: Tab switching
    trends_tab = driver.find_element(By.CSS_SELECTOR, '[data-tab="trends"]')
    trends_tab.click()
    time.sleep(1)
    assert driver.find_element(By.ID, "trends-tab").is_displayed()
    
    # Test 4: Period selection
    period_btn = driver.find_element(By.CSS_SELECTOR, '[data-period="7d"]')
    period_btn.click()
    time.sleep(2)
    
    # Test 5: Logs tab
    logs_tab = driver.find_element(By.CSS_SELECTOR, '[data-tab="logs"]')
    logs_tab.click()
    time.sleep(1)
    
    # Test 6: Filter logs
    level_filter = driver.find_element(By.ID, "log-level-filter")
    level_filter.send_keys("ERROR")
    search_btn = driver.find_element(By.CSS_SELECTOR, 'button[onclick="searchLogs()"]')
    search_btn.click()
    time.sleep(2)
    
    driver.quit()
    print("✓ All UI tests passed!")

if __name__ == "__main__":
    test_dashboard()
```

---

## WebSocket Testing

### Test Script (JavaScript)

```javascript
// test_websocket.js
const WebSocket = require('ws');

const WS_URL = 'ws://localhost:8081/api/monitoring/ws';
let messageCount = 0;
let startTime = Date.now();

console.log('Connecting to WebSocket...');

const ws = new WebSocket(WS_URL);

ws.on('open', () => {
    console.log('✓ Connected successfully');
});

ws.on('message', (data) => {
    messageCount++;
    const metrics = JSON.parse(data);
    
    console.log(`Message ${messageCount}:`, {
        timestamp: metrics.timestamp,
        cpu: metrics.cpu_usage,
        memory: metrics.memory_usage_mb,
        scans: metrics.active_scans
    });
    
    // Validate data structure
    if (!metrics.timestamp || metrics.cpu_usage === undefined) {
        console.error('✗ Invalid message structure');
        process.exit(1);
    }
    
    // Test for 10 seconds
    if (Date.now() - startTime > 10000) {
        console.log(`\n✓ Received ${messageCount} messages in 10s`);
        console.log(`✓ Average: ${messageCount/10} msg/sec`);
        ws.close();
        process.exit(0);
    }
});

ws.on('error', (error) => {
    console.error('✗ WebSocket error:', error.message);
    process.exit(1);
});

ws.on('close', () => {
    console.log('Connection closed');
});

// Run: node test_websocket.js
```

### Test Reconnection

```javascript
// test_reconnection.js
const WebSocket = require('ws');

let ws;
let reconnectCount = 0;

function connect() {
    ws = new WebSocket('ws://localhost:8081/api/monitoring/ws');
    
    ws.on('open', () => {
        console.log('✓ Connected');
    });
    
    ws.on('close', () => {
        reconnectCount++;
        console.log(`Reconnecting... (attempt ${reconnectCount})`);
        setTimeout(connect, 5000);
    });
    
    ws.on('error', (error) => {
        console.error('Error:', error.message);
    });
}

connect();

// Test: Restart the server while this runs
// Should auto-reconnect
```

---

## Load Testing

### Apache Bench (ab)

```bash
# Install Apache Bench
sudo apt-get install apache2-utils

# Test metrics endpoint
ab -n 1000 -c 10 http://localhost:8081/api/monitoring/metrics

# Test with authentication
ab -n 1000 -c 10 -H "Authorization: Bearer YOUR_TOKEN" \
   http://localhost:8081/api/monitoring/overview
```

### wrk (Advanced)

```bash
# Install wrk
sudo apt-get install wrk

# Basic test
wrk -t4 -c100 -d30s http://localhost:8081/api/monitoring/metrics

# With auth header
wrk -t4 -c100 -d30s -H "Authorization: Bearer TOKEN" \
    http://localhost:8081/api/monitoring/overview
```

### k6 Load Testing

```javascript
// load_test.js
import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
    stages: [
        { duration: '30s', target: 20 },  // Ramp up
        { duration: '1m', target: 50 },   // Stay at 50
        { duration: '30s', target: 0 },   // Ramp down
    ],
};

export default function () {
    const res = http.get('http://localhost:8081/api/monitoring/metrics');
    
    check(res, {
        'status is 200': (r) => r.status === 200,
        'response time < 200ms': (r) => r.timings.duration < 200,
        'has cpu_usage': (r) => r.json().cpu_usage !== undefined,
    });
    
    sleep(1);
}

// Run: k6 run load_test.js
```

---

## Data Validation Tests

### Test Data Integrity

```rust
// tests/monitoring_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_detection_overview() {
        let overview = get_detection_overview().await;
        
        assert!(overview.total_secrets >= 0);
        assert_eq!(
            overview.total_secrets,
            overview.critical_secrets + 
            overview.high_secrets + 
            overview.medium_secrets + 
            overview.low_secrets
        );
    }
    
    #[tokio::test]
    async fn test_trends_calculation() {
        let trends = get_detection_trends("24h".to_string()).await;
        
        assert!(trends.growth_rate >= -100.0);
        assert!(trends.total_detections_trend.len() > 0);
        
        // Verify timestamps are in order
        for i in 1..trends.total_detections_trend.len() {
            assert!(
                trends.total_detections_trend[i].timestamp >= 
                trends.total_detections_trend[i-1].timestamp
            );
        }
    }
    
    #[tokio::test]
    async fn test_log_filtering() {
        let logs = get_system_logs(
            Some("ERROR".to_string()),
            None, None, None, None, None,
            1, 50
        ).await;
        
        for log in logs.logs {
            assert_eq!(log.level, "ERROR");
        }
    }
}

// Run: cargo test --test monitoring_tests
```

---

## Error Handling Tests

### Test Error Scenarios

```bash
#!/bin/bash

API_URL="http://localhost:8081"

echo "Testing Error Handling..."

# Test 1: Invalid period
echo "1. Invalid period parameter..."
curl -s "$API_URL/api/monitoring/trends?period=invalid" | jq '.error'

# Test 2: Invalid page number
echo "2. Invalid page number..."
curl -s "$API_URL/api/monitoring/logs?page=-1" | jq '.error'

# Test 3: Missing authentication
echo "3. Missing auth header..."
curl -s "$API_URL/api/monitoring/overview" | jq '.error'

# Test 4: Invalid JWT
echo "4. Invalid JWT token..."
curl -s -H "Authorization: Bearer invalid_token" \
  "$API_URL/api/monitoring/overview" | jq '.error'

echo "Error handling tests completed!"
```

---

## Monitoring Dashboard Health Check

### Comprehensive Health Script

```bash
#!/bin/bash

API_URL="http://localhost:8081"
ERRORS=0

echo "🔍 Running Monitoring System Health Check..."
echo "=============================================="

# Check 1: Server is running
echo -n "1. Server running... "
if curl -s -f "$API_URL/api/monitoring/metrics" > /dev/null; then
    echo "✓"
else
    echo "✗ FAILED"
    ((ERRORS++))
fi

# Check 2: WebSocket available
echo -n "2. WebSocket endpoint... "
if curl -s -I "$API_URL/api/monitoring/ws" | grep -q "101\\|200"; then
    echo "✓"
else
    echo "✗ FAILED"
    ((ERRORS++))
fi

# Check 3: Dashboard loads
echo -n "3. Dashboard loads... "
if curl -s -f "$API_URL/monitoring-dashboard.html" > /dev/null; then
    echo "✓"
else
    echo "✗ FAILED"
    ((ERRORS++))
fi

# Check 4: API response time
echo -n "4. Response time < 100ms... "
TIME=$(curl -w "%{time_total}" -o /dev/null -s "$API_URL/api/monitoring/metrics")
if (( $(echo "$TIME < 0.1" | bc -l) )); then
    echo "✓ (${TIME}s)"
else
    echo "✗ FAILED (${TIME}s)"
    ((ERRORS++))
fi

# Check 5: Memory usage
echo -n "5. Memory usage reasonable... "
METRICS=$(curl -s "$API_URL/api/monitoring/metrics")
MEMORY=$(echo "$METRICS" | jq -r '.memory_usage_mb')
if (( $(echo "$MEMORY < 1024" | bc -l) )); then
    echo "✓ (${MEMORY}MB)"
else
    echo "⚠ WARNING (${MEMORY}MB)"
fi

# Check 6: CPU usage
echo -n "6. CPU usage reasonable... "
CPU=$(echo "$METRICS" | jq -r '.cpu_usage')
if (( $(echo "$CPU < 80" | bc -l) )); then
    echo "✓ (${CPU}%)"
else
    echo "⚠ WARNING (${CPU}%)"
fi

echo "=============================================="
if [ $ERRORS -eq 0 ]; then
    echo "✓ All health checks passed!"
    exit 0
else
    echo "✗ $ERRORS check(s) failed"
    exit 1
fi
```

---

## Continuous Integration Tests

### GitHub Actions Workflow

```yaml
# .github/workflows/monitoring-tests.yml
name: Monitoring System Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: postgres
          POSTGRES_DB: github_archiver
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run unit tests
        run: cargo test --all-features
      
      - name: Build project
        run: cargo build --release
      
      - name: Start server
        run: |
          cargo run --release --bin web_server &
          sleep 10
        env:
          DATABASE_URL: postgresql://postgres:postgres@localhost/github_archiver
          SERVER_PORT: 8081
      
      - name: Test API endpoints
        run: |
          curl -f http://localhost:8081/api/monitoring/metrics
          curl -f http://localhost:8081/monitoring-dashboard.html
      
      - name: Run integration tests
        run: cargo test --test integration_tests
```

---

## Test Coverage

### Generate Coverage Report

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run tests with coverage
cargo tarpaulin --out Html --output-dir ./coverage

# View report
xdg-open coverage/index.html
```

---

## Performance Benchmarks

### Expected Results

| Test | Expected | Threshold |
|------|----------|-----------|
| API Response Time | < 50ms | < 100ms |
| WebSocket Latency | < 10ms | < 50ms |
| Concurrent Users | 1000+ | 500+ |
| Memory Usage | < 200MB | < 500MB |
| CPU Usage (idle) | < 5% | < 10% |
| Dashboard Load Time | < 1s | < 2s |

---

## Debugging Tests

### Enable Debug Logging

```bash
# Run with trace logging
RUST_LOG=trace cargo test -- --nocapture

# Save debug output
RUST_LOG=debug cargo test 2>&1 | tee test_debug.log

# Filter specific module
RUST_LOG=github_archiver::api::monitoring_handlers=trace cargo test
```

### Browser DevTools

```javascript
// In browser console
localStorage.setItem('debug', 'true');
location.reload();

// Monitor WebSocket
const originalWebSocket = window.WebSocket;
window.WebSocket = function(...args) {
    console.log('WebSocket:', args);
    const ws = new originalWebSocket(...args);
    ws.addEventListener('message', e => console.log('WS Message:', e.data));
    return ws;
};
```

---

## Summary

Run all tests with:

```bash
# 1. Unit tests
cargo test

# 2. API tests
./test_api.sh

# 3. WebSocket tests
node test_websocket.js

# 4. Health check
./health_check.sh

# 5. Load test
k6 run load_test.js
```

**All tests should pass before deploying to production!**
