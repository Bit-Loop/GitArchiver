/**
 * Load Test - Normal traffic simulation
 * Duration: 5 minutes
 * Virtual Users: Ramp from 0 to 100 over 1 min, stay at 100 for 3 min, ramp down over 1 min
 * Purpose: Measure performance under expected production load
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const responseTime = new Trend('response_time');
const successfulRequests = new Counter('successful_requests');

export const options = {
  stages: [
    { duration: '1m', target: 100 },  // Ramp up
    { duration: '3m', target: 100 },  // Stay at load
    { duration: '1m', target: 0 },    // Ramp down
  ],
  
  thresholds: {
    'http_req_duration': ['p(95)<500', 'p(99)<1000'],
    'http_req_failed': ['rate<0.05'],
    'errors': ['rate<0.05'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';

export default function () {
  // Mix of endpoints simulating real traffic
  const rand = Math.random();
  
  if (rand < 0.6) {
    // 60% - Health checks (most common)
    const res = http.get(`${BASE_URL}/health/live`);
    check(res, {
      'health status 200': (r) => r.status === 200,
      'health < 200ms': (r) => r.timings.duration < 200,
    }) ? successfulRequests.add(1) : errorRate.add(1);
    
    responseTime.add(res.timings.duration);
    
  } else if (rand < 0.85) {
    // 25% - Readiness checks
    const res = http.get(`${BASE_URL}/health/ready`);
    check(res, {
      'ready status 200': (r) => r.status === 200,
      'ready < 300ms': (r) => r.timings.duration < 300,
    }) ? successfulRequests.add(1) : errorRate.add(1);
    
    responseTime.add(res.timings.duration);
    
  } else if (rand < 0.95) {
    // 10% - Full health
    const res = http.get(`${BASE_URL}/health`);
    check(res, {
      'full health status 200': (r) => r.status === 200,
      'full health < 500ms': (r) => r.timings.duration < 500,
    }) ? successfulRequests.add(1) : errorRate.add(1);
    
    responseTime.add(res.timings.duration);
    
  } else {
    // 5% - Metrics endpoint
    const res = http.get(`${BASE_URL}/metrics`);
    check(res, {
      'metrics available': (r) => r.status === 200 || r.status === 404,
    }) ? successfulRequests.add(1) : errorRate.add(1);
    
    responseTime.add(res.timings.duration);
  }
  
  sleep(Math.random() * 2 + 0.5); // Random sleep 0.5-2.5s
}

export function handleSummary(data) {
  console.log('\n=== LOAD TEST RESULTS ===\n');
  
  const duration = data.metrics.http_req_duration?.values;
  const requests = data.metrics.http_reqs?.values;
  const failed = data.metrics.http_req_failed?.values;
  const successful = data.metrics.successful_requests?.values;
  
  if (requests) {
    console.log(`Total Requests: ${requests.count}`);
    console.log(`Requests/sec: ${requests.rate.toFixed(2)}`);
  }
  
  if (successful) {
    console.log(`Successful Requests: ${successful.count}`);
  }
  
  if (duration) {
    console.log(`\nResponse Times:`);
    console.log(`  Avg: ${duration.avg.toFixed(2)}ms`);
    console.log(`  Min: ${duration.min.toFixed(2)}ms`);
    console.log(`  Med: ${duration.med.toFixed(2)}ms`);
    console.log(`  Max: ${duration.max.toFixed(2)}ms`);
    console.log(`  p(90): ${duration['p(90)'].toFixed(2)}ms`);
    console.log(`  p(95): ${duration['p(95)'].toFixed(2)}ms`);
    console.log(`  p(99): ${duration['p(99)'].toFixed(2)}ms`);
  }
  
  if (failed) {
    const successRate = ((1 - failed.rate) * 100).toFixed(2);
    const errorPct = (failed.rate * 100).toFixed(2);
    console.log(`\nSuccess Rate: ${successRate}%`);
    console.log(`Error Rate: ${errorPct}%`);
  }
  
  // Check if thresholds were met
  const thresholds = data.root_group?.checks;
  if (thresholds) {
    const passed = Object.values(thresholds).filter(c => c.passes > 0).length;
    const total = Object.values(thresholds).length;
    console.log(`\nChecks Passed: ${passed}/${total}`);
  }
  
  console.log('\n=========================\n');
  
  return {
    'stdout': '',
  };
}
