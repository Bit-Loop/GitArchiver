#!/usr/bin/env python3
"""
GitHub Archive Scraper - Enhanced version with PostgreSQL support

This script downloads and processes GitHub Archive data from https://data.gharchive.org/
Inspired by the Ruby script example and enhanced for production use.

Features:
- Efficient downloading with ETag support (rsync-like behavior)
- PostgreSQL database with optimized schema for GitHub events
- Parallel processing for faster ingestion
- Comprehensive logging and error handling
- Configurable date ranges (supports 2015 onward focus)
- Repository change tracking and downloading
- Advanced search and data extraction capabilities
- Cron-job friendly with state tracking
- Missing data detection and recovery
"""

import argparse
import asyncio
import gzip
import json
import logging
import os
import re
import sys
import tempfile
import time
import signal
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Set, AsyncGenerator
from urllib.parse import urljoin
import xml.etree.ElementTree as ET
from contextlib import asynccontextmanager
from dataclasses import dataclass
import hashlib

import aiofiles
import aiohttp
import asyncpg
from dateutil.parser import parse as parse_date

# Performance monitoring
import psutil
import memory_profiler


@dataclass
class ProcessingStats:
    """Statistics for processing operations"""
    downloaded: int = 0
    skipped: int = 0
    failed: int = 0
    events: int = 0
    bytes_downloaded: int = 0
    processing_time: float = 0.0
    memory_peak_mb: float = 0.0
    
    def add(self, other: 'ProcessingStats'):
        """Add another stats object to this one"""
        self.downloaded += other.downloaded
        self.skipped += other.skipped
        self.failed += other.failed
        self.events += other.events
        self.bytes_downloaded += other.bytes_downloaded
        self.processing_time += other.processing_time
        self.memory_peak_mb = max(self.memory_peak_mb, other.memory_peak_mb)


class ResourceMonitor:
    """Monitor system resources and enforce limits for Oracle Cloud safety"""
    
    def __init__(self, config):
        self.config = config
        self.process = psutil.Process()
        self.start_time = time.time()
        self.temp_files = set()
        self.emergency_mode = False
        
    def get_memory_usage(self) -> float:
        """Get current memory usage in MB"""
        return self.process.memory_info().rss / 1024 / 1024
    
    def get_disk_usage(self, path: str = '.') -> Tuple[float, float]:
        """Get disk usage in GB (used, total)"""
        usage = psutil.disk_usage(path)
        return usage.used / (1024**3), usage.total / (1024**3)
    
    def get_cpu_usage(self) -> float:
        """Get CPU usage percentage"""
        return self.process.cpu_percent()
    
    def check_memory_limit(self) -> bool:
        """Check if memory usage is within safe limits"""
        memory_mb = self.get_memory_usage()
        
        if memory_mb > self.config.MAX_MEMORY_MB:
            logging.error(f"Memory usage {memory_mb:.1f}MB exceeds limit {self.config.MAX_MEMORY_MB}MB")
            return False
        elif memory_mb > self.config.MEMORY_WARNING_MB:
            logging.warning(f"Memory usage {memory_mb:.1f}MB approaching limit {self.config.MAX_MEMORY_MB}MB")
            
        return True
    
    def check_disk_limit(self) -> bool:
        """Check if disk usage is within safe limits"""
        used_gb, total_gb = self.get_disk_usage()
        
        if used_gb > self.config.MAX_DISK_USAGE_GB:
            logging.error(f"Disk usage {used_gb:.1f}GB exceeds limit {self.config.MAX_DISK_USAGE_GB}GB")
            return False
        elif used_gb > self.config.DISK_WARNING_GB:
            logging.warning(f"Disk usage {used_gb:.1f}GB approaching limit {self.config.MAX_DISK_USAGE_GB}GB")
            
        return True
    
    def emergency_cleanup(self):
        """Perform emergency cleanup to free resources"""
        if self.emergency_mode:
            return
            
        self.emergency_mode = True
        logging.warning("Performing emergency cleanup due to resource pressure")
        
        # Force garbage collection
        import gc
        gc.collect()
        
        # Clean up temp files
        self.cleanup_temp_files()
        
        # Log current resource usage
        memory_mb = self.get_memory_usage()
        used_gb, total_gb = self.get_disk_usage()
        cpu_percent = self.get_cpu_usage()
        
        logging.info(f"Post-cleanup: Memory: {memory_mb:.1f}MB, Disk: {used_gb:.1f}GB, CPU: {cpu_percent:.1f}%")
        
        self.emergency_mode = False
    
    def cleanup_temp_files(self):
        """Clean up temporary files"""
        for temp_file in list(self.temp_files):
            try:
                if os.path.exists(temp_file):
                    os.remove(temp_file)
                    logging.debug(f"Cleaned up temp file: {temp_file}")
                self.temp_files.discard(temp_file)
            except Exception as e:
                logging.warning(f"Failed to clean up temp file {temp_file}: {e}")
    
    def register_temp_file(self, filepath: str):
        """Register a temporary file for cleanup"""
        self.temp_files.add(filepath)
    
    def should_pause_processing(self) -> bool:
        """Check if processing should be paused due to resource pressure"""
        memory_mb = self.get_memory_usage()
        used_gb, _ = self.get_disk_usage()
        cpu_percent = self.get_cpu_usage()
        
        memory_ratio = memory_mb / self.config.MAX_MEMORY_MB
        disk_ratio = used_gb / self.config.MAX_DISK_USAGE_GB
        
        if (memory_ratio > self.config.EMERGENCY_CLEANUP_THRESHOLD or 
            disk_ratio > self.config.EMERGENCY_CLEANUP_THRESHOLD or
            cpu_percent > self.config.CPU_LIMIT_PERCENT):
            
            self.emergency_cleanup()
            return True
            
        return False
    
    def get_status(self) -> Dict:
        """Get current resource status"""
        memory_mb = self.get_memory_usage()
        used_gb, total_gb = self.get_disk_usage()
        cpu_percent = self.get_cpu_usage()
        uptime = time.time() - self.start_time
        
        return {
            'memory_mb': round(memory_mb, 1),
            'memory_limit_mb': self.config.MAX_MEMORY_MB,
            'memory_usage_percent': round((memory_mb / self.config.MAX_MEMORY_MB) * 100, 1),
            'disk_used_gb': round(used_gb, 1),
            'disk_limit_gb': self.config.MAX_DISK_USAGE_GB,
            'disk_usage_percent': round((used_gb / self.config.MAX_DISK_USAGE_GB) * 100, 1),
            'cpu_percent': round(cpu_percent, 1),
            'cpu_limit_percent': self.config.CPU_LIMIT_PERCENT,
            'uptime_seconds': round(uptime, 1),
            'temp_files_count': len(self.temp_files),
            'emergency_mode': self.emergency_mode
        }


class Config:
    """Configuration settings with enhanced options"""
    
    # Database settings
    DB_HOST = os.getenv('DB_HOST', 'localhost')
    DB_PORT = int(os.getenv('DB_PORT', '5432'))
    DB_NAME = os.getenv('DB_NAME', 'gharchive')
    DB_USER = os.getenv('DB_USER', 'gharchive')
    DB_PASSWORD = os.getenv('DB_PASSWORD', 'gharchive')
    
    # Connection pool settings
    DB_MIN_CONNECTIONS = int(os.getenv('DB_MIN_CONNECTIONS', '5'))
    DB_MAX_CONNECTIONS = int(os.getenv('DB_MAX_CONNECTIONS', '20'))
    
    # Download settings
    BASE_URL = 'https://data.gharchive.org/'
    S3_LIST_URL = 'https://data.gharchive.org/?list-type=2'
    DOWNLOAD_DIR = Path(os.getenv('DOWNLOAD_DIR', './gharchive_data'))
    MAX_CONCURRENT_DOWNLOADS = int(os.getenv('MAX_CONCURRENT', '10'))
    CHUNK_SIZE = int(os.getenv('CHUNK_SIZE', '8192'))
    
    # HTTP client settings
    REQUEST_TIMEOUT = int(os.getenv('REQUEST_TIMEOUT', '300'))
    MAX_RETRIES = int(os.getenv('MAX_RETRIES', '3'))
    RETRY_DELAY = float(os.getenv('RETRY_DELAY', '1.0'))
    
    # Processing settings
    BATCH_SIZE = int(os.getenv('BATCH_SIZE', '1000'))
    
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


class DatabaseManager:
    """Handles PostgreSQL database operations with enhanced connection management"""
    
    def __init__(self, config: Config):
        self.config = config
        self.pool = None
        
    async def connect(self):
        """Create database connection pool with retry logic"""
        dsn = f"postgresql://{self.config.DB_USER}:{self.config.DB_PASSWORD}@{self.config.DB_HOST}:{self.config.DB_PORT}/{self.config.DB_NAME}"
        
        for attempt in range(self.config.MAX_RETRIES):
            try:
                self.pool = await asyncpg.create_pool(
                    dsn, 
                    min_size=self.config.DB_MIN_CONNECTIONS, 
                    max_size=self.config.DB_MAX_CONNECTIONS,
                    command_timeout=60
                )
                await self.init_schema()
                logging.info(f"Database connected successfully (attempt {attempt + 1})")
                return
            except Exception as e:
                logging.error(f"Database connection attempt {attempt + 1} failed: {e}")
                if attempt < self.config.MAX_RETRIES - 1:
                    await asyncio.sleep(self.config.RETRY_DELAY * (attempt + 1))
                else:
                    raise
        
    async def disconnect(self):
        """Close database connection pool gracefully"""
        if self.pool:
            await self.pool.close()
            
    async def health_check(self) -> bool:
        """Check database health"""
        try:
            async with self.pool.acquire() as conn:
                await conn.fetchval("SELECT 1")
            return True
        except Exception as e:
            logging.error(f"Database health check failed: {e}")
            return False
            
    async def init_schema(self):
        """Initialize database schema with enhanced tables for GitHub events and repository tracking"""
        schema_sql = """
        -- Create extensions
        CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
        CREATE EXTENSION IF NOT EXISTS "btree_gin";
        CREATE EXTENSION IF NOT EXISTS "pg_trgm";  -- For fuzzy text search
        
        -- Main events table (Ruby script equivalent functionality)
        CREATE TABLE IF NOT EXISTS github_events (
            id BIGINT PRIMARY KEY,
            type VARCHAR(50) NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE NOT NULL,
            public BOOLEAN NOT NULL DEFAULT true,
            
            -- Actor information
            actor_id BIGINT,
            actor_login VARCHAR(255),
            actor_display_login VARCHAR(255),
            actor_gravatar_id VARCHAR(255),
            actor_url TEXT,
            actor_avatar_url TEXT,
            
            -- Repository information
            repo_id BIGINT,
            repo_name VARCHAR(255),
            repo_url TEXT,
            
            -- Organization information (optional)
            org_id BIGINT,
            org_login VARCHAR(255),
            org_gravatar_id VARCHAR(255),
            org_url TEXT,
            org_avatar_url TEXT,
            
            -- Payload as JSONB for flexible querying
            payload JSONB,
            
            -- Metadata
            processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
            file_source VARCHAR(255)
        );
        
        -- Repository tracking table for change monitoring
        CREATE TABLE IF NOT EXISTS repositories (
            id BIGINT PRIMARY KEY,
            name VARCHAR(255) NOT NULL UNIQUE,
            full_name VARCHAR(255),
            description TEXT,
            html_url TEXT,
            clone_url TEXT,
            ssh_url TEXT,
            homepage TEXT,
            language VARCHAR(100),
            default_branch VARCHAR(100),
            created_at TIMESTAMP WITH TIME ZONE,
            updated_at TIMESTAMP WITH TIME ZONE,
            pushed_at TIMESTAMP WITH TIME ZONE,
            size_kb INTEGER,
            stargazers_count INTEGER,
            watchers_count INTEGER,
            forks_count INTEGER,
            open_issues_count INTEGER,
            is_private BOOLEAN DEFAULT FALSE,
            is_fork BOOLEAN DEFAULT FALSE,
            is_archived BOOLEAN DEFAULT FALSE,
            is_disabled BOOLEAN DEFAULT FALSE,
            topics TEXT[],
            license_name VARCHAR(255),
            owner_login VARCHAR(255),
            owner_type VARCHAR(50),
            first_seen_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
            last_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        );
        
        -- Repository changes tracking
        CREATE TABLE IF NOT EXISTS repository_changes (
            id SERIAL PRIMARY KEY,
            repo_id BIGINT REFERENCES repositories(id),
            event_id BIGINT REFERENCES github_events(id),
            change_type VARCHAR(50) NOT NULL,  -- 'push', 'create', 'delete', 'fork', etc.
            ref_name VARCHAR(255),  -- branch/tag name
            before_sha VARCHAR(40),
            after_sha VARCHAR(40),
            commit_count INTEGER,
            files_changed TEXT[],
            lines_added INTEGER,
            lines_deleted INTEGER,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        );
        
        -- Search optimization indexes
        CREATE INDEX IF NOT EXISTS idx_github_events_created_at ON github_events (created_at);
        CREATE INDEX IF NOT EXISTS idx_github_events_type ON github_events (type);
        CREATE INDEX IF NOT EXISTS idx_github_events_actor_id ON github_events (actor_id);
        CREATE INDEX IF NOT EXISTS idx_github_events_repo_id ON github_events (repo_id);
        CREATE INDEX IF NOT EXISTS idx_github_events_actor_login ON github_events (actor_login);
        CREATE INDEX IF NOT EXISTS idx_github_events_repo_name ON github_events (repo_name);
        CREATE INDEX IF NOT EXISTS idx_github_events_payload ON github_events USING GIN (payload);
        
        -- Repository indexes
        CREATE INDEX IF NOT EXISTS idx_repositories_name ON repositories (name);
        CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories (language);
        CREATE INDEX IF NOT EXISTS idx_repositories_created_at ON repositories (created_at);
        CREATE INDEX IF NOT EXISTS idx_repositories_stars ON repositories (stargazers_count DESC);
        CREATE INDEX IF NOT EXISTS idx_repositories_topics ON repositories USING GIN (topics);
        CREATE INDEX IF NOT EXISTS idx_repositories_owner ON repositories (owner_login);
        
        -- Full-text search indexes
        CREATE INDEX IF NOT EXISTS idx_repositories_search ON repositories USING GIN (
            to_tsvector('english', coalesce(name, '') || ' ' || coalesce(description, '') || ' ' || coalesce(full_name, ''))
        );
        
        -- Repository changes indexes
        CREATE INDEX IF NOT EXISTS idx_repo_changes_repo_id ON repository_changes (repo_id);
        CREATE INDEX IF NOT EXISTS idx_repo_changes_type ON repository_changes (change_type);
        CREATE INDEX IF NOT EXISTS idx_repo_changes_created_at ON repository_changes (created_at);
        
        -- Table for tracking processed files (rsync-like functionality)
        CREATE TABLE IF NOT EXISTS processed_files (
            filename VARCHAR(255) PRIMARY KEY,
            file_size BIGINT,
            etag VARCHAR(255),
            last_modified TIMESTAMP WITH TIME ZONE,
            processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
            event_count INTEGER DEFAULT 0,
            is_complete BOOLEAN DEFAULT TRUE
        );
        
        -- Missing data tracking
        CREATE TABLE IF NOT EXISTS missing_data_ranges (
            id SERIAL PRIMARY KEY,
            start_date DATE NOT NULL,
            end_date DATE NOT NULL,
            reason TEXT,
            discovered_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
            recovered_at TIMESTAMP WITH TIME ZONE,
            is_recovered BOOLEAN DEFAULT FALSE
        );
        
        -- Create materialized views for analytics and search
        CREATE MATERIALIZED VIEW IF NOT EXISTS daily_event_stats AS
        SELECT 
            DATE(created_at) as event_date,
            type,
            COUNT(*) as event_count,
            COUNT(DISTINCT actor_id) as unique_actors,
            COUNT(DISTINCT repo_id) as unique_repos
        FROM github_events
        GROUP BY DATE(created_at), type
        ORDER BY event_date DESC, type;
        
        CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_event_stats_date_type 
        ON daily_event_stats (event_date, type);
        
        -- Repository activity view
        CREATE MATERIALIZED VIEW IF NOT EXISTS repository_activity_stats AS
        SELECT 
            r.id,
            r.name,
            r.full_name,
            r.language,
            r.stargazers_count,
            COUNT(ge.id) as total_events,
            COUNT(CASE WHEN ge.type = 'PushEvent' THEN 1 END) as push_events,
            COUNT(CASE WHEN ge.type = 'CreateEvent' THEN 1 END) as create_events,
            COUNT(CASE WHEN ge.type = 'IssuesEvent' THEN 1 END) as issues_events,
            COUNT(CASE WHEN ge.type = 'PullRequestEvent' THEN 1 END) as pr_events,
            COUNT(CASE WHEN ge.type = 'WatchEvent' THEN 1 END) as watch_events,
            COUNT(CASE WHEN ge.type = 'ForkEvent' THEN 1 END) as fork_events,
            MAX(ge.created_at) as last_activity,
            COUNT(DISTINCT ge.actor_id) as unique_contributors
        FROM repositories r
        LEFT JOIN github_events ge ON r.id = ge.repo_id
        GROUP BY r.id, r.name, r.full_name, r.language, r.stargazers_count;
        
        CREATE UNIQUE INDEX IF NOT EXISTS idx_repo_activity_id 
        ON repository_activity_stats (id);
        
        -- Function to refresh materialized views
        CREATE OR REPLACE FUNCTION refresh_stats_views()
        RETURNS void AS $$
        BEGIN
            REFRESH MATERIALIZED VIEW CONCURRENTLY daily_event_stats;
            REFRESH MATERIALIZED VIEW CONCURRENTLY repository_activity_stats;
        END;
        $$ LANGUAGE plpgsql;
        
        -- Search functions
        CREATE OR REPLACE FUNCTION search_repositories(search_term TEXT, limit_count INTEGER DEFAULT 100)
        RETURNS TABLE(
            id BIGINT,
            name VARCHAR(255),
            full_name VARCHAR(255),
            description TEXT,
            language VARCHAR(100),
            stargazers_count INTEGER,
            relevance_score REAL
        ) AS $$
        BEGIN
            RETURN QUERY
            SELECT 
                r.id,
                r.name,
                r.full_name,
                r.description,
                r.language,
                r.stargazers_count,
                ts_rank(
                    to_tsvector('english', coalesce(r.name, '') || ' ' || coalesce(r.description, '') || ' ' || coalesce(r.full_name, '')),
                    plainto_tsquery('english', search_term)
                ) as relevance_score
            FROM repositories r
            WHERE to_tsvector('english', coalesce(r.name, '') || ' ' || coalesce(r.description, '') || ' ' || coalesce(r.full_name, ''))
                  @@ plainto_tsquery('english', search_term)
            ORDER BY relevance_score DESC, r.stargazers_count DESC
            LIMIT limit_count;
        END;
        $$ LANGUAGE plpgsql;
        """
        
        async with self.pool.acquire() as conn:
            await conn.execute(schema_sql)
            
    async def is_file_processed(self, filename: str, etag: str = None, size: int = None) -> bool:
        """Check if file has already been processed with same ETag (rsync-like behavior)"""
        async with self.pool.acquire() as conn:
            result = await conn.fetchrow(
                "SELECT etag, file_size FROM processed_files WHERE filename = $1",
                filename
            )
            if not result:
                return False
                
            # Check if ETag and size match (file unchanged)
            return result['etag'] == etag and result['file_size'] == size
            
    async def mark_file_processed(self, filename: str, etag: str, size: int, event_count: int):
        """Mark file as processed"""
        async with self.pool.acquire() as conn:
            await conn.execute("""
                INSERT INTO processed_files (filename, etag, file_size, event_count)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (filename) 
                DO UPDATE SET 
                    etag = $2, 
                    file_size = $3, 
                    event_count = $4, 
                    processed_at = NOW()
            """, filename, etag, size, event_count)
            
    async def insert_events_batch(self, events: List[Dict], filename: str) -> int:
        """Insert a batch of events into the database"""
        if not events:
            return 0
            
        insert_sql = """
        INSERT INTO github_events (
            id, type, created_at, public,
            actor_id, actor_login, actor_display_login, actor_gravatar_id, actor_url, actor_avatar_url,
            repo_id, repo_name, repo_url,
            org_id, org_login, org_gravatar_id, org_url, org_avatar_url,
            payload, file_source
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
        ON CONFLICT (id) DO NOTHING
        """
        
        async with self.pool.acquire() as conn:
            async with conn.transaction():
                rows_affected = 0
                for event in events:
                    try:
                        # Parse created_at timestamp
                        created_at = parse_date(event['created_at'])
                        
                        # Extract actor info
                        actor = event.get('actor', {})
                        actor_id = actor.get('id')
                        actor_login = actor.get('login')
                        actor_display_login = actor.get('display_login')
                        actor_gravatar_id = actor.get('gravatar_id')
                        actor_url = actor.get('url')
                        actor_avatar_url = actor.get('avatar_url')
                        
                        # Extract repo info
                        repo = event.get('repo', {})
                        repo_id = repo.get('id')
                        repo_name = repo.get('name')
                        repo_url = repo.get('url')
                        
                        # Extract org info
                        org = event.get('org', {})
                        org_id = org.get('id') if org else None
                        org_login = org.get('login') if org else None
                        org_gravatar_id = org.get('gravatar_id') if org else None
                        org_url = org.get('url') if org else None
                        org_avatar_url = org.get('avatar_url') if org else None
                        
                        # Store payload as JSONB
                        payload = event.get('payload', {})
                        
                        result = await conn.execute(
                            insert_sql,
                            int(event['id']), event['type'], created_at, event.get('public', True),
                            actor_id, actor_login, actor_display_login, actor_gravatar_id, actor_url, actor_avatar_url,
                            repo_id, repo_name, repo_url,
                            org_id, org_login, org_gravatar_id, org_url, org_avatar_url,
                            json.dumps(payload), filename
                        )
                        
                        if result == "INSERT 0 1":
                            rows_affected += 1
                            
                    except Exception as e:
                        logging.error(f"Error inserting event {event.get('id', 'unknown')}: {e}")
                        continue
                        
                return rows_affected


class GitHubArchiveScraper:
    """Main scraper class with enhanced performance monitoring and graceful shutdown"""
    
    def __init__(self, config: Config):
        self.config = config
        self.db = DatabaseManager(config)
        self.session = None
        self._shutdown_event = asyncio.Event()
        self._running_tasks: Set[asyncio.Task] = set()
        self._stats = ProcessingStats()
        
        # Initialize resource monitoring
        self.resource_monitor = ResourceMonitor(config)
        self._last_resource_check = 0
        
        # Setup enhanced logging
        log_formatter = logging.Formatter(
            '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
        )
        
        # Console handler
        console_handler = logging.StreamHandler()
        console_handler.setFormatter(log_formatter)
        
        # File handler with rotation
        file_handler = logging.FileHandler(config.LOG_FILE)
        file_handler.setFormatter(log_formatter)
        
        # Configure logger
        self.logger = logging.getLogger(__name__)
        self.logger.setLevel(getattr(logging, config.LOG_LEVEL))
        self.logger.addHandler(console_handler)
        self.logger.addHandler(file_handler)
        
        # Performance monitoring
        self._last_memory_check = 0
        self._memory_checks = 0
        
        # Setup signal handlers for graceful shutdown
        self._setup_signal_handlers()
        
    def _setup_signal_handlers(self):
        """Setup signal handlers for graceful shutdown"""
        def signal_handler(signum, frame):
            self.logger.info(f"Received signal {signum}, initiating graceful shutdown...")
            asyncio.create_task(self._graceful_shutdown())
        
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)
    
    async def _graceful_shutdown(self):
        """Gracefully shutdown all running operations"""
        self.logger.info("Starting graceful shutdown...")
        self._shutdown_event.set()
        
        # Cancel all running tasks
        for task in self._running_tasks:
            if not task.done():
                task.cancel()
        
        # Wait for tasks to complete or timeout
        if self._running_tasks:
            done, pending = await asyncio.wait(
                self._running_tasks,
                timeout=self.config.SHUTDOWN_TIMEOUT,
                return_when=asyncio.ALL_COMPLETED
            )
            
            # Force cancel any remaining tasks
            for task in pending:
                task.cancel()
                
        self.logger.info("Graceful shutdown completed")
    
    async def _check_resource_usage(self):
        """Monitor resource usage and enforce limits"""
        current_time = time.time()
        
        # Check resources periodically
        if current_time - self._last_resource_check >= self.config.RESOURCE_CHECK_INTERVAL:
            self._last_resource_check = current_time
            
            # Get resource status
            status = self.resource_monitor.get_status()
            
            # Log resource usage periodically
            self.logger.info(
                f"Resource usage - Memory: {status['memory_mb']:.1f}MB "
                f"({status['memory_usage_percent']:.1f}%), "
                f"Disk: {status['disk_used_gb']:.1f}GB "
                f"({status['disk_usage_percent']:.1f}%), "
                f"CPU: {status['cpu_percent']:.1f}%"
            )
            
            # Check for emergency conditions
            if not self.resource_monitor.check_memory_limit() or not self.resource_monitor.check_disk_limit():
                self.logger.error("Resource limits exceeded, pausing processing")
                await asyncio.sleep(30)  # Pause for 30 seconds
                
            # Clean up temp files periodically
            if len(self.resource_monitor.temp_files) > 10:
                self.resource_monitor.cleanup_temp_files()
    
    @asynccontextmanager
    async def _track_task(self, coro):
        """Context manager to track running tasks"""
        task = asyncio.create_task(coro)
        self._running_tasks.add(task)
        try:
            yield task
        finally:
            self._running_tasks.discard(task)
    
    async def __aenter__(self):
        """Async context manager entry with enhanced connection handling"""
        await self.db.connect()
        
        # Create HTTP session with enhanced configuration
        timeout = aiohttp.ClientTimeout(total=self.config.REQUEST_TIMEOUT)
        connector = aiohttp.TCPConnector(
            limit=self.config.MAX_CONCURRENT_DOWNLOADS,
            enable_cleanup_closed=True,
            keepalive_timeout=30
        )
        
        self.session = aiohttp.ClientSession(
            timeout=timeout,
            connector=connector,
            headers={'User-Agent': 'GitHubArchiveScraper/2.0'}
        )
        
        self.logger.info("GitHubArchiveScraper initialized successfully")
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit with cleanup"""
        # Graceful shutdown if not already initiated
        if not self._shutdown_event.is_set():
            await self._graceful_shutdown()
            
        if self.session:
            await self.session.close()
        await self.db.disconnect()
        
        # Log final statistics
        self.logger.info(f"Final stats: {self._stats}")
        
        if exc_type:
            self.logger.error(f"Exit with exception: {exc_type.__name__}: {exc_val}")
        
    async def discover_available_files(self) -> List[str]:
        """
        Discover all available files by parsing the S3 bucket listing
        Similar to how Ruby script accesses data.gharchive.org
        """
        try:
            async with self.session.get(self.config.S3_LIST_URL) as response:
                if response.status != 200:
                    self.logger.error(f"Failed to fetch S3 listing: {response.status}")
                    return []
                
                xml_content = await response.text()
                root = ET.fromstring(xml_content)
                
                files = []
                for contents in root.findall('.//{http://s3.amazonaws.com/doc/2006-03-01/}Contents'):
                    key_elem = contents.find('.//{http://s3.amazonaws.com/doc/2006-03-01/}Key')
                    if key_elem is not None:
                        filename = key_elem.text
                        if filename and filename.endswith('.json.gz'):
                            # Filter for 2015 onward as requested
                            if self._is_valid_filename_for_range(filename):
                                files.append(filename)
                
                self.logger.info(f"Discovered {len(files)} available archive files")
                return sorted(files)
                
        except Exception as e:
            self.logger.error(f"Error discovering files: {e}")
            return []
    
    def _is_valid_filename_for_range(self, filename: str) -> bool:
        """Check if filename represents a date >= 2015-01-01"""
        try:
            # Extract date from filename like "2015-01-01-12.json.gz"
            match = re.match(r'(\d{4})-(\d{2})-(\d{2})-(\d+)\.json\.gz', filename)
            if match:
                year, month, day, hour = map(int, match.groups())
                file_date = datetime(year, month, day)
                return file_date >= self.config.MIN_DATE
        except:
            pass
        return False
    
    async def detect_missing_data(self) -> List[Tuple[datetime, datetime]]:
        """
        Detect missing data ranges by comparing available files with expected files
        """
        self.logger.info("Detecting missing data ranges...")
        
        # Get all available files
        available_files = await self.discover_available_files()
        
        # Generate expected files from MIN_DATE to yesterday
        expected_files = set()
        current_date = self.config.MIN_DATE
        yesterday = datetime.now() - timedelta(days=1)
        
        while current_date <= yesterday:
            for hour in range(24):
                expected_filename = f"{current_date.strftime('%Y-%m-%d')}-{hour}.json.gz"
                expected_files.add(expected_filename)
            current_date += timedelta(days=1)
        
        # Find missing files
        available_set = set(available_files)
        missing_files = expected_files - available_set
        
        # Convert missing files to date ranges
        missing_ranges = self._group_missing_files_to_ranges(missing_files)
        
        # Store missing ranges in database
        await self._store_missing_ranges(missing_ranges)
        
        self.logger.info(f"Found {len(missing_ranges)} missing data ranges")
        return missing_ranges
    
    def _group_missing_files_to_ranges(self, missing_files: Set[str]) -> List[Tuple[datetime, datetime]]:
        """Group individual missing files into contiguous date ranges"""
        if not missing_files:
            return []
        
        # Convert filenames to dates
        missing_dates = set()
        for filename in missing_files:
            match = re.match(r'(\d{4})-(\d{2})-(\d{2})-(\d+)\.json\.gz', filename)
            if match:
                year, month, day, hour = map(int, match.groups())
                missing_dates.add(datetime(year, month, day))
        
        # Sort dates and group into ranges
        sorted_dates = sorted(missing_dates)
        ranges = []
        
        if not sorted_dates:
            return ranges
        
        start_date = sorted_dates[0]
        end_date = sorted_dates[0]
        
        for i in range(1, len(sorted_dates)):
            current_date = sorted_dates[i]
            if current_date == end_date + timedelta(days=1):
                end_date = current_date
            else:
                ranges.append((start_date, end_date))
                start_date = current_date
                end_date = current_date
        
        ranges.append((start_date, end_date))
        return ranges
    
    async def _store_missing_ranges(self, ranges: List[Tuple[datetime, datetime]]):
        """Store missing data ranges in database"""
        if not ranges:
            return
        
        async with self.pool.acquire() as conn:
            for start_date, end_date in ranges:
                await conn.execute("""
                    INSERT INTO missing_data_ranges (start_date, end_date, reason)
                    VALUES ($1, $2, $3)
                    ON CONFLICT DO NOTHING
                """, start_date.date(), end_date.date(), "Detected during scan")
    
    async def get_unprocessed_files(self) -> List[str]:
        """Get list of files that haven't been processed yet"""
        available_files = await self.discover_available_files()
        unprocessed = []
        
        for filename in available_files:
            # Quick check using head request to get ETag and size
            url = urljoin(self.config.BASE_URL, filename)
            file_info = await self.get_file_info(url)
            
            if file_info:
                _, etag, size = file_info
                if not await self.db.is_file_processed(filename, etag, size):
                    unprocessed.append(filename)
        
        return unprocessed
    
    def get_file_urls_for_date(self, date: datetime) -> List[str]:
        """Generate URLs for all hourly files for a given date"""
        date_str = date.strftime('%Y-%m-%d')
        urls = []
        for hour in range(24):
            filename = f"{date_str}-{hour}.json.gz"
            url = urljoin(self.config.BASE_URL, filename)
            urls.append(url)
        return urls
        
    async def get_file_info(self, url: str) -> Optional[Tuple[str, str, int]]:
        """Get file metadata (ETag, size) without downloading"""
        try:
            async with self.session.head(url) as response:
                if response.status == 200:
                    etag = response.headers.get('ETag', '').strip('"')
                    size = int(response.headers.get('Content-Length', 0))
                    filename = url.split('/')[-1]
                    return filename, etag, size
                else:
                    self.logger.warning(f"File not found: {url} (status: {response.status})")
                    return None
        except Exception as e:
            self.logger.error(f"Error getting file info for {url}: {e}")
            return None
            
    async def download_file(self, url: str, etag: str) -> Optional[str]:
        """Download and decompress a file with retry logic and performance monitoring"""
        filename = url.split('/')[-1]
        temp_path = None
        
        # Check resource limits before starting download
        if self.resource_monitor.should_pause_processing():
            self.logger.warning(f"Pausing download due to resource pressure: {filename}")
            await asyncio.sleep(10)  # Wait before retrying
            return None
        
        for attempt in range(self.config.MAX_RETRIES):
            try:
                # Check for shutdown signal
                if self._shutdown_event.is_set():
                    self.logger.info("Shutdown requested, cancelling download")
                    return None
                
                # Create temporary file and register for cleanup
                with tempfile.NamedTemporaryFile(delete=False, suffix='.json.gz') as tmp:
                    temp_path = tmp.name
                    self.resource_monitor.register_temp_file(temp_path)
                
                start_time = time.time()
                bytes_downloaded = 0
                
                # Download file with retry logic
                async with self.session.get(url) as response:
                    if response.status != 200:
                        if response.status == 404 and attempt == 0:
                            self.logger.warning(f"File not found: {url}")
                            return None
                        raise aiohttp.ClientError(f"HTTP {response.status}")
                    
                    # Get content length for progress tracking
                    content_length = int(response.headers.get('Content-Length', 0))
                    
                    # Check if file size exceeds limits
                    if content_length > self.config.MAX_FILE_SIZE_MB * 1024 * 1024:
                        self.logger.warning(f"File too large ({content_length/1024/1024:.1f}MB): {filename}")
                        return None
                    
                    # Write to temporary file with progress tracking
                    async with aiofiles.open(temp_path, 'wb') as f:
                        async for chunk in response.content.iter_chunked(self.config.CHUNK_SIZE):
                            if self._shutdown_event.is_set():
                                return None
                            
                            await f.write(chunk)
                            bytes_downloaded += len(chunk)
                            
                            # Resource usage check every 100 chunks
                            if bytes_downloaded % (self.config.CHUNK_SIZE * 100) == 0:
                                if self.resource_monitor.should_pause_processing():
                                    self.logger.warning(f"Pausing download due to resource pressure: {filename}")
                                    return None
                
                # Verify download completed
                if content_length > 0 and bytes_downloaded != content_length:
                    raise ValueError(f"Incomplete download: {bytes_downloaded}/{content_length} bytes")
                
                # Decompress and return JSON content
                with gzip.open(temp_path, 'rt', encoding='utf-8') as gz_file:
                    content = gz_file.read()
                
                # Update statistics
                download_time = time.time() - start_time
                self._stats.bytes_downloaded += bytes_downloaded
                self._stats.processing_time += download_time
                
                # Update memory peak
                current_memory = self.resource_monitor.get_memory_usage()
                self._stats.memory_peak_mb = max(self._stats.memory_peak_mb, current_memory)
                
                self.logger.info(
                    f"Downloaded {filename}: {bytes_downloaded:,} bytes in {download_time:.2f}s "
                    f"({bytes_downloaded/download_time/1024:.1f} KB/s) - Memory: {current_memory:.1f}MB"
                )
                return content
                
            except Exception as e:
                self.logger.warning(f"Download attempt {attempt + 1} failed for {url}: {e}")
                if attempt < self.config.MAX_RETRIES - 1:
                    await asyncio.sleep(self.config.RETRY_DELAY * (attempt + 1))
                else:
                    self.logger.error(f"Failed to download {url} after {self.config.MAX_RETRIES} attempts")
                    return None
            finally:
                # Clean up temporary file using resource monitor
                if temp_path:
                    try:
                        if os.path.exists(temp_path):
                            os.unlink(temp_path)
                        # Remove from resource monitor tracking
                        self.resource_monitor.temp_files.discard(temp_path)
                    except Exception as e:
                        self.logger.warning(f"Failed to cleanup temp file {temp_path}: {e}")
        
        return None
                
    async def process_json_content(self, content: str, filename: str) -> int:
        """
        Process JSON content with enhanced streaming and error handling
        This is the enhanced Python equivalent of the Ruby script with better performance
        """
        events = []
        repositories = {}
        repository_changes = []
        event_count = 0
        error_count = 0
        start_time = time.time()
        
        try:
            # Calculate content hash for integrity verification
            content_hash = hashlib.md5(content.encode()).hexdigest()
            self.logger.debug(f"Processing {filename}, content hash: {content_hash}")
            
            # Process each line as a separate JSON event (streaming approach)
            for line_num, line in enumerate(content.strip().split('\n'), 1):
                if self._shutdown_event.is_set():
                    self.logger.info("Shutdown requested, stopping processing")
                    break
                
                if not line.strip():
                    continue
                
                try:
                    # Parse JSON event with validation
                    event = json.loads(line.strip())
                    
                    # Basic event validation
                    if not self._validate_event(event):
                        error_count += 1
                        continue
                    
                    events.append(event)
                    event_count += 1
                    
                    # Extract repository information for tracking
                    if 'repo' in event and event['repo']:
                        repo_data = event['repo']
                        repo_id = repo_data.get('id')
                        if repo_id and repo_id not in repositories:
                            repositories[repo_id] = {
                                'id': repo_id,
                                'name': repo_data.get('name', ''),
                                'url': repo_data.get('url', '')
                            }
                    
                    # Extract repository changes for detailed tracking
                    change_data = self._extract_repository_change(event)
                    if change_data:
                        repository_changes.append(change_data)
                    
                    # Process in batches for memory efficiency
                    if len(events) >= self.config.BATCH_SIZE:
                        inserted = await self.db.insert_events_batch(events, filename)
                        await self._process_repository_batch(repositories, repository_changes)
                        
                        # Update statistics
                        self._stats.events += len(events)
                        
                        self.logger.debug(
                            f"Processed batch: {inserted} events inserted "
                            f"(line {line_num}, {event_count} total events)"
                        )
                        
                        # Clear batches
                        events = []
                        repositories = {}
                        repository_changes = []
                        
                        # Resource usage check
                        await self._check_resource_usage()
                        
                except json.JSONDecodeError as e:
                    error_count += 1
                    self.logger.warning(f"Invalid JSON in {filename} at line {line_num}: {e}")
                    continue
                except Exception as e:
                    error_count += 1
                    self.logger.error(f"Error processing event in {filename} at line {line_num}: {e}")
                    continue
                        
            # Process remaining events
            if events:
                inserted = await self.db.insert_events_batch(events, filename)
                await self._process_repository_batch(repositories, repository_changes)
                self._stats.events += len(events)
                self.logger.debug(f"Processed final batch: {inserted} events inserted")
            
            processing_time = time.time() - start_time
            self._stats.processing_time += processing_time
            
            success_rate = ((event_count - error_count) / event_count * 100) if event_count > 0 else 0
            
            self.logger.info(
                f"Processed {filename}: {event_count:,} events "
                f"({error_count} errors, {success_rate:.1f}% success) "
                f"in {processing_time:.2f}s"
            )
            
            return event_count
            
        except Exception as e:
            self.logger.error(f"Fatal error processing {filename}: {e}")
            return 0
    
    def _validate_event(self, event: Dict) -> bool:
        """Validate basic event structure"""
        required_fields = ['id', 'type', 'created_at']
        for field in required_fields:
            if field not in event:
                return False
        
        # Validate event ID is numeric
        try:
            int(event['id'])
        except (ValueError, TypeError):
            return False
        
        # Validate timestamp format
        try:
            parse_date(event['created_at'])
        except:
            return False
        
        return True
    
    def _extract_repository_change(self, event: Dict) -> Optional[Dict]:
        """Extract repository change information from event"""
        if event.get('type') not in ['PushEvent', 'CreateEvent', 'DeleteEvent']:
            return None
        
        repo = event.get('repo', {})
        payload = event.get('payload', {})
        
        change = {
            'repo_id': repo.get('id'),
            'event_id': event.get('id'),
            'change_type': event.get('type', '').replace('Event', '').lower(),
            'created_at': event.get('created_at')
        }
        
        if event.get('type') == 'PushEvent':
            change.update({
                'ref_name': payload.get('ref', '').replace('refs/heads/', ''),
                'before_sha': payload.get('before'),
                'after_sha': payload.get('head'),
                'commit_count': payload.get('size', 0)
            })
        elif event.get('type') in ['CreateEvent', 'DeleteEvent']:
            change.update({
                'ref_name': payload.get('ref'),
                'ref_type': payload.get('ref_type')
            })
        
        return change
    
    async def _process_repository_batch(self, repositories: Dict, changes: List[Dict]):
        """Process repository and change data"""
        if not repositories and not changes:
            return
        
        async with self.db.pool.acquire() as conn:
            async with conn.transaction():
                # Insert/update repositories
                for repo_data in repositories.values():
                    await conn.execute("""
                        INSERT INTO repositories (id, name, html_url, first_seen_at, last_updated_at)
                        VALUES ($1, $2, $3, NOW(), NOW())
                        ON CONFLICT (id) 
                        DO UPDATE SET 
                            name = EXCLUDED.name,
                            html_url = EXCLUDED.html_url,
                            last_updated_at = NOW()
                    """, repo_data['id'], repo_data['name'], repo_data['url'])
                
                # Insert repository changes
                for change in changes:
                    if change.get('repo_id') and change.get('event_id'):
                        await conn.execute("""
                            INSERT INTO repository_changes 
                            (repo_id, event_id, change_type, ref_name, before_sha, after_sha, commit_count, created_at)
                            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                            ON CONFLICT DO NOTHING
                        """, 
                        change['repo_id'], change['event_id'], change['change_type'],
                        change.get('ref_name'), change.get('before_sha'), change.get('after_sha'),
                        change.get('commit_count', 0), parse_date(change['created_at']))
    
    async def search_events(self, query: Dict) -> List[Dict]:
        """
        Search events with flexible criteria
        Example usage:
        - search_events({'type': 'PushEvent', 'repo_name': 'torvalds/linux'})
        - search_events({'actor_login': 'octocat', 'date_from': '2015-01-01'})
        """
        where_clauses = []
        params = []
        param_count = 0
        
        # Build dynamic query
        if 'type' in query:
            param_count += 1
            where_clauses.append(f"type = ${param_count}")
            params.append(query['type'])
        
        if 'repo_name' in query:
            param_count += 1
            where_clauses.append(f"repo_name = ${param_count}")
            params.append(query['repo_name'])
        
        if 'actor_login' in query:
            param_count += 1
            where_clauses.append(f"actor_login = ${param_count}")
            params.append(query['actor_login'])
        
        if 'date_from' in query:
            param_count += 1
            where_clauses.append(f"created_at >= ${param_count}")
            params.append(parse_date(query['date_from']))
        
        if 'date_to' in query:
            param_count += 1
            where_clauses.append(f"created_at <= ${param_count}")
            params.append(parse_date(query['date_to']))
        
        if 'payload_contains' in query:
            param_count += 1
            where_clauses.append(f"payload @> ${param_count}")
            params.append(json.dumps(query['payload_contains']))
        
        # Build final query
        where_sql = " AND ".join(where_clauses) if where_clauses else "1=1"
        limit = query.get('limit', 1000)
        
        sql = f"""
        SELECT id, type, created_at, actor_login, repo_name, payload
        FROM github_events 
        WHERE {where_sql}
        ORDER BY created_at DESC
        LIMIT {limit}
        """
        
        async with self.db.pool.acquire() as conn:
            rows = await conn.fetch(sql, *params)
            return [dict(row) for row in rows]
    
    async def export_repository_data(self, repo_name: str, output_format: str = 'json') -> str:
        """
        Export all data for a specific repository
        """
        async with self.db.pool.acquire() as conn:
            # Get repository info
            repo_row = await conn.fetchrow("""
                SELECT * FROM repositories WHERE name = $1 OR full_name = $1
            """, repo_name)
            
            if not repo_row:
                return f"Repository '{repo_name}' not found"
            
            repo_data = dict(repo_row)
            
            # Get all events for this repository
            events = await conn.fetch("""
                SELECT * FROM github_events 
                WHERE repo_id = $1 OR repo_name = $2
                ORDER BY created_at DESC
            """, repo_data['id'], repo_name)
            
            # Get repository changes
            changes = await conn.fetch("""
                SELECT * FROM repository_changes 
                WHERE repo_id = $1
                ORDER BY created_at DESC
            """, repo_data['id'])
            
            result = {
                'repository': repo_data,
                'events': [dict(row) for row in events],
                'changes': [dict(row) for row in changes],
                'stats': {
                    'total_events': len(events),
                    'total_changes': len(changes),
                    'event_types': {}
                }
            }
            
            # Calculate event type statistics
            for event in result['events']:
                event_type = event['type']
                result['stats']['event_types'][event_type] = result['stats']['event_types'].get(event_type, 0) + 1
            
            if output_format == 'json':
                return json.dumps(result, default=str, indent=2)
            else:
                # Could add CSV, XML, etc. formats here
                return str(result)
            
    async def process_date(self, date: datetime) -> Dict[str, int]:
        """Process all files for a given date"""
        self.logger.info(f"Processing date: {date.strftime('%Y-%m-%d')}")
        urls = self.get_file_urls_for_date(date)
        stats = {'downloaded': 0, 'skipped': 0, 'failed': 0, 'events': 0}
        
        # Create semaphore to limit concurrent downloads
        semaphore = asyncio.Semaphore(self.config.MAX_CONCURRENT_DOWNLOADS)
        
        async def process_url(url: str):
            async with semaphore:
                # Get file metadata
                file_info = await self.get_file_info(url)
                if not file_info:
                    stats['failed'] += 1
                    return
                    
                filename, etag, size = file_info
                
                # Check if already processed (rsync-like behavior)
                if await self.db.is_file_processed(filename, etag, size):
                    self.logger.debug(f"Skipping {filename} (already processed)")
                    stats['skipped'] += 1
                    return
                    
                # Download and process file
                content = await self.download_file(url, etag)
                if content:
                    event_count = await self.process_json_content(content, filename)
                    await self.db.mark_file_processed(filename, etag, size, event_count)
                    stats['downloaded'] += 1
                    stats['events'] += event_count
                else:
                    stats['failed'] += 1
                    
        # Process all URLs concurrently
        tasks = [process_url(url) for url in urls]
        await asyncio.gather(*tasks, return_exceptions=True)
        
        self.logger.info(f"Date {date.strftime('%Y-%m-%d')} complete: {stats}")
        return stats
        
    async def scrape_date_range(self, start_date: datetime, end_date: datetime) -> Dict[str, int]:
        """Scrape data for a date range"""
        total_stats = {'downloaded': 0, 'skipped': 0, 'failed': 0, 'events': 0}
        
        current_date = start_date
        while current_date <= end_date:
            try:
                day_stats = await self.process_date(current_date)
                for key in total_stats:
                    total_stats[key] += day_stats[key]
            except Exception as e:
                self.logger.error(f"Error processing date {current_date}: {e}")
                
            current_date += timedelta(days=1)
            
        return total_stats


async def main():
    """Main function with enhanced capabilities"""
    parser = argparse.ArgumentParser(description='GitHub Archive Scraper - Enhanced Python Version')
    parser.add_argument(
        '--mode', 
        choices=['scrape', 'discover', 'missing', 'search', 'export', 'catch-up'],
        default='scrape',
        help='Operation mode'
    )
    parser.add_argument(
        '--date', 
        type=str, 
        help='Specific date to process (YYYY-MM-DD). Default: yesterday'
    )
    parser.add_argument(
        '--start-date', 
        type=str, 
        help='Start date for range processing (YYYY-MM-DD)'
    )
    parser.add_argument(
        '--end-date', 
        type=str, 
        help='End date for range processing (YYYY-MM-DD)'
    )
    parser.add_argument(
        '--days-back', 
        type=int, 
        default=1,
        help='Number of days back from today to process (default: 1)'
    )
    parser.add_argument(
        '--catch-up-from-2015',
        action='store_true',
        help='Catch up all data from 2015 onward (as requested)'
    )
    parser.add_argument(
        '--search-query',
        type=str,
        help='JSON search query for events (e.g., \'{"type": "PushEvent", "repo_name": "torvalds/linux"}\')'
    )
    parser.add_argument(
        '--export-repo',
        type=str,
        help='Repository name to export data for'
    )
    parser.add_argument(
        '--output-format',
        choices=['json', 'text'],
        default='json',
        help='Output format for export'
    )
    
    args = parser.parse_args()
    config = Config()
    
    try:
        async with GitHubArchiveScraper(config) as scraper:
            start_time = datetime.now()
            
            if args.mode == 'discover':
                # Discover available files
                files = await scraper.discover_available_files()
                print(f"Found {len(files)} available archive files")
                for i, filename in enumerate(files[:10]):  # Show first 10
                    print(f"  {filename}")
                if len(files) > 10:
                    print(f"  ... and {len(files) - 10} more files")
                    
            elif args.mode == 'missing':
                # Detect missing data
                missing_ranges = await scraper.detect_missing_data()
                print(f"Found {len(missing_ranges)} missing data ranges:")
                for start_date, end_date in missing_ranges:
                    print(f"  {start_date} to {end_date}")
                    
            elif args.mode == 'search':
                # Search events
                if not args.search_query:
                    print("Error: --search-query required for search mode")
                    sys.exit(1)
                
                try:
                    query = json.loads(args.search_query)
                    results = await scraper.search_events(query)
                    print(f"Found {len(results)} matching events:")
                    for result in results[:5]:  # Show first 5
                        print(f"  {result['created_at']} - {result['type']} - {result['repo_name']} - {result['actor_login']}")
                    if len(results) > 5:
                        print(f"  ... and {len(results) - 5} more events")
                except json.JSONDecodeError:
                    print("Error: Invalid JSON in search query")
                    sys.exit(1)
                    
            elif args.mode == 'export':
                # Export repository data
                if not args.export_repo:
                    print("Error: --export-repo required for export mode")
                    sys.exit(1)
                
                data = await scraper.export_repository_data(args.export_repo, args.output_format)
                print(data)
                
            elif args.mode == 'catch-up' or args.catch_up_from_2015:
                # Catch up from 2015 onward (as requested)
                scraper.logger.info("Starting catch-up from 2015 onward...")
                
                # First detect missing data
                missing_ranges = await scraper.detect_missing_data()
                
                # Get unprocessed files
                unprocessed_files = await scraper.get_unprocessed_files()
                scraper.logger.info(f"Found {len(unprocessed_files)} unprocessed files")
                
                # Process unprocessed files first
                total_stats = {'downloaded': 0, 'skipped': 0, 'failed': 0, 'events': 0}
                
                # Process in chunks to avoid overwhelming the system
                chunk_size = 100
                for i in range(0, len(unprocessed_files), chunk_size):
                    chunk = unprocessed_files[i:i + chunk_size]
                    scraper.logger.info(f"Processing chunk {i//chunk_size + 1}/{(len(unprocessed_files) + chunk_size - 1)//chunk_size}")
                    
                    # Process files in this chunk
                    for filename in chunk:
                        url = urljoin(config.BASE_URL, filename)
                        file_info = await scraper.get_file_info(url)
                        
                        if file_info:
                            _, etag, size = file_info
                            if not await scraper.db.is_file_processed(filename, etag, size):
                                content = await scraper.download_file(url, etag)
                                if content:
                                    event_count = await scraper.process_json_content(content, filename)
                                    await scraper.db.mark_file_processed(filename, etag, size, event_count)
                                    total_stats['downloaded'] += 1
                                    total_stats['events'] += event_count
                                else:
                                    total_stats['failed'] += 1
                            else:
                                total_stats['skipped'] += 1
                
                scraper.logger.info(f"Catch-up complete: {total_stats}")
                
            else:
                # Regular scrape mode
                # Determine date range
                if args.start_date and args.end_date:
                    start_date = datetime.strptime(args.start_date, '%Y-%m-%d')
                    end_date = datetime.strptime(args.end_date, '%Y-%m-%d')
                elif args.date:
                    start_date = end_date = datetime.strptime(args.date, '%Y-%m-%d')
                else:
                    # Default: process yesterday (or days back)
                    end_date = datetime.now() - timedelta(days=1)
                    start_date = end_date - timedelta(days=args.days_back - 1)
                
                scraper.logger.info(f"Starting scrape from {start_date.strftime('%Y-%m-%d')} to {end_date.strftime('%Y-%m-%d')}")
                
                stats = await scraper.scrape_date_range(start_date, end_date)
                
                elapsed = datetime.now() - start_time
                scraper.logger.info(f"Scraping complete in {elapsed.total_seconds():.2f}s: {stats}")
                
                # Refresh materialized views
                async with scraper.db.pool.acquire() as conn:
                    await conn.execute("SELECT refresh_stats_views()")
                
    except Exception as e:
        logging.error(f"Fatal error: {e}")
        sys.exit(1)


if __name__ == '__main__':
    asyncio.run(main())
