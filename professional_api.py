#!/usr/bin/env python3
"""
Professional Web API for GitHub Archive Scraper
Consolidated API with authentication, resource monitoring, and comprehensive endpoints.
"""

import asyncio
import json
import logging
import secrets
import time
import jwt
from datetime import datetime, timezone, timedelta
from typing import Dict, Any, Optional
from aiohttp import web
import aiohttp_cors

# Import our consolidated modules
from core.config import Config
from core.database import DatabaseManager, QualityMetrics
from core.auth import AuthManager, UserSession
from enhanced_scraper import GitHubArchiveScraper, ResourceMonitor


class APIError(Exception):
    """API-specific errors"""
    pass


@web.middleware
async def auth_middleware(request: web.Request, handler):
    """JWT-based authentication middleware"""
    # Skip auth for public endpoints
    public_endpoints = {'/api/health', '/api/status', '/api/login', '/api/logout', '/api/auth/status', '/api/rate-limit/status', '/'}
    if request.path in public_endpoints or request.path.startswith('/static'):
        return await handler(request)
    
    # Check for JWT token
    auth_header = request.headers.get('Authorization', '')
    if auth_header.startswith('Bearer '):
        token = auth_header[7:]  # Remove 'Bearer ' prefix
    else:
        token = request.cookies.get('auth_token', '')
    
    if not token:
        return web.json_response({'error': 'Authentication required'}, status=401)
    
    try:
        # Validate JWT token
        config = request.app['config']
        jwt_secret = getattr(config.security, 'jwt_secret', 'your-secret-key')
        payload = jwt.decode(token, jwt_secret, algorithms=['HS256'])
        
        # Check if token is expired
        if datetime.utcnow() > datetime.fromtimestamp(payload['exp']):
            return web.json_response({'error': 'Token expired'}, status=401)
        
        # Store user info in request
        request['user'] = {
            'username': payload['user'],
            'role': payload['role'],
            'ip': payload.get('ip', 'unknown')
        }
        
        return await handler(request)
        
    except jwt.InvalidTokenError:
        return web.json_response({'error': 'Invalid token'}, status=401)
    except Exception as e:
        return web.json_response({'error': 'Authentication error'}, status=401)


@web.middleware
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


@web.middleware
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
        self.app['api_instance'] = self
        
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
        self.app.router.add_get('/api/auth/status', self.auth_status)
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
            import os
            # Serve the actual dashboard.html file
            dashboard_path = os.path.join(os.path.dirname(__file__), 'dashboard.html')
            if os.path.exists(dashboard_path):
                with open(dashboard_path, 'r', encoding='utf-8') as f:
                    content = f.read()
                return web.Response(text=content, content_type='text/html')
            else:
                return web.Response(
                    text="<h1>Dashboard not found</h1><p>dashboard.html file is missing</p>",
                    content_type='text/html',
                    status=404
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
        """JWT-based admin login endpoint"""
        try:
            data = await request.json()
            password = data.get('password', '')
            
            if not password:
                return web.json_response({
                    'success': False,
                    'message': 'Password required'
                }, status=400)
            
            # Simple admin password check
            if password != self.config.security.admin_password:
                return web.json_response({
                    'success': False,
                    'message': 'Invalid password'
                }, status=401)
            
            # Generate JWT token
            jwt_secret = getattr(self.config.security, 'jwt_secret', 'your-secret-key')
            payload = {
                'user': 'admin',
                'role': 'admin',
                'iat': datetime.utcnow(),
                'exp': datetime.utcnow() + timedelta(hours=24),
                'ip': request.remote or 'unknown'
            }
            
            token = jwt.encode(payload, jwt_secret, algorithm='HS256')
            
            return web.json_response({
                'success': True,
                'message': 'Login successful',
                'token': token,
                'expires_in': 24 * 3600  # 24 hours in seconds
            })
                
        except Exception as e:
            self.logger.error(f"Login error: {e}")
            return web.json_response({
                'success': False,
                'message': 'Login failed'
            }, status=500)
    
    async def logout(self, request: web.Request) -> web.Response:
        """JWT logout endpoint"""
        try:
            # For JWT, we just return success since tokens are stateless
            # In production, you might maintain a blacklist of invalidated tokens
            response = web.json_response({
                'success': True,
                'message': 'Logged out successfully'
            })
            
            # Clear cookie if present
            response.del_cookie('auth_token')
            return response
            
        except Exception as e:
            return web.json_response({'error': str(e)}, status=500)
    
    async def auth_status(self, request: web.Request) -> web.Response:
        """Check authentication status"""
        try:
            # Try to validate the token manually since this endpoint is public
            auth_header = request.headers.get('Authorization', '')
            if auth_header.startswith('Bearer '):
                token = auth_header[7:]
                try:
                    jwt_secret = getattr(self.config.security, 'jwt_secret', 'your-secret-key')
                    payload = jwt.decode(token, jwt_secret, algorithms=['HS256'])
                    
                    # Check if token is expired
                    current_time = datetime.utcnow().timestamp()
                    if current_time < payload['exp']:
                        return web.json_response({
                            'authenticated': True,
                            'user': payload['user'],
                            'role': payload['role'],
                            'expires_at': payload['exp']
                        })
                except jwt.InvalidTokenError as e:
                    self.logger.debug(f"JWT validation error: {e}")
                except Exception as e:
                    self.logger.error(f"JWT processing error: {e}")
            
            return web.json_response({
                'authenticated': False
            })
        except Exception as e:
            return web.json_response({
                'authenticated': False,
                'error': str(e)
            })
    
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
