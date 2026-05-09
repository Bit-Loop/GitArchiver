#!/bin/bash

# Compatibility wrapper for starting the GitHub Archiver API service.

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

WEB_PORT="${WEB_PORT:-3000}"
LOG_DIR="${LOG_DIR:-logs}"
LOG_FILE="$LOG_DIR/web_service.log"
PID_FILE="${PID_FILE:-web_service.pid}"

mkdir -p "$LOG_DIR"

echo -e "${BLUE}Starting GitHub Archiver Web Server${NC}"
echo -e "${YELLOW}Web Port: $WEB_PORT${NC}"

export WEB_PORT

echo -e "${BLUE}Building web server binary...${NC}"
cargo build --release --bin web_server

echo -e "${GREEN}Starting service in background...${NC}"
nohup ./target/release/web_server >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"

echo -e "${GREEN}Service started${NC}"
echo -e "${BLUE}PID: $(cat "$PID_FILE")${NC}"
echo -e "${BLUE}Logs: tail -f $LOG_FILE${NC}"
echo -e "${BLUE}Web Interface: http://localhost:$WEB_PORT${NC}"
echo ""
echo "To stop the service, run: kill \$(cat $PID_FILE)"
