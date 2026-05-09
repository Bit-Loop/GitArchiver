# Environment Variables Reference

Complete reference for all environment variables used in GitHub Archiver.

## Required Variables

### Database Configuration

#### `DATABASE_URL`
**Required**: Yes  
**Type**: String (PostgreSQL connection URL)  
**Example**: `postgresql://user:password@localhost:5432/github_archiver`  
**Description**: PostgreSQL database connection string. Must include username, password, host, port, and database name.

#### `DB_MAX_CONNECTIONS`
**Required**: No  
**Type**: Integer  
**Default**: `100`  
**Example**: `DB_MAX_CONNECTIONS=150`  
**Description**: Maximum number of database connections in the pool.

#### `DB_MIN_CONNECTIONS`
**Required**: No  
**Type**: Integer  
**Default**: `10`  
**Example**: `DB_MIN_CONNECTIONS=20`  
**Description**: Minimum number of database connections to maintain in the pool.

#### `DB_CONNECTION_TIMEOUT`
**Required**: No  
**Type**: Integer (seconds)  
**Default**: `30`  
**Example**: `DB_CONNECTION_TIMEOUT=60`  
**Description**: Timeout for establishing a database connection.

### Authentication & Security

#### `JWT_SECRET`
**Required**: Yes  
**Type**: String (base64 encoded)  
**Example**: `JWT_SECRET=your-secret-key-min-32-chars`  
**Description**: Secret key for JWT token signing. Must be at least 32 characters. Keep this secret and rotate regularly.

#### `JWT_EXPIRATION`
**Required**: No  
**Type**: Integer (seconds)  
**Default**: `3600` (1 hour)  
**Example**: `JWT_EXPIRATION=86400`  
**Description**: JWT token expiration time in seconds.

#### `ADMIN_USERNAME`
**Required**: No  
**Type**: String  
**Default**: `admin`  
**Example**: `ADMIN_USERNAME=administrator`  
**Description**: Default admin username for initial setup.

#### `ADMIN_PASSWORD`
**Required**: Yes (production)  
**Type**: String  
**Example**: `ADMIN_PASSWORD=SecurePassword123!`  
**Description**: Default admin password. Change immediately after first login.

### GitHub API

#### `GITHUB_TOKEN`
**Required**: Yes  
**Type**: String (Personal Access Token)  
**Example**: `GITHUB_TOKEN=ghp_REDACTED_EXAMPLE`  
**Description**: GitHub Personal Access Token for API access. Required scopes: `public_repo`, `read:org`, `read:user`.

#### `GITHUB_API_URL`
**Required**: No  
**Type**: String (URL)  
**Default**: `https://api.github.com`  
**Example**: `GITHUB_API_URL=https://api.github.com`  
**Description**: GitHub API base URL. Use for GitHub Enterprise Server.

### Server Configuration

#### `SERVER_HOST`
**Required**: No  
**Type**: String (IP address)  
**Default**: `0.0.0.0`  
**Example**: `SERVER_HOST=127.0.0.1`  
**Description**: Server bind address.

#### `SERVER_PORT`
**Required**: No  
**Type**: Integer  
**Default**: `8081`  
**Example**: `SERVER_PORT=3000`  
**Description**: Server listen port.

#### `SERVER_WORKERS`
**Required**: No  
**Type**: Integer  
**Default**: (CPU cores)  
**Example**: `SERVER_WORKERS=4`  
**Description**: Number of worker threads. Defaults to number of CPU cores.

### Rate Limiting

#### `RATE_LIMIT_ENABLED`
**Required**: No  
**Type**: Boolean  
**Default**: `true`  
**Example**: `RATE_LIMIT_ENABLED=true`  
**Description**: Enable or disable rate limiting.

#### `RATE_LIMIT_REQUESTS`
**Required**: No  
**Type**: Integer  
**Default**: `1000`  
**Example**: `RATE_LIMIT_REQUESTS=5000`  
**Description**: Maximum requests per window for authenticated users.

#### `RATE_LIMIT_WINDOW`
**Required**: No  
**Type**: Integer (seconds)  
**Default**: `60`  
**Example**: `RATE_LIMIT_WINDOW=300`  
**Description**: Rate limit window duration in seconds.

#### `RATE_LIMIT_BURST`
**Required**: No  
**Type**: Integer  
**Default**: `100`  
**Example**: `RATE_LIMIT_BURST=200`  
**Description**: Burst size for rate limiter (allows brief spikes).

### Logging

#### `RUST_LOG`
**Required**: No  
**Type**: String (log level)  
**Default**: `info`  
**Valid Values**: `error`, `warn`, `info`, `debug`, `trace`  
**Example**: `RUST_LOG=debug`  
**Description**: Application log level. Can be module-specific: `rust_github_archiver=debug,sqlx=warn`.

#### `LOG_FORMAT`
**Required**: No  
**Type**: String  
**Default**: `json`  
**Valid Values**: `json`, `pretty`, `compact`  
**Example**: `LOG_FORMAT=json`  
**Description**: Log output format. Use `json` for production, `pretty` for development.

#### `LOG_FILE`
**Required**: No  
**Type**: String (file path)  
**Default**: None (stdout only)  
**Example**: `LOG_FILE=/var/log/github-archiver/app.log`  
**Description**: Path to log file. If not set, logs only to stdout.

### Monitoring & Metrics

#### `METRICS_ENABLED`
**Required**: No  
**Type**: Boolean  
**Default**: `true`  
**Example**: `METRICS_ENABLED=true`  
**Description**: Enable Prometheus metrics endpoint.

#### `METRICS_PORT`
**Required**: No  
**Type**: Integer  
**Default**: `9090`  
**Example**: `METRICS_PORT=9091`  
**Description**: Port for Prometheus metrics endpoint.

#### `HEALTH_CHECK_INTERVAL`
**Required**: No  
**Type**: Integer (seconds)  
**Default**: `10`  
**Example**: `HEALTH_CHECK_INTERVAL=30`  
**Description**: Interval for internal health checks.

### Scraping & Processing

#### `SCRAPER_ENABLED`
**Required**: No  
**Type**: Boolean  
**Default**: `true`  
**Example**: `SCRAPER_ENABLED=false`  
**Description**: Enable or disable GitHub event scraping.

#### `SCRAPER_INTERVAL`
**Required**: No  
**Type**: Integer (seconds)  
**Default**: `300` (5 minutes)  
**Example**: `SCRAPER_INTERVAL=600`  
**Description**: Interval between scraping runs.

#### `SCRAPER_BATCH_SIZE`
**Required**: No  
**Type**: Integer  
**Default**: `100`  
**Example**: `SCRAPER_BATCH_SIZE=500`  
**Description**: Number of events to process in each batch.

#### `SCRAPER_MAX_RETRIES`
**Required**: No  
**Type**: Integer  
**Default**: `3`  
**Example**: `SCRAPER_MAX_RETRIES=5`  
**Description**: Maximum number of retry attempts for failed scraping.

### Secret Scanning

#### `SECRET_SCANNER_ENABLED`
**Required**: No  
**Type**: Boolean  
**Default**: `true`  
**Example**: `SECRET_SCANNER_ENABLED=true`  
**Description**: Enable secret scanning functionality.

#### `SECRET_SCANNER_THREADS`
**Required**: No  
**Type**: Integer  
**Default**: `4`  
**Example**: `SECRET_SCANNER_THREADS=8`  
**Description**: Number of threads for parallel secret scanning.

#### `SECRET_VALIDATION_ENABLED`
**Required**: No  
**Type**: Boolean  
**Default**: `true`  
**Example**: `SECRET_VALIDATION_ENABLED=false`  
**Description**: Enable validation of detected secrets (e.g., AWS key validation).

### Redis (Optional)

#### `REDIS_URL`
**Required**: No  
**Type**: String (Redis connection URL)  
**Default**: None  
**Example**: `REDIS_URL=redis://localhost:6379`  
**Description**: Redis connection URL for caching. If not set, in-memory caching is used.

#### `REDIS_POOL_SIZE`
**Required**: No  
**Type**: Integer  
**Default**: `10`  
**Example**: `REDIS_POOL_SIZE=20`  
**Description**: Size of Redis connection pool.

#### `CACHE_TTL`
**Required**: No  
**Type**: Integer (seconds)  
**Default**: `300` (5 minutes)  
**Example**: `CACHE_TTL=600`  
**Description**: Default time-to-live for cached entries.

### Webhooks

#### `WEBHOOK_ENABLED`
**Required**: No  
**Type**: Boolean  
**Default**: `true`  
**Example**: `WEBHOOK_ENABLED=false`  
**Description**: Enable webhook notifications.

#### `WEBHOOK_TIMEOUT`
**Required**: No  
**Type**: Integer (seconds)  
**Default**: `10`  
**Example**: `WEBHOOK_TIMEOUT=30`  
**Description**: Timeout for webhook delivery.

#### `WEBHOOK_MAX_RETRIES`
**Required**: No  
**Type**: Integer  
**Default**: `3`  
**Example**: `WEBHOOK_MAX_RETRIES=5`  
**Description**: Maximum retries for failed webhook deliveries.

### Notifications

#### `SLACK_WEBHOOK_URL`
**Required**: No  
**Type**: String (URL)  
**Default**: None  
**Example**: `SLACK_WEBHOOK_URL=https://hooks.slack.com/services/xxx/yyy/zzz`  
**Description**: Slack webhook URL for notifications.

#### `DISCORD_WEBHOOK_URL`
**Required**: No  
**Type**: String (URL)  
**Default**: None  
**Example**: `DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/xxx/yyy`  
**Description**: Discord webhook URL for notifications.

#### `SMTP_HOST`
**Required**: No  
**Type**: String  
**Default**: None  
**Example**: `SMTP_HOST=smtp.gmail.com`  
**Description**: SMTP server hostname for email notifications.

#### `SMTP_PORT`
**Required**: No  
**Type**: Integer  
**Default**: `587`  
**Example**: `SMTP_PORT=465`  
**Description**: SMTP server port.

#### `SMTP_USERNAME`
**Required**: No  
**Type**: String  
**Default**: None  
**Example**: `SMTP_USERNAME=notifications@example.com`  
**Description**: SMTP authentication username.

#### `SMTP_PASSWORD`
**Required**: No  
**Type**: String  
**Default**: None  
**Example**: `SMTP_PASSWORD=app-specific-password`  
**Description**: SMTP authentication password.

### Feature Flags

#### `FEATURE_AI_TRIAGE`
**Required**: No  
**Type**: Boolean  
**Default**: `false`  
**Example**: `FEATURE_AI_TRIAGE=true`  
**Description**: Enable AI-powered secret triage (requires AI feature flag at compile time).

#### `FEATURE_BIGQUERY`
**Required**: No  
**Type**: Boolean  
**Default**: `false`  
**Example**: `FEATURE_BIGQUERY=true`  
**Description**: Enable BigQuery integration.

#### `FEATURE_REALTIME`
**Required**: No  
**Type**: Boolean  
**Default**: `true`  
**Example**: `FEATURE_REALTIME=false`  
**Description**: Enable real-time event processing.

### Development & Testing

#### `DEV_MODE`
**Required**: No  
**Type**: Boolean  
**Default**: `false`  
**Example**: `DEV_MODE=true`  
**Description**: Enable development mode (more verbose logging, less strict validation).

#### `TEST_MODE`
**Required**: No  
**Type**: Boolean  
**Default**: `false`  
**Example**: `TEST_MODE=true`  
**Description**: Enable test mode (disables external API calls, uses mock data).

#### `MOCK_GITHUB_API`
**Required**: No  
**Type**: Boolean  
**Default**: `false`  
**Example**: `MOCK_GITHUB_API=true`  
**Description**: Use mock GitHub API responses instead of real API calls.

---

## Environment Variable Sets

### Minimal Production Configuration
```bash
# Required only
DATABASE_URL=postgresql://user:password@postgres:5432/github_archiver
JWT_SECRET=your-secret-key-minimum-32-characters-long
GITHUB_TOKEN=ghp_REDACTED_EXAMPLE
ADMIN_PASSWORD=ChangeMe123!
```

### Recommended Production Configuration
```bash
# Database
DATABASE_URL=postgresql://user:password@postgres:5432/github_archiver
DB_MAX_CONNECTIONS=150
DB_MIN_CONNECTIONS=20

# Security
JWT_SECRET=your-secret-key-minimum-32-characters-long
JWT_EXPIRATION=3600
ADMIN_PASSWORD=SecurePassword123!

# GitHub
GITHUB_TOKEN=ghp_REDACTED_EXAMPLE

# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8081
SERVER_WORKERS=4

# Rate Limiting
RATE_LIMIT_ENABLED=true
RATE_LIMIT_REQUESTS=1000
RATE_LIMIT_WINDOW=60

# Logging
RUST_LOG=info
LOG_FORMAT=json

# Monitoring
METRICS_ENABLED=true
METRICS_PORT=9090

# Scraping
SCRAPER_ENABLED=true
SCRAPER_INTERVAL=300
SCRAPER_BATCH_SIZE=100

# Secret Scanning
SECRET_SCANNER_ENABLED=true
SECRET_SCANNER_THREADS=4
SECRET_VALIDATION_ENABLED=true

# Notifications (optional)
SLACK_WEBHOOK_URL=https://hooks.slack.com/services/xxx/yyy/zzz
```

### Development Configuration
```bash
# Database
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/github_archiver_dev

# Security (dev only - not secure!)
JWT_SECRET=dev-secret-key-min-32-chars-long
ADMIN_PASSWORD=admin

# GitHub
GITHUB_TOKEN=ghp_REDACTED_EXAMPLE

# Server
SERVER_HOST=127.0.0.1
SERVER_PORT=8081

# Logging
RUST_LOG=debug
LOG_FORMAT=pretty

# Development
DEV_MODE=true
MOCK_GITHUB_API=false
```

### Testing Configuration
```bash
# Database
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/github_archiver_test

# Security
JWT_SECRET=test-secret-key-minimum-32-chars
ADMIN_PASSWORD=test

# Testing
TEST_MODE=true
MOCK_GITHUB_API=true
DEV_MODE=true

# Logging
RUST_LOG=debug
LOG_FORMAT=pretty

# Disable external services
SCRAPER_ENABLED=false
WEBHOOK_ENABLED=false
SECRET_VALIDATION_ENABLED=false
```

---

## Configuration Best Practices

### Security
1. **Never commit secrets** to version control
2. **Use strong JWT_SECRET**: Minimum 32 characters, random
3. **Rotate secrets regularly**: JWT_SECRET, ADMIN_PASSWORD
4. **Use environment-specific secrets**: Different for dev/staging/prod
5. **Store secrets securely**: Use Kubernetes Secrets, AWS Secrets Manager, etc.

### Performance
1. **Tune connection pools**: Set `DB_MAX_CONNECTIONS` based on workload
2. **Adjust rate limits**: Balance between protection and usability
3. **Configure worker threads**: Match `SERVER_WORKERS` to CPU cores
4. **Enable caching**: Use Redis for high-traffic deployments

### Monitoring
1. **Always enable metrics** in production
2. **Use structured logging** (`LOG_FORMAT=json`)
3. **Set appropriate log levels**: `info` for production, `debug` for troubleshooting
4. **Enable health checks**: Critical for Kubernetes

### Reliability
1. **Configure retries**: Set appropriate values for `SCRAPER_MAX_RETRIES`, `WEBHOOK_MAX_RETRIES`
2. **Set timeouts**: `DB_CONNECTION_TIMEOUT`, `WEBHOOK_TIMEOUT`
3. **Enable graceful shutdown**: (automatic, no configuration needed)
4. **Monitor and alert**: Use Prometheus metrics and alerts

---

## Troubleshooting

### "Connection refused" on startup
- Check `DATABASE_URL` is correct
- Verify database is accessible
- Check network connectivity

### "JWT validation failed"
- Verify `JWT_SECRET` matches across all instances
- Check token expiration settings
- Ensure system clocks are synchronized

### "Rate limit exceeded"
- Adjust `RATE_LIMIT_REQUESTS` and `RATE_LIMIT_WINDOW`
- Check for bot traffic or DOS attacks
- Consider implementing IP whitelisting

### High memory usage
- Reduce `DB_MAX_CONNECTIONS`
- Disable `CACHE` or reduce `CACHE_TTL`
- Check for memory leaks in logs

### GitHub API rate limit hit
- Verify `GITHUB_TOKEN` has sufficient quota
- Reduce `SCRAPER_INTERVAL` to scrape less frequently
- Consider using multiple tokens (round-robin)

---

## References

- [PostgreSQL Connection Strings](https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNSTRING)
- [GitHub Personal Access Tokens](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token)
- [Prometheus Metrics](https://prometheus.io/docs/practices/naming/)
- [Rust Log Levels](https://docs.rs/env_logger/latest/env_logger/)
