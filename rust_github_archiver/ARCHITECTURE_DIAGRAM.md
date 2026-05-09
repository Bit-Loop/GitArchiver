# GitHub Events API - System Architecture

## Current State vs Target State

### CURRENT (Broken) 🔴
```
GitHub Events API (https://api.github.com/events)
    ↓
GitHubEventMonitor (src/realtime/mod.rs)
    ↓
Secret Scanner ← ONLY scans for secrets
    ↓
Webhooks/Alerts
    
❌ NO DATABASE STORAGE
❌ NO RATE LIMITING
❌ NO GUI CONTROLS
```

### TARGET (Working) 🟢
```
                   GitHub Events API
                   (https://api.github.com/events)
                            ↓
                   [Rate Limiter] ← 429 Detection
                            ↓
              GitHubEventMonitor (polling)
                     ↙          ↘
            Database          Secret Scanner
            (batch)           (parallel)
              ↓                    ↓
        github_events          Webhooks
         table                 /Alerts
              
    ↑ Controlled by ↑
    
    Dashboard UI → API Endpoints → AppState
    (slider, btns)   (/api/realtime/*)  (event_monitor)
```

## Data Flow

### 1. Startup Flow
```
User clicks "Start" in Dashboard
    ↓
POST /api/realtime/start
    ↓
AppState.event_monitor
    ↓
GitHubEventMonitor.start_monitoring()
    ↓
[Background Task Spawned]
```

### 2. Event Polling Loop
```
[Every N seconds based on rate limit]
    ↓
RateLimiter.wait_if_needed() ← Enforces req/min limit
    ↓
HTTP GET https://api.github.com/events
    ↓
[Check Response Status]
    ├─ 200 OK → Process events
    ├─ 429 Rate Limited → Auto-pause + optional rate reduction
    └─ Other errors → Log and retry
```

### 3. Event Processing Pipeline
```
Received Events (up to 300)
    ↓
[Batch Operation]
    ├─ Convert to serde_json::Value
    └─ Call Database.insert_events_batch()
        ↓
    [Transaction]
        ├─ Validate each event
        ├─ Insert with 67 parameters
        └─ ON CONFLICT (event_id) DO NOTHING
            ↓
        Commit
    
[Parallel Operation]
    └─ Secret Scanning
        ├─ PushEvent → Check dangling commits
        ├─ PullRequestEvent → Scan PR text
        ├─ IssueCommentEvent → Scan comment
        └─ ReleaseEvent → Scan release notes
            ↓
        Create Alerts → Send Webhooks
```

### 4. Rate Limit Handling
```
Request Made
    ↓
Response Received
    ↓
[Status Check]
    ├─ 200 → Record request time, continue
    └─ 429 → Extract Retry-After header
            ↓
        RateLimiter.handle_rate_limit_response()
            ├─ Set paused_until timestamp
            ├─ Log warning
            └─ [If auto_adjust enabled]
                    ↓
                Reduce rate by 20%
                (e.g., 5 req/min → 4 req/min)
            ↓
        Next request waits until pause expires
```

### 5. Dashboard Control Flow
```
User moves slider (e.g., 10 req/min)
    ↓
JavaScript: rate-limit-slider.onChange()
    ↓
PUT /api/realtime/config
    Body: { requests_per_minute: 10, auto_adjust: true }
    ↓
RateLimiter.set_rate(10)
    ↓
Future requests use new rate
    
[Status Updates - Every 5 seconds]
    ↓
GET /api/realtime/status
    ↓
Returns: {
    running: true,
    rate_limit: {
        requests_per_minute: 10,
        requests_last_minute: 7,
        is_paused: false,
        auto_adjust_enabled: true
    },
    events_processed: 1523
}
    ↓
Update Dashboard UI
```

## Component Interactions

### GitHubEventMonitor
**Owns**:
- HTTP Client (reqwest)
- Rate Limiter (AdaptiveRateLimiter)
- Database reference (Arc<Database>)
- Secret Scanner
- Dangling Commit Fetcher

**Responsibilities**:
1. Poll GitHub Events API
2. Enforce rate limits
3. Handle 429 responses
4. Save events to database
5. Scan for secrets
6. Send alerts

### AdaptiveRateLimiter
**State**:
- `requests_per_minute: u32` - Configurable limit
- `last_request_times: Vec<Instant>` - Sliding window (60s)
- `auto_adjust: bool` - Auto-reduce rate on 429?
- `paused_until: Option<Instant>` - Pause state
- `retry_after: Option<Duration>` - From 429 response

**Methods**:
1. `wait_if_needed()` - Block until rate allows
2. `handle_rate_limit_response()` - Process 429
3. `set_rate()` - Update limit
4. `get_status()` - Current state

### Database (Existing!)
**Methods Used**:
- `insert_events_batch(events, filename)` - Batch insert
  - Takes `Vec<serde_json::Value>`
  - Returns `Result<i64>` (rows inserted)
  - Handles validation, transactions, conflicts

### AppState
**Holds**:
```rust
pub struct AppState {
    pub config: Config,
    pub scraper_manager: Arc<ScraperManager>,        // For GHArchive
    pub main_scraper: Arc<Mutex<Option<MainScraper>>>, // For GHArchive
    pub event_monitor: Arc<AsyncMutex<Option<GitHubEventMonitor>>>, // NEW!
    pub database: Arc<Database>,
    // ...
}
```

**Methods**:
- `initialize_event_monitor(github_token)` - Setup monitor
- `get_event_monitor_status()` - Query status

## Database Schema

### github_events Table (Existing!)
```sql
CREATE TABLE github_events (
    -- Core (4 fields)
    event_id BIGINT PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    event_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    event_public BOOLEAN NOT NULL DEFAULT true,
    
    -- Actor (20 fields)
    actor_id BIGINT,
    actor_login VARCHAR(255),
    actor_display_login VARCHAR(255),
    -- ... 17 more actor fields
    
    -- Repository (30 fields)
    repo_id BIGINT,
    repo_name VARCHAR(255),
    repo_url TEXT,
    -- ... 27 more repo fields
    
    -- Organization (9 fields - optional)
    org_id BIGINT,
    org_login VARCHAR(255),
    -- ... 7 more org fields
    
    -- Storage (4 fields)
    payload JSONB,           -- Full event payload
    raw_event JSONB,         -- Raw event data
    file_source VARCHAR(255), -- "github_events_api"
    api_source VARCHAR(255),  -- "github_events_api"
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_github_events_created_at ON github_events (event_created_at);
CREATE INDEX idx_github_events_type ON github_events (event_type);
CREATE INDEX idx_github_events_actor_id ON github_events (actor_id);
CREATE INDEX idx_github_events_repo_id ON github_events (repo_id);
CREATE INDEX idx_github_events_payload ON github_events USING GIN (payload);
```

## API Endpoints

### Existing Endpoints (Already Working)
- `POST /api/auth/login` - Login
- `GET /api/scraper/status` - GHArchive scraper status
- `GET /api/monitoring/overview` - System metrics
- `GET /api/secrets/list` - List secrets

### NEW Endpoints (To Implement)
```
POST /api/realtime/start
    ↓
    Body: (none)
    Auth: Required (JWT)
    ↓
    Returns: { status: "success", message: "Event monitoring started" }
    
POST /api/realtime/stop
    ↓
    Body: (none)
    Auth: Required
    ↓
    Returns: { status: "success", message: "Event monitoring stopped" }
    
GET /api/realtime/status
    ↓
    Body: (none)
    Auth: Required
    ↓
    Returns: {
        running: bool,
        rate_limit: {
            requests_per_minute: u32,
            requests_last_minute: u32,
            auto_adjust_enabled: bool,
            is_paused: bool,
            retry_after: Option<Duration>
        },
        events_processed: u64,
        last_event_time: Option<DateTime<Utc>>
    }
    
PUT /api/realtime/config
    ↓
    Body: {
        requests_per_minute: u32,  // 1-60
        auto_adjust: bool
    }
    Auth: Required
    ↓
    Returns: { status: "success", config: {...} }
```

## Dashboard UI Layout

```
+----------------------------------------------------------+
|  [Overview] [Secrets] [Logs] [Monitoring] [GitHub Events]| ← Tabs
+----------------------------------------------------------+
|                                                           |
|  +----------------------+  +---------------------------+  |
|  | Control Panel        |  | Monitor Status            |  |
|  +----------------------+  +---------------------------+  |
|  | [▶ Start] [⏸ Stop]  |  | Status: Running ✅        |  |
|  |                      |  | Requests/min: 4 / 5       |  |
|  | Rate Limit: 5 ━━━━━○|  | Rate Limited: No          |  |
|  | (1────────────────60)|  | Events Processed: 1,523   |  |
|  |                      |  | Last Event: 2s ago        |  |
|  | ☑ Auto-adjust rate   |  |                           |  |
|  +----------------------+  +---------------------------+  |
|                                                           |
|  +----------------------------------------------------+   |
|  | Recent Events                                      |   |
|  +----------------------------------------------------+   |
|  | PushEvent   | user123 | repo/name    | 5s ago     |   |
|  | CreateEvent | user456 | another/repo | 12s ago    |   |
|  | PullRequest | user789 | cool/project | 18s ago    |   |
|  +----------------------------------------------------+   |
+----------------------------------------------------------+
```

## Rate Limiting Logic

### Sliding Window Algorithm
```
Time:     0s    12s   24s   36s   48s   60s   72s
          |     |     |     |     |     |     |
Requests: R1    R2    R3    R4    R5    R1✗   R6

Rate Limit: 5 req/min

At 60s:
- Window: [R1, R2, R3, R4, R5] (5 requests)
- New request R6 → WAIT until R1 expires
- Wait time: 60s - (60s - 0s) = 0s + buffer

At 61s:
- Window: [R2, R3, R4, R5] (4 requests, R1 dropped)
- New request R6 → ALLOWED
- Record time: 61s
```

### 429 Response Handling
```
Request at time T
    ↓
Response: 429 Too Many Requests
Headers: {
    "Retry-After": "60",  ← Seconds to wait
    "X-RateLimit-Remaining": "0"
}
    ↓
RateLimiter actions:
1. paused_until = now() + 60s
2. Log: "Rate limited - waiting 60s"
3. [If auto_adjust]
       current_rate = 5
       new_rate = 5 * 0.8 = 4
       Log: "Auto-adjusting rate: 5 → 4 req/min"
    ↓
Next request:
    wait_if_needed() checks:
        if now() < paused_until:
            sleep(paused_until - now())
```

## Error Handling

### Request Failures
```rust
match poll_events().await {
    Ok(events) => {
        save_events_to_db(&events).await?;
        process_for_secrets(&events).await?;
    }
    Err(e) if e.status() == 429 => {
        // Handle rate limit
        handle_rate_limit_response().await;
    }
    Err(e) if e.status() == 401 => {
        // Invalid GitHub token
        error!("GitHub token invalid");
        stop_monitoring();
    }
    Err(e) => {
        // Network/other errors
        error!("Poll failed: {}", e);
        // Continue - will retry next interval
    }
}
```

### Database Failures
```rust
match db.insert_events_batch(events, "github_events_api").await {
    Ok(count) => {
        info!("Inserted {} events", count);
    }
    Err(e) => {
        error!("Database insert failed: {}", e);
        // Don't crash - events will be fetched again on next poll
        // Duplicate detection via ON CONFLICT prevents duplication
    }
}
```

## Performance Considerations

### Batch Processing
- GitHub Events API returns ~300 events per request
- Batch insert is 100x faster than individual inserts
- Single transaction for atomicity

### Rate Limiting
- 5 req/min = 1 request every 12 seconds
- 300 events/request × 5 req/min = **1500 events/min**
- ~2.16M events/day (theoretical max)

### Database Growth
- Average event size: ~2 KB (with JSONB)
- 1500 events/min × 60 min × 24 hours = 2.16M events/day
- 2.16M × 2 KB = **~4.3 GB/day**
- Need: Database cleanup/archiving strategy

### Memory Usage
- Event buffer: 300 events × 2 KB = 600 KB
- Minimal memory footprint
- Background task doesn't block main thread

## Security Considerations

### GitHub Token
- Required for higher rate limits (5000/hour vs 60/hour)
- Should be stored securely (environment variable)
- Should have minimal permissions (read-only)

### API Endpoints
- All endpoints require JWT authentication
- Rate limit config changes logged
- User actions audited

### Database
- Event IDs prevent duplicates
- JSONB allows querying sensitive data
- Consider PII in public repos

## Monitoring & Observability

### Logs to Watch
```
INFO: Starting GitHub Events API monitoring
INFO: Rate limit reached (5/min) - waiting 8.3s
WARN: Rate limited by GitHub API! Retry-After: 60s
WARN: Auto-adjusting rate limit: 5 → 4 req/min
INFO: Inserted 287 events from github_events_api
ERROR: Failed to insert event 12345: duplicate key
```

### Metrics to Track
- Events processed per minute
- Database insert success rate
- Rate limit violations (429 responses)
- Average response time
- Error rate

### Alerts to Set
- 🚨 Critical: Event monitor stopped unexpectedly
- ⚠️ Warning: High rate of 429 responses (>10% of requests)
- ℹ️ Info: Database insert failures
- 📊 Metric: Events processed today

## Testing Strategy

### Unit Tests
- ✅ Rate limiter enforcement
- ✅ 429 response handling
- ✅ Auto-adjust logic
- ✅ Sliding window calculation

### Integration Tests
- ✅ Database batch insert
- ✅ Event validation
- ✅ Duplicate detection
- ✅ API endpoint responses

### Manual Tests
1. Start monitor → verify database inserts
2. Set rate to 60 req/min → trigger 429 → verify auto-pause
3. Enable auto-adjust → verify rate reduction
4. Stop/start monitor → verify state management
5. 24-hour stability test → verify no crashes

## Deployment Checklist

- [ ] PostgreSQL running
- [ ] Database schema applied (`schema.sql`)
- [ ] GitHub token configured
- [ ] Web server compiled
- [ ] Dashboard accessible
- [ ] Event monitor initialized
- [ ] Database growth monitored
- [ ] Logs rotating properly

---

## Summary

**Key Points**:
1. 90% of code already exists (database, schema, monitor base)
2. Just need to connect pieces (database + rate limiter)
3. Add UI controls for user interaction
4. Test with real GitHub Events API
5. Monitor for 24 hours before considering "done"

**Critical Path**:
1. Create rate limiter (~200 lines)
2. Connect database (~50 lines)
3. Add API endpoints (~150 lines)
4. Add UI controls (~200 lines)
5. Test thoroughly

**Estimated Total Work**: ~600 new lines + ~200 lines modifications = **800 lines total**

**Time Estimate**: 8-12 hours (including testing)
