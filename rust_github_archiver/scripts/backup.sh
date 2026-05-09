#!/bin/bash

#############################################
# PostgreSQL Backup Script for GitHub Archiver
# Creates compressed backups with rotation
#############################################

set -euo pipefail

# Configuration
BACKUP_DIR="${BACKUP_DIR:-/var/backups/github-archiver}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-github_archiver}"
DB_USER="${DB_USER:-postgres}"
S3_BUCKET="${S3_BUCKET:-github-archiver-backups}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="github_archiver_${TIMESTAMP}.sql.gz"
BACKUP_PATH="${BACKUP_DIR}/${BACKUP_FILE}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Create backup directory if it doesn't exist
mkdir -p "${BACKUP_DIR}"

log_info "Starting backup of database ${DB_NAME}..."

# Check if database is accessible
if ! pg_isready -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" >/dev/null 2>&1; then
    log_error "Database is not accessible at ${DB_HOST}:${DB_PORT}"
    exit 1
fi

# Get database size before backup
DB_SIZE=$(psql -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d "${DB_NAME}" -t -c \
    "SELECT pg_size_pretty(pg_database_size('${DB_NAME}'));" | xargs)
log_info "Database size: ${DB_SIZE}"

# Create backup with compression
log_info "Creating backup: ${BACKUP_FILE}"
if pg_dump -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d "${DB_NAME}" \
    --format=plain \
    --no-owner \
    --no-privileges \
    --clean \
    --if-exists \
    | gzip > "${BACKUP_PATH}"; then
    
    log_info "Backup created successfully: ${BACKUP_PATH}"
    
    # Get backup file size
    BACKUP_SIZE=$(du -h "${BACKUP_PATH}" | cut -f1)
    log_info "Backup size: ${BACKUP_SIZE}"
else
    log_error "Backup failed"
    exit 1
fi

# Create checksum
log_info "Creating checksum..."
sha256sum "${BACKUP_PATH}" > "${BACKUP_PATH}.sha256"

# Upload to S3 if configured
if command -v aws &> /dev/null && [ -n "${S3_BUCKET}" ]; then
    log_info "Uploading to S3: s3://${S3_BUCKET}/$(date +%Y%m%d)/"
    
    if aws s3 cp "${BACKUP_PATH}" "s3://${S3_BUCKET}/$(date +%Y%m%d)/${BACKUP_FILE}" && \
       aws s3 cp "${BACKUP_PATH}.sha256" "s3://${S3_BUCKET}/$(date +%Y%m%d)/${BACKUP_FILE}.sha256"; then
        log_info "Upload to S3 successful"
    else
        log_warn "Upload to S3 failed, but local backup is available"
    fi
else
    log_warn "AWS CLI not available or S3_BUCKET not set, skipping S3 upload"
fi

# Rotate old backups
log_info "Rotating backups older than ${RETENTION_DAYS} days..."
DELETED_COUNT=$(find "${BACKUP_DIR}" -name "github_archiver_*.sql.gz" -type f -mtime +${RETENTION_DAYS} -delete -print | wc -l)
log_info "Deleted ${DELETED_COUNT} old backup(s)"

# Also delete old checksums
find "${BACKUP_DIR}" -name "github_archiver_*.sql.gz.sha256" -type f -mtime +${RETENTION_DAYS} -delete

# List current backups
log_info "Current backups:"
ls -lh "${BACKUP_DIR}"/github_archiver_*.sql.gz | tail -n 5

# Send notification (if configured)
if [ -n "${SLACK_WEBHOOK_URL:-}" ]; then
    curl -X POST "${SLACK_WEBHOOK_URL}" \
        -H 'Content-Type: application/json' \
        -d "{\"text\":\"✅ Database backup completed: ${BACKUP_FILE} (${BACKUP_SIZE})\"}" \
        2>/dev/null || true
fi

log_info "Backup completed successfully"
exit 0
