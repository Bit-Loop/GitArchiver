#!/bin/bash

#############################################
# PostgreSQL Restore Script for GitHub Archiver
# Restores from compressed backup files
#############################################

set -euo pipefail

# Configuration
BACKUP_DIR="${BACKUP_DIR:-/var/backups/github-archiver}"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-github_archiver}"
DB_USER="${DB_USER:-postgres}"
S3_BUCKET="${S3_BUCKET:-github-archiver-backups}"

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

# Show usage
usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Restore PostgreSQL database from backup

OPTIONS:
    -f FILE     Backup file to restore (required)
    -s          Download latest backup from S3
    -d DATE     Download backup from specific date (YYYYMMDD)
    -y          Skip confirmation prompt
    -h          Show this help message

EXAMPLES:
    # Restore from local file
    $0 -f /var/backups/github-archiver/github_archiver_20240101_120000.sql.gz

    # Download and restore latest from S3
    $0 -s

    # Download and restore from specific date
    $0 -s -d 20240101

EOF
    exit 1
}

# Parse arguments
BACKUP_FILE=""
DOWNLOAD_S3=false
S3_DATE=""
SKIP_CONFIRMATION=false

while getopts "f:sd:yh" opt; do
    case ${opt} in
        f)
            BACKUP_FILE="${OPTARG}"
            ;;
        s)
            DOWNLOAD_S3=true
            ;;
        d)
            S3_DATE="${OPTARG}"
            ;;
        y)
            SKIP_CONFIRMATION=true
            ;;
        h)
            usage
            ;;
        *)
            usage
            ;;
    esac
done

# Download from S3 if requested
if [ "${DOWNLOAD_S3}" = true ]; then
    if ! command -v aws &> /dev/null; then
        log_error "AWS CLI not available"
        exit 1
    fi

    if [ -z "${S3_DATE}" ]; then
        S3_DATE=$(date +%Y%m%d)
    fi

    log_info "Downloading latest backup from S3 for date ${S3_DATE}..."
    
    # List available backups
    LATEST_BACKUP=$(aws s3 ls "s3://${S3_BUCKET}/${S3_DATE}/" | grep "github_archiver_.*\.sql\.gz$" | sort | tail -n 1 | awk '{print $4}')
    
    if [ -z "${LATEST_BACKUP}" ]; then
        log_error "No backups found for date ${S3_DATE}"
        exit 1
    fi
    
    log_info "Found backup: ${LATEST_BACKUP}"
    
    BACKUP_FILE="${BACKUP_DIR}/${LATEST_BACKUP}"
    mkdir -p "${BACKUP_DIR}"
    
    aws s3 cp "s3://${S3_BUCKET}/${S3_DATE}/${LATEST_BACKUP}" "${BACKUP_FILE}"
    aws s3 cp "s3://${S3_BUCKET}/${S3_DATE}/${LATEST_BACKUP}.sha256" "${BACKUP_FILE}.sha256"
    
    log_info "Downloaded to ${BACKUP_FILE}"
fi

# Check if backup file is specified
if [ -z "${BACKUP_FILE}" ]; then
    log_error "No backup file specified"
    usage
fi

# Check if backup file exists
if [ ! -f "${BACKUP_FILE}" ]; then
    log_error "Backup file not found: ${BACKUP_FILE}"
    exit 1
fi

# Verify checksum if available
if [ -f "${BACKUP_FILE}.sha256" ]; then
    log_info "Verifying backup checksum..."
    if sha256sum -c "${BACKUP_FILE}.sha256" 2>&1 | grep -q "OK"; then
        log_info "Checksum verified successfully"
    else
        log_error "Checksum verification failed"
        exit 1
    fi
else
    log_warn "No checksum file found, skipping verification"
fi

# Get backup file size
BACKUP_SIZE=$(du -h "${BACKUP_FILE}" | cut -f1)
log_info "Backup size: ${BACKUP_SIZE}"

# Confirmation prompt
if [ "${SKIP_CONFIRMATION}" = false ]; then
    echo
    log_warn "WARNING: This will drop and recreate the database ${DB_NAME}"
    log_warn "All current data will be lost!"
    echo
    read -p "Are you sure you want to continue? (yes/no): " CONFIRM
    
    if [ "${CONFIRM}" != "yes" ]; then
        log_info "Restore cancelled"
        exit 0
    fi
fi

# Check if database is accessible
if ! pg_isready -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" >/dev/null 2>&1; then
    log_error "Database is not accessible at ${DB_HOST}:${DB_PORT}"
    exit 1
fi

# Create backup of current database before restore
log_info "Creating safety backup of current database..."
SAFETY_BACKUP="${BACKUP_DIR}/pre_restore_$(date +%Y%m%d_%H%M%S).sql.gz"
if pg_dump -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d "${DB_NAME}" | gzip > "${SAFETY_BACKUP}"; then
    log_info "Safety backup created: ${SAFETY_BACKUP}"
else
    log_warn "Failed to create safety backup, continuing anyway..."
fi

# Terminate existing connections
log_info "Terminating existing database connections..."
psql -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d postgres -c \
    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '${DB_NAME}' AND pid <> pg_backend_pid();" \
    >/dev/null 2>&1 || true

# Restore database
log_info "Restoring database from ${BACKUP_FILE}..."
START_TIME=$(date +%s)

if gunzip -c "${BACKUP_FILE}" | psql -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d "${DB_NAME}" \
    -v ON_ERROR_STOP=1 \
    --quiet \
    2>&1 | tee /tmp/restore.log; then
    
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    
    log_info "Restore completed successfully in ${DURATION} seconds"
    
    # Verify restore
    log_info "Verifying restore..."
    TABLE_COUNT=$(psql -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d "${DB_NAME}" -t -c \
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public';" | xargs)
    log_info "Restored ${TABLE_COUNT} tables"
    
    # Vacuum and analyze
    log_info "Running VACUUM ANALYZE..."
    psql -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" -d "${DB_NAME}" -c "VACUUM ANALYZE;" >/dev/null 2>&1
    
    # Send notification (if configured)
    if [ -n "${SLACK_WEBHOOK_URL:-}" ]; then
        curl -X POST "${SLACK_WEBHOOK_URL}" \
            -H 'Content-Type: application/json' \
            -d "{\"text\":\"✅ Database restore completed: ${BACKUP_FILE##*/} (${DURATION}s)\"}" \
            2>/dev/null || true
    fi
    
    log_info "Restore completed successfully"
    exit 0
else
    log_error "Restore failed, check /tmp/restore.log for details"
    
    if [ -f "${SAFETY_BACKUP}" ]; then
        log_warn "Safety backup available at: ${SAFETY_BACKUP}"
    fi
    
    # Send failure notification
    if [ -n "${SLACK_WEBHOOK_URL:-}" ]; then
        curl -X POST "${SLACK_WEBHOOK_URL}" \
            -H 'Content-Type: application/json' \
            -d "{\"text\":\"❌ Database restore failed: ${BACKUP_FILE##*/}\"}" \
            2>/dev/null || true
    fi
    
    exit 1
fi
