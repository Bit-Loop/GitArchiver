#!/usr/bin/env python3
"""
Enhanced GitHub Archive Scraper - Complete Event API Focus
Captures EVERYTHING from GitHub's event API without data loss

This is the ultimate GitHub event capture system designed to extract and preserve
every piece of data from GitHub's comprehensive event API.
"""

import asyncio
import aiohttp
import asyncpg
import gzip
import json
import logging
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional, Any, Set
from config import Config
import psutil
import signal
from dataclasses import dataclass

# Enhanced logging configuration
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler('enhanced_scraper.log')
    ]
)
logger = logging.getLogger(__name__)

@dataclass
class EventMetrics:
    """Comprehensive metrics for event processing"""
    total_processed: int = 0
    validation_errors: int = 0
    database_errors: int = 0
    data_quality_score: float = 100.0
    event_types: Dict[str, int] = None
    processing_rate: float = 0.0
    memory_usage_mb: float = 0.0
    
    def __post_init__(self):
        if self.event_types is None:
            self.event_types = {}

class EnhancedGitHubEventProcessor:
    """
    Ultra-comprehensive GitHub event processor focused on capturing EVERYTHING
    """
    
    def __init__(self, config: Config):
        self.config = config
        self.pool = None
        self.session = None
        self.metrics = EventMetrics()
        self.shutdown_flag = False
        
        # Event type handlers for specialized processing
        self.event_handlers = {
            'PushEvent': self._process_push_event,
            'PullRequestEvent': self._process_pull_request_event,
            'IssuesEvent': self._process_issues_event,
            'ReleaseEvent': self._process_release_event,
            'CreateEvent': self._process_create_event,
            'DeleteEvent': self._process_delete_event,
            'ForkEvent': self._process_fork_event,
            'WatchEvent': self._process_watch_event,
            'PullRequestReviewEvent': self._process_pr_review_event,
            'PullRequestReviewCommentEvent': self._process_pr_review_comment_event,
            'IssueCommentEvent': self._process_issue_comment_event,
            'CommitCommentEvent': self._process_commit_comment_event,
            'GollumEvent': self._process_gollum_event,
            'MemberEvent': self._process_member_event,
            'PublicEvent': self._process_public_event
        }
        
        # Register signal handlers for graceful shutdown
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)
    
    def _signal_handler(self, signum, frame):
        """Handle shutdown signals gracefully"""
        logger.info(f"Received signal {signum}, initiating graceful shutdown...")
        self.shutdown_flag = True
    
    async def initialize(self):
        """Initialize database and HTTP connections"""
        logger.info("🚀 Initializing Enhanced GitHub Event Processor")
        
        # Initialize database connection pool
        self.pool = await asyncpg.create_pool(
            host=self.config.DB_HOST,
            port=self.config.DB_PORT,
            user=self.config.DB_USER,
            password=self.config.DB_PASSWORD,
            database=self.config.DB_NAME,
            min_size=self.config.DB_MIN_CONNECTIONS,
            max_size=self.config.DB_MAX_CONNECTIONS,
            server_settings={'jit': 'off'}  # Optimize for OLTP workload
        )
        
        # Initialize HTTP session with optimized settings
        timeout = aiohttp.ClientTimeout(total=self.config.REQUEST_TIMEOUT)
        connector = aiohttp.TCPConnector(
            limit=100,
            limit_per_host=30,
            keepalive_timeout=300,
            enable_cleanup_closed=True
        )
        
        self.session = aiohttp.ClientSession(
            timeout=timeout,
            connector=connector,
            headers={'User-Agent': 'Enhanced-GitHub-Scraper/3.0'}
        )
        
        # Initialize comprehensive database schema
        await self._ensure_enhanced_schema()
        
        logger.info("✅ Enhanced processor initialization complete")
    
    async def _ensure_enhanced_schema(self):
        """Create the ultimate comprehensive schema for complete event capture"""
        logger.info("📊 Setting up comprehensive database schema...")
        
        async with self.pool.acquire() as conn:
            # Drop existing table if it exists
            await conn.execute("DROP TABLE IF EXISTS github_events CASCADE")
            
            # Create the ultimate comprehensive events table
            schema_sql = """
            CREATE TABLE IF NOT EXISTS github_events (
                -- Primary event identification
                event_id BIGINT PRIMARY KEY,
                event_type VARCHAR(100) NOT NULL,
                event_created_at TIMESTAMPTZ NOT NULL,
                event_public BOOLEAN DEFAULT true,
                
                -- Complete Actor Information (GitHub User/Organization)
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
                
                -- Complete Repository Information
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
                repo_stargazers_count INTEGER,
                repo_watchers_count INTEGER,
                repo_language VARCHAR(100),
                repo_has_issues BOOLEAN,
                repo_has_projects BOOLEAN,
                repo_has_downloads BOOLEAN,
                repo_has_wiki BOOLEAN,
                repo_has_pages BOOLEAN,
                repo_has_discussions BOOLEAN,
                repo_forks_count INTEGER,
                repo_mirror_url TEXT,
                repo_archived BOOLEAN,
                repo_disabled BOOLEAN,
                repo_open_issues_count INTEGER,
                repo_license_key VARCHAR(100),
                repo_license_name VARCHAR(255),
                repo_license_spdx_id VARCHAR(100),
                repo_license_url TEXT,
                repo_license_node_id VARCHAR(255),
                repo_allow_forking BOOLEAN,
                repo_is_template BOOLEAN,
                repo_web_commit_signoff_required BOOLEAN,
                repo_topics JSONB,
                repo_visibility VARCHAR(50),
                repo_forks INTEGER,
                repo_open_issues INTEGER,
                repo_watchers INTEGER,
                repo_default_branch VARCHAR(255),
                repo_created_at TIMESTAMPTZ,
                repo_updated_at TIMESTAMPTZ,
                repo_pushed_at TIMESTAMPTZ,
                
                -- Complete Organization Information
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
                
                -- Event-Specific Payload Data (Specialized by Event Type)
                payload_push_id BIGINT,
                payload_size INTEGER,
                payload_distinct_size INTEGER,
                payload_ref VARCHAR(255),
                payload_head VARCHAR(255),
                payload_before VARCHAR(255),
                payload_commits JSONB,
                
                payload_action VARCHAR(100),
                payload_number INTEGER,
                payload_pull_request JSONB,
                payload_issue JSONB,
                payload_comment JSONB,
                payload_release JSONB,
                payload_review JSONB,
                payload_ref_type VARCHAR(50),
                payload_master_branch VARCHAR(255),
                payload_description TEXT,
                payload_pusher_type VARCHAR(50),
                payload_member JSONB,
                payload_team JSONB,
                payload_pages JSONB,
                payload_forkee JSONB,
                
                -- Raw data preservation and metadata
                payload_raw JSONB NOT NULL,
                raw_event JSONB NOT NULL,
                file_source VARCHAR(255),
                api_source VARCHAR(100) DEFAULT 'gharchive',
                data_version VARCHAR(50) DEFAULT '3.0',
                processed_at TIMESTAMPTZ DEFAULT NOW(),
                data_quality_score FLOAT DEFAULT 100.0,
                validation_errors JSONB,
                
                -- Indexes for performance
                CONSTRAINT unique_event_id UNIQUE (event_id)
            );
            
            -- Performance indexes
            CREATE INDEX IF NOT EXISTS idx_events_created_at ON github_events(event_created_at);
            CREATE INDEX IF NOT EXISTS idx_events_type ON github_events(event_type);
            CREATE INDEX IF NOT EXISTS idx_events_actor_id ON github_events(actor_id);
            CREATE INDEX IF NOT EXISTS idx_events_repo_id ON github_events(repo_id);
            CREATE INDEX IF NOT EXISTS idx_events_org_id ON github_events(org_id);
            CREATE INDEX IF NOT EXISTS idx_events_actor_login ON github_events(actor_login);
            CREATE INDEX IF NOT EXISTS idx_events_repo_name ON github_events(repo_full_name);
            CREATE INDEX IF NOT EXISTS idx_events_payload_action ON github_events(payload_action);
            CREATE INDEX IF NOT EXISTS idx_events_processed_at ON github_events(processed_at);
            CREATE INDEX IF NOT EXISTS idx_events_quality_score ON github_events(data_quality_score);
            
            -- JSONB indexes for payload searching
            CREATE INDEX IF NOT EXISTS idx_events_payload_gin ON github_events USING GIN(payload_raw);
            CREATE INDEX IF NOT EXISTS idx_events_raw_gin ON github_events USING GIN(raw_event);
            CREATE INDEX IF NOT EXISTS idx_events_topics_gin ON github_events USING GIN(repo_topics);
            """
            
            await conn.execute(schema_sql)
            
        logger.info("✅ Comprehensive database schema initialized with 120+ fields")
    
    def validate_and_extract_event(self, raw_event: Dict, source_file: str) -> Optional[Dict]:
        """
        Ultimate event validation and data extraction
        Extracts EVERY piece of available data from GitHub events
        """
        try:
            # Core validation
            if not isinstance(raw_event, dict):
                return None
            
            event_id = raw_event.get('id')
            if not event_id:
                return None
                
            # Safe conversion helpers
            def safe_int(value):
                if value is None:
                    return None
                try:
                    if isinstance(value, str):
                        # Handle string numbers
                        return int(value)
                    return int(value)
                except (ValueError, TypeError):
                    return None
            
            def safe_str(value, max_length=None):
                if value is None:
                    return None
                str_value = str(value)
                if max_length and len(str_value) > max_length:
                    return str_value[:max_length]
                return str_value
            
            def safe_bool(value):
                if value is None:
                    return None
                if isinstance(value, bool):
                    return value
                if isinstance(value, str):
                    return value.lower() in ('true', '1', 'yes', 'on')
                return bool(value)
            
            def safe_timestamp(value):
                if value is None:
                    return None
                try:
                    if isinstance(value, str):
                        return datetime.fromisoformat(value.replace('Z', '+00:00'))
                    return value
                except:
                    return None
            
            # Extract complete actor data
            actor = raw_event.get('actor', {})
            actor_data = {
                'id': safe_int(actor.get('id')),
                'login': safe_str(actor.get('login'), 255),
                'display_login': safe_str(actor.get('display_login'), 255),
                'gravatar_id': safe_str(actor.get('gravatar_id'), 255),
                'url': safe_str(actor.get('url')),
                'avatar_url': safe_str(actor.get('avatar_url')),
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
            
            # Extract complete repository data
            repo = raw_event.get('repo', {})
            repo_owner = repo.get('owner', {})
            repo_license = repo.get('license', {})
            
            repo_data = {
                'id': safe_int(repo.get('id')),
                'name': safe_str(repo.get('name'), 255),
                'url': safe_str(repo.get('url')),
                'full_name': safe_str(repo.get('full_name'), 255),
                'owner_login': safe_str(repo_owner.get('login'), 255),
                'owner_id': safe_int(repo_owner.get('id')),
                'owner_node_id': safe_str(repo_owner.get('node_id'), 255),
                'owner_avatar_url': safe_str(repo_owner.get('avatar_url')),
                'owner_gravatar_id': safe_str(repo_owner.get('gravatar_id'), 255),
                'owner_url': safe_str(repo_owner.get('url')),
                'owner_html_url': safe_str(repo_owner.get('html_url')),
                'owner_type': safe_str(repo_owner.get('type'), 50),
                'owner_site_admin': safe_bool(repo_owner.get('site_admin')),
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
                'license_key': safe_str(repo_license.get('key'), 100),
                'license_name': safe_str(repo_license.get('name'), 255),
                'license_spdx_id': safe_str(repo_license.get('spdx_id'), 100),
                'license_url': safe_str(repo_license.get('url')),
                'license_node_id': safe_str(repo_license.get('node_id'), 255),
                'allow_forking': safe_bool(repo.get('allow_forking')),
                'is_template': safe_bool(repo.get('is_template')),
                'web_commit_signoff_required': safe_bool(repo.get('web_commit_signoff_required')),
                'topics': repo.get('topics', []),
                'visibility': safe_str(repo.get('visibility'), 50),
                'forks': safe_int(repo.get('forks')),
                'open_issues': safe_int(repo.get('open_issues')),
                'watchers': safe_int(repo.get('watchers')),
                'default_branch': safe_str(repo.get('default_branch'), 255),
                'created_at': safe_timestamp(repo.get('created_at')),
                'updated_at': safe_timestamp(repo.get('updated_at')),
                'pushed_at': safe_timestamp(repo.get('pushed_at'))
            }
            
            # Extract complete organization data
            org = raw_event.get('org', {})
            org_data = {
                'id': safe_int(org.get('id')),
                'login': safe_str(org.get('login'), 255),
                'node_id': safe_str(org.get('node_id'), 255),
                'gravatar_id': safe_str(org.get('gravatar_id'), 255),
                'url': safe_str(org.get('url')),
                'avatar_url': safe_str(org.get('avatar_url')),
                'html_url': safe_str(org.get('html_url')),
                'followers_url': safe_str(org.get('followers_url')),
                'following_url': safe_str(org.get('following_url')),
                'gists_url': safe_str(org.get('gists_url')),
                'starred_url': safe_str(org.get('starred_url')),
                'subscriptions_url': safe_str(org.get('subscriptions_url')),
                'organizations_url': safe_str(org.get('organizations_url')),
                'repos_url': safe_str(org.get('repos_url')),
                'events_url': safe_str(org.get('events_url')),
                'received_events_url': safe_str(org.get('received_events_url')),
                'type': safe_str(org.get('type'), 50),
                'user_view_type': safe_str(org.get('user_view_type'), 50),
                'site_admin': safe_bool(org.get('site_admin'))
            }
            
            # Extract event-specific payload data
            payload = raw_event.get('payload', {})
            payload_data = self._extract_payload_data(raw_event.get('type'), payload)
            
            # Build comprehensive event record
            event_record = {
                # Core event data
                'event_id': safe_int(event_id),
                'event_type': safe_str(raw_event.get('type'), 100),
                'event_created_at': safe_timestamp(raw_event.get('created_at')),
                'event_public': safe_bool(raw_event.get('public')),
                
                # Actor data (with actor_ prefix)
                **{f'actor_{k}': v for k, v in actor_data.items()},
                
                # Repository data (with repo_ prefix) 
                **{f'repo_{k}': v for k, v in repo_data.items()},
                
                # Organization data (with org_ prefix)
                **{f'org_{k}': v for k, v in org_data.items()},
                
                # Event-specific payload data (with payload_ prefix)
                **{f'payload_{k}': v for k, v in payload_data.items()},
                
                # Raw data preservation
                'payload_raw': payload,
                'raw_event': raw_event,
                'file_source': source_file,
                'api_source': 'gharchive',
                'data_version': '3.0',
                'data_quality_score': 100.0,
                'validation_errors': None
            }
            
            return event_record
            
        except Exception as e:
            logger.error(f"Event validation failed: {e}")
            return None
    
    def _extract_payload_data(self, event_type: str, payload: Dict) -> Dict:
        """Extract event-specific payload data based on event type"""
        data = {}
        
        try:
            if event_type == 'PushEvent':
                data.update({
                    'push_id': payload.get('push_id'),
                    'size': payload.get('size'),
                    'distinct_size': payload.get('distinct_size'),
                    'ref': payload.get('ref'),
                    'head': payload.get('head'),
                    'before': payload.get('before'),
                    'commits': payload.get('commits', [])
                })
            
            elif event_type in ['PullRequestEvent', 'IssuesEvent', 'PullRequestReviewEvent']:
                data.update({
                    'action': payload.get('action'),
                    'number': payload.get('number'),
                    'pull_request': payload.get('pull_request'),
                    'issue': payload.get('issue'),
                    'review': payload.get('review')
                })
            
            elif event_type in ['IssueCommentEvent', 'PullRequestReviewCommentEvent', 'CommitCommentEvent']:
                data.update({
                    'action': payload.get('action'),
                    'comment': payload.get('comment')
                })
            
            elif event_type == 'ReleaseEvent':
                data.update({
                    'action': payload.get('action'),
                    'release': payload.get('release')
                })
            
            elif event_type in ['CreateEvent', 'DeleteEvent']:
                data.update({
                    'ref': payload.get('ref'),
                    'ref_type': payload.get('ref_type'),
                    'master_branch': payload.get('master_branch'),
                    'description': payload.get('description'),
                    'pusher_type': payload.get('pusher_type')
                })
            
            elif event_type == 'ForkEvent':
                data.update({
                    'forkee': payload.get('forkee')
                })
            
            elif event_type == 'MemberEvent':
                data.update({
                    'action': payload.get('action'),
                    'member': payload.get('member')
                })
            
            elif event_type == 'GollumEvent':
                data.update({
                    'pages': payload.get('pages', [])
                })
            
            # Always include action if present
            if 'action' in payload and 'action' not in data:
                data['action'] = payload.get('action')
                
        except Exception as e:
            logger.warning(f"Error extracting payload for {event_type}: {e}")
        
        return data
    
    async def process_archive_file(self, file_url: str) -> int:
        """Process a single GitHub archive file with comprehensive event extraction"""
        start_time = time.time()
        events_processed = 0
        
        try:
            logger.info(f"📥 Processing archive: {file_url}")
            
            # Download and decompress archive
            async with self.session.get(file_url) as response:
                if response.status != 200:
                    logger.error(f"Failed to download {file_url}: {response.status}")
                    return 0
                
                compressed_data = await response.read()
            
            # Decompress and process events
            decompressed_data = gzip.decompress(compressed_data)
            events = []
            
            for line in decompressed_data.decode('utf-8').strip().split('\n'):
                if not line.strip():
                    continue
                    
                try:
                    raw_event = json.loads(line)
                    validated_event = self.validate_and_extract_event(raw_event, file_url)
                    
                    if validated_event:
                        events.append(validated_event)
                        
                        # Track event types
                        event_type = validated_event.get('event_type', 'Unknown')
                        self.metrics.event_types[event_type] = self.metrics.event_types.get(event_type, 0) + 1
                        
                        # Process in batches for memory efficiency
                        if len(events) >= self.config.BATCH_SIZE:
                            await self._batch_insert_events(events)
                            events_processed += len(events)
                            events.clear()
                            
                            # Check for shutdown signal
                            if self.shutdown_flag:
                                break
                                
                except json.JSONDecodeError as e:
                    logger.warning(f"Invalid JSON in {file_url}: {e}")
                    self.metrics.validation_errors += 1
                    continue
            
            # Process remaining events
            if events:
                await self._batch_insert_events(events)
                events_processed += len(events)
            
            processing_time = time.time() - start_time
            self.metrics.processing_rate = events_processed / processing_time if processing_time > 0 else 0
            self.metrics.memory_usage_mb = psutil.Process().memory_info().rss / 1024 / 1024
            
            logger.info(f"✅ Processed {events_processed} events from {file_url} in {processing_time:.2f}s")
            return events_processed
            
        except Exception as e:
            logger.error(f"Error processing {file_url}: {e}")
            return 0
    
    async def _batch_insert_events(self, events: List[Dict]):
        """Efficiently insert a batch of events into the database"""
        if not events:
            return
            
        try:
            async with self.pool.acquire() as conn:
                # Prepare comprehensive INSERT statement
                insert_sql = """
                INSERT INTO github_events (
                    event_id, event_type, event_created_at, event_public,
                    actor_id, actor_login, actor_display_login, actor_gravatar_id, actor_url, actor_avatar_url,
                    actor_node_id, actor_html_url, actor_followers_url, actor_following_url, actor_gists_url,
                    actor_starred_url, actor_subscriptions_url, actor_organizations_url, actor_repos_url,
                    actor_events_url, actor_received_events_url, actor_type, actor_user_view_type, actor_site_admin,
                    
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
                    
                    org_id, org_login, org_node_id, org_gravatar_id, org_url, org_avatar_url, org_html_url,
                    org_followers_url, org_following_url, org_gists_url, org_starred_url, org_subscriptions_url,
                    org_organizations_url, org_repos_url, org_events_url, org_received_events_url,
                    org_type, org_user_view_type, org_site_admin,
                    
                    payload_push_id, payload_size, payload_distinct_size, payload_ref, payload_head, payload_before,
                    payload_commits, payload_action, payload_number, payload_pull_request, payload_issue,
                    payload_comment, payload_release, payload_review, payload_ref_type, payload_master_branch,
                    payload_description, payload_pusher_type, payload_member, payload_forkee, payload_pages,
                    
                    payload_raw, raw_event, file_source, api_source, data_version, data_quality_score
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                    $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40,
                    $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56, $57, $58, $59, $60,
                    $61, $62, $63, $64, $65, $66, $67, $68, $69, $70, $71, $72, $73, $74, $75, $76, $77, $78, $79, $80,
                    $81, $82, $83, $84, $85, $86, $87, $88, $89, $90, $91, $92, $93, $94, $95, $96, $97, $98, $99, $100,
                    $101, $102, $103, $104, $105, $106, $107, $108, $109, $110, $111, $112, $113, $114, $115, $116, $117, $118, $119, $120,
                    $121, $122, $123, $124, $125, $126, $127, $128, $129, $130, $131, $132, $133, $134, $135, $136, $137, $138
                ) ON CONFLICT (event_id) DO NOTHING
                """
                
                # Prepare batch data
                batch_data = []
                for event in events:
                    row_data = [
                        # Core event fields (4)
                        event.get('event_id'), event.get('event_type'), event.get('event_created_at'), event.get('event_public'),
                        
                        # Actor fields (20)
                        event.get('actor_id'), event.get('actor_login'), event.get('actor_display_login'), 
                        event.get('actor_gravatar_id'), event.get('actor_url'), event.get('actor_avatar_url'),
                        event.get('actor_node_id'), event.get('actor_html_url'), event.get('actor_followers_url'),
                        event.get('actor_following_url'), event.get('actor_gists_url'), event.get('actor_starred_url'),
                        event.get('actor_subscriptions_url'), event.get('actor_organizations_url'), event.get('actor_repos_url'),
                        event.get('actor_events_url'), event.get('actor_received_events_url'), event.get('actor_type'),
                        event.get('actor_user_view_type'), event.get('actor_site_admin'),
                        
                        # Repository fields (80)
                        event.get('repo_id'), event.get('repo_name'), event.get('repo_url'), event.get('repo_full_name'),
                        event.get('repo_owner_login'), event.get('repo_owner_id'), event.get('repo_owner_node_id'),
                        event.get('repo_owner_avatar_url'), event.get('repo_owner_gravatar_id'), event.get('repo_owner_url'),
                        event.get('repo_owner_html_url'), event.get('repo_owner_type'), event.get('repo_owner_site_admin'),
                        event.get('repo_node_id'), event.get('repo_html_url'), event.get('repo_description'),
                        event.get('repo_fork'), event.get('repo_keys_url'), event.get('repo_collaborators_url'),
                        event.get('repo_teams_url'), event.get('repo_hooks_url'), event.get('repo_issue_events_url'),
                        event.get('repo_events_url'), event.get('repo_assignees_url'), event.get('repo_branches_url'),
                        event.get('repo_tags_url'), event.get('repo_blobs_url'), event.get('repo_git_tags_url'),
                        event.get('repo_git_refs_url'), event.get('repo_trees_url'), event.get('repo_statuses_url'),
                        event.get('repo_languages_url'), event.get('repo_stargazers_url'), event.get('repo_contributors_url'),
                        event.get('repo_subscribers_url'), event.get('repo_subscription_url'), event.get('repo_commits_url'),
                        event.get('repo_git_commits_url'), event.get('repo_comments_url'), event.get('repo_issue_comment_url'),
                        event.get('repo_contents_url'), event.get('repo_compare_url'), event.get('repo_merges_url'),
                        event.get('repo_archive_url'), event.get('repo_downloads_url'), event.get('repo_issues_url'),
                        event.get('repo_pulls_url'), event.get('repo_milestones_url'), event.get('repo_notifications_url'),
                        event.get('repo_labels_url'), event.get('repo_releases_url'), event.get('repo_deployments_url'),
                        event.get('repo_git_url'), event.get('repo_ssh_url'), event.get('repo_clone_url'),
                        event.get('repo_svn_url'), event.get('repo_homepage'), event.get('repo_size'),
                        event.get('repo_stargazers_count'), event.get('repo_watchers_count'), event.get('repo_language'),
                        event.get('repo_has_issues'), event.get('repo_has_projects'), event.get('repo_has_downloads'),
                        event.get('repo_has_wiki'), event.get('repo_has_pages'), event.get('repo_has_discussions'),
                        event.get('repo_forks_count'), event.get('repo_mirror_url'), event.get('repo_archived'),
                        event.get('repo_disabled'), event.get('repo_open_issues_count'), event.get('repo_license_key'),
                        event.get('repo_license_name'), event.get('repo_license_spdx_id'), event.get('repo_license_url'),
                        event.get('repo_license_node_id'), event.get('repo_allow_forking'), event.get('repo_is_template'),
                        event.get('repo_web_commit_signoff_required'), json.dumps(event.get('repo_topics', [])),
                        event.get('repo_visibility'), event.get('repo_forks'), event.get('repo_open_issues'),
                        event.get('repo_watchers'), event.get('repo_default_branch'), event.get('repo_created_at'),
                        event.get('repo_updated_at'), event.get('repo_pushed_at'),
                        
                        # Organization fields (19)
                        event.get('org_id'), event.get('org_login'), event.get('org_node_id'), event.get('org_gravatar_id'),
                        event.get('org_url'), event.get('org_avatar_url'), event.get('org_html_url'),
                        event.get('org_followers_url'), event.get('org_following_url'), event.get('org_gists_url'),
                        event.get('org_starred_url'), event.get('org_subscriptions_url'), event.get('org_organizations_url'),
                        event.get('org_repos_url'), event.get('org_events_url'), event.get('org_received_events_url'),
                        event.get('org_type'), event.get('org_user_view_type'), event.get('org_site_admin'),
                        
                        # Payload fields (21)
                        event.get('payload_push_id'), event.get('payload_size'), event.get('payload_distinct_size'),
                        event.get('payload_ref'), event.get('payload_head'), event.get('payload_before'),
                        json.dumps(event.get('payload_commits', [])), event.get('payload_action'), event.get('payload_number'),
                        json.dumps(event.get('payload_pull_request')) if event.get('payload_pull_request') else None,
                        json.dumps(event.get('payload_issue')) if event.get('payload_issue') else None,
                        json.dumps(event.get('payload_comment')) if event.get('payload_comment') else None,
                        json.dumps(event.get('payload_release')) if event.get('payload_release') else None,
                        json.dumps(event.get('payload_review')) if event.get('payload_review') else None,
                        event.get('payload_ref_type'), event.get('payload_master_branch'), event.get('payload_description'),
                        event.get('payload_pusher_type'),
                        json.dumps(event.get('payload_member')) if event.get('payload_member') else None,
                        json.dumps(event.get('payload_forkee')) if event.get('payload_forkee') else None,
                        json.dumps(event.get('payload_pages', [])),
                        
                        # Raw data and metadata (6)
                        json.dumps(event.get('payload_raw', {})), json.dumps(event.get('raw_event', {})),
                        event.get('file_source'), event.get('api_source'), event.get('data_version'),
                        event.get('data_quality_score', 100.0)
                    ]
                    batch_data.append(row_data)
                
                # Execute batch insert using executemany for performance
                await conn.executemany(insert_sql, batch_data)
                
                self.metrics.total_processed += len(events)
                logger.debug(f"✅ Inserted batch of {len(events)} events")
                
        except Exception as e:
            logger.error(f"Batch insert failed: {e}")
            self.metrics.database_errors += 1
    
    # Event-specific processors for specialized handling
    async def _process_push_event(self, event: Dict):
        """Process PushEvent with commit details"""
        # Additional processing for push events if needed
        pass
    
    async def _process_pull_request_event(self, event: Dict):
        """Process PullRequestEvent with PR details"""
        # Additional processing for PR events if needed
        pass
    
    async def _process_issues_event(self, event: Dict):
        """Process IssuesEvent with issue details"""
        # Additional processing for issue events if needed
        pass
    
    async def _process_release_event(self, event: Dict):
        """Process ReleaseEvent with release details"""
        # Additional processing for release events if needed
        pass
    
    async def _process_create_event(self, event: Dict):
        """Process CreateEvent (repository/branch/tag creation)"""
        # Additional processing for create events if needed
        pass
    
    async def _process_delete_event(self, event: Dict):
        """Process DeleteEvent (branch/tag deletion)"""
        # Additional processing for delete events if needed
        pass
    
    async def _process_fork_event(self, event: Dict):
        """Process ForkEvent with fork details"""
        # Additional processing for fork events if needed
        pass
    
    async def _process_watch_event(self, event: Dict):
        """Process WatchEvent (starring)"""
        # Additional processing for watch events if needed
        pass
    
    async def _process_pr_review_event(self, event: Dict):
        """Process PullRequestReviewEvent"""
        # Additional processing for PR review events if needed
        pass
    
    async def _process_pr_review_comment_event(self, event: Dict):
        """Process PullRequestReviewCommentEvent"""
        # Additional processing for PR review comment events if needed
        pass
    
    async def _process_issue_comment_event(self, event: Dict):
        """Process IssueCommentEvent"""
        # Additional processing for issue comment events if needed
        pass
    
    async def _process_commit_comment_event(self, event: Dict):
        """Process CommitCommentEvent"""
        # Additional processing for commit comment events if needed
        pass
    
    async def _process_gollum_event(self, event: Dict):
        """Process GollumEvent (wiki changes)"""
        # Additional processing for wiki events if needed
        pass
    
    async def _process_member_event(self, event: Dict):
        """Process MemberEvent (collaborator changes)"""
        # Additional processing for member events if needed
        pass
    
    async def _process_public_event(self, event: Dict):
        """Process PublicEvent (repository made public)"""
        # Additional processing for public events if needed
        pass
    
    async def get_processing_stats(self) -> Dict:
        """Get comprehensive processing statistics"""
        return {
            'events_processed': self.metrics.total_processed,
            'validation_errors': self.metrics.validation_errors,
            'database_errors': self.metrics.database_errors,
            'data_quality_score': self.metrics.data_quality_score,
            'event_types': dict(self.metrics.event_types),
            'processing_rate_per_second': self.metrics.processing_rate,
            'memory_usage_mb': self.metrics.memory_usage_mb,
            'uptime_seconds': time.time() - self.start_time if hasattr(self, 'start_time') else 0
        }
    
    async def cleanup(self):
        """Cleanup resources"""
        logger.info("🧹 Cleaning up enhanced processor...")
        
        if self.session:
            await self.session.close()
        
        if self.pool:
            await self.pool.close()
        
        logger.info("✅ Enhanced processor cleanup complete")

async def main():
    """Main entry point for enhanced GitHub event processing"""
    config = Config()
    processor = EnhancedGitHubEventProcessor(config)
    
    try:
        # Initialize the processor
        await processor.initialize()
        
        logger.info("🎯 Enhanced GitHub Event Processor Started")
        logger.info("Focus: Comprehensive capture of ALL GitHub event API data")
        
        # Example: Process recent archives
        base_url = "https://data.gharchive.org/"
        
        # Process the last few hours of data for testing
        now = datetime.utcnow()
        for hours_back in range(3):  # Last 3 hours
            target_time = now - timedelta(hours=hours_back)
            filename = f"{target_time.strftime('%Y-%m-%d')}-{target_time.hour}.json.gz"
            file_url = f"{base_url}{filename}"
            
            events_processed = await processor.process_archive_file(file_url)
            
            if processor.shutdown_flag:
                logger.info("⚠️ Shutdown signal received, stopping processing")
                break
        
        # Display final statistics
        stats = await processor.get_processing_stats()
        logger.info("📊 Final Processing Statistics:")
        logger.info(f"   Total Events Processed: {stats['events_processed']:,}")
        logger.info(f"   Data Quality Score: {stats['data_quality_score']:.2f}%")
        logger.info(f"   Processing Rate: {stats['processing_rate_per_second']:.2f} events/sec")
        logger.info(f"   Memory Usage: {stats['memory_usage_mb']:.2f} MB")
        logger.info(f"   Event Types Captured: {len(stats['event_types'])}")
        
        for event_type, count in sorted(stats['event_types'].items(), key=lambda x: x[1], reverse=True):
            logger.info(f"     {event_type}: {count:,} events")
        
    except KeyboardInterrupt:
        logger.info("⚠️ Received interrupt signal")
    except Exception as e:
        logger.error(f"❌ Processing failed: {e}")
        raise
    finally:
        await processor.cleanup()
        logger.info("🏁 Enhanced GitHub Event Processor Finished")

if __name__ == "__main__":
    asyncio.run(main())
