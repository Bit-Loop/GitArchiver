/**
 * Authenticated Load Test - Validates Audit Logging Performance
 * 
 * Purpose: Test authenticated endpoints that trigger audit logging to validate:
 * - Audit log write performance under load
 * - Database impact of frequent audit log inserts
 * - Authentication/authorization overhead
 * - Real-world usage patterns
 * 
 * Duration: 5 minutes
 * Virtual Users: Up to 100 (ramped)
 * Endpoints: Login, scraper control, status checks, logout
 * Expected Audit Logs: ~60,000-100,000 entries
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics for audit logging analysis
const auditLogWrites = new Counter('audit_log_writes');
const loginLatency = new Trend('login_latency');
const scraperStartLatency = new Trend('scraper_start_latency');
const scraperStopLatency = new Trend('scraper_stop_latency');
const logoutLatency = new Trend('logout_latency');
const authFailures = new Rate('auth_failures');
const scraperOperationFailures = new Rate('scraper_operation_failures');

export const options = {
  // 3-stage ramp: 0→50→100 VUs over 5 minutes
  stages: [
    { duration: '1m', target: 20 },   // Warm-up: 0→20 users
    { duration: '2m', target: 50 },   // Ramp-up: 20→50 users
    { duration: '1m', target: 100 },  // Peak load: 50→100 users
    { duration: '1m', target: 100 },  // Sustain: 100 users
  ],
  
  thresholds: {
    // Authentication thresholds
    'login_latency': ['p(95)<1000', 'p(99)<2000'],           // Login under 1s (p95)
    'auth_failures': ['rate<0.05'],                          // <5% auth failures
    
    // Scraper operation thresholds
    'scraper_start_latency': ['p(95)<2000', 'p(99)<5000'],   // Start under 2s (p95)
    'scraper_stop_latency': ['p(95)<2000', 'p(99)<5000'],    // Stop under 2s (p95)
    'scraper_operation_failures': ['rate<0.05'],             // <5% operation failures
    
    // Overall HTTP thresholds
    'http_req_duration': ['p(95)<3000', 'p(99)<5000'],       // Overall p95 under 3s
    'http_req_failed': ['rate<0.10'],                        // <10% failures (allowing for rate limits)
    
    // Logout threshold
    'logout_latency': ['p(95)<500'],                         // Logout under 500ms
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';

// Test credentials - will be created before test
const TEST_USERS = [
  { username: 'loadtest_user1', password: 'LoadTest123!' },
  { username: 'loadtest_user2', password: 'LoadTest123!' },
  { username: 'loadtest_user3', password: 'LoadTest123!' },
  { username: 'loadtest_user4', password: 'LoadTest123!' },
  { username: 'loadtest_user5', password: 'LoadTest123!' },
];

/**
 * Main test function - simulates real user behavior with audit logging
 * 
 * Workflow:
 * 1. Login (creates audit log: UserLogin)
 * 2. Start scraper (creates audit log: ScraperStart)
 * 3. Check status (no audit log - read-only)
 * 4. Stop scraper (creates audit log: ScraperStop)
 * 5. Logout (creates audit log: UserLogout)
 * 
 * Expected: 4 audit log entries per iteration
 */
export default function() {
  // Select a random test user to distribute load
  const user = TEST_USERS[Math.floor(Math.random() * TEST_USERS.length)];
  
  // === 1. LOGIN (AUDIT LOG: UserLogin) ===
  const loginStart = Date.now();
  const loginPayload = JSON.stringify({
    username: user.username,
    password: user.password,
  });
  
  const loginRes = http.post(`${BASE_URL}/api/auth/login`, loginPayload, {
    headers: { 'Content-Type': 'application/json' },
    tags: { name: 'Login' },
  });
  
  loginLatency.add(Date.now() - loginStart);
  
  const loginSuccess = check(loginRes, {
    'login status is 200': (r) => r.status === 200,
    'login returns token': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.token !== undefined;
      } catch (e) {
        return false;
      }
    },
  });
  
  if (!loginSuccess) {
    authFailures.add(1);
    console.log(`Login failed for ${user.username}: ${loginRes.status} ${loginRes.body}`);
    sleep(1);
    return; // Skip rest of iteration if login fails
  }
  
  authFailures.add(0);
  auditLogWrites.add(1); // Track expected audit log write
  
  // Extract token
  let token;
  try {
    const loginBody = JSON.parse(loginRes.body);
    token = loginBody.token;
  } catch (e) {
    console.log(`Failed to parse login response: ${e}`);
    sleep(1);
    return;
  }
  
  const authHeaders = {
    'Authorization': `Bearer ${token}`,
    'Content-Type': 'application/json',
  };
  
  // Small delay between operations
  sleep(0.5);
  
  // === 2. START SCRAPER (AUDIT LOG: ScraperStart) ===
  const startScraperStart = Date.now();
  const startPayload = JSON.stringify({
    hunt_id: `load-test-${__VU}-${Date.now()}`, // Unique hunt ID per VU
    interval: 60,
    max_events: 100,
    mode: 'realtime',
  });
  
  const startRes = http.post(`${BASE_URL}/api/scraper/start`, startPayload, {
    headers: authHeaders,
    tags: { name: 'StartScraper' },
  });
  
  scraperStartLatency.add(Date.now() - startScraperStart);
  
  const startSuccess = check(startRes, {
    'scraper start status is 200': (r) => r.status === 200,
    'scraper start returns success': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.status === 'started' || body.message !== undefined;
      } catch (e) {
        return false;
      }
    },
  });
  
  if (!startSuccess) {
    scraperOperationFailures.add(1);
    console.log(`Scraper start failed: ${startRes.status} ${startRes.body}`);
  } else {
    scraperOperationFailures.add(0);
    auditLogWrites.add(1); // Track expected audit log write
  }
  
  sleep(0.5);
  
  // === 3. CHECK STATUS (NO AUDIT LOG - Read-only) ===
  const statusRes = http.get(`${BASE_URL}/api/scraper/status`, {
    headers: authHeaders,
    tags: { name: 'ScraperStatus' },
  });
  
  check(statusRes, {
    'status check is 200': (r) => r.status === 200,
  });
  
  sleep(0.5);
  
  // === 4. STOP SCRAPER (AUDIT LOG: ScraperStop) ===
  const stopScraperStart = Date.now();
  
  const stopRes = http.post(`${BASE_URL}/api/scraper/stop`, null, {
    headers: authHeaders,
    tags: { name: 'StopScraper' },
  });
  
  scraperStopLatency.add(Date.now() - stopScraperStart);
  
  const stopSuccess = check(stopRes, {
    'scraper stop status is 200': (r) => r.status === 200,
    'scraper stop returns success': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.status === 'stopped' || body.message !== undefined;
      } catch (e) {
        return false;
      }
    },
  });
  
  if (!stopSuccess) {
    scraperOperationFailures.add(1);
    console.log(`Scraper stop failed: ${stopRes.status} ${stopRes.body}`);
  } else {
    scraperOperationFailures.add(0);
    auditLogWrites.add(1); // Track expected audit log write
  }
  
  sleep(0.5);
  
  // === 5. LOGOUT (AUDIT LOG: UserLogout) ===
  const logoutStart = Date.now();
  
  const logoutRes = http.post(`${BASE_URL}/api/auth/logout`, null, {
    headers: authHeaders,
    tags: { name: 'Logout' },
  });
  
  logoutLatency.add(Date.now() - logoutStart);
  
  check(logoutRes, {
    'logout status is 200': (r) => r.status === 200,
  });
  
  auditLogWrites.add(1); // Track expected audit log write
  
  // Sleep before next iteration (simulate think time)
  sleep(1);
}

/**
 * Summary handler - display custom report
 */
export function handleSummary(data) {
  const metrics = data.metrics;
  
  // Calculate custom statistics
  const totalIterations = metrics.iterations.values.count || 0;
  const expectedAuditLogs = metrics.audit_log_writes?.values?.count || 0;
  const totalDuration = data.state.testRunDurationMs / 1000;
  const requestsPerSecond = totalIterations * 5 / totalDuration; // 5 requests per iteration
  
  console.log('\n=== AUTHENTICATED LOAD TEST RESULTS ===');
  console.log(`Total Iterations: ${totalIterations}`);
  console.log(`Expected Audit Logs: ${expectedAuditLogs}`);
  console.log(`Requests/sec: ${requestsPerSecond.toFixed(2)}`);
  console.log(`Duration: ${totalDuration.toFixed(1)}s`);
  console.log('');
  
  // Authentication metrics
  console.log('Authentication Performance:');
  if (metrics.login_latency) {
    console.log(`  Login p95: ${metrics.login_latency.values['p(95)'].toFixed(2)}ms`);
    console.log(`  Login p99: ${metrics.login_latency.values['p(99)'].toFixed(2)}ms`);
    console.log(`  Login avg: ${metrics.login_latency.values.avg.toFixed(2)}ms`);
  }
  if (metrics.logout_latency) {
    console.log(`  Logout p95: ${metrics.logout_latency.values['p(95)'].toFixed(2)}ms`);
  }
  if (metrics.auth_failures) {
    console.log(`  Auth failure rate: ${(metrics.auth_failures.values.rate * 100).toFixed(2)}%`);
  }
  console.log('');
  
  // Scraper operation metrics
  console.log('Scraper Operations:');
  if (metrics.scraper_start_latency) {
    console.log(`  Start p95: ${metrics.scraper_start_latency.values['p(95)'].toFixed(2)}ms`);
    console.log(`  Start avg: ${metrics.scraper_start_latency.values.avg.toFixed(2)}ms`);
  }
  if (metrics.scraper_stop_latency) {
    console.log(`  Stop p95: ${metrics.scraper_stop_latency.values['p(95)'].toFixed(2)}ms`);
    console.log(`  Stop avg: ${metrics.scraper_stop_latency.values.avg.toFixed(2)}ms`);
  }
  if (metrics.scraper_operation_failures) {
    console.log(`  Operation failure rate: ${(metrics.scraper_operation_failures.values.rate * 100).toFixed(2)}%`);
  }
  console.log('');
  
  // Overall HTTP metrics
  console.log('Overall Performance:');
  if (metrics.http_req_duration) {
    console.log(`  HTTP p95: ${metrics.http_req_duration.values['p(95)'].toFixed(2)}ms`);
    console.log(`  HTTP p99: ${metrics.http_req_duration.values['p(99)'].toFixed(2)}ms`);
    console.log(`  HTTP avg: ${metrics.http_req_duration.values.avg.toFixed(2)}ms`);
  }
  if (metrics.http_req_failed) {
    console.log(`  HTTP failure rate: ${(metrics.http_req_failed.values.rate * 100).toFixed(2)}%`);
  }
  console.log('');
  
  // Audit logging estimate
  console.log('Audit Logging:');
  console.log(`  Expected audit log writes: ${expectedAuditLogs}`);
  console.log(`  Audit writes/sec: ${(expectedAuditLogs / totalDuration).toFixed(2)}`);
  console.log('  (Verify actual count with: SELECT COUNT(*) FROM audit_logs WHERE created_at > NOW() - INTERVAL \'10 minutes\')');
  console.log('');
  
  return {
    'stdout': '', // k6 will use the console.log output above
  };
}
