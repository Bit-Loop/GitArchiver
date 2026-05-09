# 🔒 GitHub Archiver - Secret Detection Monitoring System

> **Production-Ready Monitoring Dashboard for Real-Time Security Analytics**

## 📋 Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [API Documentation](#api-documentation)
- [Frontend Dashboard](#frontend-dashboard)
- [WebSocket Real-Time Updates](#websocket-real-time-updates)
- [Configuration](#configuration)
- [Security](#security)
- [Performance](#performance)
- [Troubleshooting](#troubleshooting)

---

## 🎯 Overview

The GitHub Archiver Monitoring System provides comprehensive real-time analytics and logging for secret detection operations. It includes:

- **Real-time metrics** via WebSocket connections
- **Detection overview** with severity and category breakdowns
- **Trend analysis** with multi-period time-series data
- **System logging** with advanced filtering and export capabilities
- **Production-ready dashboard** with responsive design and live updates

---

## ✨ Features

### Backend Features

- ✅ **Detection Overview API**
  - Total secrets detected with severity breakdown (Critical, High, Medium, Low)
  - Top risky repositories with risk scoring
  - Recent detections with full metadata
  - Category distribution analysis
  - Scan success rate and performance metrics

- ✅ **Trend Analysis API**
  - Multi-period support (24h, 7d, 30d, 90d)
  - Time-series data for total detections
  - Severity-specific trend tracking
  - Growth rate calculation
  - Trend direction indicators

- ✅ **System Logging**
  - Structured logging with levels (ERROR, WARN, INFO, DEBUG)
  - Category-based filtering (system, scanner, api, database)
  - Full-text search capabilities
  - Pagination support
  - CSV export functionality

- ✅ **Real-Time Metrics**
  - System resource monitoring (CPU, Memory, Disk)
  - Active scan tracking
  - WebSocket connection count
  - Queue depth monitoring
  - Error rate tracking

- ✅ **WebSocket Live Updates**
  - 1-second interval metric updates
  - Automatic reconnection
  - Connection lifecycle management
  - Concurrent client support

### Frontend Features

- ✅ **Modern Dashboard UI**
  - Glass-morphism design with Tailwind CSS
  - Responsive layout for all screen sizes
  - Dark theme optimized for monitoring
  - Real-time data refresh

- ✅ **Interactive Charts**
  - Chart.js integration for visualizations
  - Doughnut charts for severity distribution
  - Bar charts for category breakdown
  - Line charts for trend analysis
  - Real-time updating charts

- ✅ **Tabbed Navigation**
  - Overview tab with key metrics
  - Trends tab with historical analysis
  - Logs tab with filtering and search
  - Real-time tab with live system metrics

- ✅ **Advanced Features**
  - WebSocket client with auto-reconnect
  - Loading states and error handling
  - Toast notifications
  - Export logs to CSV
  - Time period selectors

---

## 🏗️ Architecture

### Backend Structure

```
src/api/
├── monitoring_handlers.rs  # Core monitoring logic
│   ├── DetectionOverview      # Overview data structures
│   ├── DetectionTrends        # Trend analysis
│   ├── SystemLogs             # Log management
│   ├── RealTimeMetrics        # Live metrics
│   └── MonitoringState        # In-memory state
├── routes.rs               # API routing
└── mod.rs                  # Module exports
```

### API Endpoints

```
Protected Endpoints (require JWT):
├── GET  /api/monitoring/overview
├── GET  /api/monitoring/trends?period={24h|7d|30d|90d}
├── GET  /api/monitoring/logs?page=1&page_size=50&level=ERROR
└── GET  /api/monitoring/logs/export?level=ERROR

Public Endpoints:
├── GET  /api/monitoring/metrics
└── WS   /api/monitoring/ws
```

### Data Flow

```
┌─────────────┐
│   Scanner   │
└──────┬──────┘
       │ Detections
       ▼
┌─────────────────┐     ┌──────────────┐
│ MonitoringState │────▶│ API Handlers │
│  (In-Memory)    │     └──────┬───────┘
└─────────────────┘            │
       ▲                       │ REST API
       │ Updates               ▼
┌──────────────┐        ┌─────────────┐
│  WebSocket   │◀───────│  Dashboard  │
│   Clients    │        │  (Browser)  │
└──────────────┘        └─────────────┘
```

---

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- PostgreSQL 14+
- Modern web browser (Chrome, Firefox, Safari, Edge)

### Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/yourusername/github-archiver.git
   cd github-archiver/rust_github_archiver
   ```

2. **Set up environment variables**
   ```bash
   export DATABASE_URL="postgresql://postgres:postgres@localhost/github_archiver"
   export RUST_LOG="info,github_archiver=debug"
   export SERVER_PORT="8081"
   ```

3. **Build the project**
   ```bash
   cargo build --release
   ```

4. **Start the monitoring server**
   ```bash
   ./start_monitoring.sh
   ```

5. **Open the dashboard**
   ```
   http://localhost:8081/dashboard
   ```

### Development Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run with auto-reload (requires cargo-watch)
cargo install cargo-watch
cargo watch -x run
```

---

## 📡 API Documentation

### 1. Detection Overview

**Endpoint:** `GET /api/monitoring/overview`

**Authentication:** Required (JWT)

**Response:**
```json
{
  "total_secrets": 1523,
  "critical_secrets": 45,
  "high_secrets": 234,
  "medium_secrets": 789,
  "low_secrets": 455,
  "repositories_scanned": 342,
  "files_scanned": 15678,
  "total_scans": 892,
  "scan_success_rate": 98.5,
  "avg_scan_duration_ms": 1234,
  "category_distribution": {
    "API Keys": 456,
    "Cloud Credentials": 234,
    "Passwords": 123,
    "Tokens": 710
  },
  "top_repositories": [
    {
      "repository": "user/sensitive-repo",
      "total_secrets": 34,
      "critical_count": 5,
      "high_count": 12,
      "risk_score": 8.7
    }
  ],
  "recent_detections": [
    {
      "id": "det_123",
      "repository": "user/repo",
      "secret_type": "github_oauth_token",
      "severity": "Critical",
      "category": "API Key",
      "detected_at": "2024-01-15T10:30:00Z",
      "file_path": "config/secrets.env",
      "verified": true
    }
  ]
}
```

### 2. Detection Trends

**Endpoint:** `GET /api/monitoring/trends?period={24h|7d|30d|90d}`

**Authentication:** Required (JWT)

**Parameters:**
- `period` (optional): Time period for trends. Default: `24h`
  - `24h`: Last 24 hours
  - `7d`: Last 7 days
  - `30d`: Last 30 days
  - `90d`: Last 90 days

**Response:**
```json
{
  "period": "24h",
  "total_detections": 1523,
  "growth_rate": 15.3,
  "trend_direction": "increasing",
  "average_severity_score": 6.5,
  "total_detections_trend": [
    {
      "timestamp": "2024-01-15T00:00:00Z",
      "value": 45
    },
    {
      "timestamp": "2024-01-15T01:00:00Z",
      "value": 52
    }
  ],
  "severity_trends": {
    "Critical": [
      {"timestamp": "2024-01-15T00:00:00Z", "value": 5},
      {"timestamp": "2024-01-15T01:00:00Z", "value": 7}
    ],
    "High": [...],
    "Medium": [...],
    "Low": [...]
  }
}
```

### 3. System Logs

**Endpoint:** `GET /api/monitoring/logs`

**Authentication:** Required (JWT)

**Parameters:**
- `page` (optional): Page number (default: 1)
- `page_size` (optional): Items per page (default: 50, max: 1000)
- `level` (optional): Filter by log level (ERROR, WARN, INFO, DEBUG)
- `category` (optional): Filter by category (system, scanner, api, database)
- `search` (optional): Full-text search in messages
- `start_time` (optional): Filter logs after timestamp (ISO 8601)
- `end_time` (optional): Filter logs before timestamp (ISO 8601)

**Response:**
```json
{
  "logs": [
    {
      "id": "log_123",
      "timestamp": "2024-01-15T10:30:00Z",
      "level": "ERROR",
      "category": "scanner",
      "message": "Failed to scan repository",
      "source": "scanner_service",
      "trace_id": "trace_abc123",
      "user_id": "user_456",
      "metadata": {
        "repository": "user/repo",
        "error_code": "TIMEOUT"
      }
    }
  ],
  "total_count": 10000,
  "filtered_count": 234,
  "page": 1,
  "page_size": 50,
  "levels": {
    "ERROR": 45,
    "WARN": 123,
    "INFO": 8765,
    "DEBUG": 1067
  }
}
```

### 4. Export Logs

**Endpoint:** `GET /api/monitoring/logs/export`

**Authentication:** Required (JWT)

**Parameters:**
- `level` (optional): Filter by log level
- `category` (optional): Filter by category
- `start_time` (optional): Start timestamp
- `end_time` (optional): End timestamp

**Response:** CSV file download
```csv
Timestamp,Level,Category,Message,Source,TraceID
2024-01-15T10:30:00Z,ERROR,scanner,Failed to scan repository,scanner_service,trace_abc123
```

### 5. Real-Time Metrics

**Endpoint:** `GET /api/monitoring/metrics`

**Authentication:** Public

**Response:**
```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "cpu_usage": 45.3,
  "memory_usage_mb": 2048,
  "disk_usage_mb": 102400,
  "network_rx_bytes": 1024000,
  "network_tx_bytes": 512000,
  "active_scans": 5,
  "queued_scans": 12,
  "websocket_connections": 3,
  "total_requests": 15234,
  "error_rate": 0.5
}
```

### 6. WebSocket Live Updates

**Endpoint:** `WS /api/monitoring/ws`

**Authentication:** Public

**Connection:**
```javascript
const ws = new WebSocket('ws://localhost:8081/api/monitoring/ws');

ws.onmessage = (event) => {
  const metrics = JSON.parse(event.data);
  console.log('Live metrics:', metrics);
};
```

**Message Format:**
```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "cpu_usage": 45.3,
  "memory_usage_mb": 2048,
  "active_scans": 5,
  "websocket_connections": 3
}
```

**Update Frequency:** 1 second

---

## 🎨 Frontend Dashboard

### Features

#### 1. Overview Tab
- **Key Metrics Cards**
  - Total Secrets (with critical count)
  - Repositories Scanned (with file count)
  - Success Rate (with total scans)
  - Average Scan Time

- **Severity Distribution Chart**
  - Doughnut chart with color-coded severities
  - Critical (Red), High (Orange), Medium (Yellow), Low (Green)

- **Category Breakdown Chart**
  - Bar chart showing detection counts by category
  - API Keys, Cloud Credentials, Passwords, Tokens, etc.

- **Top Risk Repositories**
  - List of repositories with highest risk scores
  - Shows total secrets, critical/high counts
  - Risk score out of 10

- **Recent Detections**
  - Live feed of latest secret detections
  - Color-coded by severity
  - Shows repository, file path, timestamp

#### 2. Trends Tab
- **Time Period Selector**
  - 24 Hours, 7 Days, 30 Days, 90 Days
  - Dynamic chart updates

- **Trend Indicators**
  - Growth rate percentage
  - Trend direction (Increasing/Decreasing/Stable)
  - Average severity score
  - Detection rate per scan

- **Detection Trends Chart**
  - Line chart showing total detections over time
  - Smooth curves with gradient fill
  - Responsive time axis

- **Severity Trends Chart**
  - Multi-line chart for each severity level
  - Stacked view option
  - Legend with color coding

#### 3. Logs Tab
- **Advanced Filtering**
  - Level filter (ERROR, WARN, INFO, DEBUG)
  - Category filter (system, scanner, api, database)
  - Full-text search
  - Real-time filtering

- **Log Statistics**
  - Total logs count
  - Error/Warning/Info breakdown
  - Color-coded metrics

- **Logs List**
  - Paginated view (50 logs per page)
  - Color-coded by level
  - Shows timestamp, category, message
  - Expandable metadata

- **Export Functionality**
  - Download logs as CSV
  - Respects current filters
  - Timestamped filename

#### 4. Real-Time Tab
- **System Metrics**
  - CPU Usage with progress bar
  - Memory Usage with progress bar
  - Active Scans counter
  - WebSocket Connections counter

- **Live Metrics Chart**
  - Dual-axis line chart
  - CPU percentage (left axis)
  - Memory MB (right axis)
  - Updates every second

- **Activity Feed**
  - Live log stream
  - Latest system events
  - Auto-scrolling
  - Limited to 50 recent items

### UI/UX Features

- **Glass Morphism Design**
  - Translucent cards with backdrop blur
  - Smooth shadows and borders
  - Modern gradient accents

- **Responsive Layout**
  - Mobile-friendly design
  - Flexbox grid system
  - Breakpoints for all screen sizes

- **Interactive Elements**
  - Hover effects on cards
  - Smooth transitions
  - Loading states
  - Toast notifications

- **Connection Status**
  - Live WebSocket connection indicator
  - Pulse animation when connected
  - Auto-reconnect on disconnect

---

## 🔌 WebSocket Real-Time Updates

### Connection Management

```javascript
let ws = null;

function connectWebSocket() {
    ws = new WebSocket('ws://localhost:8081/api/monitoring/ws');
    
    ws.onopen = () => {
        console.log('Connected to monitoring server');
        updateConnectionStatus(true);
    };
    
    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        handleRealtimeUpdate(data);
    };
    
    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        updateConnectionStatus(false);
    };
    
    ws.onclose = () => {
        console.log('Disconnected from server');
        updateConnectionStatus(false);
        // Auto-reconnect after 5 seconds
        setTimeout(connectWebSocket, 5000);
    };
}
```

### Data Handling

```javascript
function handleRealtimeUpdate(data) {
    // Update CPU usage
    document.getElementById('rt-cpu').textContent = 
        data.cpu_usage.toFixed(1) + '%';
    
    // Update memory
    document.getElementById('rt-memory').textContent = 
        data.memory_usage_mb + ' MB';
    
    // Update active scans
    document.getElementById('rt-active-scans').textContent = 
        data.active_scans;
    
    // Update charts
    updateRealtimeChart(data);
}
```

---

## ⚙️ Configuration

### Environment Variables

```bash
# Database
DATABASE_URL="postgresql://user:pass@localhost/dbname"

# Server
SERVER_PORT="8081"
ENABLE_CORS="true"

# Logging
RUST_LOG="info,github_archiver=debug"

# Monitoring
MONITORING_UPDATE_INTERVAL="1"  # seconds
MAX_LOGS_STORED="10000"
MAX_METRICS_POINTS="1440"  # 24 hours at 1-minute intervals
```

### In-Memory State Configuration

Located in `monitoring_handlers.rs`:

```rust
// Maximum logs to store (keeps last 10,000)
const MAX_LOGS: usize = 10000;

// Maximum metrics points (1440 = 24h at 1/min)
const MAX_METRICS: usize = 1440;

// WebSocket update interval
const UPDATE_INTERVAL: Duration = Duration::from_secs(1);
```

---

## 🔒 Security

### Authentication

Protected endpoints require JWT authentication:

```bash
curl -H "Authorization: Bearer YOUR_JWT_TOKEN" \
     http://localhost:8081/api/monitoring/overview
```

### CORS Configuration

CORS is enabled for development:

```rust
// In routes.rs
.layer(
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any)
)
```

**Production:** Restrict to specific origins.

### Rate Limiting

Consider implementing rate limiting for production:

```rust
// Example: Tower rate limit
use tower::limit::RateLimitLayer;

.layer(RateLimitLayer::new(100, Duration::from_secs(60)))
```

---

## ⚡ Performance

### Optimization Features

1. **In-Memory Storage**
   - Fast read/write with `RwLock`
   - Automatic data trimming (10K logs, 1440 metrics)
   - O(1) append operations

2. **WebSocket Efficiency**
   - Binary protocol option
   - Compression support (gzip)
   - Connection pooling

3. **Chart Optimization**
   - Canvas rendering (Chart.js)
   - Update throttling
   - Data point limiting

4. **Database Queries**
   - Indexed timestamps
   - Pagination support
   - Prepared statements

### Performance Benchmarks

Expected performance on moderate hardware:

- **API Response Time:** < 50ms (p95)
- **WebSocket Latency:** < 10ms
- **Memory Usage:** ~200MB (with 10K logs)
- **CPU Usage:** < 5% (idle), < 20% (active scanning)
- **Concurrent WebSocket Clients:** 1000+

---

## 🐛 Troubleshooting

### Common Issues

#### 1. WebSocket Connection Failed

**Symptoms:** Dashboard shows "Disconnected"

**Solutions:**
```bash
# Check if server is running
curl http://localhost:8081/api/monitoring/metrics

# Check firewall
sudo ufw allow 8081

# Check browser console for CORS errors
# Verify WS URL matches server address
```

#### 2. No Data in Charts

**Symptoms:** Charts show empty or zero values

**Solutions:**
```bash
# Verify database connection
psql -d github_archiver -c "SELECT COUNT(*) FROM secrets;"

# Check API responses
curl http://localhost:8081/api/monitoring/overview

# Initialize sample data (development)
# Call add_test_log() and add_test_detection()
```

#### 3. High Memory Usage

**Symptoms:** Server using excessive RAM

**Solutions:**
```rust
// Reduce MAX_LOGS in monitoring_handlers.rs
const MAX_LOGS: usize = 5000;  // Default: 10000

// Reduce MAX_METRICS
const MAX_METRICS: usize = 720;  // 12h instead of 24h
```

#### 4. Slow Chart Rendering

**Symptoms:** Dashboard UI lag, slow updates

**Solutions:**
```javascript
// Reduce real-time data points
if (realtimeData.labels.length > 10) {  // Default: 20
    realtimeData.labels.shift();
    // ...
}

// Disable animations
charts.realtime.options.animation = false;
```

### Debug Mode

Enable detailed logging:

```bash
RUST_LOG=trace cargo run 2>&1 | tee monitoring.log
```

### Health Checks

```bash
# Check server health
curl http://localhost:8081/api/monitoring/metrics

# Check WebSocket
websocat ws://localhost:8081/api/monitoring/ws

# Check database
psql -d github_archiver -c "\dt"
```

---

## 📊 Metrics Reference

### Detection Severity Levels

| Severity | Score | Description | Examples |
|----------|-------|-------------|----------|
| **Critical** | 9-10 | Verified production secrets | AWS keys, GitHub tokens, API keys |
| **High** | 7-8 | Likely valid secrets | Database passwords, Slack webhooks |
| **Medium** | 4-6 | Potential secrets | Generic API tokens, encoded strings |
| **Low** | 1-3 | Low-confidence matches | Test keys, placeholders |

### Risk Score Calculation

```
Risk Score = (
    (Critical × 10) + 
    (High × 7) + 
    (Medium × 4) + 
    (Low × 1)
) / Total Secrets
```

### Category Types

- **API Keys:** GitHub, GitLab, third-party services
- **Cloud Credentials:** AWS, Azure, GCP
- **Passwords:** Database, service accounts
- **Tokens:** JWT, OAuth, session tokens
- **Certificates:** SSL, SSH keys, PGP keys
- **Other:** Misc sensitive data

---

## 🎯 Best Practices

### Production Deployment

1. **Enable HTTPS**
   ```rust
   // Use TLS for WebSocket (wss://)
   .layer(TlsLayer::new(...))
   ```

2. **Set up Reverse Proxy**
   ```nginx
   location /api/monitoring/ws {
       proxy_pass http://localhost:8081;
       proxy_http_version 1.1;
       proxy_set_header Upgrade $http_upgrade;
       proxy_set_header Connection "upgrade";
   }
   ```

3. **Configure Log Rotation**
   ```bash
   # /etc/logrotate.d/github-archiver
   /var/log/github-archiver/*.log {
       daily
       rotate 30
       compress
       delaycompress
       notifempty
   }
   ```

4. **Monitor System Resources**
   ```bash
   # Set up Prometheus metrics
   # Configure Grafana dashboards
   # Set up alerting rules
   ```

### Development Tips

1. **Use Mock Data**
   ```rust
   // Generate test data for development
   initialize_sample_data().await;
   ```

2. **Hot Reload Frontend**
   ```bash
   # Use live-server or similar
   npm install -g live-server
   live-server --port=8082 --proxy=/api:http://localhost:8081
   ```

3. **Debug WebSocket**
   ```javascript
   // Add verbose logging
   ws.onmessage = (event) => {
       console.log('WS Data:', event.data);
       const data = JSON.parse(event.data);
       console.table(data);
   };
   ```

---

## 📝 License

This monitoring system is part of the GitHub Archiver project.

---

## 🤝 Contributing

Contributions welcome! Please ensure:

1. Code follows Rust best practices
2. Frontend is responsive and accessible
3. API changes are backward compatible
4. Documentation is updated
5. Tests are included

---

## 📧 Support

For issues and questions:

- GitHub Issues: [github.com/yourrepo/issues](https://github.com)
- Email: support@example.com
- Documentation: This file

---

**Built with ❤️ using Rust, Axum, Chart.js, and Tailwind CSS**
