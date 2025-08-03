#!/usr/bin/env bash

# GitHub Secret Hunter - Health Check Script
# Monitors system health, performance, and service status

set -e

HEALTH_FILE="/tmp/secret_hunter_health"
TIMESTAMP=$(date +%s)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="$PROJECT_ROOT/logs/health_check.log"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Create logs directory if it doesn't exist
mkdir -p "$PROJECT_ROOT/logs"

# Logging function
log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') - $1" | tee -a "$LOG_FILE"
}

# Function to check service health
check_service_health() {
    local service_name=$1
    local port=$2
    
    if command -v nc >/dev/null 2>&1 && nc -z localhost $port 2>/dev/null; then
        echo -e "✅ $service_name is running (port $port)"
        log "✅ $service_name is running (port $port)"
        return 0
    else
        echo -e "❌ $service_name is not responding (port $port)"
        log "❌ $service_name is not responding (port $port)"
        return 1
    fi
}

# Function to check system resources
check_system_resources() {
    local cpu_usage memory_usage disk_usage
    
    # Get CPU usage (works on Linux and macOS)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        cpu_usage=$(top -l 1 | grep "CPU usage" | awk '{print $3}' | sed 's/%//')
    else
        cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | awk -F'%' '{print $1}')
    fi
    
    # Get memory usage
    if [[ "$OSTYPE" == "darwin"* ]]; then
        memory_usage=$(vm_stat | awk '/Pages free/ {free=$3} /Pages active/ {active=$3} /Pages inactive/ {inactive=$3} /Pages speculative/ {spec=$3} /Pages wired/ {wired=$3} END {total=free+active+inactive+spec+wired; used=active+inactive+wired; printf "%.1f", used/total*100}')
    else
        memory_usage=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')
    fi
    
    # Get disk usage
    disk_usage=$(df / | awk 'NR==2 {print $5}' | sed 's/%//')
    
    echo -e "🖥️  CPU Usage: ${cpu_usage}%"
    echo -e "💾 Memory Usage: ${memory_usage}%"
    echo -e "💿 Disk Usage: ${disk_usage}%"
    
    log "🖥️  CPU Usage: ${cpu_usage}%"
    log "💾 Memory Usage: ${memory_usage}%"
    log "💿 Disk Usage: ${disk_usage}%"
    
    # Alert thresholds
    local alerts=0
    if (( $(echo "$cpu_usage > 90" | bc -l 2>/dev/null || echo 0) )); then
        echo -e "${RED}⚠️  HIGH CPU USAGE ALERT${NC}"
        log "⚠️  HIGH CPU USAGE ALERT"
        alerts=$((alerts + 1))
    fi
    
    if (( $(echo "$memory_usage > 90" | bc -l 2>/dev/null || echo 0) )); then
        echo -e "${RED}⚠️  HIGH MEMORY USAGE ALERT${NC}"
        log "⚠️  HIGH MEMORY USAGE ALERT"
        alerts=$((alerts + 1))
    fi
    
    if (( disk_usage > 90 )); then
        echo -e "${RED}⚠️  HIGH DISK USAGE ALERT${NC}"
        log "⚠️  HIGH DISK USAGE ALERT"
        alerts=$((alerts + 1))
    fi
    
    return $alerts
}

# Function to check Rust service
check_rust_service() {
    if pgrep -f "github_archiver" > /dev/null; then
        echo -e "✅ GitHub Secret Hunter service is running"
        log "✅ GitHub Secret Hunter service is running"
        return 0
    else
        echo -e "❌ GitHub Secret Hunter service is not running"
        log "❌ GitHub Secret Hunter service is not running"
        return 1
    fi
}

# Function to check configuration files
check_configuration() {
    local config_health=0
    
    if [ -f "$PROJECT_ROOT/.env" ]; then
        echo -e "✅ Environment configuration found"
        log "✅ Environment configuration found"
        
        # Check for required environment variables
        if grep -q "GITHUB_TOKEN=" "$PROJECT_ROOT/.env" && ! grep -q "your_github_token_here" "$PROJECT_ROOT/.env"; then
            echo -e "✅ GitHub token configured"
            log "✅ GitHub token configured"
        else
            echo -e "⚠️  GitHub token not properly configured"
            log "⚠️  GitHub token not properly configured"
            config_health=$((config_health + 1))
        fi
    else
        echo -e "❌ Environment configuration not found"
        log "❌ Environment configuration not found"
        config_health=$((config_health + 1))
    fi
    
    if [ -f "$PROJECT_ROOT/Cargo.toml" ]; then
        echo -e "✅ Rust project configuration found"
        log "✅ Rust project configuration found"
    else
        echo -e "❌ Rust project configuration not found"
        log "❌ Rust project configuration not found"
        config_health=$((config_health + 1))
    fi
    
    return $config_health
}

# Function to check dependencies
check_dependencies() {
    local dep_health=0
    
    # Check Rust
    if command -v rustc >/dev/null 2>&1; then
        echo -e "✅ Rust installed: $(rustc --version | cut -d' ' -f2)"
        log "✅ Rust installed: $(rustc --version | cut -d' ' -f2)"
    else
        echo -e "❌ Rust not installed"
        log "❌ Rust not installed"
        dep_health=$((dep_health + 1))
    fi
    
    # Check Cargo
    if command -v cargo >/dev/null 2>&1; then
        echo -e "✅ Cargo available"
        log "✅ Cargo available"
    else
        echo -e "❌ Cargo not available"
        log "❌ Cargo not available"
        dep_health=$((dep_health + 1))
    fi
    
    # Check if binary exists
    if [ -f "$PROJECT_ROOT/target/release/github_archiver" ]; then
        echo -e "✅ Release binary found"
        log "✅ Release binary found"
    elif [ -f "$PROJECT_ROOT/target/debug/github_archiver" ]; then
        echo -e "⚠️  Debug binary found (consider building release version)"
        log "⚠️  Debug binary found (consider building release version)"
    else
        echo -e "❌ No binary found - run 'cargo build --release'"
        log "❌ No binary found - run 'cargo build --release'"
        dep_health=$((dep_health + 1))
    fi
    
    return $dep_health
}

# Function to check log files and recent errors
check_logs() {
    local log_health=0
    
    if [ -f "$PROJECT_ROOT/logs/secret_hunter.log" ]; then
        local error_count=$(tail -n 100 "$PROJECT_ROOT/logs/secret_hunter.log" 2>/dev/null | grep -c "ERROR" || true)
        local warning_count=$(tail -n 100 "$PROJECT_ROOT/logs/secret_hunter.log" 2>/dev/null | grep -c "WARN" || true)
        
        echo -e "📋 Recent errors in logs: $error_count"
        echo -e "📋 Recent warnings in logs: $warning_count"
        log "📋 Recent errors in logs: $error_count"
        log "📋 Recent warnings in logs: $warning_count"
        
        if [ $error_count -gt 10 ]; then
            echo -e "${RED}⚠️  HIGH ERROR RATE ALERT${NC}"
            log "⚠️  HIGH ERROR RATE ALERT"
            log_health=$((log_health + 1))
        fi
        
        if [ $warning_count -gt 20 ]; then
            echo -e "${YELLOW}⚠️  HIGH WARNING RATE${NC}"
            log "⚠️  HIGH WARNING RATE"
        fi
    else
        echo -e "❌ Main log file not found"
        log "❌ Main log file not found"
        log_health=$((log_health + 1))
    fi
    
    return $log_health
}

# Main health check function
main() {
    echo -e "${BLUE}🔍 GitHub Secret Hunter Health Check - $(date)${NC}"
    echo "================================================"
    log "🔍 Starting health check"
    
    local overall_health=0
    
    echo ""
    echo -e "${CYAN}📊 System Resources${NC}"
    echo "==================="
    if ! check_system_resources; then
        overall_health=$((overall_health + $?))
    fi
    
    echo ""
    echo -e "${CYAN}🔧 Dependencies${NC}"
    echo "==============="
    check_dependencies
    overall_health=$((overall_health + $?))
    
    echo ""
    echo -e "${CYAN}⚙️  Configuration${NC}"
    echo "================="
    check_configuration
    overall_health=$((overall_health + $?))
    
    echo ""
    echo -e "${CYAN}🚀 Services${NC}"
    echo "==========="
    
    # Check main Rust service
    check_rust_service
    overall_health=$((overall_health + $?))
    
    # Check external services
    check_service_health "Redis" 6379 || overall_health=$((overall_health + 1))
    check_service_health "PostgreSQL" 5432 || true  # Optional
    
    echo ""
    echo -e "${CYAN}📝 Logs${NC}"
    echo "======="
    check_logs
    overall_health=$((overall_health + $?))
    
    # Generate health report
    echo ""
    echo "================================================"
    if [ $overall_health -eq 0 ]; then
        echo -e "${GREEN}✅ OVERALL HEALTH: EXCELLENT${NC}"
        echo "healthy" > "$HEALTH_FILE"
        log "✅ OVERALL HEALTH: EXCELLENT"
        exit 0
    elif [ $overall_health -le 3 ]; then
        echo -e "${YELLOW}⚠️  OVERALL HEALTH: WARNING${NC}"
        echo "warning" > "$HEALTH_FILE"
        log "⚠️  OVERALL HEALTH: WARNING"
        exit 1
    else
        echo -e "${RED}❌ OVERALL HEALTH: CRITICAL${NC}"
        echo "critical" > "$HEALTH_FILE"
        log "❌ OVERALL HEALTH: CRITICAL"
        exit 2
    fi
}

# Run the health check
main "$@"
