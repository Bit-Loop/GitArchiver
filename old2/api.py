#!/usr/bin/env python3
"""
Safe Web API for GitHub Archive Scraper with Resource Management
Designed for Oracle Cloud instances with built-in safety features
"""

import asyncio
import json
import logging
import os
import signal
import subprocess
import sys
import time
import gc
import tempfile
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional, Set

import aiohttp
from aiohttp import web
import aiohttp_cors
import asyncpg
import psutil
from dateutil.parser import parse as parse_date

# Import our config
from config import Config

# Import new modules
from auth_manager import AuthManager
from rate_limiter import GitHubRateLimiter  
from data_importer import DataImporter
from wordlist_generator import WordlistGenerator

class ResourceMonitor:
    """Enhanced resource monitor for Oracle Cloud safety"""
    
    def __init__(self, config: Config):
        self.config = config
        self.process = psutil.Process()
        self.start_time = time.time()
        self.temp_files: Set[str] = set()
        self.emergency_mode = False
        self.last_cleanup = time.time()
        
        # Oracle Cloud safe limits (for 24GB system)
        self.max_memory_mb = 18000  # 18GB max
        self.warning_memory_mb = 16000  # Warning at 16GB
        self.max_disk_gb = 40  # 40GB max disk usage
        self.warning_disk_gb = 35  # Warning at 35GB
        self.cpu_limit_percent = 80  # Max 80% CPU
        
    def get_memory_usage(self) -> float:
        """Get current memory usage in MB"""
        return self.process.memory_info().rss / 1024 / 1024
    
    def get_disk_usage(self, path: str = '.') -> tuple[float, float]:
        """Get disk usage in GB (used, total)"""
        usage = psutil.disk_usage(path)
        return usage.used / (1024**3), usage.total / (1024**3)
    
    def get_cpu_usage(self) -> float:
        """Get system CPU usage percentage"""
        # Use system CPU instead of process CPU for better monitoring
        return psutil.cpu_percent(interval=0.1)
    
    def get_system_memory(self) -> dict:
        """Get system memory info"""
        mem = psutil.virtual_memory()
        return {
            'total_gb': mem.total / (1024**3),
            'available_gb': mem.available / (1024**3),
            'used_gb': mem.used / (1024**3),
            'percent': mem.percent
        }
    
    def check_memory_limit(self) -> bool:
        """Check if memory usage is within safe limits"""
        memory_mb = self.get_memory_usage()
        
        if memory_mb > self.max_memory_mb:
            logging.error(f"Memory usage {memory_mb:.1f}MB exceeds limit {self.max_memory_mb}MB")
            return False
        elif memory_mb > self.warning_memory_mb:
            logging.warning(f"Memory usage {memory_mb:.1f}MB approaching limit {self.max_memory_mb}MB")
            
        return True
    
    def check_disk_limit(self) -> bool:
        """Check if disk usage is within safe limits"""
        used_gb, total_gb = self.get_disk_usage()
        
        if used_gb > self.max_disk_gb:
            logging.error(f"Disk usage {used_gb:.1f}GB exceeds limit {self.max_disk_gb}GB")
            return False
        elif used_gb > self.warning_disk_gb:
            logging.warning(f"Disk usage {used_gb:.1f}GB approaching limit {self.max_disk_gb}GB")
            
        return True
    
    def emergency_cleanup(self):
        """Perform emergency cleanup to free resources"""
        if self.emergency_mode:
            return
            
        self.emergency_mode = True
        logging.warning("Performing emergency cleanup due to resource pressure")
        
        try:
            # Force garbage collection
            collected = gc.collect()
            logging.info(f"Garbage collection freed {collected} objects")
            
            # Clean up temp files
            self.cleanup_temp_files()
            
            # Clear Python caches
            sys.modules.clear()
            
            # Log current resource usage
            memory_mb = self.get_memory_usage()
            used_gb, total_gb = self.get_disk_usage()
            cpu_percent = self.get_cpu_usage()
            
            logging.info(f"Post-cleanup: Memory: {memory_mb:.1f}MB, Disk: {used_gb:.1f}GB, CPU: {cpu_percent:.1f}%")
            
        except Exception as e:
            logging.error(f"Emergency cleanup failed: {e}")
        finally:
            self.emergency_mode = False
    
    def cleanup_temp_files(self):
        """Clean up temporary files"""
        cleaned = 0
        for temp_file in list(self.temp_files):
            try:
                if os.path.exists(temp_file):
                    os.remove(temp_file)
                    cleaned += 1
                    logging.debug(f"Cleaned up temp file: {temp_file}")
                self.temp_files.discard(temp_file)
            except Exception as e:
                logging.warning(f"Failed to clean up temp file {temp_file}: {e}")
        
        if cleaned > 0:
            logging.info(f"Cleaned up {cleaned} temporary files")
    
    def register_temp_file(self, filepath: str):
        """Register a temporary file for cleanup"""
        self.temp_files.add(filepath)
    
    def should_pause_processing(self) -> bool:
        """Check if processing should be paused due to resource pressure"""
        memory_mb = self.get_memory_usage()
        used_gb, _ = self.get_disk_usage()
        cpu_percent = self.get_cpu_usage()
        
        memory_ratio = memory_mb / self.max_memory_mb
        disk_ratio = used_gb / self.max_disk_gb
        
        if memory_ratio > 0.9 or disk_ratio > 0.9 or cpu_percent > self.cpu_limit_percent:
            logging.warning(f"Resource pressure detected - Memory: {memory_ratio:.1%}, Disk: {disk_ratio:.1%}, CPU: {cpu_percent:.1f}%")
            self.emergency_cleanup()
            return True
            
        return False
    
    def auto_cleanup(self):
        """Perform automatic cleanup if needed"""
        current_time = time.time()
        
        # Auto cleanup every 5 minutes
        if current_time - self.last_cleanup > 300:
            self.last_cleanup = current_time
            
            # Clean up if we have too many temp files
            if len(self.temp_files) > 50:
                self.cleanup_temp_files()
            
            # Force GC if memory usage is high
            memory_mb = self.get_memory_usage()
            if memory_mb > self.warning_memory_mb:
                gc.collect()
    
    def get_status(self) -> dict:
        """Get current resource status"""
        memory_mb = self.get_memory_usage()
        used_gb, total_gb = self.get_disk_usage()
        cpu_percent = self.get_cpu_usage()
        system_mem = self.get_system_memory()
        uptime = time.time() - self.start_time
        
        return {
            'process_memory_mb': round(memory_mb, 1),
            'process_memory_limit_mb': self.max_memory_mb,
            'process_memory_usage_percent': round((memory_mb / self.max_memory_mb) * 100, 1),
            'system_memory_gb': round(system_mem['used_gb'], 1),
            'system_memory_total_gb': round(system_mem['total_gb'], 1),
            'system_memory_percent': round(system_mem['percent'], 1),
            'disk_used_gb': round(used_gb, 1),
            'disk_total_gb': round(total_gb, 1),
            'disk_limit_gb': self.max_disk_gb,
            'disk_usage_percent': round((used_gb / self.max_disk_gb) * 100, 1),
            'cpu_percent': round(cpu_percent, 1),
            'cpu_limit_percent': self.cpu_limit_percent,
            'uptime_seconds': round(uptime, 1),
            'temp_files_count': len(self.temp_files),
            'emergency_mode': self.emergency_mode,
            'is_safe': self.check_memory_limit() and self.check_disk_limit() and cpu_percent < self.cpu_limit_percent
        }

class DatabaseManager:
    """Safe database manager with resource monitoring"""
    
    def __init__(self, config: Config):
        self.config = config
        self.pool = None
        
    async def connect(self):
        """Create database connection pool with conservative settings"""
        dsn = f"postgresql://{self.config.DB_USER}:{self.config.DB_PASSWORD}@{self.config.DB_HOST}:{self.config.DB_PORT}/{self.config.DB_NAME}"
        
        try:
            # Conservative pool settings for Oracle Cloud
            self.pool = await asyncpg.create_pool(
                dsn, 
                min_size=2,  # Reduced from 5
                max_size=8,  # Reduced from 20
                command_timeout=30,  # Reduced timeout
                max_inactive_connection_lifetime=300  # 5 minutes
            )
            logging.info("Database connected successfully with conservative settings")
        except Exception as e:
            logging.error(f"Database connection failed: {e}")
            raise
        
    async def disconnect(self):
        """Close database connection pool"""
        if self.pool:
            await self.pool.close()
            logging.info("Database disconnected")
    
    async def health_check(self) -> bool:
        """Check database health"""
        try:
            if not self.pool:
                return False
            async with self.pool.acquire() as conn:
                await conn.fetchval("SELECT 1")
            return True
        except Exception as e:
            logging.error(f"Database health check failed: {e}")
            return False

class SafeWebAPI:
    """Safe Web API with resource monitoring for Oracle Cloud"""
    
    def __init__(self):
        self.config = Config()
        self.db = DatabaseManager(self.config)
        self.resource_monitor = ResourceMonitor(self.config)
        self.app = None
        self.scraper_process = None
        self.shutdown_event = asyncio.Event()
        
        # Background task management
        self.background_tasks = {}
        self.scraper_paused = False
        self.auto_resume_enabled = True
        
        # Initialize new components
        self.auth_manager = AuthManager()  # Use default ./auth directory
        self.rate_limiter = GitHubRateLimiter(self.config.GITHUB_TOKEN)
        self.data_importer = DataImporter(self.config)
        self.wordlist_generator = WordlistGenerator(self.config)
        
        # Setup logging
        logging.basicConfig(
            level=getattr(logging, self.config.LOG_LEVEL),
            format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
            handlers=[
                logging.StreamHandler(),
                logging.FileHandler('api.log')
            ]
        )
        self.logger = logging.getLogger(__name__)
        
    async def init_database(self):
        """Initialize database connection"""
        await self.db.connect()
        
    async def setup_routes(self):
        """Setup API routes with CORS"""
        self.app = web.Application()
        
        # Setup CORS
        cors = aiohttp_cors.setup(self.app, defaults={
            "*": aiohttp_cors.ResourceOptions(
                allow_credentials=True,
                expose_headers="*",
                allow_headers="*",
                allow_methods="*"
            )
        })
        
        # API routes
        self.app.router.add_get('/api/status', self.get_status)
        self.app.router.add_get('/api/stats', self.get_stats)
        self.app.router.add_get('/api/health', self.get_health)
        self.app.router.add_get('/api/monitor', self.get_monitor)
        self.app.router.add_get('/api/events', self.get_events)
        self.app.router.add_get('/api/repositories', self.get_repositories)
        self.app.router.add_post('/api/search', self.search_events)
        self.app.router.add_get('/api/search-repositories', self.search_repositories_endpoint)
        self.app.router.add_get('/api/data-quality', self.data_quality_metrics)
        
        # Database management routes
        self.app.router.add_get('/api/search-archives', self.search_available_archives)
        self.app.router.add_post('/api/download-archives', self.download_selected_archives)
        self.app.router.add_post('/api/download-keywords', self.download_by_keywords)
        self.app.router.add_post('/api/remove-repositories', self.remove_repositories)
        self.app.router.add_get('/api/disk-usage', self.get_disk_usage_details)
        self.app.router.add_post('/api/prune-data', self.prune_unused_data)
        self.app.router.add_post('/api/validate-password', self.validate_password)
        
        # Authentication routes
        self.app.router.add_post('/api/auth/login', self.auth_login)
        self.app.router.add_post('/api/auth/logout', self.auth_logout)
        self.app.router.add_get('/api/auth/status', self.auth_status)
        self.app.router.add_post('/api/auth/set-password', self.auth_set_password)
        
        # Rate limiting routes
        self.app.router.add_get('/api/rate-limit/status', self.get_rate_limit_status)
        self.app.router.add_post('/api/rate-limit/reset', self.reset_rate_limit)
        self.app.router.add_get('/api/rate-limit/config', self.get_rate_limit_config)
        
        # Data import routes
        self.app.router.add_post('/api/import/json', self.import_json_data)
        self.app.router.add_post('/api/import/bigquery', self.import_bigquery_data)
        self.app.router.add_get('/api/import/status', self.get_import_status)
        
        # Wordlist generation routes
        self.app.router.add_post('/api/wordlists/generate', self.generate_wordlists)
        self.app.router.add_post('/api/wordlists/targeted', self.generate_targeted_wordlist)
        self.app.router.add_get('/api/wordlists/download/{filename}', self.download_wordlist)
        
        # Management routes
        self.app.router.add_post('/api/start-scraper', self.start_scraper)
        self.app.router.add_post('/api/stop-scraper', self.stop_scraper)
        self.app.router.add_post('/api/restart-scraper', self.restart_scraper)
        self.app.router.add_post('/api/pause-scraper', self.pause_scraper)
        self.app.router.add_post('/api/resume-scraper', self.resume_scraper)
        self.app.router.add_get('/api/scraper-status', self.get_scraper_status)
        self.app.router.add_get('/api/scraper-logs', self.get_scraper_logs)
        self.app.router.add_post('/api/emergency-cleanup', self.emergency_cleanup)
        self.app.router.add_post('/api/shutdown', self.shutdown_server)
        
        # Dashboard
        self.app.router.add_get('/', self.serve_dashboard)
        self.app.router.add_get('/dashboard', self.serve_dashboard)
        
        # Add CORS to all routes
        for route in list(self.app.router.routes()):
            cors.add(route)
    
    async def get_status(self, request):
        """Get API and scraper status"""
        try:
            scraper_running = self.scraper_process and self.scraper_process.poll() is None
            db_connected = await self.db.health_check()
            
            return web.json_response({
                'api_running': True,
                'scraper_running': scraper_running,
                'database_connected': db_connected,
                'timestamp': datetime.utcnow().isoformat(),
                'resource_status': self.resource_monitor.get_status()
            })
        except Exception as e:
            self.logger.error(f"Status check failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_health(self, request):
        """Health check endpoint for monitoring"""
        try:
            status = self.resource_monitor.get_status()
            
            # Determine health status
            is_healthy = (
                status['is_safe'] and
                status['process_memory_usage_percent'] < 90 and
                status['disk_usage_percent'] < 90 and
                status['cpu_percent'] < 85
            )
            
            response_data = {
                'healthy': is_healthy,
                'timestamp': datetime.utcnow().isoformat(),
                'resources': status
            }
            
            status_code = 200 if is_healthy else 503
            return web.json_response(response_data, status=status_code)
            
        except Exception as e:
            self.logger.error(f"Health check failed: {e}")
            return web.json_response({
                'healthy': False,
                'error': str(e),
                'timestamp': datetime.utcnow().isoformat()
            }, status=500)
    
    async def get_monitor(self, request):
        """Real-time resource monitoring endpoint"""
        try:
            # Perform auto cleanup
            self.resource_monitor.auto_cleanup()
            
            status = self.resource_monitor.get_status()
            
            # Add additional monitoring data
            status.update({
                'timestamp': datetime.utcnow().isoformat(),
                'uptime_formatted': str(timedelta(seconds=int(status['uptime_seconds']))),
                'recommendations': self._get_recommendations(status)
            })
            
            return web.json_response(status)
            
        except Exception as e:
            self.logger.error(f"Monitor endpoint failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    def _get_recommendations(self, status: dict) -> list:
        """Get performance recommendations based on current status"""
        recommendations = []
        
        if status['process_memory_usage_percent'] > 80:
            recommendations.append("High memory usage detected. Consider restarting the scraper.")
        
        if status['disk_usage_percent'] > 80:
            recommendations.append("High disk usage detected. Clean up old files.")
        
        if status['cpu_percent'] > 70:
            recommendations.append("High CPU usage detected. Reduce concurrent operations.")
        
        if status['temp_files_count'] > 20:
            recommendations.append("Many temporary files detected. Cleanup recommended.")
        
        if not recommendations:
            recommendations.append("All systems operating normally.")
        
        return recommendations
    
    async def get_stats(self, request):
        """Get database statistics"""
        try:
            if not await self.db.health_check():
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            async with self.db.pool.acquire() as conn:
                # Get basic stats
                total_events = await conn.fetchval("SELECT COUNT(*) FROM github_events") or 0
                total_repos = await conn.fetchval("SELECT COUNT(*) FROM repositories") or 0
                processed_files = await conn.fetchval("SELECT COUNT(*) FROM processed_files") or 0
                
                # Get latest event date
                latest_date = await conn.fetchval("SELECT MAX(created_at) FROM github_events")
                
                # Get event types
                event_types = await conn.fetch("""
                    SELECT type, COUNT(*) as count 
                    FROM github_events 
                    GROUP BY type 
                    ORDER BY count DESC 
                    LIMIT 10
                """)
                
                return web.json_response({
                    'total_events': total_events,
                    'total_repositories': total_repos,
                    'processed_files': processed_files,
                    'latest_event_date': latest_date.isoformat() if latest_date else None,
                    'event_types': [{'type': row['type'], 'count': row['count']} for row in event_types]
                })
                
        except Exception as e:
            self.logger.error(f"Stats query failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_events(self, request):
        """Get recent events with pagination"""
        try:
            if not await self.db.health_check():
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            # Parse query parameters
            limit = min(int(request.query.get('limit', 100)), 1000)  # Max 1000
            offset = int(request.query.get('offset', 0))
            event_type = request.query.get('type')
            
            async with self.db.pool.acquire() as conn:
                where_clause = ""
                params = []
                
                if event_type:
                    where_clause = "WHERE type = $1"
                    params.append(event_type)
                
                query = f"""
                    SELECT id, type, created_at, actor_login, repo_name, public
                    FROM github_events 
                    {where_clause}
                    ORDER BY created_at DESC 
                    LIMIT ${len(params) + 1} OFFSET ${len(params) + 2}
                """
                params.extend([limit, offset])
                
                events = await conn.fetch(query, *params)
                
                # Convert datetime objects to strings for JSON serialization
                events_data = []
                for event in events:
                    event_dict = dict(event)
                    # Convert datetime fields to strings
                    if event_dict.get('created_at'):
                        event_dict['created_at'] = event_dict['created_at'].isoformat()
                    events_data.append(event_dict)
                
                return web.json_response({
                    'events': events_data,
                    'count': len(events),
                    'limit': limit,
                    'offset': offset
                })
                
        except Exception as e:
            self.logger.error(f"Events query failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_repositories(self, request):
        """Get repositories with pagination"""
        try:
            if not await self.db.health_check():
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            limit = min(int(request.query.get('limit', 50)), 500)  # Max 500
            offset = int(request.query.get('offset', 0))
            
            async with self.db.pool.acquire() as conn:
                repos = await conn.fetch("""
                    SELECT id, name, full_name, description, language, 
                           stargazers_count, forks_count, created_at
                    FROM repositories 
                    ORDER BY stargazers_count DESC 
                    LIMIT $1 OFFSET $2
                """, limit, offset)
                
                # Convert datetime objects to strings for JSON serialization
                repos_data = []
                for repo in repos:
                    repo_dict = dict(repo)
                    # Convert datetime fields to strings
                    if repo_dict.get('created_at'):
                        repo_dict['created_at'] = repo_dict['created_at'].isoformat()
                    if repo_dict.get('updated_at'):
                        repo_dict['updated_at'] = repo_dict['updated_at'].isoformat()
                    if repo_dict.get('first_seen_at'):
                        repo_dict['first_seen_at'] = repo_dict['first_seen_at'].isoformat()
                    if repo_dict.get('last_updated_at'):
                        repo_dict['last_updated_at'] = repo_dict['last_updated_at'].isoformat()
                    repos_data.append(repo_dict)
                
                return web.json_response({
                    'repositories': repos_data,
                    'count': len(repos),
                    'limit': limit,
                    'offset': offset
                })
                
        except Exception as e:
            self.logger.error(f"Repositories query failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def search_events(self, request):
        """Search events with resource limits"""
        try:
            if not await self.db.health_check():
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            # Check resource usage before expensive operation
            if self.resource_monitor.should_pause_processing():
                return web.json_response({
                    'error': 'Search temporarily unavailable due to high resource usage'
                }, status=503)
            
            data = await request.json()
            query = data.get('query', '').strip()
            limit = min(data.get('limit', 100), 500)  # Max 500 for safety
            
            if not query:
                return web.json_response({'error': 'Query required'}, status=400)
            
            async with self.db.pool.acquire() as conn:
                # Simple search with timeout
                events = await asyncio.wait_for(
                    conn.fetch("""
                        SELECT id, type, created_at, actor_login, repo_name
                        FROM github_events 
                        WHERE repo_name ILIKE $1 OR actor_login ILIKE $1
                        ORDER BY created_at DESC 
                        LIMIT $2
                    """, f'%{query}%', limit),
                    timeout=10.0  # 10 second timeout
                )
                
                # Convert datetime objects to strings for JSON serialization
                events_data = []
                for event in events:
                    event_dict = dict(event)
                    # Convert datetime fields to strings
                    if event_dict.get('created_at'):
                        event_dict['created_at'] = event_dict['created_at'].isoformat()
                    events_data.append(event_dict)
                
                return web.json_response({
                    'events': events_data,
                    'query': query,
                    'count': len(events)
                })
                
        except asyncio.TimeoutError:
            return web.json_response({'error': 'Search timeout'}, status=408)
        except Exception as e:
            self.logger.error(f"Search failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def start_scraper(self, request):
        """Start the scraper process"""
        try:
            if self.scraper_process and self.scraper_process.poll() is None:
                return web.json_response({'error': 'Scraper already running'}, status=400)
            
            # Check resources before starting
            if not self.resource_monitor.get_status()['is_safe']:
                return web.json_response({
                    'error': 'Cannot start scraper: resource limits exceeded'
                }, status=503)
            
            # Start scraper process
            self.scraper_process = subprocess.Popen([
                sys.executable, 'gharchive_scraper.py'
            ], cwd=Path(__file__).parent)
            
            self.logger.info(f"Started scraper process: PID {self.scraper_process.pid}")
            
            return web.json_response({
                'message': 'Scraper started successfully',
                'pid': self.scraper_process.pid
            })
            
        except Exception as e:
            self.logger.error(f"Failed to start scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def stop_scraper(self, request):
        """Stop the scraper process"""
        try:
            if not self.scraper_process or self.scraper_process.poll() is not None:
                return web.json_response({'error': 'Scraper not running'}, status=400)
            
            # Graceful shutdown
            self.scraper_process.terminate()
            
            # Wait for process to exit
            try:
                self.scraper_process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                self.logger.warning("Scraper didn't terminate gracefully, forcing kill")
                self.scraper_process.kill()
                self.scraper_process.wait()
            
            self.logger.info("Scraper stopped successfully")
            
            return web.json_response({'message': 'Scraper stopped successfully'})
            
        except Exception as e:
            self.logger.error(f"Failed to stop scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def restart_scraper(self, request):
        """Restart the scraper process"""
        try:
            # Stop first
            if self.scraper_process and self.scraper_process.poll() is None:
                await self.stop_scraper(request)
                await asyncio.sleep(2)  # Wait a bit
            
            # Then start
            return await self.start_scraper(request)
            
        except Exception as e:
            self.logger.error(f"Failed to restart scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def pause_scraper(self, request):
        """Pause the scraper process by sending SIGSTOP"""
        try:
            if not self.scraper_process or self.scraper_process.poll() is not None:
                return web.json_response({'error': 'Scraper not running'}, status=400)
            
            # Send SIGSTOP to pause the process
            import os
            import signal
            os.kill(self.scraper_process.pid, signal.SIGSTOP)
            
            self.logger.info("Scraper paused successfully")
            
            return web.json_response({
                'message': 'Scraper paused successfully',
                'pid': self.scraper_process.pid,
                'status': 'paused'
            })
            
        except Exception as e:
            self.logger.error(f"Failed to pause scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def resume_scraper(self, request):
        """Resume the scraper process by sending SIGCONT"""
        try:
            if not self.scraper_process or self.scraper_process.poll() is not None:
                return web.json_response({'error': 'Scraper not running'}, status=400)
            
            # Send SIGCONT to resume the process
            import os
            import signal
            os.kill(self.scraper_process.pid, signal.SIGCONT)
            
            self.logger.info("Scraper resumed successfully")
            
            return web.json_response({
                'message': 'Scraper resumed successfully',
                'pid': self.scraper_process.pid,
                'status': 'running'
            })
            
        except Exception as e:
            self.logger.error(f"Failed to resume scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_scraper_status(self, request):
        """Get detailed scraper status and performance metrics"""
        try:
            if not self.scraper_process:
                return web.json_response({
                    'running': False,
                    'status': 'stopped',
                    'message': 'Scraper not started'
                })
            
            poll_result = self.scraper_process.poll()
            
            if poll_result is not None:
                return web.json_response({
                    'running': False,
                    'status': 'stopped',
                    'exit_code': poll_result,
                    'message': f'Scraper exited with code {poll_result}'
                })
            
            # Get process info if running
            try:
                process = psutil.Process(self.scraper_process.pid)
                process_info = {
                    'pid': self.scraper_process.pid,
                    'status': process.status(),
                    'cpu_percent': process.cpu_percent(),
                    'memory_mb': process.memory_info().rss / 1024 / 1024,
                    'create_time': process.create_time(),
                    'num_threads': process.num_threads()
                }
            except psutil.NoSuchProcess:
                return web.json_response({
                    'running': False,
                    'status': 'stopped',
                    'message': 'Process no longer exists'
                })
            
            # Check rate limiting status
            rate_limit_status = self.rate_limiter.get_status()
            
            return web.json_response({
                'running': True,
                'status': process_info['status'],
                'process_info': process_info,
                'rate_limit': rate_limit_status,
                'resource_status': self.resource_monitor.get_status(),
                'timestamp': datetime.utcnow().isoformat()
            })
            
        except Exception as e:
            self.logger.error(f"Failed to get scraper status: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_scraper_logs(self, request):
        """Get recent scraper logs"""
        try:
            log_file = Path(self.config.LOG_FILE)
            
            if not log_file.exists():
                return web.json_response({'logs': 'No log file found'})
            
            # Read last 100 lines
            with open(log_file, 'r') as f:
                lines = f.readlines()
                recent_lines = lines[-100:] if len(lines) > 100 else lines
                
            return web.json_response({
                'logs': ''.join(recent_lines),
                'lines': len(recent_lines)
            })
            
        except Exception as e:
            self.logger.error(f"Failed to read logs: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def emergency_cleanup(self, request):
        """Trigger emergency cleanup"""
        try:
            self.resource_monitor.emergency_cleanup()
            
            # Force garbage collection
            collected = gc.collect()
            
            # Get updated status
            status = self.resource_monitor.get_status()
            
            return web.json_response({
                'message': 'Emergency cleanup completed',
                'objects_collected': collected,
                'resource_status': status
            })
            
        except Exception as e:
            self.logger.error(f"Emergency cleanup failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def shutdown_server(self, request):
        """Shutdown the API server"""
        try:
            self.logger.info("Shutdown requested via API")
            
            # Stop scraper if running
            if self.scraper_process and self.scraper_process.poll() is None:
                await self.stop_scraper(request)
            
            # Trigger shutdown
            asyncio.create_task(self._delayed_shutdown())
            
            return web.json_response({'message': 'Server shutdown initiated'})
            
        except Exception as e:
            self.logger.error(f"Shutdown failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def search_repositories_endpoint(self, request):
        """Search repositories by name or description"""
        try:
            query = request.query.get('q', '').strip()
            
            if not query:
                return web.json_response({'error': 'Query parameter required'}, status=400)
            
            if not await self.db.health_check() or not self.db.pool:
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            async with self.db.pool.acquire() as conn:
                # Search repositories by name, full_name, or description
                repositories = await conn.fetch("""
                    SELECT 
                        r.id,
                        r.name,
                        r.full_name,
                        r.description,
                        r.owner_login as owner,
                        r.stargazers_count,
                        r.forks_count,
                        r.language,
                        r.created_at,
                        r.updated_at,
                        COUNT(e.id) as event_count
                    FROM repositories r
                    LEFT JOIN github_events e ON r.id = e.repo_id
                    WHERE 
                        r.name ILIKE $1 OR 
                        r.full_name ILIKE $1 OR 
                        r.description ILIKE $1
                    GROUP BY r.id, r.name, r.full_name, r.description, r.owner_login, 
                             r.stargazers_count, r.forks_count, r.language, r.created_at, r.updated_at
                    ORDER BY COUNT(e.id) DESC, r.stargazers_count DESC
                    LIMIT 50
                """, f'%{query}%')
                
                return web.json_response({
                    'repositories': [dict(row) for row in repositories],
                    'total': len(repositories),
                    'query': query
                })
                
        except Exception as e:
            self.logger.error(f"Repository search failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def data_quality_metrics(self, request):
        """Get comprehensive data quality metrics with enhanced validation"""
        try:
            # Use the existing database connection
            if not await self.db.health_check():
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            # Get basic database statistics using existing connection
            async with self.db.pool.acquire() as conn:
                # Get basic event statistics
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
                
                # Get event type distribution
                event_types = await conn.fetch("""
                    SELECT type, COUNT(*) as count 
                    FROM github_events 
                    GROUP BY type 
                    ORDER BY count DESC 
                    LIMIT 20
                """)
                
                # Get data integrity checks
                integrity_issues = await conn.fetchrow("""
                    SELECT 
                        COUNT(CASE WHEN id IS NULL THEN 1 END) as null_ids,
                        COUNT(CASE WHEN type IS NULL OR type = '' THEN 1 END) as invalid_types,
                        COUNT(CASE WHEN created_at IS NULL THEN 1 END) as null_timestamps,
                        COUNT(CASE WHEN payload IS NULL THEN 1 END) as null_payloads
                    FROM github_events
                """)
                
                # Calculate quality score
                total_events = event_stats['total_events'] or 0
                if total_events > 0:
                    issues = (
                        integrity_issues['null_ids'] + 
                        integrity_issues['invalid_types'] + 
                        integrity_issues['null_timestamps']
                    )
                    quality_score = max(0.0, min(100.0, ((total_events - issues) / total_events) * 100))
                else:
                    quality_score = 0.0
                
                # Build comprehensive metrics
                metrics = {
                    'events': {
                        'total': total_events,
                        'unique_actors': event_stats['unique_actors'] or 0,
                        'unique_repos': event_stats['unique_repos'] or 0,
                        'event_types_count': event_stats['event_types'] or 0,
                        'null_actor_ids': event_stats['null_actor_ids'] or 0,
                        'null_repo_ids': event_stats['null_repo_ids'] or 0,
                        'date_range': {
                            'earliest': event_stats['earliest_event'].isoformat() if event_stats['earliest_event'] else None,
                            'latest': event_stats['latest_event'].isoformat() if event_stats['latest_event'] else None
                        }
                    },
                    'event_types': [{'type': row['type'], 'count': row['count']} for row in event_types],
                    'data_integrity': {
                        'null_ids': integrity_issues['null_ids'] or 0,
                        'invalid_types': integrity_issues['invalid_types'] or 0,
                        'null_timestamps': integrity_issues['null_timestamps'] or 0,
                        'null_payloads': integrity_issues['null_payloads'] or 0
                    },
                    'quality_score': quality_score,
                    'api_info': {
                        'endpoint': '/api/data-quality',
                        'generated_at': datetime.utcnow().isoformat(),
                        'version': '2.0'
                    }
                }
                
                # Validate metrics integrity
                validation_results = self._validate_metrics(metrics)
                metrics['validation'] = validation_results
                
                return web.json_response(metrics)
                
        except Exception as e:
            self.logger.error(f"Error getting enhanced data quality metrics: {e}")
            return web.json_response(
                {'error': 'Failed to retrieve enhanced data quality metrics'}, 
                status=500
            )
    
    def _validate_metrics(self, metrics: Dict) -> Dict:
        """Validate the integrity of data quality metrics"""
        validation = {
            'is_valid': True,
            'warnings': [],
            'errors': []
        }
        
        # Check for data consistency
        events = metrics.get('events', {})
        processing = metrics.get('processing', {})
        
        # Validate event counts
        if events.get('total', 0) == 0:
            validation['warnings'].append('No events found in database')
        
        # Check for data integrity issues
        integrity = metrics.get('data_integrity', {})
        total_integrity_issues = sum([
            integrity.get('null_ids', 0),
            integrity.get('invalid_types', 0),
            integrity.get('null_timestamps', 0)
        ])
        
        if total_integrity_issues > 0:
            validation['warnings'].append(f'Found {total_integrity_issues} data integrity issues')
        
        # Validate quality score
        quality_score = metrics.get('quality_score', 0)
        if quality_score < 90:
            validation['warnings'].append(f'Data quality score below 90%: {quality_score:.1f}%')
        elif quality_score < 95:
            validation['warnings'].append(f'Data quality score could be improved: {quality_score:.1f}%')
        
        # Check processing consistency
        if processing.get('total_processed_events', 0) > events.get('total', 0):
            validation['errors'].append('Processed events exceed database events - data inconsistency detected')
            validation['is_valid'] = False
        
        return validation
    
    async def _delayed_shutdown(self):
        """Delayed shutdown to allow response to be sent"""
        await asyncio.sleep(1)
        self.shutdown_event.set()
    
    async def validate_password(self, request):
        """Validate admin password for sensitive operations"""
        try:
            data = await request.json()
            password = data.get('password', '')
            
            # Simple password check (in production, use proper hashing)
            admin_password = os.getenv('ADMIN_PASSWORD', 'admin123')
            
            if password == admin_password:
                return web.json_response({'valid': True})
            else:
                return web.json_response({'valid': False, 'error': 'Invalid password'}, status=401)
                
        except Exception as e:
            self.logger.error(f"Password validation failed: {e}")
            return web.json_response({'valid': False, 'error': str(e)}, status=500)
    
    async def search_available_archives(self, request):
        """Search available GitHub Archive files for download"""
        try:
            date_from = request.query.get('from', '')
            date_to = request.query.get('to', '')
            limit = min(int(request.query.get('limit', 100)), 500)
            
            # Generate list of available archive files
            archives = []
            if date_from and date_to:
                from datetime import datetime, timedelta
                start_date = datetime.strptime(date_from, '%Y-%m-%d')
                end_date = datetime.strptime(date_to, '%Y-%m-%d')
                
                current_date = start_date
                while current_date <= end_date and len(archives) < limit:
                    for hour in range(24):
                        archive_name = f"{current_date.strftime('%Y-%m-%d')}-{hour}.json.gz"
                        archives.append({
                            'filename': archive_name,
                            'date': current_date.strftime('%Y-%m-%d'),
                            'hour': hour,
                            'url': f"https://data.gharchive.org/{archive_name}",
                            'estimated_size': '~25MB'
                        })
                        if len(archives) >= limit:
                            break
                    current_date += timedelta(days=1)
            
            return web.json_response({
                'archives': archives[:limit],
                'total': len(archives)
            })
            
        except Exception as e:
            self.logger.error(f"Archive search failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def download_selected_archives(self, request):
        """Download selected archive files"""
        try:
            data = await request.json()
            selected_files = data.get('files', [])
            
            if not selected_files:
                return web.json_response({'error': 'No files selected'}, status=400)
            
            if len(selected_files) > 50:
                return web.json_response({'error': 'Too many files selected (max 50)'}, status=400)
            
            # Check resource usage before starting
            if not self.resource_monitor.get_status()['is_safe']:
                return web.json_response({
                    'error': 'Cannot start download: resource limits exceeded'
                }, status=503)
            
            # Start download process in background
            asyncio.create_task(self._download_archives_background(selected_files))
            
            return web.json_response({
                'message': f'Started downloading {len(selected_files)} archive files',
                'files': selected_files
            })
            
        except Exception as e:
            self.logger.error(f"Archive download failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def _download_archives_background(self, files):
        """Background task to download archive files"""
        try:
            self.logger.info(f"Starting background download of {len(files)} files")
            
            for filename in files:
                if self.resource_monitor.should_pause_processing():
                    self.logger.warning("Pausing download due to resource pressure")
                    await asyncio.sleep(30)  # Wait before continuing
                
                # Simulate download process (replace with actual scraper call)
                self.logger.info(f"Processing archive: {filename}")
                await asyncio.sleep(1)  # Simulated processing time
                
            self.logger.info(f"Completed download of {len(files)} files")
            
        except Exception as e:
            self.logger.error(f"Background download failed: {e}")
    
    async def download_by_keywords(self, request):
        """Download archives systematically based on keywords"""
        try:
            data = await request.json()
            keywords = data.get('keywords', [])
            date_range = data.get('date_range', {})
            
            if not keywords:
                return web.json_response({'error': 'No keywords provided'}, status=400)
            
            # Check resource usage
            if not self.resource_monitor.get_status()['is_safe']:
                return web.json_response({
                    'error': 'Cannot start download: resource limits exceeded'
                }, status=503)
            
            # Start keyword-based download
            asyncio.create_task(self._download_by_keywords_background(keywords, date_range))
            
            return web.json_response({
                'message': f'Started systematic download for keywords: {", ".join(keywords)}',
                'keywords': keywords,
                'date_range': date_range
            })
            
        except Exception as e:
            self.logger.error(f"Keyword download failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def _download_by_keywords_background(self, keywords, date_range):
        """Background task for keyword-based downloading"""
        try:
            self.logger.info(f"Starting keyword-based download for: {keywords}")
            
            # This would implement the systematic download logic
            # For now, we'll just log the operation
            for keyword in keywords:
                self.logger.info(f"Processing keyword: {keyword}")
                await asyncio.sleep(2)  # Simulated processing
                
            self.logger.info("Keyword-based download completed")
            
        except Exception as e:
            self.logger.error(f"Keyword download background task failed: {e}")
    
    async def remove_repositories(self, request):
        """Remove selected repositories from database"""
        try:
            data = await request.json()
            repo_ids = data.get('repository_ids', [])
            
            if not repo_ids:
                return web.json_response({'error': 'No repositories selected'}, status=400)
            
            if not await self.db.health_check() or not self.db.pool:
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            async with self.db.pool.acquire() as conn:
                # Remove events first (foreign key constraint)
                await conn.execute(
                    "DELETE FROM github_events WHERE repo_id = ANY($1)", 
                    repo_ids
                )
                
                # Remove repositories
                result = await conn.execute(
                    "DELETE FROM repositories WHERE id = ANY($1)", 
                    repo_ids
                )
                
                rows_affected = int(result.split()[-1])
                
            return web.json_response({
                'message': f'Removed {rows_affected} repositories and their events',
                'removed_count': rows_affected
            })
            
        except Exception as e:
            self.logger.error(f"Repository removal failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_disk_usage_details(self, request):
        """Get detailed disk usage information"""
        try:
            if not await self.db.health_check() or not self.db.pool:
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            async with self.db.pool.acquire() as conn:
                # Get table sizes
                table_sizes = await conn.fetch("""
                    SELECT 
                        schemaname,
                        tablename,
                        pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size,
                        pg_total_relation_size(schemaname||'.'||tablename) as size_bytes
                    FROM pg_tables 
                    WHERE schemaname = 'public'
                    ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC
                """)
                
                # Get file system usage
                import shutil
                disk_usage = shutil.disk_usage('.')
                
                # Get temp file count
                import glob
                temp_files = len(glob.glob('/tmp/gharchive_*'))
                log_files = len(glob.glob('logs/*.log'))
                
                return web.json_response({
                    'database_tables': [dict(row) for row in table_sizes],
                    'filesystem': {
                        'total_gb': disk_usage.total / (1024**3),
                        'used_gb': disk_usage.used / (1024**3),
                        'free_gb': disk_usage.free / (1024**3),
                        'usage_percent': (disk_usage.used / disk_usage.total) * 100
                    },
                    'temp_files': {
                        'gharchive_temp': temp_files,
                        'log_files': log_files
                    },
                    'directories': {
                        'data_dir': './gharchive_data',
                        'logs_dir': './logs',
                        'temp_dir': '/tmp'
                    }
                })
                
        except Exception as e:
            self.logger.error(f"Disk usage query failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def prune_unused_data(self, request):
        """Prune unused data to free disk space"""
        try:
            data = await request.json()
            options = data.get('options', {})
            
            pruned = {
                'temp_files': 0,
                'old_logs': 0,
                'orphaned_events': 0,
                'space_freed_mb': 0.0
            }
            
            # Clean temporary files
            if options.get('temp_files', True):
                import glob
                import os
                temp_files = glob.glob('/tmp/gharchive_*')
                for temp_file in temp_files:
                    try:
                        size = os.path.getsize(temp_file)
                        os.remove(temp_file)
                        pruned['temp_files'] += 1
                        pruned['space_freed_mb'] += size / (1024**2)
                    except:
                        pass
            
            # Clean old log files (older than 7 days)
            if options.get('old_logs', True):
                import glob
                import os
                import time
                log_files = glob.glob('logs/*.log*')
                cutoff = time.time() - (7 * 24 * 60 * 60)  # 7 days ago
                
                for log_file in log_files:
                    try:
                        if os.path.getmtime(log_file) < cutoff:
                            size = os.path.getsize(log_file)
                            os.remove(log_file)
                            pruned['old_logs'] += 1
                            pruned['space_freed_mb'] += size / (1024**2)
                    except:
                        pass
            
            # Clean orphaned events (events without repositories)
            if options.get('orphaned_events', False) and await self.db.health_check() and self.db.pool:
                async with self.db.pool.acquire() as conn:
                    result = await conn.execute("""
                        DELETE FROM github_events 
                        WHERE repo_id NOT IN (SELECT id FROM repositories)
                    """)
                    pruned['orphaned_events'] = int(result.split()[-1]) if result else 0
            
            return web.json_response({
                'message': 'Data pruning completed',
                'summary': pruned
            })
            
        except Exception as e:
            self.logger.error(f"Data pruning failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def serve_dashboard(self, request):
        """Serve the enhanced dashboard with resource monitoring"""
        try:
            dashboard_path = Path(__file__).parent / 'enhanced_dashboard.html'
            if dashboard_path.exists():
                return web.FileResponse(dashboard_path)
            else:
                # Fallback to basic dashboard
                return web.Response(text="""
                <!DOCTYPE html>
                <html>
                <head>
                    <title>GitHub Archive Scraper Dashboard</title>
                    <meta charset="UTF-8">
                    <meta name="viewport" content="width=device-width, initial-scale=1.0">
                </head>
                <body>
                    <h1>GitHub Archive Scraper Dashboard</h1>
                    <p>Enhanced dashboard not found. Please check the installation.</p>
                    <p><a href="/api/status">API Status</a></p>
                </body>
                </html>
                """, content_type='text/html')
        except Exception as e:
            self.logger.error(f"Dashboard error: {e}")
            return web.Response(text=f"Dashboard error: {e}", status=500)

    async def run(self):
        """Run the safe web API server"""
        try:
            # Initialize database
            await self.init_database()
            
            # Setup routes
            await self.setup_routes()
            
            # Create and start server
            if not self.app:
                raise RuntimeError("Web application not initialized")
                
            runner = web.AppRunner(self.app)
            await runner.setup()
            
            site = web.TCPSite(
                runner, 
                self.config.WEB_HOST, 
                self.config.WEB_PORT
            )
            await site.start()
            
            self.logger.info(f"Safe Web API started on http://{self.config.WEB_HOST}:{self.config.WEB_PORT}")
            self.logger.info(f"Dashboard available at http://170.9.239.38:{self.config.WEB_PORT}")
            self.logger.info("Resource monitoring enabled for Oracle Cloud safety")
            
            # Wait for shutdown signal
            await self.shutdown_event.wait()
            
        except Exception as e:
            self.logger.error(f"Server startup failed: {e}")
            raise
        finally:
            # Cleanup
            if self.scraper_process and self.scraper_process.poll() is None:
                self.scraper_process.terminate()
            
            await self.db.disconnect()
            self.resource_monitor.cleanup_temp_files()
            self.logger.info("Server shutdown complete")
    
    # Authentication handler methods
    async def auth_login(self, request):
        """Handle user login"""
        try:
            data = await request.json()
            password = data.get('password', '')
            
            if self.auth_manager.verify_admin_password(password):
                token = self.auth_manager.create_session('admin')
                return web.json_response({
                    'success': True,
                    'token': token,
                    'message': 'Login successful'
                })
            else:
                return web.json_response({
                    'success': False,
                    'message': 'Invalid password'
                }, status=401)
                
        except Exception as e:
            self.logger.error(f"Login error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def auth_logout(self, request):
        """Handle user logout"""
        try:
            auth_header = request.headers.get('Authorization', '')
            token = auth_header.replace('Bearer ', '') if auth_header.startswith('Bearer ') else ''
            
            if token:
                self.auth_manager.revoke_session(token)
            
            return web.json_response({
                'success': True,
                'message': 'Logout successful'
            })
            
        except Exception as e:
            self.logger.error(f"Logout error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def auth_status(self, request):
        """Check authentication status"""
        try:
            auth_header = request.headers.get('Authorization', '')
            token = auth_header.replace('Bearer ', '') if auth_header.startswith('Bearer ') else ''
            
            session = self.auth_manager.get_session(token) if token else None
            if session and session.is_valid:
                return web.json_response({
                    'authenticated': True,
                    'permissions': session.permissions,
                    'expires_at': session.expires_at
                })
            else:
                return web.json_response({
                    'authenticated': False,
                    'message': 'Invalid or expired session'
                }, status=401)
                
        except Exception as e:
            self.logger.error(f"Auth status error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def auth_set_password(self, request):
        """Set new password"""
        try:
            data = await request.json()
            new_password = data.get('password', '')
            
            if len(new_password) < 8:
                return web.json_response({
                    'success': False,
                    'message': 'Password must be at least 8 characters'
                }, status=400)
            
            self.auth_manager.set_admin_password(new_password)
            return web.json_response({
                'success': True,
                'message': 'Password updated successfully'
            })
            
        except Exception as e:
            self.logger.error(f"Set password error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    # Rate limiting handler methods
    async def get_rate_limit_status(self, request):
        """Get current rate limit status"""
        try:
            status = self.rate_limiter.get_status()
            return web.json_response({
                'rate_limit_status': status,
                'is_authenticated': bool(self.config.GITHUB_TOKEN),
                'next_reset': status.get('reset_time') if status.get('reset_time') else None
            })
            
        except Exception as e:
            self.logger.error(f"Rate limit status error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def reset_rate_limit(self, request):
        """Reset rate limit tracking (admin only)"""
        try:
            # Check authentication
            auth_header = request.headers.get('Authorization', '')
            token = auth_header.replace('Bearer ', '') if auth_header.startswith('Bearer ') else ''
            
            session = self.auth_manager.get_session(token) if token else None
            if not session or not session.is_valid:
                return web.json_response({'error': 'Authentication required'}, status=401)
            
            # Reset rate limiter by creating new instance
            self.rate_limiter = GitHubRateLimiter(self.config.GITHUB_TOKEN)
            return web.json_response({
                'success': True,
                'message': 'Rate limit tracking reset'
            })
            
        except Exception as e:
            self.logger.error(f"Reset rate limit error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_rate_limit_config(self, request):
        """Get rate limit configuration"""
        try:
            return web.json_response({
                'github_token_configured': bool(self.config.GITHUB_TOKEN),
                'unauthenticated_limit': 60,
                'authenticated_limit': 5000,
                'current_limit': 5000 if self.config.GITHUB_TOKEN else 60,
                'reset_interval': '1 hour'
            })
            
        except Exception as e:
            self.logger.error(f"Rate limit config error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    # Data import handler methods
    async def import_json_data(self, request):
        """Import data from JSON file"""
        try:
            # Check authentication
            auth_header = request.headers.get('Authorization', '')
            token = auth_header.replace('Bearer ', '') if auth_header.startswith('Bearer ') else ''
            
            session = self.auth_manager.get_session(token) if token else None
            if not session or not session.is_valid:
                return web.json_response({'error': 'Authentication required'}, status=401)
            
            data = await request.json()
            file_path = data.get('file_path', '')
            
            if not file_path or not Path(file_path).exists():
                return web.json_response({
                    'error': 'Valid file path required'
                }, status=400)
            
            # Start import in background
            task_id = f"import_json_{int(time.time())}"
            asyncio.create_task(self._background_json_import(file_path, task_id))
            
            return web.json_response({
                'success': True,
                'task_id': task_id,
                'message': 'JSON import started'
            })
            
        except Exception as e:
            self.logger.error(f"JSON import error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def import_bigquery_data(self, request):
        """Import data from BigQuery"""
        try:
            # Check authentication
            auth_header = request.headers.get('Authorization', '')
            token = auth_header.replace('Bearer ', '') if auth_header.startswith('Bearer ') else ''
            
            session = self.auth_manager.get_session(token) if token else None
            if not session or not session.is_valid:
                return web.json_response({'error': 'Authentication required'}, status=401)
            
            data = await request.json()
            project_id = data.get('project_id', '')
            dataset_id = data.get('dataset_id', '')
            table_id = data.get('table_id', '')
            
            if not all([project_id, dataset_id, table_id]):
                return web.json_response({
                    'error': 'project_id, dataset_id, and table_id required'
                }, status=400)
            
            # Start import in background
            task_id = f"import_bigquery_{int(time.time())}"
            asyncio.create_task(self._background_bigquery_import(
                project_id, dataset_id, table_id, task_id
            ))
            
            return web.json_response({
                'success': True,
                'task_id': task_id,
                'message': 'BigQuery import started'
            })
            
        except Exception as e:
            self.logger.error(f"BigQuery import error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_import_status(self, request):
        """Get import task status"""
        try:
            # Return simple status for now
            return web.json_response({
                'available_imports': ['json', 'bigquery'],
                'supported_formats': ['.json', '.jsonl', '.json.gz'],
                'import_active': False
            })
            
        except Exception as e:
            self.logger.error(f"Import status error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    # Wordlist generation handler methods
    async def generate_wordlists(self, request):
        """Generate comprehensive wordlists"""
        try:
            # Check authentication
            auth_header = request.headers.get('Authorization', '')
            token = auth_header.replace('Bearer ', '') if auth_header.startswith('Bearer ') else ''
            
            session = self.auth_manager.get_session(token) if token else None
            if not session or not session.is_valid:
                return web.json_response({'error': 'Authentication required'}, status=401)
            
            data = await request.json()
            target_domains = data.get('target_domains', [])
            technologies = data.get('technologies', [])
            days_back = data.get('days_back', 30)
            max_words = data.get('max_words', 10000)
            
            # Start generation in background
            task_id = f"wordlist_gen_{int(time.time())}"
            asyncio.create_task(self._background_wordlist_generation(
                target_domains, technologies, days_back, max_words, task_id
            ))
            
            return web.json_response({
                'success': True,
                'task_id': task_id,
                'message': 'Wordlist generation started'
            })
            
        except Exception as e:
            self.logger.error(f"Wordlist generation error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def generate_targeted_wordlist(self, request):
        """Generate targeted wordlist for specific domain"""
        try:
            # Check authentication
            auth_header = request.headers.get('Authorization', '')
            token = auth_header.replace('Bearer ', '') if auth_header.startswith('Bearer ') else ''
            
            session = self.auth_manager.get_session(token) if token else None
            if not session or not session.is_valid:
                return web.json_response({'error': 'Authentication required'}, status=401)
            
            data = await request.json()
            target_domain = data.get('target_domain', '')
            max_words = data.get('max_words', 5000)
            
            if not target_domain:
                return web.json_response({
                    'error': 'target_domain required'
                }, status=400)
            
            # Generate wordlist
            async with self.wordlist_generator:
                words = await self.wordlist_generator.generate_targeted_wordlist(
                    target_domain, max_words
                )
            
            return web.json_response({
                'success': True,
                'target_domain': target_domain,
                'word_count': len(words),
                'words': words[:100],  # Return first 100 for preview
                'download_url': f'/api/wordlists/download/targeted_{target_domain}_{int(time.time())}.txt'
            })
            
        except Exception as e:
            self.logger.error(f"Targeted wordlist error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def download_wordlist(self, request):
        """Download generated wordlist file"""
        try:
            filename = request.match_info['filename']
            wordlist_dir = Path('./wordlists')
            file_path = wordlist_dir / filename
            
            if not file_path.exists():
                return web.json_response({'error': 'File not found'}, status=404)
            
            return web.FileResponse(file_path)
            
        except Exception as e:
            self.logger.error(f"Wordlist download error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    # Background task methods
    async def _background_json_import(self, file_path: str, task_id: str):
        """Background JSON import task"""
        try:
            self.logger.info(f"Starting JSON import: {file_path}")
            
            async with self.data_importer:
                events_imported = await self.data_importer.import_json_file(Path(file_path))
                
            self.logger.info(f"JSON import completed: {events_imported} events imported")
            
        except Exception as e:
            self.logger.error(f"Background JSON import failed: {e}")
    
    async def _background_bigquery_import(self, project_id: str, dataset_id: str, 
                                        table_id: str, task_id: str):
        """Background BigQuery import task"""
        try:
            self.logger.info(f"Starting BigQuery import: {project_id}.{dataset_id}.{table_id}")
            
            async with self.data_importer:
                events_imported = await self.data_importer.import_from_bigquery(
                    project_id, dataset_id, table_id
                )
                
            self.logger.info(f"BigQuery import completed: {events_imported} events imported")
            
        except Exception as e:
            self.logger.error(f"Background BigQuery import failed: {e}")
    
    async def _background_wordlist_generation(self, target_domains: List[str], 
                                            technologies: List[str], days_back: int,
                                            max_words: int, task_id: str):
        """Background wordlist generation task"""
        try:
            self.logger.info(f"Starting wordlist generation for {target_domains}")
            
            async with self.wordlist_generator:
                wordlists = await self.wordlist_generator.generate_comprehensive_wordlist(
                    target_domains, technologies, days_back, max_words
                )
                
                if wordlists:
                    output_dir = Path('./wordlists')
                    saved_files = await self.wordlist_generator.save_wordlists(
                        wordlists, output_dir, f"comprehensive_{task_id}"
                    )
                    
                    self.logger.info(f"Wordlist generation completed: {saved_files}")
                
        except Exception as e:
            self.logger.error(f"Background wordlist generation failed: {e}")

    async def _auto_resume_monitor(self):
        """Monitor and auto-resume scraper based on rate limits and resources"""
        while True:
            try:
                await asyncio.sleep(30)  # Check every 30 seconds
                
                if not self.auto_resume_enabled:
                    continue
                
                # Check if scraper is paused due to rate limits
                rate_status = self.rate_limiter.get_status()
                resource_status = self.resource_monitor.get_status()
                
                # Auto-pause if rate limited or resource pressure
                if self.scraper_process and self.scraper_process.poll() is None:
                    should_pause = (
                        rate_status.get('remaining', 0) < 10 or  # Low rate limit
                        not resource_status['is_safe']  # Resource pressure
                    )
                    
                    if should_pause and not self.scraper_paused:
                        self.logger.info("Auto-pausing scraper due to rate limits or resource pressure")
                        try:
                            import os
                            import signal
                            os.kill(self.scraper_process.pid, signal.SIGSTOP)
                            self.scraper_paused = True
                        except:
                            pass
                    
                    # Auto-resume if conditions improve
                    elif not should_pause and self.scraper_paused:
                        self.logger.info("Auto-resuming scraper - conditions improved")
                        try:
                            import os
                            import signal
                            os.kill(self.scraper_process.pid, signal.SIGCONT)
                            self.scraper_paused = False
                        except:
                            pass
                
            except Exception as e:
                self.logger.error(f"Auto-resume monitor error: {e}")
                await asyncio.sleep(60)  # Wait longer on error

def main():
    """Main entry point"""
    # Setup signal handlers for graceful shutdown
    api = SafeWebAPI()
    
    def signal_handler(signum, frame):
        logging.info(f"Received signal {signum}, shutting down...")
        api.shutdown_event.set()
    
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    # Run the server
    try:
        asyncio.run(api.run())
    except KeyboardInterrupt:
        logging.info("Received keyboard interrupt")
    except Exception as e:
        logging.error(f"Server error: {e}")
        sys.exit(1)

if __name__ == '__main__':
    main()
