#!/usr/bin/env python3
"""
GitHub Archive Database Management Tool
Comprehensive database access, analysis, and management for GitHub Archive data
"""

import asyncio
import json
import sys
import argparse
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional, Any
import pandas as pd
import asyncpg
from tabulate import tabulate

# Add project directory to path
sys.path.insert(0, str(Path(__file__).parent))

from config import Config


class GitHubArchiveDB:
    """Comprehensive database management for GitHub Archive data"""
    
    def __init__(self, config: Config):
        self.config = config
        self.pool = None
        
    async def connect(self):
        """Connect to database"""
        dsn = f"postgresql://{self.config.DB_USER}:{self.config.DB_PASSWORD}@{self.config.DB_HOST}:{self.config.DB_PORT}/{self.config.DB_NAME}"
        self.pool = await asyncpg.create_pool(dsn, min_size=2, max_size=10)
        
    async def disconnect(self):
        """Disconnect from database"""
        if self.pool:
            await self.pool.close()
    
    async def get_overview(self) -> Dict[str, Any]:
        """Get comprehensive database overview"""
        async with self.pool.acquire() as conn:
            # Basic counts
            total_events = await conn.fetchval("SELECT COUNT(*) FROM github_events") or 0
            total_repos = await conn.fetchval("SELECT COUNT(*) FROM repositories") or 0
            total_processed = await conn.fetchval("SELECT COUNT(*) FROM processed_files") or 0
            
            # Date ranges
            date_range = await conn.fetchrow("""
                SELECT MIN(created_at) as earliest, MAX(created_at) as latest 
                FROM github_events
            """)
            
            # Event types
            event_types = await conn.fetch("""
                SELECT type, COUNT(*) as count 
                FROM github_events 
                GROUP BY type 
                ORDER BY count DESC 
                LIMIT 10
            """)
            
            # Top repositories by activity
            top_repos = await conn.fetch("""
                SELECT repo_name, COUNT(*) as event_count
                FROM github_events 
                WHERE repo_name IS NOT NULL
                GROUP BY repo_name 
                ORDER BY event_count DESC 
                LIMIT 10
            """)
            
            # Top actors
            top_actors = await conn.fetch("""
                SELECT actor_login, COUNT(*) as event_count
                FROM github_events 
                WHERE actor_login IS NOT NULL
                GROUP BY actor_login 
                ORDER BY event_count DESC 
                LIMIT 10
            """)
            
            # Database size
            db_size = await conn.fetchrow("""
                SELECT 
                    pg_size_pretty(pg_database_size(current_database())) as total_size,
                    pg_size_pretty(pg_total_relation_size('github_events')) as events_size,
                    pg_size_pretty(pg_total_relation_size('repositories')) as repos_size
            """)
            
            return {
                'counts': {
                    'total_events': total_events,
                    'total_repositories': total_repos,
                    'processed_files': total_processed
                },
                'date_range': {
                    'earliest': date_range['earliest'],
                    'latest': date_range['latest']
                },
                'event_types': [dict(row) for row in event_types],
                'top_repositories': [dict(row) for row in top_repos],
                'top_actors': [dict(row) for row in top_actors],
                'database_size': dict(db_size)
            }
    
    async def search_events(self, filters: Dict[str, Any], limit: int = 100) -> List[Dict[str, Any]]:
        """Advanced event search with multiple filters"""
        where_clauses = []
        params = []
        param_idx = 0
        
        if filters.get('type'):
            param_idx += 1
            where_clauses.append(f"type = ${param_idx}")
            params.append(filters['type'])
            
        if filters.get('repo_name'):
            param_idx += 1
            where_clauses.append(f"repo_name ILIKE ${param_idx}")
            params.append(f"%{filters['repo_name']}%")
            
        if filters.get('actor_login'):
            param_idx += 1
            where_clauses.append(f"actor_login ILIKE ${param_idx}")
            params.append(f"%{filters['actor_login']}%")
            
        if filters.get('date_from'):
            param_idx += 1
            where_clauses.append(f"created_at >= ${param_idx}")
            params.append(filters['date_from'])
            
        if filters.get('date_to'):
            param_idx += 1
            where_clauses.append(f"created_at <= ${param_idx}")
            params.append(filters['date_to'])
            
        if filters.get('public') is not None:
            param_idx += 1
            where_clauses.append(f"public = ${param_idx}")
            params.append(filters['public'])
        
        where_sql = " AND ".join(where_clauses) if where_clauses else "1=1"
        
        async with self.pool.acquire() as conn:
            events = await conn.fetch(f"""
                SELECT id, type, created_at, actor_login, repo_name, public, 
                       jsonb_extract_path_text(payload, 'ref') as ref_name,
                       jsonb_extract_path_text(payload, 'action') as action
                FROM github_events 
                WHERE {where_sql}
                ORDER BY created_at DESC 
                LIMIT {limit}
            """, *params)
            
            return [dict(row) for row in events]
    
    async def get_repository_details(self, repo_name: str) -> Dict[str, Any]:
        """Get comprehensive repository information"""
        async with self.pool.acquire() as conn:
            # Repository basic info
            repo_info = await conn.fetchrow("""
                SELECT * FROM repositories WHERE name = $1 OR full_name = $1
            """, repo_name)
            
            if not repo_info:
                # Try to find from events
                repo_events = await conn.fetch("""
                    SELECT DISTINCT repo_id, repo_name, COUNT(*) as event_count
                    FROM github_events 
                    WHERE repo_name ILIKE $1
                    GROUP BY repo_id, repo_name
                    ORDER BY event_count DESC
                """, f"%{repo_name}%")
                
                if repo_events:
                    repo_info = dict(repo_events[0])
                else:
                    return {'error': f'Repository {repo_name} not found'}
            
            # Event statistics
            event_stats = await conn.fetch("""
                SELECT type, COUNT(*) as count
                FROM github_events 
                WHERE repo_name = $1
                GROUP BY type 
                ORDER BY count DESC
            """, repo_name)
            
            # Activity timeline (last 30 days)
            activity = await conn.fetch("""
                SELECT DATE(created_at) as date, type, COUNT(*) as count
                FROM github_events 
                WHERE repo_name = $1 AND created_at >= NOW() - INTERVAL '30 days'
                GROUP BY DATE(created_at), type
                ORDER BY date DESC, count DESC
            """, repo_name)
            
            # Top contributors
            contributors = await conn.fetch("""
                SELECT actor_login, COUNT(*) as contributions
                FROM github_events 
                WHERE repo_name = $1 AND actor_login IS NOT NULL
                GROUP BY actor_login 
                ORDER BY contributions DESC 
                LIMIT 20
            """, repo_name)
            
            return {
                'repository': dict(repo_info),
                'event_statistics': [dict(row) for row in event_stats],
                'recent_activity': [dict(row) for row in activity],
                'top_contributors': [dict(row) for row in contributors]
            }
    
    async def get_actor_profile(self, actor_login: str) -> Dict[str, Any]:
        """Get comprehensive actor/user profile"""
        async with self.pool.acquire() as conn:
            # Basic activity stats
            activity_stats = await conn.fetchrow("""
                SELECT 
                    COUNT(*) as total_events,
                    COUNT(DISTINCT repo_name) as unique_repos,
                    COUNT(DISTINCT type) as event_types,
                    MIN(created_at) as first_activity,
                    MAX(created_at) as last_activity
                FROM github_events 
                WHERE actor_login = $1
            """, actor_login)
            
            # Event type breakdown
            event_breakdown = await conn.fetch("""
                SELECT type, COUNT(*) as count
                FROM github_events 
                WHERE actor_login = $1
                GROUP BY type 
                ORDER BY count DESC
            """, actor_login)
            
            # Repository contributions
            repo_contributions = await conn.fetch("""
                SELECT repo_name, COUNT(*) as contributions
                FROM github_events 
                WHERE actor_login = $1 AND repo_name IS NOT NULL
                GROUP BY repo_name 
                ORDER BY contributions DESC 
                LIMIT 20
            """, actor_login)
            
            # Activity timeline
            timeline = await conn.fetch("""
                SELECT DATE(created_at) as date, COUNT(*) as events
                FROM github_events 
                WHERE actor_login = $1 AND created_at >= NOW() - INTERVAL '90 days'
                GROUP BY DATE(created_at)
                ORDER BY date DESC
            """, actor_login)
            
            return {
                'actor_login': actor_login,
                'activity_stats': dict(activity_stats),
                'event_breakdown': [dict(row) for row in event_breakdown],
                'repository_contributions': [dict(row) for row in repo_contributions],
                'activity_timeline': [dict(row) for row in timeline]
            }
    
    async def analyze_trends(self, days: int = 30) -> Dict[str, Any]:
        """Analyze trends over specified period"""
        async with self.pool.acquire() as conn:
            # Daily event counts
            daily_trends = await conn.fetch("""
                SELECT 
                    DATE(created_at) as date,
                    COUNT(*) as total_events,
                    COUNT(DISTINCT actor_login) as unique_actors,
                    COUNT(DISTINCT repo_name) as unique_repos
                FROM github_events 
                WHERE created_at >= NOW() - INTERVAL '%d days'
                GROUP BY DATE(created_at)
                ORDER BY date DESC
            """ % days)
            
            # Event type trends
            type_trends = await conn.fetch("""
                SELECT 
                    type,
                    DATE(created_at) as date,
                    COUNT(*) as count
                FROM github_events 
                WHERE created_at >= NOW() - INTERVAL '%d days'
                GROUP BY type, DATE(created_at)
                ORDER BY date DESC, count DESC
            """ % days)
            
            # Language trends (from repository events)
            language_trends = await conn.fetch("""
                SELECT 
                    jsonb_extract_path_text(payload, 'repository', 'language') as language,
                    COUNT(*) as events
                FROM github_events 
                WHERE created_at >= NOW() - INTERVAL '%d days'
                    AND jsonb_extract_path_text(payload, 'repository', 'language') IS NOT NULL
                GROUP BY language
                ORDER BY events DESC
                LIMIT 20
            """ % days)
            
            return {
                'period_days': days,
                'daily_trends': [dict(row) for row in daily_trends],
                'event_type_trends': [dict(row) for row in type_trends],
                'language_trends': [dict(row) for row in language_trends]
            }
    
    async def get_data_quality_report(self) -> Dict[str, Any]:
        """Generate data quality and completeness report"""
        async with self.pool.acquire() as conn:
            # Basic completeness
            completeness = await conn.fetchrow("""
                SELECT 
                    COUNT(*) as total_events,
                    COUNT(actor_login) as events_with_actor,
                    COUNT(repo_name) as events_with_repo,
                    COUNT(payload) as events_with_payload,
                    COUNT(CASE WHEN payload != '{}' THEN 1 END) as events_with_data
                FROM github_events
            """)
            
            # File processing status
            file_status = await conn.fetch("""
                SELECT 
                    is_complete,
                    COUNT(*) as file_count,
                    SUM(event_count) as total_events,
                    AVG(event_count) as avg_events_per_file
                FROM processed_files
                GROUP BY is_complete
            """)
            
            # Missing data detection
            missing_ranges = await conn.fetch("""
                SELECT * FROM missing_data_ranges 
                WHERE is_recovered = FALSE
                ORDER BY start_date
            """)
            
            # Data distribution by day
            daily_distribution = await conn.fetch("""
                SELECT 
                    DATE(created_at) as date,
                    COUNT(*) as events,
                    COUNT(DISTINCT repo_name) as repos,
                    COUNT(DISTINCT actor_login) as actors
                FROM github_events
                WHERE created_at >= NOW() - INTERVAL '7 days'
                GROUP BY DATE(created_at)
                ORDER BY date
            """)
            
            return {
                'completeness': dict(completeness),
                'file_processing': [dict(row) for row in file_status],
                'missing_data_ranges': [dict(row) for row in missing_ranges],
                'recent_distribution': [dict(row) for row in daily_distribution]
            }
    
    async def export_data(self, query: str, format: str = 'json', limit: int = 1000) -> str:
        """Export data with custom query"""
        async with self.pool.acquire() as conn:
            # Add limit to query if not present
            if 'LIMIT' not in query.upper():
                query += f" LIMIT {limit}"
                
            rows = await conn.fetch(query)
            data = [dict(row) for row in rows]
            
            if format.lower() == 'json':
                return json.dumps(data, indent=2, default=str)
            elif format.lower() == 'csv':
                if data:
                    df = pd.DataFrame(data)
                    return df.to_csv(index=False)
                return ""
            else:
                return str(data)
    
    async def optimize_database(self) -> Dict[str, str]:
        """Perform database optimization"""
        async with self.pool.acquire() as conn:
            results = {}
            
            # Analyze tables
            await conn.execute("ANALYZE github_events")
            await conn.execute("ANALYZE repositories")
            await conn.execute("ANALYZE processed_files")
            results['analyze'] = "✅ Table statistics updated"
            
            # Vacuum
            await conn.execute("VACUUM ANALYZE github_events")
            results['vacuum'] = "✅ Tables vacuumed and analyzed"
            
            # Refresh materialized views
            try:
                await conn.execute("SELECT refresh_stats_views()")
                results['views'] = "✅ Materialized views refreshed"
            except Exception as e:
                results['views'] = f"❌ View refresh failed: {e}"
            
            return results


# CLI Interface
async def main():
    """Command line interface for database management"""
    parser = argparse.ArgumentParser(description='GitHub Archive Database Management Tool')
    parser.add_argument('command', choices=[
        'overview', 'search', 'repo', 'actor', 'trends', 'quality', 'export', 'optimize'
    ], help='Command to execute')
    
    # Command-specific arguments
    parser.add_argument('--repo', help='Repository name for repo command')
    parser.add_argument('--actor', help='Actor login for actor command')
    parser.add_argument('--type', help='Event type filter')
    parser.add_argument('--days', type=int, default=30, help='Number of days for trends')
    parser.add_argument('--limit', type=int, default=100, help='Result limit')
    parser.add_argument('--format', choices=['json', 'csv', 'table'], default='table', help='Output format')
    parser.add_argument('--query', help='Custom SQL query for export')
    parser.add_argument('--date-from', help='Start date (YYYY-MM-DD)')
    parser.add_argument('--date-to', help='End date (YYYY-MM-DD)')
    
    args = parser.parse_args()
    
    # Initialize database
    config = Config()
    db = GitHubArchiveDB(config)
    await db.connect()
    
    try:
        if args.command == 'overview':
            overview = await db.get_overview()
            
            print("📊 GitHub Archive Database Overview")
            print("=" * 50)
            print(f"Total Events: {overview['counts']['total_events']:,}")
            print(f"Total Repositories: {overview['counts']['total_repositories']:,}")
            print(f"Processed Files: {overview['counts']['processed_files']:,}")
            
            if overview['date_range']['earliest']:
                print(f"Date Range: {overview['date_range']['earliest']} to {overview['date_range']['latest']}")
            
            print(f"\nDatabase Size:")
            for key, value in overview['database_size'].items():
                print(f"  {key}: {value}")
            
            print(f"\nTop Event Types:")
            for event_type in overview['event_types'][:5]:
                print(f"  {event_type['type']}: {event_type['count']:,}")
            
            print(f"\nTop Repositories:")
            for repo in overview['top_repositories'][:5]:
                print(f"  {repo['repo_name']}: {repo['event_count']:,} events")
                
        elif args.command == 'search':
            filters = {}
            if args.type:
                filters['type'] = args.type
            if args.date_from:
                filters['date_from'] = args.date_from
            if args.date_to:
                filters['date_to'] = args.date_to
                
            events = await db.search_events(filters, args.limit)
            
            if args.format == 'json':
                print(json.dumps(events, indent=2, default=str))
            else:
                headers = ['ID', 'Type', 'Created', 'Actor', 'Repository', 'Public']
                rows = [[e['id'], e['type'], e['created_at'], e['actor_login'], e['repo_name'], e['public']] for e in events]
                print(tabulate(rows, headers=headers, tablefmt='grid'))
                
        elif args.command == 'repo':
            if not args.repo:
                print("Error: --repo required for repo command")
                return
                
            repo_data = await db.get_repository_details(args.repo)
            
            if 'error' in repo_data:
                print(repo_data['error'])
                return
                
            print(f"📁 Repository: {args.repo}")
            print("=" * 50)
            
            if args.format == 'json':
                print(json.dumps(repo_data, indent=2, default=str))
            else:
                repo_info = repo_data['repository']
                print(f"Name: {repo_info.get('name', 'N/A')}")
                print(f"Full Name: {repo_info.get('full_name', 'N/A')}")
                print(f"Language: {repo_info.get('language', 'N/A')}")
                print(f"Stars: {repo_info.get('stargazers_count', 'N/A')}")
                
                print(f"\nEvent Statistics:")
                for stat in repo_data['event_statistics'][:10]:
                    print(f"  {stat['type']}: {stat['count']:,}")
                    
                print(f"\nTop Contributors:")
                for contrib in repo_data['top_contributors'][:10]:
                    print(f"  {contrib['actor_login']}: {contrib['contributions']:,}")
                    
        elif args.command == 'actor':
            if not args.actor:
                print("Error: --actor required for actor command")
                return
                
            actor_data = await db.get_actor_profile(args.actor)
            
            print(f"👤 Actor: {args.actor}")
            print("=" * 50)
            
            if args.format == 'json':
                print(json.dumps(actor_data, indent=2, default=str))
            else:
                stats = actor_data['activity_stats']
                print(f"Total Events: {stats['total_events']:,}")
                print(f"Unique Repositories: {stats['unique_repos']:,}")
                print(f"Event Types: {stats['event_types']:,}")
                print(f"First Activity: {stats['first_activity']}")
                print(f"Last Activity: {stats['last_activity']}")
                
                print(f"\nEvent Breakdown:")
                for event in actor_data['event_breakdown'][:10]:
                    print(f"  {event['type']}: {event['count']:,}")
                    
        elif args.command == 'trends':
            trends = await db.analyze_trends(args.days)
            
            print(f"📈 Trends (Last {args.days} days)")
            print("=" * 50)
            
            if args.format == 'json':
                print(json.dumps(trends, indent=2, default=str))
            else:
                print("Daily Activity:")
                for day in trends['daily_trends'][:10]:
                    print(f"  {day['date']}: {day['total_events']:,} events, {day['unique_actors']:,} actors")
                    
        elif args.command == 'quality':
            quality = await db.get_data_quality_report()
            
            print("🔍 Data Quality Report")
            print("=" * 50)
            
            if args.format == 'json':
                print(json.dumps(quality, indent=2, default=str))
            else:
                comp = quality['completeness']
                total = comp['total_events']
                print(f"Total Events: {total:,}")
                print(f"Events with Actor: {comp['events_with_actor']:,} ({comp['events_with_actor']/total*100:.1f}%)")
                print(f"Events with Repository: {comp['events_with_repo']:,} ({comp['events_with_repo']/total*100:.1f}%)")
                print(f"Events with Payload: {comp['events_with_data']:,} ({comp['events_with_data']/total*100:.1f}%)")
                
        elif args.command == 'export':
            if not args.query:
                print("Error: --query required for export command")
                return
                
            result = await db.export_data(args.query, args.format, args.limit)
            print(result)
            
        elif args.command == 'optimize':
            results = await db.optimize_database()
            
            print("🔧 Database Optimization")
            print("=" * 50)
            for operation, result in results.items():
                print(f"{operation}: {result}")
                
    except Exception as e:
        print(f"Error: {e}")
    finally:
        await db.disconnect()


if __name__ == "__main__":
    asyncio.run(main())
