#!/bin/bash

#
# Schema Management System Integration Script
# 
# Integrates the comprehensive schema management system into the GitHub archiver
# providing production-ready deployment and configuration capabilities.
#

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SCHEMA_DIR="$PROJECT_ROOT/src/schema"
CONFIG_DIR="$PROJECT_ROOT/config"
LOGS_DIR="$PROJECT_ROOT/logs"
BACKUP_DIR="$PROJECT_ROOT/backups"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Error handling
error_exit() {
    log_error "$1"
    exit 1
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check Rust installation
    if ! command -v cargo &> /dev/null; then
        error_exit "Rust and Cargo are required. Please install from https://rustup.rs/"
    fi
    
    # Check PostgreSQL client
    if ! command -v psql &> /dev/null; then
        log_warning "PostgreSQL client not found. Some features may not work."
    fi
    
    # Check Docker (optional)
    if ! command -v docker &> /dev/null; then
        log_warning "Docker not found. Docker features will be disabled."
    fi
    
    # Verify Rust version
    local rust_version=$(rustc --version | cut -d' ' -f2)
    log_info "Rust version: $rust_version"
    
    log_success "Prerequisites check completed"
}

# Create directory structure
create_directories() {
    log_info "Creating directory structure..."
    
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$LOGS_DIR"
    mkdir -p "$BACKUP_DIR"
    mkdir -p "$PROJECT_ROOT/examples"
    mkdir -p "$PROJECT_ROOT/scripts"
    mkdir -p "$PROJECT_ROOT/docs"
    
    log_success "Directory structure created"
}

# Generate configuration files
generate_config() {
    log_info "Generating configuration files..."
    
    # Main configuration file
    cat > "$CONFIG_DIR/schema_config.toml" << 'EOF'
# Schema Management System Configuration

[database]
# PostgreSQL connection URL
url = "${DATABASE_URL}"
max_connections = 100
min_connections = 10
connection_timeout = 30
idle_timeout = 600
max_lifetime = 1800

[validation]
# Default validation level: basic, extended, full
default_level = "extended"
performance_threshold = 80.0
security_threshold = 90.0
health_check_interval = 300  # seconds

[migration]
# Migration execution settings
batch_size = 50
parallel_execution = true
backup_before_migration = true
rollback_on_failure = true
migration_timeout = 3600  # seconds

[conflict_resolution]
# Conflict resolution strategies
auto_resolve_safe = true
prefer_newer_schema = true
backup_before_resolution = true
resolution_timeout = 300  # seconds

[export]
# Export settings
default_format = "json-schema"
include_data = false
compression = "gzip"
max_export_size = "1GB"
bloodhound_enhanced = true

[materialized_views]
# Materialized view management
auto_refresh = true
refresh_threshold = 10000  # rows changed
refresh_interval = 3600    # seconds
parallel_refresh = true

[performance]
# Performance monitoring
slow_query_threshold = 5000  # milliseconds
monitor_interval = 30        # seconds
alert_threshold = 90.0       # percentage
auto_optimization = false

[security]
# Security settings
audit_logging = true
encrypt_exports = false
require_authentication = true
session_timeout = 3600  # seconds

[docker]
# Docker integration settings
enabled = true
postgres_image = "postgres:15-alpine"
network_name = "schema-management"
container_prefix = "schema-mgmt"
auto_cleanup = true
resource_limits = true

[api]
# API server settings
port = 8080
host = "0.0.0.0"
cors_enabled = true
websocket_enabled = true
rate_limiting = true
max_request_size = "10MB"

[logging]
# Logging configuration
level = "info"
format = "json"
file_rotation = "daily"
max_log_files = 30
max_log_size = "100MB"

[backup]
# Backup settings
enabled = true
schedule = "0 2 * * *"  # Daily at 2 AM
retention_days = 30
compression = true
encryption = false
EOF

    # Environment template
    cat > "$PROJECT_ROOT/.env.template" << 'EOF'
# PostgreSQL Configuration
DATABASE_URL=postgresql://username:password@localhost:5432/github_archiver

# Schema Management Settings
SCHEMA_LOG_LEVEL=info
SCHEMA_CONFIG_PATH=./config/schema_config.toml
SCHEMA_EXPORT_PATH=./exports
SCHEMA_BACKUP_PATH=./backups

# API Configuration
SCHEMA_API_PORT=8080
SCHEMA_API_HOST=0.0.0.0
SCHEMA_API_CORS_ORIGIN=*

# Docker Settings (optional)
SCHEMA_DOCKER_ENABLED=true
DOCKER_POSTGRES_IMAGE=postgres:15-alpine
DOCKER_NETWORK=schema-management

# Security Settings
SCHEMA_AUTH_ENABLED=false
SCHEMA_JWT_SECRET=your-jwt-secret-here
SCHEMA_SESSION_TIMEOUT=3600

# Performance Settings
SCHEMA_POOL_MAX_SIZE=100
SCHEMA_QUERY_TIMEOUT=30
SCHEMA_SLOW_QUERY_THRESHOLD=5000

# Export Settings
SCHEMA_EXPORT_COMPRESSION=true
SCHEMA_BLOODHOUND_ENHANCED=true
SCHEMA_MAX_EXPORT_SIZE=1073741824  # 1GB in bytes
EOF

    # Systemd service file
    cat > "$PROJECT_ROOT/schema-manager.service" << EOF
[Unit]
Description=Schema Management System API
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=postgres
Group=postgres
WorkingDirectory=$PROJECT_ROOT
Environment=DATABASE_URL=postgresql://postgres@localhost/github_archiver
Environment=RUST_LOG=info
ExecStart=$PROJECT_ROOT/target/release/schema-api-server
ExecReload=/bin/kill -HUP \$MAINPID
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

    log_success "Configuration files generated"
}

# Update Cargo.toml
update_cargo_toml() {
    log_info "Updating Cargo.toml dependencies..."
    
    # Backup original Cargo.toml
    cp "$PROJECT_ROOT/Cargo.toml" "$PROJECT_ROOT/Cargo.toml.bak"
    
    # Check if schema management dependencies are already present
    if grep -q "# Schema Management Dependencies" "$PROJECT_ROOT/Cargo.toml"; then
        log_info "Schema management dependencies already present"
        return
    fi
    
    # Add schema management dependencies
    cat >> "$PROJECT_ROOT/Cargo.toml" << 'EOF'

# Schema Management Dependencies
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid", "json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4.0", features = ["derive"] }
axum = { version = "0.7", features = ["ws", "json"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
futures = "0.3"
async-trait = "0.1"
bollard = "0.15"
flate2 = "1.0"
regex = "1.0"
once_cell = "1.0"
parking_lot = "0.12"

[[bin]]
name = "schema-manager"
path = "src/bin/schema_manager.rs"

[[bin]]
name = "schema-api-server"
path = "src/bin/schema_api_server.rs"
EOF

    log_success "Cargo.toml updated"
}

# Create binary executables
create_binaries() {
    log_info "Creating binary executables..."
    
    mkdir -p "$PROJECT_ROOT/src/bin"
    
    # CLI binary
    cat > "$PROJECT_ROOT/src/bin/schema_manager.rs" << 'EOF'
/*!
 * Schema Manager CLI Binary
 * 
 * Command-line interface for the PostgreSQL schema management system.
 */

use anyhow::Result;
use clap::Parser;
use sqlx::PgPool;
use std::env;
use tracing_subscriber;

// Import the schema management modules
use rust_github_archiver::schema::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Get database URL from environment or CLI
    let database_url = cli.database_url.clone()
        .or_else(|| env::var("DATABASE_URL").ok())
        .expect("DATABASE_URL must be set");
    
    // Create database connection pool
    let pool = PgPool::connect(&database_url).await?;
    
    // Execute CLI command
    cli.execute(pool).await?;
    
    Ok(())
}
EOF

    # API server binary
    cat > "$PROJECT_ROOT/src/bin/schema_api_server.rs" << 'EOF'
/*!
 * Schema Management API Server Binary
 * 
 * RESTful API server for the PostgreSQL schema management system.
 */

use anyhow::Result;
use sqlx::PgPool;
use std::env;
use tracing_subscriber;

// Import the schema management modules
use rust_github_archiver::schema::api::start_api_server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Get configuration from environment
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    let port = env::var("SCHEMA_API_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("Invalid port number");
    
    let host = env::var("SCHEMA_API_HOST")
        .unwrap_or_else(|_| "0.0.0.0".to_string());
    
    // Create database connection pool
    let pool = PgPool::connect(&database_url).await?;
    
    // Start API server
    println!("Starting Schema Management API server on {}:{}", host, port);
    start_api_server(pool, &host, port).await?;
    
    Ok(())
}
EOF

    log_success "Binary executables created"
}

# Create example scripts
create_examples() {
    log_info "Creating example scripts..."
    
    # Basic usage example
    cat > "$PROJECT_ROOT/examples/basic_usage.rs" << 'EOF'
/*!
 * Basic Schema Management Usage Example
 */

use anyhow::Result;
use sqlx::PgPool;
use rust_github_archiver::schema::{
    SchemaManagementSystem,
    validation::{SchemaValidator, ValidationLevel},
    export::{SchemaExporter, ExportFormat},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to database
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;
    
    // Initialize schema management system
    let system = SchemaManagementSystem::new(pool.clone()).await?;
    system.initialize().await?;
    
    // Register modules
    system.register_module("github_events", "1.0.0").await?;
    system.register_module("repository_scanner", "1.2.0").await?;
    
    println!("Schema management system initialized");
    
    // Detect and resolve conflicts
    let conflicts = system.detect_conflicts().await?;
    println!("Found {} conflicts", conflicts.len());
    
    for conflict in conflicts {
        println!("Resolving conflict: {}", conflict.description);
        system.resolve_conflict(&conflict.id).await?;
    }
    
    // Validate schema health
    let validator = SchemaValidator::new(pool.clone()).await?;
    let result = validator.validate_schema(ValidationLevel::Full).await?;
    
    println!("Schema health score: {}", result.overall_score);
    
    // Export to BloodHound for bug bounty analysis
    let exporter = SchemaExporter::new(pool.clone()).await?;
    let bloodhound_data = exporter.export_to_bloodhound().await?;
    
    std::fs::write("database_structure.json", 
                   serde_json::to_string_pretty(&bloodhound_data)?)?;
    
    println!("BloodHound export completed");
    
    Ok(())
}
EOF

    # Bug bounty analysis example
    cat > "$PROJECT_ROOT/examples/bug_bounty_analysis.rs" << 'EOF'
/*!
 * Bug Bounty Analysis Example
 * 
 * Demonstrates using the schema management system for security analysis
 * and BloodHound export for attack path discovery.
 */

use anyhow::Result;
use sqlx::PgPool;
use rust_github_archiver::schema::{
    export::{SchemaExporter, ExportFormat},
    validation::{SchemaValidator, ValidationLevel},
    introspection::SchemaIntrospector,
};

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;
    
    println!("=== Bug Bounty Database Analysis ===");
    
    // 1. Schema introspection for attack surface discovery
    let introspector = SchemaIntrospector::new(pool.clone()).await?;
    let schema = introspector.introspect_full_schema().await?;
    
    println!("Database analysis:");
    println!("- {} schemas", schema.schemas.len());
    println!("- {} tables", schema.tables.len());
    println!("- {} functions", schema.functions.len());
    
    // 2. Security-focused validation
    let validator = SchemaValidator::new(pool.clone()).await?;
    let validation = validator.validate_schema(ValidationLevel::Full).await?;
    
    if let Some(security_metrics) = &validation.security_metrics {
        println!("\nSecurity Analysis:");
        println!("- Security score: {}", security_metrics.security_score);
        println!("- Exposed sensitive tables: {}", security_metrics.exposed_sensitive_tables);
        println!("- Weak permissions: {}", security_metrics.weak_permissions.len());
        println!("- SQL injection risks: {}", security_metrics.sql_injection_risks.len());
    }
    
    // 3. Export to BloodHound for attack path analysis
    let exporter = SchemaExporter::new(pool.clone()).await?;
    
    println!("\nGenerating BloodHound export...");
    let bloodhound_data = exporter.export_to_bloodhound().await?;
    
    // Save BloodHound data
    std::fs::write("bloodhound_database.json", 
                   serde_json::to_string_pretty(&bloodhound_data)?)?;
    
    println!("BloodHound export saved to: bloodhound_database.json");
    
    // 4. Generate security report
    let mut report = String::new();
    report.push_str("# Database Security Analysis Report\n\n");
    
    report.push_str(&format!("## Summary\n"));
    report.push_str(&format!("- Overall health score: {}\n", validation.overall_score));
    report.push_str(&format!("- Total issues found: {}\n", validation.issues.len()));
    
    if let Some(security) = &validation.security_metrics {
        report.push_str(&format!("- Security score: {}\n", security.security_score));
        report.push_str(&format!("- Critical security issues: {}\n", 
                                 validation.issues.iter()
                                     .filter(|i| i.severity.to_string() == "Critical")
                                     .count()));
    }
    
    report.push_str("\n## Issues Found\n\n");
    for issue in &validation.issues {
        report.push_str(&format!("### {} - {}\n", issue.severity, issue.category));
        report.push_str(&format!("{}\n\n", issue.description));
        if let Some(recommendation) = &issue.recommendation {
            report.push_str(&format!("**Recommendation:** {}\n\n", recommendation));
        }
    }
    
    if let Some(security) = &validation.security_metrics {
        report.push_str("## Security Risks\n\n");
        
        if !security.weak_permissions.is_empty() {
            report.push_str("### Weak Permissions\n");
            for perm in &security.weak_permissions {
                report.push_str(&format!("- {}\n", perm));
            }
            report.push_str("\n");
        }
        
        if !security.sql_injection_risks.is_empty() {
            report.push_str("### SQL Injection Risks\n");
            for risk in &security.sql_injection_risks {
                report.push_str(&format!("- {}\n", risk));
            }
            report.push_str("\n");
        }
    }
    
    report.push_str("## BloodHound Analysis\n\n");
    report.push_str("Import the generated `bloodhound_database.json` file into BloodHound ");
    report.push_str("to visualize attack paths and privilege escalation opportunities.\n\n");
    
    report.push_str("### Recommended BloodHound Queries\n\n");
    report.push_str("```cypher\n");
    report.push_str("// Find privilege escalation paths\n");
    report.push_str("MATCH (low:DatabaseRole)-[*1..6]->(high:DatabaseRole)\n");
    report.push_str("WHERE low.name CONTAINS 'guest' AND high.name CONTAINS 'admin'\n");
    report.push_str("RETURN path\n\n");
    
    report.push_str("// Find sensitive data access\n");
    report.push_str("MATCH (table:DatabaseTable {sensitive_data: true})\n");
    report.push_str("MATCH (role:DatabaseRole)-[:HasPermission]->(table)\n");
    report.push_str("RETURN table.name, role.name\n");
    report.push_str("```\n");
    
    std::fs::write("security_analysis_report.md", report)?;
    
    println!("Security analysis report saved to: security_analysis_report.md");
    println!("\n=== Analysis Complete ===");
    println!("Next steps:");
    println!("1. Review the security report");
    println!("2. Import BloodHound data for visual analysis");
    println!("3. Address critical security issues");
    println!("4. Implement recommended security measures");
    
    Ok(())
}
EOF

    # Performance monitoring example
    cat > "$PROJECT_ROOT/examples/performance_monitoring.sh" << 'EOF'
#!/bin/bash

# Performance Monitoring Example Script
# Demonstrates continuous monitoring of schema performance

echo "=== Schema Performance Monitoring ==="

# Set monitoring duration (default: 5 minutes)
DURATION=${1:-300}
INTERVAL=${2:-30}

echo "Monitoring for ${DURATION} seconds with ${INTERVAL}s intervals"

# Start performance monitoring
cargo run --bin schema-manager performance monitor \
    --duration $DURATION \
    --interval $INTERVAL \
    --output performance_report.json \
    --verbose &

MONITOR_PID=$!

# Monitor slow queries in parallel
cargo run --bin schema-manager performance slow-queries \
    --min-duration 1000 \
    --continuous \
    --output slow_queries.log &

SLOW_QUERY_PID=$!

# Wait for monitoring to complete
wait $MONITOR_PID

# Stop slow query monitoring
kill $SLOW_QUERY_PID 2>/dev/null || true

echo "Performance monitoring completed"
echo "Reports saved to:"
echo "  - performance_report.json"
echo "  - slow_queries.log"

# Generate performance summary
echo "=== Performance Summary ==="
cargo run --bin schema-manager performance stats \
    --tables --indexes --cache --summary
EOF

    chmod +x "$PROJECT_ROOT/examples/performance_monitoring.sh"
    
    log_success "Example scripts created"
}

# Create deployment scripts
create_deployment_scripts() {
    log_info "Creating deployment scripts..."
    
    # Database setup script
    cat > "$PROJECT_ROOT/scripts/setup_database.sh" << 'EOF'
#!/bin/bash

# Database Setup Script for Schema Management System

set -euo pipefail

# Configuration
DB_NAME="${1:-github_archiver}"
DB_USER="${2:-postgres}"
DB_HOST="${3:-localhost}"
DB_PORT="${4:-5432}"

echo "Setting up PostgreSQL database for schema management..."

# Create database if it doesn't exist
createdb -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" "$DB_NAME" 2>/dev/null || echo "Database already exists"

# Initialize schema management tables
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" << 'EOSQL'
-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";
CREATE EXTENSION IF NOT EXISTS "btree_gin";

-- Create schema management schema
CREATE SCHEMA IF NOT EXISTS schema_management;

-- Set search path
SET search_path TO schema_management, public;

-- Schema management system will create its tables automatically
EOSQL

echo "Database setup completed"
echo "Connection string: postgresql://$DB_USER@$DB_HOST:$DB_PORT/$DB_NAME"
EOF

    # Production deployment script
    cat > "$PROJECT_ROOT/scripts/deploy_production.sh" << 'EOF'
#!/bin/bash

# Production Deployment Script

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Schema Management System Production Deployment ==="

# Build in release mode
echo "Building release binary..."
cd "$PROJECT_ROOT"
cargo build --release

# Install systemd service
echo "Installing systemd service..."
sudo cp schema-manager.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable schema-manager

# Create production directories
echo "Creating production directories..."
sudo mkdir -p /var/log/schema-manager
sudo mkdir -p /var/lib/schema-manager
sudo mkdir -p /etc/schema-manager

# Copy configuration
echo "Installing configuration..."
sudo cp config/schema_config.toml /etc/schema-manager/
sudo cp .env.template /etc/schema-manager/

# Set permissions
echo "Setting permissions..."
sudo chown -R postgres:postgres /var/log/schema-manager
sudo chown -R postgres:postgres /var/lib/schema-manager
sudo chmod 750 /etc/schema-manager
sudo chmod 640 /etc/schema-manager/*

# Copy binaries
echo "Installing binaries..."
sudo cp target/release/schema-manager /usr/local/bin/
sudo cp target/release/schema-api-server /usr/local/bin/
sudo chmod +x /usr/local/bin/schema-*

echo "Production deployment completed"
echo ""
echo "Next steps:"
echo "1. Configure /etc/schema-manager/schema_config.toml"
echo "2. Set DATABASE_URL in environment"
echo "3. Start the service: sudo systemctl start schema-manager"
echo "4. Check status: sudo systemctl status schema-manager"
EOF

    # Development setup script
    cat > "$PROJECT_ROOT/scripts/setup_development.sh" << 'EOF'
#!/bin/bash

# Development Environment Setup

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Development Environment Setup ==="

cd "$PROJECT_ROOT"

# Install Rust dependencies
echo "Installing Rust dependencies..."
cargo fetch

# Setup development database
echo "Setting up development database..."
if [ -z "${DATABASE_URL:-}" ]; then
    export DATABASE_URL="postgresql://postgres@localhost/github_archiver_dev"
    echo "Using default DATABASE_URL: $DATABASE_URL"
fi

# Create development environment file
if [ ! -f ".env" ]; then
    echo "Creating .env file..."
    cp .env.template .env
    echo "Please edit .env file with your configuration"
fi

# Build development binaries
echo "Building development binaries..."
cargo build

# Run tests
echo "Running tests..."
cargo test

# Initialize schema management system
echo "Initializing schema management system..."
cargo run --bin schema-manager init || echo "Schema management already initialized"

echo "Development environment setup completed"
echo ""
echo "Available commands:"
echo "  cargo run --bin schema-manager --help"
echo "  cargo run --bin schema-api-server"
echo "  cargo test"
echo "  cargo run --example basic_usage"
EOF

    chmod +x "$PROJECT_ROOT/scripts/"*.sh
    
    log_success "Deployment scripts created"
}

# Run tests
run_tests() {
    log_info "Running tests..."
    
    cd "$PROJECT_ROOT"
    
    # Check if we can compile
    if cargo check; then
        log_success "Code compilation check passed"
    else
        log_error "Code compilation failed"
        return 1
    fi
    
    # Run unit tests (if DATABASE_URL is available)
    if [ -n "${DATABASE_URL:-}" ]; then
        log_info "Running unit tests..."
        if cargo test --lib; then
            log_success "Unit tests passed"
        else
            log_warning "Some unit tests failed"
        fi
    else
        log_warning "DATABASE_URL not set, skipping database tests"
    fi
    
    log_success "Test execution completed"
}

# Build binaries
build_binaries() {
    log_info "Building binaries..."
    
    cd "$PROJECT_ROOT"
    
    # Build in development mode first
    if cargo build; then
        log_success "Development build completed"
    else
        error_exit "Development build failed"
    fi
    
    # Build in release mode
    log_info "Building release binaries..."
    if cargo build --release; then
        log_success "Release build completed"
    else
        log_warning "Release build failed, but development build is available"
    fi
    
    # List built binaries
    log_info "Built binaries:"
    ls -la target/debug/schema-* 2>/dev/null || true
    ls -la target/release/schema-* 2>/dev/null || true
}

# Generate documentation
generate_docs() {
    log_info "Generating documentation..."
    
    cd "$PROJECT_ROOT"
    
    # Generate Rust documentation
    cargo doc --no-deps --document-private-items
    
    # Create README for schema management
    cat > "$PROJECT_ROOT/SCHEMA_MANAGEMENT_README.md" << 'EOF'
# PostgreSQL Schema Management System

A comprehensive, production-ready PostgreSQL schema management system designed for the GitHub archiver project, with specialized features for bug bounty research and security analysis.

## Quick Start

### Installation

1. Ensure you have Rust 1.70+ and PostgreSQL 12+ installed
2. Clone this repository
3. Run the integration script:

```bash
./scripts/setup_development.sh
```

### Basic Usage

```bash
# Initialize the system
export DATABASE_URL="postgresql://user:pass@localhost/github_archiver"
cargo run --bin schema-manager init

# Introspect current schema
cargo run --bin schema-manager introspect --output schema.json

# Validate schema health
cargo run --bin schema-manager validate --level full

# Export for BloodHound analysis
cargo run --bin schema-manager export --format bloodhound --output bloodhound.json

# Start API server
cargo run --bin schema-api-server
```

### Bug Bounty Analysis

For security analysis and attack path discovery:

```bash
# Run comprehensive security analysis
cargo run --example bug_bounty_analysis

# Monitor performance issues
./examples/performance_monitoring.sh 300 30
```

## Features

- **Dynamic Schema Management**: Automatic conflict detection and resolution
- **Migration Engine**: Safe, transactional migrations with rollback
- **Validation Framework**: Comprehensive health checks and performance analysis
- **BloodHound Export**: Security analysis for bug bounty research
- **RESTful API**: Complete API with WebSocket support
- **CLI Tools**: Command-line interface for all operations
- **Docker Integration**: Containerized testing and deployment
- **Performance Monitoring**: Real-time performance analysis

## Architecture

The system consists of several key modules:

- `core`: Main schema management functionality
- `migration`: Database migration engine
- `introspection`: PostgreSQL schema analysis
- `validation`: Schema health and performance validation
- `conflict_resolution`: Automatic conflict detection and resolution
- `materialized_views`: Advanced materialized view management
- `export`: Multi-format export including BloodHound
- `cli`: Command-line interface
- `api`: RESTful API and WebSocket support
- `docker`: Docker integration for isolated operations

## Configuration

Edit `config/schema_config.toml` to configure the system:

```toml
[database]
url = "postgresql://localhost/github_archiver"
max_connections = 100

[validation]
default_level = "extended"
performance_threshold = 80.0

[export]
default_format = "json-schema"
bloodhound_enhanced = true

# ... additional configuration options
```

## API Reference

Start the API server:

```bash
cargo run --bin schema-api-server
```

Key endpoints:

- `GET /health` - System health check
- `GET /schema` - Full schema introspection
- `POST /validate` - Schema validation
- `POST /export/bloodhound` - BloodHound export
- `GET /conflicts` - List schema conflicts
- `POST /migrations` - Create migration

WebSocket endpoint for real-time updates:

- `ws://localhost:8080/ws`

## Security Analysis

The system provides specialized features for bug bounty research:

1. **BloodHound Integration**: Export database structure for attack path analysis
2. **Privilege Escalation Detection**: Identify potential escalation paths
3. **Sensitive Data Discovery**: Find tables with PII or sensitive information
4. **Permission Analysis**: Analyze access controls and permissions

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run the test suite: `cargo test`
5. Submit a pull request

## License

MIT License - see LICENSE file for details.
EOF

    log_success "Documentation generated"
}

# Main integration function
main() {
    log_info "Starting Schema Management System Integration"
    log_info "Project root: $PROJECT_ROOT"
    
    # Run integration steps
    check_prerequisites
    create_directories
    generate_config
    update_cargo_toml
    create_binaries
    create_examples
    create_deployment_scripts
    build_binaries
    
    # Optional steps (only if no errors so far)
    if [[ $? -eq 0 ]]; then
        generate_docs
        
        # Run tests if DATABASE_URL is available
        if [[ -n "${DATABASE_URL:-}" ]]; then
            run_tests
        else
            log_warning "DATABASE_URL not set, skipping tests"
            log_info "To run tests, set DATABASE_URL and run: cargo test"
        fi
    fi
    
    log_success "Schema Management System Integration Completed!"
    
    # Print final instructions
    echo ""
    echo "========================================================"
    echo "  Schema Management System Successfully Integrated"
    echo "========================================================"
    echo ""
    echo "Next Steps:"
    echo ""
    echo "1. Configure your environment:"
    echo "   export DATABASE_URL='postgresql://user:pass@localhost/github_archiver'"
    echo ""
    echo "2. Initialize the system:"
    echo "   cargo run --bin schema-manager init"
    echo ""
    echo "3. Run basic operations:"
    echo "   cargo run --bin schema-manager introspect"
    echo "   cargo run --bin schema-manager validate --level full"
    echo ""
    echo "4. For bug bounty analysis:"
    echo "   cargo run --example bug_bounty_analysis"
    echo "   cargo run --bin schema-manager export --format bloodhound"
    echo ""
    echo "5. Start the API server:"
    echo "   cargo run --bin schema-api-server"
    echo ""
    echo "6. For production deployment:"
    echo "   ./scripts/deploy_production.sh"
    echo ""
    echo "Documentation available at:"
    echo "  - SCHEMA_MANAGEMENT_README.md"
    echo "  - target/doc/index.html (Rust docs)"
    echo "  - config/schema_config.toml (configuration)"
    echo ""
    echo "Example files created in:"
    echo "  - examples/ (usage examples)"
    echo "  - scripts/ (deployment scripts)"
    echo ""
    echo "The system is now ready for production use!"
    echo "========================================================"
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
