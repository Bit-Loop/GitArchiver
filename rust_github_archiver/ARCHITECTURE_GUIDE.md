# 🏗️ GitArchiver System Architecture Guide

> **Comprehensive breakdown of the core components, their interactions, and practical usage**

---

## 📑 Table of Contents
1. [System Overview](#system-overview)
2. [Core Architecture Layers](#core-architecture-layers)
3. [Component Deep Dive](#component-deep-dive)
4. [Data Flow & Interactions](#data-flow--interactions)
5. [Practical Usage Examples](#practical-usage-examples)
6. [Extension Points](#extension-points)

---

## System Overview

### What This System Does
**GitArchiver** is a **multi-purpose data intelligence platform** that:
1. **Archives GitHub activity** from GHArchive.org (historical + real-time)
2. **Discovers secrets** across repositories using advanced pattern matching
3. **Manages dynamic schemas** for multi-source data integration
4. **Visualizes data relationships** in interactive tree structures
5. **Provides REST APIs** for programmatic access

### Why It Exists
- **Research & Analysis**: GitHub ecosystem behavioral patterns
- **Security**: Secret detection and vulnerability scanning
- **Data Integration**: Unified multi-source data management
- **Intelligence**: AI-powered triage and anomaly detection

### How It's Structured
```
┌─────────────────────────────────────────────────────────────┐
│                    Web API Layer (Axum)                      │
│  /health /api/status /api/database/* /api/scraper/*         │
└────────────┬────────────────────────────────────────────────┘
             │
    ┌────────┴─────────┐
    │   Application    │
    │      State       │  ← Single source of truth
    └────────┬─────────┘
             │
    ┌────────┴──────────────────────────────────────┐
    │                                                │
┌───▼────┐  ┌──────────┐  ┌──────────┐  ┌─────────▼──┐
│ Core   │  │ Scraper  │  │ Schema   │  │ Multi-     │
│ System │  │ Pipeline │  │ Manager  │  │ Source     │
└───┬────┘  └──────────┘  └──────────┘  └─────────┬──┘
    │                                              │
┌───▼─────────────────────────────────────────────▼───┐
│            PostgreSQL Database                      │
│  (Connection Pool + Advanced Schema Management)     │
└─────────────────────────────────────────────────────┘
```

---

## Core Architecture Layers

### Layer 1: Foundation (Core)
**Location**: `src/core/`

#### 1.1 Configuration System (`config.rs`)
**Purpose**: Centralized configuration management with env variable support

```rust
use github_archiver::core::Config;

// Load configuration
let config = Config::default(); // Uses environment variables
// Or from file
let config = Config::load_from_file("config.json")?;

// Access database settings
println!("DB: {}@{}:{}", 
    config.database.user,
    config.database.host,
    config.database.port
);

// Configure resources
config.resources.memory_limit_gb = 16.0;
config.resources.cpu_limit_percent = 80.0;

// Validate before use
config.validate()?;
```

**Key Components**:
- `DatabaseConfig`: PostgreSQL connection params
- `GitHubConfig`: GitHub API authentication
- `DownloadConfig`: Scraper download settings
- `ResourceConfig`: Memory/CPU/Disk limits
- `WebConfig`: API server host/port

#### 1.2 Database Layer (`database.rs`)
**Purpose**: PostgreSQL connection pooling + query execution

```rust
use github_archiver::core::{Config, Database};

// Initialize database
let config = Config::default();
let database = Database::new(config).await?;

// Health check
let health = database.health_check().await;
println!("Connected: {}, Connections: {}", 
    health.is_connected, 
    health.connection_count
);

// Get statistics
let stats = database.get_database_statistics().await?;
println!("Total events: {}", stats.total_events);
println!("Database size: {}", stats.database_size);

// Query example
let events = database.get_data_quality_metrics().await?;
println!("Unique actors: {}", events.unique_actors);
```

**Features**:
- Automatic reconnection with retry logic
- Connection pooling (configurable min/max)
- Schema initialization on startup
- Health monitoring with cache hit ratios
- Transaction support for batch operations

#### 1.3 Resource Monitor (`resource_monitor.rs`)
**Purpose**: System resource tracking + emergency cleanup

```rust
use github_archiver::core::{ResourceMonitor, ResourceLimits};

let limits = ResourceLimits {
    memory_limit_gb: 18.0,
    disk_limit_gb: 40.0,
    cpu_limit_percent: 80.0,
    memory_warning_threshold: 0.8,
    emergency_cleanup_threshold: 0.9,
    monitoring_interval_seconds: 30,
    ..Default::default()
};

let mut monitor = ResourceMonitor::new(limits);

// Check resources
let status = monitor.get_resource_status().await?;
if status.emergency_mode {
    println!("⚠️ EMERGENCY: Memory at {}%", status.memory.percent);
    monitor.emergency_cleanup().await?;
}

// Monitor specific resource
if status.memory.percent > 85.0 {
    println!("Memory usage high: {:.1} GB", status.memory.used_gb);
}
```

---

### Layer 2: Data Acquisition (Scraper)
**Location**: `src/scraper/`

#### 2.1 Main Scraper (`main_scraper.rs`)
**Purpose**: Orchestrates GitHub Archive downloading + processing

```rust
use github_archiver::scraper::MainScraper;
use github_archiver::core::Config;

let config = Config::default();
let mut scraper = MainScraper::new(config)?;

// Initialize (connects to DB, sets up downloaders)
scraper.initialize().await?;

// One-time scrape
let files = scraper.get_available_files().await?;
for file in files.iter().take(5) {
    let result = scraper.process_single_file(&file.filename).await?;
    println!("Processed {}: {} events in {:.2}s",
        file.filename,
        result.valid_events,
        result.processing_time_seconds
    );
}

// Continuous mode
scraper.start().await?; // Runs indefinitely

// Get status
let status = scraper.get_comprehensive_status().await?;
println!("Running: {}, Uptime: {:.1}s, Files: {}",
    status.running,
    status.uptime_seconds,
    status.total_files_processed
);
```

**Sub-components**:

**Archive Scraper** (`archive_scraper.rs`): GHArchive.org interaction
```rust
use github_archiver::scraper::ArchiveScraper;

let scraper = ArchiveScraper::new(config.clone());
let available_files = scraper.list_available_files().await?;
println!("Available: {} files", available_files.len());
```

**File Processor** (`file_processor.rs`): JSON parsing + validation
```rust
use github_archiver::scraper::FileProcessor;

let processor = FileProcessor::new(config.clone());
let result = processor.process_file_content(&json_data).await?;
println!("Parsed {} events, {} valid", 
    result.total_events,
    result.valid_events
);
```

**Downloader** (`downloader.rs`): Concurrent HTTP downloads
```rust
use github_archiver::scraper::Downloader;

let downloader = Downloader::new(config.download.clone());
let result = downloader.download(
    "https://data.gharchive.org/2024-01-01-0.json.gz",
    "/tmp/archive.json.gz"
).await?;

match result.status {
    DownloadStatus::Success => println!("Downloaded {} bytes", result.size_bytes),
    DownloadStatus::Failed => println!("Failed: {}", result.error.unwrap()),
    DownloadStatus::Skipped => println!("Already exists"),
}
```

---

### Layer 3: Schema Management (Advanced)
**Location**: `src/schema/`

#### 3.1 Schema Management System
**Purpose**: Dynamic multi-source schema evolution + migrations

```rust
use github_archiver::schema::{SchemaManager, MigrationEngine};
use sqlx::PgPool;

let pool = /* your PgPool */;

// Initialize schema manager
let schema_manager = SchemaManager::new(pool.clone()).await?;

// Introspect current schema
let current_schema = schema_manager.introspect_full_schema().await?;
println!("Found {} tables", current_schema.tables.len());

// Detect schema changes
let changes = schema_manager.detect_schema_changes(&old_schema, &new_schema).await?;
for change in &changes {
    println!("Change: {:?}", change);
}

// Migration engine
let migration_engine = MigrationEngine::new(pool.clone()).await?;

// List pending migrations
let pending = migration_engine.list_pending_migrations().await?;
println!("Pending: {} migrations", pending.len());

// Apply migration
migration_engine.apply_migration(&migration_id).await?;

// Rollback if needed
migration_engine.rollback_migration(&migration_id).await?;
```

**Key Features**:
- Automatic schema introspection
- Conflict detection between modules
- Migration generation from schema diffs
- Rollback support with checksums
- Materialized view management

#### 3.2 Conflict Resolution
```rust
use github_archiver::schema::ConflictResolver;

let resolver = ConflictResolver::new(pool.clone()).await?;

// Detect conflicts
let conflicts = resolver.scan_for_conflicts().await?;
for conflict in conflicts {
    println!("Conflict in {}.{}: {}",
        conflict.table_name,
        conflict.column_name,
        conflict.conflict_type
    );
}

// Resolve automatically
let resolution = resolver.auto_resolve_conflicts(&conflicts).await?;
println!("Resolved: {}/{} conflicts",
    resolution.resolved_count,
    conflicts.len()
);
```

---

### Layer 4: Multi-Source Data (`sources/`)
**Location**: `src/sources/`

#### 4.1 Source Manager
**Purpose**: Manage diverse data sources with unified interface

```rust
use github_archiver::sources::{SourceManager, DataSource, SourceType};

let source_manager = SourceManager::new(pool.clone()).await?;

// Register new source
let github_source = DataSource {
    id: Uuid::new_v4(),
    name: "github_api".to_string(),
    display_name: "GitHub REST API".to_string(),
    source_type: SourceType::API {
        base_url: "https://api.github.com".to_string(),
        auth_method: AuthMethod::Bearer { 
            token: env::var("GITHUB_TOKEN")?,
            refresh_token: None,
            expires_at: None,
        },
        rate_limit: Some(RateLimitConfig {
            requests_per_minute: 5000,
            burst_limit: 100,
        }),
        endpoints: vec![/* ... */],
    },
    enabled: true,
    // ... other fields
};

source_manager.register_source(github_source).await?;

// Sync data from source
let sync_result = source_manager.sync_source(&source_id).await?;
println!("Synced {} records in {:.2}s",
    sync_result.records_processed,
    sync_result.duration_seconds
);

// Health check all sources
let health_status = source_manager.check_all_sources_health().await?;
for (source_id, status) in health_status {
    println!("Source {}: {:?}", source_id, status);
}
```

**Supported Source Types**:
- API endpoints (REST/GraphQL)
- Databases (PostgreSQL, MySQL, etc.)
- File systems (CSV, JSON, Parquet)
- Streams (WebSocket, Kafka)
- Webhooks (incoming data)
- RSS feeds
- Custom connectors

---

### Layer 5: Tree Visualization (`tree/`)
**Location**: `src/tree/`

#### 5.1 Tree Manager
**Purpose**: Build & visualize hierarchical data relationships

```rust
use github_archiver::tree::{TreeManager, TreeNode, NodeType};

let tree_manager = TreeManager::new(pool.clone()).await?;

// Build tree from data
let tree = tree_manager.build_tree_from_source(&source_id).await?;

// Add node
let node = TreeNode {
    id: Uuid::new_v4(),
    node_type: NodeType::Entity {
        entity_type: "repository".to_string(),
        schema_name: "github".to_string(),
    },
    label: "torvalds/linux".to_string(),
    data: /* ... */,
    children: vec![],
    // ... other fields
};

tree_manager.add_node(&tree.id, node).await?;

// Export tree for visualization
let export = tree_manager.export_tree_json(&tree.id).await?;
// Send to frontend: JSON with nodes + relationships

// Search tree
let search_results = tree_manager.search_tree(&tree.id, "linux").await?;
println!("Found {} matching nodes", search_results.len());
```

---

### Layer 6: API Layer (`api/`)
**Location**: `src/api/`

#### 6.1 API Server
**Purpose**: REST API with authentication

```rust
use github_archiver::api::{ApiServer, AppState};
use github_archiver::core::{Config, Database};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::default();
    let database = Arc::new(Database::new(config.clone()).await?);
    
    // Create application state
    let app_state = AppState::new(config.clone(), database.clone());
    
    // Initialize scraper if needed
    app_state.initialize_main_scraper().await?;
    
    // Start API server
    let server = ApiServer::new(config).await?;
    server.start().await?;
    
    Ok(())
}
```

**Available Endpoints**:

**Health & Status**:
```bash
# Health check
curl http://localhost:8081/health

# System status
curl http://localhost:8081/api/status

# Database status
curl http://localhost:8081/api/database/status

# Database statistics
curl http://localhost:8081/api/database/stats
```

**Authentication**:
```bash
# Login
curl -X POST http://localhost:8081/api/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"admin\",\"password\":\"${ADMIN_PASSWORD:?set ADMIN_PASSWORD}\"}"

# Response: {"token": "...", "expires_at": "..."}

# Use token
curl http://localhost:8081/api/auth/user \
  -H "Authorization: Bearer <token>"
```

**Scraper Control**:
```bash
# Start scraper
curl -X POST http://localhost:8081/api/start-scraper \
  -H "Authorization: Bearer <token>"

# Stop scraper
curl -X POST http://localhost:8081/api/stop-scraper \
  -H "Authorization: Bearer <token>"

# Scraper status
curl http://localhost:8081/api/scraper/status
```

**Scanner Operations**:
```bash
# Scan repository
curl -X POST http://localhost:8081/api/scanner/scan \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "repository_url": "https://github.com/user/repo",
    "scan_type": "secrets",
    "depth": "full"
  }'

# Get scan results
curl http://localhost:8081/api/scanner/results?scan_id=<id>

# Scan statistics
curl http://localhost:8081/api/scanner/statistics
```

---

## Data Flow & Interactions

### Typical Scraping Flow

```
1. User/Scheduler triggers scraping
   │
   ├──> MainScraper.start()
        │
        ├──> ArchiveScraper.list_available_files()
        │    └─> HTTP GET https://data.gharchive.org/
        │
        ├──> Downloader.download(url, path)
        │    ├─> Concurrent downloads (max 6)
        │    ├─> Retry logic (3 attempts)
        │    └─> Progress tracking
        │
        ├──> FileProcessor.process_file_content(json)
        │    ├─> Decompress gzip
        │    ├─> Parse JSON lines
        │    ├─> Validate each event
        │    └─> Extract metadata
        │
        ├──> Database.insert_events_batch(events)
        │    ├─> Transaction start
        │    ├─> Bulk insert with ON CONFLICT
        │    └─> Transaction commit
        │
        └──> Database.mark_file_processed(filename)
             └─> Track processing metadata
```

### Multi-Source Integration Flow

```
1. Register Data Source
   │
   ├──> SourceManager.register_source(source)
        ├─> Validate connection config
        ├─> Test connectivity
        └─> Store in data_sources table
        
2. Introspect Source Schema
   │
   ├──> SchemaManager.introspect_source(source_id)
        ├─> Connect to source
        ├─> Discover tables/collections
        ├─> Infer data types
        └─> Detect relationships
        
3. Generate Target Schema
   │
   ├──> SchemaEvolutionEngine.propose_schema(sample_data)
        ├─> Analyze data patterns
        ├─> Generate PostgreSQL DDL
        ├─> Create migration script
        └─> Register schema version
        
4. Apply Migration
   │
   ├──> MigrationEngine.apply_migration(migration_id)
        ├─> Begin transaction
        ├─> Execute DDL statements
        ├─> Update schema_versions
        └─> Commit transaction
        
5. Sync Data
   │
   ├──> SourceManager.sync_source(source_id)
        ├─> Fetch data from source
        ├─> Transform to target schema
        ├─> Batch insert to PostgreSQL
        └─> Update sync metadata
        
6. Build Tree Visualization
   │
   └──> TreeManager.build_tree_from_source(source_id)
        ├─> Query relationships
        ├─> Build hierarchy
        ├─> Calculate statistics
        └─> Export JSON for frontend
```

---

## Practical Usage Examples

### Example 1: Complete Setup from Scratch

```bash
#!/bin/bash
# Complete system initialization

# 1. Start PostgreSQL
docker-compose up -d postgres

# 2. Set environment variables
export DB_HOST=localhost
export DB_PORT=5432
export DB_NAME=github_archiver
export DB_USER=github_archiver
export DB_PASSWORD=github_archiver_password
export GITHUB_TOKEN=<your_token_here>

# 3. Build and run
cargo build --release
cargo run --release --bin web_server &

# 4. Wait for startup
sleep 5

# 5. Login
TOKEN=$(curl -s -X POST http://localhost:8081/api/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"admin\",\"password\":\"${ADMIN_PASSWORD:?set ADMIN_PASSWORD}\"}" \
  | jq -r '.token')

echo "Token: $TOKEN"

# 6. Start scraper
curl -X POST http://localhost:8081/api/start-scraper \
  -H "Authorization: Bearer $TOKEN"

# 7. Monitor progress
watch -n 2 'curl -s http://localhost:8081/api/scraper/status | jq .'
```

### Example 2: Programmatic Library Usage

```rust
// src/examples/custom_analysis.rs
use github_archiver::core::{Config, Database};
use github_archiver::scraper::MainScraper;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize
    let config = Config::default();
    let database = Database::new(config.clone()).await?;
    
    // 2. Query existing data
    let stats = database.get_database_statistics().await?;
    println!("Existing events: {}", stats.total_events);
    
    // 3. Run scraper for specific hour
    let mut scraper = MainScraper::new(config)?;
    scraper.initialize().await?;
    
    let result = scraper.process_single_file(
        "2024-01-01-15.json.gz"
    ).await?;
    
    println!("Processed: {} events", result.valid_events);
    
    // 4. Custom analysis query
    let quality = database.get_data_quality_metrics().await?;
    println!("Unique actors: {}", quality.unique_actors);
    println!("Quality score: {:.2}", quality.quality_score);
    
    // 5. Cleanup
    database.close().await;
    Ok(())
}
```

### Example 3: Multi-Source Integration

```rust
use github_archiver::sources::{SourceManager, DataSource, SourceType};
use github_archiver::schema::SchemaManager;
use github_archiver::tree::TreeManager;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = /* PgPool */;
    
    // 1. Register CSV source
    let source_manager = SourceManager::new(pool.clone()).await?;
    let csv_source = DataSource {
        name: "bug_bounty_reports".to_string(),
        source_type: SourceType::File {
            path: "/data/reports.csv".to_string(),
            format: FileFormat::CSV,
            watch_for_changes: true,
            compression: None,
        },
        // ... other config
    };
    
    let source_id = source_manager.register_source(csv_source).await?;
    
    // 2. Introspect and create schema
    let schema_manager = SchemaManager::new(pool.clone()).await?;
    let detected_schema = schema_manager.introspect_source(&source_id).await?;
    
    // 3. Sync data
    let sync_result = source_manager.sync_source(&source_id).await?;
    println!("Synced {} records", sync_result.records_processed);
    
    // 4. Build visualization tree
    let tree_manager = TreeManager::new(pool.clone()).await?;
    let tree = tree_manager.build_tree_from_source(&source_id).await?;
    
    // 5. Export for frontend
    let tree_json = tree_manager.export_tree_json(&tree.id).await?;
    std::fs::write("tree.json", tree_json)?;
    
    Ok(())
}
```

---

## Extension Points

### Adding New Data Source Connectors

```rust
// src/sources/connectors/my_connector.rs
use super::*;

pub struct MyCustomConnector {
    config: CustomConfig,
}

impl Connector for MyCustomConnector {
    async fn connect(&self) -> Result<()> {
        // Implement connection logic
        Ok(())
    }
    
    async fn fetch_data(&self) -> Result<Vec<DataRecord>> {
        // Implement data fetching
        Ok(vec![])
    }
    
    async fn introspect_schema(&self) -> Result<SchemaDefinition> {
        // Implement schema introspection
        Ok(SchemaDefinition::default())
    }
}

// Register in connectors.rs
pub fn create_connector(source_type: &SourceType) -> Box<dyn Connector> {
    match source_type {
        SourceType::Custom { connector_name, .. } if connector_name == "my_custom" => {
            Box::new(MyCustomConnector::new(/* */))
        }
        // ... other types
    }
}
```

### Adding New API Endpoints

```rust
// src/api/handlers.rs
pub async fn my_custom_endpoint(
    State(app_state): State<AppState>,
    Json(request): Json<MyRequest>,
) -> impl IntoResponse {
    // Your logic here
    Json(json!({
        "success": true,
        "data": /* ... */
    }))
}

// src/api/routes.rs
pub fn create_routes(app_state: AppState) -> Router {
    Router::new()
        .route("/api/my-endpoint", post(my_custom_endpoint))
        // ... other routes
        .with_state(app_state)
}
```

### Adding Secret Detection Patterns

```rust
// src/secrets/scanner.rs
impl SecretScanner {
    pub fn add_custom_pattern(&mut self, name: &str, regex: &str) {
        self.patterns.insert(
            name.to_string(),
            Regex::new(regex).unwrap()
        );
    }
}

// Usage
let mut scanner = SecretScanner::new();
scanner.add_custom_pattern(
    "custom_api_key",
    r"CUSTOM_[A-Z0-9]{32}"
);
```

---

## Summary: Key Takeaways

### Core Components
1. **Config** → Central configuration with env vars
2. **Database** → PostgreSQL pool with auto-reconnect
3. **Scraper** → GitHub Archive download + process
4. **Schema Manager** → Dynamic schema evolution
5. **Source Manager** → Multi-source integration
6. **Tree Manager** → Hierarchical visualization
7. **API Server** → REST endpoints with auth

### Primary Use Cases
- **Archiving**: Continuous GitHub event collection
- **Security**: Secret scanning across repos
- **Analysis**: Historical trend analysis
- **Integration**: Combine multiple data sources
- **Visualization**: Interactive data exploration

### Getting Started Checklist
```bash
✓ Start PostgreSQL (docker-compose up -d postgres)
✓ Set environment variables (DB_*, GITHUB_TOKEN)
✓ Build project (cargo build --release)
✓ Run server (cargo run --bin web_server)
✓ Login via API (/api/auth/login)
✓ Start scraper (/api/start-scraper)
✓ Monitor status (/api/status)
```

### Next Steps
- Read `README.md` for deployment details
- Check `src/schema/docs.rs` for migration examples
- Explore `src/examples/` for code snippets
- Review API docs at `/api/health` endpoint

---

**Need Help?**
- Check logs: `docker logs github_archiver_db`
- Database shell: `psql postgresql://github_archiver:github_archiver_password@localhost:5432/github_archiver`
- API health: `curl http://localhost:8081/health`
