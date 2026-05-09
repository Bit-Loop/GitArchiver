-- GitHub Archiver Database Schema
-- Creating extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "btree_gin";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Drop existing table to recreate with full schema
DROP TABLE IF EXISTS github_events CASCADE;

-- Main events table with comprehensive GitHub API data capture
CREATE TABLE github_events (
    event_id BIGINT PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    event_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    event_public BOOLEAN NOT NULL DEFAULT true,
    
    -- Actor information
    actor_id BIGINT,
    actor_login VARCHAR(255),
    actor_display_login VARCHAR(255),
    actor_gravatar_id VARCHAR(255),
    actor_url TEXT,
    actor_avatar_url TEXT,
    actor_node_id VARCHAR(255),
    actor_html_url TEXT,
    actor_followers_url TEXT,
    actor_following_url TEXT,
    actor_gists_url TEXT,
    actor_starred_url TEXT,
    actor_subscriptions_url TEXT,
    actor_organizations_url TEXT,
    actor_repos_url TEXT,
    actor_events_url TEXT,
    actor_received_events_url TEXT,
    actor_type VARCHAR(50),
    actor_user_view_type VARCHAR(50),
    actor_site_admin BOOLEAN,
    
    -- Repository information
    repo_id BIGINT,
    repo_name VARCHAR(255),
    repo_url TEXT,
    repo_full_name VARCHAR(255),
    repo_owner_login VARCHAR(255),
    repo_owner_id BIGINT,
    repo_owner_node_id VARCHAR(255),
    repo_owner_avatar_url TEXT,
    repo_owner_gravatar_id VARCHAR(255),
    repo_owner_url TEXT,
    repo_owner_html_url TEXT,
    repo_owner_type VARCHAR(50),
    repo_owner_site_admin BOOLEAN,
    repo_node_id VARCHAR(255),
    repo_html_url TEXT,
    repo_description TEXT,
    repo_fork BOOLEAN,
    repo_language VARCHAR(100),
    repo_stargazers_count BIGINT,
    repo_watchers_count BIGINT,
    repo_forks_count BIGINT,
    repo_open_issues_count BIGINT,
    repo_size BIGINT,
    repo_default_branch VARCHAR(100),
    repo_topics TEXT[],
    repo_license_key VARCHAR(50),
    repo_license_name VARCHAR(255),
    repo_created_at TIMESTAMP WITH TIME ZONE,
    repo_updated_at TIMESTAMP WITH TIME ZONE,
    repo_pushed_at TIMESTAMP WITH TIME ZONE,
    
    -- Organization information (optional)
    org_id BIGINT,
    org_login VARCHAR(255),
    org_node_id VARCHAR(255),
    org_gravatar_id VARCHAR(255),
    org_url TEXT,
    org_avatar_url TEXT,
    org_html_url TEXT,
    org_type VARCHAR(50),
    org_site_admin BOOLEAN,
    
    -- Complete payload as JSONB for flexible querying
    payload JSONB,
    raw_event JSONB,
    
    -- Metadata
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    file_source VARCHAR(255),
    api_source VARCHAR(255)
);

-- Processed files tracking
CREATE TABLE IF NOT EXISTS processed_files (
    filename VARCHAR(255) PRIMARY KEY,
    file_size BIGINT,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    etag VARCHAR(255),
    last_modified TIMESTAMP WITH TIME ZONE,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    event_count INTEGER DEFAULT 0,
    events_count BIGINT DEFAULT 0,
    is_complete BOOLEAN DEFAULT TRUE
);

-- Repositories tracking table
CREATE TABLE IF NOT EXISTS repositories (
    id BIGINT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    full_name VARCHAR(255),
    description TEXT,
    html_url TEXT,
    language VARCHAR(100),
    default_branch VARCHAR(100),
    created_at TIMESTAMP WITH TIME ZONE,
    updated_at TIMESTAMP WITH TIME ZONE,
    pushed_at TIMESTAMP WITH TIME ZONE,
    stargazers_count INTEGER,
    watchers_count INTEGER,
    forks_count INTEGER,
    open_issues_count INTEGER,
    topics TEXT[],
    license_name VARCHAR(255),
    owner_login VARCHAR(255),
    owner_type VARCHAR(50),
    first_seen_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Pending push events queue feeding the scanner
CREATE TABLE IF NOT EXISTS pending_push_scans (
    event_id BIGINT PRIMARY KEY REFERENCES github_events(event_id) ON DELETE CASCADE,
    repository_full_name VARCHAR(255) NOT NULL,
    repository_url TEXT,
    before_sha VARCHAR(64) NOT NULL,
    head_sha VARCHAR(64),
    ref_name VARCHAR(255),
    forced_flag BOOLEAN NOT NULL DEFAULT false,
    commit_span INTEGER NOT NULL DEFAULT 0,
    is_zero_commit BOOLEAN NOT NULL DEFAULT false,
    event_payload JSONB,
    event_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMP WITH TIME ZONE,
    next_attempt_after TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    locked_by VARCHAR(128),
    locked_at TIMESTAMP WITH TIME ZONE,
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX IF NOT EXISTS idx_pending_push_scans_status
    ON pending_push_scans (status, next_attempt_after);
CREATE INDEX IF NOT EXISTS idx_pending_push_scans_claim_window
    ON pending_push_scans (status, next_attempt_after, event_created_at);
CREATE INDEX IF NOT EXISTS idx_pending_push_scans_repo
    ON pending_push_scans (repository_full_name);
CREATE INDEX IF NOT EXISTS idx_pending_push_scans_processing_repo_created
    ON pending_push_scans (status, LOWER(repository_full_name), event_created_at);
CREATE INDEX IF NOT EXISTS idx_pending_push_scans_created
    ON pending_push_scans (event_created_at);

-- Secret scan executions (manual, realtime, backfill)
CREATE TABLE IF NOT EXISTS secret_scans (
    id UUID PRIMARY KEY,
    repository VARCHAR(255),
    scan_type VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    source VARCHAR(50) NOT NULL,
    started_at TIMESTAMP WITH TIME ZONE NOT NULL,
    completed_at TIMESTAMP WITH TIME ZONE,
    duration_ms BIGINT,
    files_scanned BIGINT,
    secrets_found BIGINT DEFAULT 0,
    created_by VARCHAR(255) NOT NULL,
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Individual secret detections persisted for reporting/export
CREATE TABLE IF NOT EXISTS secret_detections (
    detection_id UUID PRIMARY KEY,
    scan_id UUID REFERENCES secret_scans(id) ON DELETE SET NULL,
    event_id BIGINT,
    repository VARCHAR(255) NOT NULL,
    file_path TEXT,
    detector_name VARCHAR(255) NOT NULL,
    severity VARCHAR(50) NOT NULL,
    category VARCHAR(50) NOT NULL,
    matched_text_hash VARCHAR(128) NOT NULL,
    matched_text_preview VARCHAR(255) NOT NULL,
    line_number INTEGER,
    verified BOOLEAN DEFAULT false,
    detected_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    source VARCHAR(50) NOT NULL,
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Redacted local-AI triage results and provider provenance
CREATE TABLE IF NOT EXISTS ai_triage_results (
    id UUID PRIMARY KEY,
    detection_id UUID REFERENCES secret_detections(detection_id) ON DELETE SET NULL,
    secret_hash VARCHAR(128) NOT NULL,
    provider VARCHAR(50) NOT NULL,
    model VARCHAR(255) NOT NULL,
    base_url TEXT NOT NULL,
    redacted_input JSONB NOT NULL,
    result JSONB NOT NULL,
    status VARCHAR(50) NOT NULL,
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE
);

-- Audited maintenance actions against scan/queue state
CREATE TABLE IF NOT EXISTS maintenance_repair_runs (
    id UUID PRIMARY KEY,
    repair_type VARCHAR(80) NOT NULL,
    dry_run BOOLEAN NOT NULL DEFAULT FALSE,
    backup_path TEXT,
    hard_delete_invalid_summaries BOOLEAN NOT NULL DEFAULT FALSE,
    reset_stale_processing BOOLEAN NOT NULL DEFAULT FALSE,
    pre_counts JSONB NOT NULL,
    post_counts JSONB NOT NULL,
    deleted_invalid_summaries BIGINT NOT NULL DEFAULT 0,
    reset_stale_processing_rows BIGINT NOT NULL DEFAULT 0,
    operator VARCHAR(255) NOT NULL,
    executed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_github_events_created_at ON github_events (event_created_at);
CREATE INDEX IF NOT EXISTS idx_github_events_type ON github_events (event_type);
CREATE INDEX IF NOT EXISTS idx_github_events_actor_id ON github_events (actor_id);
CREATE INDEX IF NOT EXISTS idx_github_events_repo_id ON github_events (repo_id);
CREATE INDEX IF NOT EXISTS idx_github_events_actor_login ON github_events (actor_login);
CREATE INDEX IF NOT EXISTS idx_github_events_repo_name ON github_events (repo_name);
CREATE INDEX IF NOT EXISTS idx_github_events_push_repo_created
    ON github_events (
        LOWER(COALESCE(repo_full_name, repo_owner_login || '/' || repo_name)),
        event_created_at DESC
    )
    WHERE event_type = 'PushEvent';
CREATE INDEX IF NOT EXISTS idx_github_events_payload ON github_events USING GIN (payload);
CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories (language);
CREATE INDEX IF NOT EXISTS idx_repositories_stars ON repositories (stargazers_count DESC);
CREATE INDEX IF NOT EXISTS idx_secret_scans_status_completed
    ON secret_scans (status, completed_at DESC);
CREATE INDEX IF NOT EXISTS idx_secret_scans_repository_completed
    ON secret_scans (repository, completed_at DESC);
CREATE INDEX IF NOT EXISTS idx_secret_detections_timestamp ON secret_detections (detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_secret_detections_severity ON secret_detections (severity);
CREATE INDEX IF NOT EXISTS idx_secret_detections_category ON secret_detections (category);
CREATE INDEX IF NOT EXISTS idx_secret_detections_repo ON secret_detections (repository);
CREATE INDEX IF NOT EXISTS idx_secret_detections_repo_detected
    ON secret_detections (repository, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_secret_detections_repo_trgm
    ON secret_detections USING GIN (repository gin_trgm_ops);
CREATE UNIQUE INDEX IF NOT EXISTS idx_secret_detections_unique_match
    ON secret_detections (
        matched_text_hash,
        repository,
        COALESCE(file_path, ''),
        detector_name,
        source,
        COALESCE(event_id, 0)
    );
CREATE INDEX IF NOT EXISTS idx_ai_triage_results_detection
    ON ai_triage_results (detection_id);
CREATE INDEX IF NOT EXISTS idx_ai_triage_results_created
    ON ai_triage_results (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_maintenance_repair_runs_type_time
    ON maintenance_repair_runs (repair_type, executed_at DESC);
