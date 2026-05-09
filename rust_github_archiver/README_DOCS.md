# 📚 Documentation Index - GitHub Events API Fix

## 🎯 Start Here

**If you're new to this codebase**: Read `QUICK_START_FIX.md`

**If you want to understand the architecture**: Read `ARCHITECTURE_DIAGRAM.md`

**If you want detailed task list**: Read `IMPLEMENTATION_CHECKLIST.md`

**If you want comprehensive analysis**: Read `GITHUB_EVENTS_API_ANALYSIS.md`

---

## 📄 Document Descriptions

### QUICK_START_FIX.md
**Purpose**: Get you fixing the problem FAST

**Contains**:
- Problem summary (events not saving, no rate limiting)
- Good news (90% already exists!)
- 6 specific fixes needed
- Verification commands
- Success criteria

**Best for**: Getting started immediately

**Time to read**: 5-10 minutes

---

### ARCHITECTURE_DIAGRAM.md
**Purpose**: Understand the system architecture

**Contains**:
- Current state vs target state diagrams
- Data flow visualizations
- Component interactions
- Database schema details
- API endpoint specifications
- UI layout mockups
- Rate limiting algorithm explanations
- Error handling strategies

**Best for**: Understanding how everything fits together

**Time to read**: 15-20 minutes

---

### IMPLEMENTATION_CHECKLIST.md
**Purpose**: Track your implementation progress

**Contains**:
- Detailed task breakdown (Phases 1-4)
- Acceptance criteria for each task
- File-by-file changes needed
- Testing procedures
- SQL verification queries
- Deployment checklist
- Definition of "done"

**Best for**: Methodical implementation tracking

**Time to read**: 20-30 minutes (reference document)

---

### GITHUB_EVENTS_API_ANALYSIS.md
**Purpose**: Deep dive into current state and solution

**Contains**:
- Comprehensive current state analysis
- Detailed gap identification
- Step-by-step implementation plan
- Code snippets for each component
- Phase 1 (Core) and Phase 2 (Future) breakdown
- Summary of problems and solutions

**Best for**: Understanding WHY each change is needed

**Time to read**: 30-40 minutes (reference document)

---

## 🚀 Recommended Reading Order

### For Immediate Implementation
1. **QUICK_START_FIX.md** (5 min) - Get the overview
2. **IMPLEMENTATION_CHECKLIST.md** (skim, 10 min) - See the tasks
3. Start coding!
4. Refer to **ARCHITECTURE_DIAGRAM.md** when confused

### For Deep Understanding
1. **GITHUB_EVENTS_API_ANALYSIS.md** (30 min) - Understand current state
2. **ARCHITECTURE_DIAGRAM.md** (20 min) - See how it should work
3. **IMPLEMENTATION_CHECKLIST.md** (reference) - Track progress
4. **QUICK_START_FIX.md** (reference) - Quick lookups

---

## 📊 Quick Reference

### File Locations
```
rust_github_archiver/
├── src/
│   ├── realtime/
│   │   ├── mod.rs              ← Edit: Add DB + rate limiter
│   │   └── rate_limiter.rs     ← NEW: Rate limiting logic
│   ├── api/
│   │   ├── state.rs            ← Edit: Add event_monitor
│   │   ├── routes.rs           ← Edit: Add realtime routes
│   │   ├── handlers.rs         ← Edit: Export realtime handlers
│   │   └── realtime_handlers.rs ← NEW: API endpoints
│   └── core/
│       └── database.rs         ← EXISTING: insert_events_batch()
├── dashboard.html              ← Edit: Add GitHub Events tab
└── schema.sql                  ← EXISTING: github_events table
```

### Key Functions
```rust
// EXISTING (Don't reimplement!)
Database::insert_events_batch(events, filename) -> Result<i64>

// TO IMPLEMENT
AdaptiveRateLimiter::wait_if_needed() -> Result<()>
AdaptiveRateLimiter::handle_rate_limit_response(retry: Option<u64>)
GitHubEventMonitor::save_events_to_db(events) -> Result<()>

// API HANDLERS TO CREATE
start_event_monitor(State) -> Json<Value>
stop_event_monitor(State) -> Json<Value>
get_event_monitor_status(State) -> Json<Value>
update_rate_limit(State, Json<RateLimitConfig>) -> Json<Value>
```

### API Endpoints to Add
```
POST   /api/realtime/start    - Start monitoring
POST   /api/realtime/stop     - Stop monitoring
GET    /api/realtime/status   - Get status
PUT    /api/realtime/config   - Update rate limit
```

### Dashboard Components to Add
```html
<!-- Navigation -->
<li class="nav-item">
    <a href="#events">GitHub Events</a>
</li>

<!-- Tab Content -->
<div id="events">
    <button id="start-events-btn">Start</button>
    <input type="range" id="rate-limit-slider" min="1" max="60">
    <input type="checkbox" id="auto-adjust-checkbox">
    <div id="monitor-status">Status: ...</div>
</div>
```

### Verification Commands
```sql
-- Check events in database
SELECT COUNT(*), api_source FROM github_events GROUP BY api_source;

-- Recent events
SELECT event_type, actor_login, repo_name, processed_at 
FROM github_events 
WHERE api_source = 'github_events_api' 
ORDER BY processed_at DESC LIMIT 10;
```

```bash
# Check logs for rate limiting
tail -f logs/web_service.log | grep -i "rate"

# Test API endpoints
curl -H "Authorization: Bearer $TOKEN" \
     http://localhost:8081/api/realtime/status
```

---

## 🎓 Key Concepts Explained

### Why Batch Inserts?
- GitHub Events API returns ~300 events per request
- Batch insert = **100x faster** than individual inserts
- Existing `Database::insert_events_batch()` handles this

### Why Adaptive Rate Limiting?
- GitHub API has rate limits (5000/hour authenticated)
- 429 responses = temporary ban risk
- Adaptive limiting prevents bans
- Auto-adjust reduces rate on 429 (optional)

### Why Sliding Window?
- More accurate than fixed intervals
- Allows burst requests if available
- Example: 5 req/min = can do 5 requests in first 10s, then wait

### Why Reuse Database Code?
- `insert_events_batch()` already exists
- Handles 67 parameters correctly
- Includes transaction management
- Prevents duplicates via `ON CONFLICT`

---

## ⚠️ Common Pitfalls

1. **DON'T** create new database insertion code
   - ✅ Use `Database::insert_events_batch()`
   
2. **DON'T** poll too fast
   - ✅ Respect rate limits (default 5 req/min)
   
3. **DON'T** ignore 429 responses
   - ✅ Handle with auto-pause/resume
   
4. **DON'T** start new features before core works
   - ✅ Focus on GitHub Events API first
   
5. **DON'T** forget to test with real API
   - ✅ Use actual https://api.github.com/events

---

## ✅ Definition of Done

GitHub Events API is **COMPLETE** when:

1. ✅ Events visible in database
   ```sql
   SELECT COUNT(*) FROM github_events 
   WHERE api_source = 'github_events_api';
   -- Should show > 0
   ```

2. ✅ Rate limiting enforced
   ```bash
   # Logs show waiting messages
   tail -f logs/web_service.log | grep "waiting"
   ```

3. ✅ 429 handling works
   ```bash
   # Set rate to 60 req/min, trigger 429, verify auto-pause
   ```

4. ✅ GUI controls functional
   ```
   - Slider changes rate
   - Auto-adjust checkbox toggles setting
   - Start/Stop buttons work
   - Status updates in real-time
   ```

5. ✅ 24-hour stability test passes
   ```
   No crashes, consistent event collection
   ```

**ONLY THEN** move to multi-source extensibility!

---

## 🔮 Future Work (After Core)

Once GitHub Events API works **perfectly**:

### Phase 2A: Additional GitHub Endpoints
- `/repos/{owner}/{repo}/events` - Repo-specific events
- `/orgs/{org}/events` - Organization events
- `/users/{user}/events` - User events

### Phase 2B: Multi-Source Framework
- Generic API source manager
- CSV/JSON/XML parsers
- Dynamic schema detection
- Per-source rate limiting
- Separate threads per source

### Phase 2C: Advanced Features
- Event replay from archive
- Custom webhook filters
- Real-time analytics
- Historical gap detection
- Cross-source correlation

**BUT**: Don't start Phase 2 until Phase 1 is **bulletproof**!

---

## 🆘 Getting Help

### Check Logs First
```bash
# Web server logs
tail -f logs/web_service.log

# Look for errors
tail -f logs/web_service.log | grep ERROR

# Look for rate limit messages
tail -f logs/web_service.log | grep -i rate
```

### Verify Database Connection
```sql
-- Check database is accessible
SELECT version();

-- Check table exists
\d github_events

-- Check for recent inserts
SELECT MAX(processed_at) FROM github_events;
```

### Test API Endpoints
```bash
# Get token first
TOKEN=$(curl -X POST http://localhost:8081/api/auth/login \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"admin\",\"password\":\"${ADMIN_PASSWORD:?set ADMIN_PASSWORD}\"}" \
    | jq -r '.token')

# Test status endpoint
curl -H "Authorization: Bearer $TOKEN" \
     http://localhost:8081/api/realtime/status | jq
```

### Common Error Messages

**"Event monitor not running"**
- Check if monitor was started via UI or API
- Check AppState initialization

**"Database insert failed"**
- Verify database connection
- Check schema is applied
- Look for constraint violations in logs

**"Rate limited by GitHub API"**
- This is expected! Check for auto-pause
- Verify retry after time in logs
- Consider reducing rate limit

**"Failed to parse event"**
- Event structure changed?
- Check GitHub API docs
- Log event JSON for inspection

---

## 📞 Support Resources

### Code References
- **Database**: `src/core/database.rs` (line 325: `insert_events_batch`)
- **Monitor**: `src/realtime/mod.rs` (700 lines)
- **Schema**: `schema.sql` (github_events table)
- **API**: `src/api/handlers.rs` (existing patterns)

### External Documentation
- [GitHub Events API](https://docs.github.com/en/rest/activity/events)
- [GitHub Rate Limiting](https://docs.github.com/en/rest/overview/resources-in-the-rest-api#rate-limiting)
- [PostgreSQL JSONB](https://www.postgresql.org/docs/current/datatype-json.html)

### Useful SQL Queries
```sql
-- Event type distribution
SELECT event_type, COUNT(*) as count 
FROM github_events 
WHERE api_source = 'github_events_api'
GROUP BY event_type 
ORDER BY count DESC;

-- Most active repositories
SELECT repo_name, COUNT(*) as events 
FROM github_events 
WHERE api_source = 'github_events_api'
GROUP BY repo_name 
ORDER BY events DESC 
LIMIT 10;

-- Events per hour
SELECT DATE_TRUNC('hour', event_created_at) as hour, COUNT(*) 
FROM github_events 
WHERE api_source = 'github_events_api'
GROUP BY hour 
ORDER BY hour DESC 
LIMIT 24;
```

---

## 📝 Summary

**Problem**: GitHub Events API functionality exists but doesn't work
**Solution**: Connect existing pieces + add rate limiting + add UI
**Effort**: ~800 lines code, 8-12 hours
**Result**: Fully functional real-time GitHub event scraping

**Start with**: `QUICK_START_FIX.md`
**Reference**: `IMPLEMENTATION_CHECKLIST.md` for tasks
**Understand**: `ARCHITECTURE_DIAGRAM.md` for design
**Deep dive**: `GITHUB_EVENTS_API_ANALYSIS.md` for details

**Good luck! 🚀**
