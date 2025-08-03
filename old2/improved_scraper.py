#!/usr/bin/env python3
"""
Improved GitHub Archive Scraper with robust data validation and resource management
"""

import asyncio
import asyncpg
import json
import logging
import os
from datetime import datetime, timedelta
from dateutil.parser import parse as parse_date
from pathlib import Path
import gzip
import tempfile
import aiohttp
from typing import Dict, List, Optional

from config import Config

class ImprovedGitHubArchiveScraper:
    """Enhanced GitHub Archive Scraper with better data validation"""
    
    def __init__(self, config: Config):
        self.config = config
        self.logger = logging.getLogger(__name__)
        self.session = None
        self.db_pool = None
        
    async def __aenter__(self):
        await self.connect()
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.disconnect()
        
    async def connect(self):
        """Initialize database and HTTP connections"""
        # Database connection - use the scraper's default password
        dsn = f"postgresql://gharchive:gharchive@{self.config.DB_HOST}:{self.config.DB_PORT}/{self.config.DB_NAME}"
        self.db_pool = await asyncpg.create_pool(dsn, min_size=2, max_size=10)
        
        # HTTP session
        timeout = aiohttp.ClientTimeout(total=300)
        self.session = aiohttp.ClientSession(timeout=timeout)
        
        self.logger.info("ImprovedGitHubArchiveScraper initialized successfully")
        
    async def disconnect(self):
        """Close connections"""
        if self.session:
            await self.session.close()
        if self.db_pool:
            await self.db_pool.close()
            
    def validate_and_convert_event(self, event: Dict) -> Optional[Dict]:
        """
        Validate and convert event data with robust type checking
        Returns None if event is invalid, otherwise returns cleaned event
        """
        try:
            # Required fields check
            if not all(field in event for field in ['id', 'type', 'created_at']):
                return None
                
            # Validate and convert ID
            try:
                event_id = int(event['id'])
            except (ValueError, TypeError):
                self.logger.warning(f"Invalid event ID: {event.get('id')}")
                return None
                
            # Validate event type - must be string and not numeric
            event_type = event.get('type')
            if not isinstance(event_type, str) or not event_type or event_type.isdigit():
                self.logger.warning(f"Invalid event type '{event_type}' for event {event_id}")
                return None
                
            # Validate timestamp
            try:
                created_at = parse_date(event['created_at'])
            except:
                self.logger.warning(f"Invalid timestamp for event {event_id}")
                return None
                
            # Helper function for safe integer conversion
            def safe_int(value):
                if value is None or value == '':
                    return None
                try:
                    return int(value)
                except (ValueError, TypeError):
                    return None
                    
            # Clean actor data
            actor = event.get('actor', {})
            actor_data = {
                'id': safe_int(actor.get('id')),
                'login': actor.get('login'),
                'display_login': actor.get('display_login'),
                'gravatar_id': actor.get('gravatar_id'),
                'url': actor.get('url'),
                'avatar_url': actor.get('avatar_url')
            }
            
            # Clean repo data
            repo = event.get('repo', {})
            repo_data = {
                'id': safe_int(repo.get('id')),
                'name': repo.get('name'),
                'url': repo.get('url')
            }
            
            # Clean org data
            org = event.get('org', {})
            org_data = {
                'id': safe_int(org.get('id')) if org else None,
                'login': org.get('login') if org else None,
                'gravatar_id': org.get('gravatar_id') if org else None,
                'url': org.get('url') if org else None,
                'avatar_url': org.get('avatar_url') if org else None
            }
            
            return {
                'id': event_id,
                'type': event_type,
                'created_at': created_at,
                'public': event.get('public', True),
                'actor': actor_data,
                'repo': repo_data,
                'org': org_data,
                'payload': event.get('payload', {})
            }
            
        except Exception as e:
            self.logger.error(f"Error validating event: {e}")
            return None
            
    async def insert_validated_events(self, events: List[Dict], filename: str) -> int:
        """Insert pre-validated events into database"""
        if not events:
            return 0
            
        insert_sql = """
        INSERT INTO github_events (
            id, type, created_at, public,
            actor_id, actor_login, actor_display_login, actor_gravatar_id, actor_url, actor_avatar_url,
            repo_id, repo_name, repo_url,
            org_id, org_login, org_gravatar_id, org_url, org_avatar_url,
            payload, file_source
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
        ON CONFLICT (id) DO NOTHING
        """
        
        async with self.db_pool.acquire() as conn:
            async with conn.transaction():
                rows_affected = 0
                
                for event in events:
                    try:
                        result = await conn.execute(
                            insert_sql,
                            event['id'],
                            event['type'],
                            event['created_at'],
                            event['public'],
                            event['actor']['id'],
                            event['actor']['login'],
                            event['actor']['display_login'],
                            event['actor']['gravatar_id'],
                            event['actor']['url'],
                            event['actor']['avatar_url'],
                            event['repo']['id'],
                            event['repo']['name'],
                            event['repo']['url'],
                            event['org']['id'],
                            event['org']['login'],
                            event['org']['gravatar_id'],
                            event['org']['url'],
                            event['org']['avatar_url'],
                            json.dumps(event['payload']),
                            filename
                        )
                        
                        if result == "INSERT 0 1":
                            rows_affected += 1
                            
                    except Exception as e:
                        self.logger.error(f"Database error inserting event {event['id']}: {e}")
                        continue
                        
                return rows_affected
                
    async def process_file(self, filename: str, content: bytes) -> Dict:
        """Process a single GitHub Archive file with improved validation"""
        self.logger.info(f"Processing {filename}")
        
        total_events = 0
        valid_events = 0
        invalid_events = 0
        inserted_events = 0
        
        try:
            # Decompress content
            content_str = gzip.decompress(content).decode('utf-8')
            lines = content_str.strip().split('\n')
            
            batch_size = 500  # Smaller batch size for better memory management
            validated_events = []
            
            for line_num, line in enumerate(lines, 1):
                if not line.strip():
                    continue
                    
                total_events += 1
                
                try:
                    # Parse JSON
                    raw_event = json.loads(line.strip())
                    
                    # Validate and convert
                    validated_event = self.validate_and_convert_event(raw_event)
                    
                    if validated_event:
                        validated_events.append(validated_event)
                        valid_events += 1
                    else:
                        invalid_events += 1
                        
                    # Process in batches
                    if len(validated_events) >= batch_size:
                        batch_inserted = await self.insert_validated_events(validated_events, filename)
                        inserted_events += batch_inserted
                        validated_events = []
                        
                except json.JSONDecodeError:
                    invalid_events += 1
                    self.logger.warning(f"Invalid JSON in {filename} at line {line_num}")
                except Exception as e:
                    invalid_events += 1
                    self.logger.error(f"Error processing line {line_num} in {filename}: {e}")
                    
            # Process remaining events
            if validated_events:
                batch_inserted = await self.insert_validated_events(validated_events, filename)
                inserted_events += batch_inserted
                
            self.logger.info(f"Processed {filename}: {inserted_events}/{valid_events} events inserted ({invalid_events} invalid)")
            
            return {
                'total_events': total_events,
                'valid_events': valid_events,
                'invalid_events': invalid_events,
                'inserted_events': inserted_events
            }
            
        except Exception as e:
            self.logger.error(f"Fatal error processing {filename}: {e}")
            return {
                'total_events': 0,
                'valid_events': 0,
                'invalid_events': 0,
                'inserted_events': 0
            }
            
    async def download_and_process_file(self, url: str) -> Dict:
        """Download and process a single file"""
        filename = url.split('/')[-1]
        
        try:
            async with self.session.get(url) as response:
                if response.status == 200:
                    content = await response.read()
                    return await self.process_file(filename, content)
                else:
                    self.logger.error(f"Failed to download {url}: HTTP {response.status}")
                    return {'total_events': 0, 'valid_events': 0, 'invalid_events': 0, 'inserted_events': 0}
                    
        except Exception as e:
            self.logger.error(f"Error downloading {url}: {e}")
            return {'total_events': 0, 'valid_events': 0, 'invalid_events': 0, 'inserted_events': 0}
            
async def test_improved_scraper():
    """Test the improved scraper with a single file"""
    config = Config()
    
    # Setup logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    async with ImprovedGitHubArchiveScraper(config) as scraper:
        # Test with recent file
        test_url = "https://data.gharchive.org/2025-07-26-23.json.gz"
        result = await scraper.download_and_process_file(test_url)
        
        print(f"✅ Test Results:")
        print(f"   Total events: {result['total_events']:,}")
        print(f"   Valid events: {result['valid_events']:,}")
        print(f"   Invalid events: {result['invalid_events']:,}")
        print(f"   Inserted events: {result['inserted_events']:,}")
        
if __name__ == "__main__":
    asyncio.run(test_improved_scraper())
