#!/bin/bash

# Safe API Startup Script for Oracle Cloud
# This script starts the API with resource monitoring and safety checks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] ✅${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] ⚠️${NC} $1"
}

log_error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ❌${NC} $1"
}

# Check system resources before starting
check_system_resources() {
    log "Checking system resources..."
    
    # Check available memory (should have at least 4GB free)
    available_mem=$(free -g | awk '/^Mem:/{print $7}')
    if [ "$available_mem" -lt 4 ]; then
        log_warning "Low available memory: ${available_mem}GB (recommended: 4GB+)"
        log "Consider running: sudo systemctl restart postgresql"
    else
        log_success "Available memory: ${available_mem}GB"
    fi
    
    # Check disk space (should have at least 10GB free)
    available_disk=$(df -BG / | awk 'NR==2 {print $4}' | sed 's/G//')
    if [ "$available_disk" -lt 10 ]; then
        log_warning "Low disk space: ${available_disk}GB (recommended: 10GB+)"
        log "Consider cleaning up old files"
    else
        log_success "Available disk space: ${available_disk}GB"
    fi
    
    # Check CPU load
    load_avg=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $1}' | sed 's/,//')
    cpu_cores=$(nproc)
    if (( $(echo "$load_avg > $cpu_cores" | bc -l) )); then
        log_warning "High CPU load: ${load_avg} (cores: ${cpu_cores})"
    else
        log_success "CPU load normal: ${load_avg} (cores: ${cpu_cores})"
    fi
}

# Check database connectivity
check_database() {
    log "Checking PostgreSQL database..."
    
    if ! systemctl is-active --quiet postgresql; then
        log_error "PostgreSQL is not running"
        log "Starting PostgreSQL..."
        sudo systemctl start postgresql
        sleep 3
    fi
    
    if systemctl is-active --quiet postgresql; then
        log_success "PostgreSQL is running"
    else
        log_error "Failed to start PostgreSQL"
        exit 1
    fi
    
    # Test database connection
    if python3 -c "
import sys
sys.path.append('.')
from config import Config
import asyncpg
import asyncio

async def test_db():
    config = Config()
    try:
        dsn = f'postgresql://{config.DB_USER}:{config.DB_PASSWORD}@{config.DB_HOST}:{config.DB_PORT}/{config.DB_NAME}'
        conn = await asyncpg.connect(dsn)
        await conn.fetchval('SELECT 1')
        await conn.close()
        return True
    except Exception as e:
        print(f'Database connection failed: {e}', file=sys.stderr)
        return False

result = asyncio.run(test_db())
sys.exit(0 if result else 1)
" 2>/dev/null; then
        log_success "Database connection successful"
    else
        log_error "Database connection failed"
        log "Please check database configuration in .env file"
        exit 1
    fi
}

# Setup virtual environment
setup_venv() {
    log "Setting up Python virtual environment..."
    
    if [ ! -d "venv" ]; then
        log "Creating virtual environment..."
        python3 -m venv venv
    fi
    
    source venv/bin/activate
    
    # Install/upgrade required packages
    log "Installing/updating dependencies..."
    pip install --upgrade pip
    pip install -r requirements.txt
    
    log_success "Virtual environment ready"
}

# Create necessary directories
create_directories() {
    log "Creating necessary directories..."
    
    mkdir -p gharchive_data
    mkdir -p reports
    mkdir -p logs
    
    log_success "Directories created"
}

# Start system monitor in background
start_system_monitor() {
    log "Starting system monitor..."
    
    # Kill existing monitor if running
    pkill -f "python.*system_monitor.py" || true
    
    # Start new monitor
    source venv/bin/activate
    nohup python3 system_monitor.py --interval 60 > logs/system_monitor.log 2>&1 &
    
    log_success "System monitor started"
}

# Start the API server
start_api_server() {
    log "Starting Safe API server..."
    
    # Kill existing API server if running
    pkill -f "python.*api.py" || true
    sleep 2
    
    # Activate virtual environment
    source venv/bin/activate
    
    # Set environment variables for safety
    export PYTHONUNBUFFERED=1
    export PYTHONDONTWRITEBYTECODE=1
    
    # Start API server
    if [ "$1" = "--background" ]; then
        log "Starting API in background mode..."
        nohup python3 api.py > logs/api.log 2>&1 &
        api_pid=$!
        echo $api_pid > api.pid
        
        # Wait a moment and check if it's running
        sleep 3
        if kill -0 $api_pid 2>/dev/null; then
            log_success "API server started successfully (PID: $api_pid)"
            log "Dashboard available at: http://170.9.239.38:8080"
            log "API endpoints at: http://170.9.239.38:8080/api/"
            log "Logs: tail -f logs/api.log"
        else
            log_error "API server failed to start"
            exit 1
        fi
    else
        log "Starting API in foreground mode..."
        log_success "Dashboard will be available at: http://170.9.239.38:8080"
        python3 api.py
    fi
}

# Stop all services
stop_services() {
    log "Stopping all services..."
    
    # Stop API server
    if [ -f api.pid ]; then
        api_pid=$(cat api.pid)
        if kill -0 $api_pid 2>/dev/null; then
            log "Stopping API server (PID: $api_pid)"
            kill $api_pid
            rm -f api.pid
        fi
    fi
    
    # Stop system monitor
    pkill -f "python.*system_monitor.py" || true
    
    # Stop scraper if running
    pkill -f "python.*gharchive_scraper.py" || true
    
    log_success "All services stopped"
}

# Show service status
show_status() {
    log "Service Status:"
    
    # API server
    if pgrep -f "python.*api.py" > /dev/null; then
        log_success "API Server: Running"
        if [ -f api.pid ]; then
            api_pid=$(cat api.pid)
            log "  PID: $api_pid"
        fi
    else
        log_warning "API Server: Not running"
    fi
    
    # System monitor
    if pgrep -f "python.*system_monitor.py" > /dev/null; then
        log_success "System Monitor: Running"
    else
        log_warning "System Monitor: Not running"
    fi
    
    # Scraper
    if pgrep -f "python.*gharchive_scraper.py" > /dev/null; then
        log_success "Scraper: Running"
    else
        log_warning "Scraper: Not running"
    fi
    
    # PostgreSQL
    if systemctl is-active --quiet postgresql; then
        log_success "PostgreSQL: Running"
    else
        log_error "PostgreSQL: Not running"
    fi
}

# Main function
main() {
    case "$1" in
        start)
            log "🚀 Starting Safe API for Oracle Cloud..."
            check_system_resources
            setup_venv
            create_directories
            check_database
            start_system_monitor
            start_api_server "${@:2}"
            ;;
        stop)
            stop_services
            ;;
        restart)
            stop_services
            sleep 2
            main start "${@:2}"
            ;;
        status)
            show_status
            ;;
        monitor)
            log "Starting system monitor in foreground..."
            setup_venv
            source venv/bin/activate
            python3 system_monitor.py "${@:2}"
            ;;
        check)
            log "Running system checks..."
            check_system_resources
            check_database
            log_success "All checks completed"
            ;;
        *)
            echo "Usage: $0 {start|stop|restart|status|monitor|check} [options]"
            echo ""
            echo "Commands:"
            echo "  start [--background]  Start the API server and monitoring"
            echo "  stop                  Stop all services"
            echo "  restart              Restart all services"
            echo "  status               Show service status"
            echo "  monitor              Start system monitor in foreground"
            echo "  check                Run system checks only"
            echo ""
            echo "Options:"
            echo "  --background         Run API server in background"
            echo ""
            echo "Examples:"
            echo "  $0 start             Start API in foreground"
            echo "  $0 start --background Start API in background"
            echo "  $0 monitor --interval 30  Monitor every 30 seconds"
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
