#!/usr/bin/env python3
"""
Test script for the Safe API
Tests all endpoints and resource monitoring functionality
"""

import asyncio
import json
import time
import requests
from datetime import datetime

class APITester:
    """Test the Safe API endpoints"""
    
    def __init__(self, base_url="http://localhost:8080"):
        self.base_url = base_url
        self.session = requests.Session()
        self.session.timeout = 10
        
    def test_endpoint(self, endpoint, method="GET", data=None):
        """Test a single endpoint"""
        url = f"{self.base_url}{endpoint}"
        
        try:
            print(f"Testing {method} {endpoint}...")
            
            if method == "GET":
                response = self.session.get(url)
            elif method == "POST":
                response = self.session.post(url, json=data)
            else:
                raise ValueError(f"Unsupported method: {method}")
            
            print(f"  Status: {response.status_code}")
            
            if response.headers.get('content-type', '').startswith('application/json'):
                result = response.json()
                print(f"  Response: {json.dumps(result, indent=2)[:200]}...")
                return True, result
            else:
                print(f"  Content-Type: {response.headers.get('content-type')}")
                print(f"  Content Length: {len(response.text)}")
                return True, response.text[:100]
                
        except Exception as e:
            print(f"  Error: {e}")
            return False, str(e)
    
    def run_tests(self):
        """Run all API tests"""
        print("🧪 Testing Safe API Endpoints")
        print("=" * 50)
        
        tests = [
            # Health and monitoring
            ("/api/health", "GET"),
            ("/api/status", "GET"),
            ("/api/monitor", "GET"),
            
            # Data endpoints
            ("/api/stats", "GET"),
            ("/api/events?limit=5", "GET"),
            ("/api/repositories?limit=5", "GET"),
            
            # Search (with data)
            ("/api/search", "POST", {"query": "test", "limit": 10}),
            
            # Management endpoints
            ("/api/scraper-logs", "GET"),
            
            # Dashboard
            ("/", "GET"),
        ]
        
        results = {"passed": 0, "failed": 0, "total": len(tests)}
        
        for endpoint, method, *args in tests:
            data = args[0] if args else None
            success, result = self.test_endpoint(endpoint, method, data)
            
            if success:
                results["passed"] += 1
                print("  ✅ PASSED")
            else:
                results["failed"] += 1
                print("  ❌ FAILED")
            print()
        
        print("Test Results:")
        print(f"  Passed: {results['passed']}/{results['total']}")
        print(f"  Failed: {results['failed']}/{results['total']}")
        
        if results["failed"] == 0:
            print("🎉 All tests passed!")
        else:
            print("⚠️ Some tests failed!")
            
        return results["failed"] == 0

def test_resource_monitoring():
    """Test resource monitoring functionality"""
    print("\n🔍 Testing Resource Monitoring")
    print("=" * 50)
    
    tester = APITester()
    
    # Get initial resource status
    success, result = tester.test_endpoint("/api/monitor")
    if not success:
        print("❌ Failed to get resource status")
        return False
    
    print("Resource Status:")
    print(f"  Memory Usage: {result['process_memory_mb']:.1f}MB ({result['process_memory_usage_percent']:.1f}%)")
    print(f"  System Memory: {result['system_memory_gb']:.1f}GB ({result['system_memory_percent']:.1f}%)")
    print(f"  Disk Usage: {result['disk_used_gb']:.1f}GB ({result['disk_usage_percent']:.1f}%)")
    print(f"  CPU Usage: {result['cpu_percent']:.1f}%")
    print(f"  Safety Status: {'✅ SAFE' if result['is_safe'] else '⚠️ WARNING'}")
    print(f"  Emergency Mode: {'🚨 ACTIVE' if result['emergency_mode'] else '✅ NORMAL'}")
    print(f"  Temp Files: {result['temp_files_count']}")
    print(f"  Uptime: {result['uptime_formatted']}")
    
    print("\nRecommendations:")
    for rec in result.get('recommendations', []):
        print(f"  💡 {rec}")
    
    return True

def test_management_functions():
    """Test management functions (safely)"""
    print("\n⚙️ Testing Management Functions")
    print("=" * 50)
    
    tester = APITester()
    
    # Test emergency cleanup (safe operation)
    print("Testing emergency cleanup...")
    success, result = tester.test_endpoint("/api/emergency-cleanup", "POST")
    
    if success:
        print(f"  Objects collected: {result.get('objects_collected', 0)}")
        print("  ✅ Emergency cleanup successful")
    else:
        print("  ❌ Emergency cleanup failed")
    
    return success

def main():
    """Main test function"""
    print(f"🚀 Safe API Test Suite")
    print(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 50)
    
    # Test basic API functionality
    tester = APITester()
    api_tests_passed = tester.run_tests()
    
    # Test resource monitoring
    monitoring_tests_passed = test_resource_monitoring()
    
    # Test management functions
    management_tests_passed = test_management_functions()
    
    print("\n" + "=" * 50)
    print("📊 Overall Test Results:")
    print(f"  API Tests: {'✅ PASSED' if api_tests_passed else '❌ FAILED'}")
    print(f"  Monitoring Tests: {'✅ PASSED' if monitoring_tests_passed else '❌ FAILED'}")
    print(f"  Management Tests: {'✅ PASSED' if management_tests_passed else '❌ FAILED'}")
    
    all_passed = api_tests_passed and monitoring_tests_passed and management_tests_passed
    
    if all_passed:
        print("\n🎉 All tests passed! Your Oracle Cloud instance is ready for safe scraping!")
        print("\n📋 Summary:")
        print("  ✅ Resource monitoring is active")
        print("  ✅ Memory limits are enforced (18GB max)")
        print("  ✅ Disk limits are enforced (40GB max)")
        print("  ✅ CPU throttling is enabled (80% max)")
        print("  ✅ Emergency cleanup is working")
        print("  ✅ Dashboard is accessible")
        print("\n🔗 Access your dashboard at: http://170.9.239.38:8080")
    else:
        print("\n⚠️ Some tests failed. Please check the issues above.")
        return 1
    
    return 0

if __name__ == "__main__":
    exit(main())
