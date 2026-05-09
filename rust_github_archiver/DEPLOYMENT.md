# Deployment Guide - GitHub Events API Monitoring System

## Table of Contents
1. [Prerequisites](#prerequisites)
2. [System Requirements](#system-requirements)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Deployment Options](#deployment-options)
6. [Systemd Service Setup](#systemd-service-setup)
7. [Nginx Reverse Proxy](#nginx-reverse-proxy)
8. [Database Setup](#database-setup)
9. [Monitoring & Logging](#monitoring--logging)
10. [Scaling Strategies](#scaling-strategies)
11. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Software
- **Rust**: 1.75+ (stable)
- **PostgreSQL**: 15.14+
- **Nginx**: 1.18+ (for reverse proxy)
- **Git**: 2.x
- **systemd**: (for service management)

### Optional Software
- **Docker**: 20.10+ (for containerized deployment)
- **Docker Compose**: 2.x (for multi-container setup)
- **Prometheus + Grafana**: (for advanced monitoring)

---

## System Requirements

### Minimum Requirements
- **CPU**: 2 cores (2.0 GHz+)
- **RAM**: 4 GB
- **Disk**: 50 GB SSD
- **Network**: 100 Mbps
- **OS**: Ubuntu 22.04 LTS, Debian 11+, RHEL 8+

### Recommended Requirements
- **CPU**: 4 cores (3.0 GHz+)
- **RAM**: 8 GB
- **Disk**: 200 GB SSD
- **Network**: 1 Gbps
- **OS**: Ubuntu 22.04 LTS

### Production Requirements (High Scale)
- **CPU**: 8+ cores (3.5 GHz+)
- **RAM**: 16 GB+
- **Disk**: 500 GB SSD (with auto-scaling)
- **Network**: 10 Gbps
- **OS**: Ubuntu 22.04 LTS

---

## Installation

### 1. Clone Repository
```bash
git clone https://github.com/Bit-Loop/GitArchiver.git
cd GitArchiver/rust_github_archiver
```

### 2. Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup update
```

### 3. Install PostgreSQL
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install postgresql postgresql-contrib

# Start PostgreSQL
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

### 4. Build Application
```bash
# Debug build (development)
cargo build --bin web_server

# Release build (production)
cargo build --release --bin web_server
```

### 5. Verify Build
```bash
./target/release/web_server --version
```

---

## Configuration

### 1. Environment Variables
Create `.env` file:
```bash
# Database
DATABASE_URL=postgresql://github_archiver:password@localhost/github_archiver

# GitHub API
GITHUB_TOKEN=ghp_REDACTED_EXAMPLE  # Optional

# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8081
RUST_LOG=info

# Monitoring
METRICS_ENABLED=true
PROMETHEUS_PORT=9090

# Rate Limiting
DEFAULT_RATE_LIMIT=5  # requests per minute
AUTO_ADJUST=true
```

### 2. Configuration File
Create `config.yaml`:
```yaml
server:
  host: "0.0.0.0"
  port: 8081
  workers: 4

database:
  url: "postgresql://github_archiver:password@localhost/github_archiver"
  max_connections: 20
  min_connections: 5
  connection_timeout: 30

github:
  token: ""  # Optional
  api_url: "https://api.github.com"
  events_endpoint: "/events"

rate_limiting:
  requests_per_minute: 5
  auto_adjust: true
  min_rate: 1
  max_rate: 60

monitoring:
  enabled: true
  metrics_interval: 60
  log_level: "info"
  log_file: "/var/log/github-archiver/app.log"

webhooks:
  max_retries: 3
  timeout_seconds: 10
  retry_delay_seconds: 2

token_pool:
  strategy: "round_robin"  # round_robin, least_used, best_health, most_remaining
  health_check_interval: 300

alerts:
  critical_secrets_webhook: "https://hooks.slack.com/services/YOUR/WEBHOOK/URL"
  high_severity_webhook: "https://discord.com/api/webhooks/YOUR/WEBHOOK"
```

---

## Deployment Options

### Option 1: Standalone Binary (Recommended for Development)

1. **Build and Run**:
```bash
cargo build --release
./target/release/web_server
```

2. **With nohup (background)**:
```bash
nohup ./target/release/web_server > /tmp/server.log 2>&1 &
```

3. **With environment variables**:
```bash
RUST_LOG=info DATABASE_URL=postgresql://... ./target/release/web_server
```

### Option 2: Systemd Service (Recommended for Production)

See [Systemd Service Setup](#systemd-service-setup) section below.

### Option 3: Docker (Containerized)

1. **Build Docker image**:
```bash
docker build -t github-archiver:latest .
```

2. **Run container**:
```bash
docker run -d \
  --name github-archiver \
  -p 8081:8081 \
  -e DATABASE_URL=postgresql://... \
  -e GITHUB_TOKEN=ghp_... \
  -v /var/log/github-archiver:/var/log/github-archiver \
  github-archiver:latest
```

3. **Docker Compose** (with PostgreSQL):
```yaml
version: '3.8'

services:
  postgres:
    image: postgres:15.14
    environment:
      POSTGRES_DB: github_archiver
      POSTGRES_USER: github_archiver
      POSTGRES_PASSWORD: secure_password
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  github_archiver:
    build: .
    depends_on:
      - postgres
    environment:
      DATABASE_URL: postgresql://github_archiver:secure_password@postgres/github_archiver
      RUST_LOG: info
    ports:
      - "8081:8081"
    volumes:
      - ./logs:/var/log/github-archiver
    restart: unless-stopped

volumes:
  postgres_data:
```

Run: `docker-compose up -d`

---

## Systemd Service Setup

### 1. Create Service File
```bash
sudo nano /etc/systemd/system/github-archiver.service
```

### 2. Service Configuration
```ini
[Unit]
Description=GitHub Events API Monitoring System
Documentation=https://github.com/Bit-Loop/GitArchiver
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=github-archiver
Group=github-archiver
WorkingDirectory=/opt/github-archiver

# Environment
Environment="RUST_LOG=info"
Environment="DATABASE_URL=postgresql://github_archiver:password@localhost/github_archiver"

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/github-archiver

# Execution
ExecStart=/opt/github-archiver/web_server
ExecReload=/bin/kill -HUP $MAINPID

# Restart policy
Restart=always
RestartSec=10
StartLimitBurst=5
StartLimitInterval=60

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=github-archiver

[Install]
WantedBy=multi-user.target
```

### 3. Create User and Directories
```bash
# Create user
sudo useradd -r -s /bin/false github-archiver

# Create directories
sudo mkdir -p /opt/github-archiver
sudo mkdir -p /var/log/github-archiver

# Copy binary
sudo cp target/release/web_server /opt/github-archiver/

# Set permissions
sudo chown -R github-archiver:github-archiver /opt/github-archiver
sudo chown -R github-archiver:github-archiver /var/log/github-archiver
```

### 4. Enable and Start Service
```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable github-archiver

# Start service
sudo systemctl start github-archiver

# Check status
sudo systemctl status github-archiver

# View logs
sudo journalctl -u github-archiver -f
```

### 5. Service Management Commands
```bash
# Stop service
sudo systemctl stop github-archiver

# Restart service
sudo systemctl restart github-archiver

# Reload configuration (without downtime)
sudo systemctl reload github-archiver

# Disable service
sudo systemctl disable github-archiver

# View logs (last 100 lines)
sudo journalctl -u github-archiver -n 100

# View logs (follow)
sudo journalctl -u github-archiver -f
```

---

## Nginx Reverse Proxy

### 1. Install Nginx
```bash
sudo apt install nginx
```

### 2. Create Nginx Configuration
```bash
sudo nano /etc/nginx/sites-available/github-archiver
```

### 3. Nginx Configuration
```nginx
upstream github_archiver {
    server 127.0.0.1:8081;
    keepalive 32;
}

server {
    listen 80;
    listen [::]:80;
    server_name your-domain.com;

    # Redirect HTTP to HTTPS
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name your-domain.com;

    # SSL certificates (use Let's Encrypt)
    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;
    
    # SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options DENY always;
    add_header X-Content-Type-Options nosniff always;
    add_header X-XSS-Protection "1; mode=block" always;

    # Logging
    access_log /var/log/nginx/github-archiver-access.log;
    error_log /var/log/nginx/github-archiver-error.log;

    # Proxy settings
    location / {
        proxy_pass http://github_archiver;
        proxy_http_version 1.1;
        
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        
        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # WebSocket support (for future real-time features)
    location /ws {
        proxy_pass http://github_archiver;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
    }

    # Static files (dashboard)
    location /dashboard {
        root /opt/github-archiver/public;
        try_files $uri $uri/ /dashboard.html;
    }
}
```

### 4. Enable Site
```bash
# Create symlink
sudo ln -s /etc/nginx/sites-available/github-archiver /etc/nginx/sites-enabled/

# Test configuration
sudo nginx -t

# Reload Nginx
sudo systemctl reload nginx
```

### 5. SSL with Let's Encrypt
```bash
# Install Certbot
sudo apt install certbot python3-certbot-nginx

# Obtain certificate
sudo certbot --nginx -d your-domain.com

# Auto-renewal (cron job)
sudo crontab -e
# Add: 0 0 * * * certbot renew --quiet
```

---

## Database Setup

### 1. Create Database and User
```bash
sudo -u postgres psql
```

```sql
-- Create user
CREATE USER github_archiver WITH PASSWORD 'secure_password';

-- Create database
CREATE DATABASE github_archiver OWNER github_archiver;

-- Grant privileges
GRANT ALL PRIVILEGES ON DATABASE github_archiver TO github_archiver;

-- Connect to database
\c github_archiver

-- Grant schema privileges
GRANT ALL ON SCHEMA public TO github_archiver;

-- Exit
\q
```

### 2. Run Migrations
```bash
# Migrations are auto-run on first start
# Or manually with sqlx
sqlx migrate run
```

### 3. Database Optimization
```sql
-- Create indexes
CREATE INDEX CONCURRENTLY idx_events_created_at ON github_events(event_created_at);
CREATE INDEX CONCURRENTLY idx_events_type ON github_events(event_type);
CREATE INDEX CONCURRENTLY idx_events_api_source ON github_events(api_source);
CREATE INDEX CONCURRENTLY idx_events_repo ON github_events(repo_name);

-- Partition table by month (for large datasets)
CREATE TABLE github_events_2025_10 PARTITION OF github_events
    FOR VALUES FROM ('2025-10-01') TO ('2025-11-01');

-- Auto-vacuum settings
ALTER TABLE github_events SET (autovacuum_vacuum_scale_factor = 0.05);
ALTER TABLE github_events SET (autovacuum_analyze_scale_factor = 0.02);
```

### 4. Backup Strategy
```bash
# Daily backup script
#!/bin/bash
BACKUP_DIR="/var/backups/github-archiver"
DATE=$(date +%Y-%m-%d)

pg_dump -U github_archiver github_archiver | gzip > "$BACKUP_DIR/backup-$DATE.sql.gz"

# Keep last 30 days
find "$BACKUP_DIR" -name "backup-*.sql.gz" -mtime +30 -delete
```

Add to cron:
```bash
0 2 * * * /opt/github-archiver/backup.sh
```

---

## Monitoring & Logging

### 1. Application Logs
```bash
# View logs
tail -f /var/log/github-archiver/app.log

# Or via systemd
journalctl -u github-archiver -f

# Search logs
journalctl -u github-archiver | grep ERROR
```

### 2. Metrics Endpoints
- `GET /api/health` - Health status
- `GET /api/metrics` - System metrics
- `GET /api/metrics/report` - Comprehensive report

### 3. Prometheus Integration
```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'github_archiver'
    static_configs:
      - targets: ['localhost:8081']
    metrics_path: '/api/metrics'
    scrape_interval: 15s
```

### 4. Grafana Dashboard
Import dashboard template from `monitoring/grafana-dashboard.json`

---

## Scaling Strategies

### Phase 1: Single Token (Free)
- **Capacity**: 60 req/hour = 1,800 events/hour
- **Cost**: $0
- **Setup**: Default configuration

### Phase 2: Multi-Token Rotation (Free)
- **Capacity**: 25,000-50,000 req/hour = 750K-1.5M events/hour
- **Cost**: $0 (requires 5-10 GitHub accounts)
- **Setup**:
```bash
curl -X POST http://localhost:8081/api/tokens/add \
  -H "Content-Type: application/json" \
  -d '{
    "tokens": [
      {"id": "token1", "token": "ghp_REDACTED_EXAMPLE"},
      {"id": "token2", "token": "ghp_REDACTED_EXAMPLE"},
      {"id": "token3", "token": "ghp_REDACTED_EXAMPLE"}
    ],
    "strategy": "round_robin"
  }'
```

### Phase 3: Proxy Rotation (Paid)
- **Capacity**: Near unlimited (budget-dependent)
- **Cost**: $50-100/month
- **Providers**: Bright Data, Smartproxy, Oxylabs

### Phase 4: Multi-Region VPS (Enterprise)
- **Capacity**: 100,000+ req/hour
- **Cost**: $100-500/month
- **Regions**: 3-5 VPS instances (US, EU, Asia)

---

## Troubleshooting

### Common Issues

#### 1. Server Won't Start
```bash
# Check logs
journalctl -u github-archiver -n 50

# Check port
sudo lsof -i :8081

# Check database connection
psql -U github_archiver -d github_archiver -c "SELECT 1"
```

#### 2. High Memory Usage
```bash
# Check metrics
curl http://localhost:8081/api/health

# Restart service
sudo systemctl restart github-archiver
```

#### 3. Rate Limit Errors
- Verify GitHub token is valid
- Check token rate limit: `curl -H "Authorization: token $TOKEN" https://api.github.com/rate_limit`
- Add more tokens to pool

#### 4. Database Connection Issues
```bash
# Check PostgreSQL status
sudo systemctl status postgresql

# Check connections
sudo -u postgres psql -c "SELECT count(*) FROM pg_stat_activity;"

# Increase max connections in postgresql.conf
max_connections = 100
```

#### 5. Webhook Delivery Failures
- Check webhook URL is accessible
- Verify HMAC signature if using secret
- Check webhook stats: `curl http://localhost:8081/api/webhooks/stats`

---

## Production Checklist

- [ ] PostgreSQL configured with proper user/password
- [ ] Database backups scheduled (daily)
- [ ] Systemd service enabled
- [ ] Nginx reverse proxy with SSL
- [ ] Firewall configured (allow only 80, 443)
- [ ] Log rotation configured
- [ ] Monitoring (Prometheus + Grafana) set up
- [ ] GitHub tokens configured (5-10 for scale)
- [ ] Webhooks configured for alerts
- [ ] Resource limits set (memory, CPU, disk)
- [ ] Disaster recovery plan documented
- [ ] Health check endpoints tested
- [ ] Load testing completed
- [ ] Security audit passed

---

## Support & Resources

- **Documentation**: `/docs`
- **API Reference**: `/api/docs`
- **GitHub Issues**: https://github.com/Bit-Loop/GitArchiver/issues
- **PRD**: `PRD.md`
- **Architecture**: `ARCHITECTURE.md`

For support, open an issue on GitHub or contact the maintainers.
