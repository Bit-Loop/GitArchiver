# 🎯 Quick Start: Fix GitHub Events API

## The Problem
Your webapp is **built for** https://api.github.com/events but:
- ❌ Events aren't being saved to database
- ❌ No rate limiting (hardcoded 10s polling)
- ❌ No 429 detection/handling
- ❌ No GUI controls

## The Good News
90% of the code already exists! You just need to **connect the pieces**.

## What Already Works ✅

### 1. Database Code (PERFECT!)
- **File**: `src/core/database.rs:325`
- **Function**: `insert_events_batch(events, filename)`
- **Status**: ✅ Fully functional, comprehensive, handles 67 fields
- **What it does**: Inserts events with ALL fields from your schema

### 2. Database Schema (PERFECT!)
- **File**: `schema.sql`
- **Table**: `github_events` with 90+ columns
- **Status**: ✅ Already matches GitHub Events API structure
- **Features**: JSONB payload, full actor/repo/org fields, indexes

### 3. GitHub Events Monitor (90% DONE!)
- **File**: `src/realtime/mod.rs` (700 lines)
- **Status**: ✅ Polls API, scans for secrets, webhook alerts
- **Missing**: Database connection, rate limiting

## What Needs Fixing ❌

### Fix #1: Connect Database (5 lines of code!)
```rust
// In src/realtime/mod.rs
pub struct GitHubEventMonitor {
    database: Option<Arc<Database>>,  // ADD THIS LINE
    // ... rest
}

// Add this method
async fn save_events_to_db(&self, events: &[GitHubEvent]) -> Result<()> {
    if let Some(db) = &self.database {
        let values: Vec<Value> = events.iter()
            .map(|e| serde_json::to_value(e)).collect()?;
        db.insert_events_batch(values, "github_events_api").await?;
    }
    Ok(())
}
```

### Fix #2: Add Rate Limiter (NEW FILE)
**Create**: `src/realtime/rate_limiter.rs`

**Purpose**: 
- Track requests per minute
- Enforce configurable limit (default 5/min)
- Detect 429 responses
- Auto-pause and resume
- Optional auto-adjust (reduce rate on 429)

**Size**: ~200 lines

### Fix #3: Handle 429 in poll_events()
```rust
// In poll_events() method
if response.status() == 429 {
    let retry_after = /* extract from header */;
    self.rate_limiter.handle_rate_limit_response(retry_after).await;
    return Ok(vec![]); // Will retry after pause
}
```

### Fix #4: Add API Endpoints (NEW FILE)
**Create**: `src/api/realtime_handlers.rs`

**Endpoints**:
- `POST /api/realtime/start` - Start monitor
- `POST /api/realtime/stop` - Stop monitor  
- `GET /api/realtime/status` - Get status
- `PUT /api/realtime/config` - Update rate limit

**Size**: ~150 lines

### Fix #5: Add Dashboard UI
**Edit**: `dashboard.html`

**Add**:
- New "GitHub Events" tab
- Rate limit slider (1-60 req/min)
- Auto-adjust checkbox
- Start/Stop buttons
- Status display

**Size**: ~200 lines HTML/JS

### Fix #6: Integrate into AppState
```rust
// src/api/state.rs
pub struct AppState {
    pub event_monitor: Arc<AsyncMutex<Option<GitHubEventMonitor>>>,  // ADD
    // ... rest
}
```

## Implementation Order

### Phase 1: Backend (4-6 hours)
1. ✅ Create `rate_limiter.rs` module
2. ✅ Update `realtime/mod.rs` with database & rate limiter
3. ✅ Create `realtime_handlers.rs` API endpoints
4. ✅ Add routes to `routes.rs`
5. ✅ Update `AppState` integration

### Phase 2: Frontend (2-3 hours)
1. ✅ Add GitHub Events tab to dashboard
2. ✅ Add control panel (slider, checkbox, buttons)
3. ✅ Add JavaScript event handlers
4. ✅ Add status polling (5s interval)

### Phase 3: Testing (2-3 hours)
1. ✅ Compile and run
2. ✅ Start monitor from UI
3. ✅ Verify database inserts
4. ✅ Test rate limiting
5. ✅ Trigger 429 (high rate), verify auto-pause
6. ✅ 24-hour stability test

**Total Time Estimate**: 8-12 hours

## Quick Verification Commands

### Check Database
```sql
-- See if events are being saved
SELECT COUNT(*), api_source FROM github_events GROUP BY api_source;

-- Recent events from API
SELECT event_type, actor_login, repo_name, processed_at 
FROM github_events 
WHERE api_source = 'github_events_api' 
ORDER BY processed_at DESC LIMIT 10;
```

### Check Logs
```bash
# Look for rate limit messages
tail -f logs/web_service.log | grep -i "rate"

# Look for 429 responses
tail -f logs/web_service.log | grep "429"
```

### Check API
```bash
# Get monitor status
curl -H "Authorization: Bearer $TOKEN" http://localhost:8081/api/realtime/status

# Start monitor
curl -X POST -H "Authorization: Bearer $TOKEN" http://localhost:8081/api/realtime/start
```

## Files to Create/Edit

### New Files (3)
1. `src/realtime/rate_limiter.rs` - Rate limiting logic
2. `src/api/realtime_handlers.rs` - API endpoint handlers  
3. (Optional) `src/realtime/mod.rs` - Add `pub mod rate_limiter;`

### Files to Edit (5)
1. `src/realtime/mod.rs` - Add database, rate limiter integration
2. `src/api/state.rs` - Add event_monitor to AppState
3. `src/api/routes.rs` - Add realtime routes
4. `src/api/handlers.rs` - Export realtime handlers
5. `dashboard.html` - Add GitHub Events tab & controls

**Total**: ~800-1000 lines of new code (mostly UI)

## Success Criteria

✅ **You know it's working when**:
1. Dashboard shows "GitHub Events" tab
2. Click "Start" button - monitor starts
3. Database query shows events: `SELECT COUNT(*) FROM github_events WHERE api_source = 'github_events_api'`
4. Logs show: "Rate limited - waiting..." messages
5. Slider changes rate - logs reflect new timing
6. Auto-adjust checkbox works - rate reduces on 429
7. No crashes for 24 hours

## Common Issues & Solutions

### Issue: "Events not in database"
**Check**: 
- Is monitor running? (check status endpoint)
- Is database connected? (check AppState initialization)
- Any errors in logs? (check insert_events_batch calls)

### Issue: "Too many 429 responses"
**Solution**: 
- Reduce rate limit (slider to 3-5 req/min)
- Enable auto-adjust checkbox
- Check GitHub token has proper permissions

### Issue: "Duplicate events"
**Not a problem**: Database has `ON CONFLICT (event_id) DO NOTHING` - duplicates ignored

### Issue: "Rate limit not enforcing"
**Check**:
- rate_limiter.wait_if_needed() called before each request?
- Logs showing "Rate limit reached" messages?
- Slider value propagating to backend?

## Next Steps (AFTER Core Works)

Once you have:
- ✅ Events in database
- ✅ Rate limiting working
- ✅ 429 handling proven
- ✅ 24-hour stability

**THEN** consider:
1. Multi-source API support
2. CSV/JSON/XML parsers
3. Dynamic schema detection
4. Per-source rate limiting
5. Separate threads per source

But **DON'T** touch those until the core works 100%!

## Documentation References

- **Full Analysis**: `GITHUB_EVENTS_API_ANALYSIS.md`
- **Task Checklist**: `IMPLEMENTATION_CHECKLIST.md`
- **Database Schema**: `schema.sql`
- **Existing Code**: 
  - Database: `src/core/database.rs`
  - Monitor: `src/realtime/mod.rs`
  - API: `src/api/handlers.rs`

---

## 🎯 TL;DR

**Problem**: GitHub Events API not saving to DB, no rate limiting, no UI
**Solution**: Connect existing database code, add rate limiter, add UI controls
**Effort**: ~1000 lines code, 8-12 hours work
**Result**: Fully functional GitHub Events scraping with rate limiting & GUI

**Start here**: Create `src/realtime/rate_limiter.rs` with rate limiting logic
