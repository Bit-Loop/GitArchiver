# Safe GitHub Archive Scraper for Oracle Cloud

## 🛡️ Oracle Cloud Safety Features Implemented

Your GitHub Archive Scraper now includes comprehensive resource monitoring and safety features specifically designed for Oracle Cloud instances.

## 📊 Resource Monitoring

### Memory Management
- **Process Memory Limit**: 18GB (out of 24GB system)
- **Warning Threshold**: 16GB
- **Emergency Cleanup**: Triggered at 90% usage
- **Automatic Garbage Collection**: Enabled

### Disk Management
- **Disk Usage Limit**: 40GB
- **Warning Threshold**: 35GB
- **Temporary File Cleanup**: Every 5 minutes
- **File Size Limits**: 500MB per file maximum

### CPU Management
- **CPU Usage Limit**: 80%
- **Load Monitoring**: 1, 5, and 15-minute averages
- **Process Throttling**: Automatic pausing under high load

## 🚀 New API Endpoints

### Health & Monitoring
- `GET /api/health` - System health check with resource status
- `GET /api/monitor` - Real-time resource monitoring with recommendations
- `GET /api/status` - API and service status

### Resource Management
- `POST /api/emergency-cleanup` - Force garbage collection and temp file cleanup
- `POST /api/start-scraper` - Start scraper with resource checks
- `POST /api/stop-scraper` - Gracefully stop scraper
- `POST /api/restart-scraper` - Restart scraper safely

### Data Access (Safe)
- `GET /api/stats` - Database statistics
- `GET /api/events` - Recent events with pagination limits
- `GET /api/repositories` - Repository data with limits
- `POST /api/search` - Search with timeout and result limits

## 🔧 Management Tools

### Safe Startup Script
```bash
./start_safe_api.sh start --background  # Start in background
./start_safe_api.sh status              # Check service status
./start_safe_api.sh stop                # Stop all services
./start_safe_api.sh restart             # Restart services
./start_safe_api.sh check               # Run system checks
./start_safe_api.sh monitor             # Run system monitor
```

### System Monitor
- Real-time resource tracking
- Automatic alerting for resource pressure
- Historical reporting
- Oracle Cloud optimized thresholds

### Test Suite
```bash
python test_api.py  # Comprehensive API testing
```

## 📈 Current Status

✅ **API Server**: Running on port 8080  
✅ **Database**: Connected and healthy  
✅ **Resource Monitor**: Active with Oracle Cloud limits  
✅ **System Monitor**: Background monitoring enabled  
✅ **Safety Features**: All implemented and tested  

## 🌐 Access Points

- **Dashboard**: http://170.9.239.38:8080
- **API Base**: http://170.9.239.38:8080/api/
- **Health Check**: http://170.9.239.38:8080/api/health

## 📋 Safety Configuration

### Environment Variables (.env)
```bash
# Resource Limits for Oracle Cloud (24GB RAM system)
MAX_MEMORY_MB=18000
MEMORY_WARNING_MB=16000
MAX_DISK_USAGE_GB=40
DISK_WARNING_GB=35
CPU_LIMIT_PERCENT=80
MAX_FILE_SIZE_MB=500

# Conservative Performance Settings
MAX_CONCURRENT=6          # Reduced from 10
BATCH_SIZE=500           # Reduced from 1000
CHUNK_SIZE=4096          # Reduced from 8192

# Resource Monitoring
RESOURCE_CHECK_INTERVAL=30
EMERGENCY_CLEANUP_THRESHOLD=0.9
```

## 🚨 Safety Features

### Automatic Protection
1. **Memory Monitoring**: Continuous tracking with cleanup at 90% usage
2. **Disk Management**: Automatic temp file cleanup and usage limits
3. **CPU Throttling**: Process pausing when CPU exceeds 80%
4. **File Size Limits**: Prevents downloading files over 500MB
5. **Connection Limits**: Conservative database and HTTP connection pools
6. **Timeout Protection**: All operations have reasonable timeouts

### Emergency Procedures
1. **Resource Pressure**: Automatic pausing of operations
2. **Memory Critical**: Force garbage collection and temp cleanup
3. **Disk Full**: Stop new downloads and cleanup old files
4. **High CPU**: Pause processing until load decreases

### Monitoring & Alerts
1. **Real-time Dashboard**: Live resource monitoring
2. **System Monitor**: Background process tracking
3. **Log Monitoring**: Automatic log rotation and cleanup
4. **Health Checks**: Regular system health validation

## 📊 Resource Usage (Current)

- **Process Memory**: 41.5MB (0.2% of limit)
- **System Memory**: 3.2GB (15.2% of total)
- **Disk Usage**: 10.6GB (26.4% of limit)
- **CPU Usage**: 0.0%
- **Status**: ✅ ALL SYSTEMS SAFE

## 🔄 Next Steps

1. **Test External Access**: Verify Oracle Cloud security group settings for port 8080
2. **Start Scraping**: Use the dashboard to start the GitHub Archive scraper
3. **Monitor Resources**: Watch the real-time dashboard for resource usage
4. **Adjust Limits**: Fine-tune limits based on actual usage patterns

## 🛠️ Troubleshooting

### If Memory Usage Gets High
```bash
curl -X POST http://localhost:8080/api/emergency-cleanup
```

### If Services Stop Responding
```bash
./start_safe_api.sh restart
```

### Check System Status
```bash
./start_safe_api.sh status
python test_api.py
```

### View Logs
```bash
tail -f logs/api.log
tail -f logs/system_monitor.log
```

Your Oracle Cloud instance is now protected against resource exhaustion and ready for safe GitHub Archive scraping! 🎉
