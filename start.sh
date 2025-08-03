#!/bin/bash

# GitHub Archiver - Main Launcher Script
# This script navigates to the rust project and runs the setup script

set -e

# Color definitions
readonly GREEN='\033[0;32m'
readonly CYAN='\033[0;36m'
readonly BOLD='\033[1m'
readonly NC='\033[0m'

echo -e "${CYAN}${BOLD}🚀 GitHub Archiver Setup Launcher${NC}"
echo -e "${GREEN}Navigating to rust project directory...${NC}"
echo

# Check if rust directory exists
if [[ ! -d "rust_github_archiver" ]]; then
    echo -e "${RED}[✗] Error: rust_github_archiver directory not found${NC}"
    exit 1
fi

# Navigate to rust directory and run setup
cd rust_github_archiver
exec ./setup.sh "$@"
