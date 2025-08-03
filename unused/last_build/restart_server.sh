#!/bin/bash

# GitArchiver Professional API Server Restart Script
# This script safely stops and restarts the professional API server

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_PATH="$SCRIPT_DIR/github_scraper_env"
API_SCRIPT="$SCRIPT_DIR/professional_api.py"
PID_FILE="$SCRIPT_DIR/.api_server.pid"
LOG_FILE="$SCRIPT_DIR/logs/api.log"

# Functions
log() {
    echo -e "${BLUE}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check if virtual environment exists
check_venv() {
    if [ ! -d "$VENV_PATH" ]; then
        error "Virtual environment not found at: $VENV_PATH"
        error "Please run setup.sh first to create the virtual environment"
        exit 1
    fi
}

# Check if API script exists
check_api_script() {
    if [ ! -f "$API_SCRIPT" ]; then
        error "API script not found at: $API_SCRIPT"
        exit 1
    fi
}

# Stop the server
stop_server() {
    log "Stopping Professional API server..."
    
    # Try to stop gracefully using PID file
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            log "Sending SIGTERM to process $pid..."
            kill -TERM "$pid"
            
            # Wait for graceful shutdown (up to 10 seconds)
            local count=0
            while kill -0 "$pid" 2>/dev/null && [ $count -lt 10 ]; do
                sleep 1
                count=$((count + 1))
            done
            
            # Force kill if still running
            if kill -0 "$pid" 2>/dev/null; then
                warning "Process $pid didn't exit gracefully, force killing..."
                kill -KILL "$pid"
            fi
        fi
        rm -f "$PID_FILE"
    fi
    
    # Also kill any remaining professional_api.py processes
    local remaining_pids=$(pgrep -f "professional_api.py" 2>/dev/null || true)
    if [ -n "$remaining_pids" ]; then
        warning "Found remaining professional_api.py processes: $remaining_pids"
        for pid in $remaining_pids; do
            log "Killing process $pid..."
            kill -TERM "$pid" 2>/dev/null || true
            sleep 1
            kill -KILL "$pid" 2>/dev/null || true
        done
    fi
    
    # Check if port 8080 is still in use and kill those processes
    local port_pids=$(lsof -t -i :8080 2>/dev/null | grep -E '^[0-9]+$' || true)
    if [ -n "$port_pids" ]; then
        warning "Found processes still using port 8080: $port_pids"
        for pid in $port_pids; do
            local process_name=$(ps -p "$pid" -o comm= 2>/dev/null || echo "unknown")
            if [ "$process_name" = "python" ] || [ "$process_name" = "python3" ]; then
                log "Killing Python process $pid using port 8080..."
                kill -TERM "$pid" 2>/dev/null || true
                sleep 1
                kill -KILL "$pid" 2>/dev/null || true
            fi
        done
    fi
    
    # Wait a moment for processes to clean up
    sleep 3
    
    # Final check
    local final_check=$(lsof -t -i :8080 2>/dev/null | grep -E '^[0-9]+$' || true)
    if [ -n "$final_check" ]; then
        warning "Port 8080 is still in use by processes: $final_check"
        warning "You may need to manually kill these processes"
    else
        success "Server stopped and port 8080 is free"
    fi
}

# Start the server
start_server() {
    log "Starting Professional API server..."
    
    # Activate virtual environment and start server
    cd "$SCRIPT_DIR"
    source "$VENV_PATH/bin/activate"
    
    # Create logs directory if it doesn't exist
    mkdir -p "$(dirname "$LOG_FILE")"
    
    # Start server in background
    nohup python "$API_SCRIPT" >> "$LOG_FILE" 2>&1 &
    local pid=$!
    
    # Save PID
    echo $pid > "$PID_FILE"
    
    # Wait a moment and check if it's still running
    sleep 3
    if kill -0 "$pid" 2>/dev/null; then
        success "Server started successfully (PID: $pid)"
        log "Server is running at: http://localhost:8080"
        log "Logs: $LOG_FILE"
    else
        error "Server failed to start. Check logs: $LOG_FILE"
        rm -f "$PID_FILE"
        exit 1
    fi
}

# Check server status
check_status() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            success "Server is running (PID: $pid)"
            
            # Test if server is responding
            if command -v curl >/dev/null 2>&1; then
                log "Testing server response..."
                if curl -s http://localhost:8080/api/health >/dev/null; then
                    success "Server is responding to requests"
                else
                    warning "Server process is running but not responding to requests"
                fi
            fi
            return 0
        else
            warning "PID file exists but process is not running"
            rm -f "$PID_FILE"
        fi
    else
        log "Server is not running"
    fi
    return 1
}

# Show help
show_help() {
    echo "GitArchiver Professional API Server Control Script"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  start     Start the server"
    echo "  stop      Stop the server"
    echo "  restart   Restart the server (default)"
    echo "  status    Check server status"
    echo "  logs      Show recent logs"
    echo "  help      Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                # Restart the server"
    echo "  $0 start          # Start the server"
    echo "  $0 status         # Check if server is running"
    echo "  $0 logs           # Show recent log entries"
}

# Show recent logs
show_logs() {
    if [ -f "$LOG_FILE" ]; then
        log "Showing last 50 lines of $LOG_FILE:"
        echo ""
        tail -n 50 "$LOG_FILE"
    else
        warning "Log file not found: $LOG_FILE"
    fi
}

# Main script
main() {
    local command="${1:-restart}"
    
    case "$command" in
        "start")
            check_venv
            check_api_script
            if check_status; then
                warning "Server is already running"
                exit 0
            fi
            start_server
            ;;
        "stop")
            stop_server
            ;;
        "restart")
            check_venv
            check_api_script
            stop_server
            start_server
            ;;
        "status")
            check_status
            ;;
        "logs")
            show_logs
            ;;
        "help"|"-h"|"--help")
            show_help
            ;;
        *)
            error "Unknown command: $command"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
