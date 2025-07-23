#!/usr/bin/env python3
"""
Performance monitoring and metrics collection for GitHub Archive Scraper
"""

import asyncio
import json
import time
from datetime import datetime, timedelta
from dataclasses import dataclass, asdict
from typing import Dict, List, Optional, Any, Union
import psutil
import logging


@dataclass
class SystemMetrics:
    """System performance metrics"""
    timestamp: datetime
    cpu_percent: float
    memory_percent: float
    memory_mb: float
    disk_io_read_mb: float
    disk_io_write_mb: float
    network_bytes_sent: int
    network_bytes_recv: int


@dataclass
class ScrapingMetrics:
    """Scraping-specific metrics"""
    timestamp: datetime
    files_processed: int
    events_processed: int
    bytes_downloaded: int
    processing_rate_events_per_sec: float
    download_rate_mbps: float
    error_rate_percent: float
    database_connections_active: int


class MetricsCollector:
    """Collects and manages performance metrics"""
    
    def __init__(self, collection_interval: int = 60):
        self.collection_interval = collection_interval
        self.system_metrics: List[SystemMetrics] = []
        self.scraping_metrics: List[ScrapingMetrics] = []
        self.logger = logging.getLogger(__name__)
        self._last_disk_io = None
        self._last_network_io = None
        self._running = False
        
    async def start_collection(self):
        """Start collecting metrics"""
        self._running = True
        self.logger.info("Starting metrics collection")
        
        while self._running:
            try:
                await self._collect_system_metrics()
                await asyncio.sleep(self.collection_interval)
            except Exception as e:
                self.logger.error(f"Error collecting metrics: {e}")
                await asyncio.sleep(self.collection_interval)
    
    def stop_collection(self):
        """Stop collecting metrics"""
        self._running = False
        self.logger.info("Stopping metrics collection")
    
    async def _collect_system_metrics(self):
        """Collect current system metrics"""
        try:
            # CPU and Memory
            cpu_percent = psutil.cpu_percent(interval=1)
            memory = psutil.virtual_memory()
            
            # Disk I/O
            disk_io = psutil.disk_io_counters()
            disk_read_mb = 0.0
            disk_write_mb = 0.0
            
            if self._last_disk_io and disk_io:
                disk_read_mb = (disk_io.read_bytes - self._last_disk_io.read_bytes) / 1024 / 1024
                disk_write_mb = (disk_io.write_bytes - self._last_disk_io.write_bytes) / 1024 / 1024
            
            self._last_disk_io = disk_io
            
            # Network I/O
            network_io = psutil.net_io_counters()
            
            metrics = SystemMetrics(
                timestamp=datetime.now(),
                cpu_percent=cpu_percent,
                memory_percent=memory.percent,
                memory_mb=memory.used / 1024 / 1024,
                disk_io_read_mb=disk_read_mb,
                disk_io_write_mb=disk_write_mb,
                network_bytes_sent=network_io.bytes_sent,
                network_bytes_recv=network_io.bytes_recv
            )
            
            self.system_metrics.append(metrics)
            
            # Keep only last 24 hours of metrics
            cutoff_time = datetime.now() - timedelta(hours=24)
            self.system_metrics = [m for m in self.system_metrics if m.timestamp > cutoff_time]
            
        except Exception as e:
            self.logger.error(f"Error collecting system metrics: {e}")
    
    def add_scraping_metrics(self, metrics: ScrapingMetrics):
        """Add scraping metrics"""
        self.scraping_metrics.append(metrics)
        
        # Keep only last 24 hours of metrics
        cutoff_time = datetime.now() - timedelta(hours=24)
        self.scraping_metrics = [m for m in self.scraping_metrics if m.timestamp > cutoff_time]
    
    def get_latest_metrics(self) -> Dict:
        """Get latest metrics summary"""
        latest_system = self.system_metrics[-1] if self.system_metrics else None
        latest_scraping = self.scraping_metrics[-1] if self.scraping_metrics else None
        
        return {
            'system': asdict(latest_system) if latest_system else None,
            'scraping': asdict(latest_scraping) if latest_scraping else None,
            'collection_time': datetime.now().isoformat()
        }
    
    def get_metrics_summary(self, hours: int = 1) -> Dict[str, Any]:
        """Get metrics summary for the last N hours"""
        cutoff_time = datetime.now() - timedelta(hours=hours)
        
        # Filter metrics
        recent_system = [m for m in self.system_metrics if m.timestamp > cutoff_time]
        recent_scraping = [m for m in self.scraping_metrics if m.timestamp > cutoff_time]
        
        summary: Dict[str, Any] = {
            'time_period_hours': hours,
            'system_metrics_count': len(recent_system),
            'scraping_metrics_count': len(recent_scraping)
        }
        
        if recent_system:
            summary['system'] = {
                'avg_cpu_percent': sum(m.cpu_percent for m in recent_system) / len(recent_system),
                'max_cpu_percent': max(m.cpu_percent for m in recent_system),
                'avg_memory_mb': sum(m.memory_mb for m in recent_system) / len(recent_system),
                'max_memory_mb': max(m.memory_mb for m in recent_system),
                'total_disk_read_mb': sum(m.disk_io_read_mb for m in recent_system),
                'total_disk_write_mb': sum(m.disk_io_write_mb for m in recent_system)
            }
        
        if recent_scraping:
            summary['scraping'] = {
                'total_files_processed': sum(m.files_processed for m in recent_scraping),
                'total_events_processed': sum(m.events_processed for m in recent_scraping),
                'total_bytes_downloaded': sum(m.bytes_downloaded for m in recent_scraping),
                'avg_processing_rate': sum(m.processing_rate_events_per_sec for m in recent_scraping) / len(recent_scraping),
                'avg_download_rate_mbps': sum(m.download_rate_mbps for m in recent_scraping) / len(recent_scraping),
                'avg_error_rate': sum(m.error_rate_percent for m in recent_scraping) / len(recent_scraping)
            }
        
        return summary
    
    def export_metrics(self, filename: str):
        """Export metrics to JSON file"""
        try:
            metrics_data = {
                'export_time': datetime.now().isoformat(),
                'system_metrics': [asdict(m) for m in self.system_metrics],
                'scraping_metrics': [asdict(m) for m in self.scraping_metrics]
            }
            
            with open(filename, 'w') as f:
                json.dump(metrics_data, f, indent=2, default=str)
            
            self.logger.info(f"Metrics exported to {filename}")
            
        except Exception as e:
            self.logger.error(f"Error exporting metrics: {e}")
    
    def print_current_status(self):
        """Print current system status"""
        latest = self.get_latest_metrics()
        
        print("\n" + "="*60)
        print("GitHub Archive Scraper - Current Status")
        print("="*60)
        
        if latest['system']:
            sys_metrics = latest['system']
            print(f"🖥️  System Resources:")
            print(f"   CPU Usage: {sys_metrics['cpu_percent']:.1f}%")
            print(f"   Memory Usage: {sys_metrics['memory_mb']:.0f}MB ({sys_metrics['memory_percent']:.1f}%)")
            print(f"   Disk I/O: {sys_metrics['disk_io_read_mb']:.1f}MB read, {sys_metrics['disk_io_write_mb']:.1f}MB write")
        
        if latest['scraping']:
            scrape_metrics = latest['scraping']
            print(f"\n📊 Scraping Performance:")
            print(f"   Files Processed: {scrape_metrics['files_processed']:,}")
            print(f"   Events Processed: {scrape_metrics['events_processed']:,}")
            print(f"   Download Rate: {scrape_metrics['download_rate_mbps']:.2f} Mbps")
            print(f"   Processing Rate: {scrape_metrics['processing_rate_events_per_sec']:.0f} events/sec")
            print(f"   Error Rate: {scrape_metrics['error_rate_percent']:.2f}%")
        
        print(f"\n🕒 Last Updated: {latest['collection_time']}")
        print("="*60)


class PerformanceOptimizer:
    """Automatically optimize performance based on metrics"""
    
    def __init__(self, metrics_collector: MetricsCollector):
        self.metrics = metrics_collector
        self.logger = logging.getLogger(__name__)
        
    def get_optimization_recommendations(self) -> List[str]:
        """Get performance optimization recommendations"""
        recommendations = []
        latest = self.metrics.get_latest_metrics()
        
        if not latest['system']:
            return recommendations
        
        sys_metrics = latest['system']
        
        # CPU recommendations
        if sys_metrics['cpu_percent'] > 90:
            recommendations.append("HIGH CPU: Consider reducing MAX_CONCURRENT_DOWNLOADS")
        elif sys_metrics['cpu_percent'] < 30:
            recommendations.append("LOW CPU: Consider increasing MAX_CONCURRENT_DOWNLOADS")
        
        # Memory recommendations
        if sys_metrics['memory_percent'] > 85:
            recommendations.append("HIGH MEMORY: Consider reducing BATCH_SIZE")
        elif sys_metrics['memory_percent'] < 50:
            recommendations.append("LOW MEMORY: Consider increasing BATCH_SIZE")
        
        # Disk I/O recommendations
        if sys_metrics['disk_io_write_mb'] > 100:
            recommendations.append("HIGH DISK I/O: Consider increasing database connection pool")
        
        return recommendations
    
    def auto_optimize_config(self, config) -> Dict[str, int]:
        """Automatically optimize configuration parameters"""
        recommendations = {}
        latest = self.metrics.get_latest_metrics()
        
        if not latest['system']:
            return recommendations
        
        sys_metrics = latest['system']
        
        # Auto-adjust concurrency based on CPU usage
        if sys_metrics['cpu_percent'] > 90 and config.MAX_CONCURRENT_DOWNLOADS > 5:
            recommendations['MAX_CONCURRENT_DOWNLOADS'] = max(5, config.MAX_CONCURRENT_DOWNLOADS - 2)
        elif sys_metrics['cpu_percent'] < 40 and config.MAX_CONCURRENT_DOWNLOADS < 20:
            recommendations['MAX_CONCURRENT_DOWNLOADS'] = min(20, config.MAX_CONCURRENT_DOWNLOADS + 2)
        
        # Auto-adjust batch size based on memory usage
        if sys_metrics['memory_percent'] > 80 and config.BATCH_SIZE > 500:
            recommendations['BATCH_SIZE'] = max(500, config.BATCH_SIZE - 200)
        elif sys_metrics['memory_percent'] < 50 and config.BATCH_SIZE < 2000:
            recommendations['BATCH_SIZE'] = min(2000, config.BATCH_SIZE + 200)
        
        if recommendations:
            self.logger.info(f"Auto-optimization recommendations: {recommendations}")
        
        return recommendations


if __name__ == '__main__':
    # Example usage
    collector = MetricsCollector(collection_interval=10)
    
    async def test_metrics():
        # Start collection
        collection_task = asyncio.create_task(collector.start_collection())
        
        # Simulate some work
        await asyncio.sleep(30)
        
        # Add some scraping metrics
        scraping_metrics = ScrapingMetrics(
            timestamp=datetime.now(),
            files_processed=10,
            events_processed=10000,
            bytes_downloaded=50000000,
            processing_rate_events_per_sec=333.0,
            download_rate_mbps=2.5,
            error_rate_percent=0.1,
            database_connections_active=5
        )
        collector.add_scraping_metrics(scraping_metrics)
        
        # Print status
        collector.print_current_status()
        
        # Get recommendations
        optimizer = PerformanceOptimizer(collector)
        recommendations = optimizer.get_optimization_recommendations()
        
        if recommendations:
            print("\n🚀 Performance Recommendations:")
            for rec in recommendations:
                print(f"   • {rec}")
        
        # Stop collection
        collector.stop_collection()
        collection_task.cancel()
    
    # Run test
    asyncio.run(test_metrics())
