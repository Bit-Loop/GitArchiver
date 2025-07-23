#!/usr/bin/env python3
"""
Data visualization and reporting for GitHub Archive Scraper
"""

import asyncio
import json
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Any
import logging


class DataVisualizer:
    """Generate visualizations and reports for scraped data"""
    
    def __init__(self, db_manager):
        self.db_manager = db_manager
        self.logger = logging.getLogger(__name__)
    
    async def generate_event_type_distribution(self, days: int = 30) -> Dict[str, int]:
        """Generate distribution of GitHub event types"""
        end_date = datetime.now()
        start_date = end_date - timedelta(days=days)
        
        query = """
        SELECT 
            event_type,
            COUNT(*) as event_count
        FROM github_events 
        WHERE created_at >= $1 AND created_at <= $2
        GROUP BY event_type
        ORDER BY event_count DESC;
        """
        
        async with self.db_manager.get_connection() as conn:
            rows = await conn.fetch(query, start_date, end_date)
            return {row['event_type']: row['event_count'] for row in rows}
    
    async def generate_daily_activity_stats(self, days: int = 30) -> List[Dict[str, Any]]:
        """Generate daily activity statistics"""
        end_date = datetime.now()
        start_date = end_date - timedelta(days=days)
        
        query = """
        SELECT 
            DATE(created_at) as activity_date,
            COUNT(*) as total_events,
            COUNT(DISTINCT actor_login) as unique_users,
            COUNT(DISTINCT repo_name) as unique_repos,
            AVG(EXTRACT(EPOCH FROM (created_at - LAG(created_at) OVER (ORDER BY created_at)))) as avg_time_between_events
        FROM github_events 
        WHERE created_at >= $1 AND created_at <= $2
        GROUP BY DATE(created_at)
        ORDER BY activity_date;
        """
        
        async with self.db_manager.get_connection() as conn:
            rows = await conn.fetch(query, start_date, end_date)
            return [dict(row) for row in rows]
    
    async def generate_top_repositories(self, limit: int = 50, days: int = 30) -> List[Dict[str, Any]]:
        """Generate top repositories by activity"""
        end_date = datetime.now()
        start_date = end_date - timedelta(days=days)
        
        query = """
        SELECT 
            repo_name,
            COUNT(*) as total_events,
            COUNT(DISTINCT actor_login) as unique_contributors,
            COUNT(DISTINCT event_type) as event_types_count,
            MAX(created_at) as last_activity,
            STRING_AGG(DISTINCT event_type, ', ' ORDER BY event_type) as event_types
        FROM github_events 
        WHERE created_at >= $1 AND created_at <= $2
        AND repo_name IS NOT NULL
        GROUP BY repo_name
        ORDER BY total_events DESC
        LIMIT $3;
        """
        
        async with self.db_manager.get_connection() as conn:
            rows = await conn.fetch(query, start_date, end_date, limit)
            return [dict(row) for row in rows]
    
    async def generate_programming_language_stats(self, days: int = 30) -> Dict[str, Any]:
        """Generate programming language statistics"""
        end_date = datetime.now()
        start_date = end_date - timedelta(days=days)
        
        query = """
        SELECT 
            payload->>'language' as language,
            COUNT(*) as event_count,
            COUNT(DISTINCT repo_name) as repo_count,
            COUNT(DISTINCT actor_login) as user_count
        FROM github_events 
        WHERE created_at >= $1 AND created_at <= $2
        AND payload->>'language' IS NOT NULL
        AND payload->>'language' != ''
        GROUP BY payload->>'language'
        ORDER BY event_count DESC
        LIMIT 25;
        """
        
        async with self.db_manager.get_connection() as conn:
            rows = await conn.fetch(query, start_date, end_date)
            return {
                'languages': [dict(row) for row in rows],
                'total_languages': len(rows)
            }
    
    async def generate_user_activity_patterns(self, days: int = 7) -> Dict[str, Any]:
        """Generate user activity patterns by hour and day"""
        end_date = datetime.now()
        start_date = end_date - timedelta(days=days)
        
        # Activity by hour of day
        hourly_query = """
        SELECT 
            EXTRACT(HOUR FROM created_at) as hour,
            COUNT(*) as event_count
        FROM github_events 
        WHERE created_at >= $1 AND created_at <= $2
        GROUP BY EXTRACT(HOUR FROM created_at)
        ORDER BY hour;
        """
        
        # Activity by day of week
        daily_query = """
        SELECT 
            EXTRACT(DOW FROM created_at) as day_of_week,
            COUNT(*) as event_count
        FROM github_events 
        WHERE created_at >= $1 AND created_at <= $2
        GROUP BY EXTRACT(DOW FROM created_at)
        ORDER BY day_of_week;
        """
        
        async with self.db_manager.get_connection() as conn:
            hourly_rows = await conn.fetch(hourly_query, start_date, end_date)
            daily_rows = await conn.fetch(daily_query, start_date, end_date)
            
            return {
                'hourly_activity': [dict(row) for row in hourly_rows],
                'daily_activity': [dict(row) for row in daily_rows]
            }
    
    async def generate_repository_growth_trends(self, limit: int = 20) -> List[Dict[str, Any]]:
        """Generate repository growth trends"""
        query = """
        WITH repo_stats AS (
            SELECT 
                repo_name,
                MIN(created_at) as first_seen,
                MAX(created_at) as last_seen,
                COUNT(*) as total_events,
                COUNT(DISTINCT DATE(created_at)) as active_days
            FROM github_events 
            WHERE repo_name IS NOT NULL
            GROUP BY repo_name
            HAVING COUNT(*) >= 100
        )
        SELECT 
            repo_name,
            first_seen,
            last_seen,
            total_events,
            active_days,
            ROUND(total_events::decimal / active_days, 2) as avg_events_per_day,
            EXTRACT(DAYS FROM (last_seen - first_seen)) as lifespan_days
        FROM repo_stats
        ORDER BY total_events DESC
        LIMIT $1;
        """
        
        async with self.db_manager.get_connection() as conn:
            rows = await conn.fetch(query, limit)
            return [dict(row) for row in rows]
    
    async def generate_comprehensive_report(self, days: int = 30) -> Dict[str, Any]:
        """Generate a comprehensive analysis report"""
        self.logger.info(f"Generating comprehensive report for last {days} days")
        
        # Gather all statistics
        event_distribution = await self.generate_event_type_distribution(days)
        daily_stats = await self.generate_daily_activity_stats(days)
        top_repos = await self.generate_top_repositories(limit=25, days=days)
        language_stats = await self.generate_programming_language_stats(days)
        activity_patterns = await self.generate_user_activity_patterns(min(days, 7))
        growth_trends = await self.generate_repository_growth_trends(limit=15)
        
        # Calculate summary statistics
        total_events = sum(event_distribution.values())
        total_repos = len(top_repos)
        
        report = {
            'report_metadata': {
                'generated_at': datetime.now().isoformat(),
                'period_days': days,
                'total_events_analyzed': total_events,
                'total_repositories_analyzed': total_repos
            },
            'event_type_distribution': event_distribution,
            'daily_activity_stats': daily_stats,
            'top_repositories': top_repos,
            'programming_language_stats': language_stats,
            'user_activity_patterns': activity_patterns,
            'repository_growth_trends': growth_trends,
            'summary_insights': {
                'most_active_event_type': max(event_distribution, key=lambda k: event_distribution[k]) if event_distribution else None,
                'most_active_language': language_stats['languages'][0]['language'] if language_stats['languages'] else None,
                'peak_activity_hour': max(activity_patterns['hourly_activity'], key=lambda x: x['event_count'])['hour'] if activity_patterns['hourly_activity'] else None,
                'average_daily_events': total_events / days if days > 0 else 0
            }
        }
        
        self.logger.info("Comprehensive report generated successfully")
        return report
    
    def export_report_to_file(self, report: Dict[str, Any], filename: Optional[str] = None):
        """Export report to JSON file"""
        if not filename:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            filename = f"github_archive_report_{timestamp}.json"
        
        try:
            with open(filename, 'w') as f:
                json.dump(report, f, indent=2, default=str)
            
            self.logger.info(f"Report exported to {filename}")
            return filename
            
        except Exception as e:
            self.logger.error(f"Error exporting report: {e}")
            return None
    
    def print_report_summary(self, report: Dict[str, Any]):
        """Print a formatted summary of the report"""
        metadata = report['report_metadata']
        insights = report['summary_insights']
        
        print("\n" + "="*80)
        print("GitHub Archive Analysis Report Summary")
        print("="*80)
        print(f"📊 Analysis Period: {metadata['period_days']} days")
        print(f"📈 Total Events: {metadata['total_events_analyzed']:,}")
        print(f"🏗️  Total Repositories: {metadata['total_repositories_analyzed']:,}")
        print(f"📅 Generated: {metadata['generated_at']}")
        
        print(f"\n🔥 Key Insights:")
        print(f"   • Most Active Event Type: {insights['most_active_event_type']}")
        print(f"   • Most Popular Language: {insights['most_active_language']}")
        print(f"   • Peak Activity Hour: {insights['peak_activity_hour']}:00")
        print(f"   • Average Daily Events: {insights['average_daily_events']:,.0f}")
        
        print(f"\n📊 Top 5 Event Types:")
        event_dist = report['event_type_distribution']
        for i, (event_type, count) in enumerate(list(event_dist.items())[:5], 1):
            percentage = (count / metadata['total_events_analyzed']) * 100
            print(f"   {i}. {event_type}: {count:,} ({percentage:.1f}%)")
        
        print(f"\n🏆 Top 5 Most Active Repositories:")
        for i, repo in enumerate(report['top_repositories'][:5], 1):
            print(f"   {i}. {repo['repo_name']}: {repo['total_events']:,} events")
        
        print(f"\n💻 Top 5 Programming Languages:")
        for i, lang in enumerate(report['programming_language_stats']['languages'][:5], 1):
            print(f"   {i}. {lang['language']}: {lang['event_count']:,} events")
        
        print("="*80)


async def main():
    """Example usage"""
    import sys
    import os
    
    # Add the current directory to the path so we can import our modules
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    
    try:
        from config import Config
        from gharchive_scraper import DatabaseManager
        
        config = Config()
        db_manager = DatabaseManager(config)
        
        await db_manager.connect()
        
        visualizer = DataVisualizer(db_manager)
        
        # Generate comprehensive report
        report = await visualizer.generate_comprehensive_report(days=7)
        
        # Print summary
        visualizer.print_report_summary(report)
        
        # Export to file
        filename = visualizer.export_report_to_file(report)
        print(f"\n📄 Full report saved to: {filename}")
        
    except ImportError as e:
        print(f"Import error: {e}")
        print("Make sure you're running this from the correct directory with all dependencies installed.")
    except Exception as e:
        print(f"Error: {e}")
    finally:
        try:
            await db_manager.cleanup()
        except:
            pass


if __name__ == '__main__':
    asyncio.run(main())
