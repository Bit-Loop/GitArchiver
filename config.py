#!/usr/bin/env python3

import os
from datetime import datetime
from pathlib import Path
from dotenv import load_dotenv

# Load environment variables from .env file
load_dotenv()

class Config:
    """Configuration settings with enhanced options"""
    
    # Database settings
    DB_HOST = os.getenv('DB_HOST', 'localhost')
    DB_PORT = int(os.getenv('DB_PORT', '5432'))
    DB_NAME = os.getenv('DB_NAME', 'gharchive')
    DB_USER = os.getenv('DB_USER', 'gharchive')
    DB_PASSWORD = os.getenv('DB_PASSWORD', 'gharchive_password')
    
    # GitHub API settings
    GITHUB_TOKEN = os.getenv('GITHUB_TOKEN', '')  # For higher rate limits
    GITHUB_USERNAME = os.getenv('GITHUB_USERNAME', '')
    GITHUB_API_BASE = 'https://api.github.com'
    
    # Rate limiting settings
    UNAUTHENTICATED_RATE_LIMIT = 60  # GitHub's limit per hour
    AUTHENTICATED_RATE_LIMIT = 5000  # GitHub's limit per hour
    RATE_LIMIT_BUFFER = 5  # Keep 5 requests as buffer
    RATE_LIMIT_RESET_BUFFER = 300  # 5 minutes buffer before reset
    
    # Connection pool settings
    DB_MIN_CONNECTIONS = int(os.getenv('DB_MIN_CONNECTIONS', '5'))
    DB_MAX_CONNECTIONS = int(os.getenv('DB_MAX_CONNECTIONS', '20'))
    
    # Download settings - Oracle Cloud optimized
    BASE_URL = 'https://data.gharchive.org/'
    S3_LIST_URL = 'https://data.gharchive.org/?list-type=2'
    DOWNLOAD_DIR = Path(os.getenv('DOWNLOAD_DIR', './gharchive_data'))
    MAX_CONCURRENT_DOWNLOADS = int(os.getenv('MAX_CONCURRENT', '6'))  # Reduced for safety
    CHUNK_SIZE = int(os.getenv('CHUNK_SIZE', '4096'))  # Smaller chunks
    
    # HTTP client settings - Conservative timeouts
    REQUEST_TIMEOUT = int(os.getenv('REQUEST_TIMEOUT', '180'))  # Reduced timeout
    MAX_RETRIES = int(os.getenv('MAX_RETRIES', '3'))
    RETRY_DELAY = float(os.getenv('RETRY_DELAY', '2.0'))  # Longer delay
    
    # Processing settings - Smaller batches
    BATCH_SIZE = int(os.getenv('BATCH_SIZE', '500'))  # Reduced batch size
    
    # Memory management - Oracle Cloud safe limits
    MAX_MEMORY_MB = int(os.getenv('MAX_MEMORY_MB', '18000'))  # 18GB max for 24GB system
    MEMORY_WARNING_MB = int(os.getenv('MEMORY_WARNING_MB', '16000'))  # Warning at 16GB
    MEMORY_CHECK_INTERVAL = int(os.getenv('MEMORY_CHECK_INTERVAL', '50'))  # Check more frequently
    
    # Disk management - Oracle Cloud safe limits
    MAX_DISK_USAGE_GB = int(os.getenv('MAX_DISK_USAGE_GB', '40'))  # 40GB max disk usage
    DISK_WARNING_GB = int(os.getenv('DISK_WARNING_GB', '35'))  # Warning at 35GB
    TEMP_FILE_CLEANUP_INTERVAL = int(os.getenv('TEMP_CLEANUP_INTERVAL', '300'))  # 5 minutes
    
    # Resource monitoring
    RESOURCE_CHECK_INTERVAL = int(os.getenv('RESOURCE_CHECK_INTERVAL', '30'))  # 30 seconds
    CPU_LIMIT_PERCENT = int(os.getenv('CPU_LIMIT_PERCENT', '80'))  # Max 80% CPU
    
    # Safety limits
    MAX_FILE_SIZE_MB = int(os.getenv('MAX_FILE_SIZE_MB', '500'))  # 500MB per file
    EMERGENCY_CLEANUP_THRESHOLD = float(os.getenv('EMERGENCY_CLEANUP_THRESHOLD', '0.9'))  # 90% memory
    
    # Date range settings (focus on 2015 onward as requested)
    MIN_DATE = datetime(2015, 1, 1)
    
    # GitHub API settings for repo changes
    GITHUB_TOKEN = os.getenv('GITHUB_TOKEN')  # Optional but recommended
    
    # Logging
    LOG_LEVEL = os.getenv('LOG_LEVEL', 'INFO')
    LOG_FILE = os.getenv('LOG_FILE', 'gharchive_scraper.log')
    
    # Performance monitoring
    ENABLE_METRICS = os.getenv('ENABLE_METRICS', 'true').lower() == 'true'
    METRICS_INTERVAL = int(os.getenv('METRICS_INTERVAL', '60'))
    
    # Graceful shutdown
    SHUTDOWN_TIMEOUT = int(os.getenv('SHUTDOWN_TIMEOUT', '30'))
    
    # Web API settings
    WEB_HOST = os.getenv('WEB_HOST', '0.0.0.0')
    WEB_PORT = int(os.getenv('WEB_PORT', '8080'))
