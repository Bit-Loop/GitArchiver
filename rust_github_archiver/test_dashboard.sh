#!/usr/bin/env bash
# Web Dashboard Testing Script
# This script helps test the web GUI functionality without back-and-forth

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
API_PORT="${WEB_PORT:-3000}"
API_BASE="http://localhost:${API_PORT}"
DASHBOARD_URL="${API_BASE}/dashboard.html"
REBUILD="${1:-}"
API_LOG_FILE="${API_LOG_FILE:-logs/api_server.log}"
mkdir -p "$(dirname "$API_LOG_FILE")"

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  GitHub Archiver - Web Dashboard Test Suite${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# Function to test endpoint
test_endpoint() {
    local name="$1"
    local url="$2"
    local expected_status="${3:-200}"
    local method="${4:-GET}"
    
    printf "Testing %-40s ... " "$name"
    
    if [ "$method" = "GET" ]; then
        response=$(curl -s -w "\n%{http_code}" "$url" 2>/dev/null || echo "000")
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" 2>/dev/null || echo "000")
    fi
    
    status=$(echo "$response" | tail -n1)
    body=$(echo "$response" | head -n-1)
    
    if [ "$status" = "$expected_status" ]; then
        echo -e "${GREEN}✓ PASS${NC} (HTTP $status)"
        return 0
    else
        echo -e "${RED}✗ FAIL${NC} (HTTP $status, expected $expected_status)"
        if [ "$status" = "000" ]; then
            echo -e "  ${YELLOW}Server not responding - is it running?${NC}"
        elif [ "$status" = "401" ]; then
            echo -e "  ${YELLOW}Unauthorized - endpoint requires authentication or rebuild needed${NC}"
        fi
        return 1
    fi
}

# Function to test JSON response
test_json_endpoint() {
    local name="$1"
    local url="$2"
    local json_path="$3"
    
    printf "Testing %-40s ... " "$name"
    
    response=$(curl -s "$url" 2>/dev/null)
    
    if echo "$response" | jq -e "$json_path" > /dev/null 2>&1; then
        value=$(echo "$response" | jq -r "$json_path")
        echo -e "${GREEN}✓ PASS${NC} (value: $value)"
        return 0
    else
        echo -e "${RED}✗ FAIL${NC} (JSON path not found)"
        echo -e "  ${YELLOW}Response: $(echo "$response" | head -c 100)...${NC}"
        return 1
    fi
}

# Function to check file exists
check_file() {
    local name="$1"
    local file="$2"
    
    printf "Checking %-40s ... " "$name"
    
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓ EXISTS${NC}"
        return 0
    else
        echo -e "${RED}✗ MISSING${NC}"
        return 1
    fi
}

# Function to check CSS animation
check_animation() {
    local name="$1"
    local pattern="$2"
    
    printf "Checking %-40s ... " "$name"
    
    if grep -q "$pattern" dashboard.html 2>/dev/null; then
        echo -e "${GREEN}✓ FOUND${NC}"
        return 0
    else
        echo -e "${RED}✗ MISSING${NC}"
        return 1
    fi
}

echo -e "${YELLOW}[1/7] Pre-flight Checks${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
check_file "API Binary" "./target/release/examples/api_server"
BINARY_EXISTS=$?
check_file "Dashboard HTML" "./dashboard.html"
check_file "Environment File" "./.env"
echo ""

# Check if rebuild is needed or requested
if [ "$REBUILD" = "--rebuild" ] || [ "$REBUILD" = "-r" ]; then
    echo -e "${YELLOW}[2/7] Rebuilding API Server${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo -e "${BLUE}Running: SQLX_OFFLINE=true cargo build --release --example api_server${NC}"
    if SQLX_OFFLINE=true cargo build --release --example api_server 2>&1 | tail -10; then
        echo -e "${GREEN}✓ Build successful${NC}"
    else
        echo -e "${RED}✗ Build failed${NC}"
        exit 1
    fi
    echo ""
elif [ $BINARY_EXISTS -ne 0 ]; then
    echo -e "${RED}Binary missing! Run with --rebuild flag${NC}"
    exit 1
else
    # Check binary age
    BINARY_AGE=$(stat -c %Y "./target/release/examples/api_server" 2>/dev/null || echo 0)
    NOW=$(date +%s)
    AGE_HOURS=$(( (NOW - BINARY_AGE) / 3600 ))
    
    if [ $AGE_HOURS -gt 24 ]; then
        echo -e "${YELLOW}⚠️  Binary is $AGE_HOURS hours old. Consider rebuilding with --rebuild${NC}"
        echo ""
    fi
fi

echo -e "${YELLOW}[3/7] Server Status${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if pgrep -f "api_server" > /dev/null; then
    echo -e "${GREEN}✓ API Server is running${NC}"
    pid=$(pgrep -f "api_server" | head -1)
    echo "  PID: $pid"
    echo "  Port: $API_PORT"
    
    # Check if restart is needed after rebuild
    if [ "$REBUILD" = "--rebuild" ] || [ "$REBUILD" = "-r" ]; then
        echo -e "${YELLOW}  Restarting server with new binary...${NC}"
        pkill -f "api_server"
        sleep 2
        WEB_PORT=$API_PORT RUST_LOG=info ./target/release/examples/api_server >> "$API_LOG_FILE" 2>&1 &
        sleep 3
        echo -e "${GREEN}  ✓ Server restarted${NC}"
    fi
else
    echo -e "${RED}✗ API Server is NOT running${NC}"
    echo -e "${YELLOW}  Starting API server...${NC}"
    WEB_PORT=$API_PORT RUST_LOG=info ./target/release/examples/api_server >> "$API_LOG_FILE" 2>&1 &
    sleep 3
    if pgrep -f "api_server" > /dev/null; then
        echo -e "${GREEN}  ✓ Server started successfully${NC}"
    else
        echo -e "${RED}  ✗ Failed to start server${NC}"
        echo -e "${YELLOW}  Check $API_LOG_FILE for errors${NC}"
        exit 1
    fi
fi
echo ""

echo -e "${YELLOW}[4/7] Core API Endpoints${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
test_endpoint "Health Check" "${API_BASE}/health"
test_endpoint "API Health" "${API_BASE}/api/health"
test_endpoint "System Status" "${API_BASE}/api/system/status"
test_endpoint "Dashboard HTML" "${DASHBOARD_URL}"
echo ""

echo -e "${YELLOW}[5/7] Configuration & Security${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
test_json_endpoint "Config Endpoint (Token)" "${API_BASE}/api/config" ".github.has_token" || {
    echo -e "  ${MAGENTA}💡 TIP: This endpoint requires a rebuild to work. Run: ./test_dashboard.sh --rebuild${NC}"
}
test_json_endpoint "Config Endpoint (Port)" "${API_BASE}/api/config" ".web.port" || true
echo ""

echo -e "${YELLOW}[6/7] Frontend Assets & Animations${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
check_animation "Lava Lamp Container" '<div class="lava-lamp">'
check_animation "Bubble Elements" '<div class="bubble"></div>'
check_animation "Float Animation Keyframes" '@keyframes float'
check_animation "Animation CSS Property" 'animation: float'

# Check if animation is actually being served
printf "Checking %-40s ... " "Animation in Served HTML"
if curl -s "${DASHBOARD_URL}" | grep -q "@keyframes float"; then
    echo -e "${GREEN}✓ FOUND${NC}"
else
    echo -e "${RED}✗ MISSING${NC}"
    echo -e "  ${YELLOW}Animation is in source but not being served!${NC}"
fi
echo ""

echo -e "${YELLOW}[7/7] Monitoring & Logs${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
test_endpoint "Monitoring Metrics" "${API_BASE}/api/monitoring/metrics"
test_endpoint "Realtime Status" "${API_BASE}/api/realtime/status"
test_endpoint "Database Status" "${API_BASE}/api/database/status"
echo ""

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Test Suite Complete!${NC}"
echo ""
echo -e "${BLUE}Dashboard URL:${NC} $DASHBOARD_URL"
echo ""
echo -e "${YELLOW}Manual Testing:${NC}"
echo "  1. Open: $DASHBOARD_URL"
echo "  2. Press Ctrl+Shift+R (hard refresh to bypass cache)"
echo "  3. Check browser console (F12) for errors"
echo "  4. Verify background animation is visible"
echo "  5. Check that GitHub token loads automatically"
echo "  6. Test starting the event monitor"
echo ""
echo -e "${YELLOW}Debug Commands:${NC}"
echo "  View logs:       tail -f $API_LOG_FILE"
echo "  Rebuild & test:  ./test_dashboard.sh --rebuild"
echo "  Test config:     curl $API_BASE/api/config | jq"
echo "  Debug mode:      WEB_DEBUG=1 ./target/release/examples/api_server"
echo "  Force reload:    Ctrl+Shift+R in browser (bypass cache)"
echo ""
echo -e "${MAGENTA}Common Issues:${NC}"
echo "  • No animation: Hard refresh browser (Ctrl+Shift+R)"
echo "  • 401 errors: Rebuild needed (./test_dashboard.sh --rebuild)"
echo "  • Token not loading: Check .env file has GITHUB_TOKEN"
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
