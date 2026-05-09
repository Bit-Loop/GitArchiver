/**
 * Smoke Test - Quick validation that the system is working
 * Duration: 30 seconds
 * Virtual Users: 10
 * Purpose: Verify basic functionality before heavier testing
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const responseTime = new Trend('response_time');

export const options = {
  vus: 10,
  duration: '30s',
  
  thresholds: {
    'http_req_duration': ['p(95)<200', 'p(99)<500'],
    'http_req_failed': ['rate<0.01'],
    'errors': ['rate<0.01'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';

export default function () {
  // Health check
  const healthRes = http.get(`${BASE_URL}/health/live`);
  check(healthRes, {
    'health check is 200': (r) => r.status === 200,
    'health check < 100ms': (r) => r.timings.duration < 100,
  }) || errorRate.add(1);
  
  responseTime.add(healthRes.timings.duration);
  
  sleep(1);
}

export function handleSummary(data) {
  console.log('\n=== SMOKE TEST RESULTS ===\n');
  
  const duration = data.metrics.http_req_duration?.values;
  const requests = data.metrics.http_reqs?.values;
  const failed = data.metrics.http_req_failed?.values;
  
  if (requests) {
    console.log(`Total Requests: ${requests.count}`);
    console.log(`Requests/sec: ${requests.rate.toFixed(2)}`);
  }
  
  if (duration) {
    console.log(`\nResponse Times:`);
    console.log(`  Avg: ${duration.avg.toFixed(2)}ms`);
    console.log(`  Min: ${duration.min.toFixed(2)}ms`);
    console.log(`  Med: ${duration.med.toFixed(2)}ms`);
    console.log(`  Max: ${duration.max.toFixed(2)}ms`);
    console.log(`  p(95): ${duration['p(95)'].toFixed(2)}ms`);
    console.log(`  p(99): ${duration['p(99)'].toFixed(2)}ms`);
  }
  
  if (failed) {
    const successRate = ((1 - failed.rate) * 100).toFixed(2);
    console.log(`\nSuccess Rate: ${successRate}%`);
  }
  
  console.log('\n=========================\n');
  
  return {
    'stdout': '',
  };
}
