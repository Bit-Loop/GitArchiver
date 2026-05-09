/**
 * Stress Test - High load to find breaking points
 * Duration: 10 minutes
 * Virtual Users: Ramp from 0 to 500 over 2 min, stay at 500 for 6 min, ramp down over 2 min
 * Purpose: Find system limits and identify bottlenecks
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const responseTime = new Trend('response_time');
const successfulRequests = new Counter('successful_requests');
const timeouts = new Counter('timeouts');

export const options = {
  stages: [
    { duration: '2m', target: 500 },  // Ramp up
    { duration: '6m', target: 500 },  // Stay at stress load
    { duration: '2m', target: 0 },    // Ramp down
  ],
  
  thresholds: {
    'http_req_duration': ['p(95)<1000', 'p(99)<2000'],
    'http_req_failed': ['rate<0.10'],  // Allow up to 10% errors under stress
    'errors': ['rate<0.15'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';

export default function () {
  // Aggressive mixed workload
  const rand = Math.random();
  
  let res;
  
  if (rand < 0.5) {
    // 50% - Health checks
    res = http.get(`${BASE_URL}/health/live`, { timeout: '10s' });
    check(res, {
      'health status 200': (r) => r.status === 200,
    }) ? successfulRequests.add(1) : errorRate.add(1);
    
  } else if (rand < 0.8) {
    // 30% - Readiness
    res = http.get(`${BASE_URL}/health/ready`, { timeout: '10s' });
    check(res, {
      'ready status 200': (r) => r.status === 200,
    }) ? successfulRequests.add(1) : errorRate.add(1);
    
  } else if (rand < 0.95) {
    // 15% - Full health
    res = http.get(`${BASE_URL}/health`, { timeout: '10s' });
    check(res, {
      'full health status 200': (r) => r.status === 200,
    }) ? successfulRequests.add(1) : errorRate.add(1);
    
  } else {
    // 5% - Metrics
    res = http.get(`${BASE_URL}/metrics`, { timeout: '10s' });
    check(res, {
      'metrics available': (r) => r.status === 200 || r.status === 404,
    }) ? successfulRequests.add(1) : errorRate.add(1);
  }
  
  if (res) {
    responseTime.add(res.timings.duration);
    
    if (res.error_code === 1050) { // Timeout
      timeouts.add(1);
    }
  }
  
  sleep(Math.random() * 1 + 0.2); // Random sleep 0.2-1.2s (more aggressive)
}

export function handleSummary(data) {
  console.log('\n=== STRESS TEST RESULTS ===\n');
  
  const duration = data.metrics.http_req_duration?.values;
  const requests = data.metrics.http_reqs?.values;
  const failed = data.metrics.http_req_failed?.values;
  const successful = data.metrics.successful_requests?.values;
  const timeoutCount = data.metrics.timeouts?.values;
  
  if (requests) {
    console.log(`Total Requests: ${requests.count}`);
    console.log(`Requests/sec: ${requests.rate.toFixed(2)}`);
  }
  
  if (successful) {
    console.log(`Successful Requests: ${successful.count}`);
  }
  
  if (timeoutCount) {
    console.log(`Timeouts: ${timeoutCount.count}`);
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
  
  // Performance analysis
  if (duration && requests) {
    const throughput = requests.rate;
    const p95 = duration['p(95)'];
    
    console.log(`\nPerformance Analysis:`);
    console.log(`  Throughput: ${throughput.toFixed(2)} req/s`);
    console.log(`  Target: 10000 events/s`);
    
    if (p95 < 500) {
      console.log(`  ✓ p95 latency excellent (< 500ms)`);
    } else if (p95 < 1000) {
      console.log(`  ✓ p95 latency good (< 1s)`);
    } else if (p95 < 2000) {
      console.log(`  ⚠ p95 latency acceptable (< 2s)`);
    } else {
      console.log(`  ✗ p95 latency poor (> 2s)`);
    }
  }
  
  console.log('\n===========================\n');
  
  return {
    'stdout': '',
  };
}
