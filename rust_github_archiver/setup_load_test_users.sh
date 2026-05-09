#!/bin/bash
# Setup script for authenticated load testing
# Creates test users in the database for load testing

set -e

echo "=== Authenticated Load Test Setup ==="
echo ""

# Database connection details from environment or defaults
DB_HOST="${DATABASE_HOST:-localhost}"
DB_PORT="${DATABASE_PORT:-5432}"
DB_NAME="${DATABASE_NAME:-github_archiver}"
DB_USER="${DATABASE_USER:-postgres}"

echo "Database: $DB_HOST:$DB_PORT/$DB_NAME"
echo ""

# Test user credentials
USERS=(
  "loadtest_user1:LoadTest123!"
  "loadtest_user2:LoadTest123!"
  "loadtest_user3:LoadTest123!"
  "loadtest_user4:LoadTest123!"
  "loadtest_user5:LoadTest123!"
)

echo "Creating test users..."
for user_creds in "${USERS[@]}"; do
  username="${user_creds%%:*}"
  password="${user_creds##*:}"
  
  echo -n "  - $username... "
  
  # Check if user exists
  USER_EXISTS=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c \
    "SELECT COUNT(*) FROM users WHERE username = '$username';" 2>/dev/null | tr -d ' ')
  
  if [ "$USER_EXISTS" -eq "0" ]; then
    # Create user using the API or direct SQL
    # Note: Password hash generation depends on your auth system
    # This is a placeholder - adjust based on your actual password hashing
    
    # Option 1: Use the API to create user (recommended)
    RESPONSE=$(curl -s -X POST http://localhost:3000/api/auth/register \
      -H "Content-Type: application/json" \
      -d "{\"username\":\"$username\",\"password\":\"$password\",\"email\":\"${username}@loadtest.local\"}" 2>/dev/null)
    
    if echo "$RESPONSE" | grep -q "success\|created\|token"; then
      echo "✅ Created"
    else
      # Option 2: Direct SQL insert (if API not available)
      # NOTE: This requires implementing password hashing
      echo "⚠️  API creation failed, trying SQL..."
      
      # Generate bcrypt hash (requires bcrypt command-line tool)
      # For now, just insert with a note that password needs to be set
      psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c \
        "INSERT INTO users (username, password_hash, email, role, created_at, updated_at) 
         VALUES ('$username', 'placeholder_hash', '${username}@loadtest.local', 'user', NOW(), NOW())
         ON CONFLICT (username) DO NOTHING;" >/dev/null 2>&1
      
      echo "⚠️  Created with placeholder password - manual password hash setup needed"
    fi
  else
    echo "✓ Already exists"
  fi
done

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Test users created:"
for user_creds in "${USERS[@]}"; do
  username="${user_creds%%:*}"
  password="${user_creds##*:}"
  echo "  - Username: $username"
  echo "    Password: $password"
done

echo ""
echo "Next steps:"
echo "1. Verify users can login:"
echo "   curl -X POST http://localhost:3000/api/auth/login \\"
echo "     -H 'Content-Type: application/json' \\"
echo "     -d '{\"username\":\"loadtest_user1\",\"password\":\"LoadTest123!\"}'"
echo ""
echo "2. Run the authenticated load test:"
echo "   BASE_URL=http://localhost:3000 k6 run tests/load/authenticated-load-test.js"
echo ""
echo "3. Verify audit logs after test:"
echo "   ./verify_audit_logs.sh"
echo ""
