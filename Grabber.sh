#!/bin/bash

# GitHub Archive Scraper - Enhanced Bash Wrapper
# This script provides easy access to the Python scraper with common operations

set -e  # Exit on any error

SCRIPT_DIR="$(dirname "$0")"
PYTHON_SCRIPT="$SCRIPT_DIR/gharchive_scraper.py"
VENV_DIR="$SCRIPT_DIR/venv"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

# Function to check if virtual environment exists and activate it
setup_venv() {
    if [ ! -d "$VENV_DIR" ]; then
        log_info "Creating Python virtual environment..."
        python3 -m venv "$VENV_DIR"
    fi
    
    source "$VENV_DIR/bin/activate"
    
    # Install/upgrade requirements
    if [ -f "$SCRIPT_DIR/requirements.txt" ]; then
        log_info "Installing/updating Python dependencies..."
        pip install -r "$SCRIPT_DIR/requirements.txt" > /dev/null 2>&1
    fi
}

# Function to check prerequisites
check_prerequisites() {
    # Check if Python 3 is available
    if ! command -v python3 &> /dev/null; then
        log_error "Python 3 is required but not installed"
        exit 1
    fi
    
    # Check if PostgreSQL is available (optional check)
    if ! command -v psql &> /dev/null; then
        log_warn "PostgreSQL client not found. Make sure PostgreSQL is installed and accessible."
    fi
}

# Function to show usage
show_usage() {
    echo "GitHub Archive Scraper - Enhanced Python Version"
    echo "================================================"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Common Operations:"
    echo "  $0                          # Process yesterday's data (default)"
    echo "  $0 --catch-up-from-2015     # Catch up all data from 2015 onward"
    echo "  $0 --mode discover          # Discover available archive files"
    echo "  $0 --mode missing           # Find missing data ranges"
    echo "  $0 --date 2015-01-01        # Process specific date"
    echo "  $0 --start-date 2015-01-01 --end-date 2015-01-31  # Process date range"
    echo ""
    echo "Search and Export:"
    echo "  $0 --mode search --search-query '{\"type\": \"PushEvent\", \"repo_name\": \"torvalds/linux\"}'"
    echo "  $0 --mode export --export-repo torvalds/linux"
    echo ""
    echo "For full options, run: $0 --help"
    echo ""
    echo "Cron Job Example (daily at 2 AM):"
    echo "  0 2 * * * $0 >/dev/null 2>&1"
}

# Main execution
main() {
    # Check prerequisites
    check_prerequisites
    
    # Setup virtual environment
    setup_venv
    
    # If no arguments provided, show usage
    if [ $# -eq 0 ]; then
        log_info "No arguments provided, running default operation (yesterday's data)"
        python3 "$PYTHON_SCRIPT"
    elif [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
        show_usage
        echo ""
        echo "Python script help:"
        python3 "$PYTHON_SCRIPT" --help
    else
        # Pass all arguments to the Python script
        log_info "Running GitHub Archive Scraper with arguments: $*"
        python3 "$PYTHON_SCRIPT" "$@"
        
        if [ $? -eq 0 ]; then
            log_success "Operation completed successfully"
        else
            log_error "Operation failed"
            exit 1
        fi
    fi
}

# Run main function
main "$@"