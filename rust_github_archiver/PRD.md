# Product Requirements Document (PRD)
## GitHub Events API Real-Time Monitoring System

**Version:** 1.0  
**Date:** October 6, 2025  
**Status:** In Production  
**Product Owner:** Bitloop  

---

## 1. Executive Summary

### 1.1 Product Overview
A real-time GitHub Events API monitoring system that continuously fetches, stores, and analyzes public GitHub events. The system provides intelligent rate limiting, adaptive throttling, and seamless integration with the existing GitHub Archiver platform.

### 1.2 Business Objectives
- **Data Collection**: Capture up to 9,000 GitHub events per hour (unauthenticated)
- **Scalability**: Support 25,000-50,000+ events/hour with multi-token rotation
- **Reliability**: 99.9% uptime with automatic rate limit handling
- **Cost Efficiency**: Start free (60 req/hour), scale to $100-500/month for enterprise-level monitoring

### 1.3 Success Metrics
- Events captured per hour: 9,000+ (baseline), 50,000+ (scaled)
- API success rate: >95% (accounting for GitHub rate limits)
- Database insertion success: >99%
- System uptime: >99.9%
- Response time: <100ms for API endpoints

---

## 2. User Stories & Use Cases

### 2.1 Primary Users
1. **Security Researchers**: Monitor for leaked secrets and credentials in real-time
2. **Data Analysts**: Analyze GitHub activity patterns and trends
3. **DevOps Engineers**: Track repository events for CI/CD automation
4. **Open Source Maintainers**: Monitor ecosystem activity and contributions

### 2.2 Core User Stories

**US-001**: As a security researcher, I want to monitor GitHub events in real-time so that I can detect leaked secrets immediately.
- **Acceptance Criteria**: 
  - System fetches events every 12-60 seconds (configurable)
  - Events stored in PostgreSQL within 5 seconds of fetching
  - Dashboard shows real-time event count and status

**US-002**: As a system administrator, I want the rate limiter to auto-adjust so that I don't hit GitHub's API limits.
- **Acceptance Criteria**:
  - System detects HTTP 429 responses
  - Automatically reduces request rate by 50%
  - Gradually increases rate after successful requests
  - Logs all rate limit adjustments

**US-003**: As a developer, I want to optionally provide a GitHub token so that I can increase rate limits from 60 to 5,000 requests/hour.
- **Acceptance Criteria**:
  - Token input field in dashboard UI
  - Token stored securely in localStorage
  - System works without token (graceful degradation)
  - Token included in API requests when provided

**US-004**: As a data analyst, I want to control the monitoring system via API and UI so that I can start/stop collection based on my needs.
- **Acceptance Criteria**:
  - REST API endpoints: start, stop, pause, resume
  - Dashboard UI with control buttons
  - Real-time status updates (polling every 5 seconds)
  - Configuration updates without restart

---

## 3. Technical Requirements

### 3.1 Functional Requirements

#### FR-001: Event Monitoring
- **Priority**: P0 (Critical)
- **Description**: Continuously fetch events from GitHub Events API
- **Details**:
  - Endpoint: `https://api.github.com/events`
  - Method: GET with conditional If-None-Match headers
  - Response: JSON array of ~30 events per request
  - Frequency: Configurable 1-60 requests/minute
  - Storage: PostgreSQL `github_events` table with `api_source='github_events_api'`

#### FR-002: Adaptive Rate Limiting
- **Priority**: P0 (Critical)
- **Description**: Intelligent sliding window rate limiter with auto-adjustment
- **Algorithm**: Sliding Window Counter
- **Configuration**:
  - Base rate: 1-60 req/min (user configurable)
  - Auto-adjust: Enabled/disabled flag
  - Reduction factor: 0.5 (50% on HTTP 429)
  - Increase factor: 1.1 (10% on sustained success)
  - Min rate: 1 req/min
  - Max rate: 60 req/min
- **Behavior**:
  - Record all requests with timestamps
  - Enforce sliding window limits
  - Sleep between requests to maintain rate
  - Detect HTTP 429 and reduce rate immediately
  - Gradually increase rate after 10+ successful requests

#### FR-003: REST API
- **Priority**: P0 (Critical)
- **Endpoints**:
  ```
  POST   /api/realtime/start         # Start monitoring
  POST   /api/realtime/stop          # Stop monitoring
  POST   /api/realtime/pause         # Pause monitoring
  POST   /api/realtime/resume        # Resume monitoring
  GET    /api/realtime/status        # Get current status
  POST   /api/realtime/config        # Update configuration
  POST   /api/realtime/stats/reset   # Reset statistics
  ```
- **Request/Response Format**: JSON
- **Authentication**: None (future: API key-based)
- **Rate Limiting**: None (internal endpoints)

#### FR-004: Web Dashboard
- **Priority**: P1 (High)
- **Components**:
  - GitHub Events tab in existing dashboard
  - Token input field (localStorage persistence)
  - Control panel: Start/Stop/Pause/Resume buttons
  - Status display: Running/Stopped/Paused
  - Statistics: Events collected, requests made, errors
  - Configuration form: Rate limit, auto-adjust toggle
- **Update Frequency**: Poll status every 5 seconds
- **Styling**: Bootstrap 5 with custom CSS

#### FR-005: Database Integration
- **Priority**: P0 (Critical)
- **Schema**:
  ```sql
  github_events (
    id BIGSERIAL PRIMARY KEY,
    event_id VARCHAR(255) UNIQUE NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    actor_login VARCHAR(255),
    repo_name VARCHAR(255),
    created_at TIMESTAMP NOT NULL,
    payload JSONB,
    api_source VARCHAR(50) DEFAULT 'github_events_api'
  )
  ```
- **Operations**:
  - Upsert events (INSERT ... ON CONFLICT DO NOTHING)
  - Deduplicate by event_id
  - Index on created_at, event_type, api_source
  - Partition by month (future optimization)

### 3.2 Non-Functional Requirements

#### NFR-001: Performance
- **Event Processing**: <100ms per event
- **Database Insertion**: <500ms for batch of 30 events
- **API Response Time**: <50ms (90th percentile)
- **Memory Usage**: <200MB steady state
- **CPU Usage**: <10% on 2-core system

#### NFR-002: Reliability
- **Uptime**: 99.9% (8.76 hours downtime/year)
- **Data Loss**: <0.01% (duplicate prevention via unique constraints)
- **Error Recovery**: Auto-retry on transient failures (max 3 attempts)
- **Graceful Degradation**: Continue without token if provided token invalid

#### NFR-003: Scalability
- **Phase 1 (Current)**: 60 req/hour, ~1,800 events/hour (unauthenticated)
- **Phase 2 (Token)**: 5,000 req/hour, ~150,000 events/hour (single token)
- **Phase 3 (Multi-Token)**: 25,000-50,000 req/hour, ~750,000-1.5M events/hour (5-10 tokens)
- **Phase 4 (Proxy Rotation)**: Near unlimited within budget constraints
- **Phase 5 (Multi-Region)**: 100,000+ req/hour across 3-5 VPS instances

#### NFR-004: Maintainability
- **Code Quality**: Rust with strict clippy lints
- **Documentation**: Inline comments, README, this PRD
- **Testing**: Unit tests for rate limiter, integration tests for API
- **Logging**: Structured logging with env_logger (info, warn, error levels)
- **Monitoring**: Real-time metrics via /api/realtime/status

#### NFR-005: Security
- **Token Storage**: localStorage (browser), never logged or stored server-side
- **API Authentication**: None (future: API keys, OAuth)
- **SQL Injection**: Protected via parameterized queries (sqlx)
- **CORS**: Enabled for dashboard access
- **HTTPS**: Recommended for production (currently HTTP)

---

## 4. System Architecture

### 4.1 Technology Stack
- **Backend**: Rust 1.75+ with Axum web framework
- **Database**: PostgreSQL 15.14
- **Frontend**: Vanilla JavaScript, Bootstrap 5, HTML5
- **Rate Limiting**: Custom sliding window algorithm
- **HTTP Client**: reqwest with async/await
- **Serialization**: serde_json
- **Logging**: env_logger + log crate

### 4.2 Component Diagram
```
┌─────────────────────────────────────────────────────┐
│                 Web Dashboard                       │
│  (dashboard.html - GitHub Events Tab)               │
│  - Token Input (localStorage)                       │
│  - Control Panel (Start/Stop/Pause/Resume)          │
│  - Status Display & Statistics                      │
└─────────────────┬───────────────────────────────────┘
                  │ HTTP/JSON
                  ↓
┌─────────────────────────────────────────────────────┐
│           Axum Web Server (Port 8081)               │
│  ┌───────────────────────────────────────────────┐  │
│  │     Realtime API Handlers                     │  │
│  │  - POST /api/realtime/start                   │  │
│  │  - POST /api/realtime/stop                    │  │
│  │  - POST /api/realtime/pause                   │  │
│  │  - POST /api/realtime/resume                  │  │
│  │  - GET  /api/realtime/status                  │  │
│  │  - POST /api/realtime/config                  │  │
│  │  - POST /api/realtime/stats/reset             │  │
│  └─────────────────┬─────────────────────────────┘  │
│                    │                                 │
│  ┌─────────────────▼─────────────────────────────┐  │
│  │     GitHubEventMonitor (Core Logic)           │  │
│  │  - State: Running/Stopped/Paused              │  │
│  │  - Config: requests_per_minute, auto_adjust   │  │
│  │  - Stats: events_collected, requests_made     │  │
│  │  - poll_events() async loop                   │  │
│  └─────┬───────────────────────────────┬─────────┘  │
│        │                               │             │
│  ┌─────▼──────────────┐     ┌──────────▼─────────┐  │
│  │ AdaptiveRateLimiter│     │ Database Module    │  │
│  │  - Sliding Window  │     │  - PostgreSQL Pool │  │
│  │  - Auto-Adjust     │     │  - Event Upsert    │  │
│  │  - Request History │     │  - Deduplication   │  │
│  └─────┬──────────────┘     └──────────┬─────────┘  │
│        │                               │             │
└────────┼───────────────────────────────┼─────────────┘
         │                               │
         ↓                               ↓
┌────────────────────┐         ┌─────────────────────┐
│  GitHub Events API │         │  PostgreSQL 15.14   │
│  api.github.com    │         │  github_events table│
│  /events endpoint  │         │  Partitioned by date│
└────────────────────┘         └─────────────────────┘
```

### 4.3 Data Flow
1. **User Interaction**: User clicks "Start" in dashboard
2. **API Request**: POST to /api/realtime/start with config + optional token
3. **Monitor Initialization**: GitHubEventMonitor created with rate limiter and DB
4. **Event Polling Loop**:
   - Wait for rate limiter clearance
   - Fetch events from GitHub API (with token if provided)
   - Parse JSON response (handle 304 Not Modified, 429 Rate Limit)
   - Store events in PostgreSQL (batch upsert)
   - Update statistics and metrics
   - Repeat until stopped/paused
5. **Status Updates**: Dashboard polls /api/realtime/status every 5s
6. **Rate Limit Handling**: On HTTP 429, reduce rate by 50%, log warning

### 4.4 State Machine
```
                    ┌─────────┐
                    │ STOPPED │ (Initial State)
                    └────┬────┘
                         │ start()
                         ↓
                    ┌─────────┐
              ┌────→│ RUNNING │←────┐
              │     └────┬────┘     │
              │          │          │
    resume()  │     pause()    stop()│ start()
              │          │          │
              │     ┌────▼────┐     │
              └─────│ PAUSED  │─────┘
                    └────┬────┘
                         │ stop()
                         ↓
                    ┌─────────┐
                    │ STOPPED │
                    └─────────┘
```

---

## 5. API Specification

### 5.1 Start Monitor
```http
POST /api/realtime/start
Content-Type: application/json

{
  "requests_per_minute": 5,      // 1-60 req/min
  "auto_adjust": true,            // Enable adaptive throttling
  "github_token": "ghp_REDACTED_EXAMPLE..."    // Optional, increases rate limit
}

Response 200 OK:
{
  "status": "success",
  "message": "Event monitor started successfully"
}

Response 400 Bad Request:
{
  "status": "error",
  "message": "Monitor is already running"
}
```

### 5.2 Get Status
```http
GET /api/realtime/status

Response 200 OK:
{
  "status": "running",              // running | stopped | paused
  "config": {
    "requests_per_minute": 5,
    "auto_adjust": true
  },
  "stats": {
    "events_collected": 1234,
    "requests_made": 456,
    "successful_requests": 450,
    "failed_requests": 6,
    "rate_limit_hits": 2,
    "last_event_time": "2025-10-06T12:34:56Z"
  },
  "current_rate": 5.0,              // Current effective rate
  "uptime_seconds": 3600
}
```

### 5.3 Update Configuration
```http
POST /api/realtime/config
Content-Type: application/json

{
  "requests_per_minute": 10,
  "auto_adjust": false
}

Response 200 OK:
{
  "status": "success",
  "message": "Configuration updated successfully",
  "new_config": {
    "requests_per_minute": 10,
    "auto_adjust": false
  }
}
```

---

## 6. Implementation Status

### 6.1 Completed Features ✅
- [x] AdaptiveRateLimiter with sliding window algorithm (340 lines)
- [x] GitHubEventMonitor with database integration
- [x] 7 REST API endpoints (start, stop, pause, resume, status, config, reset)
- [x] Web dashboard with GitHub Events tab
- [x] Optional GitHub token support (localStorage-based)
- [x] Auto-adjust rate limiting on HTTP 429
- [x] PostgreSQL event storage with deduplication
- [x] Background server execution (nohup)
- [x] Structured logging with RUST_LOG
- [x] Bug fixes: Deadlock in rate limiter, JSON field mapping

### 6.2 In Progress 🚧
- [ ] Database verification (events fetching, need to confirm storage)
- [ ] Dashboard status display accuracy
- [ ] Real-time event count updates

### 6.3 Planned Features 📋

#### Phase 2: Multi-Token Rotation (Free Scaling)
- [ ] Token pool management (5-10 tokens)
- [ ] Round-robin token selection with health tracking
- [ ] Auto-switch on token rate limit
- [ ] Token performance metrics
- **Capacity**: 25,000-50,000 req/hour
- **Cost**: Free (multiple GitHub accounts)

#### Phase 3: Proxy Rotation (Paid Scaling)
- [ ] Residential proxy integration (Bright Data, Smartproxy)
- [ ] Proxy pool management with rotation
- [ ] Request distribution across proxies
- [ ] Proxy health monitoring and auto-failover
- **Capacity**: Near unlimited within budget
- **Cost**: $50-100/month

#### Phase 4: Multi-Region Deployment (Enterprise Scaling)
- [ ] Deploy to 3-5 VPS regions (AWS, DigitalOcean, Linode)
- [ ] Central coordination service
- [ ] Regional token pools
- [ ] Load balancing and data aggregation
- **Capacity**: 100,000+ req/hour
- **Cost**: $100-500/month

#### Phase 5: Advanced Features
- [ ] GraphQL API support
- [ ] WebSocket real-time streaming
- [ ] Event filtering and querying
- [ ] Alerting system (webhooks, email)
- [ ] Analytics dashboard with charts
- [ ] Export to CSV, JSON, Parquet
- [ ] Machine learning anomaly detection

---

## 7. Testing Requirements

### 7.1 Unit Tests
- [x] Rate limiter sliding window logic
- [x] Rate limiter auto-adjust algorithm
- [ ] Event parsing and validation
- [ ] Database upsert logic
- [ ] API endpoint handlers

### 7.2 Integration Tests
- [ ] End-to-end event flow (fetch → parse → store)
- [ ] Rate limit enforcement with GitHub API
- [ ] Token rotation and failover
- [ ] Database concurrent writes
- [ ] API stress testing (100+ req/s)

### 7.3 Performance Tests
- [ ] Benchmark event processing throughput
- [ ] Database insertion performance (1000+ events)
- [ ] Memory leak testing (24+ hour run)
- [ ] Rate limiter accuracy under load

### 7.4 Edge Cases
- [ ] GitHub API downtime (503, 504 errors)
- [ ] Invalid/expired tokens
- [ ] Database connection loss
- [ ] Malformed JSON responses
- [ ] Clock skew in rate limiter

---

## 8. Deployment & Operations

### 8.1 Deployment Architecture
```
Production Environment:
- VPS: Ubuntu 22.04 LTS, 2 CPU, 4GB RAM
- Database: PostgreSQL 15.14 (same host or managed service)
- Web Server: Nginx reverse proxy (HTTPS termination)
- Application: Rust binary via systemd service
- Monitoring: Prometheus + Grafana (future)
```

### 8.2 Systemd Service Configuration
```ini
[Unit]
Description=GitHub Events API Monitor
After=network.target postgresql.service

[Service]
Type=simple
User=github-archiver
WorkingDirectory=/opt/github-archiver
Environment="RUST_LOG=info"
Environment="DATABASE_URL=postgresql://user:pass@localhost/github_archiver"
ExecStart=/opt/github-archiver/web_server
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### 8.3 Monitoring & Alerting
- **Metrics**: Events/hour, request success rate, error rate, database size
- **Logs**: Centralized logging to /var/log/github-archiver/
- **Alerts**: 
  - Error rate >5% → PagerDuty
  - Request success rate <90% → Slack
  - Database size >80% capacity → Email
  - Service down >5 minutes → SMS

### 8.4 Backup & Recovery
- **Database Backups**: Daily full backup, hourly incrementals
- **Retention Policy**: 30 days online, 90 days cold storage
- **Recovery Time Objective (RTO)**: <1 hour
- **Recovery Point Objective (RPO)**: <1 hour
- **Disaster Recovery**: Multi-region replication (future)

---

## 9. Success Criteria & KPIs

### 9.1 Launch Criteria (Phase 1)
- ✅ Events fetching successfully (30 events/request)
- ⏳ Database storage verified (>99% insertion success)
- ⏳ Dashboard displays accurate status
- ✅ Rate limiter prevents HTTP 429 errors
- ✅ System runs for 24+ hours without restart

### 9.2 Key Performance Indicators
- **Data Collection Rate**: 9,000 events/hour (baseline), 50,000+ (scaled)
- **API Success Rate**: >95%
- **System Availability**: >99.9%
- **Mean Time to Recovery (MTTR)**: <30 minutes
- **Database Query Performance**: <100ms for event retrieval

### 9.3 Business Metrics
- **Cost per 1M Events**: $0 (unauthenticated), $2-5 (with tokens/proxies)
- **Time to Detection (Secret Leaks)**: <5 minutes
- **User Adoption**: 10+ active users within 30 days
- **Data Quality**: <1% duplicate events, <0.1% corrupt events

---

## 10. Risk Assessment

### 10.1 Technical Risks

**Risk 1: GitHub API Rate Limit Exhaustion**
- **Probability**: Medium
- **Impact**: High (data collection stops)
- **Mitigation**: 
  - Multi-token rotation (Phase 2)
  - Auto-adjust rate limiter (implemented)
  - Proxy rotation (Phase 3)
  - Exponential backoff on 429

**Risk 2: Database Storage Exhaustion**
- **Probability**: Medium
- **Impact**: High (system crashes)
- **Mitigation**:
  - Partition tables by month
  - Archive old data to S3/cold storage
  - Set up disk usage alerts (>80%)
  - Implement data retention policies (90 days)

**Risk 3: Memory Leak in Rust Application**
- **Probability**: Low
- **Impact**: Medium (OOM crash)
- **Mitigation**:
  - Extensive testing with Valgrind/ASAN
  - Memory profiling with heaptrack
  - Auto-restart on high memory usage
  - Bounded queue sizes in event processing

### 10.2 Business Risks

**Risk 4: GitHub API Terms of Service Violation**
- **Probability**: Low
- **Impact**: Critical (account ban)
- **Mitigation**:
  - Stay within rate limits
  - Use official API, no scraping
  - Respect retry-after headers
  - Legal review of ToS compliance

**Risk 5: Data Privacy Concerns**
- **Probability**: Low
- **Impact**: High (regulatory fines)
- **Mitigation**:
  - Only collect public events
  - Implement GDPR data deletion API
  - Encrypt sensitive data at rest
  - Privacy policy and ToS

---

## 11. Future Roadmap

### Q4 2025
- [x] Phase 1: Basic event monitoring (COMPLETED)
- [ ] Database verification and dashboard fixes
- [ ] Phase 2: Multi-token rotation (5-10 tokens)
- [ ] Secret detection integration
- [ ] Basic analytics dashboard

### Q1 2026
- [ ] Phase 3: Proxy rotation for scale
- [ ] GraphQL API support
- [ ] WebSocket real-time streaming
- [ ] Alerting system (webhooks, email, Slack)
- [ ] Export to multiple formats (CSV, Parquet)

### Q2 2026
- [ ] Phase 4: Multi-region deployment
- [ ] Advanced analytics with ML anomaly detection
- [ ] Custom event filters and queries
- [ ] Public API with authentication
- [ ] Mobile app for monitoring

### Q3 2026
- [ ] Enterprise features: SSO, RBAC, audit logs
- [ ] SLA-backed uptime guarantees
- [ ] Compliance certifications (SOC 2, ISO 27001)
- [ ] Partner integrations (Datadog, Splunk)

---

## 12. Appendix

### 12.1 Glossary
- **Event**: A GitHub activity record (push, issue, PR, etc.)
- **Rate Limit**: Maximum API requests allowed per time period
- **Sliding Window**: Rate limiting algorithm that tracks requests in a time window
- **Adaptive Throttling**: Auto-adjust request rate based on API responses
- **Upsert**: INSERT or UPDATE operation (insert if not exists, ignore if exists)

### 12.2 References
- GitHub Events API: https://docs.github.com/en/rest/activity/events
- Rate Limiting: https://docs.github.com/en/rest/overview/resources-in-the-rest-api#rate-limiting
- PostgreSQL Best Practices: https://wiki.postgresql.org/wiki/Performance_Optimization
- Rust Async Book: https://rust-lang.github.io/async-book/

### 12.3 Related Documents
- `/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/README.md`
- `/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/IMPLEMENTATION_COMPLETE.md`
- `/home/bitloop/Documents/GITHUB/GitArchiver/to-do.md`

### 12.4 Change Log
- **2025-10-06 v1.0**: Initial PRD created after successful Phase 1 implementation
  - Core monitoring system operational
  - Rate limiter with auto-adjust working
  - Dashboard UI functional
  - Database integration complete
  - Known issues: Database verification pending, dashboard status accuracy

---

**Document Prepared By**: GitHub Copilot AI Assistant  
**Reviewed By**: Bitloop (Product Owner)  
**Next Review Date**: 2025-11-06  
**Distribution**: Engineering Team, Product Management, Stakeholders
