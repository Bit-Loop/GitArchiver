# Load Testing Configuration for GitHub Archiver
# Using k6 (https://k6.io) for load testing

import http from 'k6/http';
import { check, group, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const apiResponseTime = new Trend('api_response_time');
const successfulRequests = new Counter('successful_requests');

// Test configuration
export const options = {
  stages: [
    // Warm-up
    { duration: '2m', target: 10 },
    
    // Ramp up to normal load
    { duration: '5m', target: 100 },
    
    // Stay at normal load
    { duration: '10m', target: 100 },
    
    // Ramp up to peak load
    { duration: '5m', target: 500 },
    
    // Stay at peak load
    { duration: '10m', target: 500 },
    
    // Spike test
    { duration: '2m', target: 1000 },
    { duration: '3m', target: 1000 },
    
    // Ramp down
    { duration: '5m', target: 100 },
    { duration: '2m', target: 0 },
  ],
  
  thresholds: {
    // 95% of requests should be below 500ms
    'http_req_duration': ['p(95)<500'],
    
    // 99% of requests should be below 1s
    'http_req_duration': ['p(99)<1000'],
    
    // Error rate should be below 1%
    'errors': ['rate<0.01'],
    
    // At least 95% of requests should succeed
    'http_req_failed': ['rate<0.05'],
  },
  
  // Performance thresholds
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
};

// Base URL from environment or default
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';
const API_TOKEN = __ENV.API_TOKEN || '';

// Request headers
const headers = {
  'Content-Type': 'application/json',
  'Authorization': `Bearer ${API_TOKEN}`,
};

// Test scenarios
export default function () {
  group('Health Checks', () => {
    // Liveness probe
    group('Liveness', () => {
      const res = http.get(`${BASE_URL}/health/live`);
      check(res, {
        'liveness status is 200': (r) => r.status === 200,
        'liveness response time < 100ms': (r) => r.timings.duration < 100,
      });
      errorRate.add(res.status !== 200);
    });
    
    // Readiness probe
    group('Readiness', () => {
      const res = http.get(`${BASE_URL}/health/ready`);
      check(res, {
        'readiness status is 200': (r) => r.status === 200,
        'readiness response time < 200ms': (r) => r.timings.duration < 200,
      });
      errorRate.add(res.status !== 200);
    });
    
    // Detailed health
    group('Detailed Health', () => {
      const res = http.get(`${BASE_URL}/health`);
      check(res, {
        'health status is 200': (r) => r.status === 200,
        'health has status field': (r) => JSON.parse(r.body).status !== undefined,
      });
      errorRate.add(res.status !== 200);
    });
  });
  
  sleep(1);
  
  group('API Endpoints', () => {
    // List events
    group('List Events', () => {
      const res = http.get(`${BASE_URL}/api/v1/events?limit=100`, { headers });
      
      check(res, {
        'list events status is 200': (r) => r.status === 200,
        'list events response time < 500ms': (r) => r.timings.duration < 500,
        'list events has data': (r) => JSON.parse(r.body).data !== undefined,
      });
      
      if (res.status === 200) {
        successfulRequests.add(1);
      } else {
        errorRate.add(1);
      }
      
      apiResponseTime.add(res.timings.duration);
    });
    
    sleep(0.5);
    
    // Get specific event (if events exist)
    group('Get Event', () => {
      // First, get list to find an event ID
      const listRes = http.get(`${BASE_URL}/api/v1/events?limit=1`, { headers });
      
      if (listRes.status === 200) {
        const data = JSON.parse(listRes.body);
        if (data.data && data.data.length > 0) {
          const eventId = data.data[0].id;
          
          const res = http.get(`${BASE_URL}/api/v1/events/${eventId}`, { headers });
          
          check(res, {
            'get event status is 200': (r) => r.status === 200,
            'get event response time < 300ms': (r) => r.timings.duration < 300,
            'get event has id': (r) => JSON.parse(r.body).id !== undefined,
          });
          
          if (res.status === 200) {
            successfulRequests.add(1);
          } else {
            errorRate.add(1);
          }
          
          apiResponseTime.add(res.timings.duration);
        }
      }
    });
    
    sleep(0.5);
    
    // Search repositories
    group('Search Repositories', () => {
      const res = http.get(`${BASE_URL}/api/v1/repositories?search=test&limit=50`, { headers });
      
      check(res, {
        'search repos status is 200': (r) => r.status === 200,
        'search repos response time < 800ms': (r) => r.timings.duration < 800,
      });
      
      if (res.status === 200) {
        successfulRequests.add(1);
      } else {
        errorRate.add(1);
      }
      
      apiResponseTime.add(res.timings.duration);
    });
    
    sleep(0.5);
    
    // Get statistics
    group('Get Statistics', () => {
      const res = http.get(`${BASE_URL}/api/v1/statistics`, { headers });
      
      check(res, {
        'stats status is 200': (r) => r.status === 200,
        'stats response time < 1s': (r) => r.timings.duration < 1000,
        'stats has total_events': (r) => JSON.parse(r.body).total_events !== undefined,
      });
      
      if (res.status === 200) {
        successfulRequests.add(1);
      } else {
        errorRate.add(1);
      }
      
      apiResponseTime.add(res.timings.duration);
    });
  });
  
  sleep(1);
  
  // Simulate mixed workload
  const endpoint = Math.random();
  if (endpoint < 0.6) {
    // 60% - List events (most common)
    http.get(`${BASE_URL}/api/v1/events?limit=50`, { headers });
  } else if (endpoint < 0.85) {
    // 25% - Search
    http.get(`${BASE_URL}/api/v1/repositories?search=nodejs&limit=20`, { headers });
  } else if (endpoint < 0.95) {
    // 10% - Statistics
    http.get(`${BASE_URL}/api/v1/statistics`, { headers });
  } else {
    // 5% - Health check
    http.get(`${BASE_URL}/health`);
  }
  
  sleep(Math.random() * 2 + 1); // Random sleep 1-3 seconds
}

// Setup function (runs once at the start)
export function setup() {
  console.log('Starting load test...');
  console.log(`Target: ${BASE_URL}`);
  console.log(`Duration: 46 minutes`);
  console.log(`Max VUs: 1000`);
  return { startTime: Date.now() };
}

// Teardown function (runs once at the end)
export function teardown(data) {
  const duration = (Date.now() - data.startTime) / 1000;
  console.log(`Load test completed in ${duration}s`);
}

// Handle summary (custom reporting)
export function handleSummary(data) {
  return {
    'load-test-summary.json': JSON.stringify(data, null, 2),
    stdout: textSummary(data, { indent: ' ', enableColors: true }),
  };
}

// Text summary helper
function textSummary(data, options) {
  const { indent = '', enableColors = false } = options || {};
  
  let summary = '\n';
  summary += `${indent}Load Test Summary\n`;
  summary += `${indent}================\n\n`;
  
  // HTTP requests
  if (data.metrics.http_reqs) {
    summary += `${indent}HTTP Requests:\n`;
    summary += `${indent}  Total: ${data.metrics.http_reqs.values.count}\n`;
    summary += `${indent}  Rate: ${data.metrics.http_reqs.values.rate.toFixed(2)}/s\n\n`;
  }
  
  // Response times
  if (data.metrics.http_req_duration) {
    const duration = data.metrics.http_req_duration.values;
    summary += `${indent}Response Times:\n`;
    summary += `${indent}  Avg: ${duration.avg.toFixed(2)}ms\n`;
    summary += `${indent}  Min: ${duration.min.toFixed(2)}ms\n`;
    summary += `${indent}  Med: ${duration.med.toFixed(2)}ms\n`;
    summary += `${indent}  Max: ${duration.max.toFixed(2)}ms\n`;
    summary += `${indent}  p(90): ${duration['p(90)'].toFixed(2)}ms\n`;
    summary += `${indent}  p(95): ${duration['p(95)'].toFixed(2)}ms\n`;
    summary += `${indent}  p(99): ${duration['p(99)'].toFixed(2)}ms\n\n`;
  }
  
  // Error rate
  if (data.metrics.errors) {
    const errorPct = (data.metrics.errors.values.rate * 100).toFixed(2);
    summary += `${indent}Errors: ${errorPct}%\n\n`;
  }
  
  // Success rate
  if (data.metrics.http_req_failed) {
    const successPct = ((1 - data.metrics.http_req_failed.values.rate) * 100).toFixed(2);
    summary += `${indent}Success Rate: ${successPct}%\n\n`;
  }
  
  // Custom metrics
  if (data.metrics.successful_requests) {
    summary += `${indent}Successful Requests: ${data.metrics.successful_requests.values.count}\n`;
  }
  
  return summary;
}
