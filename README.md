# 🚀 GitArchiver - GitHub Archive Data Scraper & Analyzer

A comprehensive, production-ready GitHub Archive data scraper and analyzer designed for Oracle Cloud infrastructure with advanced resource management, real-time monitoring, and intelligent data processing capabilities.

## 🎯 Overview

GitArchiver is a sophisticated Python-based system that downloads, processes, and analyzes GitHub Archive data from https://data.gharchive.org/. It provides a complete solution for researchers, security professionals, and developers who need to analyze GitHub activity patterns, track repository changes, and generate actionable insights from GitHub's public event stream.

## ✨ Key Features

### 🛡️ Oracle Cloud Optimized
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
- **Real-time Dashboard**: Live monitoring of scraper status and metrics
- **Resource Visualization**: Progress bars for memory, disk, and CPU usage
- **Event Search**: Full-text search with advanced filtering capabilities
- **Log Viewer**: Real-time log streaming with download functionality
- **Control Panel**: Start/stop/restart operations with safety checks

### 🔍 Advanced Analytics
- **Event Analysis**: Comprehensive GitHub event type tracking
- **Repository Insights**: Repository change monitoring and statistics
- **Wordlist Generation**: Security research wordlist creation from GitHub data
- **Trend Analysis**: Historical data analysis and pattern recognition

### 🔐 Security & Authentication
- **Rate Limiting**: Intelligent GitHub API rate limit management
- **Authentication**: Secure web interface with session management
- **API Security**: Protected endpoints with proper authentication
- **Audit Logging**: Comprehensive activity logging and monitoring

## 🏗️ Architecture

```
GitArchiver/
├── Core Components/
│   ├── gharchive_scraper.py    # Main scraper engine
│   ├── api.py                  # Web API with resource monitoring
│   ├── db_manager.py           # Database management and queries
│   └── config.py               # Configuration management
├── Utilities/
│   ├── auth_manager.py         # Authentication system
│   ├── rate_limiter.py         # GitHub API rate limiting
│   ├── data_importer.py        # Multi-source data import
│   ├── wordlist_generator.py   # Security wordlist generation
│   └── system_monitor.py       # System resource monitoring
├── Web Interface/
│   └── dashboard.html          # Professional web dashboard
├── Scripts/
│   ├── Grabber.sh             # Bash wrapper for common operations
│   ├── start_safe_api.sh      # Safe API startup with monitoring
│   └── safe_scraper.sh        # Safe scraper execution
└── Data/
    ├── gharchive_data/        # Downloaded GitHub Archive files
    ├── logs/                  # Application logs
    └── reports/               # System monitoring reports
```

## 🚀 Quick Start

### Prerequisites

- Python 3.8+
- PostgreSQL 12+
- 16GB+ RAM (recommended for Oracle Cloud)
- 50GB+ disk space

### Installation

1. **Clone the repository:**
```bash
git clone https://github.com/Bit-Loop/GitArchiver.git
cd GitArchiver
```

2. **Create and activate virtual environment:**
```bash
python3 -m venv github_scraper_env
source github_scraper_env/bin/activate
```

3. **Install dependencies:**
```bash
pip install -r requirements.txt
```

4. **Configure environment:**
```bash
cp .env.example .env
# Edit .env with your database and GitHub API credentials
```

5. **Set up database:**
```bash
python3 db_manager.py --setup
```

### Configuration

Create a `.env` file with your settings:

```bash
# Database Configuration
DB_HOST=localhost
DB_PORT=5432
DB_NAME=gharchive
DB_USER=gharchive
DB_PASSWORD=your_secure_password

# GitHub API (optional, for higher rate limits)
GITHUB_TOKEN=your_github_token
GITHUB_USERNAME=your_username

# Resource Limits (Oracle Cloud optimized)
MAX_CONCURRENT_DOWNLOADS=6
BATCH_SIZE=500
REQUEST_TIMEOUT=180

# Storage
DOWNLOAD_DIR=./gharchive_data
```

## 🎮 Usage

### Web Dashboard

Start the web API and dashboard:

```bash
./start_safe_api.sh
```

Access the dashboard at: `http://localhost:8080`

The dashboard provides:
- Real-time system monitoring
- Scraper control (start/stop/restart)
- Event search and analysis
- Log viewing and download
- Resource usage visualization

### Command Line Interface

**Basic scraping:**
```bash
# Scrape recent data (last 24 hours)
python3 gharchive_scraper.py --recent

# Scrape specific date range
python3 gharchive_scraper.py --start-date 2024-01-01 --end-date 2024-01-31

# Continuous monitoring mode
python3 gharchive_scraper.py --monitor
```

**Database management:**
```bash
# View database statistics
python3 db_manager.py --stats

# Search events
python3 db_manager.py --search "repository_name:example"

# Export data
python3 db_manager.py --export --format json --output results.json
```

**Bash wrapper (simplified commands):**
```bash
# Quick recent scrape
./Grabber.sh recent

# Custom date range
./Grabber.sh range 2024-01-01 2024-01-31

# System status
./Grabber.sh status
```

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