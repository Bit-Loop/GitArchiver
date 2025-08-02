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
from config import Config
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
        
        -- Enhanced events table to capture EVERYTHING from GitHub API
        CREATE TABLE IF NOT EXISTS github_events (
            id BIGINT PRIMARY KEY,
            type VARCHAR(50) NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE NOT NULL,
            public BOOLEAN NOT NULL DEFAULT true,
            
            -- Complete Actor information (expanded)
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
            
            -- Complete Repository information (expanded)
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
            repo_keys_url TEXT,
            repo_collaborators_url TEXT,
            repo_teams_url TEXT,
            repo_hooks_url TEXT,
            repo_issue_events_url TEXT,
            repo_events_url TEXT,
            repo_assignees_url TEXT,
            repo_branches_url TEXT,
            repo_tags_url TEXT,
            repo_blobs_url TEXT,
            repo_git_tags_url TEXT,
            repo_git_refs_url TEXT,
            repo_trees_url TEXT,
            repo_statuses_url TEXT,
            repo_languages_url TEXT,
            repo_stargazers_url TEXT,
            repo_contributors_url TEXT,
            repo_subscribers_url TEXT,
            repo_subscription_url TEXT,
            repo_commits_url TEXT,
            repo_git_commits_url TEXT,
            repo_comments_url TEXT,
            repo_issue_comment_url TEXT,
            repo_contents_url TEXT,
            repo_compare_url TEXT,
            repo_merges_url TEXT,
            repo_archive_url TEXT,
            repo_downloads_url TEXT,
            repo_issues_url TEXT,
            repo_pulls_url TEXT,
            repo_milestones_url TEXT,
            repo_notifications_url TEXT,
            repo_labels_url TEXT,
            repo_releases_url TEXT,
            repo_deployments_url TEXT,
            repo_git_url TEXT,
            repo_ssh_url TEXT,
            repo_clone_url TEXT,
            repo_svn_url TEXT,
            repo_homepage TEXT,
            repo_size BIGINT,
            repo_stargazers_count BIGINT,
            repo_watchers_count BIGINT,
            repo_language VARCHAR(100),
            repo_has_issues BOOLEAN,
            repo_has_projects BOOLEAN,
            repo_has_downloads BOOLEAN,
            repo_has_wiki BOOLEAN,
            repo_has_pages BOOLEAN,
            repo_has_discussions BOOLEAN,
            repo_forks_count BIGINT,
            repo_mirror_url TEXT,
            repo_archived BOOLEAN,
            repo_disabled BOOLEAN,
            repo_open_issues_count BIGINT,
            repo_license_key VARCHAR(50),
            repo_license_name VARCHAR(255),
            repo_license_spdx_id VARCHAR(50),
            repo_license_url TEXT,
            repo_license_node_id VARCHAR(255),
            repo_allow_forking BOOLEAN,
            repo_is_template BOOLEAN,
            repo_web_commit_signoff_required BOOLEAN,
            repo_topics TEXT[], -- Array of topics
            repo_visibility VARCHAR(50),
            repo_forks BIGINT,
            repo_open_issues BIGINT,
            repo_watchers BIGINT,
            repo_default_branch VARCHAR(100),
            repo_created_at TIMESTAMP WITH TIME ZONE,
            repo_updated_at TIMESTAMP WITH TIME ZONE,
            repo_pushed_at TIMESTAMP WITH TIME ZONE,
            
            -- Complete Organization information (expanded, optional)
            org_id BIGINT,
            org_login VARCHAR(255),
            org_node_id VARCHAR(255),
            org_gravatar_id VARCHAR(255),
            org_url TEXT,
            org_avatar_url TEXT,
            org_html_url TEXT,
            org_followers_url TEXT,
            org_following_url TEXT,
            org_gists_url TEXT,
            org_starred_url TEXT,
            org_subscriptions_url TEXT,
            org_organizations_url TEXT,
            org_repos_url TEXT,
            org_events_url TEXT,
            org_received_events_url TEXT,
            org_type VARCHAR(50),
            org_user_view_type VARCHAR(50),
            org_site_admin BOOLEAN,
            
            -- Complete Payload as JSONB for ultra-flexible querying and complex nested data
            payload JSONB,
            
            -- Raw event data for complete preservation
            raw_event JSONB,
            
            -- Enhanced metadata
            processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
            file_source VARCHAR(255),
            api_source VARCHAR(255),
            data_version VARCHAR(10) DEFAULT '3.0'
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
            
    def validate_and_convert_event(self, event: Dict) -> Optional[Dict]:
        """
        Enhanced validation to extract EVERYTHING from GitHub API events
        Returns None if event is invalid, otherwise returns fully enriched event
        """
        try:
            # Required fields check
            if not all(field in event for field in ['id', 'type', 'created_at']):
                return None
                
            # Validate and convert ID
            try:
                event_id = int(event['id'])
            except (ValueError, TypeError):
                logging.warning(f"Invalid event ID: {event.get('id')}")
                return None
                
            # Validate event type - must be string and not numeric
            event_type = event.get('type')
            if not isinstance(event_type, str) or not event_type or event_type.isdigit():
                logging.warning(f"Invalid event type '{event_type}' for event {event_id}")
                return None
                
            # Validate timestamp
            try:
                created_at = parse_date(event['created_at'])
            except:
                logging.warning(f"Invalid timestamp for event {event_id}")
                return None
                
            # Helper function for safe integer conversion
            def safe_int(value):
                if value is None or value == '':
                    return None
                try:
                    return int(value)
                except (ValueError, TypeError):
                    return None
            
            # Helper function for safe boolean conversion
            def safe_bool(value):
                if value is None:
                    return None
                if isinstance(value, bool):
                    return value
                if isinstance(value, str):
                    return value.lower() in ('true', '1', 'yes', 'on')
                return bool(value)
            
            # Helper function for safe string conversion
            def safe_str(value, max_length=None):
                if value is None:
                    return None
                str_value = str(value)
                if max_length and len(str_value) > max_length:
                    return str_value[:max_length]
                return str_value
            
            # Helper function for safe array conversion
            def safe_array(value):
                if value is None:
                    return []
                if isinstance(value, list):
                    return value
                return [value]
                
            # Extract COMPLETE actor data
            actor = event.get('actor', {})
            actor_data = {
                # Basic actor info
                'id': safe_int(actor.get('id')),
                'login': safe_str(actor.get('login'), 255),
                'display_login': safe_str(actor.get('display_login'), 255),
                'gravatar_id': safe_str(actor.get('gravatar_id'), 255),
                'url': safe_str(actor.get('url')),
                'avatar_url': safe_str(actor.get('avatar_url')),
                
                # Extended actor info
                'node_id': safe_str(actor.get('node_id'), 255),
                'html_url': safe_str(actor.get('html_url')),
                'followers_url': safe_str(actor.get('followers_url')),
                'following_url': safe_str(actor.get('following_url')),
                'gists_url': safe_str(actor.get('gists_url')),
                'starred_url': safe_str(actor.get('starred_url')),
                'subscriptions_url': safe_str(actor.get('subscriptions_url')),
                'organizations_url': safe_str(actor.get('organizations_url')),
                'repos_url': safe_str(actor.get('repos_url')),
                'events_url': safe_str(actor.get('events_url')),
                'received_events_url': safe_str(actor.get('received_events_url')),
                'type': safe_str(actor.get('type'), 50),
                'user_view_type': safe_str(actor.get('user_view_type'), 50),
                'site_admin': safe_bool(actor.get('site_admin'))
            }
            
            # Extract COMPLETE repo data
            repo = event.get('repo', {})
            repo_owner = repo.get('owner', {})
            repo_license = repo.get('license', {})
            
            repo_data = {
                # Basic repo info
                'id': safe_int(repo.get('id')),
                'name': safe_str(repo.get('name'), 255),
                'url': safe_str(repo.get('url')),
                'full_name': safe_str(repo.get('full_name'), 255),
                
                # Repo owner info
                'owner_login': safe_str(repo_owner.get('login'), 255),
                'owner_id': safe_int(repo_owner.get('id')),
                'owner_node_id': safe_str(repo_owner.get('node_id'), 255),
                'owner_avatar_url': safe_str(repo_owner.get('avatar_url')),
                'owner_gravatar_id': safe_str(repo_owner.get('gravatar_id'), 255),
                'owner_url': safe_str(repo_owner.get('url')),
                'owner_html_url': safe_str(repo_owner.get('html_url')),
                'owner_type': safe_str(repo_owner.get('type'), 50),
                'owner_site_admin': safe_bool(repo_owner.get('site_admin')),
                
                # Extended repo metadata
                'node_id': safe_str(repo.get('node_id'), 255),
                'html_url': safe_str(repo.get('html_url')),
                'description': safe_str(repo.get('description')),
                'fork': safe_bool(repo.get('fork')),
                'keys_url': safe_str(repo.get('keys_url')),
                'collaborators_url': safe_str(repo.get('collaborators_url')),
                'teams_url': safe_str(repo.get('teams_url')),
                'hooks_url': safe_str(repo.get('hooks_url')),
                'issue_events_url': safe_str(repo.get('issue_events_url')),
                'events_url': safe_str(repo.get('events_url')),
                'assignees_url': safe_str(repo.get('assignees_url')),
                'branches_url': safe_str(repo.get('branches_url')),
                'tags_url': safe_str(repo.get('tags_url')),
                'blobs_url': safe_str(repo.get('blobs_url')),
                'git_tags_url': safe_str(repo.get('git_tags_url')),
                'git_refs_url': safe_str(repo.get('git_refs_url')),
                'trees_url': safe_str(repo.get('trees_url')),
                'statuses_url': safe_str(repo.get('statuses_url')),
                'languages_url': safe_str(repo.get('languages_url')),
                'stargazers_url': safe_str(repo.get('stargazers_url')),
                'contributors_url': safe_str(repo.get('contributors_url')),
                'subscribers_url': safe_str(repo.get('subscribers_url')),
                'subscription_url': safe_str(repo.get('subscription_url')),
                'commits_url': safe_str(repo.get('commits_url')),
                'git_commits_url': safe_str(repo.get('git_commits_url')),
                'comments_url': safe_str(repo.get('comments_url')),
                'issue_comment_url': safe_str(repo.get('issue_comment_url')),
                'contents_url': safe_str(repo.get('contents_url')),
                'compare_url': safe_str(repo.get('compare_url')),
                'merges_url': safe_str(repo.get('merges_url')),
                'archive_url': safe_str(repo.get('archive_url')),
                'downloads_url': safe_str(repo.get('downloads_url')),
                'issues_url': safe_str(repo.get('issues_url')),
                'pulls_url': safe_str(repo.get('pulls_url')),
                'milestones_url': safe_str(repo.get('milestones_url')),
                'notifications_url': safe_str(repo.get('notifications_url')),
                'labels_url': safe_str(repo.get('labels_url')),
                'releases_url': safe_str(repo.get('releases_url')),
                'deployments_url': safe_str(repo.get('deployments_url')),
                'git_url': safe_str(repo.get('git_url')),
                'ssh_url': safe_str(repo.get('ssh_url')),
                'clone_url': safe_str(repo.get('clone_url')),
                'svn_url': safe_str(repo.get('svn_url')),
                'homepage': safe_str(repo.get('homepage')),
                'size': safe_int(repo.get('size')),
                'stargazers_count': safe_int(repo.get('stargazers_count')),
                'watchers_count': safe_int(repo.get('watchers_count')),
                'language': safe_str(repo.get('language'), 100),
                'has_issues': safe_bool(repo.get('has_issues')),
                'has_projects': safe_bool(repo.get('has_projects')),
                'has_downloads': safe_bool(repo.get('has_downloads')),
                'has_wiki': safe_bool(repo.get('has_wiki')),
                'has_pages': safe_bool(repo.get('has_pages')),
                'has_discussions': safe_bool(repo.get('has_discussions')),
                'forks_count': safe_int(repo.get('forks_count')),
                'mirror_url': safe_str(repo.get('mirror_url')),
                'archived': safe_bool(repo.get('archived')),
                'disabled': safe_bool(repo.get('disabled')),
                'open_issues_count': safe_int(repo.get('open_issues_count')),
                'license_key': safe_str(repo_license.get('key'), 50),
                'license_name': safe_str(repo_license.get('name'), 255),
                'license_spdx_id': safe_str(repo_license.get('spdx_id'), 50),
                'license_url': safe_str(repo_license.get('url')),
                'license_node_id': safe_str(repo_license.get('node_id'), 255),
                'allow_forking': safe_bool(repo.get('allow_forking')),
                'is_template': safe_bool(repo.get('is_template')),
                'web_commit_signoff_required': safe_bool(repo.get('web_commit_signoff_required')),
                'topics': safe_array(repo.get('topics')),
                'visibility': safe_str(repo.get('visibility'), 50),
                'forks': safe_int(repo.get('forks')),
                'open_issues': safe_int(repo.get('open_issues')),
                'watchers': safe_int(repo.get('watchers')),
                'default_branch': safe_str(repo.get('default_branch'), 100),
                'created_at': parse_date(repo['created_at']) if repo.get('created_at') else None,
                'updated_at': parse_date(repo['updated_at']) if repo.get('updated_at') else None,
                'pushed_at': parse_date(repo['pushed_at']) if repo.get('pushed_at') else None
            }
            
            # Extract COMPLETE org data (optional)
            org = event.get('org', {})
            org_data = {
                'id': safe_int(org.get('id')) if org else None,
                'login': safe_str(org.get('login'), 255) if org else None,
                'node_id': safe_str(org.get('node_id'), 255) if org else None,
                'gravatar_id': safe_str(org.get('gravatar_id'), 255) if org else None,
                'url': safe_str(org.get('url')) if org else None,
                'avatar_url': safe_str(org.get('avatar_url')) if org else None,
                'html_url': safe_str(org.get('html_url')) if org else None,
                'followers_url': safe_str(org.get('followers_url')) if org else None,
                'following_url': safe_str(org.get('following_url')) if org else None,
                'gists_url': safe_str(org.get('gists_url')) if org else None,
                'starred_url': safe_str(org.get('starred_url')) if org else None,
                'subscriptions_url': safe_str(org.get('subscriptions_url')) if org else None,
                'organizations_url': safe_str(org.get('organizations_url')) if org else None,
                'repos_url': safe_str(org.get('repos_url')) if org else None,
                'events_url': safe_str(org.get('events_url')) if org else None,
                'received_events_url': safe_str(org.get('received_events_url')) if org else None,
                'type': safe_str(org.get('type'), 50) if org else None,
                'user_view_type': safe_str(org.get('user_view_type'), 50) if org else None,
                'site_admin': safe_bool(org.get('site_admin')) if org else None
            }
            
            return {
                'id': event_id,
                'type': event_type,
                'created_at': created_at,
                'public': event.get('public', True),
                'actor': actor_data,
                'repo': repo_data,
                'org': org_data,
                'payload': event.get('payload', {}),
                'raw_event': event,  # Store complete raw event for preservation
                'api_source': 'github_archive'
            }
            
        except Exception as e:
            logging.error(f"Error validating event: {e}")
            return None

    async def insert_events_batch(self, events: List[Dict], filename: str) -> int:
        """Insert pre-validated events into database with enhanced error handling"""
        if not events:
            return 0
            
        # Pre-validate and convert all events
        validated_events = []
        invalid_count = 0
        
        for event in events:
            validated_event = self.validate_and_convert_event(event)
            if validated_event:
                validated_events.append(validated_event)
            else:
                invalid_count += 1
        
        if invalid_count > 0:
            logging.warning(f"Filtered out {invalid_count} invalid events from {filename}")
            
        if not validated_events:
            return 0
            
        # Enhanced comprehensive INSERT statement for ALL GitHub API data
        insert_sql = """
        INSERT INTO github_events (
            -- Core event fields
            event_id, event_type, event_created_at, event_public,
            
            -- Complete Actor fields
            actor_id, actor_login, actor_display_login, actor_gravatar_id, actor_url, actor_avatar_url,
            actor_node_id, actor_html_url, actor_followers_url, actor_following_url, actor_gists_url,
            actor_starred_url, actor_subscriptions_url, actor_organizations_url, actor_repos_url,
            actor_events_url, actor_received_events_url, actor_type, actor_user_view_type, actor_site_admin,
            
            -- Complete Repository fields
            repo_id, repo_name, repo_url, repo_full_name, repo_owner_login, repo_owner_id,
            repo_owner_node_id, repo_owner_avatar_url, repo_owner_gravatar_id, repo_owner_url,
            repo_owner_html_url, repo_owner_type, repo_owner_site_admin, repo_node_id, repo_html_url,
            repo_description, repo_fork, repo_keys_url, repo_collaborators_url, repo_teams_url,
            repo_hooks_url, repo_issue_events_url, repo_events_url, repo_assignees_url, repo_branches_url,
            repo_tags_url, repo_blobs_url, repo_git_tags_url, repo_git_refs_url, repo_trees_url,
            repo_statuses_url, repo_languages_url, repo_stargazers_url, repo_contributors_url,
            repo_subscribers_url, repo_subscription_url, repo_commits_url, repo_git_commits_url,
            repo_comments_url, repo_issue_comment_url, repo_contents_url, repo_compare_url,
            repo_merges_url, repo_archive_url, repo_downloads_url, repo_issues_url, repo_pulls_url,
            repo_milestones_url, repo_notifications_url, repo_labels_url, repo_releases_url,
            repo_deployments_url, repo_git_url, repo_ssh_url, repo_clone_url, repo_svn_url,
            repo_homepage, repo_size, repo_stargazers_count, repo_watchers_count, repo_language,
            repo_has_issues, repo_has_projects, repo_has_downloads, repo_has_wiki, repo_has_pages,
            repo_has_discussions, repo_forks_count, repo_mirror_url, repo_archived, repo_disabled,
            repo_open_issues_count, repo_license_key, repo_license_name, repo_license_spdx_id,
            repo_license_url, repo_license_node_id, repo_allow_forking, repo_is_template,
            repo_web_commit_signoff_required, repo_topics, repo_visibility, repo_forks,
            repo_open_issues, repo_watchers, repo_default_branch, repo_created_at, repo_updated_at, repo_pushed_at,
            
            -- Complete Organization fields
            org_id, org_login, org_node_id, org_gravatar_id, org_url, org_avatar_url, org_html_url,
            org_followers_url, org_following_url, org_gists_url, org_starred_url, org_subscriptions_url,
            org_organizations_url, org_repos_url, org_events_url, org_received_events_url,
            org_type, org_user_view_type, org_site_admin,
            
            -- Data storage fields
            payload, raw_event, file_source, api_source, data_version
        ) VALUES (
            -- Core event values (4 params)
            $1, $2, $3, $4,
            
            -- Actor values (19 params)
            $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23,
            
            -- Repository values (70 params)
            $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41,
            $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56, $57, $58, $59,
            $60, $61, $62, $63, $64, $65, $66, $67, $68, $69, $70, $71, $72, $73, $74, $75, $76, $77,
            $78, $79, $80, $81, $82, $83, $84, $85, $86, $87, $88, $89, $90, $91, $92, $93,
            
            -- Organization values (19 params)
            $94, $95, $96, $97, $98, $99, $100, $101, $102, $103, $104, $105, $106, $107, $108, $109, $110, $111, $112,
            
            -- Data storage values (5 params)
            $113, $114, $115, $116, $117
        )
        ON CONFLICT (event_id) DO UPDATE SET
            -- Update with newer data if we get a duplicate (which shouldn't happen, but just in case)
            payload = EXCLUDED.payload,
            raw_event = EXCLUDED.raw_event,
            processed_at = NOW()
        """
        
        async with self.pool.acquire() as conn:
            async with conn.transaction():
                rows_affected = 0
                
                for event in validated_events:
                    try:
                        # Extract data for comprehensive insertion
                        actor = event['actor']
                        repo = event['repo']
                        org = event['org']
                        
                        result = await conn.execute(
                            insert_sql,
                            # Core event fields (4 params)
                            event['id'], event['type'], event['created_at'], event['public'],
                            
                            # Complete Actor fields (19 params)
                            actor['id'], actor['login'], actor['display_login'], actor['gravatar_id'],
                            actor['url'], actor['avatar_url'], actor['node_id'], actor['html_url'],
                            actor['followers_url'], actor['following_url'], actor['gists_url'],
                            actor['starred_url'], actor['subscriptions_url'], actor['organizations_url'],
                            actor['repos_url'], actor['events_url'], actor['received_events_url'],
                            actor['type'], actor['user_view_type'], actor['site_admin'],
                            
                            # Complete Repository fields (70 params)
                            repo['id'], repo['name'], repo['url'], repo['full_name'], repo['owner_login'],
                            repo['owner_id'], repo['owner_node_id'], repo['owner_avatar_url'],
                            repo['owner_gravatar_id'], repo['owner_url'], repo['owner_html_url'],
                            repo['owner_type'], repo['owner_site_admin'], repo['node_id'], repo['html_url'],
                            repo['description'], repo['fork'], repo['keys_url'], repo['collaborators_url'],
                            repo['teams_url'], repo['hooks_url'], repo['issue_events_url'], repo['events_url'],
                            repo['assignees_url'], repo['branches_url'], repo['tags_url'], repo['blobs_url'],
                            repo['git_tags_url'], repo['git_refs_url'], repo['trees_url'], repo['statuses_url'],
                            repo['languages_url'], repo['stargazers_url'], repo['contributors_url'],
                            repo['subscribers_url'], repo['subscription_url'], repo['commits_url'],
                            repo['git_commits_url'], repo['comments_url'], repo['issue_comment_url'],
                            repo['contents_url'], repo['compare_url'], repo['merges_url'], repo['archive_url'],
                            repo['downloads_url'], repo['issues_url'], repo['pulls_url'], repo['milestones_url'],
                            repo['notifications_url'], repo['labels_url'], repo['releases_url'],
                            repo['deployments_url'], repo['git_url'], repo['ssh_url'], repo['clone_url'],
                            repo['svn_url'], repo['homepage'], repo['size'], repo['stargazers_count'],
                            repo['watchers_count'], repo['language'], repo['has_issues'], repo['has_projects'],
                            repo['has_downloads'], repo['has_wiki'], repo['has_pages'], repo['has_discussions'],
                            repo['forks_count'], repo['mirror_url'], repo['archived'], repo['disabled'],
                            repo['open_issues_count'], repo['license_key'], repo['license_name'],
                            repo['license_spdx_id'], repo['license_url'], repo['license_node_id'],
                            repo['allow_forking'], repo['is_template'], repo['web_commit_signoff_required'],
                            repo['topics'], repo['visibility'], repo['forks'], repo['open_issues'],
                            repo['watchers'], repo['default_branch'], repo['created_at'], repo['updated_at'], repo['pushed_at'],
                            
                            # Complete Organization fields (19 params)
                            org['id'], org['login'], org['node_id'], org['gravatar_id'], org['url'],
                            org['avatar_url'], org['html_url'], org['followers_url'], org['following_url'],
                            org['gists_url'], org['starred_url'], org['subscriptions_url'],
                            org['organizations_url'], org['repos_url'], org['events_url'],
                            org['received_events_url'], org['type'], org['user_view_type'], org['site_admin'],
                            
                            # Data storage fields (5 params)
                            json.dumps(event['payload']), json.dumps(event['raw_event']), 
                            filename, event['api_source'], '3.0'
                        )
                        
                        if "INSERT" in result or "UPDATE" in result:
                            rows_affected += 1
                            
                    except Exception as e:
                        logging.error(f"Database error inserting comprehensive event {event['id']}: {e}")
                        continue
                        
                return rows_affected

    async def get_data_quality_metrics(self) -> Dict:
        """Get comprehensive data quality metrics for monitoring"""
        async with self.pool.acquire() as conn:
            metrics = {}
            
            # Basic event statistics
            event_stats = await conn.fetchrow("""
                SELECT 
                    COUNT(*) as total_events,
                    COUNT(DISTINCT type) as event_types,
                    COUNT(DISTINCT actor_id) as unique_actors,
                    COUNT(DISTINCT repo_id) as unique_repos,
                    COUNT(CASE WHEN actor_id IS NULL THEN 1 END) as null_actor_ids,
                    COUNT(CASE WHEN repo_id IS NULL THEN 1 END) as null_repo_ids,
                    MIN(created_at) as earliest_event,
                    MAX(created_at) as latest_event
                FROM github_events
            """)
            
            # Event type distribution
            event_types = await conn.fetch("""
                SELECT type, COUNT(*) as count 
                FROM github_events 
                GROUP BY type 
                ORDER BY count DESC 
                LIMIT 20
            """)
            
            # Data integrity checks
            integrity_issues = await conn.fetchrow("""
                SELECT 
                    COUNT(CASE WHEN id IS NULL THEN 1 END) as null_ids,
                    COUNT(CASE WHEN type IS NULL OR type = '' THEN 1 END) as invalid_types,
                    COUNT(CASE WHEN created_at IS NULL THEN 1 END) as null_timestamps,
                    COUNT(CASE WHEN payload IS NULL THEN 1 END) as null_payloads
                FROM github_events
            """)
            
            # Processing statistics
            processing_stats = await conn.fetchrow("""
                SELECT 
                    COUNT(*) as total_files,
                    SUM(event_count) as total_processed_events,
                    AVG(event_count) as avg_events_per_file,
                    MIN(processed_at) as first_processed,
                    MAX(processed_at) as last_processed,
                    SUM(file_size) / (1024*1024*1024) as total_gb_processed
                FROM processed_files
            """)
            
            # Recent processing activity (last 24 hours)
            recent_activity = await conn.fetchrow("""
                SELECT 
                    COUNT(*) as files_processed_24h,
                    SUM(event_count) as events_processed_24h
                FROM processed_files 
                WHERE processed_at > NOW() - INTERVAL '24 hours'
            """)
            
            # Repository statistics
            repo_stats = await conn.fetchrow("""
                SELECT 
                    COUNT(*) as total_repositories,
                    COUNT(CASE WHEN stargazers_count > 0 THEN 1 END) as repos_with_stars,
                    AVG(stargazers_count) as avg_stars,
                    MAX(stargazers_count) as max_stars,
                    COUNT(DISTINCT language) as unique_languages
                FROM repositories
            """)
            
            metrics = {
                'events': {
                    'total': event_stats['total_events'],
                    'unique_actors': event_stats['unique_actors'],
                    'unique_repos': event_stats['unique_repos'],
                    'event_types_count': event_stats['event_types'],
                    'null_actor_ids': event_stats['null_actor_ids'],
                    'null_repo_ids': event_stats['null_repo_ids'],
                    'date_range': {
                        'earliest': event_stats['earliest_event'].isoformat() if event_stats['earliest_event'] else None,
                        'latest': event_stats['latest_event'].isoformat() if event_stats['latest_event'] else None
                    }
                },
                'event_types': [{'type': row['type'], 'count': row['count']} for row in event_types],
                'data_integrity': {
                    'null_ids': integrity_issues['null_ids'],
                    'invalid_types': integrity_issues['invalid_types'],
                    'null_timestamps': integrity_issues['null_timestamps'],
                    'null_payloads': integrity_issues['null_payloads']
                },
                'processing': {
                    'total_files': processing_stats['total_files'] or 0,
                    'total_processed_events': processing_stats['total_processed_events'] or 0,
                    'avg_events_per_file': float(processing_stats['avg_events_per_file'] or 0),
                    'total_gb_processed': float(processing_stats['total_gb_processed'] or 0),
                    'first_processed': processing_stats['first_processed'].isoformat() if processing_stats['first_processed'] else None,
                    'last_processed': processing_stats['last_processed'].isoformat() if processing_stats['last_processed'] else None
                },
                'recent_activity': {
                    'files_processed_24h': recent_activity['files_processed_24h'] or 0,
                    'events_processed_24h': recent_activity['events_processed_24h'] or 0
                },
                'repositories': {
                    'total': repo_stats['total_repositories'] or 0,
                    'with_stars': repo_stats['repos_with_stars'] or 0,
                    'avg_stars': float(repo_stats['avg_stars'] or 0),
                    'max_stars': repo_stats['max_stars'] or 0,
                    'unique_languages': repo_stats['unique_languages'] or 0
                },
                'quality_score': self._calculate_quality_score(event_stats, integrity_issues)
            }
            
            return metrics
    
    def _calculate_quality_score(self, event_stats: Dict, integrity_issues: Dict) -> float:
        """Calculate a data quality score (0-100)"""
        if not event_stats or event_stats['total_events'] == 0:
            return 0.0
            
        total_events = event_stats['total_events']
        issues = (
            integrity_issues['null_ids'] + 
            integrity_issues['invalid_types'] + 
            integrity_issues['null_timestamps']
        )
        
        # Calculate percentage of clean data
        clean_percentage = ((total_events - issues) / total_events) * 100
        return min(100.0, max(0.0, clean_percentage))


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
        
        # Helper function for safe integer conversion
        def safe_int(value):
            if value is None or value == '':
                return None
            try:
                return int(value)
            except (ValueError, TypeError):
                return None
        
        change = {
            'repo_id': safe_int(repo.get('id')),
            'event_id': safe_int(event.get('id')),
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
