9#!/bin/bash

# Quick status check for the GitArchiver API server

echo "=== GitArchiver Server Status ==="
echo

# Check if the server process is running
if pgrep -f "professional_api.py" > /dev/null; then
    echo "✅ Server process is running"
    
    # Test API response
    if curl -s http://localhost:8080/api/health > /dev/null; then
        echo "✅ Server is responding to requests"
        echo
        echo "Health check:"
        curl -s http://localhost:8080/api/health | jq
    else
        echo "❌ Server is not responding to requests"
    fi
else
    echo "❌ Server process is not running"
fi

echo
echo "=== Quick Commands ==="
echo "Start/Restart: ./restart_server.sh"
echo "Stop:          ./restart_server.sh stop"  
echo "Logs:          ./restart_server.sh logs"
echo "Dashboard:     http://localhost:8080"
echo
echo "=== Login Credentials ==="
echo "Username: admin"
echo "Password: admin123"
