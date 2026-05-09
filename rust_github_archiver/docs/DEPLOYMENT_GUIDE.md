# Deployment Guide

This guide provides step-by-step instructions for deploying GitHub Archiver in different environments.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Local Development Deployment](#local-development-deployment)
3. [Docker Deployment](#docker-deployment)
4. [Kubernetes Production Deployment](#kubernetes-production-deployment)
5. [Configuration](#configuration)
6. [Monitoring Setup](#monitoring-setup)
7. [Backup Configuration](#backup-configuration)
8. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Tools

- **Rust**: 1.75 or higher
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup update
  ```

- **PostgreSQL**: 15 or higher
  ```bash
  # Ubuntu/Debian
  sudo apt-get install postgresql-15 postgresql-client-15
  
  # macOS
  brew install postgresql@15
  ```

- **Docker**: Latest stable
  ```bash
  # Ubuntu/Debian
  curl -fsSL https://get.docker.com | sh
  
  # macOS
  brew install --cask docker
  ```

- **kubectl**: Kubernetes CLI (for production deployment)
  ```bash
  # Ubuntu/Debian
  curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
  sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl
  
  # macOS
  brew install kubectl
  ```

### Required Accounts/Services

- GitHub account with Personal Access Token
- Kubernetes cluster (for production)
- S3-compatible storage (for backups)
- Slack workspace (for alerts, optional)

---

## Local Development Deployment

### Step 1: Clone Repository

```bash
git clone https://github.com/yourusername/GitArchiver.git
cd GitArchiver/rust_github_archiver
```

### Step 2: Set Up Database

```bash
# Start PostgreSQL service
sudo systemctl start postgresql

# Create database and user
sudo -u postgres psql <<EOF
CREATE DATABASE github_archiver;
CREATE USER archiver WITH PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE github_archiver TO archiver;
\c github_archiver
GRANT ALL ON SCHEMA public TO archiver;
EOF
```

### Step 3: Configure Environment

Create `.env` file:

```bash
cat > .env <<EOF
# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8081
RUST_LOG=info

# Database Configuration
DATABASE_URL=postgresql://archiver:secure_password@localhost:5432/github_archiver
DATABASE_MAX_CONNECTIONS=10

# GitHub API
GITHUB_TOKEN=ghp_REDACTED_EXAMPLE

# JWT Secret (generate with: openssl rand -hex 32)
JWT_SECRET=<output-of-openssl-rand-hex-32>

# Admin User (for initial setup)
ADMIN_USERNAME=admin
ADMIN_PASSWORD=<strong-random-admin-password>

# Performance
WORKER_THREADS=4
EOF
```

### Step 4: Initialize Database Schema

```bash
# Run database migrations
cargo run --bin web_server -- --migrate

# Or manually with psql
psql -U archiver -d github_archiver -f schema.sql
```

### Step 5: Build and Run

```bash
# Build in development mode
cargo build

# Run web server
cargo run --bin web_server

# Or build optimized release
cargo build --release
./target/release/web_server
```

### Step 6: Verify Deployment

```bash
# Check health
curl http://localhost:8081/health

# Check metrics
curl http://localhost:8081/metrics

# Access dashboard
open http://localhost:8081/dashboard
```

### Step 7: Create Admin User

```bash
# Use CLI to create initial admin user
cargo run -- create-admin \
  --username admin \
  --password "SecurePassword123!" \
  --email admin@example.com
```

---

## Docker Deployment

### Step 1: Build Docker Image

```bash
# Build multi-stage Docker image
docker build -t github-archiver:latest .

# Or use docker-compose
docker-compose build
```

### Step 2: Configure docker-compose.yml

```yaml
version: '3.8'

services:
  app:
    image: github-archiver:latest
    ports:
      - "8081:8081"
    environment:
      - DATABASE_URL=postgresql://archiver:password@postgres:5432/github_archiver
      - GITHUB_TOKEN=${GITHUB_TOKEN}
      - JWT_SECRET=${JWT_SECRET}
      - RUST_LOG=info
    depends_on:
      - postgres
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8081/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  postgres:
    image: postgres:15-alpine
    environment:
      - POSTGRES_DB=github_archiver
      - POSTGRES_USER=archiver
      - POSTGRES_PASSWORD=password
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U archiver"]
      interval: 10s
      timeout: 5s
      retries: 5

volumes:
  postgres_data:
```

### Step 3: Start Services

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Check status
docker-compose ps
```

### Step 4: Initialize Database

```bash
# Run database initialization
docker-compose exec app /app/scripts/init_db.sh

# Or manually
docker-compose exec postgres psql -U archiver -d github_archiver -f /init.sql
```

### Step 5: Verify Deployment

```bash
# Health check
curl http://localhost:8081/health

# Check container logs
docker-compose logs app

# Access dashboard
open http://localhost:8081/dashboard
```

---

## Kubernetes Production Deployment

### Step 1: Prepare Cluster

```bash
# Verify kubectl access
kubectl cluster-info
kubectl get nodes

# Create namespace
kubectl create namespace github-archiver
kubectl config set-context --current --namespace=github-archiver
```

### Step 2: Configure Secrets

```bash
# Create database secret
kubectl create secret generic github-archiver-secrets \
  --from-literal=POSTGRES_PASSWORD=your_secure_password \
  --from-literal=JWT_SECRET=$(openssl rand -hex 32) \
  --from-literal=GITHUB_TOKEN=ghp_REDACTED_EXAMPLE

# Create S3 backup credentials (if using S3)
kubectl create secret generic s3-credentials \
  --from-literal=AWS_ACCESS_KEY_ID=your_key \
  --from-literal=AWS_SECRET_ACCESS_KEY=your_secret
```

### Step 3: Create ConfigMap

```bash
# Create configuration
kubectl create configmap github-archiver-config \
  --from-literal=SERVER_PORT=8081 \
  --from-literal=RUST_LOG=info \
  --from-literal=DATABASE_MAX_CONNECTIONS=50 \
  --from-literal=WORKER_THREADS=8
```

### Step 4: Deploy Database

```bash
# Apply PostgreSQL StatefulSet
kubectl apply -f k8s/postgres-statefulset.yaml

# Wait for database to be ready
kubectl wait --for=condition=ready pod -l app=postgres --timeout=120s

# Verify database
kubectl exec -it postgres-0 -- psql -U archiver -c "SELECT version();"
```

### Step 5: Initialize Database Schema

```bash
# Copy schema file to pod
kubectl cp schema.sql postgres-0:/tmp/schema.sql

# Run schema initialization
kubectl exec -it postgres-0 -- psql -U archiver -d github_archiver -f /tmp/schema.sql
```

### Step 6: Deploy Application

```bash
# Apply deployment manifest
kubectl apply -f k8s-deployment.yaml

# Wait for rollout
kubectl rollout status deployment/github-archiver

# Verify pods are running
kubectl get pods -l app=github-archiver
```

### Step 7: Deploy Monitoring Stack

```bash
# Install Prometheus
kubectl apply -f monitoring/prometheus-deployment.yaml

# Install Grafana
kubectl apply -f monitoring/grafana-deployment.yaml

# Configure Prometheus scraping
kubectl apply -f monitoring/prometheus-config.yaml

# Import Grafana dashboards
kubectl apply -f monitoring/grafana-dashboards.yaml
```

### Step 8: Configure Ingress

```bash
# Install ingress controller (if not present)
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/cloud/deploy.yaml

# Apply ingress for application
kubectl apply -f k8s/ingress.yaml

# Get ingress IP
kubectl get ingress github-archiver-ingress
```

### Step 9: Set Up TLS

```bash
# Install cert-manager (for automatic TLS)
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml

# Create certificate issuer
kubectl apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-prod
spec:
  acme:
    server: https://acme-v02.api.letsencrypt.org/directory
    email: admin@yourdomain.com
    privateKeySecretRef:
      name: letsencrypt-prod
    solvers:
    - http01:
        ingress:
          class: nginx
EOF

# Update ingress to use TLS
kubectl patch ingress github-archiver-ingress -p '{"metadata":{"annotations":{"cert-manager.io/cluster-issuer":"letsencrypt-prod"}}}'
```

### Step 10: Configure Auto-Scaling

```bash
# HorizontalPodAutoscaler is already in k8s-deployment.yaml
# Verify it's working
kubectl get hpa

# Manual scaling if needed
kubectl scale deployment github-archiver --replicas=5
```

### Step 11: Verify Production Deployment

```bash
# Check all resources
kubectl get all

# Test health endpoints
INGRESS_IP=$(kubectl get ingress github-archiver-ingress -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
curl http://$INGRESS_IP/health
curl http://$INGRESS_IP/health/live
curl http://$INGRESS_IP/health/ready

# Check metrics
curl http://$INGRESS_IP/metrics

# View logs
kubectl logs -f deployment/github-archiver --all-containers=true

# Check Prometheus targets
kubectl port-forward -n monitoring svc/prometheus 9090:9090
# Open http://localhost:9090/targets

# Access Grafana
kubectl port-forward -n monitoring svc/grafana 3000:3000
# Open http://localhost:3000 (admin/admin)
```

---

## Configuration

### Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `SERVER_HOST` | Server bind address | `0.0.0.0` | No |
| `SERVER_PORT` | Server port | `8081` | No |
| `DATABASE_URL` | PostgreSQL connection string | - | Yes |
| `DATABASE_MAX_CONNECTIONS` | Max DB connections | `50` | No |
| `GITHUB_TOKEN` | GitHub Personal Access Token | - | Yes |
| `JWT_SECRET` | JWT signing secret | - | Yes |
| `RUST_LOG` | Log level | `info` | No |
| `WORKER_THREADS` | Tokio worker threads | `4` | No |
| `ADMIN_USERNAME` | Initial admin username | - | No |
| `ADMIN_PASSWORD` | Initial admin password | - | Yes |

### Configuration File (Optional)

Create `config/production.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8081

[database]
url = "postgresql://archiver:password@postgres:5432/github_archiver"
max_connections = 50
min_connections = 10
connect_timeout = 30
idle_timeout = 600

[github]
token = "ghp_REDACTED_EXAMPLE"
api_base_url = "https://api.github.com"
rate_limit_per_hour = 5000

[security]
jwt_secret = "from_env"
jwt_expiration_hours = 1
rate_limit_requests_per_minute = 1000
max_request_body_size = 10485760  # 10MB

[monitoring]
prometheus_enabled = true
prometheus_path = "/metrics"
health_check_path = "/health"

[logging]
level = "info"
format = "json"  # or "pretty"
```

---

## Monitoring Setup

### Prometheus Configuration

1. **Verify Prometheus is scraping**:
   ```bash
   kubectl port-forward -n monitoring svc/prometheus 9090:9090
   # Visit http://localhost:9090/targets
   ```

2. **Check metrics are available**:
   ```bash
   curl http://your-app-url/metrics
   ```

3. **Verify alert rules**:
   ```bash
   kubectl get prometheusrules -n monitoring
   ```

### Grafana Setup

1. **Access Grafana**:
   ```bash
   kubectl port-forward -n monitoring svc/grafana 3000:3000
   # Visit http://localhost:3000
   # Default: admin/admin
   ```

2. **Add Prometheus data source**:
   - Configuration → Data Sources → Add data source
   - Select Prometheus
   - URL: `http://prometheus:9090`
   - Save & Test

3. **Import dashboards**:
   - Dashboards → Import
   - Upload `grafana-dashboards/main-dashboard.json`
   - Upload `grafana-dashboards/database-dashboard.json`

### Alert Configuration

1. **Configure Alertmanager**:
   ```bash
   kubectl edit configmap alertmanager-config -n monitoring
   ```

2. **Add Slack webhook**:
   ```yaml
   receivers:
   - name: 'slack'
     slack_configs:
     - api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK/URL'
       channel: '#alerts'
       title: 'GitHub Archiver Alert'
   ```

3. **Test alerts**:
   ```bash
   # Trigger high CPU alert by scaling to 0 replicas briefly
   kubectl scale deployment github-archiver --replicas=0
   # Wait for alert
   # Scale back
   kubectl scale deployment github-archiver --replicas=3
   ```

---

## Backup Configuration

### Automated Database Backups

1. **Create backup CronJob**:
   ```bash
   kubectl apply -f - <<EOF
   apiVersion: batch/v1
   kind: CronJob
   metadata:
     name: postgres-backup
   spec:
     schedule: "0 2 * * *"  # Daily at 2 AM
     jobTemplate:
       spec:
         template:
           spec:
             containers:
             - name: backup
               image: postgres:15-alpine
               env:
               - name: PGPASSWORD
                 valueFrom:
                   secretKeyRef:
                     name: github-archiver-secrets
                     key: POSTGRES_PASSWORD
               command:
               - /bin/sh
               - -c
               - |
                 BACKUP_FILE="/backups/backup-\$(date +%Y%m%d-%H%M%S).sql.gz"
                 pg_dump -h postgres -U archiver github_archiver | gzip > \$BACKUP_FILE
                 echo "Backup completed: \$BACKUP_FILE"
               volumeMounts:
               - name: backup-volume
                 mountPath: /backups
             restartPolicy: OnFailure
             volumes:
             - name: backup-volume
               persistentVolumeClaim:
                 claimName: backup-pvc
   EOF
   ```

2. **Configure S3 backup sync**:
   ```bash
   # Edit scripts/backup.sh with your S3 credentials
   kubectl create configmap backup-script --from-file=scripts/backup.sh
   ```

3. **Verify backups**:
   ```bash
   # List recent backups
   kubectl exec -it postgres-0 -- ls -lh /backups/
   
   # Test restore
   kubectl exec -it postgres-0 -- bash
   gunzip -c /backups/backup-latest.sql.gz | psql -U archiver github_archiver
   ```

### Manual Backup

```bash
# Create manual backup
kubectl exec -it postgres-0 -- pg_dump -U archiver github_archiver > backup.sql

# Compress backup
gzip backup.sql

# Upload to S3 (if configured)
aws s3 cp backup.sql.gz s3://your-bucket/backups/
```

---

## Troubleshooting

### Common Issues

#### 1. Application Not Starting

**Symptoms**: Pods in CrashLoopBackOff state

**Diagnosis**:
```bash
kubectl logs -f deployment/github-archiver
kubectl describe pod <pod-name>
```

**Solutions**:
- Check database connectivity: `kubectl exec -it <pod> -- curl postgres:5432`
- Verify secrets: `kubectl get secret github-archiver-secrets -o yaml`
- Check resource limits: `kubectl describe node`

#### 2. Database Connection Failures

**Symptoms**: "Connection refused" or "Too many connections"

**Diagnosis**:
```bash
kubectl logs postgres-0
kubectl exec -it postgres-0 -- psql -U archiver -c "SELECT count(*) FROM pg_stat_activity;"
```

**Solutions**:
```bash
# Increase max connections
kubectl exec -it postgres-0 -- psql -U archiver
ALTER SYSTEM SET max_connections = 200;
SELECT pg_reload_conf();

# Scale down application if too many connections
kubectl scale deployment github-archiver --replicas=2
```

#### 3. High Memory Usage

**Symptoms**: Pods being OOMKilled

**Diagnosis**:
```bash
kubectl top pods
kubectl describe pod <pod-name> | grep -A 5 "State:"
```

**Solutions**:
```bash
# Increase memory limits
kubectl patch deployment github-archiver -p '{"spec":{"template":{"spec":{"containers":[{"name":"github-archiver","resources":{"limits":{"memory":"4Gi"}}}]}}}}'

# Reduce connection pool size
kubectl set env deployment/github-archiver DATABASE_MAX_CONNECTIONS=30
```

#### 4. Rate Limiting Issues

**Symptoms**: 429 Too Many Requests errors

**Diagnosis**:
```bash
# Check rate limit metrics
curl http://your-app-url/metrics | grep rate_limit

# Check logs
kubectl logs -f deployment/github-archiver | grep "rate limit"
```

**Solutions**:
```bash
# Increase rate limits (if appropriate)
kubectl set env deployment/github-archiver RATE_LIMIT_PER_MINUTE=2000

# Add more replicas to distribute load
kubectl scale deployment github-archiver --replicas=5
```

#### 5. Health Check Failures

**Symptoms**: Pods constantly restarting, marked unhealthy

**Diagnosis**:
```bash
kubectl describe pod <pod-name>
curl http://pod-ip:8081/health/ready
```

**Solutions**:
```bash
# Check database is accessible
kubectl exec -it <pod> -- curl postgres:5432

# Increase health check thresholds
kubectl patch deployment github-archiver -p '{"spec":{"template":{"spec":{"containers":[{"name":"github-archiver","readinessProbe":{"failureThreshold":5,"initialDelaySeconds":30}}]}}}}'
```

### Performance Optimization

1. **Database Query Optimization**:
   ```sql
   -- Check slow queries
   SELECT query, mean_exec_time, calls 
   FROM pg_stat_statements 
   ORDER BY mean_exec_time DESC 
   LIMIT 10;
   
   -- Add missing indexes
   CREATE INDEX CONCURRENTLY idx_events_timestamp ON events(timestamp);
   CREATE INDEX CONCURRENTLY idx_secrets_detected_at ON secrets(detected_at);
   ```

2. **Connection Pool Tuning**:
   ```bash
   # Adjust based on: (replicas * max_connections) < postgres max_connections
   kubectl set env deployment/github-archiver DATABASE_MAX_CONNECTIONS=40
   ```

3. **Resource Allocation**:
   ```bash
   # Update CPU/memory limits
   kubectl edit deployment github-archiver
   # Increase resources.limits and resources.requests
   ```

### Log Analysis

```bash
# Search for errors
kubectl logs deployment/github-archiver | grep ERROR

# Follow logs in real-time
kubectl logs -f deployment/github-archiver --all-containers=true

# Get logs from specific time range
kubectl logs deployment/github-archiver --since=1h

# Export logs for analysis
kubectl logs deployment/github-archiver --all-containers=true > app.log
```

### Metrics Analysis

```bash
# Check Prometheus metrics
curl http://your-app-url/metrics | grep -E "(http_requests|db_connections|circuit_breaker)"

# Query Prometheus
curl 'http://prometheus:9090/api/v1/query?query=rate(http_requests_total[5m])'
```

---

## Post-Deployment Checklist

- [ ] All pods are running and healthy
- [ ] Database is accessible and initialized
- [ ] Health endpoints respond correctly
- [ ] Metrics are being collected
- [ ] Alerts are configured and tested
- [ ] Grafana dashboards are loaded
- [ ] Backups are running successfully
- [ ] TLS certificates are valid
- [ ] Admin user is created
- [ ] Application is accessible via ingress
- [ ] Rate limiting is working
- [ ] Security headers are present
- [ ] Documentation is updated with production URLs

---

## Support

For issues and questions:
- GitHub Issues: https://github.com/yourusername/GitArchiver/issues
- Documentation: https://github.com/yourusername/GitArchiver/docs
- Slack: #github-archiver (internal)

---

**Last Updated**: 2025-01-15  
**Version**: 1.0.0  
**Maintained By**: GitHub Archiver Team
