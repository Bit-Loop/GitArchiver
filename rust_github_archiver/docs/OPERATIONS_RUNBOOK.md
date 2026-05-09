# GitHub Archiver Operations Runbook

## Table of Contents
- [Quick Reference](#quick-reference)
- [System Architecture](#system-architecture)
- [Deployment](#deployment)
- [Monitoring](#monitoring)
- [Common Operations](#common-operations)
- [Troubleshooting](#troubleshooting)
- [Incident Response](#incident-response)
- [Disaster Recovery](#disaster-recovery)

## Quick Reference

### Service URLs
- **Production**: https://github-archiver.example.com
- **Staging**: https://staging.github-archiver.example.com
- **Monitoring**: https://monitoring.github-archiver.example.com
- **Grafana**: https://grafana.github-archiver.example.com
- **Prometheus**: https://prometheus.github-archiver.example.com

### Emergency Contacts
- **On-Call Engineer**: [PagerDuty/Slack]
- **Database Admin**: [Contact]
- **Security Team**: [Contact]

### Operator Roles
- `admin`: use for database lifecycle, user administration, API key control, token/webhook mutation, and audit-log access
- `operator`: use for scraper lifecycle, scan launch/scheduling, realtime monitor control, and operational queue/runtime workflows
- `read_only`: use for dashboard review, findings/log inspection, export access, and other non-mutating workflows

Legacy `user` and `viewer` assignments are accepted only as compatibility aliases and normalize to `operator` and `read_only`.

### Health Check Endpoints
```bash
# Liveness probe
curl https://github-archiver.example.com/health/live

# Readiness probe
curl https://github-archiver.example.com/health/ready

# Detailed health
curl https://github-archiver.example.com/health
```

## System Architecture

### Components
1. **Application**: Rust-based API server (3-10 replicas)
2. **Database**: PostgreSQL 15 (StatefulSet with persistent storage)
3. **Monitoring**: Prometheus + Grafana
4. **Load Balancer**: Kubernetes Ingress (nginx)

### Resource Requirements
- **Application Pod**: 512Mi-2Gi RAM, 500m-2000m CPU
- **Database Pod**: 256Mi-1Gi RAM, 250m-1000m CPU
- **Storage**: 10Gi persistent volume for database

### Network Architecture
```
Internet → Load Balancer → Ingress → Service → Pods
                                    ↓
                              PostgreSQL StatefulSet
```

## Deployment

### Pre-Deployment Checklist
- [ ] All tests passing in CI/CD
- [ ] Security scan passed
- [ ] Database migrations tested
- [ ] Rollback plan documented
- [ ] Backup completed
- [ ] Team notified

### Deployment Process

#### Staging Deployment (Automatic)
```bash
# Push to develop branch
git push origin develop

# Monitor deployment
kubectl rollout status deployment/github-archiver -n github-archiver-staging

# Check logs
kubectl logs -f deployment/github-archiver -n github-archiver-staging

# Run smoke tests
./scripts/smoke-tests.sh staging
```

#### Production Deployment (Manual Approval)
```bash
# Create release tag
git tag -a v1.2.3 -m "Release v1.2.3"
git push origin v1.2.3

# Wait for GitHub Actions workflow approval
# Monitor deployment
kubectl rollout status deployment/github-archiver -n github-archiver-production

# Verify health
curl https://github-archiver.example.com/health

# Monitor for 5 minutes
watch -n 10 'kubectl top pods -n github-archiver-production'
```

### Rollback Procedure
```bash
# Immediate rollback
kubectl rollout undo deployment/github-archiver -n github-archiver-production

# Check rollback status
kubectl rollout status deployment/github-archiver -n github-archiver-production

# Verify health after rollback
curl https://github-archiver.example.com/health
```

## Monitoring

### Key Metrics

#### Application Metrics
- **Request Rate**: `rate(http_requests_total[5m])`
- **Response Time (p95)**: `histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))`
- **Error Rate**: `rate(http_requests_total{status=~"5.."}[5m])`
- **Active Connections**: `http_connections_active`

#### Database Metrics
- **Connection Count**: `pg_stat_database_numbackends`
- **Query Rate**: `rate(pg_stat_database_xact_commit[5m])`
- **Cache Hit Ratio**: `pg_stat_database_blks_hit / (pg_stat_database_blks_hit + pg_stat_database_blks_read)`
- **Slow Queries**: `pg_stat_activity_max_tx_duration`

#### System Metrics
- **CPU Usage**: `rate(process_cpu_seconds_total[5m])`
- **Memory Usage**: `process_resident_memory_bytes`
- **Disk Usage**: `(node_filesystem_size_bytes - node_filesystem_avail_bytes) / node_filesystem_size_bytes`

### Alerts

#### Critical Alerts (Page Immediately)
- Service Down
- Database Down
- High Error Rate (>5%)
- Disk Space Critical (<5%)

#### Warning Alerts (Investigate)
- High Response Time (>1s p95)
- High Memory Usage (>90%)
- High CPU Usage (>90%)
- Disk Space Low (<15%)

### Dashboards
- **Main Dashboard**: Application metrics, request rates, response times
- **Database Dashboard**: Query performance, connection pools, cache hits
- **Infrastructure Dashboard**: CPU, memory, disk, network

## Common Operations

### Scaling

#### Manual Scaling
```bash
# Scale up
kubectl scale deployment/github-archiver --replicas=10 -n github-archiver-production

# Scale down
kubectl scale deployment/github-archiver --replicas=3 -n github-archiver-production

# Check scaling status
kubectl get pods -n github-archiver-production
```

#### Auto-Scaling Configuration
HPA targets:
- **CPU**: 70%
- **Memory**: 80%
- **Min Replicas**: 3
- **Max Replicas**: 10

### Database Operations

#### Create Backup
```bash
# Run backup script
./scripts/backup.sh

# Verify backup
ls -lh /var/backups/github-archiver/

# Check S3 upload
aws s3 ls s3://github-archiver-backups/$(date +%Y%m%d)/
```

#### Restore from Backup
```bash
# List available backups
ls -lh /var/backups/github-archiver/

# Restore (with confirmation)
./scripts/restore.sh -f /var/backups/github-archiver/github_archiver_20240101_120000.sql.gz

# Restore from S3 (latest)
./scripts/restore.sh -s

# Restore from specific date
./scripts/restore.sh -s -d 20240101
```

#### Database Maintenance
```bash
# Connect to database
kubectl exec -it statefulset/postgres -n github-archiver-production -- psql -U postgres github_archiver

# Vacuum and analyze
VACUUM ANALYZE;

# Check table sizes
SELECT
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;

# Check index usage
SELECT
    schemaname,
    tablename,
    indexname,
    idx_scan,
    pg_size_pretty(pg_relation_size(indexrelid)) AS size
FROM pg_stat_user_indexes
WHERE idx_scan = 0 AND schemaname = 'public'
ORDER BY pg_relation_size(indexrelid) DESC;
```

### Log Management

#### View Logs
```bash
# Application logs
kubectl logs -f deployment/github-archiver -n github-archiver-production

# All replicas
kubectl logs -f -l app=github-archiver -n github-archiver-production

# Previous container (if crashed)
kubectl logs --previous deployment/github-archiver -n github-archiver-production

# Filter by level
kubectl logs deployment/github-archiver -n github-archiver-production | grep ERROR
```

#### Download Logs
```bash
# Last 1000 lines
kubectl logs --tail=1000 deployment/github-archiver -n github-archiver-production > app.log

# Time range (requires logging plugin)
kubectl logs --since=1h deployment/github-archiver -n github-archiver-production > app-1h.log
```

## Troubleshooting

### Service Not Responding

#### Symptoms
- HTTP 504 Gateway Timeout
- Connection refused errors
- Health checks failing

#### Investigation
```bash
# Check pod status
kubectl get pods -n github-archiver-production

# Check events
kubectl get events -n github-archiver-production --sort-by='.lastTimestamp'

# Check logs
kubectl logs -f deployment/github-archiver -n github-archiver-production

# Check resource usage
kubectl top pods -n github-archiver-production
```

#### Resolution
```bash
# Restart deployment (rolling restart)
kubectl rollout restart deployment/github-archiver -n github-archiver-production

# If pods are stuck, force delete
kubectl delete pod <pod-name> --grace-period=0 --force -n github-archiver-production
```

### High Error Rate

#### Symptoms
- Alert: HighErrorRate
- 5xx errors in logs
- Increased response times

#### Investigation
```bash
# Check error logs
kubectl logs deployment/github-archiver -n github-archiver-production | grep ERROR

# Check database connectivity
kubectl exec -it deployment/github-archiver -n github-archiver-production -- \
    curl http://localhost:8081/health/ready

# Check Prometheus for patterns
# Query: rate(http_requests_total{status=~"5.."}[5m])
```

#### Common Causes
1. **Database connection pool exhausted**: Increase pool size
2. **Slow queries**: Check `pg_stat_activity`
3. **Memory pressure**: Check memory usage, increase limits
4. **External API failures**: Check circuit breaker state

### Database Connection Issues

#### Symptoms
- "Connection refused" errors
- "Too many connections" errors
- Slow query performance

#### Investigation
```bash
# Check database pod
kubectl get pods -l app=postgres -n github-archiver-production

# Check connections
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres -c "SELECT count(*) FROM pg_stat_activity;"

# Check for blocking queries
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres -c "
        SELECT pid, usename, query, state, wait_event_type
        FROM pg_stat_activity
        WHERE state != 'idle'
        ORDER BY query_start;"
```

#### Resolution
```bash
# Terminate long-running queries
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres -c "SELECT pg_terminate_backend(<pid>);"

# Restart database (last resort)
kubectl delete pod postgres-0 -n github-archiver-production
```

### High Memory Usage

#### Symptoms
- Alert: HighMemoryUsage
- OOMKilled pods
- Slow performance

#### Investigation
```bash
# Check memory usage
kubectl top pods -n github-archiver-production

# Check pod resource limits
kubectl describe pod <pod-name> -n github-archiver-production

# Check for memory leaks in logs
kubectl logs deployment/github-archiver -n github-archiver-production | grep -i memory
```

#### Resolution
```bash
# Increase memory limits
kubectl set resources deployment/github-archiver \
    --limits=memory=4Gi \
    --requests=memory=1Gi \
    -n github-archiver-production

# Restart to apply changes
kubectl rollout restart deployment/github-archiver -n github-archiver-production
```

## Incident Response

### Severity Levels

#### SEV-1 (Critical)
- **Response Time**: Immediate (page on-call)
- **Examples**: Service down, data loss, security breach
- **Actions**: All hands on deck, frequent updates

#### SEV-2 (High)
- **Response Time**: 15 minutes
- **Examples**: Degraded performance, partial outage
- **Actions**: Assign engineer, regular updates

#### SEV-3 (Medium)
- **Response Time**: 1 hour
- **Examples**: Non-critical bugs, minor issues
- **Actions**: Fix in next sprint

### Incident Workflow

1. **Detection**: Alert fires or user report
2. **Acknowledgment**: On-call acknowledges within 5 minutes
3. **Investigation**: Gather logs, metrics, context
4. **Mitigation**: Implement temporary fix (rollback, restart, etc.)
5. **Communication**: Update stakeholders every 15-30 minutes
6. **Resolution**: Deploy permanent fix
7. **Post-Mortem**: Document incident, root cause, action items

### Communication Template
```
INCIDENT: [Brief description]
STATUS: Investigating / Mitigating / Resolved
IMPACT: [Who/what is affected]
TIMELINE:
  - HH:MM: Incident detected
  - HH:MM: Investigation started
  - HH:MM: Root cause identified
  - HH:MM: Mitigation deployed
NEXT UPDATE: [Time]
```

## Disaster Recovery

### Recovery Objectives
- **RTO (Recovery Time Objective)**: 1 hour
- **RPO (Recovery Point Objective)**: 24 hours (daily backups)

### Disaster Scenarios

#### Complete Database Loss

1. **Stop application traffic**
   ```bash
   kubectl scale deployment/github-archiver --replicas=0 -n github-archiver-production
   ```

2. **Restore from latest backup**
   ```bash
   ./scripts/restore.sh -s
   ```

3. **Verify restore**
   ```bash
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       psql -U postgres github_archiver -c "SELECT COUNT(*) FROM events;"
   ```

4. **Resume traffic**
   ```bash
   kubectl scale deployment/github-archiver --replicas=3 -n github-archiver-production
   ```

#### Complete Cluster Loss

1. **Provision new cluster**
2. **Apply Kubernetes manifests**
   ```bash
   kubectl apply -f k8s-deployment.yaml
   ```

3. **Restore database**
   ```bash
   ./scripts/restore.sh -s
   ```

4. **Update DNS to point to new cluster**
5. **Verify all services**

#### Data Corruption

1. **Identify corruption timestamp**
2. **Find backup before corruption**
   ```bash
   ls -lh /var/backups/github-archiver/
   ```

3. **Restore to staging and verify**
   ```bash
   ./scripts/restore.sh -f <backup-file> -e staging
   ```

4. **If verified, restore to production**

### Backup Schedule
- **Frequency**: Daily at 02:00 UTC
- **Retention**: 30 days local, 90 days S3
- **Location**: 
  - Local: `/var/backups/github-archiver/`
  - S3: `s3://github-archiver-backups/`

### Testing DR Plan
- **Frequency**: Quarterly
- **Procedure**:
  1. Restore backup to staging
  2. Run full test suite
  3. Verify data integrity
  4. Document any issues
  5. Update runbook

---

## Appendix

### Environment Variables
```bash
# Required
DATABASE_URL=postgresql://user:pass@host:5432/dbname
GITHUB_TOKEN=ghp_REDACTED_EXAMPLE
JWT_SECRET=xxxxxxxxxxxx

# Optional
RUST_LOG=info
SERVER_PORT=8081
MAX_CONNECTIONS=100
RATE_LIMIT_REQUESTS=1000
RATE_LIMIT_WINDOW=60
```

### Useful Commands
```bash
# Get cluster info
kubectl cluster-info

# Get node status
kubectl get nodes

# Get all resources in namespace
kubectl get all -n github-archiver-production

# Port forward to service
kubectl port-forward svc/github-archiver 8081:80 -n github-archiver-production

# Execute command in pod
kubectl exec -it <pod-name> -n github-archiver-production -- /bin/bash

# Copy files from pod
kubectl cp <pod-name>:/path/to/file ./local-file -n github-archiver-production
```

### Monitoring Queries
```promql
# Request rate
rate(http_requests_total[5m])

# Error rate percentage
rate(http_requests_total{status=~"5.."}[5m]) / rate(http_requests_total[5m]) * 100

# p95 response time
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))

# Database connections
pg_stat_database_numbackends

# Memory usage percentage
(process_resident_memory_bytes / node_memory_MemTotal_bytes) * 100
```
