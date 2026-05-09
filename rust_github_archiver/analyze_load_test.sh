#!/bin/bash
# Post-Load Test Analysis Script
# Run this after the load test completes to analyze results

echo "=== POST-LOAD TEST ANALYSIS ==="
echo ""

API_LOG_FILE="${API_LOG_FILE:-logs/api_server.log}"
mkdir -p "$(dirname "$API_LOG_FILE")"

# 1. Check if server is still running
echo "1. Server Status:"
if ps aux | grep -q "[a]pi_server"; then
    echo "   ✅ Server still running"
else
    echo "   ❌ Server stopped (may have crashed)"
fi
echo ""

# 2. Check server logs for errors
echo "2. Server Errors (last 50 lines):"
if [ -f "$API_LOG_FILE" ]; then
    tail -50 "$API_LOG_FILE" | grep -i "error\|panic\|fatal" || echo "   ✅ No errors in recent logs"
else
    echo "   ⚠️  Log file not found: $API_LOG_FILE"
fi
echo ""

# 3. Show request statistics from logs
echo "3. Request Log Summary:"
echo "   Total requests:"
grep "HTTP request received" "$API_LOG_FILE" 2>/dev/null | wc -l
echo "   Successful requests (200):"
grep "HTTP request completed successfully" "$API_LOG_FILE" 2>/dev/null | wc -l
echo "   Failed requests (4xx/5xx):"
grep "HTTP request failed" "$API_LOG_FILE" 2>/dev/null | wc -l
echo ""

# 4. Response time analysis
echo "4. Response Time Distribution (from logs):"
echo "   Fast (<10ms):"
grep "duration_ms=" "$API_LOG_FILE" 2>/dev/null | awk -F'duration_ms=' '{print $2}' | awk '{print $1}' | awk '$1<10' | wc -l
echo "   Medium (10-100ms):"
grep "duration_ms=" "$API_LOG_FILE" 2>/dev/null | awk -F'duration_ms=' '{print $2}' | awk '{print $1}' | awk '$1>=10 && $1<100' | wc -l
echo "   Slow (>100ms):"
grep "duration_ms=" "$API_LOG_FILE" 2>/dev/null | awk -F'duration_ms=' '{print $2}' | awk '{print $1}' | awk '$1>=100' | wc -l
echo ""

echo "=== LOAD TEST RESULTS READY FOR REVIEW ==="
echo ""
echo "Next steps:"
echo "1. Review k6 output for final metrics"
echo "2. Check PostgreSQL for audit logs (see verify_audit_logs.sh)"
echo "3. Compare performance with/without audit logging"
echo "4. Document results in SESSION_7_LOAD_TEST_RESULTS.md"
