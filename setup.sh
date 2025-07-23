#!/bin/bash

# GitHub Archive Scraper Setup Script
# Enhanced setup with comprehensive configuration and testing

set -e

SCRIPT_DIR="$(dirname "$0")"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}� [INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}⚠️  [WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}❌ [ERROR]${NC} $1"
}

log_success() {
    echo -e "${GREEN}✅ [SUCCESS]${NC} $1"
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

echo "🚀 GitHub Archive Scraper Setup"
echo "================================="
echo ""

# Check if PostgreSQL is installed
if ! command_exists psql; then
    log_info "Installing PostgreSQL..."
    sudo apt update
    sudo apt install -y postgresql postgresql-contrib python3-dev libpq-dev
    sudo systemctl start postgresql
    sudo systemctl enable postgresql
    log_success "PostgreSQL installed and started"
else
    log_success "PostgreSQL already installed"
fi

# Setup database
log_info "Setting up database..."
sudo -u postgres psql -c "CREATE DATABASE gharchive;" 2>/dev/null || log_info "Database 'gharchive' already exists"
sudo -u postgres psql -c "CREATE USER gharchive WITH ENCRYPTED PASSWORD 'gharchive_password';" 2>/dev/null || log_info "User 'gharchive' already exists"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE gharchive TO gharchive;"
sudo -u postgres psql -c "ALTER USER gharchive CREATEDB;"
log_success "Database setup complete"

# Setup Python virtual environment
log_info "Setting up Python virtual environment..."
if [ ! -d "venv" ]; then
    python3 -m venv venv
    log_success "Virtual environment created"
else
    log_info "Virtual environment already exists"
fi

source venv/bin/activate

# Install Python dependencies
log_info "Installing Python dependencies..."
pip install --upgrade pip
pip install -r requirements.txt
log_success "Python dependencies installed"

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    log_info "Creating configuration file..."
    cat > .env << 'EOF'
# Database Configuration
DB_HOST=localhost
DB_PORT=5432
DB_NAME=gharchive
DB_USER=gharchive
DB_PASSWORD=gharchive_password

# GitHub API Token (optional, but recommended for rate limiting)
# Get one from: https://github.com/settings/tokens
# GITHUB_TOKEN=your_github_token_here

# Performance Tuning
MAX_CONCURRENT=10
BATCH_SIZE=1000
CHUNK_SIZE=8192

# Logging
LOG_LEVEL=INFO
LOG_FILE=gharchive_scraper.log

# Cron Mode
ENABLE_CRON_MODE=true
CRON_LOCK_FILE=/tmp/gharchive_scraper.lock
EOF
    log_success "Configuration file (.env) created"
else
    log_info "Configuration file (.env) already exists"
fi

# Make scripts executable
chmod +x Grabber.sh
chmod +x gharchive_scraper.py
chmod +x setup.sh

log_success "Setup complete!"
echo ""
echo "📖 Usage Examples:"
echo ""
echo "  # Discover available files (Ruby script equivalent):"
echo "  ./Grabber.sh --mode discover"
echo ""
echo "  # Catch up all data from 2015 onward (as requested):"
echo "  ./Grabber.sh --catch-up-from-2015"
echo ""
echo "  # Find missing data ranges:"
echo "  ./Grabber.sh --mode missing"
echo ""
echo "  # Process yesterday's data:"
echo "  ./Grabber.sh"
echo ""
echo "  # Process specific date:"
echo "  ./Grabber.sh --date 2015-01-01"
echo ""
echo "  # Process date range:"
echo "  ./Grabber.sh --start-date 2015-01-01 --end-date 2015-01-31"
echo ""
echo "  # Search events:"
echo "  ./Grabber.sh --mode search --search-query '{\"type\": \"PushEvent\", \"repo_name\": \"torvalds/linux\"}'"
echo ""
echo "  # Export repository data:"
echo "  ./Grabber.sh --mode export --export-repo torvalds/linux"
echo ""
echo "🕒 For daily cron job, add this line to your crontab (crontab -e):"
echo "0 2 * * * $PWD/Grabber.sh >/dev/null 2>&1"
echo ""
echo "⚙️  Next steps:"
echo "1. Edit .env file to add your GitHub token"
echo "2. Test with: ./Grabber.sh --mode discover"
echo "3. Start scraping: ./Grabber.sh --catch-up-from-2015"
