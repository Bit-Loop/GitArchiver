#!/usr/bin/env python3
"""
Web API for GitHub Archive Scraper with Management Dashboard
Provides REST API and serves the dashboard on port 8080
"""

import asyncio
import json
import logging
import os
import signal
import subprocess
import sys
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional

import aiohttp
from aiohttp import web
import aiohttp_cors
import asyncpg
import psutil
from dateutil.parser import parse as parse_date

# Import our config and database manager
from config import Config

class DatabaseManager:
    """Simplified database manager for the web API"""
    
    def __init__(self, config: Config):
        self.config = config
        self.pool = None
        
    async def connect(self):
        """Create database connection pool"""
        dsn = f"postgresql://{self.config.DB_USER}:{self.config.DB_PASSWORD}@{self.config.DB_HOST}:{self.config.DB_PORT}/{self.config.DB_NAME}"
        
        try:
            self.pool = await asyncpg.create_pool(
                dsn, 
                min_size=2, 
                max_size=10,
                command_timeout=60
            )
            logging.info("Database connected successfully")
        except Exception as e:
            logging.error(f"Database connection failed: {e}")
            raise
        
    async def disconnect(self):
        """Close database connection pool"""
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

class WebAPI:
    """Web API server for GitHub Archive Scraper"""
    
    def __init__(self, config: Config, port: int = 8080):
        self.config = config
        self.port = port
        self.db = DatabaseManager(config)
        self.app = None
        self.scraper_process = None
        
        # Setup logging
        logging.basicConfig(level=logging.INFO)
        self.logger = logging.getLogger(__name__)
        
    async def init_database(self):
        """Initialize database connection"""
        await self.db.connect()
        
    async def cleanup(self):
        """Cleanup resources"""
        if self.db:
            await self.db.disconnect()
            
    def setup_routes(self):
        """Setup API routes"""
        self.app = web.Application()
        
        # Setup CORS for external access
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
        self.app.router.add_get('/api/events', self.get_events)
        self.app.router.add_get('/api/repositories', self.get_repositories)
        self.app.router.add_post('/api/search', self.search_events)
        self.app.router.add_get('/api/recent-activity', self.get_recent_activity)
        
        # Management routes
        self.app.router.add_post('/api/start-scraper', self.start_scraper)
        self.app.router.add_post('/api/stop-scraper', self.stop_scraper)
        self.app.router.add_post('/api/restart-scraper', self.restart_scraper)
        self.app.router.add_get('/api/scraper-logs', self.get_scraper_logs)
        self.app.router.add_post('/api/shutdown', self.shutdown_server)
        
        # Serve dashboard
        self.app.router.add_get('/', self.serve_dashboard)
        self.app.router.add_get('/dashboard', self.serve_dashboard)
        
        # Add CORS to all routes
        for route in list(self.app.router.routes()):
            cors.add(route)
    
    async def serve_dashboard(self, request):
        """Serve the dashboard HTML"""
        dashboard_html = """
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>GitHub Archive Scraper Dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { 
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; 
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            color: #333;
        }
        .container { 
            max-width: 1200px; 
            margin: 0 auto; 
            padding: 20px; 
        }
        .header {
            background: rgba(255,255,255,0.95);
            padding: 20px;
            border-radius: 10px;
            margin-bottom: 20px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
        }
        .header h1 {
            color: #2c3e50;
            margin-bottom: 10px;
        }
        .status-bar {
            display: flex;
            gap: 15px;
            align-items: center;
            flex-wrap: wrap;
        }
        .status-indicator {
            padding: 5px 15px;
            border-radius: 20px;
            font-weight: bold;
            font-size: 12px;
        }
        .status-running { background: #2ecc71; color: white; }
        .status-stopped { background: #e74c3c; color: white; }
        .status-unknown { background: #95a5a6; color: white; }
        
        .controls {
            background: rgba(255,255,255,0.95);
            padding: 20px;
            border-radius: 10px;
            margin-bottom: 20px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
        }
        .controls h2 {
            color: #2c3e50;
            margin-bottom: 15px;
        }
        .button-group {
            display: flex;
            gap: 10px;
            flex-wrap: wrap;
        }
        button {
            padding: 10px 20px;
            border: none;
            border-radius: 5px;
            cursor: pointer;
            font-weight: bold;
            transition: all 0.3s;
        }
        .btn-primary { background: #3498db; color: white; }
        .btn-success { background: #2ecc71; color: white; }
        .btn-warning { background: #f39c12; color: white; }
        .btn-danger { background: #e74c3c; color: white; }
        button:hover { transform: translateY(-2px); box-shadow: 0 4px 8px rgba(0,0,0,0.2); }
        button:disabled { opacity: 0.6; cursor: not-allowed; transform: none; }
        
        .dashboard-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 20px;
        }
        .card {
            background: rgba(255,255,255,0.95);
            padding: 20px;
            border-radius: 10px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
        }
        .card h3 {
            color: #2c3e50;
            margin-bottom: 15px;
            border-bottom: 2px solid #3498db;
            padding-bottom: 5px;
        }
        
        .stats-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 15px;
        }
        .stat-item {
            text-align: center;
            padding: 15px;
            background: #f8f9fa;
            border-radius: 8px;
        }
        .stat-number {
            font-size: 24px;
            font-weight: bold;
            color: #3498db;
        }
        .stat-label {
            font-size: 12px;
            color: #666;
            margin-top: 5px;
        }
        
        .search-section {
            margin-bottom: 20px;
        }
        .search-form {
            display: flex;
            gap: 10px;
            margin-bottom: 15px;
            flex-wrap: wrap;
        }
        .search-form input, .search-form select {
            padding: 10px;
            border: 1px solid #ddd;
            border-radius: 5px;
            flex: 1;
            min-width: 150px;
        }
        
        .data-table {
            width: 100%;
            border-collapse: collapse;
            background: white;
            border-radius: 8px;
            overflow: hidden;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .data-table th {
            background: #3498db;
            color: white;
            padding: 12px;
            text-align: left;
        }
        .data-table td {
            padding: 10px 12px;
            border-bottom: 1px solid #eee;
        }
        .data-table tr:hover {
            background: #f8f9fa;
        }
        
        .logs-container {
            background: #2c3e50;
            color: #ecf0f1;
            padding: 15px;
            border-radius: 8px;
            font-family: 'Courier New', monospace;
            max-height: 400px;
            overflow-y: auto;
            white-space: pre-wrap;
        }
        
        .loading {
            text-align: center;
            padding: 20px;
            font-style: italic;
            color: #666;
        }
        
        .error {
            background: #e74c3c;
            color: white;
            padding: 10px;
            border-radius: 5px;
            margin: 10px 0;
        }
        
        .success {
            background: #2ecc71;
            color: white;
            padding: 10px;
            border-radius: 5px;
            margin: 10px 0;
        }
        
        @media (max-width: 768px) {
            .dashboard-grid {
                grid-template-columns: 1fr;
            }
            .search-form {
                flex-direction: column;
            }
            .search-form input, .search-form select {
                width: 100%;
            }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🐙 GitHub Archive Scraper Dashboard</h1>
            <div class="status-bar">
                <div class="status-indicator status-unknown" id="scraperStatus">Checking...</div>
                <div class="status-indicator status-unknown" id="dbStatus">Database: Checking...</div>
                <div id="lastUpdate">Last update: Never</div>
            </div>
        </div>

        <div class="controls">
            <h2>🎛️ Scraper Management</h2>
            <div class="button-group">
                <button class="btn-success" onclick="startScraper()" id="startBtn">▶️ Start Scraper</button>
                <button class="btn-warning" onclick="stopScraper()" id="stopBtn">⏹️ Stop Scraper</button>
                <button class="btn-primary" onclick="restartScraper()" id="restartBtn">🔄 Restart Scraper</button>
                <button class="btn-danger" onclick="shutdownServer()" id="shutdownBtn">🛑 Shutdown Server</button>
                <button class="btn-primary" onclick="refreshData()">🔄 Refresh Data</button>
            </div>
            <div id="operationStatus"></div>
        </div>

        <div class="dashboard-grid">
            <div class="card">
                <h3>📊 Statistics</h3>
                <div class="stats-grid" id="statsGrid">
                    <div class="loading">Loading stats...</div>
                </div>
            </div>

            <div class="card">
                <h3>🏃 Recent Activity</h3>
                <div id="recentActivity">
                    <div class="loading">Loading recent activity...</div>
                </div>
            </div>
        </div>

        <div class="card search-section">
            <h3>🔍 Search Events</h3>
            <div class="search-form">
                <input type="text" id="searchRepo" placeholder="Repository name (e.g. torvalds/linux)">
                <input type="text" id="searchActor" placeholder="Actor login (e.g. octocat)">
                <select id="searchType">
                    <option value="">All event types</option>
                    <option value="PushEvent">Push Events</option>
                    <option value="CreateEvent">Create Events</option>
                    <option value="IssuesEvent">Issues Events</option>
                    <option value="PullRequestEvent">Pull Request Events</option>
                    <option value="WatchEvent">Watch Events</option>
                    <option value="ForkEvent">Fork Events</option>
                </select>
                <input type="date" id="searchDateFrom" placeholder="From date">
                <input type="date" id="searchDateTo" placeholder="To date">
                <button class="btn-primary" onclick="searchEvents()">🔍 Search</button>
            </div>
            <div id="searchResults">
                <div class="loading">Enter search criteria above</div>
            </div>
        </div>

        <div class="card">
            <h3>📋 Scraper Logs</h3>
            <div class="button-group" style="margin-bottom: 10px;">
                <button class="btn-primary" onclick="refreshLogs()">🔄 Refresh Logs</button>
                <button class="btn-warning" onclick="clearLogs()">🗑️ Clear Display</button>
            </div>
            <div class="logs-container" id="logsContainer">
                <div class="loading">Loading logs...</div>
            </div>
        </div>
    </div>

    <script>
        const API_BASE = window.location.origin;
        let refreshInterval;

        // Auto-refresh every 30 seconds
        function startAutoRefresh() {
            refreshInterval = setInterval(refreshData, 30000);
        }

        function stopAutoRefresh() {
            if (refreshInterval) {
                clearInterval(refreshInterval);
            }
        }

        // Initialize dashboard
        document.addEventListener('DOMContentLoaded', function() {
            refreshData();
            startAutoRefresh();
        });

        async function apiCall(endpoint, options = {}) {
            try {
                const response = await fetch(`${API_BASE}/api/${endpoint}`, {
                    headers: {
                        'Content-Type': 'application/json',
                        ...options.headers
                    },
                    ...options
                });

                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
                }

                return await response.json();
            } catch (error) {
                console.error(`API call failed: ${endpoint}`, error);
                throw error;
            }
        }

        async function refreshData() {
            await Promise.all([
                updateStatus(),
                updateStats(),
                updateRecentActivity()
            ]);
            document.getElementById('lastUpdate').textContent = `Last update: ${new Date().toLocaleTimeString()}`;
        }

        async function updateStatus() {
            try {
                const status = await apiCall('status');
                
                const scraperStatus = document.getElementById('scraperStatus');
                const dbStatus = document.getElementById('dbStatus');
                
                // Update scraper status
                scraperStatus.textContent = `Scraper: ${status.scraper_running ? 'Running' : 'Stopped'}`;
                scraperStatus.className = `status-indicator ${status.scraper_running ? 'status-running' : 'status-stopped'}`;
                
                // Update database status
                dbStatus.textContent = `Database: ${status.database_connected ? 'Connected' : 'Disconnected'}`;
                dbStatus.className = `status-indicator ${status.database_connected ? 'status-running' : 'status-stopped'}`;
                
                // Update button states
                document.getElementById('startBtn').disabled = status.scraper_running;
                document.getElementById('stopBtn').disabled = !status.scraper_running;
                
            } catch (error) {
                showError('Failed to get status: ' + error.message);
            }
        }

        async function updateStats() {
            try {
                const stats = await apiCall('stats');
                const statsGrid = document.getElementById('statsGrid');
                
                statsGrid.innerHTML = `
                    <div class="stat-item">
                        <div class="stat-number">${(stats.total_events || 0).toLocaleString()}</div>
                        <div class="stat-label">Total Events</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-number">${(stats.total_repositories || 0).toLocaleString()}</div>
                        <div class="stat-label">Repositories</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-number">${(stats.processed_files || 0).toLocaleString()}</div>
                        <div class="stat-label">Files Processed</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-number">${stats.latest_event_date || 'N/A'}</div>
                        <div class="stat-label">Latest Event</div>
                    </div>
                `;
            } catch (error) {
                document.getElementById('statsGrid').innerHTML = '<div class="error">Failed to load stats</div>';
            }
        }

        async function updateRecentActivity() {
            try {
                const activity = await apiCall('recent-activity');
                const container = document.getElementById('recentActivity');
                
                if (activity.length === 0) {
                    container.innerHTML = '<div class="loading">No recent activity</div>';
                    return;
                }
                
                container.innerHTML = activity.slice(0, 5).map(event => `
                    <div style="padding: 8px; border-bottom: 1px solid #eee; font-size: 14px;">
                        <strong>${event.type}</strong> by ${event.actor_login || 'Unknown'}<br>
                        <small>${event.repo_name || 'Unknown repo'} - ${new Date(event.created_at).toLocaleString()}</small>
                    </div>
                `).join('');
                
            } catch (error) {
                document.getElementById('recentActivity').innerHTML = '<div class="error">Failed to load activity</div>';
            }
        }

        async function startScraper() {
            try {
                showOperation('Starting scraper...');
                await apiCall('start-scraper', { method: 'POST' });
                showSuccess('Scraper started successfully');
                setTimeout(updateStatus, 2000);
            } catch (error) {
                showError('Failed to start scraper: ' + error.message);
            }
        }

        async function stopScraper() {
            try {
                showOperation('Stopping scraper...');
                await apiCall('stop-scraper', { method: 'POST' });
                showSuccess('Scraper stopped successfully');
                setTimeout(updateStatus, 2000);
            } catch (error) {
                showError('Failed to stop scraper: ' + error.message);
            }
        }

        async function restartScraper() {
            try {
                showOperation('Restarting scraper...');
                await apiCall('restart-scraper', { method: 'POST' });
                showSuccess('Scraper restarted successfully');
                setTimeout(updateStatus, 3000);
            } catch (error) {
                showError('Failed to restart scraper: ' + error.message);
            }
        }

        async function shutdownServer() {
            if (!confirm('Are you sure you want to shutdown the server? This will stop both the API and scraper.')) {
                return;
            }
            
            try {
                showOperation('Shutting down server...');
                await apiCall('shutdown', { method: 'POST' });
                showSuccess('Server shutdown initiated');
                stopAutoRefresh();
                setTimeout(() => {
                    document.body.innerHTML = '<div style="text-align: center; padding: 50px; font-size: 24px;">Server has been shut down</div>';
                }, 3000);
            } catch (error) {
                showError('Failed to shutdown server: ' + error.message);
            }
        }

        async function searchEvents() {
            try {
                const query = {};
                
                const repo = document.getElementById('searchRepo').value.trim();
                const actor = document.getElementById('searchActor').value.trim();
                const type = document.getElementById('searchType').value;
                const dateFrom = document.getElementById('searchDateFrom').value;
                const dateTo = document.getElementById('searchDateTo').value;
                
                if (repo) query.repo_name = repo;
                if (actor) query.actor_login = actor;
                if (type) query.type = type;
                if (dateFrom) query.date_from = dateFrom;
                if (dateTo) query.date_to = dateTo;
                
                query.limit = 50;
                
                showOperation('Searching events...');
                
                const results = await apiCall('search', {
                    method: 'POST',
                    body: JSON.stringify(query)
                });
                
                const container = document.getElementById('searchResults');
                
                if (results.length === 0) {
                    container.innerHTML = '<div class="loading">No events found matching your criteria</div>';
                    return;
                }
                
                container.innerHTML = `
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>Date</th>
                                <th>Type</th>
                                <th>Actor</th>
                                <th>Repository</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${results.map(event => `
                                <tr>
                                    <td>${new Date(event.created_at).toLocaleString()}</td>
                                    <td>${event.type}</td>
                                    <td>${event.actor_login || 'Unknown'}</td>
                                    <td>${event.repo_name || 'Unknown'}</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                `;
                
                showSuccess(`Found ${results.length} events`);
                
            } catch (error) {
                showError('Search failed: ' + error.message);
            }
        }

        async function refreshLogs() {
            try {
                const logs = await apiCall('scraper-logs');
                document.getElementById('logsContainer').textContent = logs.content || 'No logs available';
            } catch (error) {
                document.getElementById('logsContainer').textContent = 'Failed to load logs: ' + error.message;
            }
        }

        function clearLogs() {
            document.getElementById('logsContainer').textContent = 'Logs cleared from display (not from file)';
        }

        function showOperation(message) {
            const container = document.getElementById('operationStatus');
            container.innerHTML = `<div style="padding: 10px; background: #3498db; color: white; border-radius: 5px; margin-top: 10px;">${message}</div>`;
        }

        function showSuccess(message) {
            const container = document.getElementById('operationStatus');
            container.innerHTML = `<div class="success">${message}</div>`;
            setTimeout(() => container.innerHTML = '', 5000);
        }

        function showError(message) {
            const container = document.getElementById('operationStatus');
            container.innerHTML = `<div class="error">${message}</div>`;
            setTimeout(() => container.innerHTML = '', 10000);
        }

        // Handle page visibility changes
        document.addEventListener('visibilitychange', function() {
            if (document.hidden) {
                stopAutoRefresh();
            } else {
                refreshData();
                startAutoRefresh();
            }
        });
    </script>
</body>
</html>
        """
        return web.Response(text=dashboard_html, content_type='text/html')
    
    async def get_status(self, request):
        """Get scraper and system status"""
        try:
            # Check if scraper is running
            scraper_running = self.is_scraper_running()
            
            # Check database connection
            db_connected = await self.db.health_check()
            
            return web.json_response({
                'scraper_running': scraper_running,
                'database_connected': db_connected,
                'api_running': True,
                'timestamp': datetime.now().isoformat()
            })
        except Exception as e:
            self.logger.error(f"Error getting status: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_stats(self, request):
        """Get database statistics"""
        try:
            async with self.db.pool.acquire() as conn:
                # Get basic counts
                total_events = await conn.fetchval("SELECT COUNT(*) FROM github_events") or 0
                total_repos = await conn.fetchval("SELECT COUNT(*) FROM repositories") or 0
                processed_files = await conn.fetchval("SELECT COUNT(*) FROM processed_files") or 0
                
                # Get latest event date
                latest_event = await conn.fetchval(
                    "SELECT MAX(created_at) FROM github_events"
                )
                
                # Get event type distribution
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
                    'latest_event_date': latest_event.strftime('%Y-%m-%d %H:%M') if latest_event else None,
                    'event_types': [dict(row) for row in event_types]
                })
        except Exception as e:
            self.logger.error(f"Error getting stats: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_events(self, request):
        """Get recent events"""
        try:
            limit = int(request.query.get('limit', 100))
            
            async with self.db.pool.acquire() as conn:
                events = await conn.fetch("""
                    SELECT id, type, created_at, actor_login, repo_name
                    FROM github_events 
                    ORDER BY created_at DESC 
                    LIMIT $1
                """, limit)
                
                return web.json_response([dict(row) for row in events])
        except Exception as e:
            self.logger.error(f"Error getting events: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_repositories(self, request):
        """Get repositories"""
        try:
            limit = int(request.query.get('limit', 100))
            
            async with self.db.pool.acquire() as conn:
                repos = await conn.fetch("""
                    SELECT id, name, full_name, language, stargazers_count, last_updated_at
                    FROM repositories 
                    ORDER BY stargazers_count DESC NULLS LAST
                    LIMIT $1
                """, limit)
                
                return web.json_response([dict(row) for row in repos])
        except Exception as e:
            self.logger.error(f"Error getting repositories: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def search_events(self, request):
        """Search events"""
        try:
            query = await request.json()
            
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
                where_clauses.append(f"repo_name ILIKE ${param_count}")
                params.append(f"%{query['repo_name']}%")
            
            if 'actor_login' in query:
                param_count += 1
                where_clauses.append(f"actor_login ILIKE ${param_count}")
                params.append(f"%{query['actor_login']}%")
            
            if 'date_from' in query:
                param_count += 1
                where_clauses.append(f"created_at >= ${param_count}")
                params.append(parse_date(query['date_from']))
            
            if 'date_to' in query:
                param_count += 1
                where_clauses.append(f"created_at <= ${param_count}")
                params.append(parse_date(query['date_to']))
            
            # Build final query
            where_sql = " AND ".join(where_clauses) if where_clauses else "1=1"
            limit = query.get('limit', 100)
            
            sql = f"""
            SELECT id, type, created_at, actor_login, repo_name
            FROM github_events 
            WHERE {where_sql}
            ORDER BY created_at DESC
            LIMIT {limit}
            """
            
            async with self.db.pool.acquire() as conn:
                events = await conn.fetch(sql, *params)
                return web.json_response([dict(row) for row in events])
                
        except Exception as e:
            self.logger.error(f"Error searching events: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_recent_activity(self, request):
        """Get recent activity summary"""
        try:
            async with self.db.pool.acquire() as conn:
                activity = await conn.fetch("""
                    SELECT type, actor_login, repo_name, created_at
                    FROM github_events 
                    ORDER BY created_at DESC 
                    LIMIT 20
                """)
                
                return web.json_response([dict(row) for row in activity])
        except Exception as e:
            self.logger.error(f"Error getting recent activity: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    def is_scraper_running(self):
        """Check if scraper process is running"""
        try:
            result = subprocess.run(['pgrep', '-f', 'gharchive_scraper.py'], 
                                  capture_output=True, text=True)
            return result.returncode == 0
        except:
            return False
    
    async def start_scraper(self, request):
        """Start the scraper process"""
        try:
            if self.is_scraper_running():
                return web.json_response({'error': 'Scraper is already running'}, status=400)
            
            # Start scraper in background
            subprocess.Popen([
                'python3', 'gharchive_scraper.py', '--mode', 'scrape'
            ], cwd='/home/ubuntu/GitArchiver')
            
            return web.json_response({'message': 'Scraper started successfully'})
        except Exception as e:
            self.logger.error(f"Error starting scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def stop_scraper(self, request):
        """Stop the scraper process"""
        try:
            result = subprocess.run(['pkill', '-f', 'gharchive_scraper.py'], 
                                  capture_output=True)
            if result.returncode == 0:
                return web.json_response({'message': 'Scraper stopped successfully'})
            else:
                return web.json_response({'message': 'No scraper process found'})
        except Exception as e:
            self.logger.error(f"Error stopping scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def restart_scraper(self, request):
        """Restart the scraper process"""
        try:
            # Stop first
            subprocess.run(['pkill', '-f', 'gharchive_scraper.py'])
            
            # Wait a moment
            await asyncio.sleep(2)
            
            # Start again
            subprocess.Popen([
                'python3', 'gharchive_scraper.py', '--mode', 'scrape'
            ], cwd='/home/ubuntu/GitArchiver')
            
            return web.json_response({'message': 'Scraper restarted successfully'})
        except Exception as e:
            self.logger.error(f"Error restarting scraper: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_scraper_logs(self, request):
        """Get scraper logs"""
        try:
            log_file = Path('/home/ubuntu/GitArchiver/gharchive_scraper.log')
            if log_file.exists():
                # Get last 1000 lines
                result = subprocess.run(['tail', '-n', '1000', str(log_file)], 
                                      capture_output=True, text=True)
                content = result.stdout
            else:
                content = "Log file not found"
            
            return web.json_response({'content': content})
        except Exception as e:
            self.logger.error(f"Error getting logs: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def shutdown_server(self, request):
        """Shutdown the API server"""
        try:
            # Stop scraper first
            subprocess.run(['pkill', '-f', 'gharchive_scraper.py'])
            
            # Schedule server shutdown
            asyncio.create_task(self._delayed_shutdown())
            
            return web.json_response({'message': 'Server shutdown initiated'})
        except Exception as e:
            self.logger.error(f"Error shutting down: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def _delayed_shutdown(self):
        """Delayed shutdown to allow response to be sent"""
        await asyncio.sleep(2)
        os.kill(os.getpid(), signal.SIGTERM)
    
    async def run(self):
        """Run the web server"""
        await self.init_database()
        self.setup_routes()
        
        runner = web.AppRunner(self.app)
        await runner.setup()
        
        site = web.TCPSite(runner, host='0.0.0.0', port=self.port)
        await site.start()
        
        self.logger.info(f"Web API started on http://0.0.0.0:{self.port}")
        self.logger.info(f"Dashboard available at http://170.9.239.38:{self.port}")
        
        # Keep the server running
        try:
            while True:
                await asyncio.sleep(1)
        except KeyboardInterrupt:
            self.logger.info("Shutting down...")
        finally:
            await self.cleanup()

if __name__ == '__main__':
    config = Config()
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    
    api = WebAPI(config, port)
    asyncio.run(api.run())
