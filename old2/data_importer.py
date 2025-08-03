#!/usr/bin/env python3
"""
Multi-Source Data Importer for GitHub Archive Scraper
Supports JSON files, BigQuery, and other data sources
"""

import asyncio
import json
import gzip
import logging
from pathlib import Path
from typing import Dict, List, Any, Optional, AsyncGenerator
from datetime import datetime
import aiofiles
import sys

# Add project directory to path
sys.path.insert(0, str(Path(__file__).parent))

from config import Config
from db_manager import GitHubArchiveDB


class DataImporter:
    """Multi-source data importer"""
    
    def __init__(self, config: Config):
        self.config = config
        self.db = GitHubArchiveDB(config)
        self.logger = logging.getLogger(__name__)
        
        # Stats tracking
        self.import_stats = {
            'events_processed': 0,
            'events_inserted': 0,
            'repositories_processed': 0,
            'repositories_inserted': 0,
            'errors': 0,
            'start_time': None,
            'end_time': None
        }
        
    async def __aenter__(self):
        """Async context manager entry"""
        await self.db.connect()
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit"""
        await self.db.disconnect()
        
    async def import_json_file(self, file_path: Path, source: str = 'manual_import') -> Dict[str, int]:
        """Import data from a JSON file (supports .gz compression)"""
        self.logger.info(f"Starting import from JSON file: {file_path}")
        self._reset_stats()
        
        try:
            if file_path.suffix == '.gz':
                # Handle gzipped files
                async for event_data in self._read_gzipped_json(file_path):
                    await self._process_event(event_data, source)
            else:
                # Handle regular JSON files
                async for event_data in self._read_json_file(file_path):
                    await self._process_event(event_data, source)
                    
            self._finalize_stats()
            self.logger.info(f"Import completed: {self.import_stats}")
            return self.import_stats.copy()
            
        except Exception as e:
            self.logger.error(f"Error importing JSON file {file_path}: {e}")
            self.import_stats['errors'] += 1
            return self.import_stats.copy()
            
    async def import_json_directory(self, directory_path: Path, 
                                  pattern: str = "*.json*") -> Dict[str, int]:
        """Import all JSON files from a directory"""
        self.logger.info(f"Starting bulk import from directory: {directory_path}")
        self._reset_stats()
        
        try:
            json_files = list(directory_path.glob(pattern))
            self.logger.info(f"Found {len(json_files)} files to import")
            
            for file_path in json_files:
                self.logger.info(f"Importing {file_path.name}")
                file_stats = await self.import_json_file(file_path, f'bulk_import_{file_path.name}')
                
                # Aggregate stats
                for key in ['events_processed', 'events_inserted', 'repositories_processed', 
                          'repositories_inserted', 'errors']:
                    self.import_stats[key] += file_stats[key]
                    
            self._finalize_stats()
            self.logger.info(f"Bulk import completed: {self.import_stats}")
            return self.import_stats.copy()
            
        except Exception as e:
            self.logger.error(f"Error in bulk import: {e}")
            return self.import_stats.copy()
            
    async def import_from_bigquery(self, project_id: str, dataset_id: str, 
                                 table_id: str, date_filter: Optional[str] = None) -> Dict[str, int]:
        """Import data from Google BigQuery"""
        try:
            from google.cloud import bigquery
            from google.oauth2 import service_account
            
            self.logger.info(f"Starting BigQuery import from {project_id}.{dataset_id}.{table_id}")
            self._reset_stats()
            
            # Initialize BigQuery client
            client = bigquery.Client(project=project_id)
            
            # Build query
            query = f"""
            SELECT 
                id,
                type,
                actor,
                repo,
                payload,
                public,
                created_at,
                org
            FROM `{project_id}.{dataset_id}.{table_id}`
            """
            
            if date_filter:
                query += f" WHERE created_at >= '{date_filter}'"
                
            query += " ORDER BY created_at"
            
            self.logger.info(f"Executing BigQuery: {query}")
            
            # Execute query and process results
            query_job = client.query(query)
            
            async for row in self._bigquery_rows_async(query_job):
                event_data = {
                    'id': row.id,
                    'type': row.type,
                    'actor': dict(row.actor) if row.actor else {},
                    'repo': dict(row.repo) if row.repo else {},
                    'payload': dict(row.payload) if row.payload else {},
                    'public': row.public,
                    'created_at': row.created_at.isoformat() if row.created_at else None,
                    'org': dict(row.org) if row.org else None
                }
                
                await self._process_event(event_data, f'bigquery_{table_id}')
                
            self._finalize_stats()
            self.logger.info(f"BigQuery import completed: {self.import_stats}")
            return self.import_stats.copy()
            
        except ImportError:
            self.logger.error("Google Cloud BigQuery library not installed. Run: pip install google-cloud-bigquery")
            return {'error': 'BigQuery library not available'}
        except Exception as e:
            self.logger.error(f"BigQuery import error: {e}")
            self.import_stats['errors'] += 1
            return self.import_stats.copy()
            
    async def _read_gzipped_json(self, file_path: Path) -> AsyncGenerator[Dict[str, Any], None]:
        """Read events from gzipped JSON file"""
        try:
            with gzip.open(file_path, 'rt', encoding='utf-8') as f:
                for line_num, line in enumerate(f, 1):
                    try:
                        if line.strip():
                            yield json.loads(line.strip())
                    except json.JSONDecodeError as e:
                        self.logger.warning(f"JSON decode error on line {line_num}: {e}")
                        self.import_stats['errors'] += 1
                        continue
                        
        except Exception as e:
            self.logger.error(f"Error reading gzipped file {file_path}: {e}")
            raise
            
    async def _read_json_file(self, file_path: Path) -> AsyncGenerator[Dict[str, Any], None]:
        """Read events from regular JSON file"""
        try:
            async with aiofiles.open(file_path, 'r', encoding='utf-8') as f:
                async for line_num, line in enumerate(f, 1):
                    try:
                        if line.strip():
                            yield json.loads(line.strip())
                    except json.JSONDecodeError as e:
                        self.logger.warning(f"JSON decode error on line {line_num}: {e}")
                        self.import_stats['errors'] += 1
                        continue
                        
        except Exception as e:
            self.logger.error(f"Error reading file {file_path}: {e}")
            raise
            
    async def _bigquery_rows_async(self, query_job):
        """Convert BigQuery job results to async generator"""
        # This is a simple implementation - in production you'd want proper async BigQuery
        for row in query_job:
            yield row
            await asyncio.sleep(0)  # Allow other coroutines to run
            
    async def _process_event(self, event_data: Dict[str, Any], source: str):
        """Process a single event"""
        try:
            self.import_stats['events_processed'] += 1
            
            # Extract event information
            event_id = event_data.get('id')
            event_type = event_data.get('type')
            actor = event_data.get('actor', {})
            repo = event_data.get('repo', {})
            payload = event_data.get('payload', {})
            created_at = event_data.get('created_at')
            
            if not all([event_id, event_type, created_at]):
                self.logger.warning(f"Skipping incomplete event: {event_id}")
                return
                
            # Parse created_at
            if isinstance(created_at, str):
                try:
                    created_at = datetime.fromisoformat(created_at.replace('Z', '+00:00'))
                except ValueError:
                    created_at = datetime.fromisoformat(created_at)
                    
            # Extract repository information if available
            repo_id = repo.get('id')
            repo_name = repo.get('name')
            
            # Insert/update repository if we have the data
            if repo_id and repo_name:
                await self._upsert_repository(repo_id, repo_name, repo)
                
            # Insert event
            await self._insert_event(
                event_id=event_id,
                event_type=event_type,
                actor_id=actor.get('id'),
                actor_login=actor.get('login'),
                repo_id=repo_id,
                repo_name=repo_name,
                payload=payload,
                created_at=created_at,
                source=source
            )
            
            self.import_stats['events_inserted'] += 1
            
        except Exception as e:
            self.logger.error(f"Error processing event {event_data.get('id', 'unknown')}: {e}")
            self.import_stats['errors'] += 1
            
    async def _upsert_repository(self, repo_id: int, repo_name: str, repo_data: Dict[str, Any]):
        """Insert or update repository information"""
        try:
            async with self.db.pool.acquire() as conn:
                # Check if repository exists
                existing = await conn.fetchrow(
                    "SELECT id FROM repositories WHERE id = $1", repo_id
                )
                
                if not existing:
                    # Insert new repository with available data
                    await conn.execute("""
                        INSERT INTO repositories (
                            id, name, full_name, description, html_url, 
                            owner_login, owner_type, created_at, updated_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
                        ON CONFLICT (id) DO UPDATE SET
                            name = EXCLUDED.name,
                            full_name = EXCLUDED.full_name,
                            last_updated_at = NOW()
                    """,
                        repo_id,
                        repo_name,
                        repo_data.get('name', repo_name),
                        repo_data.get('description'),
                        repo_data.get('url'),
                        repo_data.get('owner', {}).get('login'),
                        repo_data.get('owner', {}).get('type'),
                    )
                    self.import_stats['repositories_inserted'] += 1
                    
            self.import_stats['repositories_processed'] += 1
            
        except Exception as e:
            self.logger.error(f"Error upserting repository {repo_id}: {e}")
            
    async def _insert_event(self, event_id: str, event_type: str, actor_id: Optional[int],
                          actor_login: Optional[str], repo_id: Optional[int], 
                          repo_name: Optional[str], payload: Dict[str, Any],
                          created_at: datetime, source: str):
        """Insert event into database"""
        try:
            async with self.db.pool.acquire() as conn:
                await conn.execute("""
                    INSERT INTO github_events (
                        id, type, actor_id, actor_login, repo_id, repo_name,
                        payload, created_at, source
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (id) DO NOTHING
                """,
                    event_id, event_type, actor_id, actor_login, repo_id, 
                    repo_name, json.dumps(payload), created_at, source
                )
                
        except Exception as e:
            self.logger.error(f"Error inserting event {event_id}: {e}")
            raise
            
    def _reset_stats(self):
        """Reset import statistics"""
        self.import_stats = {
            'events_processed': 0,
            'events_inserted': 0,
            'repositories_processed': 0,
            'repositories_inserted': 0,
            'errors': 0,
            'start_time': datetime.now(),
            'end_time': None
        }
        
    def _finalize_stats(self):
        """Finalize import statistics"""
        self.import_stats['end_time'] = datetime.now()
        duration = self.import_stats['end_time'] - self.import_stats['start_time']
        self.import_stats['duration_seconds'] = duration.total_seconds()
        
    def get_import_status(self) -> Dict[str, Any]:
        """Get current import status"""
        return self.import_stats.copy()


async def main():
    """CLI interface for data importer"""
    import argparse
    
    parser = argparse.ArgumentParser(description='GitHub Archive Data Importer')
    parser.add_argument('command', choices=['json-file', 'json-dir', 'bigquery'], 
                       help='Import command')
    parser.add_argument('--file', help='JSON file path')
    parser.add_argument('--directory', help='Directory containing JSON files')
    parser.add_argument('--pattern', default='*.json*', help='File pattern for directory import')
    parser.add_argument('--project', help='BigQuery project ID')
    parser.add_argument('--dataset', help='BigQuery dataset ID')
    parser.add_argument('--table', help='BigQuery table ID')
    parser.add_argument('--date-filter', help='Date filter for BigQuery (YYYY-MM-DD)')
    
    args = parser.parse_args()
    
    logging.basicConfig(level=logging.INFO)
    config = Config()
    
    async with DataImporter(config) as importer:
        if args.command == 'json-file':
            if not args.file:
                print("--file is required for json-file command")
                return
            result = await importer.import_json_file(Path(args.file))
            
        elif args.command == 'json-dir':
            if not args.directory:
                print("--directory is required for json-dir command")
                return
            result = await importer.import_json_directory(Path(args.directory), args.pattern)
            
        elif args.command == 'bigquery':
            if not all([args.project, args.dataset, args.table]):
                print("--project, --dataset, and --table are required for bigquery command")
                return
            result = await importer.import_from_bigquery(
                args.project, args.dataset, args.table, args.date_filter
            )
            
        print(f"Import completed: {result}")


if __name__ == '__main__':
    asyncio.run(main())
