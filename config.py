#!/usr/bin/env python3
"""
Configuration file for GitHub Archive Scraper
"""

import os
from datetime import datetime
from pathlib import Path

class Config:
    """Configuration settings for the GitHub Archive Scraper"""
    
    # Database settings
    DB_HOST = os.getenv('DB_HOST', 'localhost')
    DB_PORT = int(os.getenv('DB_PORT', '5432'))
    DB_NAME = os.getenv('DB_NAME', 'gharchive')
    DB_USER = os.getenv('DB_USER', 'gharchive')
    DB_PASSWORD = os.getenv('DB_PASSWORD', 'gharchive_password')
    
    # Download settings
    BASE_URL = 'https://data.gharchive.org/'
    S3_LIST_URL = 'https://data.gharchive.org/?list-type=2'
    DOWNLOAD_DIR = Path(os.getenv('DOWNLOAD_DIR', './gharchive_data'))
    MAX_CONCURRENT_DOWNLOADS = int(os.getenv('MAX_CONCURRENT', '10'))
    CHUNK_SIZE = int(os.getenv('CHUNK_SIZE', '8192'))
    
    # Processing settings
    BATCH_SIZE = int(os.getenv('BATCH_SIZE', '1000'))
    
    # Date range settings (focus on 2015 onward as requested)
    MIN_DATE = datetime(2015, 1, 1)
    
    # GitHub API settings for repo changes
    GITHUB_TOKEN = os.getenv('GITHUB_TOKEN')  # Optional but recommended
    
    # Logging
    LOG_LEVEL = os.getenv('LOG_LEVEL', 'INFO')
    LOG_FILE = os.getenv('LOG_FILE', 'gharchive_scraper.log')
    
    # Cron job settings
    ENABLE_CRON_MODE = os.getenv('ENABLE_CRON_MODE', 'false').lower() == 'true'
    CRON_LOCK_FILE = os.getenv('CRON_LOCK_FILE', '/tmp/gharchive_scraper.lock')

# Example environment file content (.env)
ENV_EXAMPLE = """
# Database Configuration
DB_HOST=localhost
DB_PORT=5432
DB_NAME=gharchive
DB_USER=gharchive
DB_PASSWORD=your_secure_password_here

# GitHub API Token (optional, but recommended for rate limiting)
GITHUB_TOKEN=your_github_token_here

# Performance Tuning
MAX_CONCURRENT=10
BATCH_SIZE=1000
CHUNK_SIZE=8192

# Logging
LOG_LEVEL=INFO
LOG_FILE=gharchive_scraper.log

# Cron Mode
ENABLE_CRON_MODE=true
CRON_LOCK_FILE=/tmp/gharchive_scraper.lock
"""

if __name__ == '__main__':
    print("GitHub Archive Scraper Configuration")
    print("====================================")
    print()
    print("Create a .env file with the following content:")
    print(ENV_EXAMPLE)
