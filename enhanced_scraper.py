#!/usr/bin/env python3
"""
Professional GitHub Archive Scraper - Enhanced Version
Consolidated, professional implementation with comprehensive event capture,
resource monitoring, and robust error handling.
"""

import asyncio
import aiohttp
import gzip
import json
import logging
import signal
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Any, AsyncGenerator
import psutil
import xml.etree.ElementTree as ET

# Import our consolidated modules
from core.config import Config
from core.database import DatabaseManager, QualityMetrics, DatabaseHealth
from core.auth import AuthManager


class ResourceMonitor:
    """Professional resource monitoring with Oracle Cloud optimization"""
    
    def __init__(self, config: Config):
        self.config = config
        self.logger = logging.getLogger(__name__)
        self.limits = config.get_resource_limits()
        self.emergency_mode = False
        
    def get_resource_status(self) -> Dict[str, Any]:
        """Get current resource usage status"""
        try:
            # Memory usage
            memory = psutil.virtual_memory()
            memory_used_gb = memory.used / (1024**3)
            memory_percent = (memory_used_gb / self.config.resources.memory_limit_gb) * 100
            
            # Disk usage
            disk = psutil.disk_usage('/')
            disk_used_gb = disk.used / (1024**3)
            disk_percent = (disk_used_gb / self.config.resources.disk_limit_gb) * 100
            
            # CPU usage
            cpu_percent = psutil.cpu_percent(interval=1)
            
            # Check emergency thresholds
            emergency_conditions = []
            if memory_percent > (self.config.resources.emergency_cleanup_threshold * 100):
                emergency_conditions.append('memory')
            if disk_percent > (self.config.resources.emergency_cleanup_threshold * 100):
                emergency_conditions.append('disk')
            if cpu_percent > (self.config.resources.cpu_limit_percent * self.config.resources.emergency_cleanup_threshold):
                emergency_conditions.append('cpu')
            
            self.emergency_mode = bool(emergency_conditions)
            
            return {
                'memory': {
                    'used_gb': round(memory_used_gb, 2),
                    'limit_gb': self.config.resources.memory_limit_gb,
                    'percent': round(memory_percent, 1),
                    'warning': memory_percent > (self.config.resources.memory_warning_threshold * 100)
                },
                'disk': {
                    'used_gb': round(disk_used_gb, 2),
                    'limit_gb': self.config.resources.disk_limit_gb,
                    'percent': round(disk_percent, 1),
                    'warning': disk_percent > (self.config.resources.disk_warning_threshold * 100)
                },
                'cpu': {
                    'percent': round(cpu_percent, 1),
                    'limit_percent': self.config.resources.cpu_limit_percent,
                    'warning': cpu_percent > (self.config.resources.cpu_warning_threshold * self.config.resources.cpu_limit_percent)
                },
                'emergency_mode': self.emergency_mode,
                'emergency_conditions': emergency_conditions,
                'timestamp': datetime.now(timezone.utc).isoformat()
            }
            
        except Exception as e:
            self.logger.error(f"Resource monitoring error: {e}")
            return {
                'error': str(e),
                'emergency_mode': True,
                'timestamp': datetime.now(timezone.utc).isoformat()
            }
    
    async def emergency_cleanup(self) -> Dict[str, Any]:
        """Perform emergency resource cleanup"""
        self.logger.warning("Starting emergency resource cleanup")
        cleanup_actions = []
        
        try:
            # Clean temporary files
            temp_files_cleaned = await self._cleanup_temp_files()
            cleanup_actions.append(f"Cleaned {temp_files_cleaned} temporary files")
            
            # Clean old logs
            log_files_cleaned = await self._cleanup_old_logs()
            cleanup_actions.append(f"Cleaned {log_files_cleaned} old log files")
            
            # Force garbage collection
            import gc
            gc.collect()
            cleanup_actions.append("Forced garbage collection")
            
            # Clear caches if possible
            await self._clear_caches()
            cleanup_actions.append("Cleared application caches")
            
            self.logger.info(f"Emergency cleanup completed: {cleanup_actions}")
            
            return {
                'success': True,
                'actions': cleanup_actions,
                'timestamp': datetime.now(timezone.utc).isoformat()
            }
            
        except Exception as e:
            self.logger.error(f"Emergency cleanup failed: {e}")
            return {
                'success': False,
                'error': str(e),
                'actions': cleanup_actions,
                'timestamp': datetime.now(timezone.utc).isoformat()
            }
    
    async def _cleanup_temp_files(self) -> int:
        """Clean temporary files"""
        count = 0
        try:
            # Clean download directory temporary files
            temp_patterns = ['*.tmp', '*.temp', '*.partial']
            for pattern in temp_patterns:
                for temp_file in self.config.download.download_dir.glob(pattern):
                    try:
                        temp_file.unlink()
                        count += 1
                    except Exception:
                        pass
        except Exception as e:
            self.logger.error(f"Temp file cleanup error: {e}")
        return count
    
    async def _cleanup_old_logs(self) -> int:
        """Clean old log files"""
        count = 0
        try:
            # Keep only recent log files
            cutoff_time = time.time() - (7 * 24 * 3600)  # 7 days
            for log_file in self.config.logging.log_dir.glob('*.log*'):
                try:
                    if log_file.stat().st_mtime < cutoff_time:
                        log_file.unlink()
                        count += 1
                except Exception:
                    pass
        except Exception as e:
            self.logger.error(f"Log cleanup error: {e}")
        return count
    
    async def _clear_caches(self) -> None:
        """Clear application caches"""
        # This can be extended to clear specific application caches
        pass


class GitHubArchiveScraper:
    """
    Professional GitHub Archive Scraper with comprehensive event capture,
    resource monitoring, and robust error handling.
    """
    
    def __init__(self, config: Optional[Config] = None):
        """Initialize the enhanced scraper"""
        self.config = config or Config()
        self.logger = logging.getLogger(__name__)
        
        # Initialize core components
        self.db = DatabaseManager(self.config)
        self.auth = AuthManager()
        self.resource_monitor = ResourceMonitor(self.config)
        
        # HTTP session for downloads
        self.session: Optional[aiohttp.ClientSession] = None
        
        # Shutdown handling
        self.shutdown_event = asyncio.Event()
        self._setup_signal_handlers()
        
        # Statistics tracking
        self.stats = {
            'files_processed': 0,
            'events_processed': 0,
            'errors_encountered': 0,
            'start_time': None,
            'last_activity': None
        }
    
    async def initialize(self) -> None:
        """Initialize all components"""
        self.logger.info("Initializing GitHub Archive Scraper...")
        
        try:
            # Connect to database
            await self.db.connect()
            self.logger.info("Database connection established")
            
            # Initialize HTTP session
            connector = aiohttp.TCPConnector(
                limit=50,
                limit_per_host=10,
                ttl_dns_cache=300,
                use_dns_cache=True
            )
            
            timeout = aiohttp.ClientTimeout(total=self.config.download.request_timeout)
            
            self.session = aiohttp.ClientSession(
                connector=connector,
                timeout=timeout,
                headers={
                    'User-Agent': 'GitHub-Archive-Scraper/2.0 (Professional)',
                    'Accept-Encoding': 'gzip, deflate'
                }
            )
            
            self.logger.info("HTTP session initialized")
            
            # Initialize auth manager with admin user if needed
            self._initialize_auth()
            
            self.logger.info("GitHub Archive Scraper initialization complete")
            
        except Exception as e:
            self.logger.error(f"Initialization failed: {e}")
            raise
    
    async def shutdown(self) -> None:
        """Graceful shutdown"""
        self.logger.info("Starting graceful shutdown...")
        
        try:
            # Close HTTP session
            if self.session:
                await self.session.close()
                self.logger.info("HTTP session closed")
            
            # Disconnect from database
            await self.db.disconnect()
            self.logger.info("Database connection closed")
            
            self.logger.info("Graceful shutdown complete")
            
        except Exception as e:
            self.logger.error(f"Shutdown error: {e}")
    
    async def get_available_files(self) -> List[Dict[str, Any]]:
        """Get list of available archive files from GitHub Archive"""
        try:
            self.logger.info("Fetching available archive files...")
            
            async with self.session.get(self.config.download.s3_list_url) as response:
                if response.status != 200:
                    raise Exception(f"Failed to fetch file list: HTTP {response.status}")
                
                content = await response.text()
                
            # Parse XML response
            root = ET.fromstring(content)
            files = []
            
            for contents in root.findall('.//{http://doc.s3.amazonaws.com/2006-03-01}Contents'):
                key = contents.find('{http://doc.s3.amazonaws.com/2006-03-01}Key')
                last_modified = contents.find('{http://doc.s3.amazonaws.com/2006-03-01}LastModified')
                size = contents.find('{http://doc.s3.amazonaws.com/2006-03-01}Size')
                etag = contents.find('{http://doc.s3.amazonaws.com/2006-03-01}ETag')
                
                if key is not None and key.text.endswith('.json.gz'):
                    files.append({
                        'filename': key.text,
                        'url': f"{self.config.download.base_url}{key.text}",
                        'last_modified': last_modified.text if last_modified is not None else None,
                        'size': int(size.text) if size is not None else 0,
                        'etag': etag.text.strip('"') if etag is not None else None
                    })
            
            self.logger.info(f"Found {len(files)} archive files")
            return sorted(files, key=lambda x: x['filename'])
            
        except Exception as e:
            self.logger.error(f"Failed to get available files: {e}")
            raise
    
    async def process_archive_file(self, file_info: Dict[str, Any]) -> Dict[str, Any]:
        """
        Process a single archive file with comprehensive event capture.
        
        Args:
            file_info: File information dictionary
            
        Returns:
            Processing results dictionary
        """
        filename = file_info['filename']
        url = file_info['url']
        etag = file_info.get('etag')
        size = file_info.get('size', 0)
        
        self.logger.info(f"Processing archive file: {filename}")
        
        # Check if already processed
        if await self.db.is_file_processed(filename, etag, size):
            self.logger.info(f"File {filename} already processed, skipping")
            return {
                'filename': filename,
                'status': 'skipped',
                'reason': 'already_processed',
                'events_processed': 0
            }
        
        try:
            # Check resource status before processing
            resource_status = self.resource_monitor.get_resource_status()
            if resource_status.get('emergency_mode'):
                self.logger.warning(f"Emergency mode active, performing cleanup before processing {filename}")
                await self.resource_monitor.emergency_cleanup()
            
            # Download and process file
            events_processed = 0
            
            async with self.session.get(url) as response:
                if response.status != 200:
                    raise Exception(f"Download failed: HTTP {response.status}")
                
                # Read compressed data
                compressed_data = await response.read()
                
                # Decompress and parse events
                decompressed_data = gzip.decompress(compressed_data)
                lines = decompressed_data.decode('utf-8').strip().split('\n')
                
                # Process events in batches
                events_batch = []
                for line in lines:
                    if not line.strip():
                        continue
                    
                    try:
                        event = json.loads(line)
                        events_batch.append(event)
                        
                        # Process batch when it reaches configured size
                        if len(events_batch) >= self.config.download.batch_size:
                            batch_processed = await self.db.insert_events_batch(events_batch, filename)
                            events_processed += batch_processed
                            events_batch = []
                            
                            # Update statistics
                            self.stats['events_processed'] += batch_processed
                            self.stats['last_activity'] = time.time()
                            
                    except json.JSONDecodeError as e:
                        self.logger.warning(f"Invalid JSON in {filename}: {e}")
                        continue
                
                # Process remaining events
                if events_batch:
                    batch_processed = await self.db.insert_events_batch(events_batch, filename)
                    events_processed += batch_processed
                    self.stats['events_processed'] += batch_processed
            
            # Mark file as processed
            await self.db.mark_file_processed(filename, etag, size, events_processed)
            
            # Update statistics
            self.stats['files_processed'] += 1
            self.stats['last_activity'] = time.time()
            
            self.logger.info(f"Successfully processed {filename}: {events_processed} events")
            
            return {
                'filename': filename,
                'status': 'success',
                'events_processed': events_processed,
                'file_size': size,
                'processing_time': time.time()
            }
            
        except Exception as e:
            self.stats['errors_encountered'] += 1
            self.logger.error(f"Failed to process {filename}: {e}")
            return {
                'filename': filename,
                'status': 'error',
                'error': str(e),
                'events_processed': 0
            }
    
    async def run_continuous_scraping(self, max_files: Optional[int] = None) -> None:
        """
        Run continuous scraping of GitHub Archive files.
        
        Args:
            max_files: Maximum number of files to process (None for unlimited)
        """
        self.logger.info("Starting continuous scraping...")
        self.stats['start_time'] = time.time()
        
        try:
            # Get available files
            available_files = await self.get_available_files()
            
            if max_files:
                available_files = available_files[:max_files]
            
            self.logger.info(f"Processing {len(available_files)} files...")
            
            # Process files with concurrency control
            semaphore = asyncio.Semaphore(self.config.download.max_concurrent_downloads)
            
            async def process_with_semaphore(file_info):
                async with semaphore:
                    if self.shutdown_event.is_set():
                        return None
                    return await self.process_archive_file(file_info)
            
            # Process files in batches to avoid overwhelming the system
            if len(available_files) == 0:
                self.logger.warning("No archive files to process")
                return
            
            batch_size = min(10, len(available_files))
            for i in range(0, len(available_files), batch_size):
                if self.shutdown_event.is_set():
                    break
                
                batch = available_files[i:i + batch_size]
                tasks = [process_with_semaphore(file_info) for file_info in batch]
                
                results = await asyncio.gather(*tasks, return_exceptions=True)
                
                # Log batch results
                successful = sum(1 for r in results if isinstance(r, dict) and r.get('status') == 'success')
                self.logger.info(f"Batch {i//batch_size + 1}: {successful}/{len(batch)} files processed successfully")
                
                # Brief pause between batches to allow system recovery
                if i + batch_size < len(available_files):
                    await asyncio.sleep(2)
            
            # Final statistics
            duration = time.time() - self.stats['start_time']
            self.logger.info(f"Scraping completed: {self.stats['files_processed']} files, "
                           f"{self.stats['events_processed']} events, "
                           f"{self.stats['errors_encountered']} errors in {duration:.1f}s")
            
        except Exception as e:
            self.logger.error(f"Continuous scraping failed: {e}")
            raise
    
    async def get_status(self) -> Dict[str, Any]:
        """Get comprehensive scraper status"""
        try:
            # Get resource status
            resource_status = self.resource_monitor.get_resource_status()
            
            # Get database health
            db_health = await self.db.health_check()
            
            # Get data quality metrics
            quality_metrics = await self.db.get_data_quality_metrics()
            
            # Calculate runtime
            runtime = time.time() - self.stats['start_time'] if self.stats['start_time'] else 0
            
            return {
                'scraper': {
                    'status': 'running' if not self.shutdown_event.is_set() else 'stopping',
                    'runtime_seconds': runtime,
                    'files_processed': self.stats['files_processed'],
                    'events_processed': self.stats['events_processed'],
                    'errors_encountered': self.stats['errors_encountered'],
                    'last_activity': self.stats['last_activity']
                },
                'resources': resource_status,
                'database': {
                    'connected': db_health.is_connected,
                    'connection_count': db_health.connection_count,
                    'active_queries': db_health.active_queries,
                    'cache_hit_ratio': db_health.cache_hit_ratio,
                    'error_message': db_health.error_message
                },
                'data_quality': {
                    'total_events': quality_metrics.total_events,
                    'unique_actors': quality_metrics.unique_actors,
                    'unique_repos': quality_metrics.unique_repos,
                    'quality_score': quality_metrics.quality_score,
                    'integrity_issues': quality_metrics.integrity_issues
                },
                'timestamp': datetime.now(timezone.utc).isoformat()
            }
            
        except Exception as e:
            self.logger.error(f"Status check failed: {e}")
            return {
                'error': str(e),
                'timestamp': datetime.now(timezone.utc).isoformat()
            }
    
    def _setup_signal_handlers(self) -> None:
        """Set up signal handlers for graceful shutdown"""
        def signal_handler(signum, frame):
            self.logger.info(f"Received signal {signum}, initiating shutdown...")
            self.shutdown_event.set()
        
        signal.signal(signal.SIGTERM, signal_handler)
        signal.signal(signal.SIGINT, signal_handler)
    
    def _initialize_auth(self) -> None:
        """Initialize authentication with default admin user if needed"""
        try:
            # Create default admin user if it doesn't exist
            self.auth.create_admin_user('admin', self.config.security.admin_password)
        except Exception as e:
            self.logger.warning(f"Auth initialization warning: {e}")


async def main():
    """Main entry point for the scraper"""
    logger = logging.getLogger(__name__)
    
    try:
        # Initialize configuration
        config = Config()
        logger.info("Configuration loaded successfully")
        
        # Initialize scraper
        scraper = GitHubArchiveScraper(config)
        await scraper.initialize()
        
        # Run scraping
        await scraper.run_continuous_scraping()
        
    except KeyboardInterrupt:
        logger.info("Scraping interrupted by user")
    except Exception as e:
        logger.error(f"Scraping failed: {e}")
        return 1
    finally:
        try:
            await scraper.shutdown()
        except:
            pass
    
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
