#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# ██████╗ ██╗████████╗██╗  ██╗██╗   ██╗██████╗      █████╗ ██████╗  ██████╗██╗  ██╗██╗██╗   ██╗███████╗██████╗ 
# ██╔════╝ ██║╚══██╔══╝██║  ██║██║   ██║██╔══██╗    ██╔══██╗██╔══██╗██╔════╝██║  ██║██║██║   ██║██╔════╝██╔══██╗
# ██║  ███╗██║   ██║   ███████║██║   ██║██████╔╝    ███████║██████╔╝██║     ███████║██║██║   ██║█████╗  ██████╔╝
# ██║   ██║██║   ██║   ██╔══██║██║   ██║██╔══██╗    ██╔══██║██╔══██╗██║     ██╔══██║██║╚██╗ ██╔╝██╔══╝  ██╔══██╗
# ╚██████╔╝██║   ██║   ██║  ██║╚██████╔╝██████╔╝    ██║  ██║██║  ██║╚██████╗██║  ██║██║ ╚████╔╝ ███████╗██║  ██║
#  ╚═════╝ ╚═╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ╚═════╝     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═╝
# ═══════════════════════════════════════════════════════════════════════════════
# 🚀 Professional Interactive Setup & Management System
# Version: 3.0.0 | Rust Edition
# Author: GitHub Copilot Advanced AI System
# ═══════════════════════════════════════════════════════════════════════════════

set -e  # Exit on any error

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                         🎨 ANSI STYLING SYSTEM                              │
# └─────────────────────────────────────────────────────────────────────────────┘
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly PURPLE='\033[0;35m'
readonly CYAN='\033[0;36m'
readonly WHITE='\033[1;37m'
readonly BOLD='\033[1m'
readonly DIM='\033[2m'
readonly UNDERLINE='\033[4m'
readonly BLINK='\033[5m'
readonly REVERSE='\033[7m'
readonly STRIKE='\033[9m'
readonly NC='\033[0m'

# Enhanced colors
readonly BRIGHT_RED='\033[1;31m'
readonly BRIGHT_GREEN='\033[1;32m'
readonly BRIGHT_YELLOW='\033[1;33m'
readonly BRIGHT_BLUE='\033[1;34m'
readonly BRIGHT_PURPLE='\033[1;35m'
readonly BRIGHT_CYAN='\033[1;36m'
readonly BRIGHT_WHITE='\033[1;37m'

# Background colors
readonly BG_RED='\033[41m'
readonly BG_GREEN='\033[42m'
readonly BG_YELLOW='\033[43m'
readonly BG_BLUE='\033[44m'
readonly BG_PURPLE='\033[45m'
readonly BG_CYAN='\033[46m'
readonly BG_WHITE='\033[47m'

# Special effects
readonly RAINBOW='\033[38;5;196m\033[38;5;208m\033[38;5;226m\033[38;5;46m\033[38;5;21m\033[38;5;93m\033[38;5;201m'

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                      🎭 ANIMATION & UI FUNCTIONS                            │
# └─────────────────────────────────────────────────────────────────────────────┘

# Advanced spinner with multiple styles
spinner() {
    local pid=$1
    local style=${2:-"dots"}
    local delay=0.1
    local msg=${3:-"Working"}
    
    case $style in
        "dots")
            local spinstr='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
            ;;
        "line")
            local spinstr='|/-\'
            ;;
        "arrow")
            local spinstr='▹▸▹'
            ;;
        "bouncing")
            local spinstr='⠁⠂⠄⡀⢀⠠⠐⠈'
            ;;
        "pulsing")
            local spinstr='●○◐◑◒◓'
            ;;
    esac
    
    while kill -0 "$pid" 2>/dev/null; do
        for (( i=0; i<${#spinstr}; i++ )); do
            printf "\r${BRIGHT_CYAN}${BOLD}[${spinstr:$i:1}] ${msg}...${NC}"
            sleep $delay
        done
    done
    printf "\r${GREEN}${BOLD}[✓] ${msg} Complete!${NC}\n"
}

# Enhanced progress bar with percentage and ETA
progress_bar() {
    local current=$1
    local total=$2
    local width=${3:-50}
    local label=${4:-"Progress"}
    
    local percentage=$((current * 100 / total))
    local filled=$((current * width / total))
    local empty=$((width - filled))
    
    printf "\r${BOLD}${label}: ${BRIGHT_CYAN}["
    printf "%*s" $filled | tr ' ' '█'
    printf "%*s" $empty | tr ' ' '░'
    printf "] ${percentage}%%${NC}"
    
    if [ $current -eq $total ]; then
        printf "\n"
    fi
}

# Animated typewriter effect
typewriter() {
    local text="$1"
    local delay=${2:-0.03}
    
    for (( i=0; i<${#text}; i++ )); do
        printf "${text:$i:1}"
        sleep $delay
    done
    printf "\n"
}

# Flashing text effect
flash_text() {
    local text="$1"
    local color="$2"
    local times=${3:-3}
    
    for ((i=1; i<=times; i++)); do
        printf "\r${color}${BOLD}${BLINK}${text}${NC}"
        sleep 0.5
        printf "\r${DIM}${text}${NC}"
        sleep 0.3
    done
    printf "\r${color}${BOLD}${text}${NC}\n"
}

# Gradient text effect
gradient_text() {
    local text="$1"
    local colors=("${BRIGHT_RED}" "${BRIGHT_YELLOW}" "${BRIGHT_GREEN}" "${BRIGHT_CYAN}" "${BRIGHT_BLUE}" "${BRIGHT_PURPLE}")
    
    for (( i=0; i<${#text}; i++ )); do
        local color_index=$((i % ${#colors[@]}))
        printf "${colors[$color_index]}${text:$i:1}${NC}"
    done
    printf "\n"
}

# Box drawing functions
draw_box() {
    local width=$1
    local height=$2
    local title="$3"
    local color="${4:-$CYAN}"
    
    # Top border
    printf "${color}╭"
    printf "─%.0s" $(seq 1 $((width-2)))
    printf "╮${NC}\n"
    
    # Title
    if [ -n "$title" ]; then
        local padding=$(((width - ${#title} - 2) / 2))
        printf "${color}│${NC}"
        printf " %.0s" $(seq 1 $padding)
        printf "${BOLD}${WHITE}%s${NC}" "$title"
        printf " %.0s" $(seq 1 $((width - ${#title} - padding - 2)))
        printf "${color}│${NC}\n"
        
        # Separator
        printf "${color}├"
        printf "─%.0s" $(seq 1 $((width-2)))
        printf "┤${NC}\n"
    fi
    
    # Empty lines
    for ((i=1; i<=height-3; i++)); do
        printf "${color}│${NC}"
        printf " %.0s" $(seq 1 $((width-2)))
        printf "${color}│${NC}\n"
    done
    
    # Bottom border
    printf "${color}╰"
    printf "─%.0s" $(seq 1 $((width-2)))
    printf "╯${NC}\n"
}

# System information display
show_system_info() {
    echo -e "${BRIGHT_BLUE}${BOLD}┌─────────────────────────────────────────────┐"
    echo -e "│           📊 SYSTEM INFORMATION              │"
    echo -e "└─────────────────────────────────────────────┘${NC}"
    echo
    echo -e "${CYAN}🖥️  OS:${NC} $(lsb_release -d 2>/dev/null | cut -f2 || uname -s)"
    echo -e "${CYAN}🔧 Kernel:${NC} $(uname -r)"
    echo -e "${CYAN}💾 Memory:${NC} $(free -h | awk 'NR==2{printf "%.1f/%.1fGB (%.2f%%)", $3/1024/1024, $2/1024/1024, $3*100/$2}')"
    echo -e "${CYAN}💽 Disk:${NC} $(df -h / | awk 'NR==2{printf "%s/%s (%s)", $3, $2, $5}')"
    echo -e "${CYAN}⚡ CPU:${NC} $(nproc) cores"
    echo -e "${CYAN}🌐 Network:${NC} $(hostname -I | awk '{print $1}')"
    echo
}

# Clear screen with animation
clear_screen() {
    printf "\033[2J\033[H"
    sleep 0.1
}
    for ((i=0; i<=steps; i++)); do
        echo -ne "${GREEN}█"
        sleep $step_duration
    done
    echo -e "${CYAN}] ${GREEN}✓${NC}"
}

flash_text() {
    local text="$1"
    local color="$2"
    for i in {1..3}; do
        echo -ne "\r${color}${BOLD}${text}${NC}"
        sleep 0.3
        echo -ne "\r$(printf '%*s' ${#text} '')"
        sleep 0.2
    done
    echo -ne "\r${color}${BOLD}${text}${NC}\n"
}

typewriter() {
    local text="$1"
    local delay="${2:-0.05}"
    for ((i=0; i<${#text}; i++)); do
        echo -n "${text:$i:1}"
        sleep $delay
    done
    echo
}

banner() {
    clear
    echo -e "${CYAN}${BOLD}"
    echo "╔══════════════════════════════════════════════════════════════════════════════╗"
    echo "║                                                                              ║"
    echo "║            ${WHITE}🚀 GITHUB ARCHIVE SCRAPER v2.0.0 🚀${CYAN}                         ║"
    echo "║                                                                              ║"
    echo "║                   ${YELLOW}Professional Setup & Management Tool${CYAN}                   ║"
    echo "║                                                                              ║"
    echo "╚══════════════════════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
    echo
}

separator() {
    echo -e "${DIM}${CYAN}$(printf '─%.0s' {1..80})${NC}"
}

section_header() {
    local title="$1"
    echo
    echo -e "${PURPLE}${BOLD}▶ ${title}${NC}"
    separator
}

success_msg() {
    echo -e "${GREEN}${BOLD}✓ $1${NC}"
}

error_msg() {
    echo -e "${RED}${BOLD}✗ $1${NC}"
}

warning_msg() {
    echo -e "${YELLOW}${BOLD}⚠ $1${NC}"
}

info_msg() {
    echo -e "${BLUE}${BOLD}ℹ $1${NC}"
}

prompt_user() {
    local prompt="$1"
    local default="$2"
    local response
    
    if [ -n "$default" ]; then
        echo -ne "${YELLOW}${BOLD}? ${prompt} ${DIM}(${default}):${NC} "
    else
        echo -ne "${YELLOW}${BOLD}? ${prompt}:${NC} "
    fi
    
    read -r response
    echo "${response:-$default}"
}

confirm() {
    local prompt="$1"
    local response
    
    while true; do
        echo -ne "${YELLOW}${BOLD}? ${prompt} ${DIM}(y/N):${NC} "
        read -r response
        case $response in
            [Yy]* ) return 0;;
            [Nn]* ) return 1;;
            "" ) return 1;;
            * ) echo -e "${RED}Please answer yes (y) or no (n).${NC}";;
        esac
    done
}

loading() {
    local message="$1"
    local duration="${2:-2}"
    
    echo -ne "${BLUE}${message}${NC}"
    progress_bar $duration
}

# =============================================================================
# System Information and Checks
# =============================================================================

detect_system() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if [ -f /etc/debian_version ]; then
            echo "ubuntu"
        elif [ -f /etc/redhat-release ]; then
            echo "rhel"
        else
            echo "linux"
        fi
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    else
        echo "unknown"
    fi
}

check_requirements() {
    section_header "System Requirements Check"
    
    local all_good=true
    
    # Check OS
    local os=$(detect_system)
    case $os in
        ubuntu)
            success_msg "Operating System: Ubuntu/Debian detected"
            ;;
        rhel)
            success_msg "Operating System: RHEL/CentOS detected"
            ;;
        macos)
            success_msg "Operating System: macOS detected"
            ;;
        *)
            error_msg "Operating System: Unsupported OS detected"
            all_good=false
            ;;
    esac
    
    # Check if running as non-root
    if [ "$EUID" -eq 0 ]; then
        warning_msg "Running as root - some operations may need adjustment"
    else
        success_msg "Running as non-root user"
    fi
    
    # Check available tools
    local tools=("curl" "git" "docker" "docker-compose")
    for tool in "${tools[@]}"; do
        if command -v "$tool" &> /dev/null; then
            success_msg "$tool: Available"
        else
            error_msg "$tool: Not found"
            all_good=false
        fi
    done
    
    # Check Rust
    if command -v rustc &> /dev/null; then
        local rust_version=$(rustc --version | cut -d' ' -f2)
        success_msg "Rust: $rust_version"
    else
        warning_msg "Rust: Not installed (will be installed if needed)"
    fi
    
    # Check Python
    if command -v python3 &> /dev/null; then
        local python_version=$(python3 --version | cut -d' ' -f2)
        success_msg "Python: $python_version"
    else
        error_msg "Python3: Not found"
        all_good=false
    fi
    
    # Check PostgreSQL
    if command -v psql &> /dev/null; then
        success_msg "PostgreSQL: Client available"
    else
        warning_msg "PostgreSQL: Client not found (will check for Docker)"
    fi
    
    if [ "$all_good" = false ]; then
        echo
        error_msg "Some requirements are missing. Continue anyway?"
        if ! confirm "Proceed with installation"; then
            exit 1
        fi
    fi
    
    echo
    success_msg "Requirements check completed"
    sleep 1
}

# =============================================================================
# Installation Functions
# =============================================================================

install_rust() {
    section_header "Rust Installation"
    
    if command -v rustc &> /dev/null; then
        info_msg "Rust is already installed"
        return 0
    fi
    
    echo -e "${YELLOW}Installing Rust via rustup...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    
    success_msg "Rust installed successfully"
}

setup_database() {
    section_header "Database Setup"
    
    echo -e "${BLUE}Choose database setup method:${NC}"
    echo "1. Use existing PostgreSQL instance"
    echo "2. Start PostgreSQL with Docker"
    echo "3. Skip database setup"
    
    local choice=$(prompt_user "Enter choice (1-3)" "2")
    
    case $choice in
        1)
            setup_existing_postgres
            ;;
        2)
            setup_docker_postgres
            ;;
        3)
            warning_msg "Skipping database setup"
            ;;
        *)
            error_msg "Invalid choice, skipping database setup"
            ;;
    esac
}

setup_existing_postgres() {
    info_msg "Using existing PostgreSQL instance"
    
    local db_host=$(prompt_user "Database host" "localhost")
    local db_port=$(prompt_user "Database port" "5432")
    local db_name=$(prompt_user "Database name" "github_archiver")
    local db_user=$(prompt_user "Database user" "postgres")
    local db_pass=$(prompt_user "Database password" "")
    
    # Test connection
    echo -e "${BLUE}Testing database connection...${NC}"
    if PGPASSWORD="$db_pass" psql -h "$db_host" -p "$db_port" -U "$db_user" -d "$db_name" -c "SELECT 1;" &>/dev/null; then
        success_msg "Database connection successful"
        
        # Save configuration
        cat > .env << EOF
DATABASE_HOST=$db_host
DATABASE_PORT=$db_port
DATABASE_NAME=$db_name
DATABASE_USER=$db_user
DATABASE_PASSWORD=$db_pass
EOF
        success_msg "Database configuration saved to .env"
    else
        error_msg "Database connection failed"
        if confirm "Continue anyway"; then
            return 0
        else
            exit 1
        fi
    fi
}

setup_docker_postgres() {
    info_msg "Setting up PostgreSQL with Docker"
    
    if ! command -v docker &> /dev/null; then
        error_msg "Docker not found. Please install Docker first."
        return 1
    fi
    
    local db_password=$(prompt_user "Set PostgreSQL password" "github_archiver_pass")
    
    # Create docker-compose.yml
    cat > docker-compose.yml << EOF
version: '3.8'
services:
  postgres:
    image: postgres:16
    container_name: github_archiver_db
    environment:
      POSTGRES_DB: github_archiver
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: $db_password
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    restart: unless-stopped

volumes:
  postgres_data:
EOF

    loading "Starting PostgreSQL container" 3
    docker-compose up -d postgres
    
    # Wait for database to be ready
    echo -e "${BLUE}Waiting for database to be ready...${NC}"
    for i in {1..30}; do
        if docker exec github_archiver_db pg_isready -U postgres &>/dev/null; then
            success_msg "PostgreSQL is ready"
            break
        fi
        sleep 1
        echo -n "."
    done
    
    # Save configuration
    cat > .env << EOF
DATABASE_HOST=localhost
DATABASE_PORT=5432
DATABASE_NAME=github_archiver
DATABASE_USER=postgres
DATABASE_PASSWORD=$db_password
EOF
    
    success_msg "PostgreSQL setup completed"
}

build_rust_application() {
    section_header "Building Rust Application"
    
    if [ ! -d "rust_github_archiver" ]; then
        error_msg "Rust project directory not found"
        return 1
    fi
    
    cd rust_github_archiver
    
    loading "Downloading dependencies" 2
    cargo fetch
    
    loading "Building application (this may take a while)" 5
    cargo build --release
    
    if [ $? -eq 0 ]; then
        success_msg "Rust application built successfully"
        success_msg "Binary location: target/release/github_archiver"
    else
        error_msg "Build failed"
        return 1
    fi
    
    cd ..
}

configure_environment() {
    section_header "Environment Configuration"
    
    # Create .env if it doesn't exist
    if [ ! -f .env ]; then
        info_msg "Creating environment configuration..."
        
        local github_token=$(prompt_user "GitHub Personal Access Token (optional)" "")
        local jwt_secret=$(prompt_user "JWT Secret Key" "github-archive-scraper-jwt-secret-$(date +%s)")
        local admin_password=$(prompt_user "Admin Password" "admin123")
        local web_port=$(prompt_user "Web Server Port" "8081")
        
        cat >> .env << EOF

# GitHub Configuration
GITHUB_TOKEN=$github_token

# Security Configuration
JWT_SECRET=$jwt_secret
ADMIN_PASSWORD=$admin_password

# Web Configuration
WEB_HOST=0.0.0.0
WEB_PORT=$web_port
WEB_DEBUG=false

# Logging Configuration
RUST_LOG=info

# Download Configuration
DOWNLOAD_DIR=./gharchive_data
EOF
        
        success_msg "Environment configuration created"
    else
        info_msg "Environment configuration already exists"
    fi
    
    # Create directories
    mkdir -p gharchive_data logs static
    success_msg "Created required directories"
}

# =============================================================================
# Service Management
# =============================================================================

start_services() {
    section_header "Starting Services"
    
    # Start database if using Docker
    if [ -f docker-compose.yml ]; then
        loading "Starting database service" 2
        docker-compose up -d postgres
        success_msg "Database service started"
    fi
    
    # Start Rust application
    if [ -f rust_github_archiver/target/release/github_archiver ]; then
        info_msg "Starting Rust application..."
        
        # Load environment variables
        if [ -f .env ]; then
            source .env
        fi
        
        # Start in background
        cd rust_github_archiver
        nohup ./target/release/github_archiver > ../logs/app.log 2>&1 &
        local app_pid=$!
        echo $app_pid > ../app.pid
        cd ..
        
        # Wait a moment and check if it's still running
        sleep 2
        if ps -p $app_pid > /dev/null; then
            success_msg "Application started successfully (PID: $app_pid)"
            info_msg "Logs: tail -f logs/app.log"
            info_msg "Web interface: http://localhost:${WEB_PORT:-8081}"
        else
            error_msg "Application failed to start"
            return 1
        fi
    else
        warning_msg "Application binary not found. Please build first."
    fi
}

stop_services() {
    section_header "Stopping Services"
    
    # Stop application
    if [ -f app.pid ]; then
        local app_pid=$(cat app.pid)
        if ps -p $app_pid > /dev/null; then
            loading "Stopping application" 1
            kill $app_pid
            rm -f app.pid
            success_msg "Application stopped"
        else
            info_msg "Application not running"
        fi
    else
        info_msg "No application PID file found"
    fi
    
    # Stop database if using Docker
    if [ -f docker-compose.yml ]; then
        loading "Stopping database service" 1
        docker-compose down
        success_msg "Database service stopped"
    fi
}

show_status() {
    section_header "Service Status"
    
    # Check application
    if [ -f app.pid ]; then
        local app_pid=$(cat app.pid)
        if ps -p $app_pid > /dev/null; then
            success_msg "Application: Running (PID: $app_pid)"
            
            # Check if web server is responding
            local web_port=${WEB_PORT:-8081}
            if curl -s "http://localhost:$web_port/health" > /dev/null; then
                success_msg "Web Server: Responding on port $web_port"
            else
                warning_msg "Web Server: Not responding on port $web_port"
            fi
        else
            error_msg "Application: Not running (stale PID file)"
            rm -f app.pid
        fi
    else
        error_msg "Application: Not running"
    fi
    
    # Check database
    if [ -f docker-compose.yml ]; then
        if docker-compose ps postgres | grep -q "Up"; then
            success_msg "Database: Running (Docker)"
        else
            error_msg "Database: Not running (Docker)"
        fi
    else
        info_msg "Database: External (not managed by this script)"
    fi
    
    # Show resource usage
    echo
    info_msg "System Resources:"
    echo -e "${DIM}CPU Usage: $(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | awk -F'%' '{print $1}')%${NC}"
    echo -e "${DIM}Memory Usage: $(free | grep Mem | awk '{printf("%.1f%%", $3/$2 * 100.0)}')${NC}"
    echo -e "${DIM}Disk Usage: $(df -h . | awk 'NR==2{printf "%s", $5}')${NC}"
}

view_logs() {
    section_header "Application Logs"
    
    if [ -f logs/app.log ]; then
        echo -e "${BLUE}Showing last 50 lines of application logs:${NC}"
        echo -e "${DIM}$(separator)${NC}"
        tail -n 50 logs/app.log
        echo -e "${DIM}$(separator)${NC}"
        echo
        if confirm "Follow logs in real-time"; then
            tail -f logs/app.log
        fi
    else
        warning_msg "No log file found"
    fi
}

# =============================================================================
# Interactive Menu
# =============================================================================

show_menu() {
    echo
    echo -e "${BOLD}${WHITE}Available Actions:${NC}"
    echo
    echo -e "${CYAN}1.${NC} 🔧 Full Setup (Install + Configure + Build + Start)"
    echo -e "${CYAN}2.${NC} 🦀 Install Rust"
    echo -e "${CYAN}3.${NC} 🗄️  Setup Database"
    echo -e "${CYAN}4.${NC} 🔨 Build Application"
    echo -e "${CYAN}5.${NC} ⚙️  Configure Environment"
    echo -e "${CYAN}6.${NC} 🚀 Start Services"
    echo -e "${CYAN}7.${NC} 🛑 Stop Services"
    echo -e "${CYAN}8.${NC} 📊 Show Status"
    echo -e "${CYAN}9.${NC} 📝 View Logs"
    echo -e "${CYAN}0.${NC} 🚪 Exit"
    echo
}

full_setup() {
    flash_text "🚀 STARTING FULL SETUP" "$GREEN"
    
    check_requirements
    install_rust
    setup_database
    configure_environment
    build_rust_application
    start_services
    
    echo
    flash_text "🎉 SETUP COMPLETED SUCCESSFULLY!" "$GREEN"
    echo
    show_status
}

# =============================================================================
# Main Application Loop
# =============================================================================

main() {
    banner
    
    typewriter "Welcome to the GitHub Archive Scraper setup script!" 0.03
    echo
    info_msg "This script will help you install, configure, and manage the application."
    
    # If arguments provided, run specific function
    case "${1:-}" in
        "install")
            full_setup
            ;;
        "start")
            start_services
            ;;
        "stop")
            stop_services
            ;;
        "status")
            show_status
            ;;
        "logs")
            view_logs
            ;;
        *)
            # Interactive mode
            while true; do
                show_menu
                local choice=$(prompt_user "Choose an action" "1")
                
                case $choice in
                    1)
                        full_setup
                        ;;
                    2)
                        install_rust
                        ;;
                    3)
                        setup_database
                        ;;
                    4)
                        build_rust_application
                        ;;
                    5)
                        configure_environment
                        ;;
                    6)
                        start_services
                        ;;
                    7)
                        stop_services
                        ;;
                    8)
                        show_status
                        ;;
                    9)
                        view_logs
                        ;;
                    0)
                        echo
                        flash_text "👋 Goodbye!" "$CYAN"
                        exit 0
                        ;;
                    *)
                        error_msg "Invalid choice. Please try again."
                        sleep 1
                        ;;
                esac
                
                echo
                echo -e "${DIM}Press any key to continue...${NC}"
                read -n 1 -s
                banner
            done
            ;;
    esac
}

# =============================================================================
# Script Entry Point
# =============================================================================

# Trap to handle script interruption
trap 'echo -e "\n${YELLOW}Script interrupted.${NC}"; exit 1' INT

# Check if script is being sourced or executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
