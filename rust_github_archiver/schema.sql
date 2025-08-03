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
    etag VARCHAR(255),
    last_modified TIMESTAMP WITH TIME ZONE,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    event_count INTEGER DEFAULT 0,
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

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_github_events_created_at ON github_events (event_created_at);
CREATE INDEX IF NOT EXISTS idx_github_events_type ON github_events (event_type);
CREATE INDEX IF NOT EXISTS idx_github_events_actor_id ON github_events (actor_id);
CREATE INDEX IF NOT EXISTS idx_github_events_repo_id ON github_events (repo_id);
CREATE INDEX IF NOT EXISTS idx_github_events_actor_login ON github_events (actor_login);
CREATE INDEX IF NOT EXISTS idx_github_events_repo_name ON github_events (repo_name);
CREATE INDEX IF NOT EXISTS idx_github_events_payload ON github_events USING GIN (payload);
CREATE INDEX IF NOT EXISTS idx_repositories_language ON repositories (language);
CREATE INDEX IF NOT EXISTS idx_repositories_stars ON repositories (stargazers_count DESC);
