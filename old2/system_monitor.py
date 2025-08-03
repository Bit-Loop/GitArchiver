#!/usr/bin/env python3
"""
System Resource Monitor for Oracle Cloud
Real-time monitoring and alerting for GitArchiver
"""

import time
import sys
import json
import psutil
import logging
from datetime import datetime
from pathlib import Path

class SystemMonitor:
    """Oracle Cloud system monitor with alerting"""
    
    def __init__(self):
        self.start_time = time.time()
        
        # Oracle Cloud limits (24GB RAM system)
        self.memory_limit_gb = 22  # Leave 2GB for system
        self.disk_limit_gb = 45    # Leave 5GB free
        self.cpu_limit_percent = 85
        
        # Alert thresholds
        self.memory_warning = 18   # GB
        self.disk_warning = 40     # GB
        self.cpu_warning = 70      # %
        
        # Setup logging
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s',
            handlers=[
                logging.StreamHandler(),
                logging.FileHandler('system_monitor.log')
            ]
        )
        self.logger = logging.getLogger(__name__)
    
    def get_memory_info(self):
        """Get detailed memory information"""
        mem = psutil.virtual_memory()
        swap = psutil.swap_memory()
        
        return {
            'total_gb': round(mem.total / (1024**3), 2),
            'available_gb': round(mem.available / (1024**3), 2),
            'used_gb': round(mem.used / (1024**3), 2),
            'percent': mem.percent,
            'swap_used_gb': round(swap.used / (1024**3), 2),
            'swap_total_gb': round(swap.total / (1024**3), 2)
        }
    
    def get_disk_info(self):
        """Get disk usage information"""
        usage = psutil.disk_usage('/')
        
        return {
            'total_gb': round(usage.total / (1024**3), 2),
            'used_gb': round(usage.used / (1024**3), 2),
            'free_gb': round(usage.free / (1024**3), 2),
            'percent': round((usage.used / usage.total) * 100, 2)
        }
    
    def get_cpu_info(self):
        """Get CPU usage information"""
        cpu_percent = psutil.cpu_percent(interval=1)
        cpu_count = psutil.cpu_count()
        load_avg = psutil.getloadavg()
        
        return {
            'percent': cpu_percent,
            'count': cpu_count,
            'load_1min': load_avg[0],
            'load_5min': load_avg[1],
            'load_15min': load_avg[2]
        }
    
    def get_process_info(self):
        """Get information about running processes"""
        processes = []
        
        for proc in psutil.process_iter(['pid', 'name', 'memory_percent', 'cpu_percent']):
            try:
                if proc.info['memory_percent'] > 1.0:  # Only processes using >1% memory
                    processes.append({
                        'pid': proc.info['pid'],
                        'name': proc.info['name'],
                        'memory_percent': round(proc.info['memory_percent'], 2),
                        'cpu_percent': round(proc.info['cpu_percent'], 2)
                    })
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                continue
        
        # Sort by memory usage
        processes.sort(key=lambda x: x['memory_percent'], reverse=True)
        return processes[:10]  # Top 10
    
    def check_alerts(self, memory, disk, cpu):
        """Check for alert conditions"""
        alerts = []
        
        # Memory alerts
        if memory['used_gb'] > self.memory_limit_gb:
            alerts.append({
                'type': 'CRITICAL',
                'category': 'MEMORY',
                'message': f"Memory usage critical: {memory['used_gb']:.1f}GB / {memory['total_gb']:.1f}GB"
            })
        elif memory['used_gb'] > self.memory_warning:
            alerts.append({
                'type': 'WARNING',
                'category': 'MEMORY',
                'message': f"Memory usage high: {memory['used_gb']:.1f}GB / {memory['total_gb']:.1f}GB"
            })
        
        # Disk alerts
        if disk['used_gb'] > self.disk_limit_gb:
            alerts.append({
                'type': 'CRITICAL',
                'category': 'DISK',
                'message': f"Disk usage critical: {disk['used_gb']:.1f}GB / {disk['total_gb']:.1f}GB"
            })
        elif disk['used_gb'] > self.disk_warning:
            alerts.append({
                'type': 'WARNING',
                'category': 'DISK',
                'message': f"Disk usage high: {disk['used_gb']:.1f}GB / {disk['total_gb']:.1f}GB"
            })
        
        # CPU alerts
        if cpu['percent'] > self.cpu_limit_percent:
            alerts.append({
                'type': 'CRITICAL',
                'category': 'CPU',
                'message': f"CPU usage critical: {cpu['percent']:.1f}%"
            })
        elif cpu['percent'] > self.cpu_warning:
            alerts.append({
                'type': 'WARNING',
                'category': 'CPU',
                'message': f"CPU usage high: {cpu['percent']:.1f}%"
            })
        
        return alerts
    
    def generate_report(self):
        """Generate a comprehensive system report"""
        memory = self.get_memory_info()
        disk = self.get_disk_info()
        cpu = self.get_cpu_info()
        processes = self.get_process_info()
        alerts = self.check_alerts(memory, disk, cpu)
        
        uptime = time.time() - self.start_time
        
        report = {
            'timestamp': datetime.now().isoformat(),
            'uptime_seconds': round(uptime, 1),
            'memory': memory,
            'disk': disk,
            'cpu': cpu,
            'top_processes': processes,
            'alerts': alerts,
            'system_healthy': len([a for a in alerts if a['type'] == 'CRITICAL']) == 0
        }
        
        return report
    
    def log_alerts(self, alerts):
        """Log any alerts to the logger"""
        for alert in alerts:
            if alert['type'] == 'CRITICAL':
                self.logger.error(f"{alert['category']}: {alert['message']}")
            else:
                self.logger.warning(f"{alert['category']}: {alert['message']}")
    
    def save_report(self, report, filename='system_report.json'):
        """Save report to JSON file"""
        try:
            with open(filename, 'w') as f:
                json.dump(report, f, indent=2)
        except Exception as e:
            self.logger.error(f"Failed to save report: {e}")
    
    def monitor_loop(self, interval=30, save_reports=True):
        """Main monitoring loop"""
        self.logger.info("Starting system monitor...")
        self.logger.info(f"Memory limit: {self.memory_limit_gb}GB")
        self.logger.info(f"Disk limit: {self.disk_limit_gb}GB")
        self.logger.info(f"CPU limit: {self.cpu_limit_percent}%")
        
        try:
            while True:
                report = self.generate_report()
                
                # Log alerts
                if report['alerts']:
                    self.log_alerts(report['alerts'])
                
                # Print summary
                memory = report['memory']
                disk = report['disk']
                cpu = report['cpu']
                
                print(f"\n{datetime.now().strftime('%Y-%m-%d %H:%M:%S')} - System Status")
                print(f"Memory: {memory['used_gb']:.1f}GB/{memory['total_gb']:.1f}GB ({memory['percent']:.1f}%)")
                print(f"Disk:   {disk['used_gb']:.1f}GB/{disk['total_gb']:.1f}GB ({disk['percent']:.1f}%)")
                print(f"CPU:    {cpu['percent']:.1f}% (Load: {cpu['load_1min']:.2f})")
                
                if report['alerts']:
                    print("🚨 ALERTS:")
                    for alert in report['alerts']:
                        icon = "🔴" if alert['type'] == 'CRITICAL' else "🟡"
                        print(f"  {icon} {alert['message']}")
                else:
                    print("✅ All systems normal")
                
                # Save report if requested
                if save_reports:
                    timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
                    self.save_report(report, f'reports/system_report_{timestamp}.json')
                
                time.sleep(interval)
                
        except KeyboardInterrupt:
            self.logger.info("Monitor stopped by user")
        except Exception as e:
            self.logger.error(f"Monitor error: {e}")

def main():
    """Main entry point"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Oracle Cloud System Monitor')
    parser.add_argument('--interval', type=int, default=30, help='Monitor interval in seconds (default: 30)')
    parser.add_argument('--no-save', action='store_true', help='Disable saving reports to files')
    parser.add_argument('--once', action='store_true', help='Run once and exit')
    
    args = parser.parse_args()
    
    # Create reports directory
    Path('reports').mkdir(exist_ok=True)
    
    monitor = SystemMonitor()
    
    if args.once:
        # Single report mode
        report = monitor.generate_report()
        print(json.dumps(report, indent=2))
        
        if report['alerts']:
            monitor.log_alerts(report['alerts'])
            sys.exit(1)  # Exit with error if there are critical alerts
    else:
        # Continuous monitoring mode
        monitor.monitor_loop(
            interval=args.interval,
            save_reports=not args.no_save
        )

if __name__ == '__main__':
    main()
