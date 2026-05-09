#!/bin/bash
# Verify audit logs created during authenticated load test

set -e

echo "=== Audit Log Verification ==="
echo ""

# Database connection details
DB_HOST="${DATABASE_HOST:-localhost}"
DB_PORT="${DATABASE_PORT:-5432}"
DB_NAME="${DATABASE_NAME:-github_archiver}"
DB_USER="${DATABASE_USER:-postgres}"

echo "Database: $DB_HOST:$DB_PORT/$DB_NAME"
echo ""

# Function to run SQL query
run_query() {
  psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "$1" 2>/dev/null
}

# 1. Total audit logs in last 10 minutes
echo "1. Recent Audit Logs (last 10 minutes):"
RECENT_COUNT=$(run_query "SELECT COUNT(*) FROM audit_logs WHERE created_at > NOW() - INTERVAL '10 minutes';" | tr -d ' ')
echo "   Total: $RECENT_COUNT"
echo ""

# 2. Breakdown by action
echo "2. Audit Logs by Action:"
run_query "
  SELECT 
    action,
    COUNT(*) as count,
    ROUND(AVG(EXTRACT(EPOCH FROM (created_at - created_at)) * 1000)::numeric, 2) as avg_ms
  FROM audit_logs 
  WHERE created_at > NOW() - INTERVAL '10 minutes'
  GROUP BY action
  ORDER BY count DESC;
" | while read line; do
  echo "   $line"
done
echo ""

# 3. Success vs failure breakdown
echo "3. Success vs Failure:"
run_query "
  SELECT 
    success,
    COUNT(*) as count,
    ROUND((COUNT(*) * 100.0 / SUM(COUNT(*)) OVER())::numeric, 2) as percentage
  FROM audit_logs 
  WHERE created_at > NOW() - INTERVAL '10 minutes'
  GROUP BY success
  ORDER BY success DESC;
" | while read line; do
  echo "   $line"
done
echo ""

# 4. Top users (load test users)
echo "4. Top Users (Load Test Users):"
run_query "
  SELECT 
    username,
    COUNT(*) as actions
  FROM audit_logs 
  WHERE created_at > NOW() - INTERVAL '10 minutes'
    AND username LIKE 'loadtest_%'
  GROUP BY username
  ORDER BY actions DESC
  LIMIT 10;
" | while read line; do
  echo "   $line"
done
echo ""

# 5. Timeline - logs per minute
echo "5. Audit Log Timeline (logs per minute):"
run_query "
  SELECT 
    DATE_TRUNC('minute', created_at) as minute,
    COUNT(*) as logs_count
  FROM audit_logs 
  WHERE created_at > NOW() - INTERVAL '10 minutes'
  GROUP BY DATE_TRUNC('minute', created_at)
  ORDER BY minute DESC;
" | while read line; do
  echo "   $line"
done
echo ""

# 6. Average details size (JSON payload)
echo "6. Audit Log Details Size:"
run_query "
  SELECT 
    AVG(LENGTH(details::text)) as avg_size_bytes,
    MAX(LENGTH(details::text)) as max_size_bytes,
    MIN(LENGTH(details::text)) as min_size_bytes
  FROM audit_logs 
  WHERE created_at > NOW() - INTERVAL '10 minutes'
    AND details IS NOT NULL;
" | while read line; do
  echo "   $line"
done
echo ""

# 7. Failed operations (for investigation)
echo "7. Failed Operations (last 20):"
FAILED_COUNT=$(run_query "SELECT COUNT(*) FROM audit_logs WHERE created_at > NOW() - INTERVAL '10 minutes' AND success = false;" | tr -d ' ')
echo "   Total failures: $FAILED_COUNT"

if [ "$FAILED_COUNT" -gt "0" ]; then
  echo "   Recent failures:"
  run_query "
    SELECT 
      created_at,
      action,
      username,
      error_message
    FROM audit_logs 
    WHERE created_at > NOW() - INTERVAL '10 minutes'
      AND success = false
    ORDER BY created_at DESC
    LIMIT 20;
  " | while read line; do
    echo "     $line"
  done
fi
echo ""

# 8. Database table statistics
echo "8. Audit Logs Table Statistics:"
run_query "
  SELECT 
    pg_size_pretty(pg_total_relation_size('audit_logs')) as total_size,
    pg_size_pretty(pg_relation_size('audit_logs')) as table_size,
    pg_size_pretty(pg_total_relation_size('audit_logs') - pg_relation_size('audit_logs')) as index_size
  FROM pg_class 
  WHERE relname = 'audit_logs';
" | while read line; do
  echo "   $line"
done
echo ""

# 9. Index usage statistics
echo "9. Index Usage (audit_logs table):"
run_query "
  SELECT 
    indexrelname as index_name,
    idx_scan as scans,
    idx_tup_read as tuples_read,
    idx_tup_fetch as tuples_fetched
  FROM pg_stat_user_indexes 
  WHERE relname = 'audit_logs'
  ORDER BY idx_scan DESC;
" | while read line; do
  echo "   $line"
done
echo ""

# Summary
echo "=== SUMMARY ==="
echo "Total audit logs (last 10 min): $RECENT_COUNT"
echo "Failed operations: $FAILED_COUNT"
echo ""

if [ "$RECENT_COUNT" -gt "0" ]; then
  echo "✅ Audit logging is working!"
  echo ""
  echo "Expected from load test:"
  echo "  - 4 audit logs per iteration (login, start, stop, logout)"
  echo "  - Check if count matches k6 'iterations' metric × 4"
  echo ""
  echo "Performance analysis:"
  echo "  - Compare k6 response times with/without audit logging"
  echo "  - Check database query performance during test"
  echo "  - Verify no database connection pool exhaustion"
else
  echo "⚠️  No audit logs found in the last 10 minutes"
  echo ""
  echo "Possible issues:"
  echo "  - Load test didn't run or completed >10 minutes ago"
  echo "  - Audit logging not configured correctly"
  echo "  - Test users don't have permission to create audit logs"
  echo "  - Database connection issue"
fi
echo ""
