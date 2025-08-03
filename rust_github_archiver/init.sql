-- GitHub Archive Scraper - Database Initialization
-- PostgreSQL schema for GitHub event storage and analysis

-- ═══════════════════════════════════════════════════════════════════════════════
-- Drop existing tables if they exist (for clean reinstall)
-- ═══════════════════════════════════════════════════════════════════════════════
DROP TABLE IF EXISTS github_events CASCADE;
DROP TABLE IF EXISTS repositories CASCADE;
DROP TABLE IF EXISTS actors CASCADE;
DROP TABLE IF EXISTS processing_status CASCADE;
DROP TABLE IF EXISTS download_queue CASCADE;
DROP TABLE IF EXISTS system_metrics CASCADE;

-- Drop existing types if they exist
DROP TYPE IF EXISTS event_type CASCADE;
DROP TYPE IF EXISTS processing_status_type CASCADE;
DROP TYPE IF EXISTS download_status_type CASCADE;

-- ═══════════════════════════════════════════════════════════════════════════════
-- Create ENUM types for better data integrity
-- ═══════════════════════════════════════════════════════════════════════════════

-- GitHub event types
CREATE TYPE event_type AS ENUM (
    'CommitCommentEvent',
    'CreateEvent', 
    'DeleteEvent',
    'ForkEvent',
    'GollumEvent',
    'IssueCommentEvent',
    'IssuesEvent',
    'MemberEvent',
    'PublicEvent',
    'PullRequestEvent',
    'PullRequestReviewCommentEvent',
    'PushEvent',
    'ReleaseEvent',
    'SponsorshipEvent',
    'WatchEvent',
    'GistEvent',
    'FollowEvent',
    'DownloadEvent',
    'TeamAddEvent',
    'ForkApplyEvent'
);

-- Processing status types
CREATE TYPE processing_status_type AS ENUM (
    'pending',
    'processing',
    'completed',
    'failed',
    'skipped'
);

-- Download status types
CREATE TYPE download_status_type AS ENUM (
    'queued',
    'downloading',
    'completed',
    'failed',
    'retrying'
);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Actors (GitHub users/organizations)
-- ═══════════════════════════════════════════════════════════════════════════════
CREATE TABLE actors (
    id BIGINT PRIMARY KEY,
    login VARCHAR(255) NOT NULL,
    display_login VARCHAR(255),
    gravatar_id VARCHAR(255),
    url VARCHAR(500),
    avatar_url VARCHAR(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT unique_actor_login UNIQUE (login)
);

CREATE INDEX idx_actors_login ON actors(login);
CREATE INDEX idx_actors_id ON actors(id);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Repositories
-- ═══════════════════════════════════════════════════════════════════════════════
CREATE TABLE repositories (
    id BIGINT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    full_name VARCHAR(500) NOT NULL,
    owner_id BIGINT,
    private BOOLEAN DEFAULT FALSE,
    html_url VARCHAR(500),
    description TEXT,
    fork BOOLEAN DEFAULT FALSE,
    homepage VARCHAR(500),
    size_kb INTEGER,
    stargazers_count INTEGER DEFAULT 0,
    watchers_count INTEGER DEFAULT 0,
    language VARCHAR(100),
    has_issues BOOLEAN DEFAULT TRUE,
    has_projects BOOLEAN DEFAULT TRUE,
    has_wiki BOOLEAN DEFAULT TRUE,
    has_pages BOOLEAN DEFAULT FALSE,
    forks_count INTEGER DEFAULT 0,
    archived BOOLEAN DEFAULT FALSE,
    disabled BOOLEAN DEFAULT FALSE,
    open_issues_count INTEGER DEFAULT 0,
    topics JSONB,
    default_branch VARCHAR(255) DEFAULT 'main',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT unique_repo_full_name UNIQUE (full_name),
    CONSTRAINT fk_repo_owner FOREIGN KEY (owner_id) REFERENCES actors(id)
);

CREATE INDEX idx_repositories_id ON repositories(id);
CREATE INDEX idx_repositories_full_name ON repositories(full_name);
CREATE INDEX idx_repositories_owner_id ON repositories(owner_id);
CREATE INDEX idx_repositories_language ON repositories(language);
CREATE INDEX idx_repositories_stargazers ON repositories(stargazers_count);
CREATE INDEX idx_repositories_created_at ON repositories(created_at);

-- ═══════════════════════════════════════════════════════════════════════════════
-- GitHub Events (Main table)
-- ═══════════════════════════════════════════════════════════════════════════════
CREATE TABLE github_events (
    id BIGINT PRIMARY KEY,
    event_type event_type NOT NULL,
    actor_id BIGINT,
    repo_id BIGINT,
    public BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL,
    payload JSONB,
    
    -- Additional metadata
    processed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    archive_file VARCHAR(500),
    archive_date DATE,
    
    -- Constraints
    CONSTRAINT fk_event_actor FOREIGN KEY (actor_id) REFERENCES actors(id),
    CONSTRAINT fk_event_repo FOREIGN KEY (repo_id) REFERENCES repositories(id)
);

-- Partitioning by month for better performance
CREATE INDEX idx_github_events_created_at ON github_events(created_at);
CREATE INDEX idx_github_events_type ON github_events(event_type);
CREATE INDEX idx_github_events_actor_id ON github_events(actor_id);
CREATE INDEX idx_github_events_repo_id ON github_events(repo_id);
CREATE INDEX idx_github_events_archive_date ON github_events(archive_date);
CREATE INDEX idx_github_events_payload_gin ON github_events USING GIN(payload);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Processing Status Tracking
-- ═══════════════════════════════════════════════════════════════════════════════
CREATE TABLE processing_status (
    id SERIAL PRIMARY KEY,
    file_path VARCHAR(1000) NOT NULL,
    file_size BIGINT,
    status processing_status_type DEFAULT 'pending',
    events_count INTEGER DEFAULT 0,
    events_processed INTEGER DEFAULT 0,
    events_inserted INTEGER DEFAULT 0,
    events_failed INTEGER DEFAULT 0,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    error_message TEXT,
    processing_time_seconds INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT unique_file_path UNIQUE (file_path)
);

CREATE INDEX idx_processing_status_status ON processing_status(status);
CREATE INDEX idx_processing_status_created_at ON processing_status(created_at);
CREATE INDEX idx_processing_status_file_path ON processing_status(file_path);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Download Queue Management
-- ═══════════════════════════════════════════════════════════════════════════════
CREATE TABLE download_queue (
    id SERIAL PRIMARY KEY,
    url VARCHAR(1000) NOT NULL,
    local_path VARCHAR(1000),
    status download_status_type DEFAULT 'queued',
    priority INTEGER DEFAULT 100,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    file_size BIGINT,
    downloaded_size BIGINT DEFAULT 0,
    download_speed_mbps DECIMAL(10,2),
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT unique_download_url UNIQUE (url)
);

CREATE INDEX idx_download_queue_status ON download_queue(status);
CREATE INDEX idx_download_queue_priority ON download_queue(priority DESC);
CREATE INDEX idx_download_queue_created_at ON download_queue(created_at);

-- ═══════════════════════════════════════════════════════════════════════════════
-- System Metrics for Monitoring
-- ═══════════════════════════════════════════════════════════════════════════════
CREATE TABLE system_metrics (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    -- Resource Usage
    memory_used_gb DECIMAL(10,2),
    memory_total_gb DECIMAL(10,2),
    memory_usage_percent DECIMAL(5,2),
    cpu_usage_percent DECIMAL(5,2),
    disk_used_gb DECIMAL(10,2),
    disk_total_gb DECIMAL(10,2),
    disk_usage_percent DECIMAL(5,2),
    
    -- Application Metrics
    events_processed_total BIGINT DEFAULT 0,
    events_per_second DECIMAL(10,2),
    files_processed_total INTEGER DEFAULT 0,
    downloads_active INTEGER DEFAULT 0,
    downloads_queued INTEGER DEFAULT 0,
    database_connections_active INTEGER DEFAULT 0,
    
    -- Performance Metrics
    avg_processing_time_ms DECIMAL(10,2),
    avg_download_speed_mbps DECIMAL(10,2),
    error_rate_percent DECIMAL(5,2)
);

CREATE INDEX idx_system_metrics_timestamp ON system_metrics(timestamp);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Views for Common Queries
-- ═══════════════════════════════════════════════════════════════════════════════

-- Events with full details
CREATE VIEW events_detailed AS
SELECT 
    e.id,
    e.event_type,
    e.created_at,
    e.public,
    a.login as actor_login,
    a.display_login as actor_display_login,
    r.full_name as repo_full_name,
    r.language as repo_language,
    r.stargazers_count as repo_stars,
    e.payload,
    e.archive_date,
    e.archive_file
FROM github_events e
LEFT JOIN actors a ON e.actor_id = a.id
LEFT JOIN repositories r ON e.repo_id = r.id;

-- Daily statistics
CREATE VIEW daily_stats AS
SELECT 
    DATE(created_at) as date,
    COUNT(*) as total_events,
    COUNT(DISTINCT actor_id) as unique_actors,
    COUNT(DISTINCT repo_id) as unique_repositories,
    event_type,
    COUNT(*) as event_count
FROM github_events
GROUP BY DATE(created_at), event_type
ORDER BY date DESC, event_count DESC;

-- Repository popularity
CREATE VIEW repo_popularity AS
SELECT 
    r.full_name,
    r.language,
    r.stargazers_count,
    r.forks_count,
    COUNT(e.id) as total_events,
    COUNT(DISTINCT e.actor_id) as unique_contributors,
    MAX(e.created_at) as last_activity
FROM repositories r
LEFT JOIN github_events e ON r.id = e.repo_id
GROUP BY r.id, r.full_name, r.language, r.stargazers_count, r.forks_count
ORDER BY total_events DESC;

-- ═══════════════════════════════════════════════════════════════════════════════
-- Functions for Data Quality and Maintenance
-- ═══════════════════════════════════════════════════════════════════════════════

-- Function to update repository statistics
CREATE OR REPLACE FUNCTION update_repository_stats()
RETURNS VOID AS $$
BEGIN
    -- This would be populated from GitHub API calls
    -- For now, we'll calculate based on events
    UPDATE repositories SET
        updated_at = CURRENT_TIMESTAMP
    WHERE id IN (
        SELECT DISTINCT repo_id 
        FROM github_events 
        WHERE created_at > CURRENT_TIMESTAMP - INTERVAL '1 day'
    );
END;
$$ LANGUAGE plpgsql;

-- Function to clean old system metrics
CREATE OR REPLACE FUNCTION cleanup_old_metrics()
RETURNS VOID AS $$
BEGIN
    DELETE FROM system_metrics 
    WHERE timestamp < CURRENT_TIMESTAMP - INTERVAL '30 days';
    
    DELETE FROM processing_status 
    WHERE status = 'completed' 
    AND completed_at < CURRENT_TIMESTAMP - INTERVAL '7 days';
END;
$$ LANGUAGE plpgsql;

-- Function to get processing statistics
CREATE OR REPLACE FUNCTION get_processing_stats()
RETURNS TABLE (
    total_files INTEGER,
    completed_files INTEGER,
    failed_files INTEGER,
    pending_files INTEGER,
    total_events BIGINT,
    avg_processing_time DECIMAL(10,2)
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        COUNT(*)::INTEGER as total_files,
        COUNT(CASE WHEN status = 'completed' THEN 1 END)::INTEGER as completed_files,
        COUNT(CASE WHEN status = 'failed' THEN 1 END)::INTEGER as failed_files,
        COUNT(CASE WHEN status = 'pending' THEN 1 END)::INTEGER as pending_files,
        COALESCE(SUM(events_inserted), 0) as total_events,
        COALESCE(AVG(processing_time_seconds), 0) as avg_processing_time
    FROM processing_status;
END;
$$ LANGUAGE plpgsql;

-- ═══════════════════════════════════════════════════════════════════════════════
-- Triggers for automatic updates
-- ═══════════════════════════════════════════════════════════════════════════════

-- Update timestamp trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply triggers to tables with updated_at columns
CREATE TRIGGER update_actors_updated_at
    BEFORE UPDATE ON actors
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_repositories_updated_at
    BEFORE UPDATE ON repositories
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_processing_status_updated_at
    BEFORE UPDATE ON processing_status
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_download_queue_updated_at
    BEFORE UPDATE ON download_queue
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ═══════════════════════════════════════════════════════════════════════════════
-- Initial Data and Configuration
-- ═══════════════════════════════════════════════════════════════════════════════

-- Insert initial system metrics record
INSERT INTO system_metrics (
    memory_used_gb, memory_total_gb, memory_usage_percent,
    cpu_usage_percent, disk_used_gb, disk_total_gb, disk_usage_percent,
    events_processed_total, events_per_second, files_processed_total,
    downloads_active, downloads_queued, database_connections_active,
    avg_processing_time_ms, avg_download_speed_mbps, error_rate_percent
) VALUES (
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0, 0.0, 0, 0, 0, 0,
    0.0, 0.0, 0.0
);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Performance Optimization Settings
-- ═══════════════════════════════════════════════════════════════════════════════

-- Optimize for bulk inserts
ALTER TABLE github_events SET (
    fillfactor = 85,
    parallel_workers = 4
);

ALTER TABLE repositories SET (
    fillfactor = 90
);

ALTER TABLE actors SET (
    fillfactor = 90
);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Grant Permissions
-- ═══════════════════════════════════════════════════════════════════════════════

-- Grant all privileges to the application user
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO github_archiver;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO github_archiver;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO github_archiver;

-- ═══════════════════════════════════════════════════════════════════════════════
-- Summary Information
-- ═══════════════════════════════════════════════════════════════════════════════

-- Display schema information
DO $$
BEGIN
    RAISE NOTICE 'GitHub Archive Scraper Database Schema Initialized Successfully!';
    RAISE NOTICE '================================================================';
    RAISE NOTICE 'Tables Created:';
    RAISE NOTICE '  - actors: GitHub users and organizations';
    RAISE NOTICE '  - repositories: GitHub repositories';
    RAISE NOTICE '  - github_events: Main events table';
    RAISE NOTICE '  - processing_status: File processing tracking';
    RAISE NOTICE '  - download_queue: Download management';
    RAISE NOTICE '  - system_metrics: Performance monitoring';
    RAISE NOTICE '';
    RAISE NOTICE 'Views Created:';
    RAISE NOTICE '  - events_detailed: Events with full details';
    RAISE NOTICE '  - daily_stats: Daily event statistics';
    RAISE NOTICE '  - repo_popularity: Repository popularity metrics';
    RAISE NOTICE '';
    RAISE NOTICE 'Functions Created:';
    RAISE NOTICE '  - update_repository_stats(): Update repo statistics';
    RAISE NOTICE '  - cleanup_old_metrics(): Clean old monitoring data';
    RAISE NOTICE '  - get_processing_stats(): Get processing statistics';
    RAISE NOTICE '';
    RAISE NOTICE 'Database is ready for GitHub Archive Scraper!';
    RAISE NOTICE '================================================================';
END $$;
