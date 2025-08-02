#!/usr/bin/env python3
"""
Professional Web API for GitHub Archive Scraper
Consolidated API with authentication, resource monitoring, and comprehensive endpoints.
"""

import asyncio
import json
import logging
import time
from datetime import datetime, timezone
from typing import Dict, Any, Optional
from aiohttp import web, web_middlewares
from aiohttp.web_middlewares import cors_handler
import aiohttp_cors

# Import our consolidated modules
from core.config import Config
from core.database import DatabaseManager, QualityMetrics
from core.auth import AuthManager, UserSession
from enhanced_scraper import GitHubArchiveScraper, ResourceMonitor


class APIError(Exception):
    """API-specific errors"""
    pass


@web_middlewares.middleware
async def auth_middleware(request: web.Request, handler):
    """Authentication middleware for protected endpoints"""
    # Skip auth for public endpoints
    public_endpoints = {'/api/health', '/api/status', '/api/login', '/'}
    if request.path in public_endpoints or request.path.startswith('/static'):
        return await handler(request)
    
    # Check for session token
    auth_header = request.headers.get('Authorization', '')
    if auth_header.startswith('Bearer '):
        token = auth_header[7:]  # Remove 'Bearer ' prefix
    else:
        token = request.cookies.get('session_token', '')
    
    if not token:
        return web.json_response({'error': 'Authentication required'}, status=401)
    
    # Validate session
    auth_manager: AuthManager = request.app['auth_manager']
    session = auth_manager.validate_session(token)
    
    if not session:
        return web.json_response({'error': 'Invalid or expired session'}, status=401)
    
    # Store session in request for use in handlers
    request['session'] = session
    return await handler(request)


@web_middlewares.middleware
async def error_middleware(request: web.Request, handler):
    """Global error handling middleware"""
    try:
        return await handler(request)
    except web.HTTPException:
        raise
    except Exception as e:
        logger = logging.getLogger(__name__)
        logger.error(f"Unhandled error in {request.path}: {e}")
        return web.json_response({
            'error': 'Internal server error',
            'message': str(e) if request.app['config'].web.debug else 'An unexpected error occurred'
        }, status=500)


@web_middlewares.middleware
async def logging_middleware(request: web.Request, handler):
    """Request logging middleware"""
    start_time = time.time()
    
    try:
        response = await handler(request)
        duration = time.time() - start_time
        
        logger = logging.getLogger('api_access')
        logger.info(f"{request.method} {request.path} - {response.status} - {duration:.3f}s")
        
        return response
    except Exception as e:
        duration = time.time() - start_time
        logger = logging.getLogger('api_access')
        logger.error(f"{request.method} {request.path} - ERROR: {e} - {duration:.3f}s")
        raise


class ProfessionalAPI:
    """
    Professional API server with comprehensive endpoints,
    authentication, and resource monitoring.
    """
    
    def __init__(self, config: Optional[Config] = None):
        """Initialize API server"""
        self.config = config or Config()
        self.logger = logging.getLogger(__name__)
        
        # Initialize core components
        self.db = DatabaseManager(self.config)
        self.auth = AuthManager()
        self.resource_monitor = ResourceMonitor(self.config)
        self.scraper: Optional[GitHubArchiveScraper] = None
        
        # Web application
        self.app = web.Application(middlewares=[
            logging_middleware,
            error_middleware,
            auth_middleware
        ])
        
        # Store components in app for middleware access
        self.app['config'] = self.config
        self.app['db'] = self.db
        self.app['auth_manager'] = self.auth
        self.app['resource_monitor'] = self.resource_monitor
        
        # Setup routes and CORS
        self._setup_routes()
        self._setup_cors()
    
    async def initialize(self) -> None:
        """Initialize API components"""
        self.logger.info("Initializing Professional API...")
        
        try:
            # Initialize database
            await self.db.connect()
            self.logger.info("Database connected")
            
            # Initialize scraper instance
            self.scraper = GitHubArchiveScraper(self.config)
            await self.scraper.initialize()
            self.logger.info("Scraper initialized")
            
            # Setup authentication
            self.auth.create_admin_user('admin', self.config.security.admin_password)
            
            self.logger.info("Professional API initialization complete")
            
        except Exception as e:
            self.logger.error(f"API initialization failed: {e}")
            raise
    
    async def shutdown(self) -> None:
        """Graceful shutdown"""
        self.logger.info("Shutting down Professional API...")
        
        try:
            if self.scraper:
                await self.scraper.shutdown()
            await self.db.disconnect()
            self.logger.info("Professional API shutdown complete")
        except Exception as e:
            self.logger.error(f"Shutdown error: {e}")
    
    def _setup_routes(self) -> None:
        """Setup API routes"""
        # Public endpoints
        self.app.router.add_get('/', self.serve_dashboard)
        self.app.router.add_get('/api/health', self.health_check)
        self.app.router.add_get('/api/status', self.get_status)
        
        # Authentication endpoints
        self.app.router.add_post('/api/login', self.login)
        self.app.router.add_post('/api/logout', self.logout)
        self.app.router.add_post('/api/change-password', self.change_password)
        
        # Data endpoints (require authentication)
        self.app.router.add_get('/api/events', self.get_events)
        self.app.router.add_get('/api/repositories', self.get_repositories)
        self.app.router.add_get('/api/data-quality', self.get_data_quality)
        self.app.router.add_get('/api/statistics', self.get_statistics)
        
        # Search endpoints
        self.app.router.add_post('/api/search/events', self.search_events)
        self.app.router.add_post('/api/search/repositories', self.search_repositories)
        
        # Scraper control endpoints (require admin permission)
        self.app.router.add_post('/api/scraper/start', self.start_scraper)
        self.app.router.add_post('/api/scraper/stop', self.stop_scraper)
        self.app.router.add_get('/api/scraper/status', self.get_scraper_status)
        self.app.router.add_get('/api/scraper/logs', self.get_scraper_logs)
        
        # Resource monitoring endpoints
        self.app.router.add_get('/api/resources', self.get_resources)
        self.app.router.add_post('/api/resources/cleanup', self.emergency_cleanup)
        
        # Admin endpoints
        self.app.router.add_get('/api/admin/sessions', self.list_sessions)
        self.app.router.add_post('/api/admin/cleanup-sessions', self.cleanup_sessions)
        self.app.router.add_get('/api/admin/audit-log', self.get_audit_log)
        
        # Static files
        self.app.router.add_static('/', path='static', name='static')
    
    def _setup_cors(self) -> None:
        """Setup CORS configuration"""
        cors = aiohttp_cors.setup(self.app, defaults={
            origin: aiohttp_cors.ResourceOptions(
                allow_credentials=True,
                expose_headers="*",
                allow_headers="*",
                allow_methods="*"
            ) for origin in self.config.web.cors_origins
        })
        
        # Add CORS to all routes
        for route in list(self.app.router.routes()):
            cors.add(route)
    
    # Public endpoints
    async def serve_dashboard(self, request: web.Request) -> web.Response:
        """Serve the main dashboard"""
        try:
            # In a real implementation, this would serve an HTML file
            return web.Response(
                text="""
                <!DOCTYPE html>
                <html>
                <head>
                    <title>GitHub Archive Scraper - Professional</title>
                    <meta charset="utf-8">
                    <meta name="viewport" content="width=device-width, initial-scale=1">
                    <style>
                        body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
                        .container { max-width: 1200px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
                        .header { text-align: center; margin-bottom: 30px; }
                        .status-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; }
                        .status-card { padding: 20px; border: 1px solid #ddd; border-radius: 8px; }
                        .status-card h3 { margin-top: 0; color: #333; }
                        .metric { display: flex; justify-content: space-between; margin: 10px 0; }
                        .metric-label { font-weight: bold; }
                        .metric-value { color: #007bff; }
                    </style>
                </head>
                <body>
                    <div class="container">
                        <div class="header">
                            <h1>🚀 GitHub Archive Scraper</h1>
                            <p>Professional Edition - Real-time GitHub Event Processing</p>
                        </div>
                        
                        <div class="status-grid" id="statusGrid">
                            <div class="status-card">
                                <h3>📊 System Status</h3>
                                <div class="metric">
                                    <span class="metric-label">Status:</span>
                                    <span class="metric-value" id="systemStatus">Loading...</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">Uptime:</span>
                                    <span class="metric-value" id="uptime">Loading...</span>
                                </div>
                            </div>
                            
                            <div class="status-card">
                                <h3>💾 Database</h3>
                                <div class="metric">
                                    <span class="metric-label">Events:</span>
                                    <span class="metric-value" id="totalEvents">Loading...</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">Quality Score:</span>
                                    <span class="metric-value" id="qualityScore">Loading...</span>
                                </div>
                            </div>
                            
                            <div class="status-card">
                                <h3>⚡ Resources</h3>
                                <div class="metric">
                                    <span class="metric-label">Memory:</span>
                                    <span class="metric-value" id="memoryUsage">Loading...</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">CPU:</span>
                                    <span class="metric-value" id="cpuUsage">Loading...</span>
                                </div>
                            </div>
                        </div>
                        
                        <div style="margin-top: 30px; text-align: center;">
                            <p><strong>API Endpoints Available:</strong></p>
                            <p>
                                <a href="/api/health">Health Check</a> | 
                                <a href="/api/status">System Status</a> | 
                                <a href="/api/data-quality">Data Quality</a> |
                                <a href="/api/resources">Resource Monitor</a>
                            </p>
                        </div>
                    </div>
                    
                    <script>
                        async function updateStatus() {
                            try {
                                const response = await fetch('/api/status');
                                const status = await response.json();
                                
                                document.getElementById('systemStatus').textContent = status.api?.status || 'Unknown';
                                document.getElementById('uptime').textContent = status.api?.uptime || 'Unknown';
                                document.getElementById('totalEvents').textContent = status.data_quality?.total_events?.toLocaleString() || '0';
                                document.getElementById('qualityScore').textContent = status.data_quality?.quality_score?.toFixed(1) + '%' || 'N/A';
                                document.getElementById('memoryUsage').textContent = status.resources?.memory?.percent + '%' || 'N/A';
                                document.getElementById('cpuUsage').textContent = status.resources?.cpu?.percent + '%' || 'N/A';
                            } catch (error) {
                                console.error('Failed to update status:', error);
                            }
                        }
                        
                        // Update status every 10 seconds
                        updateStatus();
                        setInterval(updateStatus, 10000);
                    </script>
                </body>
                </html>
                """,
                content_type='text/html'
            )
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def health_check(self, request: web.Request) -> web.Response:
        """Health check endpoint"""
        try:
            db_health = await self.db.health_check()
            resource_status = self.resource_monitor.get_resource_status()
            
            is_healthy = (
                db_health.is_connected and 
                not resource_status.get('emergency_mode', False)
            )
            
            return web.json_response({
                'status': 'healthy' if is_healthy else 'degraded',
                'timestamp': datetime.now(timezone.utc).isoformat(),
                'database': db_health.is_connected,
                'emergency_mode': resource_status.get('emergency_mode', False),
                'version': '2.0.0'
            }, status=200 if is_healthy else 503)
            
        except Exception as e:
            return web.json_response({
                'status': 'unhealthy',
                'error': str(e),
                'timestamp': datetime.now(timezone.utc).isoformat()
            }, status=503)
    
    async def get_status(self, request: web.Request) -> web.Response:
        """Get comprehensive system status"""
        try:
            # Get database health
            db_health = await self.db.health_check()
            
            # Get resource status
            resource_status = self.resource_monitor.get_resource_status()
            
            # Get data quality metrics
            quality_metrics = await self.db.get_data_quality_metrics()
            
            # Get scraper status if available
            scraper_status = {}
            if self.scraper:
                scraper_status = await self.scraper.get_status()
            
            return web.json_response({
                'api': {
                    'status': 'running',
                    'version': '2.0.0',
                    'uptime': time.time(),
                    'timestamp': datetime.now(timezone.utc).isoformat()
                },
                'database': {
                    'connected': db_health.is_connected,
                    'connection_count': db_health.connection_count,
                    'active_queries': db_health.active_queries,
                    'cache_hit_ratio': db_health.cache_hit_ratio
                },
                'resources': resource_status,
                'data_quality': {
                    'total_events': quality_metrics.total_events,
                    'unique_actors': quality_metrics.unique_actors,
                    'unique_repos': quality_metrics.unique_repos,
                    'quality_score': quality_metrics.quality_score,
                    'integrity_issues': quality_metrics.integrity_issues
                },
                'scraper': scraper_status.get('scraper', {})
            })
            
        except Exception as e:
            self.logger.error(f"Status check failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    # Authentication endpoints
    async def login(self, request: web.Request) -> web.Response:
        """User login endpoint"""
        try:
            data = await request.json()
            username = data.get('username', '')
            password = data.get('password', '')
            
            if not username or not password:
                return web.json_response({
                    'success': False,
                    'message': 'Username and password required'
                }, status=400)
            
            # Get client info
            ip_address = request.remote
            user_agent = request.headers.get('User-Agent', '')
            
            # Authenticate user
            session_token = self.auth.authenticate(username, password, ip_address, user_agent)
            
            if session_token:
                response = web.json_response({
                    'success': True,
                    'message': 'Login successful',
                    'token': session_token
                })
                
                # Set secure cookie
                response.set_cookie(
                    'session_token', 
                    session_token,
                    max_age=self.config.security.session_duration_hours * 3600,
                    httponly=True,
                    secure=True,
                    samesite='Strict'
                )
                
                return response
            else:
                return web.json_response({
                    'success': False,
                    'message': 'Invalid credentials'
                }, status=401)
                
        except Exception as e:
            self.logger.error(f"Login error: {e}")
            return web.json_response({
                'success': False,
                'message': 'Login failed'
            }, status=500)
    
    async def logout(self, request: web.Request) -> web.Response:
        """User logout endpoint"""
        try:
            session: UserSession = request.get('session')
            if session:
                self.auth.logout(session.session_id)
            
            response = web.json_response({
                'success': True,
                'message': 'Logged out successfully'
            })
            
            # Clear cookie
            response.del_cookie('session_token')
            return response
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def change_password(self, request: web.Request) -> web.Response:
        """Change password endpoint"""
        try:
            session: UserSession = request['session']
            data = await request.json()
            
            old_password = data.get('old_password', '')
            new_password = data.get('new_password', '')
            
            if not old_password or not new_password:
                return web.json_response({
                    'success': False,
                    'message': 'Old and new passwords required'
                }, status=400)
            
            success = self.auth.change_password(session.username, old_password, new_password)
            
            if success:
                return web.json_response({
                    'success': True,
                    'message': 'Password changed successfully'
                })
            else:
                return web.json_response({
                    'success': False,
                    'message': 'Password change failed'
                }, status=400)
                
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    # Data endpoints
    async def get_events(self, request: web.Request) -> web.Response:
        """Get events with pagination"""
        try:
            # Parse query parameters
            limit = int(request.query.get('limit', '50'))
            offset = int(request.query.get('offset', '0'))
            event_type = request.query.get('type', '')
            
            limit = min(limit, 1000)  # Cap at 1000
            
            # Build query
            query = """
                SELECT event_id, event_type, event_created_at, actor_login, repo_name, repo_full_name
                FROM github_events
            """
            params = []
            
            if event_type:
                query += " WHERE event_type = $1"
                params.append(event_type)
            
            query += " ORDER BY event_created_at DESC LIMIT $" + str(len(params) + 1) + " OFFSET $" + str(len(params) + 2)
            params.extend([limit, offset])
            
            events = await self.db.execute_query(query, *params)
            
            # Convert to JSON-serializable format
            result_events = []
            for event in events:
                result_events.append({
                    'id': event['event_id'],
                    'type': event['event_type'],
                    'created_at': event['event_created_at'].isoformat(),
                    'actor_login': event['actor_login'],
                    'repo_name': event['repo_name'],
                    'repo_full_name': event['repo_full_name']
                })
            
            return web.json_response({
                'events': result_events,
                'limit': limit,
                'offset': offset,
                'count': len(result_events)
            })
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_repositories(self, request: web.Request) -> web.Response:
        """Get repositories with pagination"""
        try:
            limit = int(request.query.get('limit', '50'))
            offset = int(request.query.get('offset', '0'))
            
            limit = min(limit, 1000)
            
            repositories = await self.db.execute_query("""
                SELECT DISTINCT repo_id, repo_name, repo_full_name, repo_description, 
                       repo_language, repo_stargazers_count, repo_forks_count
                FROM github_events
                WHERE repo_id IS NOT NULL
                ORDER BY repo_stargazers_count DESC NULLS LAST
                LIMIT $1 OFFSET $2
            """, limit, offset)
            
            result_repos = []
            for repo in repositories:
                result_repos.append({
                    'id': repo['repo_id'],
                    'name': repo['repo_name'],
                    'full_name': repo['repo_full_name'],
                    'description': repo['repo_description'],
                    'language': repo['repo_language'],
                    'stargazers_count': repo['repo_stargazers_count'],
                    'forks_count': repo['repo_forks_count']
                })
            
            return web.json_response({
                'repositories': result_repos,
                'limit': limit,
                'offset': offset,
                'count': len(result_repos)
            })
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_data_quality(self, request: web.Request) -> web.Response:
        """Get comprehensive data quality metrics"""
        try:
            quality_metrics = await self.db.get_data_quality_metrics()
            
            return web.json_response({
                'quality_metrics': {
                    'total_events': quality_metrics.total_events,
                    'unique_actors': quality_metrics.unique_actors,
                    'unique_repos': quality_metrics.unique_repos,
                    'event_types': quality_metrics.event_types,
                    'quality_score': quality_metrics.quality_score,
                    'integrity_issues': quality_metrics.integrity_issues,
                    'processing_stats': quality_metrics.processing_stats,
                    'recent_activity': quality_metrics.recent_activity
                },
                'timestamp': datetime.now(timezone.utc).isoformat()
            })
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_resources(self, request: web.Request) -> web.Response:
        """Get resource monitoring data"""
        try:
            resource_status = self.resource_monitor.get_resource_status()
            return web.json_response(resource_status)
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def emergency_cleanup(self, request: web.Request) -> web.Response:
        """Trigger emergency resource cleanup"""
        try:
            # Require admin permission
            session: UserSession = request['session']
            if not session.has_permission('admin'):
                return web.json_response({'error': 'Admin permission required'}, status=403)
            
            cleanup_result = await self.resource_monitor.emergency_cleanup()
            return web.json_response(cleanup_result)
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    # Admin endpoints
    async def list_sessions(self, request: web.Request) -> web.Response:
        """List active sessions (admin only)"""
        try:
            session: UserSession = request['session']
            if not session.has_permission('admin'):
                return web.json_response({'error': 'Admin permission required'}, status=403)
            
            sessions = self.auth.list_active_sessions()
            return web.json_response({'sessions': sessions})
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def cleanup_sessions(self, request: web.Request) -> web.Response:
        """Clean up expired sessions (admin only)"""
        try:
            session: UserSession = request['session']
            if not session.has_permission('admin'):
                return web.json_response({'error': 'Admin permission required'}, status=403)
            
            removed_count = self.auth.cleanup_expired_sessions()
            return web.json_response({
                'success': True,
                'removed_sessions': removed_count
            })
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    # Placeholder methods for additional endpoints
    async def get_statistics(self, request: web.Request) -> web.Response:
        """Get system statistics"""
        return web.json_response({'message': 'Statistics endpoint - Coming soon'})
    
    async def search_events(self, request: web.Request) -> web.Response:
        """Search events"""
        return web.json_response({'message': 'Event search endpoint - Coming soon'})
    
    async def search_repositories(self, request: web.Request) -> web.Response:
        """Search repositories"""
        return web.json_response({'message': 'Repository search endpoint - Coming soon'})
    
    async def start_scraper(self, request: web.Request) -> web.Response:
        """Start scraper"""
        return web.json_response({'message': 'Scraper control endpoint - Coming soon'})
    
    async def stop_scraper(self, request: web.Request) -> web.Response:
        """Stop scraper"""
        return web.json_response({'message': 'Scraper control endpoint - Coming soon'})
    
    async def get_scraper_status(self, request: web.Request) -> web.Response:
        """Get scraper status"""
        return web.json_response({'message': 'Scraper status endpoint - Coming soon'})
    
    async def get_scraper_logs(self, request: web.Request) -> web.Response:
        """Get scraper logs"""
        return web.json_response({'message': 'Scraper logs endpoint - Coming soon'})
    
    async def get_audit_log(self, request: web.Request) -> web.Response:
        """Get audit log"""
        return web.json_response({'message': 'Audit log endpoint - Coming soon'})


async def create_app() -> web.Application:
    """Create and initialize the web application"""
    config = Config()
    api = ProfessionalAPI(config)
    await api.initialize()
    return api.app


async def main():
    """Main entry point for the API server"""
    logger = logging.getLogger(__name__)
    
    try:
        # Create configuration
        config = Config()
        
        # Create API instance
        api = ProfessionalAPI(config)
        await api.initialize()
        
        # Start web server
        logger.info(f"Starting Professional API server on {config.web.host}:{config.web.port}")
        
        runner = web.AppRunner(api.app)
        await runner.setup()
        
        site = web.TCPSite(runner, config.web.host, config.web.port)
        await site.start()
        
        logger.info(f"Professional API server running at {config.web.base_url}")
        
        # Keep running until shutdown
        try:
            while True:
                await asyncio.sleep(1)
        except KeyboardInterrupt:
            logger.info("Shutting down API server...")
        finally:
            await api.shutdown()
            await runner.cleanup()
    
    except Exception as e:
        logger.error(f"API server failed: {e}")
        return 1
    
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(asyncio.run(main()))
