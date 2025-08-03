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
    echo -e "${CYAN}💾 Memory:${NC} $(free -h | awk 'NR==2{printf "%.1f/%.1fGB (%.2f%%)", $3/1024/1024, $2/1024/1024, $3*100/$2}' 2>/dev/null || echo "N/A")"
    echo -e "${CYAN}💽 Disk:${NC} $(df -h / | awk 'NR==2{printf "%s/%s (%s)", $3, $2, $5}' 2>/dev/null || echo "N/A")"
    echo -e "${CYAN}⚡ CPU:${NC} $(nproc) cores"
    echo -e "${CYAN}🌐 Network:${NC} $(hostname -I | awk '{print $1}' 2>/dev/null || echo "localhost")"
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
    echo -e "${CYAN}🔄 Updating package list...${NC}"
    sudo pacman -Syu --noconfirm
    
    # Install essential packages
    echo -e "${CYAN}🔧 Installing build tools...${NC}"
    sudo pacman -S --noconfirm \
        base-devel \
        curl \
        wget \
        git \
        pkg-config \
        openssl \
        libffi \
        gmp \
        sqlite \
        postgresql-libs \
        jq \
        unzip \
        ca-certificates
    
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
    echo -e "${BRIGHT_GREEN}${BOLD}🚀 Starting GitHub Archiver...${NC}"
    
    # Check if already running
    if pgrep -f "target/release/rust-github-archiver" >/dev/null; then
        echo -e "${YELLOW}${BOLD}[!]${NC} Service is already running"
        return 0
    fi
    
    # Start the service
    echo -e "${CYAN}⚡ Starting server on port 8081...${NC}"
    nohup ./target/release/rust-github-archiver > service.log 2>&1 &
    local pid=$!
    echo $pid > service.pid
    
    sleep 2
    
    # Check if it started successfully
    if kill -0 $pid 2>/dev/null; then
        echo -e "${GREEN}${BOLD}[✓]${NC} Service started successfully (PID: $pid)"
        echo -e "${CYAN}🌐 Server running at: ${BOLD}http://127.0.0.1:8081${NC}"
    else
        echo -e "${RED}${BOLD}[✗]${NC} Failed to start service"
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
        local pid=$(pgrep -f "target/release/rust-github-archiver")
        if [[ -n $pid ]]; then
            kill $pid
            echo -e "${GREEN}${BOLD}[✓]${NC} Service stopped (PID: $pid)"
        else
            echo -e "${YELLOW}${BOLD}[!]${NC} Service is not running"
        fi
    fi
}

service_status() {
    echo -e "${BRIGHT_BLUE}${BOLD}📊 Service Status${NC}"
    echo -e "${CYAN}═══════════════════════════════════════${NC}"
    
    if [[ -f service.pid ]]; then
        local pid=$(cat service.pid)
        if kill -0 $pid 2>/dev/null; then
            echo -e "${GREEN}${BOLD}Status:${NC} Running"
            echo -e "${GREEN}${BOLD}PID:${NC} $pid"
            echo -e "${GREEN}${BOLD}Port:${NC} 8081"
            echo -e "${GREEN}${BOLD}URL:${NC} http://127.0.0.1:8081"
            
            # Memory usage
            local mem=$(ps -p $pid -o rss= 2>/dev/null | awk '{print $1/1024 " MB"}')
            echo -e "${GREEN}${BOLD}Memory:${NC} ${mem:-"N/A"}"
            
            # Uptime
            local start_time=$(ps -p $pid -o lstart= 2>/dev/null)
            echo -e "${GREEN}${BOLD}Started:${NC} ${start_time:-"N/A"}"
        else
            echo -e "${RED}${BOLD}Status:${NC} Stopped (stale PID file)"
            rm service.pid
        fi
    else
        local pid=$(pgrep -f "target/release/rust-github-archiver")
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
    echo
    gradient_text "    ███████╗██╗████████╗██╗  ██╗██╗   ██╗██████╗      █████╗ ██████╗  ██████╗██╗  ██╗██╗██╗   ██╗███████╗██████╗ "
    gradient_text "    ██╔════╝██║╚══██╔══╝██║  ██║██║   ██║██╔══██╗    ██╔══██╗██╔══██╗██╔════╝██║  ██║██║██║   ██║██╔════╝██╔══██╗"
    gradient_text "    ██║  ███╗██║   ██║   ███████║██║   ██║██████╔╝    ███████║██████╔╝██║     ███████║██║██║   ██║█████╗  ██████╔╝"
    gradient_text "    ██║   ██║██║   ██║   ██╔══██║██║   ██║██╔══██╗    ██╔══██║██╔══██╗██║     ██╔══██║██║╚██╗ ██╔╝██╔══╝  ██╔══██╗"
    gradient_text "    ╚██████╔╝██║   ██║   ██║  ██║╚██████╔╝██████╔╝    ██║  ██║██║  ██║╚██████╗██║  ██║██║ ╚████╔╝ ███████╗██║  ██║"
    gradient_text "     ╚═════╝ ╚═╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ╚═════╝     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═╝"
    echo
    echo -e "${BRIGHT_CYAN}${BOLD}                           🚀 Professional Interactive Setup & Management System${NC}"
    echo -e "${DIM}                                      Version 3.0.0 | Rust Edition${NC}"
    echo
}

show_main_menu() {
    echo -e "${BRIGHT_BLUE}${BOLD}┌─────────────────────────────────────────────┐"
    echo -e "│                MAIN MENU                    │"
    echo -e "└─────────────────────────────────────────────┘${NC}"
    echo
    echo -e "${BRIGHT_WHITE}${BOLD}[1]${NC} 🔍 ${CYAN}System Check${NC}        - Check dependencies and environment"
    echo -e "${BRIGHT_WHITE}${BOLD}[2]${NC} � ${BRIGHT_GREEN}Install Dependencies${NC} - Install Rust, OpenSSL & build tools"
    echo -e "${BRIGHT_WHITE}${BOLD}[3]${NC} �📦 ${YELLOW}Full Installation${NC}   - Install all dependencies and setup"
    echo -e "${BRIGHT_WHITE}${BOLD}[4]${NC} 🔨 ${BLUE}Build Project${NC}       - Compile the Rust application"
    echo -e "${BRIGHT_WHITE}${BOLD}[5]${NC} 🚀 ${GREEN}Start Service${NC}       - Start the GitHub Archiver service"
    echo -e "${BRIGHT_WHITE}${BOLD}[6]${NC} 🛑 ${RED}Stop Service${NC}        - Stop the running service"
    echo -e "${BRIGHT_WHITE}${BOLD}[7]${NC} 📊 ${PURPLE}Service Status${NC}     - Check service status and info"
    echo -e "${BRIGHT_WHITE}${BOLD}[8]${NC} 📋 ${CYAN}View Logs${NC}          - Display service logs"
    echo -e "${BRIGHT_WHITE}${BOLD}[9]${NC} ⚙️  ${YELLOW}Configuration${NC}      - Edit configuration files"
    echo -e "${BRIGHT_WHITE}${BOLD}[A]${NC} 📊 ${BLUE}System Info${NC}        - Display system information"
    echo -e "${BRIGHT_WHITE}${BOLD}[0]${NC} 🚪 ${DIM}Exit${NC}               - Exit the setup script"
    echo
    echo -e "${BRIGHT_WHITE}${BOLD}Enter your choice [0-9,A]:${NC} "
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
                service_status
                echo
                echo -e "${DIM}Press Enter to continue...${NC}"
                read -r
                ;;
            8)
                clear_screen
                view_logs
                ;;
            9)
                clear_screen
                edit_config
                ;;
            [Aa])
                clear_screen
                show_system_info
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
                echo -e "${RED}${BOLD}Invalid choice! Please enter a number between 0-9 or A.${NC}"
                sleep 1
                ;;
        esac
    done
}

# Script entry point
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
