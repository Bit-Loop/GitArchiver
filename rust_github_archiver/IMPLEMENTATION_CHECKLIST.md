# GitHub Events API - Implementation Checklist

## 🎯 Goal
Make https://api.github.com/events scraping work with:
- ✅ Database persistence (code exists!)
- ✅ Rate limiting (5 req/min, configurable)
- ✅ 429 detection & auto-pause/resume
- ✅ GUI controls (slider, auto-adjust checkbox)

---

## 📋 Implementation Tasks

### ✅ Phase 0: Verification (BEFORE coding)
- [x] Confirm `Database::insert_events_batch()` exists - **YES!** (src/core/database.rs:325)
- [x] Confirm `github_events` table schema - **YES!** (schema.sql, 90+ columns)
- [x] Confirm `GitHubEventMonitor` exists - **YES!** (src/realtime/mod.rs, 700 lines)
- [x] Identify gaps - **Database not connected, no rate limiting, no GUI**

---

### 🔧 Phase 1: Core Fixes (Required)

#### Task 1.1: Create Rate Limiter Module
**File**: `src/realtime/rate_limiter.rs` (NEW FILE)

**What to implement**:
```rust
pub struct AdaptiveRateLimiter {
    // Configurable rate (default 5 req/min)
    // Request history (sliding window)
    // Auto-adjust flag
    // Pause state (when 429 hit)
}

pub async fn wait_if_needed() -> Result<()>
pub async fn handle_rate_limit_response(retry_after: Option<u64>)
pub async fn set_rate(requests_per_minute: u32)
pub async fn set_auto_adjust(enabled: bool)
pub async fn get_status() -> RateLimitStatus
```

**Acceptance Criteria**:
- [ ] Enforces configurable req/min limit
- [ ] Tracks requests in 60-second sliding window
- [ ] Auto-pauses when 429 received
- [ ] Auto-resumes after retry period
- [ ] Optionally reduces rate by 20% on 429

---

#### Task 1.2: Connect Database to GitHubEventMonitor
**File**: `src/realtime/mod.rs`

**Changes needed**:
1. Add `database: Option<Arc<Database>>` field
2. Add `with_database()` method
3. Add `save_events_to_db()` that calls `Database::insert_events_batch()`
4. Update `process_events()` to save before scanning

**Code snippet**:
```rust
async fn process_events(&self, events: Vec<GitHubEvent>) -> Result<()> {
    // BATCH SAVE TO DATABASE
    self.save_events_to_db(&events).await?;
    
    // Then scan for secrets
    {
        let mut queue = self.processing_queue.write().await;
        queue.extend(events);
    }
    self.process_queue().await?;
    Ok(())
}
```

**Acceptance Criteria**:
- [ ] Events saved to `github_events` table
- [ ] Uses existing `Database::insert_events_batch()`
- [ ] Sets `api_source = 'github_events_api'`
- [ ] Batch insertion for efficiency
- [ ] No duplicates (ON CONFLICT handled in DB code)

---

#### Task 1.3: Add Rate Limiting to poll_events()
**File**: `src/realtime/mod.rs`

**Changes**:
```rust
async fn poll_events(&self) -> Result<Vec<GitHubEvent>> {
    // WAIT FOR RATE LIMITER
    self.rate_limiter.wait_if_needed().await?;
    
    let response = self.client.get("https://api.github.com/events")
        .send().await?;

    // CHECK FOR 429
    if response.status() == 429 {
        let retry_after = response.headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        
        self.rate_limiter.handle_rate_limit_response(retry_after).await;
        return Ok(vec![]); // Return empty, will retry later
    }
    
    // ... rest of logic
}
```

**Acceptance Criteria**:
- [ ] Respects rate limit before each request
- [ ] Detects 429 status code
- [ ] Extracts `Retry-After` header
- [ ] Auto-pauses on rate limit
- [ ] Logs rate limit events

---

#### Task 1.4: Integrate into AppState
**File**: `src/api/state.rs`

**Add field**:
```rust
pub struct AppState {
    // ... existing fields
    pub event_monitor: Arc<AsyncMutex<Option<GitHubEventMonitor>>>,
}
```

**Add initialization method**:
```rust
pub async fn initialize_event_monitor(&self, github_token: &str) -> Result<()> {
    let monitor = GitHubEventMonitor::new(github_token).await?
        .with_database(self.database.clone());
    
    *self.event_monitor.lock().await = Some(monitor);
    Ok(())
}
```

**Acceptance Criteria**:
- [ ] `event_monitor` field added to `AppState`
- [ ] Initialization method works
- [ ] Database connection passed correctly
- [ ] Compiles without errors

---

### 🌐 Phase 2: API Endpoints (Required)

#### Task 2.1: Create Realtime Handlers
**File**: `src/api/realtime_handlers.rs` (NEW FILE)

**Endpoints to implement**:
1. `POST /api/realtime/start` - Start monitoring
2. `POST /api/realtime/stop` - Stop monitoring
3. `GET /api/realtime/status` - Get status & rate limit info
4. `PUT /api/realtime/config` - Update rate limit config

**Data structures**:
```rust
#[derive(Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub auto_adjust: bool,
}

#[derive(Serialize)]
pub struct MonitorStatus {
    pub running: bool,
    pub rate_limit: RateLimitStatus,
    pub events_processed: u64,
    pub last_event_time: Option<DateTime<Utc>>,
}
```

**Acceptance Criteria**:
- [ ] All 4 endpoints implemented
- [ ] Proper error handling
- [ ] JSON responses
- [ ] JWT authentication required

---

#### Task 2.2: Add Routes
**File**: `src/api/routes.rs`

**Add to router**:
```rust
.route("/api/realtime/start", post(start_event_monitor))
.route("/api/realtime/stop", post(stop_event_monitor))
.route("/api/realtime/status", get(get_event_monitor_status))
.route("/api/realtime/config", put(update_rate_limit))
```

**Acceptance Criteria**:
- [ ] Routes accessible via HTTP
- [ ] Protected by authentication middleware
- [ ] Returns proper status codes

---

### 🎨 Phase 3: Dashboard UI (Required)

#### Task 3.1: Add GitHub Events Tab
**File**: `dashboard.html`

**Navigation item**:
```html
<li class="nav-item">
    <a class="nav-link" id="events-tab" data-bs-toggle="tab" href="#events">
        <i class="bi bi-broadcast"></i> GitHub Events
    </a>
</li>
```

**Tab content with**:
- Control panel (Start/Stop buttons)
- Rate limit slider (1-60 req/min)
- Auto-adjust checkbox
- Status display (running, requests/min, events processed)
- Rate limit indicator (is paused?, retry after?)

**Acceptance Criteria**:
- [ ] New tab visible in navigation
- [ ] All controls functional
- [ ] Real-time status updates (5s interval)
- [ ] Visual feedback on actions

---

#### Task 3.2: Add JavaScript Controls
**File**: `dashboard.html` (in `<script>` section)

**Functions to implement**:
```javascript
async function startEventMonitor() { /* POST /api/realtime/start */ }
async function stopEventMonitor() { /* POST /api/realtime/stop */ }
async function updateRateLimit(rate, autoAdjust) { /* PUT /api/realtime/config */ }
async function updateEventMonitorStatus() { /* GET /api/realtime/status */ }
setInterval(updateEventMonitorStatus, 5000); // Poll every 5s
```

**Acceptance Criteria**:
- [ ] Slider updates rate limit in real-time
- [ ] Auto-adjust checkbox toggles setting
- [ ] Status updates automatically
- [ ] Notifications on success/error
- [ ] Proper error handling

---

### ✅ Phase 4: Testing (Critical)

#### Task 4.1: Manual Testing
- [ ] Start event monitor via UI
- [ ] Verify events appear in `github_events` table
- [ ] Change rate limit via slider - verify enforcement
- [ ] Enable auto-adjust - verify 429 handling
- [ ] Stop/start - verify state management
- [ ] Check logs for rate limit messages

#### Task 4.2: Database Verification
```sql
-- Check events are being saved
SELECT COUNT(*), api_source FROM github_events 
WHERE api_source = 'github_events_api' 
GROUP BY api_source;

-- Check recent events
SELECT event_id, event_type, actor_login, repo_name, processed_at 
FROM github_events 
WHERE api_source = 'github_events_api' 
ORDER BY processed_at DESC 
LIMIT 20;

-- Check event distribution
SELECT event_type, COUNT(*) as count 
FROM github_events 
WHERE api_source = 'github_events_api' 
GROUP BY event_type 
ORDER BY count DESC;
```

#### Task 4.3: Rate Limiting Verification
- [ ] Set rate to 5 req/min, monitor logs - should request every ~12s
- [ ] Intentionally trigger 429 (set rate to 100 req/min temporarily)
- [ ] Verify auto-pause message in logs
- [ ] Verify resume after retry period
- [ ] Verify rate reduction if auto-adjust enabled

---

## 🚀 Deployment Checklist

### Before Running
- [ ] Database schema applied (`schema.sql`)
- [ ] GitHub token configured (`.env` or config)
- [ ] Web server compiled (`cargo build --bin web_server`)

### Startup Sequence
1. [ ] Start PostgreSQL
2. [ ] Run database migrations (if any)
3. [ ] Start web server
4. [ ] Access dashboard (http://localhost:8081)
5. [ ] Initialize event monitor from UI

### Monitoring
- [ ] Check logs for rate limit warnings
- [ ] Monitor database growth
- [ ] Verify no crashes on 429 responses
- [ ] Check for duplicate events (should be prevented)

---

## 📊 Success Metrics

### Functional Requirements
✅ Events from https://api.github.com/events saved to database
✅ Rate limiting enforced (configurable 1-60 req/min)
✅ 429 detection and auto-pause/resume
✅ GUI controls work (slider, checkbox, buttons)
✅ No crashes or errors in logs

### Performance Requirements
✅ Batch database inserts (efficient)
✅ No duplicate events (DB constraint)
✅ Graceful 429 handling (no API bans)
✅ Real-time UI updates (<5s latency)

### Code Quality
✅ Compiles without warnings
✅ Follows existing code style
✅ Reuses existing database code
✅ Proper error handling throughout
✅ Comprehensive logging

---

## 🔮 Future Enhancements (AFTER core works)

Once GitHub Events API is **proven to work reliably**, consider:

1. **Multi-Source Support**
   - Generic API source manager
   - CSV/JSON/XML parsers
   - Per-source configuration
   - Separate threads per source

2. **Advanced Features**
   - Event replay from archive
   - Custom webhook filters
   - Event aggregation/analytics
   - Historical gap detection

3. **Optimizations**
   - Incremental updates only
   - Deduplication before insert
   - Compression for old events
   - Partitioning by date

**BUT**: Don't touch these until core GitHub Events API works 100%!

---

## 📝 Notes

### Why Batch Inserts?
- GitHub Events API returns up to 300 events per request
- Batch insert is 10-100x faster than individual inserts
- Existing code already handles this efficiently

### Why Adaptive Rate Limiting?
- GitHub API has rate limits (5000/hour for authenticated)
- Events endpoint might have different limits
- Auto-adjust prevents API bans
- Manual control allows tuning

### Why Database Reuse?
- Don't reinvent the wheel
- `Database::insert_events_batch()` already exists
- Handles transactions, validation, conflicts
- Comprehensive field mapping (67 parameters!)

---

## ⚠️ Common Pitfalls to Avoid

1. **Don't** create new database insertion code - use existing!
2. **Don't** poll too fast - respect rate limits
3. **Don't** ignore 429 responses - handle gracefully
4. **Don't** start new features before core works
5. **Don't** forget to test with real API

---

## 🎯 Definition of Done

GitHub Events API scraping is **DONE** when:

- [ ] ✅ Events visible in database (`SELECT * FROM github_events WHERE api_source = 'github_events_api'`)
- [ ] ✅ Rate limiting enforced (log messages show waiting)
- [ ] ✅ 429 responses handled (no crashes, auto-pause/resume)
- [ ] ✅ GUI controls functional (slider, checkbox, buttons work)
- [ ] ✅ No errors in logs for 24-hour run
- [ ] ✅ All acceptance criteria met
- [ ] ✅ Code review passed
- [ ] ✅ Documentation updated

**ONLY THEN** move to multi-source extensibility!
