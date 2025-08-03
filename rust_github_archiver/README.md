# GitHub Archive Scraper - Rust Edition

<div align="center">

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-316192?style=for-the-badge&logo=postgresql&logoColor=white)
![GitHub](https://img.shields.io/badge/GitHub-100000?style=for-the-badge&logo=github&logoColor=white)

**⚡ High-Performance GitHub Archive Processing with Resource Management**

*Professional-grade scraper built for Oracle Cloud Free Tier with comprehensive monitoring*

</div>

## 🚀 Features

### Core Functionality
- **📊 GitHub Archive Processing**: Complete processing of GHArchive.org data
- **🗄️ PostgreSQL Integration**: Advanced database management with connection pooling
- **⚡ Concurrent Downloads**: Multi-threaded downloads with retry mechanisms
- **📈 Resource Monitoring**: Real-time system resource tracking and emergency cleanup
- **🌐 Web Dashboard**: Interactive web interface with real-time metrics
- **🔐 Authentication**: Secure JWT-based authentication system
- **📝 Comprehensive Logging**: Structured logging with rotation and archival

### Performance Features
- **🔄 Async/Await**: Full async implementation for maximum performance
- **🏃 Parallel Processing**: Multi-threaded event processing and file handling
- **💾 Memory Management**: Intelligent memory usage with automatic cleanup
- **🎯 Resource Limits**: Oracle Cloud optimized with configurable limits
- **⏱️ Rate Limiting**: GitHub API rate limit management
- **🔄 Auto-Recovery**: Automatic recovery from failures and resource exhaustion

## 📋 Requirements

### System Requirements
- **OS**: Linux (Ubuntu 20.04+ recommended)
- **Memory**: 4GB+ (18GB limit for Oracle Cloud)
- **Storage**: 20GB+ available space
- **CPU**: 2+ cores (ARM64 and x86_64 supported)

### Software Dependencies
- **Rust**: 1.75.0+ (latest stable)
- **PostgreSQL**: 12.0+
- **Git**: For cloning and updates
- **OpenSSL**: For secure communications

## 🛠️ Quick Start

### 1. Clone and Setup
```bash
git clone <repository-url>
cd rust_github_archiver

# Run interactive setup
./setup.sh
```

### 2. Configure Environment
```bash
# Edit configuration (created by setup)
nano .env

# Key settings to review:
# DB_HOST, DB_NAME, DB_USER, DB_PASSWORD
# GITHUB_TOKEN (optional, for higher rate limits)
# MEMORY_LIMIT_GB, DISK_LIMIT_GB
```

### 3. Start Services
```bash
# Start all services
./run.sh start

# Check status
./run.sh status

# View logs
./run.sh logs -f
```

### 4. Access Web Interface
```
http://localhost:8081
Username: admin
Password: admin123 (change in .env)
```

## 🔧 Configuration

### Environment Variables

#### Database Configuration
```bash
DB_HOST=localhost                    # PostgreSQL host
DB_PORT=5432                        # PostgreSQL port
DB_NAME=github_archiver             # Database name
DB_USER=github_archiver             # Database user
DB_PASSWORD=github_archiver_password # Database password
DB_MIN_CONNECTIONS=5                # Minimum connection pool size
DB_MAX_CONNECTIONS=20               # Maximum connection pool size
```

#### Download Configuration
```bash
DOWNLOAD_DIR=./gharchive_data       # Download directory
MAX_CONCURRENT=6                    # Concurrent downloads
BATCH_SIZE=500                      # Events per batch
REQUEST_TIMEOUT=180                 # HTTP timeout (seconds)
MAX_RETRIES=3                       # Retry attempts
```

#### Resource Limits (Oracle Cloud Optimized)
```bash
MEMORY_LIMIT_GB=18.0               # Memory limit
DISK_LIMIT_GB=40.0                 # Disk limit
CPU_LIMIT_PERCENT=80.0             # CPU usage limit
```

#### Security Configuration
```bash
ADMIN_PASSWORD=admin123            # Web interface password
JWT_SECRET=your-secret-key         # JWT signing secret
WEB_HOST=0.0.0.0                  # Web server bind address
WEB_PORT=8081                     # Web server port
```

#### GitHub API (Optional)
```bash
GITHUB_TOKEN=ghp_xxxxxxxxxxxx     # GitHub personal access token
GITHUB_USERNAME=your-username     # GitHub username
```

## 🎯 Usage

### Command Line Interface

#### Server Management
```bash
# Start all services
./run.sh start

# Stop all services
./run.sh stop

# Restart services
./run.sh restart

# Check status
./run.sh status

# View logs
./run.sh logs [options]
```

#### Direct Operations
```bash
# Run only API server
./run.sh server

# Run only scraper
./run.sh scraper

# Process specific file
./run.sh process /path/to/file.json.gz

# Download specific date
./run.sh download 2024-01-01

# Show comprehensive status
./run.sh status --detailed

# Emergency cleanup
./run.sh cleanup --emergency
```

#### Using Cargo Directly
```bash
# Build application
cargo build --release

# Run with specific command
cargo run --release -- server
cargo run --release -- scraper
cargo run --release -- process /path/to/file.json.gz
cargo run --release -- download 2024-01-01
cargo run --release -- status
cargo run --release -- cleanup
```

### Web Interface

Access the web dashboard at `http://localhost:8081`:

#### Available Pages
- **Dashboard**: Real-time metrics and system status
- **Downloads**: Manage and monitor downloads
- **Processing**: File processing status and history
- **Database**: Database metrics and query interface
- **Logs**: View and search application logs
- **Settings**: Configuration management

#### API Endpoints
```bash
# System status
GET /api/status

# Resource metrics
GET /api/metrics

# Download management
POST /api/downloads
GET /api/downloads/status

# File processing
POST /api/process
GET /api/process/status

# Database queries
POST /api/query
GET /api/database/metrics
```

## 📊 Architecture

### System Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Web Dashboard  │    │  CLI Interface  │    │  Direct Cargo   │
│   (Port 8081)   │    │   (run.sh)     │    │   Commands      │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────┴─────────────┐
                    │      Main Scraper        │
                    │   (Orchestration)        │
                    └─────────────┬─────────────┘
                                  │
        ┌─────────────────────────┼─────────────────────────┐
        │                         │                         │
┌───────▼───────┐        ┌────────▼────────┐        ┌───────▼───────┐
│    Resource   │        │     Archive     │        │      File     │
│   Monitor     │        │    Scraper      │        │   Processor   │
│               │        │                 │        │               │
└───────┬───────┘        └────────┬────────┘        └───────┬───────┘
        │                         │                         │
        │              ┌──────────▼──────────┐              │
        │              │     Downloader      │              │
        │              │   (Concurrent)      │              │
        │              └──────────┬──────────┘              │
        │                         │                         │
        └─────────────────────────┼─────────────────────────┘
                                  │
                        ┌─────────▼─────────┐
                        │   PostgreSQL      │
                        │   Database        │
                        │  (Connection      │
                        │     Pool)         │
                        └───────────────────┘
```

### Data Flow

1. **Archive Discovery**: Scraper identifies available GHArchive files
2. **Concurrent Download**: Multi-threaded download with retry logic
3. **Decompression**: Gzip decompression and JSON parsing
4. **Event Processing**: GitHub event validation and enrichment
5. **Database Storage**: Batch insertion with conflict resolution
6. **Resource Monitoring**: Continuous system monitoring and cleanup
7. **Web Interface**: Real-time metrics and management

## 🔍 Monitoring & Debugging

### Log Files
```bash
logs/
├── scraper.log              # Main scraper operations
├── server.log               # Web server access logs
├── database.log             # Database operations
├── resource_monitor.log     # Resource monitoring
└── error.log               # Error aggregation
```

### Debug Mode
```bash
# Enable debug logging
export RUST_LOG=debug
export RUST_BACKTRACE=full

# Run with debugging
./run.sh start

# Or with cargo
cargo run --release -- scraper
```

### Performance Monitoring
```bash
# Check resource usage
./run.sh status --resources

# Monitor in real-time
watch -n 1 './run.sh status --brief'

# Check database performance
./run.sh status --database
```

## 🛠️ Development

### Building from Source
```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Check code quality
cargo clippy
cargo fmt
```

### Project Structure
```
src/
├── main.rs                 # Application entry point
├── cli.rs                  # Command-line interface
├── core/
│   ├── mod.rs              # Core module exports
│   ├── config.rs           # Configuration management
│   ├── database.rs         # Basic database operations
│   ├── enhanced_database.rs # Advanced database features
│   └── resource_monitor.rs # System monitoring
├── scraper/
│   ├── mod.rs              # Scraper module exports
│   ├── archive_scraper.rs  # Main scraping logic
│   ├── downloader.rs       # Download management
│   ├── file_processor.rs   # File processing
│   └── main_scraper.rs     # Orchestration
└── web/
    ├── mod.rs              # Web module exports
    ├── server.rs           # Web server
    ├── api.rs              # API endpoints
    └── auth.rs             # Authentication
```

### Adding Features
1. Implement in appropriate module
2. Add tests
3. Update CLI interface
4. Add web API endpoints if needed
5. Update documentation

## 🚨 Troubleshooting

### Common Issues

#### Build Failures
```bash
# Update Rust toolchain
rustup update

# Clean build cache
cargo clean
cargo build --release

# Check dependencies
cargo check
```

#### Database Connection Issues
```bash
# Check PostgreSQL status
sudo systemctl status postgresql

# Test connection
psql -h localhost -U github_archiver -d github_archiver

# Reset database
./run.sh cleanup --database
./setup.sh
```

#### Memory Issues (Oracle Cloud)
```bash
# Check current usage
free -h
df -h

# Trigger emergency cleanup
./run.sh cleanup --emergency

# Adjust limits in .env
MEMORY_LIMIT_GB=16.0
MAX_CONCURRENT=4
```

#### Port Conflicts
```bash
# Check what's using port
sudo netstat -tlnp | grep 8081

# Change port in .env
WEB_PORT=8082
```

### Performance Tuning

#### For Oracle Cloud Free Tier
```bash
# Optimized settings
MAX_CONCURRENT=4
BATCH_SIZE=250
DB_MAX_CONNECTIONS=10
MEMORY_LIMIT_GB=16.0
```

#### For Higher-End Systems
```bash
# Performance settings
MAX_CONCURRENT=12
BATCH_SIZE=1000
DB_MAX_CONNECTIONS=50
MEMORY_LIMIT_GB=32.0
```

## 📈 Metrics & Analytics

### Available Metrics
- **Processing Rate**: Events per second
- **Download Speed**: MB/s with concurrent downloads
- **Database Performance**: Insert/query performance
- **Resource Usage**: Memory, CPU, disk utilization
- **Error Rates**: Download failures, processing errors
- **Queue Status**: Pending downloads and processing

### Export Options
- **JSON API**: Real-time metrics via `/api/metrics`
- **Log Analysis**: Structured logs for external tools
- **Database Queries**: Direct PostgreSQL access
- **Web Dashboard**: Visual metrics and charts

## 🔐 Security

### Best Practices
- Change default passwords immediately
- Use strong JWT secrets
- Limit network access to necessary ports
- Regular security updates
- Monitor access logs

### Network Security
```bash
# Firewall rules (example)
sudo ufw allow 8081/tcp     # Web interface
sudo ufw allow 5432/tcp     # PostgreSQL (if remote)
sudo ufw enable
```

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📞 Support

- **Documentation**: Check this README and inline code comments
- **Issues**: Use GitHub Issues for bug reports
- **Discussions**: Use GitHub Discussions for questions
- **Logs**: Check application logs for detailed error information

## 🎯 Roadmap

### Planned Features
- [ ] Real-time GitHub event streaming
- [ ] Advanced analytics and reporting
- [ ] Multi-repository filtering
- [ ] Export to various formats (CSV, Parquet)
- [ ] Integration with BI tools
- [ ] Distributed processing support
- [ ] Machine learning insights
- [ ] API rate optimization

### Performance Improvements
- [ ] Memory usage optimization
- [ ] Database query optimization
- [ ] Caching layer
- [ ] Compression improvements
- [ ] Network optimization

---

<div align="center">

**Built with ❤️ using Rust**

*For more information, visit our [GitHub repository](https://github.com/your-repo/github-archiver)*

</div>
