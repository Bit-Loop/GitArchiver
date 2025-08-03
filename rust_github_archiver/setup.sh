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

# Enhanced progress bar with percentage
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

# Clear screen with animation
clear_screen() {
    printf "\033[2J\033[H"
    sleep 0.1
}

# System information display
show_system_info() {
    echo -e "${BRIGHT_BLUE}${BOLD}┌─────────────────────────────────────────────┐"
    echo -e "│           📊 SYSTEM INFORMATION              │"
    echo -e "└─────────────────────────────────────────────┘${NC}"
    echo
    echo -e "${CYAN}🖥️  OS:${NC} $(lsb_release -d 2>/dev/null | cut -f2 || uname -s)"
    echo -e "${CYAN}🔧 Kernel:${NC} $(uname -r)"
    
    # Fixed memory calculation - get values in KB and convert properly
    local mem_info=$(free | awk 'NR==2{printf "%.1f/%.1fGB (%.1f%%)", $3/1024/1024, $2/1024/1024, $3*100/$2}' 2>/dev/null)
    echo -e "${CYAN}💾 Memory:${NC} ${mem_info:-"N/A"}"
    
    echo -e "${CYAN}💽 Disk:${NC} $(df -h / | awk 'NR==2{printf "%s/%s (%s)", $3, $2, $5}' 2>/dev/null || echo "N/A")"
    echo -e "${CYAN}⚡ CPU:${NC} $(nproc) cores"
    echo -e "${CYAN}🌐 Network:${NC} $(hostname -I | awk '{print $1}' 2>/dev/null || echo "localhost")"
    echo
    
    # Add enhanced service status information
    echo -e "${BRIGHT_BLUE}${BOLD}📊 Service Status${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    
    # Get external IP
    local external_ip=$(curl -s ifconfig.me 2>/dev/null || curl -s ipinfo.io/ip 2>/dev/null || echo "Unable to determine")
    
    if [[ -f service.pid ]]; then
        local pid=$(cat service.pid)
        if kill -0 $pid 2>/dev/null; then
            echo -e "${GREEN}${BOLD}Status:${NC} Running"
            echo -e "${GREEN}${BOLD}PID:${NC} $pid"
            echo -e "${GREEN}${BOLD}Port:${NC} 8081"
            echo -e "${GREEN}${BOLD}Local URL:${NC} http://127.0.0.1:8081"
            echo -e "${GREEN}${BOLD}External URL:${NC} http://${external_ip}:8081"
            
            # Check if port is actually open
            local port_status=$(ss -tln | grep ":8081 " >/dev/null 2>&1 && echo "Open" || echo "Closed")
            echo -e "${GREEN}${BOLD}Port Status:${NC} ${port_status}"
            
            # Memory usage
            local mem=$(ps -p $pid -o rss= 2>/dev/null | awk '{print $1/1024 " MB"}')
            echo -e "${GREEN}${BOLD}Memory:${NC} ${mem:-"N/A"}"
            
            # CPU usage
            local cpu=$(ps -p $pid -o %cpu= 2>/dev/null | awk '{print $1"%"}')
            echo -e "${GREEN}${BOLD}CPU:${NC} ${cpu:-"N/A"}"
            
            # Uptime
            local start_time=$(ps -p $pid -o lstart= 2>/dev/null)
            echo -e "${GREEN}${BOLD}Started:${NC} ${start_time:-"N/A"}"
            
            echo
            echo -e "${BRIGHT_CYAN}${BOLD}🌐 Network Information${NC}"
            echo -e "${CYAN}───────────────────────────────────────${NC}"
            echo -e "${BLUE}${BOLD}External IP:${NC} ${external_ip}"
            
            # Show open ports related to the service
            echo -e "${BLUE}${BOLD}Open Ports:${NC}"
            ss -tln | grep ":808[0-9]" | while read line; do
                local port=$(echo "$line" | awk '{print $4}' | sed 's/.*://')
                echo -e "  ${GREEN}→${NC} Port ${port}"
            done
            
            echo
            echo -e "${BRIGHT_PURPLE}${BOLD}🗄️ Database Information${NC}"
            echo -e "${PURPLE}───────────────────────────────────────${NC}"
            
            # Check if .env file exists and get database info
            if [[ -f .env ]]; then
                local db_url=$(grep "DATABASE_URL" .env 2>/dev/null | cut -d'=' -f2- | sed 's/"//g')
                if [[ -n "$db_url" ]]; then
                    local db_host=$(echo "$db_url" | sed -n 's/.*@\([^:]*\):.*/\1/p')
                    local db_port=$(echo "$db_url" | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
                    local db_name=$(echo "$db_url" | sed -n 's/.*\/\([^?]*\).*/\1/p')
                    
                    echo -e "${PURPLE}${BOLD}Database Host:${NC} ${db_host:-"localhost"}"
                    echo -e "${PURPLE}${BOLD}Database Port:${NC} ${db_port:-"5432"}"
                    echo -e "${PURPLE}${BOLD}Database Name:${NC} ${db_name:-"N/A"}"
                    
                    # Test database connection
                    local db_status="Unknown"
                    if command -v psql >/dev/null 2>&1; then
                        if psql "$db_url" -c "SELECT 1;" >/dev/null 2>&1; then
                            db_status="${GREEN}Connected${NC}"
                            
                            # Get database statistics
                            local db_size=$(psql "$db_url" -t -c "SELECT pg_size_pretty(pg_database_size(current_database()));" 2>/dev/null | xargs)
                            local table_count=$(psql "$db_url" -t -c "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public';" 2>/dev/null | xargs)
                            
                            echo -e "${PURPLE}${BOLD}Connection:${NC} ${db_status}"
                            echo -e "${PURPLE}${BOLD}Database Size:${NC} ${db_size:-"N/A"}"
                            echo -e "${PURPLE}${BOLD}Tables:${NC} ${table_count:-"N/A"}"
                        else
                            db_status="${RED}Connection Failed${NC}"
                            echo -e "${PURPLE}${BOLD}Connection:${NC} ${db_status}"
                        fi
                    else
                        echo -e "${PURPLE}${BOLD}Connection:${NC} ${YELLOW}psql not available${NC}"
                    fi
                else
                    echo -e "${YELLOW}No DATABASE_URL found in .env${NC}"
                fi
            else
                echo -e "${YELLOW}No .env file found${NC}"
            fi
            
            echo
            echo -e "${BRIGHT_YELLOW}${BOLD}📈 Service Health${NC}"
            echo -e "${YELLOW}───────────────────────────────────────${NC}"
            
            # Test API endpoint
            local api_status=$(curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8081/health 2>/dev/null || echo "000")
            if [[ "$api_status" == "200" ]]; then
                echo -e "${YELLOW}${BOLD}API Health:${NC} ${GREEN}Healthy (HTTP $api_status)${NC}"
            else
                echo -e "${YELLOW}${BOLD}API Health:${NC} ${RED}Unhealthy (HTTP $api_status)${NC}"
            fi
            
        else
            echo -e "${RED}${BOLD}Status:${NC} Stopped (stale PID file)"
            rm service.pid
        fi
    else
        echo -e "${RED}${BOLD}Status:${NC} Not running"
    fi
    echo
}

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                     🛠️  SYSTEM CHECK FUNCTIONS                              │
# └─────────────────────────────────────────────────────────────────────────────┘

check_command() {
    local cmd="$1"
    local name="$2"
    
    if command -v "$cmd" >/dev/null 2>&1; then
        echo -e "${GREEN}${BOLD}[✓]${NC} ${name} is installed"
        return 0
    else
        echo -e "${RED}${BOLD}[✗]${NC} ${name} is ${RED}not installed${NC}"
        return 1
    fi
}

detect_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        echo "$ID"
    elif [[ -f /etc/redhat-release ]]; then
        echo "rhel"
    elif [[ -f /etc/debian_version ]]; then
        echo "debian"
    else
        echo "unknown"
    fi
}

check_openssl() {
    echo -e "${YELLOW}${BOLD}🔐 Checking OpenSSL installation...${NC}"
    
    if command -v openssl >/dev/null 2>&1; then
        local openssl_version=$(openssl version | cut -d' ' -f2)
        echo -e "${GREEN}${BOLD}[✓]${NC} OpenSSL ${openssl_version} is installed"
        
        # Check for development headers
        if pkg-config --exists openssl 2>/dev/null || [[ -f /usr/include/openssl/ssl.h ]] || [[ -f /usr/local/include/openssl/ssl.h ]]; then
            echo -e "${GREEN}${BOLD}[✓]${NC} OpenSSL development headers are available"
            return 0
        else
            echo -e "${YELLOW}${BOLD}[!]${NC} OpenSSL development headers are missing"
            return 2
        fi
    else
        echo -e "${RED}${BOLD}[✗]${NC} OpenSSL is not installed"
        return 1
    fi
}

check_build_tools() {
    echo -e "${YELLOW}${BOLD}🔧 Checking build tools...${NC}"
    
    local missing_tools=()
    
    # Check GCC/Clang
    if ! command -v gcc >/dev/null 2>&1 && ! command -v clang >/dev/null 2>&1; then
        missing_tools+=("build-essential/gcc")
    else
        echo -e "${GREEN}${BOLD}[✓]${NC} C compiler is available"
    fi
    
    # Check pkg-config
    if ! command -v pkg-config >/dev/null 2>&1; then
        missing_tools+=("pkg-config")
    else
        echo -e "${GREEN}${BOLD}[✓]${NC} pkg-config is available"
    fi
    
    # Check make
    if ! command -v make >/dev/null 2>&1; then
        missing_tools+=("make")
    else
        echo -e "${GREEN}${BOLD}[✓]${NC} make is available"
    fi
    
    if [[ ${#missing_tools[@]} -eq 0 ]]; then
        echo -e "${GREEN}${BOLD}[✓]${NC} All build tools are available"
        return 0
    else
        echo -e "${RED}${BOLD}[✗]${NC} Missing build tools: ${missing_tools[*]}"
        return 1
    fi
}

check_rust() {
    echo -e "${YELLOW}${BOLD}🦀 Checking Rust installation...${NC}"
    if command -v rustc >/dev/null 2>&1; then
        local rust_version=$(rustc --version | cut -d' ' -f2)
        echo -e "${GREEN}${BOLD}[✓]${NC} Rust ${rust_version} is installed"
        
        if command -v cargo >/dev/null 2>&1; then
            local cargo_version=$(cargo --version | cut -d' ' -f2)
            echo -e "${GREEN}${BOLD}[✓]${NC} Cargo ${cargo_version} is available"
            return 0
        else
            echo -e "${RED}${BOLD}[✗]${NC} Cargo is missing"
            return 1
        fi
    else
        echo -e "${RED}${BOLD}[✗]${NC} Rust is not installed"
        return 1
    fi
}

check_postgresql() {
    echo -e "${YELLOW}${BOLD}🐘 Checking PostgreSQL...${NC}"
    
    # Check if PostgreSQL is installed
    if command -v psql >/dev/null 2>&1; then
        echo -e "${GREEN}${BOLD}[✓]${NC} PostgreSQL client is installed"
        
        # Check if PostgreSQL service is running
        if systemctl is-active --quiet postgresql 2>/dev/null || pgrep -x postgres >/dev/null 2>&1; then
            echo -e "${GREEN}${BOLD}[✓]${NC} PostgreSQL service is running"
            return 0
        else
            echo -e "${YELLOW}${BOLD}[!]${NC} PostgreSQL is installed but not running"
            return 2
        fi
    else
        echo -e "${RED}${BOLD}[✗]${NC} PostgreSQL is not installed"
        return 1
    fi
}

check_environment() {
    echo -e "${BRIGHT_BLUE}${BOLD}┌─────────────────────────────────────────────┐"
    echo -e "│           🔍 ENVIRONMENT CHECK              │"
    echo -e "└─────────────────────────────────────────────┘${NC}"
    echo
    
    local all_good=true
    
    # Check OS
    local os=$(detect_os)
    echo -e "${CYAN}🖥️  Detected OS: ${BOLD}${os}${NC}"
    echo
    
    # Check OpenSSL
    local openssl_status
    check_openssl
    openssl_status=$?
    if [ $openssl_status -eq 1 ]; then
        all_good=false
    elif [ $openssl_status -eq 2 ]; then
        echo -e "${YELLOW}${BOLD}[!]${NC} OpenSSL development headers needed for Rust compilation"
        all_good=false
    fi
    echo
    
    # Check build tools
    if ! check_build_tools; then
        all_good=false
    fi
    echo
    
    # Check Rust
    if ! check_rust; then
        all_good=false
    fi
    echo
    
    # Check PostgreSQL
    local pg_status
    check_postgresql
    pg_status=$?
    if [ $pg_status -eq 1 ]; then
        all_good=false
    elif [ $pg_status -eq 2 ]; then
        echo -e "${YELLOW}${BOLD}[!]${NC} PostgreSQL needs to be started"
    fi
    echo
    
    # Check other dependencies
    check_command "git" "Git"
    check_command "curl" "cURL"
    check_command "jq" "jq (JSON processor)"
    
    echo
    if $all_good; then
        echo -e "${GREEN}${BOLD}🎉 All dependencies are satisfied!${NC}"
        return 0
    else
        echo -e "${RED}${BOLD}⚠️  Some dependencies are missing!${NC}"
        return 1
    fi
}

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                    📦 INSTALLATION FUNCTIONS                                │
# └─────────────────────────────────────────────────────────────────────────────┘

install_system_dependencies() {
    echo -e "${BRIGHT_YELLOW}${BOLD}🔧 Installing System Dependencies...${NC}"
    
    local os=$(detect_os)
    echo -e "${CYAN}📋 Detected OS: ${BOLD}${os}${NC}"
    
    case $os in
        ubuntu|debian)
            install_dependencies_debian
            ;;
        fedora|rhel|centos|rocky|almalinux)
            install_dependencies_redhat
            ;;
        arch|manjaro)
            install_dependencies_arch
            ;;
        opensuse*|suse|sles)
            install_dependencies_suse
            ;;
        alpine)
            install_dependencies_alpine
            ;;
        *)
            echo -e "${YELLOW}${BOLD}[!]${NC} Unsupported OS: ${os}. Trying generic installation..."
            install_dependencies_generic
            ;;
    esac
}

install_dependencies_debian() {
    echo -e "${CYAN}📦 Installing dependencies for Debian/Ubuntu...${NC}"
    
    # Update package list
    echo -e "${CYAN}🔄 Updating package list...${NC}"
    sudo apt-get update
    
    # Install essential build tools
    echo -e "${CYAN}🔧 Installing build tools...${NC}"
    sudo apt-get install -y \
        build-essential \
        curl \
        wget \
        git \
        pkg-config \
        libssl-dev \
        libffi-dev \
        libgmp-dev \
        libsqlite3-dev \
        libpq-dev \
        jq \
        unzip \
        ca-certificates \
        gnupg \
        lsb-release
    
    echo -e "${GREEN}${BOLD}[✓]${NC} Debian/Ubuntu dependencies installed successfully!"
}

install_dependencies_redhat() {
    echo -e "${CYAN}📦 Installing dependencies for RHEL/CentOS/Fedora...${NC}"
    
    # Detect package manager
    if command -v dnf >/dev/null 2>&1; then
        local pkg_mgr="dnf"
    elif command -v yum >/dev/null 2>&1; then
        local pkg_mgr="yum"
    else
        echo -e "${RED}${BOLD}[✗]${NC} No package manager found (dnf/yum)"
        return 1
    fi
    
    # Update package list
    echo -e "${CYAN}🔄 Updating package list...${NC}"
    sudo $pkg_mgr update -y
    
    # Install essential build tools
    echo -e "${CYAN}🔧 Installing build tools...${NC}"
    sudo $pkg_mgr install -y \
        gcc \
        gcc-c++ \
        make \
        curl \
        wget \
        git \
        pkg-config \
        openssl-devel \
        libffi-devel \
        gmp-devel \
        sqlite-devel \
        postgresql-devel \
        jq \
        unzip \
        ca-certificates
    
    # Install development tools group if available
    sudo $pkg_mgr groupinstall -y "Development Tools" 2>/dev/null || true
    
    echo -e "${GREEN}${BOLD}[✓]${NC} RHEL/CentOS/Fedora dependencies installed successfully!"
}

install_dependencies_arch() {
    echo -e "${CYAN}📦 Installing dependencies for Arch Linux...${NC}"
    
    # Update package list
    echo -e "${CYAN}🔄 Updating package database...${NC}"
    if ! sudo pacman -Sy --noconfirm; then
        echo -e "${RED}[✗] Failed to update package database${NC}"
        return 1
    fi
    
    # Install essential packages for Rust development
    echo -e "${CYAN}🔧 Installing build tools and dependencies...${NC}"
    local packages=(
        "base-devel"        # Essential build tools (includes gcc, make, etc.)
        "rust"              # Rust programming language
        "cargo"             # Rust package manager (usually included with rust)
        "postgresql"        # PostgreSQL database server
        "postgresql-libs"   # PostgreSQL client libraries  
        "curl"              # HTTP client
        "wget"              # Download tool
        "git"               # Version control
        "pkg-config"        # Package configuration tool
        "openssl"           # OpenSSL library
        "libffi"            # Foreign function interface library
        "gmp"               # GNU Multiple Precision Arithmetic Library
        "sqlite"            # SQLite database
        "jq"                # JSON processor
        "unzip"             # Archive extraction
        "ca-certificates"   # Certificate authorities
        "cmake"             # Build system (needed for some Rust crates)
    )
    
    for package in "${packages[@]}"; do
        echo -e "${CYAN}  📦 Installing: $package${NC}"
        if ! sudo pacman -S --needed --noconfirm "$package"; then
            echo -e "${YELLOW}[⚠] Warning: Failed to install $package, continuing...${NC}"
        fi
    done
    
    # Initialize and configure PostgreSQL
    echo -e "${CYAN}🗄️  Configuring PostgreSQL...${NC}"
    if ! sudo -u postgres test -d /var/lib/postgres/data; then
        echo -e "${CYAN}  📊 Initializing PostgreSQL database cluster...${NC}"
        sudo -u postgres initdb -D /var/lib/postgres/data --locale=C.UTF-8 --encoding=UTF8
    fi
    
    # Enable and start PostgreSQL service  
    echo -e "${CYAN}  ⚡ Starting PostgreSQL service...${NC}"
    sudo systemctl enable postgresql >/dev/null 2>&1
    sudo systemctl start postgresql
    
    # Wait for PostgreSQL to start
    sleep 3
    if sudo systemctl is-active postgresql >/dev/null 2>&1; then
        echo -e "${GREEN}  [✓] PostgreSQL is running${NC}"
    else
        echo -e "${YELLOW}  [⚠] PostgreSQL may not be running properly${NC}"
    fi
    
    # Install Rust toolchain if not present
    if ! command -v rustc >/dev/null 2>&1; then
        echo -e "${CYAN}🦀 Installing Rust toolchain...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
    fi
    
    echo -e "${GREEN}${BOLD}[✓]${NC} Arch Linux dependencies installed successfully!"
}

install_dependencies_suse() {
    echo -e "${CYAN}📦 Installing dependencies for openSUSE...${NC}"
    
    # Update package list
    echo -e "${CYAN}🔄 Refreshing repositories...${NC}"
    sudo zypper refresh
    
    # Install essential packages
    echo -e "${CYAN}🔧 Installing build tools...${NC}"
    sudo zypper install -y \
        gcc \
        gcc-c++ \
        make \
        curl \
        wget \
        git \
        pkg-config \
        libopenssl-devel \
        libffi-devel \
        gmp-devel \
        sqlite3-devel \
        postgresql-devel \
        jq \
        unzip \
        ca-certificates
    
    # Install patterns for development
    sudo zypper install -y -t pattern devel_basis 2>/dev/null || true
    
    echo -e "${GREEN}${BOLD}[✓]${NC} openSUSE dependencies installed successfully!"
}

install_dependencies_alpine() {
    echo -e "${CYAN}📦 Installing dependencies for Alpine Linux...${NC}"
    
    # Update package list
    echo -e "${CYAN}🔄 Updating package index...${NC}"
    sudo apk update
    
    # Install essential packages
    echo -e "${CYAN}🔧 Installing build tools...${NC}"
    sudo apk add \
        build-base \
        curl \
        wget \
        git \
        pkgconfig \
        openssl-dev \
        libffi-dev \
        gmp-dev \
        sqlite-dev \
        postgresql-dev \
        jq \
        unzip \
        ca-certificates
    
    echo -e "${GREEN}${BOLD}[✓]${NC} Alpine Linux dependencies installed successfully!"
}

install_dependencies_generic() {
    echo -e "${YELLOW}${BOLD}[!]${NC} Attempting generic installation..."
    echo -e "${CYAN}Please install the following packages manually:${NC}"
    echo -e "${WHITE}• C compiler (gcc/clang)${NC}"
    echo -e "${WHITE}• make${NC}"
    echo -e "${WHITE}• pkg-config${NC}"
    echo -e "${WHITE}• OpenSSL development headers${NC}"
    echo -e "${WHITE}• libffi development headers${NC}"
    echo -e "${WHITE}• curl${NC}"
    echo -e "${WHITE}• git${NC}"
    echo -e "${WHITE}• jq${NC}"
    echo
    echo -e "${YELLOW}${BOLD}[!]${NC} Press Enter when you have installed the required packages..."
    read -r
}

install_rust() {
    echo -e "${BRIGHT_YELLOW}${BOLD}🦀 Installing Rust...${NC}"
    
    if command -v rustc >/dev/null 2>&1; then
        local rust_version=$(rustc --version | cut -d' ' -f2)
        echo -e "${GREEN}${BOLD}[✓]${NC} Rust ${rust_version} is already installed"
        
        # Check if cargo is available
        if command -v cargo >/dev/null 2>&1; then
            echo -e "${GREEN}${BOLD}[✓]${NC} Cargo is available"
            return 0
        fi
    fi
    
    # Ensure curl is available
    if ! command -v curl >/dev/null 2>&1; then
        echo -e "${RED}${BOLD}[✗]${NC} curl is required for Rust installation"
        return 1
    fi
    
    echo -e "${CYAN}📥 Downloading Rust installer...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    
    echo -e "${CYAN}🔄 Updating PATH...${NC}"
    if [[ -f ~/.cargo/env ]]; then
        source ~/.cargo/env
    fi
    
    # Add to shell profile for persistence
    if [[ -f ~/.bashrc ]]; then
        grep -qxF 'source ~/.cargo/env' ~/.bashrc || echo 'source ~/.cargo/env' >> ~/.bashrc
    fi
    if [[ -f ~/.zshrc ]]; then
        grep -qxF 'source ~/.cargo/env' ~/.zshrc || echo 'source ~/.cargo/env' >> ~/.zshrc
    fi
    
    # Verify installation
    if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
        local rust_version=$(rustc --version | cut -d' ' -f2)
        echo -e "${GREEN}${BOLD}[✓]${NC} Rust ${rust_version} installation complete!"
        echo -e "${GREEN}${BOLD}[✓]${NC} Cargo is available"
        
        # Install common tools
        echo -e "${CYAN}🔧 Installing useful Rust tools...${NC}"
        ~/.cargo/bin/rustup component add rustfmt clippy 2>/dev/null || true
    else
        echo -e "${RED}${BOLD}[✗]${NC} Rust installation failed"
        return 1
    fi
}

install_postgresql() {
    echo -e "${BRIGHT_YELLOW}${BOLD}🐘 Installing PostgreSQL...${NC}"
    
    if command -v psql >/dev/null 2>&1; then
        echo -e "${GREEN}${BOLD}[✓]${NC} PostgreSQL is already installed"
        return 0
    fi
    
    echo -e "${CYAN}📥 Installing PostgreSQL...${NC}"
    
    # Detect OS and install accordingly
    if [[ -f /etc/debian_version ]]; then
        sudo apt-get update
        sudo apt-get install -y postgresql postgresql-contrib
    elif [[ -f /etc/redhat-release ]]; then
        sudo yum install -y postgresql postgresql-server postgresql-contrib
        sudo postgresql-setup initdb
    else
        echo -e "${RED}${BOLD}[✗]${NC} Unsupported OS for automatic PostgreSQL installation"
        return 1
    fi
    
    echo -e "${CYAN}🚀 Starting PostgreSQL service...${NC}"
    sudo systemctl enable postgresql
    sudo systemctl start postgresql
    
    echo -e "${GREEN}${BOLD}[✓]${NC} PostgreSQL installation complete!"
}

setup_database() {
    echo -e "${BRIGHT_YELLOW}${BOLD}🗄️  Setting up database...${NC}"
    
    # Check if .env file exists
    if [[ ! -f .env ]]; then
        echo -e "${CYAN}📝 Creating .env file...${NC}"
        cat > .env << 'EOF'
DATABASE_URL=postgresql://github_archiver:secure_password@localhost/github_archiver
JWT_SECRET=your-super-secure-jwt-secret-key-change-this-in-production
SERVER_HOST=127.0.0.1
SERVER_PORT=8081
EOF
        echo -e "${GREEN}${BOLD}[✓]${NC} .env file created"
    else
        echo -e "${GREEN}${BOLD}[✓]${NC} .env file already exists"
    fi
    
    # Setup database user and database
    echo -e "${CYAN}🔧 Setting up database user and database...${NC}"
    
    sudo -u postgres psql << 'EOF'
-- Create user if not exists
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'github_archiver') THEN
        CREATE USER github_archiver WITH PASSWORD 'secure_password';
    END IF;
END
$$;

-- Create database if not exists
SELECT 'CREATE DATABASE github_archiver OWNER github_archiver'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'github_archiver')\gexec

-- Grant privileges
GRANT ALL PRIVILEGES ON DATABASE github_archiver TO github_archiver;
EOF
    
    # Setup database schema with proper permissions
    echo -e "${CYAN}🗄️  Setting up database schema...${NC}"
    
    # Create schema file if it doesn't exist
    if [[ ! -f schema.sql ]]; then
        cat > schema.sql << 'SCHEMA_EOF'
-- GitHub Archiver Database Schema
-- Creating extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "btree_gin";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Main events table with comprehensive GitHub API data capture
CREATE TABLE IF NOT EXISTS github_events (
    event_id BIGINT PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    event_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    event_public BOOLEAN NOT NULL DEFAULT true,
    
    -- Actor information
    actor_id BIGINT,
    actor_login VARCHAR(255),
    actor_display_login VARCHAR(255),
    actor_gravatar_id VARCHAR(255),
    actor_url TEXT,
    actor_avatar_url TEXT,
    actor_node_id VARCHAR(255),
    actor_html_url TEXT,
    actor_followers_url TEXT,
    actor_following_url TEXT,
    actor_gists_url TEXT,
    actor_starred_url TEXT,
    actor_subscriptions_url TEXT,
    actor_organizations_url TEXT,
    actor_repos_url TEXT,
    actor_events_url TEXT,
    actor_received_events_url TEXT,
    actor_type VARCHAR(50),
    actor_user_view_type VARCHAR(50),
    actor_site_admin BOOLEAN,
    
    -- Repository information
    repo_id BIGINT,
    repo_name VARCHAR(255),
    repo_url TEXT,
    repo_full_name VARCHAR(255),
    repo_owner_login VARCHAR(255),
    repo_owner_id BIGINT,
    repo_owner_node_id VARCHAR(255),
    repo_owner_avatar_url TEXT,
    repo_owner_gravatar_id VARCHAR(255),
    repo_owner_url TEXT,
    repo_owner_html_url TEXT,
    repo_owner_type VARCHAR(50),
    repo_owner_site_admin BOOLEAN,
    repo_node_id VARCHAR(255),
    repo_html_url TEXT,
    repo_description TEXT,
    repo_fork BOOLEAN,
    repo_language VARCHAR(100),
    repo_stargazers_count BIGINT,
    repo_watchers_count BIGINT,
    repo_forks_count BIGINT,
    repo_open_issues_count BIGINT,
    repo_size BIGINT,
    repo_default_branch VARCHAR(100),
    repo_topics TEXT[],
    repo_license_key VARCHAR(50),
    repo_license_name VARCHAR(255),
    repo_created_at TIMESTAMP WITH TIME ZONE,
    repo_updated_at TIMESTAMP WITH TIME ZONE,
    repo_pushed_at TIMESTAMP WITH TIME ZONE,
    
    -- Organization information (optional)
    org_id BIGINT,
    org_login VARCHAR(255),
    org_node_id VARCHAR(255),
    org_gravatar_id VARCHAR(255),
    org_url TEXT,
    org_avatar_url TEXT,
    org_html_url TEXT,
    org_type VARCHAR(50),
    org_site_admin BOOLEAN,
    
    -- Complete payload as JSONB for flexible querying
    payload JSONB,
    raw_event JSONB,
    
    -- Metadata
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    file_source VARCHAR(255),
    api_source VARCHAR(255)
);

-- Processed files tracking
CREATE TABLE IF NOT EXISTS processed_files (
    filename VARCHAR(255) PRIMARY KEY,
    file_size BIGINT,
    etag VARCHAR(255),
    last_modified TIMESTAMP WITH TIME ZONE,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    event_count INTEGER DEFAULT 0,
    is_complete BOOLEAN DEFAULT TRUE
);

-- Repositories tracking table
CREATE TABLE IF NOT EXISTS repositories (
    id BIGINT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    full_name VARCHAR(255),
    description TEXT,
    html_url TEXT,
    language VARCHAR(100),
    default_branch VARCHAR(100),
    created_at TIMESTAMP WITH TIME ZONE,
    updated_at TIMESTAMP WITH TIME ZONE,
    pushed_at TIMESTAMP WITH TIME ZONE,
    stargazers_count INTEGER,
    watchers_count INTEGER,
    forks_count INTEGER,
    open_issues_count INTEGER,
    topics TEXT[],
    license_name VARCHAR(255),
    owner_login VARCHAR(255),
    owner_type VARCHAR(50),
    first_seen_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_github_events_created_at ON github_events (event_created_at);
CREATE INDEX IF NOT EXISTS idx_github_events_type ON github_events (event_type);
CREATE INDEX IF NOT EXISTS idx_github_events_actor_id ON github_events (actor_id);
CREATE INDEX IF NOT EXISTS idx_github_events_repo_id ON github_events (repo_id);
CREATE INDEX IF NOT EXISTS idx_github_events_actor_login ON github_events (actor_login);
CREATE INDEX IF NOT EXISTS idx_github_events_repo_name ON github_events (repo_name);
CREATE INDEX IF NOT EXISTS idx_github_events_payload ON github_events USING GIN (payload);
CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories (language);
CREATE INDEX IF NOT EXISTS idx_repositories_stars ON repositories (stargazers_count DESC);

-- Grant permissions to github_archiver user
ALTER TABLE github_events OWNER TO github_archiver;
ALTER TABLE processed_files OWNER TO github_archiver;
ALTER TABLE repositories OWNER TO github_archiver;

GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO github_archiver;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO github_archiver;
GRANT USAGE ON SCHEMA public TO github_archiver;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO github_archiver;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO github_archiver;
SCHEMA_EOF
    fi
    
    # Execute schema creation
    sudo -u postgres psql -d github_archiver -f schema.sql >/dev/null 2>&1
    
    echo -e "${GREEN}${BOLD}[✓]${NC} Database setup complete!"
}

build_project() {
    echo -e "${BRIGHT_YELLOW}${BOLD}🔨 Building project...${NC}"
    
    echo -e "${CYAN}📦 Installing dependencies...${NC}"
    cargo build --release &
    spinner $! "dots" "Building release binary"
    
    echo -e "${GREEN}${BOLD}[✓]${NC} Project build complete!"
}

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                    🎮 SERVICE MANAGEMENT                                    │
# └─────────────────────────────────────────────────────────────────────────────┘

start_service() {
    echo -e "\n${BRIGHT_BLUE}${BOLD}🚀 Starting GitHub Archiver...${NC}"
    echo -e "${BRIGHT_BLUE}${BOLD}┌─────────────────────────────────────────────┐${NC}"
    echo -e "${BRIGHT_BLUE}${BOLD}│            STARTING SERVICE                │${NC}"
    echo -e "${BRIGHT_BLUE}${BOLD}└─────────────────────────────────────────────┘${NC}"
    
    # Check if binary exists
    if [ ! -f "./target/release/github_archiver" ]; then
        echo -e "${RED}[✗] Binary not found: ./target/release/github_archiver${NC}"
        echo -e "${YELLOW}💡 Run 'Build Project' first to compile the application${NC}"
        return 1
    fi
    
    if [ -f "service.pid" ]; then
        PID=$(cat service.pid)
        if ps -p $PID > /dev/null 2>&1; then
            echo -e "${YELLOW}⚠️  Service is already running (PID: $PID)${NC}"
            echo -e "${CYAN}📡 API accessible at: http://localhost:8081${NC}"
            return 0
        else
            rm -f service.pid
        fi
    fi
    
    echo -e "${BLUE}⚡ Starting server on port 8081...${NC}"
    
    # Start the service in background with correct binary name
    nohup ./target/release/github_archiver > service.log 2>&1 &
    SERVICE_PID=$!
    echo $SERVICE_PID > service.pid
    
    # Wait a moment and check if it started successfully
    sleep 3
    if ps -p $SERVICE_PID > /dev/null 2>&1; then
        echo -e "${GREEN}[✓] Service started successfully (PID: $SERVICE_PID)${NC}"
        echo -e "${CYAN}📡 API accessible at: http://localhost:8081${NC}"
        echo -e "${CYAN}📊 Dashboard available at: http://localhost:8081${NC}"
        
        # Test if service is responding
        echo -e "${BLUE}🔍 Testing service connectivity...${NC}"
        sleep 2
        if command -v curl >/dev/null 2>&1; then
            if curl -s --connect-timeout 5 http://localhost:8081/health >/dev/null 2>&1; then
                echo -e "${GREEN}[✓] Service is responding to requests${NC}"
            else
                echo -e "${YELLOW}[⚠] Service started but may not be fully ready yet${NC}"
            fi
        fi
    else
        echo -e "${RED}[✗] Failed to start service${NC}"
        echo -e "${YELLOW}💡 Check service.log for details:${NC}"
        if [ -f "service.log" ]; then
            tail -n 10 service.log
        fi
        return 1
    fi
}

stop_service() {
    echo -e "${BRIGHT_RED}${BOLD}🛑 Stopping GitHub Archiver...${NC}"
    
    if [[ -f service.pid ]]; then
        local pid=$(cat service.pid)
        if kill -0 $pid 2>/dev/null; then
            kill $pid
            echo -e "${GREEN}${BOLD}[✓]${NC} Service stopped (PID: $pid)"
            rm service.pid
        else
            echo -e "${YELLOW}${BOLD}[!]${NC} Service was not running"
            rm service.pid
        fi
    else
        # Try to find and kill the process
        local pid=$(pgrep -f "target/release/github_archiver")
        if [[ -n $pid ]]; then
            kill $pid
            echo -e "${GREEN}${BOLD}[✓]${NC} Service stopped (PID: $pid)"
        else
            echo -e "${YELLOW}${BOLD}[!]${NC} Service is not running"
        fi
    fi
}

restart_service() {
    echo -e "${BRIGHT_MAGENTA}${BOLD}🔄 Restarting GitHub Archiver...${NC}"
    echo
    
    # First stop the service
    echo -e "${YELLOW}📴 Stopping current service...${NC}"
    if [[ -f service.pid ]]; then
        local pid=$(cat service.pid)
        if kill -0 $pid 2>/dev/null; then
            kill $pid
            echo -e "${GREEN}${BOLD}[✓]${NC} Service stopped (PID: $pid)"
            rm service.pid
            sleep 2  # Give it a moment to fully stop
        else
            echo -e "${YELLOW}${BOLD}[!]${NC} Service was not running"
            rm service.pid
        fi
    else
        # Try to find and kill the process
        local pid=$(pgrep -f "target/release/github_archiver")
        if [[ -n $pid ]]; then
            kill $pid
            echo -e "${GREEN}${BOLD}[✓]${NC} Service stopped (PID: $pid)"
            sleep 2
        else
            echo -e "${YELLOW}${BOLD}[!]${NC} No running service found"
        fi
    fi
    
    echo
    echo -e "${GREEN}🚀 Starting service...${NC}"
    
    # Check if binary exists
    if [[ ! -f target/release/github_archiver ]]; then
        echo -e "${RED}${BOLD}[✗]${NC} Binary not found. Building project first..."
        echo
        build_project
        echo
    fi
    
    # Start the service
    if [[ -f target/release/github_archiver ]]; then
        nohup ./target/release/github_archiver > service.log 2>&1 &
        local SERVICE_PID=$!
        echo $SERVICE_PID > service.pid
        
        # Verify the service started
        sleep 2
        if kill -0 $SERVICE_PID 2>/dev/null; then
            echo -e "${GREEN}${BOLD}[✓]${NC} Service restarted successfully (PID: $SERVICE_PID)"
            echo -e "${CYAN}📍 Log file: service.log${NC}"
            echo -e "${CYAN}🌐 Server should be available at: http://localhost:3000${NC}"
        else
            echo -e "${RED}${BOLD}[✗]${NC} Service failed to start. Check logs for details."
            rm -f service.pid
        fi
    else
        echo -e "${RED}${BOLD}[✗]${NC} Failed to build project. Cannot start service."
    fi
}

service_status() {
    echo -e "${BRIGHT_BLUE}${BOLD}📊 Service Status${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    
    # Get external IP
    local external_ip=$(curl -s ifconfig.me 2>/dev/null || curl -s ipinfo.io/ip 2>/dev/null || echo "Unable to determine")
    
    if [[ -f service.pid ]]; then
        local pid=$(cat service.pid)
        if kill -0 $pid 2>/dev/null; then
            echo -e "${GREEN}${BOLD}Status:${NC} Running"
            echo -e "${GREEN}${BOLD}PID:${NC} $pid"
            echo -e "${GREEN}${BOLD}Port:${NC} 8081"
            echo -e "${GREEN}${BOLD}Local URL:${NC} http://127.0.0.1:8081"
            echo -e "${GREEN}${BOLD}External URL:${NC} http://${external_ip}:8081"
            
            # Check if port is actually open
            local port_status=$(ss -tln | grep ":8081 " >/dev/null 2>&1 && echo "Open" || echo "Closed")
            echo -e "${GREEN}${BOLD}Port Status:${NC} ${port_status}"
            
            # Memory usage
            local mem=$(ps -p $pid -o rss= 2>/dev/null | awk '{print $1/1024 " MB"}')
            echo -e "${GREEN}${BOLD}Memory:${NC} ${mem:-"N/A"}"
            
            # CPU usage
            local cpu=$(ps -p $pid -o %cpu= 2>/dev/null | awk '{print $1"%"}')
            echo -e "${GREEN}${BOLD}CPU:${NC} ${cpu:-"N/A"}"
            
            # Uptime
            local start_time=$(ps -p $pid -o lstart= 2>/dev/null)
            echo -e "${GREEN}${BOLD}Started:${NC} ${start_time:-"N/A"}"
            
            echo
            echo -e "${BRIGHT_CYAN}${BOLD}🌐 Network Information${NC}"
            echo -e "${CYAN}───────────────────────────────────────${NC}"
            echo -e "${BLUE}${BOLD}External IP:${NC} ${external_ip}"
            
            # Show open ports related to the service
            echo -e "${BLUE}${BOLD}Open Ports:${NC}"
            ss -tln | grep ":808[0-9]" | while read line; do
                local port=$(echo "$line" | awk '{print $4}' | sed 's/.*://')
                echo -e "  ${GREEN}→${NC} Port ${port}"
            done
            
            echo
            echo -e "${BRIGHT_PURPLE}${BOLD}🗄️ Database Information${NC}"
            echo -e "${PURPLE}───────────────────────────────────────${NC}"
            
            # Check if .env file exists and get database info
            if [[ -f .env ]]; then
                local db_url=$(grep "DATABASE_URL" .env 2>/dev/null | cut -d'=' -f2- | sed 's/"//g')
                if [[ -n "$db_url" ]]; then
                    local db_host=$(echo "$db_url" | sed -n 's/.*@\([^:]*\):.*/\1/p')
                    local db_port=$(echo "$db_url" | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
                    local db_name=$(echo "$db_url" | sed -n 's/.*\/\([^?]*\).*/\1/p')
                    
                    echo -e "${PURPLE}${BOLD}Database Host:${NC} ${db_host:-"localhost"}"
                    echo -e "${PURPLE}${BOLD}Database Port:${NC} ${db_port:-"5432"}"
                    echo -e "${PURPLE}${BOLD}Database Name:${NC} ${db_name:-"N/A"}"
                    
                    # Test database connection
                    local db_status="Unknown"
                    if command -v psql >/dev/null 2>&1; then
                        if psql "$db_url" -c "SELECT 1;" >/dev/null 2>&1; then
                            db_status="${GREEN}Connected${NC}"
                            
                            # Get database statistics
                            local db_size=$(psql "$db_url" -t -c "SELECT pg_size_pretty(pg_database_size(current_database()));" 2>/dev/null | xargs)
                            local table_count=$(psql "$db_url" -t -c "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public';" 2>/dev/null | xargs)
                            
                            echo -e "${PURPLE}${BOLD}Connection:${NC} ${db_status}"
                            echo -e "${PURPLE}${BOLD}Database Size:${NC} ${db_size:-"N/A"}"
                            echo -e "${PURPLE}${BOLD}Tables:${NC} ${table_count:-"N/A"}"
                        else
                            db_status="${RED}Connection Failed${NC}"
                            echo -e "${PURPLE}${BOLD}Connection:${NC} ${db_status}"
                        fi
                    else
                        echo -e "${PURPLE}${BOLD}Connection:${NC} ${YELLOW}psql not available${NC}"
                    fi
                else
                    echo -e "${YELLOW}No DATABASE_URL found in .env${NC}"
                fi
            else
                echo -e "${YELLOW}No .env file found${NC}"
            fi
            
            echo
            echo -e "${BRIGHT_YELLOW}${BOLD}📈 Service Health${NC}"
            echo -e "${YELLOW}───────────────────────────────────────${NC}"
            
            # Test API endpoint
            local api_status=$(curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8081/health 2>/dev/null || echo "000")
            if [[ "$api_status" == "200" ]]; then
                echo -e "${YELLOW}${BOLD}API Health:${NC} ${GREEN}Healthy (HTTP $api_status)${NC}"
            else
                echo -e "${YELLOW}${BOLD}API Health:${NC} ${RED}Unhealthy (HTTP $api_status)${NC}"
            fi
            
        else
            echo -e "${RED}${BOLD}Status:${NC} Stopped (stale PID file)"
            rm service.pid
        fi
    else
        local pid=$(pgrep -f "target/release/github_archiver")
        if [[ -n $pid ]]; then
            echo -e "${YELLOW}${BOLD}Status:${NC} Running (no PID file)"
            echo -e "${YELLOW}${BOLD}PID:${NC} $pid"
        else
            echo -e "${RED}${BOLD}Status:${NC} Stopped"
        fi
    fi
}

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                      📋 MENU FUNCTIONS                                      │
# └─────────────────────────────────────────────────────────────────────────────┘

show_banner() {
    clear_screen
    
    # Get terminal dimensions
    local cols=$(tput cols)
    local lines=$(tput lines)
    
    echo
    
    # Determine logo size based on terminal dimensions
    # Large: 100+ cols, 35+ lines
    # Medium: 80+ cols, 25+ lines  
    # Tiny: anything smaller
    
    if [ $cols -ge 100 ] && [ $lines -ge 35 ]; then
        # LARGE LOGO - Full ASCII art with gradient
        gradient_text "    ███████╗██╗████████╗██╗  ██╗██╗   ██╗██████╗      █████╗ ██████╗  ██████╗██╗  ██╗██╗██╗   ██╗███████╗██████╗ "
        gradient_text "    ██╔════╝██║╚══██╔══╝██║  ██║██║   ██║██╔══██╗    ██╔══██╗██╔══██╗██╔════╝██║  ██║██║██║   ██║██╔════╝██╔══██╗"
        gradient_text "    ██║  ███╗██║   ██║   ███████║██║   ██║██████╔╝    ███████║██████╔╝██║     ███████║██║██║   ██║█████╗  ██████╔╝"
        gradient_text "    ██║   ██║██║   ██║   ██╔══██║██║   ██║██╔══██╗    ██╔══██║██╔══██╗██║     ██╔══██║██║╚██╗ ██╔╝██╔══╝  ██╔══██╗"
        gradient_text "    ╚██████╔╝██║   ██║   ██║  ██║╚██████╔╝██████╔╝    ██║  ██║██║  ██║╚██████╗██║  ██║██║ ╚████╔╝ ███████╗██║  ██║"
        gradient_text "     ╚═════╝ ╚═╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ╚═════╝     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═╝"
        echo
        echo -e "${BRIGHT_CYAN}${BOLD}                           🚀 Professional Interactive Setup & Management System${NC}"
        echo -e "${DIM}                                      Version 3.0.0 | Rust Edition${NC}"
        
    elif [ $cols -ge 80 ] && [ $lines -ge 25 ]; then
        # MEDIUM LOGO - Compact ASCII art with gradient
        gradient_text "  ███████╗██╗████████╗██╗  ██╗██╗   ██╗██████╗      █████╗ ██████╗  ██████╗██╗  ██╗██╗██╗   ██╗███████╗██████╗ "
        gradient_text "  ██╔════╝██║╚══██╔══╝██║  ██║██║   ██║██╔══██╗    ██╔══██╗██╔══██╗██╔════╝██║  ██║██║██║   ██║██╔════╝██╔══██╗"
        gradient_text "  ██║  ███╗██║   ██║   ███████║██║   ██║██████╔╝    ███████║██████╔╝██║     ███████║██║██║   ██║█████╗  ██████╔╝"
        gradient_text "  ╚██████╔╝██║   ██║   ██║  ██║╚██████╔╝██████╔╝    ██║  ██║██║  ██║╚██████╗██║  ██║██║ ╚████╔╝ ███████╗██║  ██║"
        gradient_text "   ╚═════╝ ╚═╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ╚═════╝     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═╝"
        echo
        echo -e "${BRIGHT_CYAN}${BOLD}             🚀 Professional Interactive Setup & Management System${NC}"
        echo -e "${DIM}                        Version 3.0.0 | Rust Edition${NC}"
        
    else
        # TINY LOGO - Text-based minimal
        echo -e "${BRIGHT_BLUE}${BOLD}╔══════════════════════════════════════════════════════════╗${NC}"
        echo -e "${BRIGHT_BLUE}${BOLD}║                     GITHUB ARCHIVER                     ║${NC}"
        gradient_text "║             🚀 PROFESSIONAL SETUP SYSTEM 🚀             ║"
        echo -e "${BRIGHT_BLUE}${BOLD}╚══════════════════════════════════════════════════════════╝${NC}"
        echo
        echo -e "${BRIGHT_CYAN}${BOLD}       Professional Interactive Setup & Management${NC}"
        echo -e "${DIM}                Version 3.0.0 | Rust Edition${NC}"
    fi
    
    echo
}

show_main_menu() {
    echo -e "${BRIGHT_BLUE}${BOLD}┌─────────────────────────────────────────────┐"
    echo -e "│                MAIN MENU                    │"
    echo -e "└─────────────────────────────────────────────┘${NC}"
    echo
    echo -e "${BRIGHT_WHITE}${BOLD}[1]${NC} 🔍 ${CYAN}System Check${NC}        - Check dependencies and environment"
    echo -e "${BRIGHT_WHITE}${BOLD}[2]${NC} 🛠️  ${BRIGHT_GREEN}Install Dependencies${NC} - Install Rust, OpenSSL & build tools"
    echo -e "${BRIGHT_WHITE}${BOLD}[3]${NC} 📦 ${YELLOW}Full Installation${NC}   - Install all dependencies and setup"
    echo -e "${BRIGHT_WHITE}${BOLD}[4]${NC} 🔨 ${BLUE}Build Project${NC}       - Compile the Rust application"
    echo -e "${BRIGHT_WHITE}${BOLD}[5]${NC} 🚀 ${GREEN}Start Service${NC}       - Start the GitHub Archiver service"
    echo -e "${BRIGHT_WHITE}${BOLD}[6]${NC} 🛑 ${RED}Stop Service${NC}        - Stop the running service"
    echo -e "${BRIGHT_WHITE}${BOLD}[7]${NC} � ${BRIGHT_MAGENTA}Restart Service${NC}     - Restart the GitHub Archiver service"
    echo -e "${BRIGHT_WHITE}${BOLD}[8]${NC} �📊 ${PURPLE}Service Status${NC}     - Check service status and info"
    echo -e "${BRIGHT_WHITE}${BOLD}[9]${NC} 📋 ${CYAN}View Logs${NC}          - Display service logs"
    echo -e "${BRIGHT_WHITE}${BOLD}[10]${NC} ⚙️  ${YELLOW}Configuration${NC}     - Edit configuration files"
    echo -e "${BRIGHT_WHITE}${BOLD}[A]${NC} 📊 ${BLUE}System Info${NC}        - Display system information"
    echo -e "${BRIGHT_WHITE}${BOLD}[B]${NC} 🏥 ${BRIGHT_CYAN}Health Monitor${NC}     - Comprehensive system health check"
    echo -e "${BRIGHT_WHITE}${BOLD}[C]${NC} 📊 ${BRIGHT_PURPLE}Generate Report${NC}    - Generate comprehensive system report"
    echo -e "${BRIGHT_WHITE}${BOLD}[D]${NC} 💾 ${BRIGHT_YELLOW}Backup System${NC}      - Create system backup"
    echo -e "${BRIGHT_WHITE}${BOLD}[0]${NC} 🚪 ${DIM}Exit${NC}               - Exit the setup script"
    echo
    echo -e "${BRIGHT_WHITE}${BOLD}Enter your choice [0-10,A-D]:${NC} "
}

view_logs() {
    echo -e "${BRIGHT_BLUE}${BOLD}📋 Service Logs${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    
    if [[ -f service.log ]]; then
        echo -e "${GREEN}${BOLD}[Last 20 lines]${NC}"
        echo
        tail -n 20 service.log | while IFS= read -r line; do
            if [[ $line == *"ERROR"* ]]; then
                echo -e "${RED}$line${NC}"
            elif [[ $line == *"WARN"* ]]; then
                echo -e "${YELLOW}$line${NC}"
            elif [[ $line == *"INFO"* ]]; then
                echo -e "${GREEN}$line${NC}"
            else
                echo -e "${WHITE}$line${NC}"
            fi
        done
    else
        echo -e "${YELLOW}${BOLD}[!]${NC} No logs found. Service may not have been started yet."
    fi
    
    echo
    echo -e "${DIM}Press Enter to continue...${NC}"
    read -r
}

edit_config() {
    echo -e "${BRIGHT_BLUE}${BOLD}⚙️ Configuration Management${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    echo
    echo -e "${BRIGHT_WHITE}${BOLD}[1]${NC} Edit .env file"
    echo -e "${BRIGHT_WHITE}${BOLD}[2]${NC} View current configuration"
    echo -e "${BRIGHT_WHITE}${BOLD}[3]${NC} Reset to defaults"
    echo -e "${BRIGHT_WHITE}${BOLD}[0]${NC} Back to main menu"
    echo
    echo -e "${BRIGHT_WHITE}${BOLD}Enter your choice [0-3]:${NC} "
    
    read -r config_choice
    
    case $config_choice in
        1)
            if command -v nano >/dev/null 2>&1; then
                nano .env
            elif command -v vim >/dev/null 2>&1; then
                vim .env
            else
                echo -e "${RED}${BOLD}[✗]${NC} No text editor found (nano/vim)"
            fi
            ;;
        2)
            echo -e "${CYAN}${BOLD}Current Configuration:${NC}"
            if [[ -f .env ]]; then
                cat .env
            else
                echo -e "${YELLOW}${BOLD}[!]${NC} .env file not found"
            fi
            echo
            echo -e "${DIM}Press Enter to continue...${NC}"
            read -r
            ;;
        3)
            setup_database
            ;;
        0)
            return
            ;;
        *)
            echo -e "${RED}${BOLD}Invalid choice!${NC}"
            ;;
    esac
}

install_all_dependencies() {
    echo -e "${BRIGHT_YELLOW}${BOLD}🔧 Complete Dependencies Installation${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    echo
    
    flash_text "🚀 Starting dependencies installation..." "$BRIGHT_GREEN" 2
    
    # Step 1: Install system dependencies
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 1/3: System Dependencies${NC}"
    progress_bar 1 3 50 "Dependencies Progress"
    install_system_dependencies
    
    # Step 2: Install Rust
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 2/3: Rust Toolchain${NC}"
    progress_bar 2 3 50 "Dependencies Progress"
    install_rust
    
    # Step 3: Verify installation
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 3/3: Verification${NC}"
    progress_bar 3 3 50 "Dependencies Progress"
    verify_installation
    
    echo
    flash_text "🎉 Dependencies Installation Complete!" "$BRIGHT_GREEN" 3
    echo
    echo -e "${GREEN}${BOLD}✅ All dependencies are now installed!${NC}"
    echo -e "${CYAN}You can now proceed with building the project.${NC}"
    echo
    echo -e "${DIM}Press Enter to continue...${NC}"
    read -r
}

verify_installation() {
    echo -e "${CYAN}🔍 Verifying installation...${NC}"
    
    local verification_failed=false
    
    # Check Rust
    if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
        echo -e "${GREEN}${BOLD}[✓]${NC} Rust: $(rustc --version | cut -d' ' -f1-2)"
    else
        echo -e "${RED}${BOLD}[✗]${NC} Rust installation verification failed"
        verification_failed=true
    fi
    
    # Check OpenSSL
    if command -v openssl >/dev/null 2>&1; then
        echo -e "${GREEN}${BOLD}[✓]${NC} OpenSSL: $(openssl version | cut -d' ' -f1-2)"
    else
        echo -e "${RED}${BOLD}[✗]${NC} OpenSSL verification failed"
        verification_failed=true
    fi
    
    # Check build tools
    if command -v gcc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1; then
        if command -v gcc >/dev/null 2>&1; then
            echo -e "${GREEN}${BOLD}[✓]${NC} GCC: $(gcc --version | head -n1 | cut -d' ' -f1-3)"
        else
            echo -e "${GREEN}${BOLD}[✓]${NC} Clang: $(clang --version | head -n1 | cut -d' ' -f1-3)"
        fi
    else
        echo -e "${RED}${BOLD}[✗]${NC} C compiler verification failed"
        verification_failed=true
    fi
    
    # Check pkg-config
    if command -v pkg-config >/dev/null 2>&1; then
        echo -e "${GREEN}${BOLD}[✓]${NC} pkg-config is available"
    else
        echo -e "${RED}${BOLD}[✗]${NC} pkg-config verification failed"
        verification_failed=true
    fi
    
    if $verification_failed; then
        echo -e "${RED}${BOLD}[✗]${NC} Some verifications failed"
        return 1
    else
        echo -e "${GREEN}${BOLD}[✓]${NC} All verifications passed"
        return 0
    fi
}

full_installation() {
    echo -e "${BRIGHT_YELLOW}${BOLD}📦 Full Installation Process${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    echo
    
    flash_text "🚀 Starting full installation..." "$BRIGHT_GREEN" 2
    
    # Step 1: Install all dependencies
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 1/5: System Dependencies & Rust${NC}"
    progress_bar 1 5 50 "Installation Progress"
    install_system_dependencies
    install_rust
    
    # Step 2: Install PostgreSQL
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 2/5: PostgreSQL Database${NC}"
    progress_bar 2 5 50 "Installation Progress"
    install_postgresql
    
    # Step 3: Setup Database
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 3/5: Database Configuration${NC}"
    progress_bar 3 5 50 "Installation Progress"
    setup_database
    
    # Step 4: Build Project
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 4/5: Project Build${NC}"
    progress_bar 4 5 50 "Installation Progress"
    build_project
    
    # Step 5: Final verification
    echo -e "\n${BRIGHT_BLUE}${BOLD}Step 5/5: Final Verification${NC}"
    progress_bar 5 5 50 "Installation Progress"
    verify_installation
    
    echo
    flash_text "🎉 Full Installation Complete!" "$BRIGHT_GREEN" 3
    echo
    echo -e "${GREEN}${BOLD}✅ GitHub Archiver is ready to use!${NC}"
    echo -e "${CYAN}You can now start the service from the main menu.${NC}"
    echo
    echo -e "${DIM}Press Enter to continue...${NC}"
    read -r
}

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                   🏥 HEALTH MONITORING FUNCTIONS                            │
# └─────────────────────────────────────────────────────────────────────────────┘

comprehensive_health_check() {
    echo -e "${BRIGHT_CYAN}${BOLD}🏥 COMPREHENSIVE SYSTEM HEALTH CHECK${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
    echo
    
    local health_score=0
    local max_score=12
    local start_time=$(date +%s)
    
    # System Resource Check
    echo -e "${YELLOW}${BOLD}📊 System Resources${NC}"
    echo -e "${YELLOW}────────────────────────────────────────${NC}"
    
    # Memory Check
    local mem_info=$(free | grep '^Mem:')
    local total_mem=$(echo $mem_info | awk '{print $2}')
    local used_mem=$(echo $mem_info | awk '{print $3}')
    local mem_usage=$((used_mem * 100 / total_mem))
    
    echo -e "${CYAN}Memory Usage:${NC} ${used_mem}KB / ${total_mem}KB (${mem_usage}%)"
    if [ $mem_usage -lt 80 ]; then
        echo -e "${GREEN}[✓] Memory usage healthy${NC}"
        ((health_score++))
    elif [ $mem_usage -lt 90 ]; then
        echo -e "${YELLOW}[⚠] Memory usage high${NC}"
    else
        echo -e "${RED}[✗] Memory usage critical${NC}"
    fi
    
    # Disk Space Check
    local disk_info=$(df -h . | tail -1)
    local disk_usage=$(echo $disk_info | awk '{print $5}' | sed 's/%//')
    local disk_avail=$(echo $disk_info | awk '{print $4}')
    
    echo -e "${CYAN}Disk Usage:${NC} ${disk_usage}% (${disk_avail} available)"
    if [ $disk_usage -lt 80 ]; then
        echo -e "${GREEN}[✓] Disk space healthy${NC}"
        ((health_score++))
    elif [ $disk_usage -lt 90 ]; then
        echo -e "${YELLOW}[⚠] Disk space getting low${NC}"
    else
        echo -e "${RED}[✗] Disk space critical${NC}"
    fi
    
    # CPU Load Check
    local load_avg=$(uptime | awk -F'load average:' '{ print $2 }' | awk '{ print $1 }' | sed 's/,//')
    local cpu_cores=$(nproc)
    local load_percent=$(echo "$load_avg $cpu_cores" | awk '{printf "%.0f", ($1/$2)*100}')
    
    echo -e "${CYAN}CPU Load:${NC} ${load_avg} (${load_percent}% of ${cpu_cores} cores)"
    if [ $load_percent -lt 70 ]; then
        echo -e "${GREEN}[✓] CPU load healthy${NC}"
        ((health_score++))
    elif [ $load_percent -lt 90 ]; then
        echo -e "${YELLOW}[⚠] CPU load high${NC}"
    else
        echo -e "${RED}[✗] CPU load critical${NC}"
    fi
    
    echo
    
    # Service Health Check
    echo -e "${YELLOW}${BOLD}🚀 Service Health${NC}"
    echo -e "${YELLOW}────────────────────────────────────────${NC}"
    
    # Check if service is running
    if [[ -f service.pid ]]; then
        local pid=$(cat service.pid)
        if kill -0 $pid 2>/dev/null; then
            echo -e "${GREEN}[✓] GitHub Archiver service is running (PID: $pid)${NC}"
            ((health_score++))
            
            # Check service memory usage
            local service_mem=$(ps -p $pid -o rss= 2>/dev/null | awk '{print $1}')
            echo -e "${CYAN}Service Memory:${NC} ${service_mem}KB"
            if [ $service_mem -lt 1048576 ]; then  # 1GB in KB
                echo -e "${GREEN}[✓] Service memory usage normal${NC}"
                ((health_score++))
            else
                echo -e "${YELLOW}[⚠] Service using high memory${NC}"
            fi
            
            # Test API health endpoint
            if command -v curl >/dev/null 2>&1; then
                local api_status=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 http://127.0.0.1:8081/health 2>/dev/null || echo "000")
                if [[ "$api_status" == "200" ]]; then
                    echo -e "${GREEN}[✓] API health endpoint responding${NC}"
                    ((health_score++))
                else
                    echo -e "${RED}[✗] API health endpoint not responding (HTTP $api_status)${NC}"
                fi
            fi
        else
            echo -e "${RED}[✗] Service PID file exists but process not running${NC}"
            rm -f service.pid
        fi
    else
        echo -e "${YELLOW}[⚠] GitHub Archiver service not running${NC}"
    fi
    
    echo
    
    # Database Health Check
    echo -e "${YELLOW}${BOLD}🗄️ Database Health${NC}"
    echo -e "${YELLOW}────────────────────────────────────────${NC}"
    
    if [[ -f .env ]]; then
        local db_url=$(grep "DATABASE_URL" .env 2>/dev/null | cut -d'=' -f2- | sed 's/"//g')
        if [[ -n "$db_url" ]] && command -v psql >/dev/null 2>&1; then
            if psql "$db_url" -c "SELECT 1;" >/dev/null 2>&1; then
                echo -e "${GREEN}[✓] Database connection successful${NC}"
                ((health_score++))
                
                # Check database size
                local db_size=$(psql "$db_url" -t -c "SELECT pg_size_pretty(pg_database_size(current_database()));" 2>/dev/null | xargs)
                echo -e "${CYAN}Database Size:${NC} ${db_size:-"N/A"}"
                
                # Check table count
                local table_count=$(psql "$db_url" -t -c "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public';" 2>/dev/null | xargs)
                echo -e "${CYAN}Tables:${NC} ${table_count:-"N/A"}"
                
                # Check recent activity
                local recent_events=$(psql "$db_url" -t -c "SELECT count(*) FROM github_events WHERE processed_at > NOW() - INTERVAL '1 hour';" 2>/dev/null | xargs)
                if [[ "$recent_events" =~ ^[0-9]+$ ]] && [ "$recent_events" -gt 0 ]; then
                    echo -e "${GREEN}[✓] Recent database activity detected (${recent_events} events in last hour)${NC}"
                    ((health_score++))
                else
                    echo -e "${YELLOW}[⚠] No recent database activity${NC}"
                fi
            else
                echo -e "${RED}[✗] Database connection failed${NC}"
            fi
        else
            echo -e "${YELLOW}[⚠] Database configuration incomplete${NC}"
        fi
    else
        echo -e "${YELLOW}[⚠] No .env configuration file found${NC}"
    fi
    
    echo
    
    # File System Check
    echo -e "${YELLOW}${BOLD}📁 File System Check${NC}"
    echo -e "${YELLOW}────────────────────────────────────────${NC}"
    
    # Check for required files
    local required_files=("Cargo.toml" "src/main.rs" "schema.sql")
    local files_ok=0
    for file in "${required_files[@]}"; do
        if [[ -f "$file" ]]; then
            echo -e "${GREEN}[✓] Found: $file${NC}"
            ((files_ok++))
        else
            echo -e "${RED}[✗] Missing: $file${NC}"
        fi
    done
    
    if [ $files_ok -eq ${#required_files[@]} ]; then
        ((health_score++))
        echo -e "${GREEN}[✓] All required files present${NC}"
    fi
    
    # Check log files
    if [[ -f service.log ]]; then
        local log_size=$(stat -f%z service.log 2>/dev/null || stat -c%s service.log 2>/dev/null || echo "0")
        local log_mb=$((log_size / 1024 / 1024))
        echo -e "${CYAN}Service Log Size:${NC} ${log_mb}MB"
        if [ $log_mb -lt 100 ]; then
            echo -e "${GREEN}[✓] Log file size reasonable${NC}"
            ((health_score++))
        else
            echo -e "${YELLOW}[⚠] Log file getting large (${log_mb}MB)${NC}"
        fi
    fi
    
    # Check backup directory
    if [[ -d backups ]]; then
        local backup_count=$(find backups -name "*.tar.gz" -mtime -7 | wc -l)
        if [ $backup_count -gt 0 ]; then
            echo -e "${GREEN}[✓] Recent backups found (${backup_count} in last 7 days)${NC}"
            ((health_score++))
        else
            echo -e "${YELLOW}[⚠] No recent backups found${NC}"
        fi
    else
        echo -e "${YELLOW}[⚠] No backup directory found${NC}"
    fi
    
    echo
    
    # Calculate and display health score
    local health_percentage=$((health_score * 100 / max_score))
    local end_time=$(date +%s)
    local check_duration=$((end_time - start_time))
    
    echo -e "${BRIGHT_BLUE}${BOLD}🏥 HEALTH SUMMARY${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${CYAN}Health Score:${NC} ${health_score}/${max_score} (${health_percentage}%)"
    echo -e "${CYAN}Check Duration:${NC} ${check_duration}s"
    echo -e "${CYAN}Timestamp:${NC} $(date)"
    
    # Health status indication
    if [ $health_percentage -ge 90 ]; then
        echo -e "${GREEN}${BOLD}🎉 EXCELLENT HEALTH${NC}"
        echo -e "${GREEN}All systems are operating optimally!${NC}"
    elif [ $health_percentage -ge 75 ]; then
        echo -e "${YELLOW}${BOLD}⚠️  GOOD HEALTH${NC}"
        echo -e "${YELLOW}System is healthy with minor issues.${NC}"
    elif [ $health_percentage -ge 50 ]; then
        echo -e "${YELLOW}${BOLD}⚠️  FAIR HEALTH${NC}"
        echo -e "${YELLOW}Several issues need attention.${NC}"
    else
        echo -e "${RED}${BOLD}🚨 POOR HEALTH${NC}"
        echo -e "${RED}Critical issues require immediate attention!${NC}"
    fi
    
    echo
    echo -e "${DIM}Press Enter to continue...${NC}"
    read -r
}

generate_system_report() {
    echo -e "${BRIGHT_PURPLE}${BOLD}📊 SYSTEM REPORT GENERATOR${NC}"
    echo -e "${PURPLE}═══════════════════════════════════════════════════════════════${NC}"
    echo
    
    local timestamp=$(date +"%Y%m%d_%H%M%S")
    local report_dir="reports"
    local report_file="$report_dir/system_report_$timestamp.md"
    
    # Create reports directory if it doesn't exist
    mkdir -p "$report_dir"
    
    echo -e "${CYAN}📝 Generating comprehensive system report...${NC}"
    echo
    
    # Generate report header
    cat > "$report_file" << EOF
# GitHub Archiver System Report
**Generated:** $(date)  
**Hostname:** $(hostname)  
**User:** $(whoami)  
**Working Directory:** $(pwd)

---

## 📊 System Overview

### System Information
- **OS:** $(uname -s) $(uname -r)
- **Architecture:** $(uname -m)
- **Uptime:** $(uptime -p 2>/dev/null || uptime)
- **Load Average:** $(uptime | awk -F'load average:' '{ print $2 }')

### Hardware Resources
- **CPU Cores:** $(nproc)
- **Total Memory:** $(free -h | grep '^Mem:' | awk '{print $2}')
- **Available Memory:** $(free -h | grep '^Mem:' | awk '{print $7}')
- **Disk Usage:** $(df -h . | tail -1 | awk '{print $3 "/" $2 " (" $5 " used)"}')

---

## 🚀 Service Status

EOF

    # Add service status
    if [[ -f service.pid ]]; then
        local pid=$(cat service.pid)
        if kill -0 $pid 2>/dev/null; then
            echo "### GitHub Archiver Service: ✅ RUNNING" >> "$report_file"
            echo "- **PID:** $pid" >> "$report_file"
            echo "- **Memory Usage:** $(ps -p $pid -o rss= 2>/dev/null | awk '{print int($1/1024) "MB"}')" >> "$report_file"
            echo "- **CPU Usage:** $(ps -p $pid -o %cpu= 2>/dev/null | awk '{print $1"%"}')" >> "$report_file"
            echo "- **Start Time:** $(ps -p $pid -o lstart= 2>/dev/null)" >> "$report_file"
        else
            echo "### GitHub Archiver Service: ❌ STOPPED" >> "$report_file"
            echo "- Service PID file exists but process not running" >> "$report_file"
        fi
    else
        echo "### GitHub Archiver Service: ⏹️ NOT RUNNING" >> "$report_file"
        echo "- No PID file found" >> "$report_file"
    fi
    
    echo "" >> "$report_file"
    echo "---" >> "$report_file"
    echo "" >> "$report_file"
    echo "## 🗄️ Database Status" >> "$report_file"
    echo "" >> "$report_file"
    
    # Database information
    if [[ -f .env ]]; then
        local db_url=$(grep "DATABASE_URL" .env 2>/dev/null | cut -d'=' -f2- | sed 's/"//g')
        if [[ -n "$db_url" ]] && command -v psql >/dev/null 2>&1; then
            if psql "$db_url" -c "SELECT 1;" >/dev/null 2>&1; then
                echo "### Database Connection: ✅ CONNECTED" >> "$report_file"
                
                local db_size=$(psql "$db_url" -t -c "SELECT pg_size_pretty(pg_database_size(current_database()));" 2>/dev/null | xargs)
                local table_count=$(psql "$db_url" -t -c "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public';" 2>/dev/null | xargs)
                local total_events=$(psql "$db_url" -t -c "SELECT count(*) FROM github_events;" 2>/dev/null | xargs)
                
                echo "- **Database Size:** ${db_size:-"N/A"}" >> "$report_file"
                echo "- **Table Count:** ${table_count:-"N/A"}" >> "$report_file"
                echo "- **Total Events:** ${total_events:-"N/A"}" >> "$report_file"
                
                # Recent activity
                local recent_events=$(psql "$db_url" -t -c "SELECT count(*) FROM github_events WHERE processed_at > NOW() - INTERVAL '24 hours';" 2>/dev/null | xargs)
                echo "- **Events (Last 24h):** ${recent_events:-"N/A"}" >> "$report_file"
            else
                echo "### Database Connection: ❌ FAILED" >> "$report_file"
                echo "- Cannot connect to database" >> "$report_file"
            fi
        else
            echo "### Database Connection: ⚠️ NOT CONFIGURED" >> "$report_file"
            echo "- Database URL not found or psql not available" >> "$report_file"
        fi
    else
        echo "### Database Connection: ⚠️ NO CONFIG" >> "$report_file"
        echo "- No .env file found" >> "$report_file"
    fi
    
    # Add file system information
    cat >> "$report_file" << EOF

---

## 📁 File System Analysis

### Project Structure
\`\`\`
$(find . -maxdepth 2 -type f -name "*.rs" -o -name "*.toml" -o -name "*.sql" -o -name "*.sh" -o -name "*.md" | head -20)
\`\`\`

### Log Files
EOF
    
    if [[ -f service.log ]]; then
        local log_size=$(stat -f%z service.log 2>/dev/null || stat -c%s service.log 2>/dev/null || echo "0")
        local log_mb=$((log_size / 1024 / 1024))
        echo "- **service.log:** ${log_mb}MB" >> "$report_file"
        echo "- **Last Modified:** $(stat -f%Sm service.log 2>/dev/null || stat -c%y service.log 2>/dev/null)" >> "$report_file"
    else
        echo "- No service log found" >> "$report_file"
    fi
    
    # Add backup information
    echo "" >> "$report_file"
    echo "### Backup Status" >> "$report_file"
    if [[ -d backups ]]; then
        local backup_count=$(find backups -name "*.tar.gz" 2>/dev/null | wc -l)
        local recent_backups=$(find backups -name "*.tar.gz" -mtime -7 2>/dev/null | wc -l)
        echo "- **Total Backups:** $backup_count" >> "$report_file"
        echo "- **Recent Backups (7 days):** $recent_backups" >> "$report_file"
        
        if [ $backup_count -gt 0 ]; then
            echo "- **Latest Backup:** $(find backups -name "*.tar.gz" -type f -exec stat -f%Sm {} \; 2>/dev/null | head -1 || find backups -name "*.tar.gz" -type f -exec stat -c%y {} \; 2>/dev/null | head -1)" >> "$report_file"
        fi
    else
        echo "- No backup directory found" >> "$report_file"
    fi
    
    # Add dependencies information
    cat >> "$report_file" << EOF

---

## 🔧 Dependencies & Environment

### Development Tools
EOF
    
    # Check development tools
    for tool in "rustc" "cargo" "git" "psql" "curl" "jq"; do
        if command -v "$tool" >/dev/null 2>&1; then
            local version=$($tool --version 2>/dev/null | head -1 || echo "Version unknown")
            echo "- **$tool:** ✅ $version" >> "$report_file"
        else
            echo "- **$tool:** ❌ Not installed" >> "$report_file"
        fi
    done
    
    # Add network information
    cat >> "$report_file" << EOF

### Network Configuration
- **Local IP:** $(hostname -I 2>/dev/null | awk '{print $1}' || echo "Unknown")
- **Public IP:** $(curl -s ifconfig.me 2>/dev/null || echo "Unknown")
- **Open Ports:** $(ss -tln | grep ":808[0-9]" | awk '{print $4}' | sed 's/.*://' | tr '\n' ' ' || echo "None detected")

---

## 📋 Configuration

### Environment Variables (.env)
EOF
    
    if [[ -f .env ]]; then
        echo "\`\`\`" >> "$report_file"
        # Sanitize sensitive information
        grep -v "PASSWORD\|SECRET\|TOKEN" .env >> "$report_file" 2>/dev/null || echo "Configuration file exists but couldn't read"
        echo "\`\`\`" >> "$report_file"
    else
        echo "- No .env file found" >> "$report_file"
    fi
    
    # Add report footer
    cat >> "$report_file" << EOF

---

## 🏥 Health Check Summary

**Report Generated Successfully**  
**Location:** \`$report_file\`  
**Size:** $(stat -f%z "$report_file" 2>/dev/null || stat -c%s "$report_file" 2>/dev/null)B  

> This report provides a comprehensive overview of the GitHub Archiver system status.
> Use this information for troubleshooting, monitoring, and maintenance purposes.

---
*Generated by GitHub Archiver Setup Script v3.0.0*
EOF
    
    echo -e "${GREEN}✅ Report generated successfully!${NC}"
    echo -e "${CYAN}📍 Location: ${report_file}${NC}"
    echo -e "${CYAN}📏 Size: $(stat -f%z "$report_file" 2>/dev/null || stat -c%s "$report_file" 2>/dev/null)B${NC}"
    echo
    
    # Ask if user wants to view the report
    echo -e "${YELLOW}Would you like to view the report now? [y/N]: ${NC}"
    read -r view_choice
    if [[ "$view_choice" =~ ^[Yy]$ ]]; then
        if command -v less >/dev/null 2>&1; then
            less "$report_file"
        elif command -v more >/dev/null 2>&1; then
            more "$report_file"
        else
            cat "$report_file"
        fi
    fi
    
    echo
    echo -e "${DIM}Press Enter to continue...${NC}"
    read -r
}

backup_system() {
    echo -e "${BRIGHT_YELLOW}${BOLD}💾 SYSTEM BACKUP UTILITY${NC}"
    echo -e "${YELLOW}═══════════════════════════════════════════════════════════════${NC}"
    echo
    
    local timestamp=$(date +"%Y%m%d_%H%M%S")
    local backup_dir="backups"
    local temp_dir="/tmp/github_archiver_backup_$$"
    
    # Create backup directory if it doesn't exist
    mkdir -p "$backup_dir"
    mkdir -p "$temp_dir"
    
    echo -e "${CYAN}🎯 Backup Options:${NC}"
    echo -e "${WHITE}[1] Full system backup (code + database + logs)${NC}"
    echo -e "${WHITE}[2] Code-only backup (source code + config)${NC}"
    echo -e "${WHITE}[3] Database-only backup${NC}"
    echo -e "${WHITE}[4] Configuration backup${NC}"
    echo -e "${WHITE}[0] Cancel${NC}"
    echo
    echo -e "${YELLOW}Select backup type [0-4]: ${NC}"
    read -r backup_choice
    
    case $backup_choice in
        1)
            echo -e "${CYAN}📦 Creating full system backup...${NC}"
            create_full_backup "$timestamp" "$backup_dir" "$temp_dir"
            ;;
        2)
            echo -e "${CYAN}📦 Creating code-only backup...${NC}"
            create_code_backup "$timestamp" "$backup_dir" "$temp_dir"
            ;;
        3)
            echo -e "${CYAN}📦 Creating database backup...${NC}"
            create_database_backup "$timestamp" "$backup_dir"
            ;;
        4)
            echo -e "${CYAN}📦 Creating configuration backup...${NC}"
            create_config_backup "$timestamp" "$backup_dir" "$temp_dir"
            ;;
        0)
            echo -e "${YELLOW}Backup cancelled.${NC}"
            return 0
            ;;
        *)
            echo -e "${RED}Invalid choice!${NC}"
            return 1
            ;;
    esac
    
    # Cleanup temp directory
    rm -rf "$temp_dir"
}

create_full_backup() {
    local timestamp="$1"
    local backup_dir="$2"
    local temp_dir="$3"
    local backup_name="full_backup_$timestamp"
    local backup_path="$backup_dir/${backup_name}.tar.gz"
    
    echo -e "${BLUE}🔍 Collecting system data...${NC}"
    
    # Create backup structure
    mkdir -p "$temp_dir/$backup_name"/{code,config,logs,database}
    
    # Backup source code
    echo -e "${CYAN}  📋 Backing up source code...${NC}"
    cp -r src "$temp_dir/$backup_name/code/" 2>/dev/null || echo "No src directory found"
    cp Cargo.toml "$temp_dir/$backup_name/code/" 2>/dev/null || echo "No Cargo.toml found"
    cp Cargo.lock "$temp_dir/$backup_name/code/" 2>/dev/null || echo "No Cargo.lock found"
    cp *.rs "$temp_dir/$backup_name/code/" 2>/dev/null || echo "No .rs files in root"
    cp *.sh "$temp_dir/$backup_name/code/" 2>/dev/null || echo "No .sh files found"
    cp *.md "$temp_dir/$backup_name/code/" 2>/dev/null || echo "No .md files found"
    
    # Backup configuration
    echo -e "${CYAN}  ⚙️  Backing up configuration...${NC}"
    cp .env "$temp_dir/$backup_name/config/" 2>/dev/null || echo "No .env file found"
    cp schema.sql "$temp_dir/$backup_name/config/" 2>/dev/null || echo "No schema.sql found"
    
    # Backup logs
    echo -e "${CYAN}  📋 Backing up logs...${NC}"
    cp *.log "$temp_dir/$backup_name/logs/" 2>/dev/null || echo "No log files found"
    
    # Backup database if possible
    echo -e "${CYAN}  🗄️  Backing up database...${NC}"
    if [[ -f .env ]] && command -v pg_dump >/dev/null 2>&1; then
        local db_url=$(grep "DATABASE_URL" .env 2>/dev/null | cut -d'=' -f2- | sed 's/"//g')
        if [[ -n "$db_url" ]]; then
            if pg_dump "$db_url" > "$temp_dir/$backup_name/database/github_archiver.sql" 2>/dev/null; then
                echo -e "${GREEN}    ✅ Database backup successful${NC}"
            else
                echo -e "${YELLOW}    ⚠️  Database backup failed${NC}"
            fi
        fi
    else
        echo -e "${YELLOW}    ⚠️  Database backup skipped (pg_dump not available)${NC}"
    fi
    
    # Create metadata
    cat > "$temp_dir/$backup_name/backup_info.txt" << EOF
GitHub Archiver Full Backup
Created: $(date)
Hostname: $(hostname)
User: $(whoami)
Backup Type: Full System Backup
Working Directory: $(pwd)
Git Commit: $(git rev-parse HEAD 2>/dev/null || echo "Not a git repository")
EOF
    
    # Create compressed archive
    echo -e "${CYAN}  🗜️  Compressing backup...${NC}"
    (cd "$temp_dir" && tar -czf "../$backup_path" "$backup_name")
    
    if [[ -f "$backup_path" ]]; then
        local backup_size=$(stat -f%z "$backup_path" 2>/dev/null || stat -c%s "$backup_path" 2>/dev/null)
        local backup_mb=$((backup_size / 1024 / 1024))
        
        echo -e "${GREEN}✅ Full backup completed successfully!${NC}"
        echo -e "${CYAN}📍 Location: ${backup_path}${NC}"
        echo -e "${CYAN}📏 Size: ${backup_mb}MB${NC}"
    else
        echo -e "${RED}❌ Backup failed!${NC}"
    fi
}

create_code_backup() {
    local timestamp="$1"
    local backup_dir="$2"
    local temp_dir="$3"
    local backup_name="code_backup_$timestamp"
    local backup_path="$backup_dir/${backup_name}.tar.gz"
    
    echo -e "${BLUE}📋 Collecting source code...${NC}"
    
    mkdir -p "$temp_dir/$backup_name"
    
    # Copy source files
    cp -r src "$temp_dir/$backup_name/" 2>/dev/null || echo "No src directory"
    cp Cargo.toml "$temp_dir/$backup_name/" 2>/dev/null
    cp Cargo.lock "$temp_dir/$backup_name/" 2>/dev/null
    cp *.rs "$temp_dir/$backup_name/" 2>/dev/null || true
    cp *.sh "$temp_dir/$backup_name/" 2>/dev/null || true
    cp *.md "$temp_dir/$backup_name/" 2>/dev/null || true
    cp .env "$temp_dir/$backup_name/" 2>/dev/null || true
    cp schema.sql "$temp_dir/$backup_name/" 2>/dev/null || true
    
    # Create archive
    (cd "$temp_dir" && tar -czf "../$backup_path" "$backup_name")
    
    if [[ -f "$backup_path" ]]; then
        local backup_size=$(stat -f%z "$backup_path" 2>/dev/null || stat -c%s "$backup_path" 2>/dev/null)
        local backup_mb=$((backup_size / 1024 / 1024))
        
        echo -e "${GREEN}✅ Code backup completed!${NC}"
        echo -e "${CYAN}📍 Location: ${backup_path}${NC}"
        echo -e "${CYAN}📏 Size: ${backup_mb}MB${NC}"
    else
        echo -e "${RED}❌ Code backup failed!${NC}"
    fi
}

create_database_backup() {
    local timestamp="$1"
    local backup_dir="$2"
    local backup_path="$backup_dir/database_backup_$timestamp.sql"
    
    if [[ -f .env ]] && command -v pg_dump >/dev/null 2>&1; then
        local db_url=$(grep "DATABASE_URL" .env 2>/dev/null | cut -d'=' -f2- | sed 's/"//g')
        if [[ -n "$db_url" ]]; then
            echo -e "${BLUE}🗄️  Exporting database...${NC}"
            if pg_dump "$db_url" > "$backup_path" 2>/dev/null; then
                # Compress the SQL file
                gzip "$backup_path"
                backup_path="${backup_path}.gz"
                
                local backup_size=$(stat -f%z "$backup_path" 2>/dev/null || stat -c%s "$backup_path" 2>/dev/null)
                local backup_mb=$((backup_size / 1024 / 1024))
                
                echo -e "${GREEN}✅ Database backup completed!${NC}"
                echo -e "${CYAN}📍 Location: ${backup_path}${NC}"
                echo -e "${CYAN}📏 Size: ${backup_mb}MB${NC}"
            else
                echo -e "${RED}❌ Database backup failed!${NC}"
                rm -f "$backup_path"
            fi
        else
            echo -e "${RED}❌ No database URL found in .env${NC}"
        fi
    else
        echo -e "${RED}❌ pg_dump not available or no .env file${NC}"
    fi
}

create_config_backup() {
    local timestamp="$1"
    local backup_dir="$2"
    local temp_dir="$3"
    local backup_name="config_backup_$timestamp"
    local backup_path="$backup_dir/${backup_name}.tar.gz"
    
    mkdir -p "$temp_dir/$backup_name"
    
    # Backup configuration files
    cp .env "$temp_dir/$backup_name/" 2>/dev/null || echo "No .env file"
    cp schema.sql "$temp_dir/$backup_name/" 2>/dev/null || echo "No schema.sql"
    cp *.toml "$temp_dir/$backup_name/" 2>/dev/null || echo "No .toml files"
    
    # Create archive
    (cd "$temp_dir" && tar -czf "../$backup_path" "$backup_name")
    
    if [[ -f "$backup_path" ]]; then
        echo -e "${GREEN}✅ Configuration backup completed!${NC}"
        echo -e "${CYAN}📍 Location: ${backup_path}${NC}"
    else
        echo -e "${RED}❌ Configuration backup failed!${NC}"
    fi
}

# ┌─────────────────────────────────────────────────────────────────────────────┐
# │                        🎯 MAIN PROGRAM                                      │
# └─────────────────────────────────────────────────────────────────────────────┘

main() {
    # Check if we're in the right directory
    if [[ ! -f "Cargo.toml" ]]; then
        echo -e "${RED}${BOLD}[✗]${NC} Error: This script must be run from the project root directory"
        echo -e "${YELLOW}${BOLD}[!]${NC} Please run: cd /path/to/github-archiver && ./setup.sh"
        exit 1
    fi
    
    # Main menu loop
    while true; do
        show_banner
        show_main_menu
        
        read -r choice
        
        case $choice in
            1)
                clear_screen
                check_environment
                echo
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            2)
                clear_screen
                install_all_dependencies
                ;;
            3)
                clear_screen
                full_installation
                ;;
            4)
                clear_screen
                build_project
                echo
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            5)
                clear_screen
                start_service
                echo
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            6)
                clear_screen
                stop_service
                echo
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            7)
                clear_screen
                restart_service
                echo
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            8)
                clear_screen
                service_status
                echo
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            9)
                clear_screen
                view_logs
                ;;
            10)
                clear_screen
                edit_config
                ;;
            [Aa])
                clear_screen
                show_system_info
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            [Bb])
                clear_screen
                comprehensive_health_check
                ;;
            [Cc])
                clear_screen
                generate_system_report
                ;;
            [Dd])
                clear_screen
                backup_system
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            0)
                clear_screen
                echo
                flash_text "👋 Thank you for using GitHub Archiver!" "$BRIGHT_BLUE" 2
                echo -e "${DIM}Goodbye!${NC}"
                echo
                exit 0
                ;;
            *)
                echo -e "${RED}${BOLD}Invalid choice! Please enter a number between 0-10 or A-D.${NC}"
                sleep 1
                ;;
        esac
    done
}

# Script entry point
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
