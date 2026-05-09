# GitHub Archiver Troubleshooting Guide

## Table of Contents
- [Quick Diagnostics](#quick-diagnostics)
- [Application Issues](#application-issues)
- [Database Issues](#database-issues)
- [Performance Issues](#performance-issues)
- [Network Issues](#network-issues)
- [Authentication Issues](#authentication-issues)
- [Data Quality Issues](#data-quality-issues)

## Quick Diagnostics

### Health Check Commands
```bash
# Quick health check
curl -f https://github-archiver.example.com/health || echo "FAILED"

# Detailed health check
curl https://github-archiver.example.com/health | jq

# Check all pods
kubectl get pods -n github-archiver-production

# Check recent events
kubectl get events -n github-archiver-production --sort-by='.lastTimestamp' | tail -20
```

### Log Analysis
```bash
# Recent errors
kubectl logs --tail=100 deployment/github-archiver -n github-archiver-production | grep -i error

# Count errors by type
kubectl logs --since=1h deployment/github-archiver -n github-archiver-production | \
    grep ERROR | awk '{print $5}' | sort | uniq -c | sort -rn

# Find slow requests
kubectl logs --since=1h deployment/github-archiver -n github-archiver-production | \
    grep "duration_ms" | awk '$NF > 1000' | tail -20
```

## Application Issues

### Issue: Pods Crashing (CrashLoopBackOff)

**Symptoms:**
- Pods in `CrashLoopBackOff` state
- Service unavailable
- Restart count increasing

**Diagnosis:**
```bash
# Check pod status
kubectl get pods -n github-archiver-production

# View recent logs
kubectl logs --tail=100 <pod-name> -n github-archiver-production

# View previous container logs
kubectl logs --previous <pod-name> -n github-archiver-production

# Describe pod for events
kubectl describe pod <pod-name> -n github-archiver-production
```

**Common Causes & Solutions:**

1. **Database connection failure**
   ```bash
   # Check database connectivity
   kubectl exec -it deployment/github-archiver -n github-archiver-production -- \
       curl http://localhost:8081/health/ready
   
   # Verify DATABASE_URL secret
   kubectl get secret github-archiver-secrets -n github-archiver-production -o yaml
   ```

2. **Missing environment variables**
   ```bash
   # Check configmap
   kubectl get configmap github-archiver-config -n github-archiver-production -o yaml
   
   # Check secrets
   kubectl get secret github-archiver-secrets -n github-archiver-production -o yaml
   ```

3. **Out of memory (OOMKilled)**
   ```bash
   # Check if pod was OOMKilled
   kubectl describe pod <pod-name> -n github-archiver-production | grep -A 5 "Last State"
   
   # Solution: Increase memory limits
   kubectl set resources deployment/github-archiver \
       --limits=memory=4Gi --requests=memory=1Gi \
       -n github-archiver-production
   ```

4. **Port already in use**
   ```bash
   # Check port configuration
   kubectl get deployment github-archiver -n github-archiver-production -o yaml | grep -A 5 ports
   ```

### Issue: Service Returns 503 Service Unavailable

**Symptoms:**
- HTTP 503 errors
- Readiness probe failing
- Pods not ready

**Diagnosis:**
```bash
# Check readiness probe
kubectl get pods -n github-archiver-production

# Test readiness endpoint
kubectl exec -it deployment/github-archiver -n github-archiver-production -- \
    curl http://localhost:8081/health/ready

# Check logs for database errors
kubectl logs deployment/github-archiver -n github-archiver-production | grep -i "database"
```

**Solutions:**

1. **Database not ready**
   ```bash
   # Check database pod
   kubectl get pods -l app=postgres -n github-archiver-production
   
   # Check database logs
   kubectl logs statefulset/postgres -n github-archiver-production
   
   # Restart database if needed
   kubectl delete pod postgres-0 -n github-archiver-production
   ```

2. **Circuit breaker open**
   ```bash
   # Check circuit breaker metrics
   curl https://github-archiver.example.com/metrics | grep circuit_breaker_state
   
   # Manual reset (if available)
   curl -X POST https://github-archiver.example.com/api/v1/admin/circuit-breaker/reset
   ```

3. **Health check configuration wrong**
   ```bash
   # Check probe configuration
   kubectl get deployment github-archiver -n github-archiver-production -o yaml | \
       grep -A 10 readinessProbe
   ```

### Issue: High CPU Usage

**Symptoms:**
- CPU throttling
- Slow response times
- HPA scaling up

**Diagnosis:**
```bash
# Check CPU usage
kubectl top pods -n github-archiver-production

# Check HPA status
kubectl get hpa -n github-archiver-production

# Profile application (if profiling enabled)
curl https://github-archiver.example.com/debug/pprof/profile?seconds=30 > cpu.prof
```

**Solutions:**

1. **Inefficient queries**
   ```bash
   # Check slow queries
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       psql -U postgres github_archiver -c "
           SELECT pid, query, state, query_start
           FROM pg_stat_activity
           WHERE state != 'idle'
           AND query_start < now() - interval '5 seconds'
           ORDER BY query_start;"
   ```

2. **Increase CPU limits**
   ```bash
   kubectl set resources deployment/github-archiver \
       --limits=cpu=4000m --requests=cpu=1000m \
       -n github-archiver-production
   ```

3. **Scale horizontally**
   ```bash
   kubectl scale deployment/github-archiver --replicas=10 -n github-archiver-production
   ```

## Database Issues

### Issue: Database Connection Pool Exhausted

**Symptoms:**
- "Too many connections" errors
- "Connection pool timeout" errors
- Requests timing out

**Diagnosis:**
```bash
# Check current connections
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres -c "SELECT count(*) FROM pg_stat_activity;"

# Check connections by state
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres -c "
        SELECT state, count(*)
        FROM pg_stat_activity
        GROUP BY state;"

# Check application pool metrics
curl https://github-archiver.example.com/metrics | grep db_connections
```

**Solutions:**

1. **Increase max_connections**
   ```bash
   # Edit PostgreSQL config
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       psql -U postgres -c "ALTER SYSTEM SET max_connections = 200;"
   
   # Restart database
   kubectl delete pod postgres-0 -n github-archiver-production
   ```

2. **Increase application pool size**
   ```bash
   # Update MAX_CONNECTIONS env var
   kubectl set env deployment/github-archiver MAX_CONNECTIONS=150 \
       -n github-archiver-production
   ```

3. **Kill idle connections**
   ```bash
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       psql -U postgres -c "
           SELECT pg_terminate_backend(pid)
           FROM pg_stat_activity
           WHERE state = 'idle'
           AND state_change < now() - interval '30 minutes';"
   ```

### Issue: Slow Database Queries

**Symptoms:**
- High response times
- Database CPU at 100%
- Timeouts

**Diagnosis:**
```bash
# Find slow queries
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres github_archiver -c "
        SELECT pid, now() - query_start AS duration, query
        FROM pg_stat_activity
        WHERE state != 'idle'
        ORDER BY duration DESC;"

# Check table sizes
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres github_archiver -c "
        SELECT schemaname, tablename,
               pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
        FROM pg_tables
        WHERE schemaname = 'public'
        ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC
        LIMIT 10;"

# Check missing indexes
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres github_archiver -c "
        SELECT schemaname, tablename, attname, n_distinct, correlation
        FROM pg_stats
        WHERE schemaname = 'public'
        AND n_distinct > 100
        ORDER BY abs(correlation) ASC
        LIMIT 10;"
```

**Solutions:**

1. **Add missing indexes**
   ```sql
   -- Create index on frequently queried columns
   CREATE INDEX CONCURRENTLY idx_events_created_at ON events(created_at);
   CREATE INDEX CONCURRENTLY idx_events_repo_id ON events(repository_id);
   ```

2. **Vacuum and analyze**
   ```bash
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       psql -U postgres github_archiver -c "VACUUM ANALYZE;"
   ```

3. **Increase shared_buffers**
   ```bash
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       psql -U postgres -c "ALTER SYSTEM SET shared_buffers = '2GB';"
   ```

### Issue: Disk Space Running Out

**Symptoms:**
- Alert: DiskSpaceLow or DiskSpaceCritical
- Write failures
- Database errors

**Diagnosis:**
```bash
# Check disk usage
kubectl exec -it statefulset/postgres -n github-archiver-production -- df -h

# Check database size
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres -c "SELECT pg_size_pretty(pg_database_size('github_archiver'));"

# Check table sizes
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres github_archiver -c "
        SELECT schemaname, tablename,
               pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
        FROM pg_tables
        WHERE schemaname = 'public'
        ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC
        LIMIT 10;"
```

**Solutions:**

1. **Clean old data**
   ```sql
   -- Delete events older than 90 days
   DELETE FROM events WHERE created_at < now() - interval '90 days';
   
   -- Vacuum to reclaim space
   VACUUM FULL;
   ```

2. **Increase persistent volume size**
   ```bash
   # Edit PVC
   kubectl edit pvc postgres-data-postgres-0 -n github-archiver-production
   # Change spec.resources.requests.storage to larger value
   ```

3. **Archive old data**
   ```bash
   # Export old data to S3
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       pg_dump -U postgres github_archiver \
       --table=events \
       --where="created_at < '2024-01-01'" \
       | gzip | aws s3 cp - s3://github-archiver-archives/events-2023.sql.gz
   
   # Delete archived data
   # Then VACUUM FULL
   ```

## Performance Issues

### Issue: High Response Times

**Symptoms:**
- Alert: HighResponseTime
- User complaints about slowness
- p95 latency > 1s

**Diagnosis:**
```bash
# Check response time metrics
curl https://github-archiver.example.com/metrics | \
    grep http_request_duration_seconds

# Check slow endpoints
kubectl logs --since=1h deployment/github-archiver -n github-archiver-production | \
    grep "duration_ms" | awk '{print $4, $NF}' | sort -k2 -rn | head -20

# Check database query times
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres github_archiver -c "
        SELECT query, calls, total_time, mean_time
        FROM pg_stat_statements
        ORDER BY mean_time DESC
        LIMIT 10;"
```

**Solutions:**

1. **Add caching**
   - Implement Redis caching for frequently accessed data
   - Cache expensive computations

2. **Optimize database queries**
   - Add indexes
   - Rewrite inefficient queries
   - Use connection pooling

3. **Scale horizontally**
   ```bash
   kubectl scale deployment/github-archiver --replicas=10 -n github-archiver-production
   ```

### Issue: High Memory Usage

**Symptoms:**
- Alert: HighMemoryUsage
- Pods getting OOMKilled
- Frequent restarts

**Diagnosis:**
```bash
# Check memory usage
kubectl top pods -n github-archiver-production

# Check for memory leaks
kubectl logs deployment/github-archiver -n github-archiver-production | grep -i "memory"

# Check process memory
kubectl exec -it deployment/github-archiver -n github-archiver-production -- \
    ps aux | head -10
```

**Solutions:**

1. **Increase memory limits**
   ```bash
   kubectl set resources deployment/github-archiver \
       --limits=memory=4Gi --requests=memory=2Gi \
       -n github-archiver-production
   ```

2. **Fix memory leaks**
   - Review code for unclosed connections
   - Check for large in-memory caches
   - Profile application

3. **Implement backpressure**
   - Limit concurrent requests
   - Implement queue size limits

## Network Issues

### Issue: Cannot Connect to Service

**Symptoms:**
- Connection refused errors
- DNS resolution failures
- Timeouts

**Diagnosis:**
```bash
# Check service
kubectl get svc github-archiver -n github-archiver-production

# Check endpoints
kubectl get endpoints github-archiver -n github-archiver-production

# Test from another pod
kubectl run test-pod --rm -i --restart=Never --image=curlimages/curl -- \
    curl -v http://github-archiver.github-archiver-production.svc.cluster.local

# Check DNS
kubectl run test-pod --rm -i --restart=Never --image=busybox -- \
    nslookup github-archiver.github-archiver-production.svc.cluster.local
```

**Solutions:**

1. **Service selector mismatch**
   ```bash
   # Check service selector
   kubectl get svc github-archiver -n github-archiver-production -o yaml | grep -A 5 selector
   
   # Check pod labels
   kubectl get pods -l app=github-archiver -n github-archiver-production
   ```

2. **Ingress misconfiguration**
   ```bash
   # Check ingress
   kubectl get ingress -n github-archiver-production
   
   # Describe ingress
   kubectl describe ingress github-archiver -n github-archiver-production
   ```

3. **Network policy blocking**
   ```bash
   # Check network policies
   kubectl get networkpolicies -n github-archiver-production
   ```

## Authentication Issues

### Issue: JWT Token Validation Failed

**Symptoms:**
- 401 Unauthorized errors
- "Invalid token" errors
- Users can't log in

**Diagnosis:**
```bash
# Check JWT secret
kubectl get secret github-archiver-secrets -n github-archiver-production -o yaml

# Test token validation
curl -H "Authorization: Bearer <token>" \
    https://github-archiver.example.com/api/v1/events

# Check logs
kubectl logs deployment/github-archiver -n github-archiver-production | \
    grep -i "auth\|jwt\|token"
```

**Solutions:**

1. **JWT secret mismatch**
   ```bash
   # Verify JWT_SECRET in all pods
   kubectl get secret github-archiver-secrets -n github-archiver-production \
       -o jsonpath='{.data.JWT_SECRET}' | base64 -d
   ```

2. **Token expired**
   - Increase token expiration time
   - Implement token refresh

3. **Clock skew**
   - Check server time synchronization
   - Allow clock skew in validation

## Data Quality Issues

### Issue: Missing Events

**Symptoms:**
- Event count lower than expected
- Gaps in data
- User reports missing data

**Diagnosis:**
```bash
# Check event counts
kubectl exec -it statefulset/postgres -n github-archiver-production -- \
    psql -U postgres github_archiver -c "
        SELECT DATE(created_at), COUNT(*)
        FROM events
        WHERE created_at > now() - interval '7 days'
        GROUP BY DATE(created_at)
        ORDER BY DATE(created_at);"

# Check scraper logs
kubectl logs deployment/github-archiver -n github-archiver-production | \
    grep -i "scraper\|event"

# Check for errors
kubectl logs deployment/github-archiver -n github-archiver-production | \
    grep ERROR | grep -i "event"
```

**Solutions:**

1. **Scraper not running**
   ```bash
   # Check if scraper is enabled
   kubectl exec -it deployment/github-archiver -n github-archiver-production -- \
       env | grep SCRAPER_ENABLED
   ```

2. **Rate limiting**
   ```bash
   # Check GitHub API rate limit
   curl -H "Authorization: Bearer $GITHUB_TOKEN" \
       https://api.github.com/rate_limit
   ```

3. **Data corruption**
   ```bash
   # Check data integrity
   kubectl exec -it statefulset/postgres -n github-archiver-production -- \
       psql -U postgres github_archiver -c "
           SELECT COUNT(*) FROM events WHERE repository_id IS NULL;
           SELECT COUNT(*) FROM events WHERE created_at IS NULL;"
   ```

---

## Getting Help

If you can't resolve the issue:

1. **Gather diagnostics**
   ```bash
   # Save all logs
   kubectl logs deployment/github-archiver -n github-archiver-production > app.log
   kubectl logs statefulset/postgres -n github-archiver-production > db.log
   kubectl get events -n github-archiver-production > events.log
   kubectl describe deployment github-archiver -n github-archiver-production > deployment.yaml
   ```

2. **Create incident ticket** with:
   - Clear problem description
   - Steps to reproduce
   - Expected vs actual behavior
   - Diagnostic logs
   - Error messages

3. **Escalate** if:
   - SEV-1 incident (service down)
   - Data loss risk
   - Security incident
   - No resolution after 1 hour

4. **Contact**:
   - On-call engineer: [PagerDuty]
   - Database team: [Email/Slack]
   - Security team: [Email/Slack]
