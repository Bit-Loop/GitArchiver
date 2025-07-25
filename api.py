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
        """Get CPU usage percentage"""
        return self.process.cpu_percent()
    
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
        
        # Management routes
        self.app.router.add_post('/api/start-scraper', self.start_scraper)
        self.app.router.add_post('/api/stop-scraper', self.stop_scraper)
        self.app.router.add_post('/api/restart-scraper', self.restart_scraper)
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
                
                return web.json_response({
                    'events': [dict(event) for event in events],
                    'count': len(events),
                    'limit': limit,
                    'offset': offset
                }, default=str)  # Handle datetime serialization
                
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
                
                return web.json_response({
                    'repositories': [dict(repo) for repo in repos],
                    'count': len(repos),
                    'limit': limit,
                    'offset': offset
                }, default=str)
                
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
                
                return web.json_response({
                    'events': [dict(event) for event in events],
                    'query': query,
                    'count': len(events)
                }, default=str)
                
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
    
    async def _delayed_shutdown(self):
        """Delayed shutdown to allow response to be sent"""
        await asyncio.sleep(1)
        self.shutdown_event.set()
    
    async def serve_dashboard(self, request):
        """Serve the enhanced dashboard with resource monitoring"""
        dashboard_html = """
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>GitHub Archive Scraper - Safe Dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { 
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; 
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            color: #333;
        }
        .container { 
            max-width: 1400px; 
            margin: 0 auto; 
            padding: 20px; 
        }
        .header {
            text-align: center;
            margin-bottom: 30px;
            color: white;
        }
        .status-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }
        .card {
            background: rgba(255,255,255,0.95);
            border-radius: 15px;
            padding: 20px;
            box-shadow: 0 8px 32px rgba(0,0,0,0.1);
            backdrop-filter: blur(10px);
        }
        .card h3 {
            margin-bottom: 15px;
            color: #4a5568;
            border-bottom: 2px solid #e2e8f0;
            padding-bottom: 10px;
        }
        .metric {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin: 10px 0;
            padding: 8px;
            background: #f7fafc;
            border-radius: 8px;
        }
        .metric-label {
            font-weight: 500;
        }
        .metric-value {
            font-weight: 600;
            padding: 4px 8px;
            border-radius: 4px;
        }
        .status-ok { background: #c6f6d5; color: #22543d; }
        .status-warning { background: #fefcbf; color: #744210; }
        .status-error { background: #fed7d7; color: #742a2a; }
        .progress-bar {
            width: 100%;
            height: 20px;
            background: #e2e8f0;
            border-radius: 10px;
            overflow: hidden;
            margin: 10px 0;
        }
        .progress-fill {
            height: 100%;
            transition: width 0.3s ease;
        }
        .progress-ok { background: #48bb78; }
        .progress-warning { background: #ed8936; }
        .progress-error { background: #f56565; }
        .controls {
            display: flex;
            gap: 10px;
            flex-wrap: wrap;
            justify-content: center;
            margin: 20px 0;
        }
        .btn {
            padding: 10px 20px;
            border: none;
            border-radius: 8px;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.2s;
            text-decoration: none;
            color: white;
        }
        .btn-primary { background: #4299e1; }
        .btn-success { background: #48bb78; }
        .btn-warning { background: #ed8936; }
        .btn-danger { background: #f56565; }
        .btn:hover { transform: translateY(-2px); box-shadow: 0 4px 12px rgba(0,0,0,0.2); }
        .btn:disabled { opacity: 0.6; cursor: not-allowed; transform: none; }
        .recommendations {
            background: #edf2f7;
            border-left: 4px solid #4299e1;
            padding: 15px;
            margin: 15px 0;
            border-radius: 0 8px 8px 0;
        }
        .recommendations ul {
            margin-left: 20px;
        }
        .log-container {
            background: #1a202c;
            color: #e2e8f0;
            padding: 15px;
            border-radius: 8px;
            font-family: 'Courier New', monospace;
            font-size: 12px;
            max-height: 300px;
            overflow-y: auto;
        }
        .auto-refresh {
            text-align: center;
            margin: 20px 0;
            color: white;
        }
        @media (max-width: 768px) {
            .container { padding: 10px; }
            .status-grid { grid-template-columns: 1fr; }
            .controls { flex-direction: column; align-items: center; }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 GitHub Archive Scraper</h1>
            <p>Safe Resource Management Dashboard</p>
            <p>Oracle Cloud Optimized</p>
        </div>
        
        <div class="auto-refresh">
            <label>
                <input type="checkbox" id="autoRefresh" checked> Auto-refresh (10s)
            </label>
        </div>
        
        <div class="status-grid">
            <!-- API Status -->
            <div class="card">
                <h3>🔌 API Status</h3>
                <div class="metric">
                    <span class="metric-label">API Server</span>
                    <span class="metric-value status-ok" id="apiStatus">Online</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Database</span>
                    <span class="metric-value" id="dbStatus">Checking...</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Scraper</span>
                    <span class="metric-value" id="scraperStatus">Checking...</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Last Update</span>
                    <span class="metric-value" id="lastUpdate">-</span>
                </div>
            </div>
            
            <!-- Resource Monitoring -->
            <div class="card">
                <h3>💻 Resource Usage</h3>
                <div class="metric">
                    <span class="metric-label">Process Memory</span>
                    <span class="metric-value" id="processMemory">-</span>
                </div>
                <div class="progress-bar">
                    <div class="progress-fill" id="processMemoryBar"></div>
                </div>
                <div class="metric">
                    <span class="metric-label">System Memory</span>
                    <span class="metric-value" id="systemMemory">-</span>
                </div>
                <div class="progress-bar">
                    <div class="progress-fill" id="systemMemoryBar"></div>
                </div>
                <div class="metric">
                    <span class="metric-label">Disk Usage</span>
                    <span class="metric-value" id="diskUsage">-</span>
                </div>
                <div class="progress-bar">
                    <div class="progress-fill" id="diskBar"></div>
                </div>
                <div class="metric">
                    <span class="metric-label">CPU Usage</span>
                    <span class="metric-value" id="cpuUsage">-</span>
                </div>
                <div class="progress-bar">
                    <div class="progress-fill" id="cpuBar"></div>
                </div>
            </div>
            
            <!-- Safety Status -->
            <div class="card">
                <h3>🛡️ Safety Status</h3>
                <div class="metric">
                    <span class="metric-label">Overall Status</span>
                    <span class="metric-value" id="safetyStatus">Checking...</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Emergency Mode</span>
                    <span class="metric-value" id="emergencyMode">-</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Temp Files</span>
                    <span class="metric-value" id="tempFiles">-</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Uptime</span>
                    <span class="metric-value" id="uptime">-</span>
                </div>
                <div class="recommendations" id="recommendations" style="display: none;">
                    <h4>💡 Recommendations</h4>
                    <ul id="recommendationsList"></ul>
                </div>
            </div>
            
            <!-- Database Stats -->
            <div class="card">
                <h3>📊 Database Stats</h3>
                <div class="metric">
                    <span class="metric-label">Total Events</span>
                    <span class="metric-value" id="totalEvents">-</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Repositories</span>
                    <span class="metric-value" id="totalRepos">-</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Processed Files</span>
                    <span class="metric-value" id="processedFiles">-</span>
                </div>
                <div class="metric">
                    <span class="metric-label">Latest Event</span>
                    <span class="metric-value" id="latestEvent">-</span>
                </div>
            </div>
        </div>
        
        <!-- Controls -->
        <div class="controls">
            <button class="btn btn-success" onclick="startScraper()">▶️ Start Scraper</button>
            <button class="btn btn-warning" onclick="stopScraper()">⏹️ Stop Scraper</button>
            <button class="btn btn-primary" onclick="restartScraper()">🔄 Restart Scraper</button>
            <button class="btn btn-warning" onclick="emergencyCleanup()">🧹 Emergency Cleanup</button>
            <button class="btn btn-danger" onclick="shutdownServer()">🛑 Shutdown Server</button>
        </div>
        
        <!-- Logs -->
        <div class="card">
            <h3>📋 Recent Logs</h3>
            <button class="btn btn-primary" onclick="refreshLogs()" style="margin-bottom: 10px;">Refresh Logs</button>
            <div class="log-container" id="logs">
                Click "Refresh Logs" to load recent log entries...
            </div>
        </div>
    </div>
    
    <script>
        let autoRefreshEnabled = true;
        let refreshInterval;
        
        document.getElementById('autoRefresh').addEventListener('change', function(e) {
            autoRefreshEnabled = e.target.checked;
            if (autoRefreshEnabled) {
                startAutoRefresh();
            } else {
                clearInterval(refreshInterval);
            }
        });
        
        function startAutoRefresh() {
            clearInterval(refreshInterval);
            refreshInterval = setInterval(refreshStatus, 10000);
        }
        
        function formatBytes(bytes) {
            const sizes = ['B', 'KB', 'MB', 'GB'];
            if (bytes === 0) return '0 B';
            const i = Math.floor(Math.log(bytes) / Math.log(1024));
            return Math.round(bytes / Math.pow(1024, i) * 100) / 100 + ' ' + sizes[i];
        }
        
        function formatUptime(seconds) {
            const days = Math.floor(seconds / 86400);
            const hours = Math.floor((seconds % 86400) / 3600);
            const minutes = Math.floor((seconds % 3600) / 60);
            return `${days}d ${hours}h ${minutes}m`;
        }
        
        function updateProgressBar(elementId, percent, thresholds = {warning: 70, error: 90}) {
            const bar = document.getElementById(elementId);
            if (!bar) return;
            
            bar.style.width = percent + '%';
            bar.className = 'progress-fill ';
            
            if (percent >= thresholds.error) {
                bar.className += 'progress-error';
            } else if (percent >= thresholds.warning) {
                bar.className += 'progress-warning';
            } else {
                bar.className += 'progress-ok';
            }
        }
        
        function updateStatus(elementId, value, isGood = null) {
            const element = document.getElementById(elementId);
            if (!element) return;
            
            element.textContent = value;
            element.className = 'metric-value ';
            
            if (isGood === true) {
                element.className += 'status-ok';
            } else if (isGood === false) {
                element.className += 'status-error';
            } else if (isGood === null) {
                element.className += 'status-warning';
            }
        }
        
        async function refreshStatus() {
            try {
                // Get API status
                const statusResponse = await fetch('/api/status');
                const status = await statusResponse.json();
                
                // Update API status
                updateStatus('dbStatus', status.database_connected ? 'Connected' : 'Disconnected', status.database_connected);
                updateStatus('scraperStatus', status.scraper_running ? 'Running' : 'Stopped', status.scraper_running);
                
                // Get resource monitoring
                const monitorResponse = await fetch('/api/monitor');
                const monitor = await monitorResponse.json();
                
                // Update resource usage
                updateStatus('processMemory', `${monitor.process_memory_mb} MB`, monitor.process_memory_usage_percent < 80);
                updateProgressBar('processMemoryBar', monitor.process_memory_usage_percent);
                
                updateStatus('systemMemory', `${monitor.system_memory_gb}/${monitor.system_memory_total_gb} GB`, monitor.system_memory_percent < 80);
                updateProgressBar('systemMemoryBar', monitor.system_memory_percent);
                
                updateStatus('diskUsage', `${monitor.disk_used_gb}/${monitor.disk_total_gb} GB`, monitor.disk_usage_percent < 80);
                updateProgressBar('diskBar', monitor.disk_usage_percent);
                
                updateStatus('cpuUsage', `${monitor.cpu_percent}%`, monitor.cpu_percent < 70);
                updateProgressBar('cpuBar', monitor.cpu_percent);
                
                // Update safety status
                updateStatus('safetyStatus', monitor.is_safe ? 'Safe' : 'Warning', monitor.is_safe);
                updateStatus('emergencyMode', monitor.emergency_mode ? 'Active' : 'Normal', !monitor.emergency_mode);
                updateStatus('tempFiles', monitor.temp_files_count, monitor.temp_files_count < 20);
                updateStatus('uptime', monitor.uptime_formatted || formatUptime(monitor.uptime_seconds));
                
                // Update recommendations
                if (monitor.recommendations && monitor.recommendations.length > 0) {
                    const recDiv = document.getElementById('recommendations');
                    const recList = document.getElementById('recommendationsList');
                    recDiv.style.display = 'block';
                    recList.innerHTML = monitor.recommendations.map(r => `<li>${r}</li>`).join('');
                } else {
                    document.getElementById('recommendations').style.display = 'none';
                }
                
                // Get database stats
                const statsResponse = await fetch('/api/stats');
                if (statsResponse.ok) {
                    const stats = await statsResponse.json();
                    
                    updateStatus('totalEvents', stats.total_events?.toLocaleString() || '0');
                    updateStatus('totalRepos', stats.total_repositories?.toLocaleString() || '0');
                    updateStatus('processedFiles', stats.processed_files?.toLocaleString() || '0');
                    updateStatus('latestEvent', stats.latest_event_date ? new Date(stats.latest_event_date).toLocaleString() : 'None');
                }
                
                updateStatus('lastUpdate', new Date().toLocaleTimeString());
                
            } catch (error) {
                console.error('Failed to refresh status:', error);
                updateStatus('lastUpdate', 'Error: ' + new Date().toLocaleTimeString());
            }
        }
        
        async function startScraper() {
            try {
                const response = await fetch('/api/start-scraper', { method: 'POST' });
                const result = await response.json();
                alert(response.ok ? result.message : result.error);
                if (response.ok) refreshStatus();
            } catch (error) {
                alert('Failed to start scraper: ' + error.message);
            }
        }
        
        async function stopScraper() {
            try {
                const response = await fetch('/api/stop-scraper', { method: 'POST' });
                const result = await response.json();
                alert(response.ok ? result.message : result.error);
                if (response.ok) refreshStatus();
            } catch (error) {
                alert('Failed to stop scraper: ' + error.message);
            }
        }
        
        async function restartScraper() {
            try {
                const response = await fetch('/api/restart-scraper', { method: 'POST' });
                const result = await response.json();
                alert(response.ok ? result.message : result.error);
                if (response.ok) refreshStatus();
            } catch (error) {
                alert('Failed to restart scraper: ' + error.message);
            }
        }
        
        async function emergencyCleanup() {
            if (!confirm('Perform emergency cleanup? This will free memory and temporary files.')) return;
            
            try {
                const response = await fetch('/api/emergency-cleanup', { method: 'POST' });
                const result = await response.json();
                alert(response.ok ? `Cleanup completed: ${result.objects_collected} objects collected` : result.error);
                if (response.ok) refreshStatus();
            } catch (error) {
                alert('Failed to perform cleanup: ' + error.message);
            }
        }
        
        async function shutdownServer() {
            if (!confirm('Shutdown the entire server? This will stop both the API and scraper.')) return;
            
            try {
                const response = await fetch('/api/shutdown', { method: 'POST' });
                const result = await response.json();
                alert(result.message);
            } catch (error) {
                alert('Server may have shut down.');
            }
        }
        
        async function refreshLogs() {
            try {
                const response = await fetch('/api/scraper-logs');
                const result = await response.json();
                document.getElementById('logs').textContent = result.logs || 'No logs available';
            } catch (error) {
                document.getElementById('logs').textContent = 'Failed to load logs: ' + error.message;
            }
        }
        
        // Initial load
        refreshStatus();
        startAutoRefresh();
        
        // Load logs on startup
        setTimeout(refreshLogs, 1000);
    </script>
</body>
</html>
        """
        return web.Response(text=dashboard_html, content_type='text/html')
    
    async def run(self):
        """Run the safe web API server"""
        try:
            # Initialize database
            await self.init_database()
            
            # Setup routes
            await self.setup_routes()
            
            # Create and start server
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
