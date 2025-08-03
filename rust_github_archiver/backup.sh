#!/usr/bin/env bash

# GitHub Secret Hunter - Automated Backup System
# Creates secure backups of scan data, configurations, and reports

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKUP_DIR="$PROJECT_ROOT/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
DATE_READABLE=$(date "+%Y-%m-%d %H:%M:%S")

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Create backup directory
mkdir -p "$BACKUP_DIR"

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Function to create comprehensive backup
create_backup() {
    local backup_file="$BACKUP_DIR/secret_hunter_backup_$TIMESTAMP.tar.gz"
    local temp_dir="/tmp/secret_hunter_backup_$TIMESTAMP"
    
    print_status "Creating comprehensive backup..."
    
    # Create temporary directory for staging
    mkdir -p "$temp_dir"
    
    # Copy essential files and directories
    print_status "Staging files for backup..."
    
    # Configuration files
    if [ -f "$PROJECT_ROOT/.env" ]; then
        cp "$PROJECT_ROOT/.env" "$temp_dir/"
        print_status "✓ Environment configuration"
    fi
    
    if [ -f "$PROJECT_ROOT/Cargo.toml" ]; then
        cp "$PROJECT_ROOT/Cargo.toml" "$temp_dir/"
        print_status "✓ Rust project configuration"
    fi
    
    if [ -f "$PROJECT_ROOT/logging.toml" ]; then
        cp "$PROJECT_ROOT/logging.toml" "$temp_dir/"
        print_status "✓ Logging configuration"
    fi
    
    # Scripts
    for script in setup.sh health_check.sh generate_report.sh backup.sh; do
        if [ -f "$PROJECT_ROOT/$script" ]; then
            cp "$PROJECT_ROOT/$script" "$temp_dir/"
            print_status "✓ Script: $script"
        fi
    done
    
    # Source code (excluding build artifacts)
    if [ -d "$PROJECT_ROOT/src" ]; then
        cp -r "$PROJECT_ROOT/src" "$temp_dir/"
        print_status "✓ Source code"
    fi
    
    # Tauri application (excluding node_modules)
    if [ -d "$PROJECT_ROOT/tauri-app" ]; then
        mkdir -p "$temp_dir/tauri-app"
        cp -r "$PROJECT_ROOT/tauri-app/src" "$temp_dir/tauri-app/" 2>/dev/null || true
        cp "$PROJECT_ROOT/tauri-app"/*.json "$temp_dir/tauri-app/" 2>/dev/null || true
        cp "$PROJECT_ROOT/tauri-app"/*.toml "$temp_dir/tauri-app/" 2>/dev/null || true
        cp "$PROJECT_ROOT/tauri-app"/*.ts "$temp_dir/tauri-app/" 2>/dev/null || true
        cp "$PROJECT_ROOT/tauri-app"/*.js "$temp_dir/tauri-app/" 2>/dev/null || true
        cp "$PROJECT_ROOT/tauri-app"/*.html "$temp_dir/tauri-app/" 2>/dev/null || true
        print_status "✓ Tauri application (source only)"
    fi
    
    # Logs (last 7 days only to keep backup size reasonable)
    if [ -d "$PROJECT_ROOT/logs" ]; then
        mkdir -p "$temp_dir/logs"
        find "$PROJECT_ROOT/logs" -name "*.log" -mtime -7 -exec cp {} "$temp_dir/logs/" \; 2>/dev/null || true
        print_status "✓ Recent logs (last 7 days)"
    fi
    
    # Reports (last 30 days)
    if [ -d "$PROJECT_ROOT/reports" ]; then
        mkdir -p "$temp_dir/reports"
        find "$PROJECT_ROOT/reports" -name "*" -mtime -30 -exec cp {} "$temp_dir/reports/" \; 2>/dev/null || true
        print_status "✓ Recent reports (last 30 days)"
    fi
    
    # Credentials directory (if exists)
    if [ -d "$PROJECT_ROOT/credentials" ]; then
        cp -r "$PROJECT_ROOT/credentials" "$temp_dir/"
        print_status "✓ Credentials (encrypted)"
    fi
    
    # Create backup metadata
    cat > "$temp_dir/backup_metadata.json" << EOF
{
  "backup_created": "$DATE_READABLE",
  "backup_timestamp": "$TIMESTAMP",
  "hostname": "$(hostname)",
  "user": "$(whoami)",
  "project_version": "1.0.0",
  "backup_type": "comprehensive",
  "included_components": [
    "configuration",
    "source_code",
    "scripts",
    "logs",
    "reports",
    "credentials"
  ],
  "backup_size_mb": "$(du -sm "$temp_dir" | cut -f1)",
  "git_commit": "$(cd "$PROJECT_ROOT" && git rev-parse HEAD 2>/dev/null || echo 'not_available')",
  "rust_version": "$(rustc --version 2>/dev/null || echo 'not_available')"
}
EOF
    
    # Create the actual backup archive
    print_status "Creating compressed archive..."
    cd "$(dirname "$temp_dir")"
    tar -czf "$backup_file" "$(basename "$temp_dir")"
    
    # Clean up temporary directory
    rm -rf "$temp_dir"
    
    # Get backup size
    local backup_size=$(du -h "$backup_file" | cut -f1)
    
    print_success "Backup created: $backup_file ($backup_size)"
    echo "$backup_file"
}

# Function to create incremental backup (changes only)
create_incremental_backup() {
    local backup_file="$BACKUP_DIR/secret_hunter_incremental_$TIMESTAMP.tar.gz"
    local last_backup_time
    
    print_status "Creating incremental backup..."
    
    # Find the most recent full backup
    local last_full_backup=$(ls -t "$BACKUP_DIR"/secret_hunter_backup_*.tar.gz 2>/dev/null | head -n 1)
    
    if [ -z "$last_full_backup" ]; then
        print_warning "No previous full backup found. Creating full backup instead."
        create_backup
        return
    fi
    
    # Get timestamp of last full backup
    last_backup_time=$(stat -c %Y "$last_full_backup" 2>/dev/null || stat -f %m "$last_full_backup" 2>/dev/null)
    
    print_status "Creating incremental backup since $(date -d @$last_backup_time 2>/dev/null || date -r $last_backup_time 2>/dev/null)"
    
    # Find files modified since last backup
    local temp_dir="/tmp/secret_hunter_incremental_$TIMESTAMP"
    mkdir -p "$temp_dir"
    
    # Copy modified files
    find "$PROJECT_ROOT" -type f \( \
        -name "*.rs" -o \
        -name "*.toml" -o \
        -name "*.json" -o \
        -name "*.log" -o \
        -name "*.md" -o \
        -name "*.env" -o \
        -name "*.sh" \
    \) -newer "$last_full_backup" -exec cp --parents {} "$temp_dir/" \; 2>/dev/null || true
    
    # Create incremental metadata
    cat > "$temp_dir/incremental_metadata.json" << EOF
{
  "backup_type": "incremental",
  "created": "$DATE_READABLE",
  "timestamp": "$TIMESTAMP",
  "base_backup": "$(basename "$last_full_backup")",
  "changes_since": "$(date -d @$last_backup_time 2>/dev/null || date -r $last_backup_time 2>/dev/null)",
  "files_changed": $(find "$PROJECT_ROOT" -type f -newer "$last_full_backup" 2>/dev/null | wc -l)
}
EOF
    
    # Create archive
    cd "$(dirname "$temp_dir")"
    tar -czf "$backup_file" "$(basename "$temp_dir")"
    rm -rf "$temp_dir"
    
    local backup_size=$(du -h "$backup_file" | cut -f1)
    print_success "Incremental backup created: $backup_file ($backup_size)"
}

# Function to encrypt backup
encrypt_backup() {
    local backup_file="$1"
    local encrypted_file="${backup_file}.gpg"
    
    if command -v gpg >/dev/null 2>&1; then
        print_status "Encrypting backup..."
        gpg --symmetric --cipher-algo AES256 --output "$encrypted_file" "$backup_file"
        
        if [ $? -eq 0 ]; then
            print_success "Backup encrypted: $encrypted_file"
            print_warning "Original unencrypted backup retained for convenience"
        else
            print_warning "Encryption failed, keeping unencrypted backup"
        fi
    else
        print_warning "GPG not available. Backup will remain unencrypted."
    fi
}

# Function to cleanup old backups
cleanup_old_backups() {
    local retention_days=${1:-30}  # Default 30 days retention
    
    print_status "Cleaning up backups older than $retention_days days..."
    
    # Clean up full backups
    local deleted_count=0
    while IFS= read -r -d '' file; do
        rm "$file"
        deleted_count=$((deleted_count + 1))
        print_status "Deleted: $(basename "$file")"
    done < <(find "$BACKUP_DIR" -name "secret_hunter_backup_*.tar.gz" -mtime +$retention_days -print0 2>/dev/null)
    
    # Clean up incremental backups
    while IFS= read -r -d '' file; do
        rm "$file"
        deleted_count=$((deleted_count + 1))
        print_status "Deleted: $(basename "$file")"
    done < <(find "$BACKUP_DIR" -name "secret_hunter_incremental_*.tar.gz" -mtime +$retention_days -print0 2>/dev/null)
    
    # Clean up encrypted backups
    while IFS= read -r -d '' file; do
        rm "$file"
        deleted_count=$((deleted_count + 1))
        print_status "Deleted: $(basename "$file")"
    done < <(find "$BACKUP_DIR" -name "*.tar.gz.gpg" -mtime +$retention_days -print0 2>/dev/null)
    
    if [ $deleted_count -eq 0 ]; then
        print_status "No old backups to clean up"
    else
        print_success "Cleaned up $deleted_count old backup files"
    fi
}

# Function to verify backup integrity
verify_backup() {
    local backup_file="$1"
    
    print_status "Verifying backup integrity..."
    
    if tar -tzf "$backup_file" >/dev/null 2>&1; then
        print_success "Backup integrity verified: $(basename "$backup_file")"
        
        # Show backup contents summary
        local file_count=$(tar -tzf "$backup_file" | wc -l)
        print_status "Backup contains $file_count files"
        
        return 0
    else
        print_warning "Backup integrity check failed: $(basename "$backup_file")"
        return 1
    fi
}

# Function to create backup index
create_backup_index() {
    local index_file="$BACKUP_DIR/backup_index.json"
    
    print_status "Creating backup index..."
    
    echo "{" > "$index_file"
    echo "  \"last_updated\": \"$DATE_READABLE\"," >> "$index_file"
    echo "  \"backups\": [" >> "$index_file"
    
    local first=true
    for backup in "$BACKUP_DIR"/*.tar.gz; do
        if [ -f "$backup" ]; then
            if [ "$first" = true ]; then
                first=false
            else
                echo "," >> "$index_file"
            fi
            
            local size=$(du -h "$backup" | cut -f1)
            local date=$(stat -c %y "$backup" 2>/dev/null | cut -d' ' -f1 || stat -f %Sm -t %Y-%m-%d "$backup" 2>/dev/null)
            
            echo "    {" >> "$index_file"
            echo "      \"filename\": \"$(basename "$backup")\"," >> "$index_file"
            echo "      \"size\": \"$size\"," >> "$index_file"
            echo "      \"created\": \"$date\"," >> "$index_file"
            echo "      \"type\": \"$(if [[ "$backup" == *"incremental"* ]]; then echo "incremental"; else echo "full"; fi)\"" >> "$index_file"
            echo -n "    }" >> "$index_file"
        fi
    done
    
    echo "" >> "$index_file"
    echo "  ]" >> "$index_file"
    echo "}" >> "$index_file"
    
    print_success "Backup index updated: $index_file"
}

# Function to show backup statistics
show_backup_stats() {
    print_status "Backup Statistics:"
    echo "=================="
    
    local total_backups=$(ls -1 "$BACKUP_DIR"/*.tar.gz 2>/dev/null | wc -l)
    local total_size=$(du -sh "$BACKUP_DIR" 2>/dev/null | cut -f1)
    local full_backups=$(ls -1 "$BACKUP_DIR"/secret_hunter_backup_*.tar.gz 2>/dev/null | wc -l)
    local incremental_backups=$(ls -1 "$BACKUP_DIR"/secret_hunter_incremental_*.tar.gz 2>/dev/null | wc -l)
    
    echo "• Total backups: $total_backups"
    echo "• Full backups: $full_backups"
    echo "• Incremental backups: $incremental_backups"
    echo "• Total size: $total_size"
    
    if [ $total_backups -gt 0 ]; then
        echo "• Latest backup: $(ls -t "$BACKUP_DIR"/*.tar.gz 2>/dev/null | head -n 1 | xargs basename)"
        echo "• Oldest backup: $(ls -tr "$BACKUP_DIR"/*.tar.gz 2>/dev/null | head -n 1 | xargs basename)"
    fi
}

# Main function
main() {
    echo -e "${CYAN}💾 GitHub Secret Hunter - Backup System${NC}"
    echo "========================================"
    
    local backup_type="${1:-full}"  # Default to full backup
    local encrypt="${2:-false}"     # Default no encryption
    local retention="${3:-30}"      # Default 30 days retention
    
    case "$backup_type" in
        "full")
            local backup_file=$(create_backup)
            verify_backup "$backup_file"
            
            if [ "$encrypt" = "true" ] || [ "$encrypt" = "encrypt" ]; then
                encrypt_backup "$backup_file"
            fi
            ;;
            
        "incremental")
            create_incremental_backup
            ;;
            
        "cleanup")
            cleanup_old_backups "$retention"
            ;;
            
        "stats")
            show_backup_stats
            ;;
            
        "verify")
            if [ -n "$2" ] && [ -f "$BACKUP_DIR/$2" ]; then
                verify_backup "$BACKUP_DIR/$2"
            else
                echo "Usage: $0 verify <backup_filename>"
                exit 1
            fi
            ;;
            
        *)
            echo "Usage: $0 [full|incremental|cleanup|stats|verify] [encrypt] [retention_days]"
            echo ""
            echo "Options:"
            echo "  full         - Create full backup (default)"
            echo "  incremental  - Create incremental backup"
            echo "  cleanup      - Remove old backups"
            echo "  stats        - Show backup statistics"
            echo "  verify       - Verify backup integrity"
            echo ""
            echo "Examples:"
            echo "  $0                          # Create full backup"
            echo "  $0 full encrypt             # Create encrypted full backup"
            echo "  $0 incremental              # Create incremental backup"
            echo "  $0 cleanup 14               # Clean backups older than 14 days"
            echo "  $0 stats                    # Show backup statistics"
            exit 1
            ;;
    esac
    
    # Always update the backup index
    create_backup_index
    
    echo ""
    show_backup_stats
    
    print_success "🎉 Backup operation completed successfully!"
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
