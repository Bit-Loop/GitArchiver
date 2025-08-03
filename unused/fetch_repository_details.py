#!/usr/bin/env python3
"""
Proper fix for GitHub Archive repository tracking
The issue is that GitHub Archive format doesn't include full repository data in payload
We need to fetch repository details using GitHub API
"""

import asyncio
import sys
import aiohttp
import json
from pathlib import Path
import logging
from datetime import datetime

# Add project directory to path
sys.path.insert(0, str(Path(__file__).parent))

from config import Config
from gharchive_scraper import GitHubArchiveScraper


def parse_github_datetime(date_string):
    """Convert GitHub API datetime string to datetime object"""
    if date_string is None:
        return None
    # GitHub API returns dates in ISO format like "2021-12-02T16:58:58Z"
    try:
        return datetime.fromisoformat(date_string.replace('Z', '+00:00'))
    except (ValueError, AttributeError):
        return None


async def fetch_repository_details():
    """Fetch repository details from GitHub API for existing repo_ids"""
    
    print("🔧 Fetching Repository Details from GitHub API")
    print("=" * 50)
    
    config = Config()
    
    # Initialize scraper
    async with GitHubArchiveScraper(config) as scraper:
        async with scraper.db.pool.acquire() as conn:
            
            # 1. Get unique repository IDs and names from events
            print("📊 Getting unique repositories from events...")
            
            repos_from_events = await conn.fetch("""
                SELECT DISTINCT 
                    repo_id,
                    repo_name,
                    COUNT(*) as event_count,
                    MIN(created_at) as first_seen,
                    MAX(created_at) as last_activity
                FROM github_events 
                WHERE repo_id IS NOT NULL AND repo_name IS NOT NULL
                GROUP BY repo_id, repo_name
                ORDER BY event_count DESC
            """)
            
            print(f"Found {len(repos_from_events)} unique repositories")
            
            # 2. Fetch details from GitHub API
            async with aiohttp.ClientSession() as session:
                inserted_count = 0
                updated_count = 0
                failed_count = 0
                
                for i, repo_info in enumerate(repos_from_events[:100]):  # Limit to first 100 for demo
                    repo_id = repo_info['repo_id']
                    repo_name = repo_info['repo_name']
                    
                    try:
                        # GitHub API endpoint
                        url = f"https://api.github.com/repositories/{repo_id}"
                        headers = {}
                        
                        # Add GitHub token if available
                        github_token = getattr(config, 'GITHUB_TOKEN', None)
                        if github_token:
                            headers['Authorization'] = f'token {github_token}'
                        
                        print(f"[{i+1}/{len(repos_from_events)}] Fetching {repo_name}...")
                        
                        async with session.get(url, headers=headers) as response:
                            if response.status == 200:
                                repo_data = await response.json()
                                
                                # Insert/update repository
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
                                repo_data['id'],
                                repo_data['name'],
                                repo_data['full_name'],
                                repo_data.get('description'),
                                repo_data['html_url'],
                                repo_data.get('language'),
                                repo_data.get('default_branch'),
                                parse_github_datetime(repo_data['created_at']),
                                parse_github_datetime(repo_data['updated_at']),
                                repo_data.get('size', 0),
                                repo_data.get('stargazers_count', 0),
                                repo_data.get('forks_count', 0),
                                repo_data.get('private', False),
                                repo_data.get('fork', False),
                                repo_data.get('archived', False),
                                repo_data['owner']['login'],
                                repo_data['owner']['type'],
                                repo_info['first_seen'])
                                
                                if "INSERT" in result:
                                    inserted_count += 1
                                else:
                                    updated_count += 1
                                    
                            elif response.status == 404:
                                print(f"  Repository {repo_name} not found (deleted?)")
                                failed_count += 1
                            elif response.status == 403:
                                print(f"  Rate limited or access denied for {repo_name}")
                                failed_count += 1
                                # Add a longer delay for rate limiting
                                await asyncio.sleep(2)
                            else:
                                print(f"  Error {response.status} for {repo_name}")
                                failed_count += 1
                        
                        # Rate limiting - be nice to GitHub API
                        await asyncio.sleep(0.1)
                        
                    except Exception as e:
                        print(f"  Error processing {repo_name}: {e}")
                        failed_count += 1
                        continue
                
                print(f"\n✅ Repository fetch completed:")
                print(f"  Inserted: {inserted_count}")
                print(f"  Updated: {updated_count}")
                print(f"  Failed: {failed_count}")
            
            # 3. Create simplified repository entries for remaining repos
            print(f"\n📝 Creating simplified entries for remaining repositories...")
            
            # Get repos not in repositories table
            remaining_repos = await conn.fetch("""
                SELECT DISTINCT e.repo_id, e.repo_name, 
                       COUNT(*) as event_count,
                       MIN(e.created_at) as first_seen,
                       MAX(e.created_at) as last_activity
                FROM github_events e
                LEFT JOIN repositories r ON e.repo_id = r.id
                WHERE e.repo_id IS NOT NULL AND e.repo_name IS NOT NULL 
                  AND r.id IS NULL
                GROUP BY e.repo_id, e.repo_name
            """)
            
            simple_inserted = 0
            for repo_info in remaining_repos:
                try:
                    # Extract owner from repo name
                    parts = repo_info['repo_name'].split('/')
                    owner_login = parts[0] if len(parts) > 1 else 'unknown'
                    name = parts[1] if len(parts) > 1 else repo_info['repo_name']
                    
                    await conn.execute("""
                        INSERT INTO repositories (
                            id, name, full_name, owner_login, owner_type,
                            first_seen_at, last_updated_at
                        ) VALUES ($1, $2, $3, $4, 'User', $5, NOW())
                        ON CONFLICT (id) DO NOTHING
                    """,
                    repo_info['repo_id'],
                    name,
                    repo_info['repo_name'],
                    owner_login,
                    repo_info['first_seen'])
                    
                    simple_inserted += 1
                    
                except Exception as e:
                    print(f"Error creating simple entry for {repo_info['repo_name']}: {e}")
            
            print(f"✅ Created {simple_inserted} simplified repository entries")
            
            # 4. Update processed files tracking
            print(f"\n📁 Updating processed files tracking...")
            
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
            
            # 5. Final statistics
            print(f"\n📊 Final Statistics:")
            
            total_events = await conn.fetchval("SELECT COUNT(*) FROM github_events")
            total_repos = await conn.fetchval("SELECT COUNT(*) FROM repositories")
            total_files = await conn.fetchval("SELECT COUNT(*) FROM processed_files")
            
            print(f"  Total Events: {total_events:,}")
            print(f"  Total Repositories: {total_repos:,}")
            print(f"  Processed Files: {total_files:,}")
            
            # Top repositories by activity
            top_repos = await conn.fetch("""
                SELECT r.full_name, r.stargazers_count, r.language, COUNT(e.id) as events
                FROM repositories r
                JOIN github_events e ON r.id = e.repo_id
                WHERE r.stargazers_count > 0
                GROUP BY r.id, r.full_name, r.stargazers_count, r.language
                ORDER BY events DESC
                LIMIT 10
            """)
            
            print(f"\n🌟 Top Repositories by Activity:")
            for repo in top_repos:
                print(f"  {repo['full_name']} ({repo['language']}) - {repo['stargazers_count']} stars, {repo['events']} events")
    
    print(f"\n🎉 Repository processing completed successfully!")


if __name__ == "__main__":
    asyncio.run(fetch_repository_details())
