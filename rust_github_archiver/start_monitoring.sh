#!/bin/bash

# GitHub Archiver - Monitoring Dashboard Startup Script
# This script starts the Rust API server with monitoring capabilities

set -e

echo "🚀 Starting GitHub Archiver Monitoring System..."
echo "================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Error: Cargo is not installed${NC}"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo -e "${GREEN}✓${NC} Rust/Cargo found"

# Check if PostgreSQL is running
if ! pg_isready &> /dev/null; then
    echo -e "${YELLOW}⚠${NC} Warning: PostgreSQL might not be running"
    echo "Starting PostgreSQL..."
    sudo systemctl start postgresql || true
fi

echo -e "${GREEN}✓${NC} PostgreSQL ready"

# Check if database exists
DB_NAME="github_archiver"
if psql -lqt | cut -d \| -f 1 | grep -qw "$DB_NAME"; then
    echo -e "${GREEN}✓${NC} Database '$DB_NAME' exists"
else
    echo -e "${YELLOW}⚠${NC} Database '$DB_NAME' not found, creating..."
    createdb "$DB_NAME" || true
fi

# Set environment variables
export DATABASE_URL="postgresql://postgres:postgres@localhost/$DB_NAME"
export RUST_LOG="info,github_archiver=debug"
export SERVER_PORT="8081"
export ENABLE_CORS="true"

echo ""
echo -e "${BLUE}Environment Configuration:${NC}"
echo "  DATABASE_URL: $DATABASE_URL"
echo "  SERVER_PORT: $SERVER_PORT"
echo "  RUST_LOG: $RUST_LOG"
echo ""

# Build the project
echo -e "${BLUE}📦 Building project...${NC}"
cargo build --release

if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Build successful"

# Kill any existing server on the port
if lsof -Pi :$SERVER_PORT -sTCP:LISTEN -t >/dev/null ; then
    echo -e "${YELLOW}⚠${NC} Port $SERVER_PORT is in use, killing existing process..."
    lsof -ti:$SERVER_PORT | xargs kill -9 2>/dev/null || true
    sleep 2
fi

# Start the server
echo ""
echo -e "${GREEN}🌟 Starting server on port $SERVER_PORT...${NC}"
echo "================================================="
echo ""
echo -e "${BLUE}📊 Monitoring Dashboard:${NC}"
echo "   http://localhost:$SERVER_PORT/monitoring-dashboard.html"
echo ""
echo -e "${BLUE}🔌 API Endpoints:${NC}"
echo "   Overview:    http://localhost:$SERVER_PORT/api/monitoring/overview"
echo "   Trends:      http://localhost:$SERVER_PORT/api/monitoring/trends?period=24h"
echo "   Logs:        http://localhost:$SERVER_PORT/api/monitoring/logs"
echo "   Metrics:     http://localhost:$SERVER_PORT/api/monitoring/metrics"
echo "   WebSocket:   ws://localhost:$SERVER_PORT/api/monitoring/ws"
echo ""
echo -e "${BLUE}🔑 Authentication:${NC}"
echo "   Use JWT token for protected endpoints"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop the server${NC}"
echo "================================================="
echo ""

# Run the server
cargo run --release

# Cleanup on exit
trap "echo -e '\n${YELLOW}Shutting down server...${NC}'; exit 0" INT TERM
