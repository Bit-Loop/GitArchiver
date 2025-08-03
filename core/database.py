#!/usr/bin/env python3
"""
Core Database Management Module for GitHub Archive Scraper
Consolidated, professional database operations with connection pooling and transaction management.
"""

import asyncio
import asyncpg
import logging
import json
from datetime import datetime, timezone
from typing import Dict, List, Optional, Any, Union
from dataclasses import dataclass
from contextlib import asynccontextmanager

from config import Config


@dataclass
class DatabaseHealth:
    """Database health status"""
    is_connected: bool
    connection_count: int
    active_queries: int
    cache_hit_ratio: float
    error_message: Optional[str] = None


@dataclass
class QualityMetrics:
    """Data quality metrics"""
    total_events: int
    unique_actors: int
    unique_repos: int
    event_types: int
    quality_score: float
    integrity_issues: Dict[str, int]
    processing_stats: Dict[str, Any]
    recent_activity: Dict[str, int]


class DatabaseError(Exception):
    """Custom database exception"""
    pass


class DatabaseManager:
    """
    Professional PostgreSQL database manager with connection pooling,
    transaction management, and comprehensive error handling.
    """
    
    def __init__(self, config: Config):
        self.config = config
        self.pool: Optional[asyncpg.Pool] = None
        self.logger = logging.getLogger(__name__)
        self._connection_attempts = 0
        self._max_connection_attempts = 3
        
    async def connect(self) -> None:
        """
        Establish database connection pool with retry logic and health checks.
        
        Raises:
            DatabaseError: If connection fails after all retry attempts
        """
        dsn = self._build_connection_string()
        
        for attempt in range(self._max_connection_attempts):
            try:
                self.pool = await asyncpg.create_pool(
                    dsn,
                    min_size=self.config.DB_MIN_CONNECTIONS,
                    max_size=self.config.DB_MAX_CONNECTIONS,
                    command_timeout=60,
                    server_settings={
                        'jit': 'off',  # Disable JIT for Oracle Cloud compatibility
                        'statement_timeout': '300000',  # 5 minutes
                    }
                )
                
                # Verify connection with health check
                await self._verify_connection()
                await self._initialize_schema()
                
                self.logger.info(f"Database connected successfully (attempt {attempt + 1})")
                return
                
            except Exception as e:
                self._connection_attempts += 1
                self.logger.error(f"Database connection attempt {attempt + 1} failed: {e}")
                
                if attempt < self._max_connection_attempts - 1:
                    await asyncio.sleep(self.config.RETRY_DELAY * (attempt + 1))
                else:
                    raise DatabaseError(f"Failed to connect after {self._max_connection_attempts} attempts: {e}")
    
    async def disconnect(self) -> None:
        """Gracefully close database connection pool."""
        if self.pool:
            await self.pool.close()
            self.pool = None
            self.logger.info("Database disconnected gracefully")
    
    @asynccontextmanager
    async def get_connection(self):
        """
        Context manager for database connections with automatic cleanup.
        
        Usage:
            async with db.get_connection() as conn:
                result = await conn.fetchrow("SELECT * FROM table")
        """
        if not self.pool:
            raise DatabaseError("Database not connected")
            
        async with self.pool.acquire() as connection:
            yield connection
    
    @asynccontextmanager
    async def get_transaction(self):
        """
        Context manager for database transactions with automatic rollback on error.
        
        Usage:
            async with db.get_transaction() as conn:
                await conn.execute("INSERT INTO table VALUES ($1)", value)
        """
        async with self.get_connection() as conn:
            async with conn.transaction():
                yield conn
    
    async def health_check(self) -> DatabaseHealth:
        """
        Comprehensive database health check.
        
        Returns:
            DatabaseHealth: Current database health status
        """
        try:
            if not self.pool:
                return DatabaseHealth(
                    is_connected=False,
                    connection_count=0,
                    active_queries=0,
                    cache_hit_ratio=0.0,
                    error_message="No connection pool"
                )
            
            async with self.get_connection() as conn:
                # Basic connectivity test
                await conn.fetchval("SELECT 1")
                
                # Get connection statistics
                stats = await conn.fetchrow("""
                    SELECT 
                        count(*) as active_connections,
                        count(CASE WHEN state = 'active' THEN 1 END) as active_queries
                    FROM pg_stat_activity 
                    WHERE datname = current_database()
                """)
                
                # Get cache hit ratio
                cache_stats = await conn.fetchrow("""
                    SELECT 
                        round(
                            100 * sum(blks_hit) / nullif(sum(blks_hit + blks_read), 0), 2
                        ) as cache_hit_ratio
                    FROM pg_stat_database 
                    WHERE datname = current_database()
                """)
                
                return DatabaseHealth(
                    is_connected=True,
                    connection_count=stats['active_connections'],
                    active_queries=stats['active_queries'],
                    cache_hit_ratio=float(cache_stats['cache_hit_ratio'] or 0.0)
                )
                
        except Exception as e:
            self.logger.error(f"Database health check failed: {e}")
            return DatabaseHealth(
                is_connected=False,
                connection_count=0,
                active_queries=0,
                cache_hit_ratio=0.0,
                error_message=str(e)
            )
    
    async def execute_query(self, query: str, *args, fetch_method: str = 'fetch') -> Any:
        """
        Execute a query with proper error handling and logging.
        
        Args:
            query: SQL query to execute
            *args: Query parameters
            fetch_method: 'fetch', 'fetchrow', 'fetchval', or 'execute'
            
        Returns:
            Query result based on fetch_method
        """
        try:
            async with self.get_connection() as conn:
                if fetch_method == 'fetch':
                    return await conn.fetch(query, *args)
                elif fetch_method == 'fetchrow':
                    return await conn.fetchrow(query, *args)
                elif fetch_method == 'fetchval':
                    return await conn.fetchval(query, *args)
                elif fetch_method == 'execute':
                    return await conn.execute(query, *args)
                else:
                    raise ValueError(f"Invalid fetch_method: {fetch_method}")
                    
        except Exception as e:
            self.logger.error(f"Query execution failed: {e}\nQuery: {query[:100]}...")
            raise DatabaseError(f"Query execution failed: {e}")
    
    async def insert_events_batch(self, events: List[Dict[str, Any]], filename: str) -> int:
        """
        Insert a batch of validated events with comprehensive error handling.
        
        Args:
            events: List of validated event dictionaries
            filename: Source filename for tracking
            
        Returns:
            Number of successfully inserted events
        """
        if not events:
            return 0
        
        # Pre-validate events
        validated_events = []
        for event in events:
            validated_event = self._validate_and_convert_event(event)
            if validated_event:
                validated_events.append(validated_event)
        
        if not validated_events:
            self.logger.warning(f"No valid events found in {filename}")
            return 0
        
        insert_sql = self._get_comprehensive_insert_sql()
        rows_inserted = 0
        
        try:
            async with self.get_transaction() as conn:
                for event in validated_events:
                    try:
                        params = self._extract_event_parameters(event, filename)
                        result = await conn.execute(insert_sql, *params)
                        
                        if "INSERT" in result or "UPDATE" in result:
                            rows_inserted += 1
                            
                    except Exception as e:
                        self.logger.error(f"Failed to insert event {event.get('id', 'unknown')}: {e}")
                        continue
                
                self.logger.info(f"Inserted {rows_inserted} events from {filename}")
                return rows_inserted
                
        except Exception as e:
            self.logger.error(f"Batch insert failed for {filename}: {e}")
            raise DatabaseError(f"Batch insert failed: {e}")
    
    async def get_data_quality_metrics(self) -> QualityMetrics:
        """
        Generate comprehensive data quality metrics.
        
        Returns:
            QualityMetrics: Complete data quality assessment
        """
        try:
            async with self.get_connection() as conn:
                # Event statistics
                event_stats = await conn.fetchrow("""
                    SELECT 
                        COUNT(*) as total_events,
                        COUNT(DISTINCT actor_id) as unique_actors,
                        COUNT(DISTINCT repo_id) as unique_repos,
                        COUNT(DISTINCT event_type) as event_types,
                        MIN(event_created_at) as earliest_event,
                        MAX(event_created_at) as latest_event
                    FROM github_events
                """)
                
                # Data integrity issues
                integrity_issues = await conn.fetchrow("""
                    SELECT 
                        COUNT(CASE WHEN event_id IS NULL THEN 1 END) as null_ids,
                        COUNT(CASE WHEN event_type IS NULL OR event_type = '' THEN 1 END) as invalid_types,
                        COUNT(CASE WHEN event_created_at IS NULL THEN 1 END) as null_timestamps,
                        COUNT(CASE WHEN payload IS NULL THEN 1 END) as null_payloads
                    FROM github_events
                """)
                
                # Processing statistics
                processing_stats = await conn.fetchrow("""
                    SELECT 
                        COUNT(*) as total_files,
                        SUM(event_count) as total_processed_events,
                        AVG(event_count) as avg_events_per_file,
                        SUM(file_size) / (1024*1024*1024.0) as total_gb_processed
                    FROM processed_files
                """)
                
                # Recent activity (24 hours)
                recent_activity = await conn.fetchrow("""
                    SELECT 
                        COUNT(*) as files_processed_24h,
                        COALESCE(SUM(event_count), 0) as events_processed_24h
                    FROM processed_files 
                    WHERE processed_at > NOW() - INTERVAL '24 hours'
                """)
                
                # Calculate quality score
                quality_score = self._calculate_quality_score(event_stats, integrity_issues)
                
                return QualityMetrics(
                    total_events=event_stats['total_events'] or 0,
                    unique_actors=event_stats['unique_actors'] or 0,
                    unique_repos=event_stats['unique_repos'] or 0,
                    event_types=event_stats['event_types'] or 0,
                    quality_score=quality_score,
                    integrity_issues={
                        'null_ids': integrity_issues['null_ids'] or 0,
                        'invalid_types': integrity_issues['invalid_types'] or 0,
                        'null_timestamps': integrity_issues['null_timestamps'] or 0,
                        'null_payloads': integrity_issues['null_payloads'] or 0
                    },
                    processing_stats={
                        'total_files': processing_stats['total_files'] or 0,
                        'total_processed_events': processing_stats['total_processed_events'] or 0,
                        'avg_events_per_file': float(processing_stats['avg_events_per_file'] or 0),
                        'total_gb_processed': float(processing_stats['total_gb_processed'] or 0)
                    },
                    recent_activity={
                        'files_processed_24h': recent_activity['files_processed_24h'] or 0,
                        'events_processed_24h': recent_activity['events_processed_24h'] or 0
                    }
                )
                
        except Exception as e:
            self.logger.error(f"Failed to get quality metrics: {e}")
            raise DatabaseError(f"Quality metrics generation failed: {e}")
    
    async def is_file_processed(self, filename: str, etag: str = None, size: int = None) -> bool:
        """Check if file has already been processed (rsync-like functionality)."""
        try:
            query = "SELECT etag, file_size FROM processed_files WHERE filename = $1"
            result = await self.execute_query(query, filename, fetch_method='fetchrow')
            
            if not result:
                return False
            
            # Check ETag and size if provided
            if etag and result['etag'] != etag:
                return False
            if size and result['file_size'] != size:
                return False
                
            return True
            
        except Exception as e:
            self.logger.error(f"Error checking file processed status: {e}")
            return False
    
    async def mark_file_processed(self, filename: str, etag: str, size: int, event_count: int) -> None:
        """Mark file as processed with metadata."""
        query = """
            INSERT INTO processed_files (filename, etag, file_size, event_count)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (filename) 
            DO UPDATE SET 
                etag = $2, 
                file_size = $3, 
                event_count = $4, 
                processed_at = NOW()
        """
        await self.execute_query(query, filename, etag, size, event_count, fetch_method='execute')
    
    # Private methods
    def _build_connection_string(self) -> str:
        """Build PostgreSQL connection string."""
        return (f"postgresql://{self.config.DB_USER}:{self.config.DB_PASSWORD}"
                f"@{self.config.DB_HOST}:{self.config.DB_PORT}/{self.config.DB_NAME}")
    
    async def _verify_connection(self) -> None:
        """Verify database connection is working."""
        async with self.pool.acquire() as conn:
            result = await conn.fetchval("SELECT version()")
            self.logger.info(f"Connected to PostgreSQL: {result}")
    
    async def _initialize_schema(self) -> None:
        """Initialize database schema if needed."""
        schema_sql = self._get_schema_sql()
        async with self.pool.acquire() as conn:
            await conn.execute(schema_sql)
        self.logger.info("Database schema initialized")
    
    def _validate_and_convert_event(self, event: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Validate and convert event data with comprehensive type safety."""
        try:
            # Core validation
            event_id = self._safe_int(event.get('id'))
            if not event_id:
                return None
                
            event_type = self._safe_str(event.get('type'), 50)
            if not event_type:
                return None
            
            # Date parsing
            created_at = self._parse_date(event.get('created_at'))
            if not created_at:
                return None
            
            # Extract and validate actor data
            actor = event.get('actor', {})
            actor_data = {
                'id': self._safe_int(actor.get('id')),
                'login': self._safe_str(actor.get('login'), 255),
                'display_login': self._safe_str(actor.get('display_login'), 255),
                'gravatar_id': self._safe_str(actor.get('gravatar_id'), 255),
                'url': self._safe_str(actor.get('url')),
                'avatar_url': self._safe_str(actor.get('avatar_url')),
                'node_id': self._safe_str(actor.get('node_id'), 255),
                'html_url': self._safe_str(actor.get('html_url')),
                'followers_url': self._safe_str(actor.get('followers_url')),
                'following_url': self._safe_str(actor.get('following_url')),
                'gists_url': self._safe_str(actor.get('gists_url')),
                'starred_url': self._safe_str(actor.get('starred_url')),
                'subscriptions_url': self._safe_str(actor.get('subscriptions_url')),
                'organizations_url': self._safe_str(actor.get('organizations_url')),
                'repos_url': self._safe_str(actor.get('repos_url')),
                'events_url': self._safe_str(actor.get('events_url')),
                'received_events_url': self._safe_str(actor.get('received_events_url')),
                'type': self._safe_str(actor.get('type'), 50),
                'user_view_type': self._safe_str(actor.get('user_view_type'), 50),
                'site_admin': self._safe_bool(actor.get('site_admin'))
            }
            
            # Extract and validate repo data
            repo = event.get('repo', {})
            repo_owner = repo.get('owner', {})
            repo_license = repo.get('license', {})
            
            repo_data = {
                'id': self._safe_int(repo.get('id')),
                'name': self._safe_str(repo.get('name'), 255),
                'url': self._safe_str(repo.get('url')),
                'full_name': self._safe_str(repo.get('full_name'), 255),
                'owner_login': self._safe_str(repo_owner.get('login'), 255),
                'owner_id': self._safe_int(repo_owner.get('id')),
                'owner_node_id': self._safe_str(repo_owner.get('node_id'), 255),
                'owner_avatar_url': self._safe_str(repo_owner.get('avatar_url')),
                'owner_gravatar_id': self._safe_str(repo_owner.get('gravatar_id'), 255),
                'owner_url': self._safe_str(repo_owner.get('url')),
                'owner_html_url': self._safe_str(repo_owner.get('html_url')),
                'owner_type': self._safe_str(repo_owner.get('type'), 50),
                'owner_site_admin': self._safe_bool(repo_owner.get('site_admin')),
                'node_id': self._safe_str(repo.get('node_id'), 255),
                'html_url': self._safe_str(repo.get('html_url')),
                'description': self._safe_str(repo.get('description')),
                'fork': self._safe_bool(repo.get('fork')),
                'language': self._safe_str(repo.get('language'), 100),
                'stargazers_count': self._safe_int(repo.get('stargazers_count')),
                'watchers_count': self._safe_int(repo.get('watchers_count')),
                'forks_count': self._safe_int(repo.get('forks_count')),
                'open_issues_count': self._safe_int(repo.get('open_issues_count')),
                'size': self._safe_int(repo.get('size')),
                'default_branch': self._safe_str(repo.get('default_branch'), 100),
                'topics': self._safe_array(repo.get('topics')),
                'license_key': self._safe_str(repo_license.get('key'), 50) if repo_license else None,
                'license_name': self._safe_str(repo_license.get('name'), 255) if repo_license else None,
                'created_at': self._parse_date(repo.get('created_at')),
                'updated_at': self._parse_date(repo.get('updated_at')),
                'pushed_at': self._parse_date(repo.get('pushed_at'))
            }
            
            # Extract and validate org data (optional)
            org = event.get('org', {})
            org_data = {
                'id': self._safe_int(org.get('id')) if org else None,
                'login': self._safe_str(org.get('login'), 255) if org else None,
                'node_id': self._safe_str(org.get('node_id'), 255) if org else None,
                'gravatar_id': self._safe_str(org.get('gravatar_id'), 255) if org else None,
                'url': self._safe_str(org.get('url')) if org else None,
                'avatar_url': self._safe_str(org.get('avatar_url')) if org else None,
                'html_url': self._safe_str(org.get('html_url')) if org else None,
                'type': self._safe_str(org.get('type'), 50) if org else None,
                'site_admin': self._safe_bool(org.get('site_admin')) if org else None
            }
            
            return {
                'id': event_id,
                'type': event_type,
                'created_at': created_at,
                'public': self._safe_bool(event.get('public', True)),
                'actor': actor_data,
                'repo': repo_data,
                'org': org_data,
                'payload': event.get('payload', {}),
                'raw_event': event,
                'api_source': 'github_archive'
            }
            
        except Exception as e:
            self.logger.error(f"Event validation failed: {e}")
            return None
    
    def _extract_event_parameters(self, event: Dict[str, Any], filename: str) -> List[Any]:
        """Extract parameters for comprehensive INSERT statement."""
        actor = event['actor']
        repo = event['repo']
        org = event['org']
        
        return [
            # Core event fields (4 params)
            event['id'], event['type'], event['created_at'], event['public'],
            
            # Actor fields (20 params)
            actor['id'], actor['login'], actor['display_login'], actor['gravatar_id'],
            actor['url'], actor['avatar_url'], actor['node_id'], actor['html_url'],
            actor['followers_url'], actor['following_url'], actor['gists_url'],
            actor['starred_url'], actor['subscriptions_url'], actor['organizations_url'],
            actor['repos_url'], actor['events_url'], actor['received_events_url'],
            actor['type'], actor['user_view_type'], actor['site_admin'],
            
            # Repository fields (25 params)
            repo['id'], repo['name'], repo['url'], repo['full_name'], repo['owner_login'],
            repo['owner_id'], repo['owner_node_id'], repo['owner_avatar_url'], repo['owner_gravatar_id'],
            repo['owner_url'], repo['owner_html_url'], repo['owner_type'], repo['owner_site_admin'],
            repo['node_id'], repo['html_url'], repo['description'], repo['fork'], repo['language'],
            repo['stargazers_count'], repo['watchers_count'], repo['forks_count'], repo['open_issues_count'],
            repo['size'], repo['default_branch'], repo['topics'], repo['license_key'], repo['license_name'],
            repo['created_at'], repo['updated_at'], repo['pushed_at'],
            
            # Organization fields (9 params)
            org['id'], org['login'], org['node_id'], org['gravatar_id'], org['url'],
            org['avatar_url'], org['html_url'], org['type'], org['site_admin'],
            
            # Data storage fields (4 params)
            json.dumps(event['payload']), json.dumps(event['raw_event']), filename, event['api_source']
        ]
    
    def _calculate_quality_score(self, event_stats: Dict, integrity_issues: Dict) -> float:
        """Calculate data quality score (0-100)."""
        if not event_stats or event_stats['total_events'] == 0:
            return 0.0
            
        total_events = event_stats['total_events']
        issues = sum(integrity_issues.values())
        
        clean_percentage = ((total_events - issues) / total_events) * 100
        return min(100.0, max(0.0, clean_percentage))
    
    # Utility methods for safe type conversion
    def _safe_int(self, value: Any) -> Optional[int]:
        """Safely convert value to integer."""
        if value is None or value == '':
            return None
        try:
            return int(value)
        except (ValueError, TypeError):
            return None
    
    def _safe_str(self, value: Any, max_length: Optional[int] = None) -> Optional[str]:
        """Safely convert value to string with optional length limit."""
        if value is None:
            return None
        str_value = str(value)
        if max_length and len(str_value) > max_length:
            return str_value[:max_length]
        return str_value
    
    def _safe_bool(self, value: Any) -> Optional[bool]:
        """Safely convert value to boolean."""
        if value is None:
            return None
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            return value.lower() in ('true', '1', 'yes', 'on')
        return bool(value)
    
    def _safe_array(self, value: Any) -> List[Any]:
        """Safely convert value to array."""
        if value is None:
            return []
        if isinstance(value, list):
            return value
        return [value]
    
    def _parse_date(self, date_str: Any) -> Optional[datetime]:
        """Parse ISO date string to datetime."""
        if not date_str:
            return None
        try:
            if isinstance(date_str, str):
                return datetime.fromisoformat(date_str.replace('Z', '+00:00'))
            return date_str
        except (ValueError, TypeError):
            return None
    
    def _get_comprehensive_insert_sql(self) -> str:
        """Get the comprehensive INSERT SQL statement."""
        return """
            INSERT INTO github_events (
                -- Core event fields
                event_id, event_type, event_created_at, event_public,
                
                -- Actor fields
                actor_id, actor_login, actor_display_login, actor_gravatar_id, actor_url, 
                actor_avatar_url, actor_node_id, actor_html_url, actor_followers_url, 
                actor_following_url, actor_gists_url, actor_starred_url, actor_subscriptions_url,
                actor_organizations_url, actor_repos_url, actor_events_url, actor_received_events_url,
                actor_type, actor_user_view_type, actor_site_admin,
                
                -- Repository fields
                repo_id, repo_name, repo_url, repo_full_name, repo_owner_login, repo_owner_id,
                repo_owner_node_id, repo_owner_avatar_url, repo_owner_gravatar_id, repo_owner_url,
                repo_owner_html_url, repo_owner_type, repo_owner_site_admin, repo_node_id, 
                repo_html_url, repo_description, repo_fork, repo_language, repo_stargazers_count,
                repo_watchers_count, repo_forks_count, repo_open_issues_count, repo_size,
                repo_default_branch, repo_topics, repo_license_key, repo_license_name,
                repo_created_at, repo_updated_at, repo_pushed_at,
                
                -- Organization fields
                org_id, org_login, org_node_id, org_gravatar_id, org_url, org_avatar_url,
                org_html_url, org_type, org_site_admin,
                
                -- Data storage fields
                payload, raw_event, file_source, api_source
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38,
                $39, $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56,
                $57, $58, $59, $60, $61, $62, $63, $64, $65, $66, $67
            )
            ON CONFLICT (event_id) DO UPDATE SET
                payload = EXCLUDED.payload,
                raw_event = EXCLUDED.raw_event,
                processed_at = NOW()
        """
    
    def _get_schema_sql(self) -> str:
        """Get the database schema SQL."""
        return """
            -- Create extensions
            CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
            CREATE EXTENSION IF NOT EXISTS "btree_gin";
            CREATE EXTENSION IF NOT EXISTS "pg_trgm";
            
            -- Main events table with comprehensive GitHub API data capture
            CREATE TABLE IF NOT EXISTS github_events (
                event_id BIGINT PRIMARY KEY,
                event_type VARCHAR(50) NOT NULL,
                event_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
                event_public BOOLEAN NOT NULL DEFAULT true,
                
                -- Actor information
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
                
                -- Repository information
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
                repo_language VARCHAR(100),
                repo_stargazers_count BIGINT,
                repo_watchers_count BIGINT,
                repo_forks_count BIGINT,
                repo_open_issues_count BIGINT,
                repo_size BIGINT,
                repo_default_branch VARCHAR(100),
                repo_topics TEXT[],
                repo_license_key VARCHAR(50),
                repo_license_name VARCHAR(255),
                repo_created_at TIMESTAMP WITH TIME ZONE,
                repo_updated_at TIMESTAMP WITH TIME ZONE,
                repo_pushed_at TIMESTAMP WITH TIME ZONE,
                
                -- Organization information (optional)
                org_id BIGINT,
                org_login VARCHAR(255),
                org_node_id VARCHAR(255),
                org_gravatar_id VARCHAR(255),
                org_url TEXT,
                org_avatar_url TEXT,
                org_html_url TEXT,
                org_type VARCHAR(50),
                org_site_admin BOOLEAN,
                
                -- Complete payload as JSONB for flexible querying
                payload JSONB,
                raw_event JSONB,
                
                -- Metadata
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                file_source VARCHAR(255),
                api_source VARCHAR(255)
            );
            
            -- Processed files tracking
            CREATE TABLE IF NOT EXISTS processed_files (
                filename VARCHAR(255) PRIMARY KEY,
                file_size BIGINT,
                etag VARCHAR(255),
                last_modified TIMESTAMP WITH TIME ZONE,
                processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                event_count INTEGER DEFAULT 0,
                is_complete BOOLEAN DEFAULT TRUE
            );
            
            -- Repositories tracking table
            CREATE TABLE IF NOT EXISTS repositories (
                id BIGINT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                full_name VARCHAR(255),
                description TEXT,
                html_url TEXT,
                language VARCHAR(100),
                default_branch VARCHAR(100),
                created_at TIMESTAMP WITH TIME ZONE,
                updated_at TIMESTAMP WITH TIME ZONE,
                pushed_at TIMESTAMP WITH TIME ZONE,
                stargazers_count INTEGER,
                watchers_count INTEGER,
                forks_count INTEGER,
                open_issues_count INTEGER,
                topics TEXT[],
                license_name VARCHAR(255),
                owner_login VARCHAR(255),
                owner_type VARCHAR(50),
                first_seen_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                last_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            );
            
            -- Performance indexes
            CREATE INDEX IF NOT EXISTS idx_github_events_created_at ON github_events (event_created_at);
            CREATE INDEX IF NOT EXISTS idx_github_events_type ON github_events (event_type);
            CREATE INDEX IF NOT EXISTS idx_github_events_actor_id ON github_events (actor_id);
            CREATE INDEX IF NOT EXISTS idx_github_events_repo_id ON github_events (repo_id);
            CREATE INDEX IF NOT EXISTS idx_github_events_actor_login ON github_events (actor_login);
            CREATE INDEX IF NOT EXISTS idx_github_events_repo_name ON github_events (repo_name);
            CREATE INDEX IF NOT EXISTS idx_github_events_payload ON github_events USING GIN (payload);
            CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories (language);
            CREATE INDEX IF NOT EXISTS idx_repositories_stars ON repositories (stargazers_count DESC);
        """
