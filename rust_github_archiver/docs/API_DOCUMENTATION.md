# GitHub Archiver API Documentation

## Overview

The GitHub Archiver API provides programmatic access to GitHub event data, repository information, and secret scanning results.

**Base URL**: `https://github-archiver.example.com`

**API Version**: v1

**Authentication**: JWT Bearer tokens

## Table of Contents
- [Authentication](#authentication)
- [Rate Limiting](#rate-limiting)
- [Error Handling](#error-handling)
- [Endpoints](#endpoints)
  - [Health](#health-endpoints)
  - [Events](#events-endpoints)
  - [Repositories](#repositories-endpoints)
  - [Secrets](#secrets-endpoints)
  - [Statistics](#statistics-endpoints)
  - [Admin](#admin-endpoints)
- [Webhooks](#webhooks)
- [Examples](#examples)

## Authentication

### Obtaining a Token

**Endpoint**: `POST /api/v1/auth/login`

**Request Body**:
```json
{
  "username": "admin",
  "password": "your-password"
}
```

**Response**:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_at": "2024-12-31T23:59:59Z",
  "user": {
    "id": "123",
    "username": "admin",
    "role": "admin"
  }
}
```

### Using the Token

Include the token in the `Authorization` header:

```bash
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

## Rate Limiting

Rate limits apply per IP address and per user:

- **Authenticated requests**: 1000 requests per minute
- **Unauthenticated requests**: 100 requests per minute
- **Auth endpoints**: 5 requests per minute

Rate limit headers are included in every response:

```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 995
X-RateLimit-Reset: 1640995200
```

When rate limited, you'll receive a `429 Too Many Requests` response:

```json
{
  "error": "Rate limit exceeded",
  "retry_after": 60
}
```

## Error Handling

### Error Response Format

```json
{
  "error": "Error message",
  "code": "ERROR_CODE",
  "details": {
    "field": "Additional context"
  }
}
```

### HTTP Status Codes

- `200 OK`: Request successful
- `201 Created`: Resource created
- `400 Bad Request`: Invalid request parameters
- `401 Unauthorized`: Authentication required or failed
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource not found
- `429 Too Many Requests`: Rate limit exceeded
- `500 Internal Server Error`: Server error
- `503 Service Unavailable`: Service temporarily unavailable

## Endpoints

### Health Endpoints

#### Liveness Probe
**GET** `/health/live`

Checks if the application is running.

**Response** (200):
```json
{
  "status": "healthy"
}
```

#### Readiness Probe
**GET** `/health/ready`

Checks if the application is ready to serve requests.

**Response** (200):
```json
{
  "status": "healthy",
  "database": "connected"
}
```

**Response** (503):
```json
{
  "status": "unhealthy",
  "database": "disconnected"
}
```

#### Detailed Health
**GET** `/health`

Provides detailed health information.

**Response** (200):
```json
{
  "status": "healthy",
  "uptime": 3600,
  "checks": {
    "database": {
      "status": "healthy",
      "response_time_ms": 5
    },
    "memory": {
      "status": "healthy",
      "used_percent": 65
    },
    "disk": {
      "status": "healthy",
      "used_percent": 45
    }
  }
}
```

### Events Endpoints

#### List Events
**GET** `/api/v1/events`

Retrieve a paginated list of GitHub events.

**Query Parameters**:
- `limit` (integer, optional): Number of results (1-1000, default: 100)
- `offset` (integer, optional): Offset for pagination (default: 0)
- `event_type` (string, optional): Filter by event type
- `repository` (string, optional): Filter by repository name
- `from_date` (string, optional): Start date (ISO 8601)
- `to_date` (string, optional): End date (ISO 8601)
- `sort` (string, optional): Sort field (created_at, -created_at)

**Response** (200):
```json
{
  "data": [
    {
      "id": "123456",
      "type": "PushEvent",
      "repository": {
        "id": 789,
        "name": "owner/repo",
        "url": "https://github.com/owner/repo"
      },
      "actor": {
        "id": 456,
        "login": "username",
        "avatar_url": "https://avatars.githubusercontent.com/u/456"
      },
      "payload": { /* event-specific data */ },
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "pagination": {
    "total": 50000,
    "limit": 100,
    "offset": 0,
    "has_more": true
  }
}
```

#### Get Event
**GET** `/api/v1/events/{event_id}`

Retrieve a specific event by ID.

**Response** (200):
```json
{
  "id": "123456",
  "type": "PushEvent",
  "repository": {
    "id": 789,
    "name": "owner/repo",
    "url": "https://github.com/owner/repo"
  },
  "actor": {
    "id": 456,
    "login": "username",
    "avatar_url": "https://avatars.githubusercontent.com/u/456"
  },
  "payload": {
    "commits": [
      {
        "sha": "abc123...",
        "message": "Fix bug",
        "author": {
          "name": "John Doe",
          "email": "john@example.com"
        }
      }
    ]
  },
  "created_at": "2024-01-15T10:30:00Z"
}
```

### Repositories Endpoints

#### List Repositories
**GET** `/api/v1/repositories`

Retrieve a list of tracked repositories.

**Query Parameters**:
- `limit` (integer, optional): Number of results (default: 100)
- `offset` (integer, optional): Offset for pagination (default: 0)
- `search` (string, optional): Search query
- `language` (string, optional): Filter by programming language
- `min_stars` (integer, optional): Minimum star count

**Response** (200):
```json
{
  "data": [
    {
      "id": 789,
      "name": "owner/repo",
      "full_name": "owner/repo",
      "description": "A cool project",
      "language": "Rust",
      "stars": 1234,
      "forks": 56,
      "watchers": 78,
      "url": "https://github.com/owner/repo",
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-15T10:30:00Z"
    }
  ],
  "pagination": {
    "total": 5000,
    "limit": 100,
    "offset": 0
  }
}
```

#### Get Repository
**GET** `/api/v1/repositories/{repo_id}`

Retrieve detailed information about a repository.

**Response** (200):
```json
{
  "id": 789,
  "name": "owner/repo",
  "full_name": "owner/repo",
  "description": "A cool project",
  "language": "Rust",
  "stars": 1234,
  "forks": 56,
  "watchers": 78,
  "url": "https://github.com/owner/repo",
  "homepage": "https://example.com",
  "topics": ["rust", "github", "archiver"],
  "license": "MIT",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-15T10:30:00Z",
  "statistics": {
    "total_events": 5678,
    "total_commits": 1234,
    "total_secrets_found": 2
  }
}
```

### Secrets Endpoints

#### List Detected Secrets
**GET** `/api/v1/secrets`

Retrieve a list of detected secrets.

**Query Parameters**:
- `limit` (integer, optional): Number of results (default: 100)
- `offset` (integer, optional): Offset for pagination (default: 0)
- `type` (string, optional): Filter by secret type
- `severity` (string, optional): Filter by severity (low, medium, high, critical)
- `status` (string, optional): Filter by status (active, resolved, false_positive)
- `repository` (string, optional): Filter by repository

**Response** (200):
```json
{
  "data": [
    {
      "id": "secret-123",
      "type": "aws_access_key",
      "severity": "critical",
      "status": "active",
      "repository": {
        "id": 789,
        "name": "owner/repo"
      },
      "file_path": "config/secrets.json",
      "line_number": 42,
      "matched_content": "AKIA****************",
      "commit_sha": "abc123...",
      "commit_url": "https://github.com/owner/repo/commit/abc123",
      "detected_at": "2024-01-15T10:30:00Z"
    }
  ],
  "pagination": {
    "total": 245,
    "limit": 100,
    "offset": 0
  }
}
```

#### Get Secret
**GET** `/api/v1/secrets/{secret_id}`

Retrieve detailed information about a detected secret.

**Response** (200):
```json
{
  "id": "secret-123",
  "type": "aws_access_key",
  "severity": "critical",
  "status": "active",
  "repository": {
    "id": 789,
    "name": "owner/repo",
    "url": "https://github.com/owner/repo"
  },
  "file_path": "config/secrets.json",
  "line_number": 42,
  "matched_content": "AKIA****************",
  "full_context": "...",
  "commit_sha": "abc123...",
  "commit_url": "https://github.com/owner/repo/commit/abc123",
  "commit_author": "john@example.com",
  "detected_at": "2024-01-15T10:30:00Z",
  "validation": {
    "validated": true,
    "is_active": true,
    "checked_at": "2024-01-15T11:00:00Z"
  }
}
```

#### Update Secret Status
**PATCH** `/api/v1/secrets/{secret_id}`

Update the status of a detected secret.

**Request Body**:
```json
{
  "status": "resolved",
  "notes": "Revoked and rotated"
}
```

**Response** (200):
```json
{
  "id": "secret-123",
  "status": "resolved",
  "updated_at": "2024-01-15T12:00:00Z"
}
```

### Statistics Endpoints

#### Get Statistics
**GET** `/api/v1/statistics`

Retrieve overall statistics.

**Response** (200):
```json
{
  "total_events": 5000000,
  "total_repositories": 10000,
  "total_secrets_detected": 245,
  "secrets_by_severity": {
    "critical": 50,
    "high": 100,
    "medium": 75,
    "low": 20
  },
  "events_by_type": {
    "PushEvent": 2000000,
    "PullRequestEvent": 500000,
    "IssuesEvent": 300000
  },
  "top_languages": [
    { "language": "JavaScript", "count": 3000 },
    { "language": "Python", "count": 2500 },
    { "language": "Rust", "count": 1000 }
  ],
  "last_updated": "2024-01-15T12:00:00Z"
}
```

#### Get Repository Statistics
**GET** `/api/v1/repositories/{repo_id}/statistics`

Retrieve statistics for a specific repository.

**Response** (200):
```json
{
  "repository_id": 789,
  "total_events": 5678,
  "total_commits": 1234,
  "total_secrets_found": 2,
  "events_by_type": {
    "PushEvent": 3000,
    "PullRequestEvent": 1000,
    "IssuesEvent": 500
  },
  "timeline": [
    {
      "date": "2024-01-15",
      "events": 123
    }
  ]
}
```

### Admin Endpoints

**Note**: Requires admin role

#### List Users
**GET** `/api/v1/admin/users`

List all users.

**Response** (200):
```json
{
  "data": [
    {
      "id": "123",
      "username": "admin",
      "role": "admin",
      "created_at": "2024-01-01T00:00:00Z",
      "last_login": "2024-01-15T10:00:00Z"
    }
  ]
}
```

#### Create User
**POST** `/api/v1/admin/users`

Create a new user.

**Request Body**:
```json
{
  "username": "newuser",
  "password": "secure-password",
  "role": "user"
}
```

**Response** (201):
```json
{
  "id": "124",
  "username": "newuser",
  "role": "user",
  "created_at": "2024-01-15T12:00:00Z"
}
```

#### System Health
**GET** `/api/v1/admin/system`

Get detailed system information.

**Response** (200):
```json
{
  "version": "1.0.0",
  "uptime": 3600,
  "database": {
    "connections": 45,
    "max_connections": 100,
    "size_mb": 2048
  },
  "metrics": {
    "requests_per_second": 150,
    "avg_response_time_ms": 85,
    "error_rate": 0.005
  }
}
```

## Webhooks

Subscribe to real-time notifications.

### Create Webhook
**POST** `/api/v1/webhooks`

**Request Body**:
```json
{
  "url": "https://your-server.com/webhook",
  "events": ["secret.detected", "repository.added"],
  "secret": "webhook-secret"
}
```

### Webhook Payload Example
```json
{
  "event": "secret.detected",
  "timestamp": "2024-01-15T12:00:00Z",
  "data": {
    "secret_id": "secret-123",
    "type": "aws_access_key",
    "severity": "critical",
    "repository": "owner/repo"
  }
}
```

## Examples

### cURL

```bash
# Login
TOKEN=$(curl -X POST https://github-archiver.example.com/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"password"}' | jq -r '.token')

# List events
curl -H "Authorization: Bearer $TOKEN" \
  "https://github-archiver.example.com/api/v1/events?limit=10"

# Search repositories
curl -H "Authorization: Bearer $TOKEN" \
  "https://github-archiver.example.com/api/v1/repositories?search=rust&min_stars=100"

# Get statistics
curl -H "Authorization: Bearer $TOKEN" \
  "https://github-archiver.example.com/api/v1/statistics"
```

### Python

```python
import requests

# Login
response = requests.post(
    "https://github-archiver.example.com/api/v1/auth/login",
    json={"username": "admin", "password": "password"}
)
token = response.json()["token"]

# Set headers
headers = {"Authorization": f"Bearer {token}"}

# List events
events = requests.get(
    "https://github-archiver.example.com/api/v1/events",
    headers=headers,
    params={"limit": 10, "event_type": "PushEvent"}
).json()

# Get repository
repo = requests.get(
    "https://github-archiver.example.com/api/v1/repositories/789",
    headers=headers
).json()
```

### JavaScript

```javascript
// Login
const loginResponse = await fetch(
  'https://github-archiver.example.com/api/v1/auth/login',
  {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username: 'admin', password: 'password' })
  }
);
const { token } = await loginResponse.json();

// Set headers
const headers = { 'Authorization': `Bearer ${token}` };

// List events
const eventsResponse = await fetch(
  'https://github-archiver.example.com/api/v1/events?limit=10',
  { headers }
);
const events = await eventsResponse.json();

// Get statistics
const statsResponse = await fetch(
  'https://github-archiver.example.com/api/v1/statistics',
  { headers }
);
const stats = await statsResponse.json();
```

## Support

For API support:
- Email: api-support@example.com
- Documentation: https://docs.github-archiver.example.com
- Status Page: https://status.github-archiver.example.com
