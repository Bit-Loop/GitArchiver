# GitHub Events API - Current State & Implementation Plan

## 📊 Current State Analysis

### ✅ What Already Exists

#### 1. **Realtime Module** (`src/realtime/mod.rs`)
- **Location**: Fully implemented in `src/realtime/mod.rs` (700 lines)
- **Purpose**: Monitors GitHub Events API (https://api.github.com/events)
- **Functionality**:
  - Polls events every 10 seconds
  - Processes PushEvent, PullRequestEvent, IssueCommentEvent, ReleaseEvent
  - Scans for secrets using `SecretScanner`
  - Detects dangling commits
  - Webhook notifications for alerts
  - Real-time secret detection

#### 2. **Database Schema** (`schema.sql`)
- **Table**: `github_events` - Comprehensive schema for storing GitHub Events
- **Columns**: 90+ columns including:
  - Event metadata (id, type, created_at, public)
  - Actor details (id, login, avatar, urls, etc.)
  - Repository details (id, name, owner, language, stars, forks, etc.)
  - Organization details (optional)
  - **`payload JSONB`** - Full event payload
  - **`raw_event JSONB`** - Raw event data
  - Source tracking (`file_source`, `api_source`)
  
- **Supporting Tables**:
  - `processed_files` - Tracks processed archive files
  - `repositories` - Repository metadata

- **Indexes**: Optimized for queries on:
  - `event_created_at`, `event_type`, `actor_id`, `repo_id`
  - `actor_login`, `repo_name`
  - GIN index on `payload` JSONB column

#### 3. **Integration Points**
- ✅ Module exported in `src/lib.rs`: `pub mod realtime;`
- ✅ `GitHubEventMonitor` is public and usable
- ⚠️ **NOT** integrated into `AppState` 
- ⚠️ **NOT** connected to API routes
- ⚠️ **NOT** saving to database

### ❌ Critical Gaps

#### 1. **Database Persistence Exists But Not Connected!**
**GOOD NEWS**: Database insertion code already exists!
- ✅ `src/core/database.rs` has `insert_events_batch()` function
- ✅ Comprehensive 67-parameter INSERT statement with all fields
- ✅ Transaction-based batch processing
- ✅ Event validation and conversion

**PROBLEM**: `GitHubEventMonitor` doesn't use it!
```rust
// Current realtime/mod.rs: Only scans for secrets
async fn process_single_event(&self, event: GitHubEvent) -> Result<()> {
    match event.event_type.as_str() {
        "PushEvent" => self.process_push_event(event).await,
        // ❌ NO DATABASE INSERTION!
    }
}
```

**Fix Required**: Connect `GitHubEventMonitor` to `Database::insert_events_batch()`

#### 2. **No Rate Limiting**
```rust
// Current: Fixed 10-second polling
let mut poll_interval = interval(Duration::from_secs(10)); // HARDCODED!
```

**Problems**:
- No rate limit detection (429 responses)
- No adaptive rate limiting
- No backoff strategy
- No pause/resume on rate limiting
- No GUI controls

#### 3. **No API Integration**
**Missing API Endpoints**:
- `POST /api/realtime/start` - Start monitoring
- `POST /api/realtime/stop` - Stop monitoring
- `POST /api/realtime/pause` - Pause monitoring
- `POST /api/realtime/resume` - Resume monitoring
- `GET /api/realtime/status` - Get status and stats
- `PUT /api/realtime/config` - Update rate limit config
- `GET /api/realtime/events` - Retrieve stored events

#### 4. **No Dashboard UI**
**Missing GUI Components**:
- Rate limit slider (1-60 requests/minute)
- Auto-adjust rate limit checkbox
- Start/Stop/Pause/Resume buttons
- Real-time metrics display
- Events processed counter
- Rate limit status indicator
- Last event timestamp

#### 5. **No State Management**
```rust
pub struct AppState {
    pub config: Config,
    pub scraper_manager: Arc<ScraperManager>,
    pub main_scraper: Arc<Mutex<Option<MainScraper>>>,
    // ... NO GitHubEventMonitor here!
}
```

**Missing**: `event_monitor: Arc<Mutex<Option<GitHubEventMonitor>>>`

---

## 🎯 Implementation Plan

### Phase 1: Core Functionality (Make it Work!)

#### Step 1.1: Connect to Existing Database Module
**File**: `src/realtime/mod.rs`

**USE THE EXISTING DATABASE CODE!** Don't reinvent the wheel.

Add Database reference to GitHubEventMonitor:
```rust
use crate::core::Database;

pub struct GitHubEventMonitor {
    client: Client,
    database: Option<Arc<Database>>,  // ADD THIS - use existing Database!
    rate_limiter: AdaptiveRateLimiter,
    // ... rest
}

impl GitHubEventMonitor {
    pub async fn new(github_token: &str) -> Result<Self> {
        Ok(Self {
            database: None,  // Will be set with with_database()
            // ...
        })
    }

    pub fn with_database(mut self, database: Arc<Database>) -> Self {
        self.database = Some(database);
        self
    }

    async fn save_events_to_db(&self, events: &[GitHubEvent]) -> Result<()> {
        if let Some(db) = &self.database {
            // Convert GitHubEvent to serde_json::Value for batch insert
            let event_values: Vec<serde_json::Value> = events.iter()
                .map(|e| serde_json::to_value(e))
                .collect::<Result<Vec<_>, _>>()?;
            
            // Use the EXISTING insert_events_batch function!
            db.insert_events_batch(event_values, "github_events_api").await?;
        }
        Ok(())
    }
}
```

Update `process_events` to batch save:
```rust
async fn process_events(&self, events: Vec<GitHubEvent>) -> Result<()> {
    // SAVE ALL EVENTS TO DATABASE FIRST (batch insert is efficient!)
    self.save_events_to_db(&events).await?;
    
    // Then add to processing queue for secret scanning
    {
        let mut queue = self.processing_queue.write().await;
        queue.extend(events);
    }
    
    self.process_queue().await?;
    Ok(())
}
```

#### Step 1.2: Add Adaptive Rate Limiter
**File**: `src/realtime/rate_limiter.rs` (NEW)

```rust
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct AdaptiveRateLimiter {
    requests_per_minute: Arc<RwLock<u32>>,
    last_request_times: Arc<RwLock<Vec<Instant>>>,
    auto_adjust: Arc<RwLock<bool>>,
    paused_until: Arc<RwLock<Option<Instant>>>,
    retry_after: Arc<RwLock<Option<Duration>>>,
}

impl AdaptiveRateLimiter {
    pub fn new(requests_per_minute: u32, auto_adjust: bool) -> Self {
        Self {
            requests_per_minute: Arc::new(RwLock::new(requests_per_minute)),
            last_request_times: Arc::new(RwLock::new(Vec::new())),
            auto_adjust: Arc::new(RwLock::new(auto_adjust)),
            paused_until: Arc::new(RwLock::new(None)),
            retry_after: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn wait_if_needed(&self) -> Result<()> {
        // Check if paused due to rate limiting
        let paused_until = self.paused_until.read().await;
        if let Some(until) = *paused_until {
            if Instant::now() < until {
                let wait_time = until.duration_since(Instant::now());
                info!("Rate limited - waiting {:?}", wait_time);
                drop(paused_until); // Release lock before sleeping
                tokio::time::sleep(wait_time).await;
                return Ok(());
            } else {
                // Pause expired, clear it
                drop(paused_until);
                *self.paused_until.write().await = None;
            }
        }

        // Clean old request times (older than 1 minute)
        let mut times = self.last_request_times.write().await;
        let now = Instant::now();
        times.retain(|&t| now.duration_since(t) < Duration::from_secs(60));

        // Check if we've hit the limit
        let limit = *self.requests_per_minute.read().await;
        if times.len() >= limit as usize {
            // Calculate wait time to next available slot
            if let Some(&oldest) = times.first() {
                let wait_time = Duration::from_secs(60)
                    .saturating_sub(now.duration_since(oldest));
                
                if !wait_time.is_zero() {
                    info!("Rate limit reached ({}/min) - waiting {:?}", limit, wait_time);
                    drop(times); // Release lock
                    tokio::time::sleep(wait_time).await;
                }
            }
        }

        // Record this request
        let mut times = self.last_request_times.write().await;
        times.push(now);
        
        Ok(())
    }

    pub async fn handle_rate_limit_response(&self, retry_after_seconds: Option<u64>) {
        let retry_duration = retry_after_seconds
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(60)); // Default 1 minute

        *self.paused_until.write().await = Some(Instant::now() + retry_duration);
        *self.retry_after.write().await = Some(retry_duration);

        // Auto-adjust rate if enabled
        if *self.auto_adjust.read().await {
            let mut current_rate = self.requests_per_minute.write().await;
            let new_rate = (*current_rate as f64 * 0.8).max(1.0) as u32; // Reduce by 20%
            warn!("Auto-adjusting rate limit: {} -> {} req/min", *current_rate, new_rate);
            *current_rate = new_rate;
        }
    }

    pub async fn set_rate(&self, requests_per_minute: u32) {
        *self.requests_per_minute.write().await = requests_per_minute;
    }

    pub async fn set_auto_adjust(&self, enabled: bool) {
        *self.auto_adjust.write().await = enabled;
    }

    pub async fn get_status(&self) -> RateLimitStatus {
        let times = self.last_request_times.read().await;
        let now = Instant::now();
        let recent_requests = times.iter()
            .filter(|&&t| now.duration_since(t) < Duration::from_secs(60))
            .count() as u32;

        RateLimitStatus {
            requests_per_minute: *self.requests_per_minute.read().await,
            requests_last_minute: recent_requests,
            auto_adjust_enabled: *self.auto_adjust.read().await,
            is_paused: self.paused_until.read().await.is_some(),
            retry_after: *self.retry_after.read().await,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RateLimitStatus {
    pub requests_per_minute: u32,
    pub requests_last_minute: u32,
    pub auto_adjust_enabled: bool,
    pub is_paused: bool,
    pub retry_after: Option<Duration>,
}
```

#### Step 1.3: Update GitHubEventMonitor with Rate Limiter
**File**: `src/realtime/mod.rs`

```rust
pub struct GitHubEventMonitor {
    client: Client,
    db_pool: Option<PgPool>,  // ADD THIS
    rate_limiter: AdaptiveRateLimiter,  // ADD THIS
    secret_scanner: SecretScanner,
    // ... rest of fields
}

impl GitHubEventMonitor {
    pub async fn new(github_token: &str) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            db_pool: None,
            rate_limiter: AdaptiveRateLimiter::new(5, true), // 5 req/min, auto-adjust ON
            // ... rest
        })
    }

    async fn poll_events(&self) -> Result<Vec<GitHubEvent>> {
        // WAIT FOR RATE LIMITER
        self.rate_limiter.wait_if_needed().await?;
        
        let url = "https://api.github.com/events";
        let response = self.client.get(url)
            .header("User-Agent", "GitHubArchiver/2.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        // CHECK FOR RATE LIMITING
        if response.status() == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            
            error!("Rate limited by GitHub API! Retry-After: {:?} seconds", retry_after);
            self.rate_limiter.handle_rate_limit_response(retry_after).await;
            
            return Ok(vec![]); // Return empty, will retry after pause
        }

        if !response.status().is_success() {
            return Err(anyhow!("GitHub API returned status: {}", response.status()));
        }

        let events: Vec<GitHubEvent> = response.json().await?;
        // ... rest of logic
    }
}
```

#### Step 1.4: Integrate into AppState
**File**: `src/api/state.rs`

```rust
use crate::realtime::GitHubEventMonitor;

pub struct AppState {
    pub config: Config,
    pub scraper_manager: Arc<ScraperManager>,
    pub main_scraper: Arc<Mutex<Option<MainScraper>>>,
    pub event_monitor: Arc<AsyncMutex<Option<GitHubEventMonitor>>>,  // ADD THIS
    pub user_manager: Arc<UserManager>,
    pub resource_monitor: Arc<AsyncMutex<ResourceMonitor>>,
    pub scanning_service: Arc<ScanningService>,
    pub database: Arc<Database>,
}

impl AppState {
    pub fn new(config: Config, database: Arc<Database>) -> Self {
        Self {
            // ... existing fields
            event_monitor: Arc::new(AsyncMutex::new(None)),  // ADD THIS
            // ... rest
        }
    }

    pub async fn initialize_event_monitor(&self, github_token: &str) -> Result<()> {
        let monitor = GitHubEventMonitor::new(github_token)
            .await?
            .with_database(self.database.get_pool());
        
        *self.event_monitor.lock().await = Some(monitor);
        Ok(())
    }
}
```

#### Step 1.5: Add API Endpoints
**File**: `src/api/realtime_handlers.rs` (NEW)

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub auto_adjust: bool,
}

pub async fn start_event_monitor(State(app_state): State<AppState>) -> Json<Value> {
    // Start monitoring in background
    let monitor = app_state.event_monitor.clone();
    
    tokio::spawn(async move {
        if let Some(monitor) = monitor.lock().await.as_ref() {
            if let Err(e) = monitor.start_monitoring().await {
                error!("Event monitor failed: {}", e);
            }
        }
    });

    Json(json!({
        "status": "success",
        "message": "Event monitoring started"
    }))
}

pub async fn stop_event_monitor(State(app_state): State<AppState>) -> Json<Value> {
    *app_state.event_monitor.lock().await = None;
    
    Json(json!({
        "status": "success",
        "message": "Event monitoring stopped"
    }))
}

pub async fn get_event_monitor_status(State(app_state): State<AppState>) -> Json<Value> {
    if let Some(monitor) = app_state.event_monitor.lock().await.as_ref() {
        let rate_status = monitor.rate_limiter.get_status().await;
        
        Json(json!({
            "running": true,
            "rate_limit": rate_status,
        }))
    } else {
        Json(json!({
            "running": false,
        }))
    }
}

pub async fn update_rate_limit(
    State(app_state): State<AppState>,
    Json(config): Json<RateLimitConfig>,
) -> Json<Value> {
    if let Some(monitor) = app_state.event_monitor.lock().await.as_ref() {
        monitor.rate_limiter.set_rate(config.requests_per_minute).await;
        monitor.rate_limiter.set_auto_adjust(config.auto_adjust).await;
        
        Json(json!({
            "status": "success",
            "message": "Rate limit updated",
            "config": config
        }))
    } else {
        Json(json!({
            "status": "error",
            "message": "Event monitor not running"
        }))
    }
}
```

**File**: `src/api/routes.rs`

Add to routes:
```rust
.route("/api/realtime/start", post(start_event_monitor))
.route("/api/realtime/stop", post(stop_event_monitor))
.route("/api/realtime/status", get(get_event_monitor_status))
.route("/api/realtime/config", put(update_rate_limit))
```

#### Step 1.6: Add Dashboard UI
**File**: `dashboard.html`

Add new tab in navigation:
```html
<li class="nav-item">
    <a class="nav-link" id="events-tab" data-bs-toggle="tab" href="#events" role="tab">
        <i class="bi bi-broadcast"></i> GitHub Events
    </a>
</li>
```

Add tab content:
```html
<div class="tab-pane fade" id="events" role="tabpanel">
    <div class="row g-4">
        <!-- Control Panel -->
        <div class="col-md-6">
            <div class="card">
                <div class="card-header">
                    <h5>Event Monitor Control</h5>
                </div>
                <div class="card-body">
                    <!-- Start/Stop Buttons -->
                    <div class="btn-group mb-3">
                        <button id="start-events-btn" class="btn btn-success">
                            <i class="bi bi-play-circle"></i> Start
                        </button>
                        <button id="stop-events-btn" class="btn btn-danger">
                            <i class="bi bi-stop-circle"></i> Stop
                        </button>
                    </div>

                    <!-- Rate Limit Slider -->
                    <div class="mb-3">
                        <label for="rate-limit-slider" class="form-label">
                            Requests per Minute: <span id="rate-limit-value">5</span>
                        </label>
                        <input type="range" class="form-range" id="rate-limit-slider"
                               min="1" max="60" value="5">
                    </div>

                    <!-- Auto-Adjust Checkbox -->
                    <div class="form-check mb-3">
                        <input class="form-check-input" type="checkbox" id="auto-adjust-checkbox" checked>
                        <label class="form-check-label" for="auto-adjust-checkbox">
                            Auto-adjust rate on 429 responses
                        </label>
                    </div>
                </div>
            </div>
        </div>

        <!-- Status Display -->
        <div class="col-md-6">
            <div class="card">
                <div class="card-header">
                    <h5>Monitor Status</h5>
                </div>
                <div class="card-body">
                    <div class="status-item">
                        <span class="label">Status:</span>
                        <span id="monitor-status" class="badge bg-secondary">Stopped</span>
                    </div>
                    <div class="status-item">
                        <span class="label">Requests (last min):</span>
                        <span id="requests-count">0</span>
                    </div>
                    <div class="status-item">
                        <span class="label">Rate Limited:</span>
                        <span id="rate-limited-status">No</span>
                    </div>
                    <div class="status-item">
                        <span class="label">Events Processed:</span>
                        <span id="events-processed">0</span>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>
```

Add JavaScript:
```javascript
// GitHub Events Monitor Control
document.getElementById('start-events-btn').addEventListener('click', async () => {
    try {
        const response = await fetch('/api/realtime/start', {
            method: 'POST',
            headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
        });
        const data = await response.json();
        showNotification('Event monitor started', 'success');
        updateEventMonitorStatus();
    } catch (error) {
        showNotification('Failed to start event monitor', 'error');
    }
});

document.getElementById('stop-events-btn').addEventListener('click', async () => {
    try {
        const response = await fetch('/api/realtime/stop', {
            method: 'POST',
            headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
        });
        const data = await response.json();
        showNotification('Event monitor stopped', 'success');
        updateEventMonitorStatus();
    } catch (error) {
        showNotification('Failed to stop event monitor', 'error');
    }
});

// Rate limit slider
document.getElementById('rate-limit-slider').addEventListener('change', async (e) => {
    const value = parseInt(e.target.value);
    document.getElementById('rate-limit-value').textContent = value;
    
    const autoAdjust = document.getElementById('auto-adjust-checkbox').checked;
    
    try {
        await fetch('/api/realtime/config', {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({
                requests_per_minute: value,
                auto_adjust: autoAdjust
            })
        });
        showNotification('Rate limit updated', 'success');
    } catch (error) {
        showNotification('Failed to update rate limit', 'error');
    }
});

// Auto-adjust checkbox
document.getElementById('auto-adjust-checkbox').addEventListener('change', async (e) => {
    const slider = document.getElementById('rate-limit-slider');
    const value = parseInt(slider.value);
    
    try {
        await fetch('/api/realtime/config', {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${localStorage.getItem('token')}`
            },
            body: JSON.stringify({
                requests_per_minute: value,
                auto_adjust: e.target.checked
            })
        });
        showNotification('Auto-adjust setting updated', 'success');
    } catch (error) {
        showNotification('Failed to update setting', 'error');
    }
});

// Update status periodically
async function updateEventMonitorStatus() {
    try {
        const response = await fetch('/api/realtime/status', {
            headers: { 'Authorization': `Bearer ${localStorage.getItem('token')}` }
        });
        const data = await response.json();
        
        const statusBadge = document.getElementById('monitor-status');
        if (data.running) {
            statusBadge.textContent = 'Running';
            statusBadge.className = 'badge bg-success';
            
            if (data.rate_limit) {
                document.getElementById('requests-count').textContent = 
                    data.rate_limit.requests_last_minute;
                document.getElementById('rate-limited-status').textContent = 
                    data.rate_limit.is_paused ? 'Yes' : 'No';
            }
        } else {
            statusBadge.textContent = 'Stopped';
            statusBadge.className = 'badge bg-secondary';
        }
    } catch (error) {
        console.error('Failed to update event monitor status:', error);
    }
}

// Update every 5 seconds
setInterval(updateEventMonitorStatus, 5000);
```

---

## Phase 2: Future Extensibility (After Core Works)

### Multi-Source Architecture

Once GitHub Events API is working perfectly, add:

1. **Generic API Source Manager**
   - Source registry (CSV, JSON, XML parsers)
   - Dynamic schema detection
   - Per-source rate limiting
   - Separate threads per source
   
2. **Source Configuration UI**
   - Add new API endpoints dynamically
   - Configure formats (CSV/JSON/XML)
   - Set rate limits per source
   - Enable/disable sources

3. **Data Normalization Layer**
   - Auto-detect JSON structure
   - Map to generic schema
   - Store in source-specific tables
   - Cross-source querying

---

## 📝 Summary

### Current Problems:
1. ❌ Events not saved to database
2. ❌ No rate limiting (hardcoded 10s polling)
3. ❌ No 429 detection/handling
4. ❌ No GUI controls
5. ❌ Not integrated into AppState/API

### Solution:
1. ✅ Add database persistence to `GitHubEventMonitor`
2. ✅ Implement `AdaptiveRateLimiter` with 429 handling
3. ✅ Add to `AppState` and create API endpoints
4. ✅ Build dashboard UI with controls
5. ✅ Test with real GitHub Events API

### Priority:
**FIRST**: Make GitHub Events API work perfectly
**SECOND**: Add multi-source extensibility

This ensures the core value proposition works before feature creep!
