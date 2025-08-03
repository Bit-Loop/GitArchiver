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
        api_instance = request.app['api_instance']
        jwt_secret = api_instance._get_jwt_secret()
        payload = jwt.decode(token, jwt_secret, algorithms=['HS256'])
        
        # Check if token is expired
        if datetime.utcnow() > datetime.fromtimestamp(payload['exp']):
            return web.json_response({'error': 'Token expired'}, status=401)
        
        # Create a session-like object for compatibility
        from core.auth import UserSession
        import time
        
        session = UserSession(
            session_id=token[:16],  # Use first 16 chars of token as session ID
            user_id='admin',
            username=payload['user'],
            permissions={'admin'},  # Admin permissions
            created_at=payload['iat'],
            expires_at=payload['exp'],
            last_activity=time.time(),
            ip_address=payload.get('ip', 'unknown')
        )
        
        # Store session in request for endpoint compatibility
        request['session'] = session
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
        # Initialize resource monitoring (placeholder for now)
        self.resource_monitor = self._create_mock_resource_monitor()
        self.scraper = None  # Will be initialized after database connection
        
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
            try:
                from enhanced_scraper import GitHubArchiveScraper
                self.scraper = GitHubArchiveScraper(self.config)
                await self.scraper.initialize()
                self.logger.info("Enhanced scraper initialized successfully")
            except ImportError as e:
                self.logger.warning(f"Scraper module not available: {e}")
                self.scraper = None
            except Exception as e:
                self.logger.error(f"Scraper initialization failed: {e}")
                self.scraper = None
            
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
            # Stop scraper if running
            if self.scraper:
                await self.scraper.shutdown()
            await self.db.disconnect()
            self.logger.info("Professional API shutdown complete")
        except Exception as e:
            self.logger.error(f"Shutdown error: {e}")
    
    def _create_mock_resource_monitor(self):
        """Create a mock resource monitor for testing"""
        class MockResourceMonitor:
            def get_resource_status(self):
                import psutil
                memory = psutil.virtual_memory()
                disk = psutil.disk_usage('/')
                return {
                    'memory': {
                        'used_gb': round(memory.used / (1024**3), 2),
                        'limit_gb': 18.0,
                        'percent': round(memory.percent, 1),
                        'warning': False
                    },
                    'disk': {
                        'used_gb': round(disk.used / (1024**3), 2),
                        'limit_gb': 40.0,
                        'percent': round((disk.used / disk.total) * 100, 1),
                        'warning': False
                    },
                    'cpu': {
                        'percent': round(psutil.cpu_percent(), 1),
                        'limit_percent': 80,
                        'warning': False
                    },
                    'emergency_mode': False,
                    'emergency_conditions': [],
                    'timestamp': datetime.now().isoformat()
                }
            
            async def emergency_cleanup(self):
                return {
                    'success': True,
                    'actions': ['Mock cleanup completed'],
                    'timestamp': datetime.now().isoformat()
                }
        
        return MockResourceMonitor()
    
    def _get_jwt_secret(self) -> str:
        """Get JWT secret from config consistently"""
        return self.config.security.jwt_secret
    
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
        
        # Data management endpoints (require authentication)
        self.app.router.add_post('/api/data/prune', self.prune_data)
        self.app.router.add_post('/api/data/remove-repositories', self.remove_repositories)
        self.app.router.add_get('/api/system/disk-usage', self.get_disk_usage)
        
        # Search endpoints
        self.app.router.add_post('/api/search/events', self.search_events)
        self.app.router.add_post('/api/search/repositories', self.search_repositories)
        
        # Scraper control endpoints (require admin permission)
        self.app.router.add_post('/api/scraper/start', self.start_scraper)
        self.app.router.add_post('/api/scraper/stop', self.stop_scraper)
        self.app.router.add_get('/api/scraper/status', self.get_scraper_status)
        self.app.router.add_get('/api/scraper/logs', self.get_scraper_logs)
        
        # Dashboard-compatible endpoints (aliases for backward compatibility)
        self.app.router.add_post('/api/start-scraper', self.start_scraper)
        self.app.router.add_post('/api/stop-scraper', self.stop_scraper)
        self.app.router.add_get('/api/scraper-status', self.get_scraper_status)
        self.app.router.add_get('/api/scraper-logs', self.get_scraper_logs)
        
        # Wordlist generation endpoints
        self.app.router.add_post('/api/wordlists/generate', self.generate_comprehensive_wordlists)
        self.app.router.add_post('/api/wordlists/targeted', self.generate_targeted_wordlist)
        
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
            # if self.scraper:
            #     scraper_status = await self.scraper.get_status()
            
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
            jwt_secret = self._get_jwt_secret()
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
                    jwt_secret = self._get_jwt_secret()
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
        try:
            session: UserSession = request['session']
            # Only admin can control scraper
            if not session.has_permission('admin'):
                return web.json_response({'error': 'Admin permission required'}, status=403)
            
            # Log the action
            self.auth._audit_log('SCRAPER_START_REQUESTED', session.username, 'Scraper start requested')
            
            # Check if scraper is available
            if not self.scraper:
                return web.json_response({
                    'success': False,
                    'message': 'Scraper module not available',
                    'timestamp': datetime.now().isoformat()
                }, status=503)
            
            # Parse request parameters
            data = await request.json() if request.can_read_body else {}
            hours_back = data.get('hours_back', 24)  # Default: last 24 hours
            
            # Start the scraper
            result = await self.scraper.start(hours_back=hours_back)
            
            if result['success']:
                self.auth._audit_log('SCRAPER_STARTED', session.username, f'Scraper started successfully (hours_back={hours_back})')
            else:
                self.auth._audit_log('SCRAPER_START_FAILED', session.username, f'Scraper start failed: {result.get("message", "Unknown error")}')
            
            return web.json_response(result)
            
        except Exception as e:
            self.logger.error(f"Start scraper error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def stop_scraper(self, request: web.Request) -> web.Response:
        """Stop scraper"""
        try:
            session: UserSession = request['session']
            # Only admin can control scraper
            if not session.has_permission('admin'):
                return web.json_response({'error': 'Admin permission required'}, status=403)
            
            # Log the action
            self.auth._audit_log('SCRAPER_STOP_REQUESTED', session.username, 'Scraper stop requested')
            
            # Check if scraper is available
            if not self.scraper:
                return web.json_response({
                    'success': False,
                    'message': 'Scraper module not available',
                    'timestamp': datetime.now().isoformat()
                }, status=503)
            
            # Stop the scraper
            result = await self.scraper.stop()
            
            if result['success']:
                self.auth._audit_log('SCRAPER_STOPPED', session.username, 'Scraper stopped successfully')
            else:
                self.auth._audit_log('SCRAPER_STOP_FAILED', session.username, f'Scraper stop failed: {result.get("message", "Unknown error")}')
            
            return web.json_response(result)
            
        except Exception as e:
            self.logger.error(f"Stop scraper error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_scraper_status(self, request: web.Request) -> web.Response:
        """Get scraper status"""
        try:
            session: UserSession = request['session']
            
            # Check if scraper is available
            if not self.scraper:
                return web.json_response({
                    'running': False,
                    'status': 'not_available',
                    'message': 'Scraper module not available',
                    'last_run': None,
                    'events_processed': 0,
                    'timestamp': datetime.now().isoformat()
                })
            
            # Get actual scraper status
            status = await self.scraper.get_status()
            return web.json_response(status)
            
        except Exception as e:
            self.logger.error(f"Get scraper status error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_scraper_logs(self, request: web.Request) -> web.Response:
        """Get scraper logs"""
        try:
            session: UserSession = request['session']
            
            # Check if scraper is available
            if not self.scraper:
                return web.json_response({
                    'logs': 'Scraper module not available - no logs to display',
                    'timestamp': datetime.now().isoformat()
                })
            
            # Parse query parameters
            lines = int(request.query.get('lines', '100'))
            lines = min(lines, 1000)  # Cap at 1000 lines
            
            # Get actual scraper logs
            logs = await self.scraper.get_logs(lines=lines)
            
            return web.json_response({
                'logs': logs,
                'lines_requested': lines,
                'timestamp': datetime.now().isoformat()
            })
            
        except Exception as e:
            self.logger.error(f"Get scraper logs error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_audit_log(self, request: web.Request) -> web.Response:
        """Get audit log"""
        return web.json_response({'message': 'Audit log endpoint - Coming soon'})
    
    # Data management endpoints
    async def prune_data(self, request: web.Request) -> web.Response:
        """Prune old data and orphaned records (requires authentication)"""
        try:
            session: UserSession = request['session']
            # Session is guaranteed to exist due to auth middleware
            
            # Parse request parameters
            data = await request.json() if request.can_read_body else {}
            days_old = data.get('days_old', 30)  # Default: prune data older than 30 days
            dry_run = data.get('dry_run', False)  # Default: actually perform the pruning
            
            # Log the action
            self.auth._audit_log('DATA_PRUNE_STARTED', session.username, 
                               f'Data pruning initiated (days_old={days_old}, dry_run={dry_run})')
            
            # Implement actual data pruning logic
            pruning_actions = []
            deleted_counts = {}
            
            # 1. Remove events with corrupted data (numeric types)
            corrupted_query = """
                SELECT COUNT(*) FROM github_events 
                WHERE event_type ~ '^[0-9]+$' OR event_type IS NULL OR event_type = ''
            """
            corrupted_count = await self.db.execute_query(corrupted_query, fetch_method='fetchval')
            
            if corrupted_count > 0:
                if not dry_run:
                    delete_corrupted = """
                        DELETE FROM github_events 
                        WHERE event_type ~ '^[0-9]+$' OR event_type IS NULL OR event_type = ''
                    """
                    await self.db.execute_query(delete_corrupted, fetch_method='execute')
                
                deleted_counts['corrupted_events'] = corrupted_count
                pruning_actions.append(f"Removed {corrupted_count:,} corrupted events")
            
            # 2. Remove old events beyond retention period
            cutoff_date = datetime.now() - timedelta(days=days_old)
            old_events_query = """
                SELECT COUNT(*) FROM github_events 
                WHERE event_created_at < $1
            """
            old_events_count = await self.db.execute_query(old_events_query, cutoff_date, fetch_method='fetchval')
            
            if old_events_count > 0:
                if not dry_run:
                    delete_old = """
                        DELETE FROM github_events 
                        WHERE event_created_at < $1
                    """
                    await self.db.execute_query(delete_old, cutoff_date, fetch_method='execute')
                
                deleted_counts['old_events'] = old_events_count
                pruning_actions.append(f"Removed {old_events_count:,} events older than {days_old} days")
            
            # 3. Remove orphaned processed files entries
            orphaned_files_query = """
                SELECT COUNT(*) FROM processed_files 
                WHERE processed_at < $1
            """
            orphaned_files_count = await self.db.execute_query(orphaned_files_query, cutoff_date, fetch_method='fetchval')
            
            if orphaned_files_count > 0:
                if not dry_run:
                    delete_orphaned = """
                        DELETE FROM processed_files 
                        WHERE processed_at < $1
                    """
                    await self.db.execute_query(delete_orphaned, cutoff_date, fetch_method='execute')
                
                deleted_counts['orphaned_files'] = orphaned_files_count
                pruning_actions.append(f"Removed {orphaned_files_count:,} orphaned processed file entries")
            
            # 4. Clean up temporary and log files
            import os
            import glob
            temp_files_cleaned = 0
            log_files_cleaned = 0
            
            if not dry_run:
                # Clean temporary files
                temp_patterns = ['/tmp/gharchive_*', '/tmp/*.gz.tmp', './temp_*']
                for pattern in temp_patterns:
                    for temp_file in glob.glob(pattern):
                        try:
                            if os.path.isfile(temp_file) and (time.time() - os.path.getctime(temp_file)) > (24 * 3600):
                                os.remove(temp_file)
                                temp_files_cleaned += 1
                        except Exception:
                            pass
                
                # Clean old log files
                log_patterns = ['./logs/*.log.old', './logs/*.log.[0-9]*']
                for pattern in log_patterns:
                    for log_file in glob.glob(pattern):
                        try:
                            if os.path.isfile(log_file) and (time.time() - os.path.getctime(log_file)) > (7 * 24 * 3600):
                                os.remove(log_file)
                                log_files_cleaned += 1
                        except Exception:
                            pass
            
            if temp_files_cleaned > 0:
                pruning_actions.append(f"Cleaned {temp_files_cleaned} temporary files")
            if log_files_cleaned > 0:
                pruning_actions.append(f"Cleaned {log_files_cleaned} old log files")
            
            # 5. Vacuum and analyze database if significant deletions occurred
            total_deleted = sum(deleted_counts.values())
            if total_deleted > 1000 and not dry_run:
                try:
                    # Note: VACUUM cannot be run in transaction, so we use a separate connection
                    async with self.db.get_connection() as conn:
                        await conn.execute("VACUUM ANALYZE github_events")
                        await conn.execute("VACUUM ANALYZE processed_files")
                    pruning_actions.append("Optimized database tables with VACUUM ANALYZE")
                except Exception as e:
                    self.logger.warning(f"Database optimization failed: {e}")
            
            # Prepare result
            result = {
                'success': True,
                'dry_run': dry_run,
                'summary': f"Data pruning {'simulation' if dry_run else 'completed'} successfully",
                'actions': pruning_actions if pruning_actions else ['No data needed pruning'],
                'deleted_counts': deleted_counts,
                'total_deleted': total_deleted,
                'parameters': {
                    'days_old': days_old,
                    'cutoff_date': cutoff_date.isoformat()
                },
                'timestamp': datetime.now().isoformat()
            }
            
            action_summary = f"Data pruning {'simulated' if dry_run else 'completed'}: {total_deleted:,} records affected"
            self.auth._audit_log('DATA_PRUNE_COMPLETED', session.username, action_summary)
            return web.json_response(result)
            
        except Exception as e:
            self.logger.error(f"Data pruning error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def remove_repositories(self, request: web.Request) -> web.Response:
        """Remove selected repositories and their events (requires authentication)"""
        try:
            session: UserSession = request['session']
            # Session is guaranteed to exist due to auth middleware
            
            data = await request.json()
            repository_ids = data.get('repository_ids', [])
            repository_names = data.get('repository_names', [])  # Support removal by name too
            dry_run = data.get('dry_run', False)
            
            if not repository_ids and not repository_names:
                return web.json_response({'error': 'No repository IDs or names provided'}, status=400)
            
            # Log the action
            target_count = len(repository_ids) + len(repository_names)
            self.auth._audit_log('REPOSITORIES_REMOVAL_STARTED', session.username, 
                               f'Repository removal initiated for {target_count} repositories (dry_run={dry_run})')
            
            # Implement actual repository removal logic
            removal_actions = []
            removed_details = {
                'repositories': [],
                'events_deleted': 0,
                'total_repositories_affected': 0
            }
            
            # Build the WHERE clause for repository selection
            where_conditions = []
            query_params = []
            param_counter = 1
            
            if repository_ids:
                # Convert string IDs to integers if needed
                valid_repo_ids = []
                for repo_id in repository_ids:
                    try:
                        valid_repo_ids.append(int(repo_id))
                    except (ValueError, TypeError):
                        self.logger.warning(f"Invalid repository ID: {repo_id}")
                
                if valid_repo_ids:
                    placeholders = ','.join([f'${i + param_counter}' for i in range(len(valid_repo_ids))])
                    where_conditions.append(f"repo_id IN ({placeholders})")
                    query_params.extend(valid_repo_ids)
                    param_counter += len(valid_repo_ids)
            
            if repository_names:
                placeholders = ','.join([f'${i + param_counter}' for i in range(len(repository_names))])
                where_conditions.append(f"repo_full_name IN ({placeholders})")
                query_params.extend(repository_names)
                param_counter += len(repository_names)
            
            if not where_conditions:
                return web.json_response({'error': 'No valid repository identifiers provided'}, status=400)
            
            where_clause = ' OR '.join(where_conditions)
            
            # First, get information about repositories that will be affected
            repo_info_query = f"""
                SELECT DISTINCT repo_id, repo_name, repo_full_name, COUNT(*) as event_count
                FROM github_events 
                WHERE {where_clause}
                GROUP BY repo_id, repo_name, repo_full_name
                ORDER BY event_count DESC
            """
            
            affected_repos = await self.db.execute_query(repo_info_query, *query_params)
            
            if not affected_repos:
                return web.json_response({
                    'success': True,
                    'message': 'No repositories found matching the provided criteria',
                    'removed_count': 0,
                    'timestamp': datetime.now().isoformat()
                })
            
            total_events_to_delete = sum(repo['event_count'] for repo in affected_repos)
            removed_details['total_repositories_affected'] = len(affected_repos)
            
            # Collect repository details
            for repo in affected_repos:
                removed_details['repositories'].append({
                    'id': repo['repo_id'],
                    'name': repo['repo_name'],
                    'full_name': repo['repo_full_name'],
                    'events_count': repo['event_count']
                })
            
            if not dry_run:
                # Perform the actual deletion
                delete_query = f"""
                    DELETE FROM github_events 
                    WHERE {where_clause}
                """
                
                delete_result = await self.db.execute_query(delete_query, *query_params, fetch_method='execute')
                
                # Extract the number of deleted rows from the result
                # PostgreSQL returns something like "DELETE 1234"
                if isinstance(delete_result, str) and delete_result.startswith('DELETE '):
                    removed_details['events_deleted'] = int(delete_result.split(' ')[1])
                else:
                    removed_details['events_deleted'] = total_events_to_delete  # Fallback estimate
                
                removal_actions.append(f"Deleted {removed_details['events_deleted']:,} events from {len(affected_repos)} repositories")
                
                # Clean up any orphaned processed file entries related to these repositories
                # This is a best-effort cleanup since processed_files doesn't directly reference repositories
                try:
                    cleanup_query = """
                        DELETE FROM processed_files 
                        WHERE event_count = 0 AND processed_at < NOW() - INTERVAL '7 days'
                    """
                    cleanup_result = await self.db.execute_query(cleanup_query, fetch_method='execute')
                    if isinstance(cleanup_result, str) and cleanup_result.startswith('DELETE '):
                        cleaned_files = int(cleanup_result.split(' ')[1])
                        if cleaned_files > 0:
                            removal_actions.append(f"Cleaned up {cleaned_files} orphaned processed file entries")
                except Exception as e:
                    self.logger.warning(f"Failed to clean up processed files: {e}")
                
                # Optimize database if significant deletions occurred
                if removed_details['events_deleted'] > 1000:
                    try:
                        async with self.db.get_connection() as conn:
                            await conn.execute("VACUUM ANALYZE github_events")
                        removal_actions.append("Optimized database tables after deletion")
                    except Exception as e:
                        self.logger.warning(f"Database optimization failed: {e}")
            else:
                # Dry run - just report what would be deleted
                removed_details['events_deleted'] = total_events_to_delete
                removal_actions.append(f"Would delete {total_events_to_delete:,} events from {len(affected_repos)} repositories")
            
            # Prepare result
            result = {
                'success': True,
                'dry_run': dry_run,
                'removed_count': removed_details['total_repositories_affected'],
                'events_deleted': removed_details['events_deleted'],
                'message': f"Successfully {'simulated removal of' if dry_run else 'removed'} {len(affected_repos)} repositories and their associated events",
                'actions': removal_actions,
                'details': removed_details,
                'timestamp': datetime.now().isoformat()
            }
            
            action_summary = f"Repository removal {'simulated' if dry_run else 'completed'}: {len(affected_repos)} repositories, {removed_details['events_deleted']:,} events"
            self.auth._audit_log('REPOSITORIES_REMOVAL_COMPLETED', session.username, action_summary)
            return web.json_response(result)
            
        except Exception as e:
            self.logger.error(f"Repository removal error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_disk_usage(self, request: web.Request) -> web.Response:
        """Get disk usage analysis (requires authentication)"""
        try:
            session: UserSession = request['session']
            # Session is guaranteed to exist due to auth middleware
            
            import psutil
            import os
            
            # Get disk usage for root partition
            disk_usage = psutil.disk_usage('/')
            
            # Convert to GB
            total_gb = round(disk_usage.total / (1024**3), 2)
            used_gb = round(disk_usage.used / (1024**3), 2)
            free_gb = round(disk_usage.free / (1024**3), 2)
            percent = round((disk_usage.used / disk_usage.total) * 100, 1)
            
            # Get directory breakdown for common directories
            directories = []
            common_dirs = [
                '/home/ubuntu/GitArchiver/gharchive_data',
                '/home/ubuntu/GitArchiver/logs',
                '/home/ubuntu/GitArchiver/reports',
                '/tmp'
            ]
            
            for dir_path in common_dirs:
                if os.path.exists(dir_path):
                    try:
                        total_size = 0
                        for dirpath, dirnames, filenames in os.walk(dir_path):
                            for filename in filenames:
                                filepath = os.path.join(dirpath, filename)
                                try:
                                    total_size += os.path.getsize(filepath)
                                except (OSError, FileNotFoundError):
                                    pass
                        
                        if total_size > 0:
                            directories.append({
                                'path': dir_path,
                                'size_gb': round(total_size / (1024**3), 3)
                            })
                    except Exception:
                        pass
            
            result = {
                'success': True,
                'usage': {
                    'total_gb': total_gb,
                    'used_gb': used_gb,
                    'free_gb': free_gb,
                    'percent': percent,
                    'directories': directories
                },
                'timestamp': datetime.now().isoformat()
            }
            
            return web.json_response(result)
            
        except Exception as e:
            self.logger.error(f"Disk usage analysis error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    # Wordlist generation endpoints
    async def generate_comprehensive_wordlists(self, request: web.Request) -> web.Response:
        """Generate comprehensive wordlists for bug bounty hunting"""
        try:
            session: UserSession = request['session']
            
            # Log the action
            self.auth._audit_log('WORDLIST_GENERATION_REQUESTED', session.username, 'Comprehensive wordlist generation requested')
            
            data = await request.json()
            target_domains = data.get('target_domains', [])
            technologies = data.get('technologies', [])
            days_back = data.get('days_back', 30)
            max_words = data.get('max_words', 10000)
            
            # For now, return a mock response since wordlist generation is not implemented
            result = {
                'success': True,
                'message': 'Wordlist generation initiated (placeholder - feature not available)',
                'task_id': f'wordlist_{int(time.time())}',
                'parameters': {
                    'target_domains': target_domains,
                    'technologies': technologies,
                    'days_back': days_back,
                    'max_words': max_words
                },
                'estimated_completion': '5-10 minutes',
                'timestamp': datetime.now().isoformat()
            }
            
            return web.json_response(result)
            
        except Exception as e:
            self.logger.error(f"Comprehensive wordlist generation error: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def generate_targeted_wordlist(self, request: web.Request) -> web.Response:
        """Generate targeted wordlist for specific domains"""
        try:
            session: UserSession = request['session']
            
            # Log the action
            self.auth._audit_log('TARGETED_WORDLIST_REQUESTED', session.username, 'Targeted wordlist generation requested')
            
            data = await request.json()
            target_domain = data.get('target_domain', '')
            wordlist_type = data.get('type', 'comprehensive')
            
            # For now, return a mock response since wordlist generation is not implemented
            result = {
                'success': True,
                'message': 'Targeted wordlist generation initiated (placeholder - feature not available)',
                'task_id': f'targeted_{int(time.time())}',
                'parameters': {
                    'target_domain': target_domain,
                    'type': wordlist_type
                },
                'estimated_completion': '2-5 minutes',
                'timestamp': datetime.now().isoformat()
            }
            
            return web.json_response(result)
            
        except Exception as e:
            self.logger.error(f"Targeted wordlist generation error: {e}")
            return web.json_response({'error': str(e)}, status=500)


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
