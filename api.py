#!/usr/bin/env python3
"""
REST API for GitHub Archive Scraper
Provides endpoints for querying and monitoring the scraper
"""

import asyncio
import json
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Any
import logging
from aiohttp import web, web_request
import aiohttp_cors
from aiohttp.web_exceptions import HTTPBadRequest, HTTPNotFound, HTTPInternalServerError


class GitHubArchiveAPI:
    """REST API for GitHub Archive data"""
    
    def __init__(self, db_manager, metrics_collector=None, visualizer=None):
        self.db_manager = db_manager
        self.metrics_collector = metrics_collector
        self.visualizer = visualizer
        self.logger = logging.getLogger(__name__)
        self.app = web.Application()
        self._setup_routes()
        self._setup_cors()
    
    def _setup_cors(self):
        """Setup CORS for the API"""
        cors = aiohttp_cors.setup(self.app, defaults={
            "*": aiohttp_cors.ResourceOptions(
                allow_credentials=True,
                expose_headers="*",
                allow_headers="*",
                allow_methods="*"
            )
        })
        
        # Add CORS to all routes
        for route in list(self.app.router.routes()):
            cors.add(route)
    
    def _setup_routes(self):
        """Setup API routes"""
        # Health and status endpoints
        self.app.router.add_get('/health', self.health_check)
        self.app.router.add_get('/status', self.get_status)
        self.app.router.add_get('/metrics', self.get_metrics)
        
        # Data query endpoints
        self.app.router.add_get('/events', self.search_events)
        self.app.router.add_get('/events/stats', self.get_event_stats)
        self.app.router.add_get('/repositories', self.search_repositories)
        self.app.router.add_get('/repositories/top', self.get_top_repositories)
        self.app.router.add_get('/users/top', self.get_top_users)
        self.app.router.add_get('/languages/stats', self.get_language_stats)
        
        # Analysis endpoints
        self.app.router.add_get('/analysis/daily', self.get_daily_analysis)
        self.app.router.add_get('/analysis/trends', self.get_trends)
        self.app.router.add_get('/analysis/report', self.generate_report)
        
        # File status endpoints
        self.app.router.add_get('/files/status', self.get_file_status)
        self.app.router.add_get('/files/missing', self.get_missing_files)
    
    async def health_check(self, request: web_request.Request) -> web.Response:
        """Health check endpoint"""
        try:
            # Test database connection
            async with self.db_manager.get_connection() as conn:
                await conn.fetchval("SELECT 1")
            
            return web.json_response({
                'status': 'healthy',
                'timestamp': datetime.now().isoformat(),
                'database': 'connected'
            })
        except Exception as e:
            self.logger.error(f"Health check failed: {e}")
            return web.json_response({
                'status': 'unhealthy',
                'timestamp': datetime.now().isoformat(),
                'error': str(e)
            }, status=503)
    
    async def get_status(self, request: web_request.Request) -> web.Response:
        """Get scraper status"""
        try:
            # Get database stats
            async with self.db_manager.get_connection() as conn:
                total_events = await conn.fetchval("SELECT COUNT(*) FROM github_events")
                total_files = await conn.fetchval("SELECT COUNT(*) FROM file_downloads")
                latest_event = await conn.fetchval("SELECT MAX(created_at) FROM github_events")
                earliest_event = await conn.fetchval("SELECT MIN(created_at) FROM github_events")
            
            status = {
                'timestamp': datetime.now().isoformat(),
                'database': {
                    'total_events': total_events,
                    'total_files_processed': total_files,
                    'latest_event': latest_event.isoformat() if latest_event else None,
                    'earliest_event': earliest_event.isoformat() if earliest_event else None
                }
            }
            
            # Add metrics if available
            if self.metrics_collector:
                latest_metrics = self.metrics_collector.get_latest_metrics()
                status['metrics'] = latest_metrics
            
            return web.json_response(status)
        except Exception as e:
            self.logger.error(f"Error getting status: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_metrics(self, request: web_request.Request) -> web.Response:
        """Get performance metrics"""
        if not self.metrics_collector:
            raise HTTPNotFound(text="Metrics collector not available")
        
        try:
            hours = int(request.query.get('hours', 1))
            summary = self.metrics_collector.get_metrics_summary(hours)
            return web.json_response(summary)
        except Exception as e:
            self.logger.error(f"Error getting metrics: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def search_events(self, request: web_request.Request) -> web.Response:
        """Search GitHub events"""
        try:
            # Parse query parameters
            limit = min(int(request.query.get('limit', 100)), 1000)
            offset = int(request.query.get('offset', 0))
            event_type = request.query.get('type')
            repo_name = request.query.get('repo')
            actor_login = request.query.get('user')
            start_date = request.query.get('start_date')
            end_date = request.query.get('end_date')
            
            # Build query
            conditions = []
            params = []
            param_count = 0
            
            if event_type:
                param_count += 1
                conditions.append(f"event_type = ${param_count}")
                params.append(event_type)
            
            if repo_name:
                param_count += 1
                conditions.append(f"repo_name ILIKE ${param_count}")
                params.append(f"%{repo_name}%")
            
            if actor_login:
                param_count += 1
                conditions.append(f"actor_login ILIKE ${param_count}")
                params.append(f"%{actor_login}%")
            
            if start_date:
                param_count += 1
                conditions.append(f"created_at >= ${param_count}")
                params.append(datetime.fromisoformat(start_date))
            
            if end_date:
                param_count += 1
                conditions.append(f"created_at <= ${param_count}")
                params.append(datetime.fromisoformat(end_date))
            
            where_clause = " WHERE " + " AND ".join(conditions) if conditions else ""
            
            query = f"""
            SELECT 
                event_id, event_type, actor_login, repo_name, 
                created_at, payload
            FROM github_events
            {where_clause}
            ORDER BY created_at DESC
            LIMIT ${param_count + 1} OFFSET ${param_count + 2}
            """
            
            params.extend([limit, offset])
            
            async with self.db_manager.get_connection() as conn:
                rows = await conn.fetch(query, *params)
                events = [dict(row) for row in rows]
                
                # Get total count
                count_query = f"SELECT COUNT(*) FROM github_events{where_clause}"
                total_count = await conn.fetchval(count_query, *params[:-2])
            
            return web.json_response({
                'events': events,
                'total_count': total_count,
                'limit': limit,
                'offset': offset
            })
            
        except Exception as e:
            self.logger.error(f"Error searching events: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_event_stats(self, request: web_request.Request) -> web.Response:
        """Get event statistics"""
        try:
            days = int(request.query.get('days', 30))
            end_date = datetime.now()
            start_date = end_date - timedelta(days=days)
            
            query = """
            SELECT 
                event_type,
                COUNT(*) as count,
                COUNT(DISTINCT actor_login) as unique_users,
                COUNT(DISTINCT repo_name) as unique_repos
            FROM github_events 
            WHERE created_at >= $1 AND created_at <= $2
            GROUP BY event_type
            ORDER BY count DESC
            """
            
            async with self.db_manager.get_connection() as conn:
                rows = await conn.fetch(query, start_date, end_date)
                stats = [dict(row) for row in rows]
            
            return web.json_response({
                'period_days': days,
                'event_stats': stats,
                'generated_at': datetime.now().isoformat()
            })
            
        except Exception as e:
            self.logger.error(f"Error getting event stats: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def search_repositories(self, request: web_request.Request) -> web.Response:
        """Search repositories"""
        try:
            query_text = request.query.get('q', '')
            limit = min(int(request.query.get('limit', 50)), 500)
            
            if not query_text:
                raise HTTPBadRequest(text="Query parameter 'q' is required")
            
            query = """
            SELECT 
                repo_name,
                COUNT(*) as total_events,
                COUNT(DISTINCT actor_login) as unique_contributors,
                MAX(created_at) as last_activity,
                STRING_AGG(DISTINCT event_type, ', ') as event_types
            FROM github_events 
            WHERE repo_name ILIKE $1
            GROUP BY repo_name
            ORDER BY total_events DESC
            LIMIT $2
            """
            
            async with self.db_manager.get_connection() as conn:
                rows = await conn.fetch(query, f"%{query_text}%", limit)
                repos = [dict(row) for row in rows]
            
            return web.json_response({
                'repositories': repos,
                'query': query_text,
                'count': len(repos)
            })
            
        except Exception as e:
            self.logger.error(f"Error searching repositories: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_top_repositories(self, request: web_request.Request) -> web.Response:
        """Get top repositories by activity"""
        try:
            limit = min(int(request.query.get('limit', 50)), 100)
            days = int(request.query.get('days', 30))
            
            if self.visualizer:
                repos = await self.visualizer.generate_top_repositories(limit, days)
            else:
                # Fallback implementation
                end_date = datetime.now()
                start_date = end_date - timedelta(days=days)
                
                query = """
                SELECT 
                    repo_name,
                    COUNT(*) as total_events,
                    COUNT(DISTINCT actor_login) as unique_contributors
                FROM github_events 
                WHERE created_at >= $1 AND created_at <= $2
                AND repo_name IS NOT NULL
                GROUP BY repo_name
                ORDER BY total_events DESC
                LIMIT $3
                """
                
                async with self.db_manager.get_connection() as conn:
                    rows = await conn.fetch(query, start_date, end_date, limit)
                    repos = [dict(row) for row in rows]
            
            return web.json_response({
                'repositories': repos,
                'period_days': days,
                'limit': limit
            })
            
        except Exception as e:
            self.logger.error(f"Error getting top repositories: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_top_users(self, request: web_request.Request) -> web.Response:
        """Get top users by activity"""
        try:
            limit = min(int(request.query.get('limit', 50)), 100)
            days = int(request.query.get('days', 30))
            end_date = datetime.now()
            start_date = end_date - timedelta(days=days)
            
            query = """
            SELECT 
                actor_login,
                COUNT(*) as total_events,
                COUNT(DISTINCT repo_name) as unique_repos,
                COUNT(DISTINCT event_type) as event_types_count,
                STRING_AGG(DISTINCT event_type, ', ') as event_types
            FROM github_events 
            WHERE created_at >= $1 AND created_at <= $2
            AND actor_login IS NOT NULL
            GROUP BY actor_login
            ORDER BY total_events DESC
            LIMIT $3
            """
            
            async with self.db_manager.get_connection() as conn:
                rows = await conn.fetch(query, start_date, end_date, limit)
                users = [dict(row) for row in rows]
            
            return web.json_response({
                'users': users,
                'period_days': days,
                'limit': limit
            })
            
        except Exception as e:
            self.logger.error(f"Error getting top users: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_language_stats(self, request: web_request.Request) -> web.Response:
        """Get programming language statistics"""
        try:
            days = int(request.query.get('days', 30))
            
            if self.visualizer:
                stats = await self.visualizer.generate_programming_language_stats(days)
            else:
                # Fallback implementation
                end_date = datetime.now()
                start_date = end_date - timedelta(days=days)
                
                query = """
                SELECT 
                    payload->>'language' as language,
                    COUNT(*) as event_count
                FROM github_events 
                WHERE created_at >= $1 AND created_at <= $2
                AND payload->>'language' IS NOT NULL
                GROUP BY payload->>'language'
                ORDER BY event_count DESC
                LIMIT 25
                """
                
                async with self.db_manager.get_connection() as conn:
                    rows = await conn.fetch(query, start_date, end_date)
                    stats = {'languages': [dict(row) for row in rows]}
            
            return web.json_response({
                'language_stats': stats,
                'period_days': days
            })
            
        except Exception as e:
            self.logger.error(f"Error getting language stats: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_daily_analysis(self, request: web_request.Request) -> web.Response:
        """Get daily activity analysis"""
        try:
            days = int(request.query.get('days', 30))
            
            if self.visualizer:
                analysis = await self.visualizer.generate_daily_activity_stats(days)
            else:
                # Fallback implementation
                end_date = datetime.now()
                start_date = end_date - timedelta(days=days)
                
                query = """
                SELECT 
                    DATE(created_at) as activity_date,
                    COUNT(*) as total_events,
                    COUNT(DISTINCT actor_login) as unique_users
                FROM github_events 
                WHERE created_at >= $1 AND created_at <= $2
                GROUP BY DATE(created_at)
                ORDER BY activity_date
                """
                
                async with self.db_manager.get_connection() as conn:
                    rows = await conn.fetch(query, start_date, end_date)
                    analysis = [dict(row) for row in rows]
            
            return web.json_response({
                'daily_analysis': analysis,
                'period_days': days
            })
            
        except Exception as e:
            self.logger.error(f"Error getting daily analysis: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_trends(self, request: web_request.Request) -> web.Response:
        """Get trending repositories and topics"""
        try:
            if self.visualizer:
                trends = await self.visualizer.generate_repository_growth_trends(limit=20)
                return web.json_response({'trends': trends})
            else:
                raise HTTPNotFound(text="Trends analysis not available")
            
        except Exception as e:
            self.logger.error(f"Error getting trends: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def generate_report(self, request: web_request.Request) -> web.Response:
        """Generate comprehensive analysis report"""
        try:
            days = int(request.query.get('days', 30))
            
            if not self.visualizer:
                raise HTTPNotFound(text="Report generation not available")
            
            report = await self.visualizer.generate_comprehensive_report(days)
            return web.json_response(report)
            
        except Exception as e:
            self.logger.error(f"Error generating report: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_file_status(self, request: web_request.Request) -> web.Response:
        """Get file download status"""
        try:
            limit = min(int(request.query.get('limit', 100)), 1000)
            
            query = """
            SELECT 
                filename, file_url, etag, last_modified, 
                file_size, download_date, status
            FROM file_downloads 
            ORDER BY download_date DESC
            LIMIT $1
            """
            
            async with self.db_manager.get_connection() as conn:
                rows = await conn.fetch(query, limit)
                files = [dict(row) for row in rows]
            
            return web.json_response({
                'files': files,
                'count': len(files)
            })
            
        except Exception as e:
            self.logger.error(f"Error getting file status: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def get_missing_files(self, request: web_request.Request) -> web.Response:
        """Get list of missing files that should be downloaded"""
        try:
            days = int(request.query.get('days', 7))
            end_date = datetime.now()
            start_date = end_date - timedelta(days=days)
            
            # Generate expected file list
            expected_files = []
            current_date = start_date
            while current_date <= end_date:
                for hour in range(24):
                    filename = f"{current_date.strftime('%Y-%m-%d')}-{hour}.json.gz"
                    expected_files.append(filename)
                current_date += timedelta(days=1)
            
            # Check which files are missing
            query = "SELECT filename FROM file_downloads WHERE filename = ANY($1)"
            
            async with self.db_manager.get_connection() as conn:
                existing_rows = await conn.fetch(query, expected_files)
                existing_files = {row['filename'] for row in existing_rows}
            
            missing_files = [f for f in expected_files if f not in existing_files]
            
            return web.json_response({
                'missing_files': missing_files,
                'total_expected': len(expected_files),
                'total_missing': len(missing_files),
                'period_days': days
            })
            
        except Exception as e:
            self.logger.error(f"Error getting missing files: {e}")
            raise HTTPInternalServerError(text=str(e))
    
    async def start_server(self, host: str = '0.0.0.0', port: int = 8080):
        """Start the API server"""
        runner = web.AppRunner(self.app)
        await runner.setup()
        site = web.TCPSite(runner, host, port)
        await site.start()
        
        self.logger.info(f"GitHub Archive API server started on http://{host}:{port}")
        print(f"🚀 GitHub Archive API server started on http://{host}:{port}")
        print("📋 Available endpoints:")
        print("   GET /health - Health check")
        print("   GET /status - Scraper status")
        print("   GET /metrics - Performance metrics")
        print("   GET /events - Search events")
        print("   GET /events/stats - Event statistics")
        print("   GET /repositories - Search repositories")
        print("   GET /repositories/top - Top repositories")
        print("   GET /users/top - Top users")
        print("   GET /languages/stats - Language statistics")
        print("   GET /analysis/daily - Daily analysis")
        print("   GET /analysis/trends - Trending analysis")
        print("   GET /analysis/report - Comprehensive report")
        print("   GET /files/status - File status")
        print("   GET /files/missing - Missing files")
        
        return runner


async def main():
    """Run the API server standalone"""
    import sys
    import os
    
    # Add the current directory to the path
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    
    try:
        from config import Config
        from gharchive_scraper import DatabaseManager
        from visualizer import DataVisualizer
        from metrics import MetricsCollector
        
        config = Config()
        db_manager = DatabaseManager(config)
        await db_manager.connect()
        
        # Initialize optional components
        visualizer = DataVisualizer(db_manager)
        metrics_collector = MetricsCollector()
        
        # Create and start API
        api = GitHubArchiveAPI(db_manager, metrics_collector, visualizer)
        runner = await api.start_server(port=8080)
        
        # Keep server running
        try:
            while True:
                await asyncio.sleep(3600)  # Sleep for 1 hour
        except KeyboardInterrupt:
            print("\n🛑 Shutting down API server...")
        finally:
            await runner.cleanup()
            
    except ImportError as e:
        print(f"Import error: {e}")
        print("Make sure you're running this from the correct directory with all dependencies installed.")
    except Exception as e:
        print(f"Error: {e}")


if __name__ == '__main__':
    asyncio.run(main())
