use anyhow::{anyhow, Result};
use octocrab::{Octocrab, models::Repository};
use redis::{Client as RedisClient, Connection, Commands};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

/// GitHub API client for fetching dangling commits
pub struct DanglingCommitFetcher {
    github: Octocrab,
    redis: Option<RedisClient>,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub repository: String,
    pub url: String,
    pub author: Option<CommitAuthor>,
    pub committer: Option<CommitAuthor>,
    pub message: String,
    pub tree_sha: String,
    pub parents: Vec<String>,
    pub stats: Option<CommitStats>,
    pub files: Vec<CommitFile>,
    pub html_url: String,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitFile {
    pub filename: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub changes: u32,
    pub patch: Option<String>,
    pub raw_url: Option<String>,
    pub blob_url: Option<String>,
}

/// Rate limiter for GitHub API
pub struct RateLimiter {
    requests_remaining: i32,
    reset_time: Instant,
    delay_factor: f64,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            requests_remaining: 5000, // Default GitHub API limit
            reset_time: Instant::now() + Duration::from_secs(3600),
            delay_factor: 1.0,
        }
    }
}

impl RateLimiter {
    /// Check if we can make a request and wait if necessary
    pub async fn wait_if_needed(&mut self) -> Result<()> {
        if self.requests_remaining <= 100 { // Conservative buffer
            let wait_time = self.reset_time.saturating_duration_since(Instant::now());
            if !wait_time.is_zero() {
                warn!("Rate limit low ({}), waiting {:?}", self.requests_remaining, wait_time);
                sleep(wait_time).await;
                self.requests_remaining = 5000; // Reset
                self.reset_time = Instant::now() + Duration::from_secs(3600);
            }
        }
        
        // Add exponential backoff delay
        if self.delay_factor > 1.0 {
            let delay = Duration::from_millis((1000.0 * self.delay_factor) as u64);
            debug!("Applying exponential backoff: {:?}", delay);
            sleep(delay).await;
        }
        
        Ok(())
    }
    
    /// Update rate limit info from GitHub response headers
    pub fn update_from_response(&mut self, remaining: Option<i32>, reset_timestamp: Option<i64>) {
        if let Some(remaining) = remaining {
            self.requests_remaining = remaining;
        }
        
        if let Some(reset_ts) = reset_timestamp {
            let reset_duration = Duration::from_secs((reset_ts - chrono::Utc::now().timestamp()) as u64);
            self.reset_time = Instant::now() + reset_duration;
        }
        
        // Adjust delay factor based on remaining requests
        self.delay_factor = match self.requests_remaining {
            r if r > 1000 => 1.0,
            r if r > 500 => 1.5,
            r if r > 100 => 2.0,
            _ => 3.0,
        };
    }
}

impl DanglingCommitFetcher {
    /// Create a new fetcher with GitHub token
    pub async fn new(github_token: &str, redis_url: Option<&str>) -> Result<Self> {
        info!("Initializing dangling commit fetcher");
        
        let github = Octocrab::builder()
            .personal_token(github_token.to_string())
            .build()
            .map_err(|e| anyhow!("Failed to create GitHub client: {}", e))?;
        
        let redis = if let Some(url) = redis_url {
            Some(RedisClient::open(url)
                .map_err(|e| anyhow!("Failed to connect to Redis: {}", e))?)
        } else {
            None
        };
        
        Ok(Self {
            github,
            redis,
            rate_limiter: RateLimiter::default(),
        })
    }

    /// Fetch a single commit from GitHub
    pub async fn fetch_commit(
        &mut self,
        repository: &str,
        commit_sha: &str,
    ) -> Result<Option<CommitInfo>> {
        // Check cache first
        if let Some(cached) = self.get_cached_commit(repository, commit_sha).await? {
            debug!("Retrieved commit {} from cache", commit_sha);
            return Ok(Some(cached));
        }

        // Wait for rate limit if needed
        self.rate_limiter.wait_if_needed().await?;

        let parts: Vec<&str> = repository.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid repository format: {}", repository));
        }
        let (owner, repo) = (parts[0], parts[1]);

        info!("Fetching commit {}/{}/{}", owner, repo, commit_sha);

        match self.github.repos(owner, repo).commits(commit_sha).get().await {
            Ok(commit) => {
                // Update rate limit info (if available in response)
                // Note: octocrab doesn't expose rate limit headers directly,
                // so we'll implement a conservative approach
                self.rate_limiter.requests_remaining -= 1;

                let commit_info = CommitInfo {
                    sha: commit.sha.clone(),
                    repository: repository.to_string(),
                    url: commit.url.clone(),
                    author: commit.commit.author.as_ref().map(|a| CommitAuthor {
                        name: a.name.clone().unwrap_or_default(),
                        email: a.email.clone().unwrap_or_default(),
                        date: a.date.unwrap_or_else(|| chrono::Utc::now()),
                    }),
                    committer: commit.commit.committer.as_ref().map(|c| CommitAuthor {
                        name: c.name.clone().unwrap_or_default(),
                        email: c.email.clone().unwrap_or_default(),
                        date: c.date.unwrap_or_else(|| chrono::Utc::now()),
                    }),
                    message: commit.commit.message.clone(),
                    tree_sha: commit.commit.tree.sha.clone(),
                    parents: commit.parents.iter().map(|p| p.sha.clone()).collect(),
                    stats: commit.stats.as_ref().map(|s| CommitStats {
                        additions: s.additions as u32,
                        deletions: s.deletions as u32,
                        total: s.total as u32,
                    }),
                    files: commit.files.iter().map(|f| CommitFile {
                        filename: f.filename.clone(),
                        status: f.status.clone(),
                        additions: f.additions as u32,
                        deletions: f.deletions as u32,
                        changes: f.changes as u32,
                        patch: f.patch.clone(),
                        raw_url: f.raw_url.clone(),
                        blob_url: f.blob_url.clone(),
                    }).collect(),
                    html_url: commit.html_url.clone(),
                    fetched_at: chrono::Utc::now(),
                };

                // Cache the result
                self.cache_commit(&commit_info).await?;

                Ok(Some(commit_info))
            }
            Err(octocrab::Error::GitHub { source, .. }) => {
                match source.status_code.as_u16() {
                    404 => {
                        debug!("Commit not found: {}/{}/{}", owner, repo, commit_sha);
                        Ok(None)
                    }
                    403 => {
                        warn!("Rate limited or forbidden: {}/{}/{}", owner, repo, commit_sha);
                        // Apply exponential backoff
                        self.rate_limiter.delay_factor *= 2.0;
                        self.rate_limiter.requests_remaining = 0;
                        Err(anyhow!("GitHub API rate limited or forbidden"))
                    }
                    429 => {
                        warn!("Rate limited: {}/{}/{}", owner, repo, commit_sha);
                        self.rate_limiter.requests_remaining = 0;
                        Err(anyhow!("GitHub API rate limited"))
                    }
                    _ => {
                        error!("GitHub API error {}: {}/{}/{}", source.status_code, owner, repo, commit_sha);
                        Err(anyhow!("GitHub API error: {}", source.status_code))
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch commit {}/{}/{}: {}", owner, repo, commit_sha, e);
                Err(anyhow!("Failed to fetch commit: {}", e))
            }
        }
    }

    /// Fetch multiple commits with batching and error handling
    pub async fn fetch_commits_batch(
        &mut self,
        repository: &str,
        commit_shas: &[String],
        max_concurrent: usize,
    ) -> Result<Vec<CommitInfo>> {
        info!("Fetching {} commits from {}", commit_shas.len(), repository);
        
        let mut results = Vec::new();
        let mut errors = 0;
        
        for chunk in commit_shas.chunks(max_concurrent) {
            let mut tasks = Vec::new();
            
            for sha in chunk {
                let repo = repository.to_string();
                let commit_sha = sha.clone();
                
                // Clone self for async task (note: this is a simplified approach)
                // In practice, you'd want to use Arc<Mutex<Self>> or similar
                match self.fetch_commit(&repo, &commit_sha).await {
                    Ok(Some(commit)) => {
                        results.push(commit);
                    }
                    Ok(None) => {
                        debug!("Commit not found: {}/{}", repository, commit_sha);
                    }
                    Err(e) => {
                        error!("Failed to fetch commit {}/{}: {}", repository, commit_sha, e);
                        errors += 1;
                        
                        // If too many errors, stop
                        if errors > chunk.len() / 2 {
                            return Err(anyhow!("Too many errors fetching commits"));
                        }
                    }
                }
                
                // Small delay between requests
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        
        info!("Successfully fetched {} commits, {} errors", results.len(), errors);
        Ok(results)
    }

    /// Get commit from cache
    async fn get_cached_commit(
        &self,
        repository: &str,
        commit_sha: &str,
    ) -> Result<Option<CommitInfo>> {
        if let Some(redis_client) = &self.redis {
            let mut conn = redis_client.get_connection()
                .map_err(|e| anyhow!("Redis connection failed: {}", e))?;
            
            let key = format!("commit:{}:{}", repository, commit_sha);
            let cached: Option<String> = conn.get(&key)
                .map_err(|e| anyhow!("Redis get failed: {}", e))?;
            
            if let Some(json) = cached {
                match serde_json::from_str::<CommitInfo>(&json) {
                    Ok(commit) => return Ok(Some(commit)),
                    Err(e) => {
                        warn!("Failed to deserialize cached commit: {}", e);
                        // Remove invalid cache entry
                        let _: () = conn.del(&key).unwrap_or(());
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Cache commit information
    async fn cache_commit(&self, commit: &CommitInfo) -> Result<()> {
        if let Some(redis_client) = &self.redis {
            let mut conn = redis_client.get_connection()
                .map_err(|e| anyhow!("Redis connection failed: {}", e))?;
            
            let key = format!("commit:{}:{}", commit.repository, commit.sha);
            let json = serde_json::to_string(commit)
                .map_err(|e| anyhow!("Failed to serialize commit: {}", e))?;
            
            // Cache for 24 hours
            let _: () = conn.set_ex(&key, json, 86400)
                .map_err(|e| anyhow!("Redis set failed: {}", e))?;
        }
        
        Ok(())
    }

    /// Check if a commit exists without fetching full data
    pub async fn commit_exists(
        &mut self,
        repository: &str,
        commit_sha: &str,
    ) -> Result<bool> {
        let parts: Vec<&str> = repository.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid repository format: {}", repository));
        }
        let (owner, repo) = (parts[0], parts[1]);

        self.rate_limiter.wait_if_needed().await?;

        match self.github.repos(owner, repo).commits(commit_sha).get().await {
            Ok(_) => {
                self.rate_limiter.requests_remaining -= 1;
                Ok(true)
            }
            Err(octocrab::Error::GitHub { source, .. }) => {
                match source.status_code.as_u16() {
                    404 => Ok(false),
                    403 | 429 => {
                        self.rate_limiter.requests_remaining = 0;
                        Err(anyhow!("GitHub API rate limited"))
                    }
                    _ => Err(anyhow!("GitHub API error: {}", source.status_code))
                }
            }
            Err(e) => Err(anyhow!("Failed to check commit existence: {}", e))
        }
    }

    /// Get current rate limit status
    pub fn get_rate_limit_status(&self) -> (i32, Duration) {
        let remaining_time = self.rate_limiter.reset_time.saturating_duration_since(Instant::now());
        (self.rate_limiter.requests_remaining, remaining_time)
    }

    /// Attempt to brute force partial commit hashes
    pub async fn brute_force_partial_hash(
        &mut self,
        repository: &str,
        partial_hash: &str,
    ) -> Result<Vec<String>> {
        if partial_hash.len() < 4 || partial_hash.len() >= 40 {
            return Err(anyhow!("Partial hash must be 4-39 characters long"));
        }

        info!("Brute forcing partial hash {} in {}", partial_hash, repository);
        
        let mut found_hashes = Vec::new();
        let hex_chars = "0123456789abcdef";
        
        // For practical reasons, only brute force up to 7-8 character hashes
        if partial_hash.len() > 8 {
            return Err(anyhow!("Partial hash too long for brute force"));
        }
        
        let missing_chars = 40 - partial_hash.len();
        let max_combinations = 16_u64.pow(missing_chars as u32);
        
        if max_combinations > 1_000_000 {
            return Err(anyhow!("Too many combinations to brute force"));
        }
        
        for i in 0..max_combinations {
            let mut full_hash = partial_hash.to_string();
            let mut remaining = i;
            
            for _ in 0..missing_chars {
                let char_index = (remaining % 16) as usize;
                full_hash.push(hex_chars.chars().nth(char_index).unwrap());
                remaining /= 16;
            }
            
            if self.commit_exists(repository, &full_hash).await? {
                found_hashes.push(full_hash);
                info!("Found matching commit: {}", found_hashes.last().unwrap());
            }
            
            // Rate limiting
            if i % 10 == 0 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        
        info!("Brute force completed, found {} matches", found_hashes.len());
        Ok(found_hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_default() {
        let limiter = RateLimiter::default();
        assert_eq!(limiter.requests_remaining, 5000);
        assert_eq!(limiter.delay_factor, 1.0);
    }

    #[test]
    fn test_rate_limiter_delay_factor() {
        let mut limiter = RateLimiter::default();
        
        limiter.update_from_response(Some(1500), None);
        assert_eq!(limiter.delay_factor, 1.0);
        
        limiter.update_from_response(Some(800), None);
        assert_eq!(limiter.delay_factor, 1.5);
        
        limiter.update_from_response(Some(300), None);
        assert_eq!(limiter.delay_factor, 2.0);
        
        limiter.update_from_response(Some(50), None);
        assert_eq!(limiter.delay_factor, 3.0);
    }

    #[tokio::test]
    async fn test_commit_info_serialization() {
        let commit = CommitInfo {
            sha: "abc123".to_string(),
            repository: "owner/repo".to_string(),
            url: "https://api.github.com/repos/owner/repo/commits/abc123".to_string(),
            author: Some(CommitAuthor {
                name: "Test User".to_string(),
                email: "test@example.com".to_string(),
                date: chrono::Utc::now(),
            }),
            committer: None,
            message: "Test commit".to_string(),
            tree_sha: "def456".to_string(),
            parents: vec!["parent1".to_string()],
            stats: Some(CommitStats {
                additions: 10,
                deletions: 5,
                total: 15,
            }),
            files: vec![],
            html_url: "https://github.com/owner/repo/commit/abc123".to_string(),
            fetched_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&commit).unwrap();
        let deserialized: CommitInfo = serde_json::from_str(&json).unwrap();
        
        assert_eq!(commit.sha, deserialized.sha);
        assert_eq!(commit.repository, deserialized.repository);
        assert_eq!(commit.message, deserialized.message);
    }
}
