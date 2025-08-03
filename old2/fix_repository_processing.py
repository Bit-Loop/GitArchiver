#!/usr/bin/env python3
"""
Fix for GitHub Archive Scraper repository processing
This script fixes the data type conversion issues and enhances repository tracking
"""

import asyncio
import sys
from pathlib import Path
import logging

# Add project directory to path
sys.path.insert(0, str(Path(__file__).parent))

from config import Config
from gharchive_scraper import GitHubArchiveScraper


async def fix_repository_processing():
    """Fix and enhance repository processing in existing data"""
    
    print("🔧 Fixing Repository Processing Issues")
    print("=" * 50)
    
    config = Config()
    
    # Initialize scraper
    async with GitHubArchiveScraper(config) as scraper:
        async with scraper.db.pool.acquire() as conn:
            
            # 1. Fix missing repositories by extracting from existing events
            print("📊 Analyzing existing events...")
            
            # Get repository data from events
            repo_events = await conn.fetch("""
                SELECT DISTINCT 
                    repo_id,
                    repo_name,
                    jsonb_extract_path_text(payload, 'repository', 'full_name') as full_name,
                    jsonb_extract_path_text(payload, 'repository', 'description') as description,
                    jsonb_extract_path_text(payload, 'repository', 'html_url') as html_url,
                    jsonb_extract_path_text(payload, 'repository', 'language') as language,
                    jsonb_extract_path_text(payload, 'repository', 'default_branch') as default_branch,
                    jsonb_extract_path_text(payload, 'repository', 'created_at') as repo_created_at,
                    jsonb_extract_path_text(payload, 'repository', 'updated_at') as repo_updated_at,
                    jsonb_extract_path_text(payload, 'repository', 'stargazers_count') as stargazers_count,
                    jsonb_extract_path_text(payload, 'repository', 'forks_count') as forks_count,
                    jsonb_extract_path_text(payload, 'repository', 'size') as size_kb,
                    jsonb_extract_path_text(payload, 'repository', 'private') as is_private,
                    jsonb_extract_path_text(payload, 'repository', 'fork') as is_fork,
                    jsonb_extract_path_text(payload, 'repository', 'archived') as is_archived,
                    jsonb_extract_path_text(payload, 'repository', 'owner', 'login') as owner_login,
                    jsonb_extract_path_text(payload, 'repository', 'owner', 'type') as owner_type,
                    MIN(created_at) as first_seen_at,
                    MAX(created_at) as last_activity_at,
                    COUNT(*) as event_count
                FROM github_events 
                WHERE repo_id IS NOT NULL 
                    AND payload ? 'repository'
                GROUP BY repo_id, repo_name, full_name, description, html_url, language, 
                         default_branch, repo_created_at, repo_updated_at, stargazers_count, 
                         forks_count, size_kb, is_private, is_fork, is_archived, owner_login, owner_type
                ORDER BY event_count DESC
            """)
            
            print(f"Found {len(repo_events)} unique repositories in events")
            
            # 2. Insert repositories with proper data types
            inserted_count = 0
            updated_count = 0
            
            for repo in repo_events:
                try:
                    # Convert string values to appropriate types
                    repo_id = int(repo['repo_id']) if repo['repo_id'] else None
                    stargazers_count = int(repo['stargazers_count']) if repo['stargazers_count'] and repo['stargazers_count'].isdigit() else 0
                    forks_count = int(repo['forks_count']) if repo['forks_count'] and repo['forks_count'].isdigit() else 0
                    size_kb = int(repo['size_kb']) if repo['size_kb'] and repo['size_kb'].isdigit() else 0
                    is_private = repo['is_private'] == 'true' if repo['is_private'] else False
                    is_fork = repo['is_fork'] == 'true' if repo['is_fork'] else False
                    is_archived = repo['is_archived'] == 'true' if repo['is_archived'] else False
                    
                    # Handle timestamps
                    repo_created_at = repo['repo_created_at'] if repo['repo_created_at'] else None
                    repo_updated_at = repo['repo_updated_at'] if repo['repo_updated_at'] else None
                    
                    if repo_id:
                        result = await conn.execute("""
                            INSERT INTO repositories (
                                id, name, full_name, description, html_url, language, 
                                default_branch, created_at, updated_at, size_kb,
                                stargazers_count, forks_count, is_private, is_fork, 
                                is_archived, owner_login, owner_type, first_seen_at, 
                                last_updated_at
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, NOW()
                            )
                            ON CONFLICT (id) DO UPDATE SET
                                name = EXCLUDED.name,
                                full_name = EXCLUDED.full_name,
                                description = EXCLUDED.description,
                                html_url = EXCLUDED.html_url,
                                language = EXCLUDED.language,
                                default_branch = EXCLUDED.default_branch,
                                updated_at = EXCLUDED.updated_at,
                                size_kb = EXCLUDED.size_kb,
                                stargazers_count = EXCLUDED.stargazers_count,
                                forks_count = EXCLUDED.forks_count,
                                is_private = EXCLUDED.is_private,
                                is_fork = EXCLUDED.is_fork,
                                is_archived = EXCLUDED.is_archived,
                                owner_login = EXCLUDED.owner_login,
                                owner_type = EXCLUDED.owner_type,
                                last_updated_at = NOW()
                        """, 
                        repo_id, repo['repo_name'], repo['full_name'], repo['description'],
                        repo['html_url'], repo['language'], repo['default_branch'],
                        repo_created_at, repo_updated_at, size_kb, stargazers_count,
                        forks_count, is_private, is_fork, is_archived, repo['owner_login'],
                        repo['owner_type'], repo['first_seen_at'])
                        
                        if "INSERT" in result:
                            inserted_count += 1
                        else:
                            updated_count += 1
                            
                except Exception as e:
                    print(f"Error processing repository {repo['repo_name']}: {e}")
                    continue
            
            print(f"✅ Processed repositories: {inserted_count} inserted, {updated_count} updated")
            
            # 3. Create processed_files entries for existing data
            print("\n📁 Creating processed files tracking...")
            
            # Get unique filenames from events  
            file_sources = await conn.fetch("""
                SELECT DISTINCT file_source, COUNT(*) as event_count
                FROM github_events 
                WHERE file_source IS NOT NULL
                GROUP BY file_source
            """)
            
            files_tracked = 0
            for file_info in file_sources:
                await conn.execute("""
                    INSERT INTO processed_files (filename, event_count, processed_at, is_complete)
                    VALUES ($1, $2, NOW(), TRUE)
                    ON CONFLICT (filename) DO UPDATE SET
                        event_count = EXCLUDED.event_count,
                        processed_at = NOW()
                """, file_info['file_source'], file_info['event_count'])
                files_tracked += 1
            
            print(f"✅ Tracked {files_tracked} processed files")
            
            # 4. Update statistics
            print("\n📊 Updating statistics...")
            
            total_events = await conn.fetchval("SELECT COUNT(*) FROM github_events")
            total_repos = await conn.fetchval("SELECT COUNT(*) FROM repositories") 
            total_files = await conn.fetchval("SELECT COUNT(*) FROM processed_files")
            
            print(f"Final counts:")
            print(f"  Events: {total_events:,}")
            print(f"  Repositories: {total_repos:,}")
            print(f"  Processed Files: {total_files:,}")
            
            # 5. Refresh materialized views
            print("\n🔄 Refreshing materialized views...")
            try:
                await conn.execute("SELECT refresh_stats_views()")
                print("✅ Materialized views refreshed")
            except Exception as e:
                print(f"⚠️ Warning: Could not refresh views: {e}")
    
    print("\n🎉 Repository processing fix completed!")


async def create_enhanced_api_endpoints():
    """Add enhanced API endpoints for repository and data access"""
    
    # This will be added to the API class
    enhanced_endpoints = """
    
    # Add these methods to the SafeWebAPI class in api.py:
    
    async def get_repositories(self, request):
        \"\"\"Get repositories with enhanced filtering and pagination\"\"\"
        try:
            if not await self.db.health_check():
                return web.json_response({'error': 'Database not connected'}, status=503)
            
            # Parse query parameters
            limit = min(int(request.query.get('limit', 50)), 500)
            offset = int(request.query.get('offset', 0))
            language = request.query.get('language')
            min_stars = request.query.get('min_stars', type=int)
            sort_by = request.query.get('sort_by', 'stargazers_count')
            order = request.query.get('order', 'desc')
            
            # Build query
            where_clauses = []
            params = []
            param_idx = 0
            
            if language:
                param_idx += 1
                where_clauses.append(f"language = ${param_idx}")
                params.append(language)
            
            if min_stars:
                param_idx += 1
                where_clauses.append(f"stargazers_count >= ${param_idx}")
                params.append(min_stars)
            
            where_sql = " AND ".join(where_clauses) if where_clauses else "1=1"
            
            # Validate sort column
            valid_sorts = ['stargazers_count', 'forks_count', 'created_at', 'name']
            if sort_by not in valid_sorts:
                sort_by = 'stargazers_count'
            
            order = 'ASC' if order.lower() == 'asc' else 'DESC'
            
            async with self.db.pool.acquire() as conn:
                repos = await conn.fetch(f\"\"\"
                    SELECT id, name, full_name, description, language, 
                           stargazers_count, forks_count, created_at, html_url,
                           owner_login, is_private, is_fork
                    FROM repositories 
                    WHERE {where_sql}
                    ORDER BY {sort_by} {order}
                    LIMIT ${param_idx + 1} OFFSET ${param_idx + 2}
                \"\"\", *params, limit, offset)
                
                # Get total count
                total_count = await conn.fetchval(f\"\"\"
                    SELECT COUNT(*) FROM repositories WHERE {where_sql}
                \"\"\", *params)
                
                return web.json_response({
                    'repositories': [dict(repo) for repo in repos],
                    'count': len(repos),
                    'total': total_count,
                    'limit': limit,
                    'offset': offset
                }, default=str)
                
        except Exception as e:
            self.logger.error(f"Repositories query failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_repository_details(self, request):
        \"\"\"Get detailed repository information\"\"\"
        try:
            repo_id = request.match_info.get('repo_id')
            if not repo_id:
                return web.json_response({'error': 'Repository ID required'}, status=400)
            
            async with self.db.pool.acquire() as conn:
                # Repository info
                repo = await conn.fetchrow(\"\"\"
                    SELECT * FROM repositories WHERE id = $1
                \"\"\", int(repo_id))
                
                if not repo:
                    return web.json_response({'error': 'Repository not found'}, status=404)
                
                # Event statistics
                event_stats = await conn.fetch(\"\"\"
                    SELECT type, COUNT(*) as count
                    FROM github_events 
                    WHERE repo_id = $1
                    GROUP BY type 
                    ORDER BY count DESC
                \"\"\", int(repo_id))
                
                # Recent activity
                recent_events = await conn.fetch(\"\"\"
                    SELECT type, created_at, actor_login
                    FROM github_events 
                    WHERE repo_id = $1
                    ORDER BY created_at DESC 
                    LIMIT 20
                \"\"\", int(repo_id))
                
                return web.json_response({
                    'repository': dict(repo),
                    'event_statistics': [dict(stat) for stat in event_stats],
                    'recent_activity': [dict(event) for event in recent_events]
                }, default=str)
                
        except Exception as e:
            self.logger.error(f"Repository details query failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def get_languages(self, request):
        \"\"\"Get programming language statistics\"\"\"
        try:
            async with self.db.pool.acquire() as conn:
                languages = await conn.fetch(\"\"\"
                    SELECT 
                        language,
                        COUNT(*) as repository_count,
                        SUM(stargazers_count) as total_stars,
                        AVG(stargazers_count) as avg_stars
                    FROM repositories 
                    WHERE language IS NOT NULL
                    GROUP BY language 
                    ORDER BY repository_count DESC
                    LIMIT 50
                \"\"\")
                
                return web.json_response({
                    'languages': [dict(lang) for lang in languages]
                }, default=str)
                
        except Exception as e:
            self.logger.error(f"Languages query failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    
    async def search_repositories(self, request):
        \"\"\"Search repositories with full-text search\"\"\"
        try:
            data = await request.json()
            query = data.get('query', '').strip()
            limit = min(data.get('limit', 50), 500)
            
            if not query:
                return web.json_response({'error': 'Query required'}, status=400)
            
            async with self.db.pool.acquire() as conn:
                repos = await conn.fetch(\"\"\"
                    SELECT id, name, full_name, description, language,
                           stargazers_count, forks_count, owner_login,
                           ts_rank(to_tsvector('english', coalesce(name, '') || ' ' || coalesce(description, '')), 
                                   plainto_tsquery('english', $1)) as rank
                    FROM repositories 
                    WHERE to_tsvector('english', coalesce(name, '') || ' ' || coalesce(description, ''))
                          @@ plainto_tsquery('english', $1)
                       OR name ILIKE $2
                       OR full_name ILIKE $2
                    ORDER BY rank DESC, stargazers_count DESC
                    LIMIT $3
                \"\"\", query, f'%{query}%', limit)
                
                return web.json_response({
                    'repositories': [dict(repo) for repo in repos],
                    'query': query,
                    'count': len(repos)
                }, default=str)
                
        except Exception as e:
            self.logger.error(f"Repository search failed: {e}")
            return web.json_response({'error': str(e)}, status=500)
    """
    
    print("📝 Enhanced API endpoints code generated")
    print("To add these endpoints, add the following routes to SafeWebAPI.setup_routes():")
    print("self.app.router.add_get('/api/repositories', self.get_repositories)")
    print("self.app.router.add_get('/api/repositories/{repo_id}', self.get_repository_details)")
    print("self.app.router.add_get('/api/languages', self.get_languages)")
    print("self.app.router.add_post('/api/repositories/search', self.search_repositories)")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "endpoints":
        asyncio.run(create_enhanced_api_endpoints())
    else:
        asyncio.run(fix_repository_processing())
