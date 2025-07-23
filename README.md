# GitHub Archive Scraper

A comprehensive, high-performance Python application for scraping, processing, and analyzing GitHub Archive data. This enterprise-grade solution features concurrent processing, advanced database capabilities, real-time monitoring, and a beautiful web dashboard.

## 🚀 Features

### Core Functionality
- **Complete GitHub Archive Coverage**: Scrapes all hourly archive files from 2015 onwards
- **Intelligent File Management**: ETag-based caching system prevents unnecessary downloads
- **Concurrent Processing**: Asynchronous downloads with configurable concurrency limits
- **Robust Error Handling**: Comprehensive retry logic with exponential backoff
- **Missing Data Detection**: Automatically identifies and recovers missing archive files

### Database & Storage
- **PostgreSQL Integration**: Advanced schema with JSONB support for flexible event storage
- **Full-Text Search**: GIN indexes enable fast text searches across all event data
- **Materialized Views**: Pre-computed statistics for lightning-fast analytics
- **Connection Pooling**: Optimized database performance with async connection management
- **Data Integrity**: Comprehensive constraints and validation

### Performance & Monitoring
- **Real-Time Metrics**: System resource monitoring with psutil integration
- **Performance Analytics**: Processing rates, error tracking, and throughput analysis
- **Graceful Shutdown**: Signal handling for clean process termination
- **Memory Management**: Streaming JSON processing for large files
- **Auto-Optimization**: Dynamic performance tuning based on system metrics

### Web Interface & API
- **REST API**: Complete HTTP API for data querying and system monitoring
- **Interactive Dashboard**: Beautiful web interface with real-time updates
- **Data Visualization**: Comprehensive analytics and reporting capabilities
- **Search & Filtering**: Advanced search across events, repositories, and users
- **Export Capabilities**: JSON export for analysis reports

## 📋 Requirements

### System Requirements
- **Operating System**: Linux/Unix (tested on Ubuntu 20.04+)
- **Python**: 3.8 or higher
- **PostgreSQL**: 13 or higher
- **Memory**: 4GB+ RAM recommended
- **Storage**: 100GB+ for comprehensive archive storage

### Dependencies
```
aiofiles>=23.0.0      # Async file operations
aiohttp>=3.8.0        # HTTP client and web server
aiohttp-cors>=0.7.0   # CORS support for API
asyncpg>=0.28.0       # Async PostgreSQL driver
psutil>=5.9.0         # System monitoring
memory-profiler>=0.60.0  # Memory profiling
python-dateutil>=2.8.0  # Date parsing
```

## 🛠️ Installation

### Automated Setup
```bash
# Clone the repository
git clone <repository-url>
cd GitArchiver

# Run the automated setup script
chmod +x setup.sh
./setup.sh
```

## Usage

### Basic Operations

```bash
# Process yesterday's data (default)
./Grabber.sh

# Discover available archive files (Ruby script equivalent)
./Grabber.sh --mode discover

# Catch up all data from 2015 onward (as requested)
./Grabber.sh --catch-up-from-2015

# Find missing data ranges
./Grabber.sh --mode missing

# Process specific date
./Grabber.sh --date 2015-01-01

# Process date range
./Grabber.sh --start-date 2015-01-01 --end-date 2015-01-31
```

### Search and Export

```bash
# Search for PushEvents in Linux kernel repo
./Grabber.sh --mode search --search-query '{"type": "PushEvent", "repo_name": "torvalds/linux"}'

# Search for events by specific user
./Grabber.sh --mode search --search-query '{"actor_login": "octocat"}'

# Export all data for a repository
./Grabber.sh --mode export --export-repo torvalds/linux

# Search with date range
./Grabber.sh --mode search --search-query '{"type": "CreateEvent", "date_from": "2015-01-01", "date_to": "2015-01-31"}'
```

### Automation

Set up daily cron job:
```bash
# Add to crontab (crontab -e)
0 2 * * * /path/to/GitArchiver/Grabber.sh >/dev/null 2>&1
```

## Database Schema

The scraper creates an optimized PostgreSQL schema with:

### Tables
- **github_events**: Main events table with JSONB payload support
- **repositories**: Repository metadata and tracking
- **repository_changes**: Detailed change tracking for repos
- **processed_files**: File processing state (rsync-like behavior)
- **missing_data_ranges**: Missing data detection and recovery

### Indexes
- Full-text search on repositories
- GIN indexes on JSONB payloads
- Optimized indexes for common query patterns

### Materialized Views
- **daily_event_stats**: Aggregated daily statistics
- **repository_activity_stats**: Repository activity analytics

## Configuration

Environment variables (`.env` file):

```bash
# Database
DB_HOST=localhost
DB_PORT=5432
DB_NAME=gharchive
DB_USER=gharchive
DB_PASSWORD=gharchive_password

# GitHub API (optional but recommended)
GITHUB_TOKEN=your_github_token_here

# Performance
MAX_CONCURRENT=10
BATCH_SIZE=1000
CHUNK_SIZE=8192

# Logging
LOG_LEVEL=INFO
LOG_FILE=gharchive_scraper.log
```

## Python Script Equivalent

This tool provides Python equivalent functionality to the Ruby script example:

**Ruby (original)**:
```ruby
require 'open-uri'
require 'zlib'
require 'yajl'

gz = open('http://data.gharchive.org/2015-01-01-12.json.gz')
js = Zlib::GzipReader.new(gz).read

Yajl::Parser.parse(js) do |event|
  print event
end
```

**Python (this tool)**:
```python
# Automatically handles:
# - gzip decompression
# - JSON parsing line by line
# - Database storage
# - Error handling
# - Parallel processing
# - ETag-based caching (rsync-like)

./Grabber.sh --date 2015-01-01
```

## Architecture

The scraper follows these principles:

1. **Efficient Download**: Uses HTTP HEAD requests to check ETags before downloading
2. **Streaming Processing**: Processes JSON line-by-line to handle large files
3. **Batch Insertion**: Groups database operations for performance
4. **Error Recovery**: Continues processing even if individual events fail
5. **State Tracking**: Remembers processed files to avoid reprocessing
6. **Comprehensive Logging**: Detailed logs for monitoring and debugging

## Monitoring

View logs:
```bash
tail -f gharchive_scraper.log
```

Check database stats:
```sql
-- Daily event statistics
SELECT * FROM daily_event_stats ORDER BY event_date DESC LIMIT 10;

-- Repository activity
SELECT * FROM repository_activity_stats ORDER BY total_events DESC LIMIT 10;

-- Processing status
SELECT COUNT(*) as processed_files, SUM(event_count) as total_events 
FROM processed_files WHERE is_complete = true;
```

## Advanced Usage

### Direct Python Script

```bash
# Activate virtual environment
source venv/bin/activate

# Run Python script directly
python3 gharchive_scraper.py --mode discover
python3 gharchive_scraper.py --catch-up-from-2015
python3 gharchive_scraper.py --mode search --search-query '{"type": "PushEvent"}'
```

### Database Queries

```sql
-- Search for events by repository
SELECT * FROM github_events 
WHERE repo_name = 'torvalds/linux' 
ORDER BY created_at DESC LIMIT 10;

-- Find most active repositories
SELECT repo_name, COUNT(*) as events 
FROM github_events 
GROUP BY repo_name 
ORDER BY events DESC LIMIT 10;

-- Search in payload data
SELECT * FROM github_events 
WHERE payload @> '{"action": "opened"}' 
AND type = 'PullRequestEvent';

-- Full-text search repositories
SELECT * FROM search_repositories('machine learning', 20);
```

## Performance Tips

1. **GitHub Token**: Add a GitHub token to avoid rate limiting
2. **Concurrent Downloads**: Adjust `MAX_CONCURRENT` based on your system
3. **Batch Size**: Increase `BATCH_SIZE` for better database performance
4. **Disk Space**: Monitor disk usage during catch-up operations
5. **Database Tuning**: Consider PostgreSQL tuning for large datasets

## Troubleshooting

### Common Issues

1. **Database Connection Error**: Check PostgreSQL is running and credentials
2. **Permission Error**: Ensure scripts are executable (`chmod +x`)
3. **Disk Space**: Monitor available disk space during processing
4. **Rate Limiting**: Add GitHub token to avoid API limits

### Debug Mode

```bash
# Enable debug logging
export LOG_LEVEL=DEBUG
./Grabber.sh --mode discover
```

## Contributing

The scraper is designed to be extensible:

1. Add new search functions in the `search_events()` method
2. Extend export formats in the `export_repository_data()` method
3. Add new database views for analytics
4. Implement additional change tracking for specific event types

## License

This project is designed for educational and research purposes. Please respect GitHub's API terms of service and the original data.gharchive.org terms.
