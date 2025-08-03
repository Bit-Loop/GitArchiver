# 🚀 GitArchiver - GitHub Archive Data Scraper & Analyzer

A comprehensive, production-ready GitHub Archive data scraper and analyzer built with **Rust** for maximum performance and safety. Designed for cloud infrastructure with advanced resource management, real-time monitoring, and intelligent data processing capabilities.

## 🎯 Overview

GitArchiver is a sophisticated **Rust-based system** that downloads, processes, and analyzes GitHub Archive data from https://data.gharchive.org/. It provides a complete solution for researchers, security professionals, and developers who need to analyze GitHub activity patterns, track repository changes, and generate actionable insights from GitHub's public event stream.

## ✨ Key Features

### 🦀 Rust-Powered Performance
- **Memory Safety**: Zero-cost abstractions with guaranteed memory safety
- **Async Processing**: High-performance concurrent downloads and processing
- **Type Safety**: Compile-time guarantees for data integrity
- **Cross-Platform**: Runs on any Linux distribution with automatic dependency detection

### 🛡️ Cloud Optimized
- **Resource Management**: Intelligent memory, CPU, and disk usage monitoring
- **Safety Limits**: Automatic throttling and cleanup to prevent system overload
- **Emergency Protocols**: Graceful degradation under resource pressure
- **Cost Optimization**: Efficient resource utilization for cloud environments

### 📊 Data Processing Engine
- **Parallel Downloads**: Concurrent processing with configurable limits
- **ETag Support**: Rsync-like behavior to avoid re-downloading unchanged files
- **PostgreSQL Integration**: Optimized database schema for GitHub events
- **Batch Processing**: Efficient data ingestion with configurable batch sizes
- **Data Validation**: Comprehensive error handling and data integrity checks

### 🌐 Professional Web Interface
- **JWT Authentication**: Secure token-based authentication system
- **Real-time API**: RESTful endpoints with WebSocket support
- **Resource Monitoring**: Live system resource tracking
- **Event Management**: Comprehensive GitHub event processing
- **Admin Interface**: User management and system configuration

### 🔍 Advanced Analytics
- **Event Analysis**: Comprehensive GitHub event type tracking
- **Repository Insights**: Repository change monitoring and statistics
- **Security Research**: Advanced data mining capabilities
- **Trend Analysis**: Historical data analysis and pattern recognition

### 🔐 Security & Authentication
- **JWT Tokens**: Secure authentication with configurable expiration
- **Password Hashing**: Argon2 password hashing for security
- **Rate Limiting**: Intelligent GitHub API rate limit management
- **Audit Logging**: Comprehensive activity logging and monitoring

## 🏗️ Architecture

```
GitArchiver/
├── Core Components (Rust)/
│   ├── src/core/
│   │   ├── config.rs              # Configuration management
│   │   └── database.rs            # PostgreSQL integration
│   ├── src/auth/
│   │   ├── jwt.rs                 # JWT token management
│   │   ├── users.rs               # User management
│   │   └── middleware.rs          # Authentication middleware
│   ├── src/api/
│   │   ├── server.rs              # Axum web server
│   │   ├── routes.rs              # API routing
│   │   └── handlers.rs            # Request handlers
│   └── src/scraper/
│       ├── archive_scraper.rs     # GitHub Archive scraper
│       ├── file_processor.rs      # File processing engine
│       └── downloader.rs          # Concurrent downloader
├── Setup & Management/
│   ├── setup.sh                   # Professional interactive setup
│   ├── start.sh                   # Launcher script
│   └── Cargo.toml                 # Rust dependencies
├── Configuration/
│   ├── .env                       # Environment configuration
│   └── .env.example               # Configuration template
└── Data & Logs/
    ├── gharchive_data/            # Downloaded GitHub Archive files
    ├── logs/                      # Application logs
    └── target/release/            # Compiled Rust binary
```

## 🚀 Quick Start

### Prerequisites

- **Any Linux Distribution** (Ubuntu, CentOS, Fedora, Arch, Alpine, etc.)
- **PostgreSQL 12+** (can be installed automatically)
- **8GB+ RAM** (recommended for optimal performance)
- **20GB+ disk space** for data storage

### One-Command Installation

The enhanced setup script automatically detects your Linux distribution and installs all required dependencies:

```bash
git clone https://github.com/Bit-Loop/GitArchiver.git
cd GitArchiver
./start.sh
```

### Interactive Setup Menu

The professional setup script provides a comprehensive installation experience:

- **🔍 System Check** - Verify all dependencies and environment
- **🔧 Install Dependencies** - Auto-install Rust, OpenSSL, build tools for any Linux distro
- **📦 Full Installation** - Complete automated setup with progress tracking
- **🔨 Build Project** - Compile the optimized Rust binary
- **🚀 Service Management** - Start/stop/monitor the application
- **📊 System Monitoring** - Real-time resource usage and status

### Supported Linux Distributions

The setup script automatically detects and supports:
- **Ubuntu/Debian** - `apt-get` package management
- **RHEL/CentOS/Fedora** - `dnf/yum` package management  
- **Arch Linux/Manjaro** - `pacman` package management
- **openSUSE/SLES** - `zypper` package management
- **Alpine Linux** - `apk` package management
- **Generic Linux** - Manual installation guidance

### Manual Installation (Advanced Users)

If you prefer manual installation:

1. **Install system dependencies:**
```bash
# Ubuntu/Debian
sudo apt-get install build-essential curl git pkg-config libssl-dev jq

# RHEL/CentOS/Fedora
sudo dnf install gcc gcc-c++ curl git pkg-config openssl-devel jq

# Arch Linux
sudo pacman -S base-devel curl git pkg-config openssl jq
```

2. **Install Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

3. **Build the project:**
```bash
cd GitArchiver/rust_github_archiver
cargo build --release
```

### Configuration

The application uses a `.env` file for configuration (automatically created during setup):

```bash
# Database Configuration
DATABASE_URL=postgresql://github_archiver:secure_password@localhost/github_archiver

# JWT Authentication
JWT_SECRET=your-super-secure-jwt-secret-key-change-this-in-production

# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8081

# GitHub API (optional, for higher rate limits)
GITHUB_TOKEN=your_github_token_here

# Resource Limits (auto-configured based on system)
MAX_CONCURRENT_DOWNLOADS=6
BATCH_SIZE=500
REQUEST_TIMEOUT=180

# Storage
DOWNLOAD_DIR=./gharchive_data
```

## 🎮 Usage

### Web API and Authentication

Start the service:

```bash
./start.sh
# Then select option 5 (Start Service)
```

Access the API at: `http://localhost:8081`

**Authentication Endpoints:**
- `POST /auth/login` - User login
- `POST /auth/logout` - User logout  
- `GET /auth/user` - Get current user info

**Default Admin User:**
- Username: `admin`
- Password: `admin123` (change after first login)

### API Endpoints

**Health & Status:**
- `GET /health` - Service health check
- `GET /api/status` - System status and metrics

**Authentication Required:**
- `GET /api/events` - Search and retrieve GitHub events
- `POST /api/scraper/start` - Start scraper
- `POST /api/scraper/stop` - Stop scraper
- `GET /api/stats` - Database statistics

### Command Line Interface

**Service Management:**
```bash
# Start the complete application
./start.sh

# Quick actions
cargo run --release  # Direct binary execution
```

**Interactive Setup Menu:**
- System dependency checking and installation
- Automatic PostgreSQL setup
- Build optimization
- Service monitoring
- Configuration management

## 📊 Data Schema

The system stores GitHub events in an optimized PostgreSQL schema:

### Main Tables

- **`github_events`**: Core GitHub events with full event data
- **`repositories`**: Repository information and metadata
- **`processed_files`**: Tracking of processed archive files
- **`system_metrics`**: Resource usage and performance metrics

### Event Types Supported

- Push events
- Pull request events
- Issue events
- Fork events
- Watch/Star events
- Release events
- Create/Delete events
- And 20+ other GitHub event types

## 🔧 Advanced Features

### Wordlist Generation

Generate security research wordlists from GitHub data:

```bash
python3 wordlist_generator.py --type endpoints --min-frequency 10
python3 wordlist_generator.py --type subdomains --organization target-org
```

### Data Import/Export

Import data from multiple sources:

```bash
# Import from JSON files
python3 data_importer.py --source json --path ./data/

# Import from BigQuery
python3 data_importer.py --source bigquery --query "SELECT * FROM dataset.table"
```

### System Monitoring

Real-time system monitoring:

```bash
# Start system monitor
python3 system_monitor.py --interval 60

# Generate system report
python3 system_monitor.py --report
```

## 🛡️ Oracle Cloud Safety Features

### Resource Management
- **Memory Monitoring**: Automatic cleanup at 90% usage
- **Disk Management**: Cleanup of temporary files and old logs
- **CPU Throttling**: Process throttling at high CPU usage
- **Emergency Protocols**: Graceful shutdown under resource pressure

### Cost Optimization
- **Efficient Downloads**: ETag support to avoid re-downloading
- **Batch Processing**: Optimized database operations
- **Resource Limits**: Configurable limits for different instance sizes
- **Automated Cleanup**: Regular cleanup of temporary data

## 📈 Monitoring & Logging

### Real-time Monitoring
- System resource usage (CPU, memory, disk)
- Scraper performance metrics
- Database query performance
- API response times

### Comprehensive Logging
- Application logs with rotation
- Error tracking and alerting
- Performance metrics collection
- Audit trail for all operations

### Dashboard Analytics
- Event processing rates
- Resource usage trends
- Error rate monitoring
- Data quality metrics

## 🔍 Search & Analytics

### Event Search
```bash
# Search by repository
python3 db_manager.py --search "repo:microsoft/vscode"

# Search by event type
python3 db_manager.py --search "type:PushEvent"

# Complex queries
python3 db_manager.py --search "type:PullRequestEvent AND created:>2024-01-01"
```

### Analytics Queries
- Top repositories by activity
- Event type distributions
- User activity patterns
- Repository growth trends

## 🚨 Troubleshooting

### Common Issues

**High Memory Usage:**
```bash
# Check current usage
python3 system_monitor.py --check-memory

# Force cleanup
python3 api.py --emergency-cleanup
```

**Database Connection Issues:**
```bash
# Test database connection
python3 db_manager.py --test-connection

# Reset database
python3 db_manager.py --reset-db
```

**Rate Limiting:**
```bash
# Check rate limit status
python3 rate_limiter.py --status

# Wait for reset
python3 rate_limiter.py --wait-for-reset
```

## 📝 API Documentation

### REST Endpoints

- `GET /api/status` - System status and metrics
- `GET /api/events` - Search and retrieve events
- `POST /api/scraper/start` - Start scraper
- `POST /api/scraper/stop` - Stop scraper
- `GET /api/stats` - Database statistics
- `GET /api/logs` - Application logs

### WebSocket Endpoints

- `/ws/logs` - Real-time log streaming
- `/ws/metrics` - Real-time metrics updates
- `/ws/events` - Live event stream

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- GitHub Archive team for providing the data
- PostgreSQL community for the excellent database
- Oracle Cloud for the infrastructure platform
- Open source community for the tools and libraries

## 📞 Support

For support, issues, or feature requests:
- Open an issue on GitHub
- Check the troubleshooting section
- Review the logs in the dashboard

---

**Built with ❤️ for the open source community**