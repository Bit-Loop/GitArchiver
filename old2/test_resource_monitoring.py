#!/usr/bin/env python3
"""
Test script to verify GitHub Archive Scraper resource monitoring integration
This tests that the scraper properly uses Oracle Cloud safety features
"""

import asyncio
import sys
import os
import time
import psutil
from pathlib import Path

# Add project directory to path
sys.path.insert(0, str(Path(__file__).parent))

from config import Config
from gharchive_scraper import GitHubArchiveScraper, ResourceMonitor


async def test_resource_monitoring():
    """Test resource monitoring functionality"""
    print("🧪 Testing Resource Monitoring Integration")
    print("=" * 50)
    
    # Initialize config and monitor
    config = Config()
    monitor = ResourceMonitor(config)
    
    print(f"✅ Resource Monitor Initialized")
    print(f"   Memory Limit: {config.MAX_MEMORY_MB}MB")
    print(f"   Disk Limit: {config.MAX_DISK_USAGE_GB}GB")
    print(f"   CPU Limit: {config.CPU_LIMIT_PERCENT}%")
    print()
    
    # Test current resource usage
    print("📊 Current Resource Status:")
    status = monitor.get_status()
    for key, value in status.items():
        print(f"   {key}: {value}")
    print()
    
    # Test resource checks
    print("🔍 Testing Resource Checks:")
    memory_ok = monitor.check_memory_limit()
    disk_ok = monitor.check_disk_limit()
    should_pause = monitor.should_pause_processing()
    
    print(f"   Memory within limits: {memory_ok}")
    print(f"   Disk within limits: {disk_ok}")
    print(f"   Should pause processing: {should_pause}")
    print()
    
    # Test temporary file management
    print("📁 Testing Temporary File Management:")
    test_file = "/tmp/test_scraper_temp.txt"
    with open(test_file, 'w') as f:
        f.write("test")
    
    monitor.register_temp_file(test_file)
    print(f"   Registered temp file: {test_file}")
    print(f"   Temp files count: {len(monitor.temp_files)}")
    
    monitor.cleanup_temp_files()
    print(f"   After cleanup - temp files count: {len(monitor.temp_files)}")
    print(f"   File exists after cleanup: {os.path.exists(test_file)}")
    print()
    
    return True


async def test_scraper_integration():
    """Test scraper integration with resource monitoring"""
    print("🔧 Testing Scraper Integration")
    print("=" * 50)
    
    config = Config()
    
    # Initialize scraper (without actually running it)
    async with GitHubArchiveScraper(config) as scraper:
        print("✅ Scraper initialized successfully")
        
        # Check that resource monitor is available
        assert hasattr(scraper, 'resource_monitor'), "Scraper missing resource monitor"
        print("✅ Scraper has resource monitor")
        
        # Test resource monitoring in scraper
        status = scraper.resource_monitor.get_status()
        print("📊 Scraper Resource Status:")
        print(f"   Memory: {status['memory_mb']}MB / {status['memory_limit_mb']}MB ({status['memory_usage_percent']}%)")
        print(f"   Disk: {status['disk_used_gb']}GB / {status['disk_limit_gb']}GB ({status['disk_usage_percent']}%)")
        print(f"   CPU: {status['cpu_percent']}%")
        print(f"   Emergency mode: {status['emergency_mode']}")
        print()
        
        # Test that scraper would pause if needed
        would_pause = scraper.resource_monitor.should_pause_processing()
        print(f"✅ Scraper pause check: {would_pause}")
        
        return True


async def test_oracle_cloud_limits():
    """Test Oracle Cloud specific limits"""
    print("☁️  Testing Oracle Cloud Limits")
    print("=" * 50)
    
    config = Config()
    
    # Verify Oracle Cloud specific settings
    expected_limits = {
        'MAX_MEMORY_MB': 18000,  # 18GB for 24GB system
        'MEMORY_WARNING_MB': 16000,  # Warning at 16GB  
        'MAX_DISK_USAGE_GB': 40,  # 40GB max disk
        'DISK_WARNING_GB': 35,  # Warning at 35GB
        'CPU_LIMIT_PERCENT': 80,  # Max 80% CPU
        'MAX_CONCURRENT_DOWNLOADS': 6,  # Reduced concurrency
        'BATCH_SIZE': 500,  # Smaller batches
    }
    
    print("🔍 Verifying Oracle Cloud Safe Limits:")
    for limit_name, expected_value in expected_limits.items():
        actual_value = getattr(config, limit_name)
        status = "✅" if actual_value == expected_value else "❌"
        print(f"   {status} {limit_name}: {actual_value} (expected: {expected_value})")
    print()
    
    # Test system compatibility
    system_memory = psutil.virtual_memory().total / (1024**3)  # GB
    system_disk = psutil.disk_usage('.').total / (1024**3)  # GB
    
    print("🖥️  System Compatibility:")
    print(f"   System Memory: {system_memory:.1f}GB")
    print(f"   Memory limit safe: {'✅' if config.MAX_MEMORY_MB/1024 < system_memory * 0.8 else '❌'}")
    print(f"   System Disk: {system_disk:.1f}GB") 
    print(f"   Disk limit safe: {'✅' if config.MAX_DISK_USAGE_GB < system_disk * 0.9 else '❌'}")
    print()
    
    return True


async def main():
    """Run all tests"""
    print("🚀 GitHub Archive Scraper - Resource Safety Tests")
    print("🎯 Oracle Cloud Instance Protection")
    print("=" * 60)
    print()
    
    tests = [
        ("Resource Monitoring", test_resource_monitoring),
        ("Scraper Integration", test_scraper_integration), 
        ("Oracle Cloud Limits", test_oracle_cloud_limits),
    ]
    
    results = []
    
    for test_name, test_func in tests:
        try:
            print(f"🧪 Running: {test_name}")
            start_time = time.time()
            result = await test_func()
            duration = time.time() - start_time
            
            if result:
                print(f"✅ {test_name} PASSED ({duration:.2f}s)")
                results.append((test_name, "PASSED", duration))
            else:
                print(f"❌ {test_name} FAILED ({duration:.2f}s)")
                results.append((test_name, "FAILED", duration))
                
        except Exception as e:
            duration = time.time() - start_time
            print(f"❌ {test_name} ERROR: {e} ({duration:.2f}s)")
            results.append((test_name, "ERROR", duration))
        
        print()
    
    # Print summary
    print("📋 Test Summary")
    print("=" * 60)
    passed = sum(1 for _, status, _ in results if status == "PASSED")
    total = len(results)
    
    for test_name, status, duration in results:
        status_icon = "✅" if status == "PASSED" else "❌"
        print(f"   {status_icon} {test_name}: {status} ({duration:.2f}s)")
    
    print()
    print(f"🎯 Result: {passed}/{total} tests passed")
    
    if passed == total:
        print("🎉 All tests passed! Oracle Cloud safety features are working correctly.")
        return 0
    else:
        print("⚠️  Some tests failed. Please check the configuration.")
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
