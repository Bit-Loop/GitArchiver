#!/bin/bash

# GitHub Archive Scraper - Rust Version
# Professional startup script with proper error handling and logging

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BINARY_NAME="github_archiver"
BUILD_DIR="target/release"
LOG_DIR="logs"
PID_FILE="service.pid"
LOG_FILE="$LOG_DIR/service.log"

# Ensure log directory exists
mkdir -p "$LOG_DIR"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $1"
}

print_error() {
    echo -e "${RED}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $1"
}

# Function to check if process is running
is_running() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE" 2>/dev/null || echo "")
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            return 0
        else
            rm -f "$PID_FILE"
            return 1
        fi
    fi
    return 1
}

# Function to stop the service
stop_service() {
    if is_running; then
        local pid=$(cat "$PID_FILE")
        print_status "Stopping GitHub Archive Scraper (PID: $pid)..."
        
        # Try graceful shutdown first
        if kill -TERM "$pid" 2>/dev/null; then
            # Wait up to 30 seconds for graceful shutdown
            local count=0
            while [ $count -lt 30 ] && kill -0 "$pid" 2>/dev/null; do
                sleep 1
                count=$((count + 1))
            done
            
            # Force kill if still running
            if kill -0 "$pid" 2>/dev/null; then
                print_warning "Graceful shutdown failed, forcing termination..."
                kill -KILL "$pid" 2>/dev/null || true
            fi
        fi
        
        rm -f "$PID_FILE"
        print_success "GitHub Archive Scraper stopped"
    else
        print_warning "GitHub Archive Scraper is not running"
    fi
}

# Function to start the service
start_service() {
    if is_running; then
        print_warning "GitHub Archive Scraper is already running (PID: $(cat $PID_FILE))"
        return 0
    fi

    print_status "Starting GitHub Archive Scraper..."

    # Check if binary exists
    if [ ! -f "$BUILD_DIR/$BINARY_NAME" ]; then
        print_error "Binary not found: $BUILD_DIR/$BINARY_NAME"
        print_status "Building application..."
        if ! cargo build --release; then
            print_error "Build failed"
            exit 1
        fi
    fi

    # Start the service in background
    nohup "$BUILD_DIR/$BINARY_NAME" >> "$LOG_FILE" 2>&1 &
    local pid=$!
    
    # Save PID
    echo "$pid" > "$PID_FILE"
    
    # Give it a moment to start
    sleep 2
    
    # Check if it's still running
    if kill -0 "$pid" 2>/dev/null; then
        print_success "GitHub Archive Scraper started successfully (PID: $pid)"
        print_status "Logs: tail -f $LOG_FILE"
        return 0
    else
        print_error "Failed to start GitHub Archive Scraper"
        rm -f "$PID_FILE"
        print_status "Check logs: tail $LOG_FILE"
        exit 1
    fi
}

# Function to restart the service
restart_service() {
    print_status "Restarting GitHub Archive Scraper..."
    stop_service
    sleep 2
    start_service
}

# Function to show status
show_status() {
    if is_running; then
        local pid=$(cat "$PID_FILE")
        print_success "GitHub Archive Scraper is running (PID: $pid)"
        
        # Show process info
        if command -v ps >/dev/null; then
            echo "Process info:"
            ps -p "$pid" -o pid,ppid,cmd,etime,%cpu,%mem 2>/dev/null || true
        fi
        
        # Show recent logs
        if [ -f "$LOG_FILE" ]; then
            echo ""
            echo "Recent logs (last 10 lines):"
            tail -n 10 "$LOG_FILE"
        fi
    else
        print_warning "GitHub Archive Scraper is not running"
    fi
}

# Function to show logs
show_logs() {
    if [ -f "$LOG_FILE" ]; then
        if [ "${1:-}" = "-f" ]; then
            print_status "Following logs (Ctrl+C to stop)..."
            tail -f "$LOG_FILE"
        else
            local lines="${1:-50}"
            print_status "Showing last $lines lines of logs..."
            tail -n "$lines" "$LOG_FILE"
        fi
    else
        print_warning "Log file not found: $LOG_FILE"
    fi
}

# Function to build the application
build_app() {
    print_status "Building GitHub Archive Scraper..."
    
    if cargo build --release; then
        print_success "Build completed successfully"
        
        # Show binary info
        if [ -f "$BUILD_DIR/$BINARY_NAME" ]; then
            local size=$(du -h "$BUILD_DIR/$BINARY_NAME" | cut -f1)
            print_status "Binary size: $size"
            print_status "Binary path: $BUILD_DIR/$BINARY_NAME"
        fi
    else
        print_error "Build failed"
        exit 1
    fi
}

# Function to run in development mode
dev_run() {
    print_status "Running in development mode..."
    stop_service 2>/dev/null || true
    
    # Run with cargo for hot reloading
    cargo run
}

# Function to run specific commands
run_command() {
    local cmd="$1"
    shift
    
    print_status "Running command: $cmd"
    
    if [ ! -f "$BUILD_DIR/$BINARY_NAME" ]; then
        print_status "Binary not found, building..."
        build_app
    fi
    
    "$BUILD_DIR/$BINARY_NAME" "$cmd" "$@"
}

# Function to show help
show_help() {
    echo "GitHub Archive Scraper - Control Script"
    echo ""
    echo "Usage: $0 {start|stop|restart|status|build|dev|logs|help} [options]"
    echo ""
    echo "Commands:"
    echo "  start           Start the scraper service"
    echo "  stop            Stop the scraper service"
    echo "  restart         Restart the scraper service"
    echo "  status          Show service status"
    echo "  build           Build the application"
    echo "  dev             Run in development mode (with cargo run)"
    echo "  logs [n|-f]     Show logs (n=number of lines, -f=follow)"
    echo "  help            Show this help message"
    echo ""
    echo "Service Commands (passed to binary):"
    echo "  server          Run only the API server"
    echo "  scraper         Run only the scraper"
    echo "  process <file>  Process a specific file"
    echo "  download <url>  Download a specific file"
    echo "  cleanup         Clean up old files"
    echo ""
    echo "Examples:"
    echo "  $0 start                    # Start full service"
    echo "  $0 logs -f                  # Follow logs"
    echo "  $0 logs 100                 # Show last 100 log lines"
    echo "  $0 process 2024-01-01-0.json.gz"
    echo "  $0 download https://data.gharchive.org/2024-01-01-0.json.gz"
}

# Main script logic
case "${1:-help}" in
    start)
        start_service
        ;;
    stop)
        stop_service
        ;;
    restart)
        restart_service
        ;;
    status)
        show_status
        ;;
    build)
        build_app
        ;;
    dev)
        dev_run
        ;;
    logs)
        show_logs "${2:-}"
        ;;
    server|scraper|process|download|cleanup)
        run_command "$@"
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        print_error "Unknown command: ${1:-}"
        echo ""
        show_help
        exit 1
        ;;
esac
