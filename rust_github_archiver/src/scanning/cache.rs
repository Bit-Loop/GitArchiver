use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Maximum repo size (in bytes) we will clone into the cache.
const MAX_REPO_BYTES: u64 = 1_000_000_000; // 1GB
/// Default slot we request when sizing eviction (acts as a pre-alloc hint).
const DEFAULT_RESERVATION: u64 = 256 * 1024 * 1024; // 256MB
const DEFAULT_COOLDOWN_SECS: i64 = 15 * 60;
const DEFAULT_CACHE_RETENTION_HOURS: i64 = 24;
const DEFAULT_MAX_CACHE_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10GB ceiling

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    path: PathBuf,
    size_bytes: u64,
    last_accessed: DateTime<Utc>,
    cooldown_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheMetadata {
    entries: HashMap<String, CacheEntry>,
}

struct CacheState {
    metadata: CacheMetadata,
}

#[derive(Debug, Clone, Copy)]
struct CachePolicy {
    max_cache_bytes: u64,
    retention_hours: i64,
    cooldown_secs: i64,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            max_cache_bytes: std::env::var("REPO_CACHE_MAX_BYTES")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_MAX_CACHE_BYTES),
            retention_hours: std::env::var("REPO_CACHE_RETENTION_HOURS")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_CACHE_RETENTION_HOURS),
            cooldown_secs: std::env::var("REPO_CACHE_COOLDOWN_SECS")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_COOLDOWN_SECS),
        }
    }
}

pub struct CacheManager {
    root: PathBuf,
    meta_path: PathBuf,
    state: Mutex<CacheState>,
    policy: CachePolicy,
}

static GLOBAL_CACHE: OnceCell<CacheManager> = OnceCell::new();

impl CacheManager {
    pub fn global() -> &'static CacheManager {
        GLOBAL_CACHE.get_or_init(|| Self::from_root(Self::select_cache_root()))
    }

    pub async fn allocate_repo(&self, repo_id: &str, estimated_bytes: u64) -> Result<PathBuf> {
        let mut state = self.state.lock().await;
        let removed = self.prune_retained_locked(&mut state);
        if removed > 0 {
            self.persist_locked(&state)?;
        }

        if self.is_on_cooldown_locked(repo_id, &state) {
            return Err(anyhow!(
                "Repository {} is on cooldown due to recent failures",
                repo_id
            ));
        }

        self.evict_if_needed_locked(estimated_bytes.max(DEFAULT_RESERVATION), &mut state)
            .await?;

        let repo_dir = self.root.join(Self::safe_dirname(repo_id));
        if repo_dir.exists() {
            fs::remove_dir_all(&repo_dir).ok();
        }
        fs::create_dir_all(&repo_dir)
            .with_context(|| format!("Failed to create cache dir {:?}", repo_dir))?;

        state.metadata.entries.insert(
            repo_id.to_string(),
            CacheEntry {
                path: repo_dir.clone(),
                size_bytes: 0,
                last_accessed: Utc::now(),
                cooldown_until: None,
            },
        );
        self.persist_locked(&state)?;

        Ok(repo_dir)
    }

    pub async fn finalize_success(&self, repo_id: &str, path: &Path) -> Result<()> {
        let size = Self::dir_size(path)?;
        let mut state = self.state.lock().await;
        let removed = self.prune_retained_locked(&mut state);
        if removed > 0 {
            self.persist_locked(&state)?;
        }
        if let Some(entry) = state.metadata.entries.get_mut(repo_id) {
            entry.size_bytes = size;
            entry.last_accessed = Utc::now();
            entry.cooldown_until = None;
        }

        if size > MAX_REPO_BYTES {
            drop(state);
            self.remove_entry(repo_id).await?;
            return Err(anyhow!(
                "Repository {} exceeds size limit ({} bytes)",
                repo_id,
                size
            ));
        }

        self.persist_locked(&state)?;
        Ok(())
    }

    pub async fn remove_entry(&self, repo_id: &str) -> Result<()> {
        let mut state = self.state.lock().await;
        if let Some(entry) = state.metadata.entries.remove(repo_id) {
            if entry.path.exists() {
                fs::remove_dir_all(&entry.path).ok();
            }
        }
        self.persist_locked(&state)?;
        Ok(())
    }

    pub async fn mark_cooldown(&self, repo_id: &str) -> Result<()> {
        let until = Utc::now() + chrono::Duration::seconds(self.policy.cooldown_secs);
        let mut state = self.state.lock().await;
        state
            .metadata
            .entries
            .entry(repo_id.to_string())
            .and_modify(|e| e.cooldown_until = Some(until))
            .or_insert(CacheEntry {
                path: self.root.join(Self::safe_dirname(repo_id)),
                size_bytes: 0,
                last_accessed: Utc::now(),
                cooldown_until: Some(until),
            });
        self.persist_locked(&state)?;
        Ok(())
    }

    pub fn is_on_cooldown_sync(&self, repo_id: &str) -> bool {
        if let Ok(state) = self.state.try_lock() {
            return self.is_on_cooldown_locked(repo_id, &state);
        }
        false
    }

    pub async fn enforce_retention(&self) -> Result<usize> {
        let mut state = self.state.lock().await;
        let removed = self.prune_retained_locked(&mut state);
        if removed > 0 {
            self.persist_locked(&state)?;
        }
        Ok(removed)
    }

    pub async fn clear_all(&self) -> Result<usize> {
        let mut state = self.state.lock().await;
        let removed = state.metadata.entries.len();
        for entry in state.metadata.entries.values() {
            if entry.path.exists() {
                fs::remove_dir_all(&entry.path).ok();
            }
        }
        state.metadata.entries.clear();
        self.persist_locked(&state)?;
        Ok(removed)
    }

    fn is_on_cooldown_locked(&self, repo_id: &str, state: &CacheState) -> bool {
        state
            .metadata
            .entries
            .get(repo_id)
            .and_then(|e| e.cooldown_until)
            .map(|until| until > Utc::now())
            .unwrap_or(false)
    }

    async fn evict_if_needed_locked(
        &self,
        needed_bytes: u64,
        state: &mut CacheState,
    ) -> Result<()> {
        let (budget, floor) = self.memory_budget();
        let mut current_usage: u64 = state.metadata.entries.values().map(|e| e.size_bytes).sum();

        if needed_bytes > budget {
            return Err(anyhow!(
                "Insufficient memory budget for cloning (requested {} bytes, budget {} bytes)",
                needed_bytes,
                budget
            ));
        }

        // Evict while usage + needed exceeds budget or free memory would dip below floor.
        loop {
            let free_mem = sys_info::mem_info()
                .map(|m| (m.free + m.buffers + m.cached) * 1024)
                .unwrap_or(0);
            let would_use = current_usage + needed_bytes;
            if would_use <= budget && free_mem > floor {
                break;
            }

            let Some((oldest_key, oldest_entry)) = state
                .metadata
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_accessed)
                .map(|(k, v)| (k.clone(), v.clone()))
            else {
                break;
            };

            debug!(
                "Evicting cached repo {} ({} bytes) to honor budget",
                oldest_key, oldest_entry.size_bytes
            );
            if oldest_entry.path.exists() {
                fs::remove_dir_all(&oldest_entry.path).ok();
            }
            current_usage = current_usage.saturating_sub(oldest_entry.size_bytes);
            state.metadata.entries.remove(&oldest_key);
        }

        self.persist_locked(state)?;
        Ok(())
    }

    fn persist_locked(&self, state: &CacheState) -> Result<()> {
        Self::persist_metadata(&self.meta_path, &state.metadata)
    }

    fn load_metadata(path: &Path) -> Result<CacheMetadata> {
        if !path.exists() {
            return Ok(CacheMetadata::default());
        }
        let data = fs::read(path)?;
        Ok(serde_json::from_slice(&data)?)
    }

    fn memory_budget(&self) -> (u64, u64) {
        let info = sys_info::mem_info().unwrap_or(sys_info::MemInfo {
            total: 0,
            free: 0,
            avail: 0,
            buffers: 0,
            cached: 0,
            swap_total: 0,
            swap_free: 0,
        });
        let total = info.total * 1024;
        let free = (info.free + info.buffers + info.cached) * 1024;
        let mut budget = (free as f64 * 0.20) as u64;
        if budget > self.policy.max_cache_bytes {
            budget = self.policy.max_cache_bytes;
        }
        let floor = (total as f64 * 0.05) as u64;
        (budget, floor)
    }

    fn from_root(root: PathBuf) -> Self {
        if let Err(error) = fs::create_dir_all(&root) {
            warn!("Failed to create cache root {:?}: {}", root, error);
        }

        let meta_path = root.join("cache_metadata.json");
        let policy = CachePolicy::default();
        let mut metadata = Self::load_metadata(&meta_path).unwrap_or_default();
        if Self::prune_metadata_for_policy(&mut metadata, policy) > 0 {
            Self::persist_metadata(&meta_path, &metadata).ok();
        }

        CacheManager {
            root,
            meta_path,
            state: Mutex::new(CacheState { metadata }),
            policy,
        }
    }

    fn prune_retained_locked(&self, state: &mut CacheState) -> usize {
        Self::prune_metadata_for_policy(&mut state.metadata, self.policy)
    }

    fn prune_metadata_for_policy(metadata: &mut CacheMetadata, policy: CachePolicy) -> usize {
        let retention_cutoff = Utc::now() - chrono::Duration::hours(policy.retention_hours);
        let stale_keys: Vec<String> = metadata
            .entries
            .iter()
            .filter(|(_, entry)| {
                !entry.path.exists()
                    || entry.last_accessed < retention_cutoff
                    || entry.cooldown_until.is_some_and(|until| until < Utc::now())
                        && entry.size_bytes == 0
            })
            .map(|(repo_id, _)| repo_id.clone())
            .collect();

        for repo_id in &stale_keys {
            if let Some(entry) = metadata.entries.remove(repo_id) {
                if entry.path.exists() {
                    fs::remove_dir_all(&entry.path).ok();
                }
            }
        }

        stale_keys.len()
    }

    fn persist_metadata(meta_path: &Path, metadata: &CacheMetadata) -> Result<()> {
        let tmp_path = meta_path.with_extension(format!("tmp-{}", std::process::id()));
        let data = serde_json::to_vec_pretty(metadata)?;
        fs::write(&tmp_path, data)?;
        fs::rename(&tmp_path, meta_path)?;
        Ok(())
    }

    fn dir_size(path: &Path) -> Result<u64> {
        let mut total = 0u64;
        if !path.exists() {
            return Ok(0);
        }
        let mut stack = vec![path.to_path_buf()];
        while let Some(p) = stack.pop() {
            for entry in fs::read_dir(&p)? {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_dir() {
                    stack.push(entry.path());
                } else {
                    total = total.saturating_add(meta.len());
                }
            }
        }
        Ok(total)
    }

    fn select_cache_root() -> PathBuf {
        let candidates = [
            "/dev/shm/github_cache",
            "/run/shm/github_cache",
            "/tmp/github_cache",
        ];
        for candidate in candidates {
            let p = PathBuf::from(candidate);
            if p.parent().map(|d| d.exists()).unwrap_or(false) {
                return p;
            }
        }
        std::env::temp_dir().join("github_cache")
    }

    fn safe_dirname(repo_id: &str) -> String {
        let mut name: String = repo_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        if name.len() > 100 {
            name.truncate(100);
        }
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn clear_all_removes_cached_repositories() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let manager = CacheManager::from_root(temp_dir.path().join("repo-cache"));
        let repo_dir = manager
            .allocate_repo("owner/repo", 1024)
            .await
            .expect("cache allocation should succeed");
        fs::write(repo_dir.join("artifact.txt"), b"evidence").expect("artifact should be written");
        manager
            .finalize_success("owner/repo", &repo_dir)
            .await
            .expect("cache finalization should succeed");

        let removed = manager
            .clear_all()
            .await
            .expect("cache clear should succeed");

        assert_eq!(removed, 1);
        assert!(
            !repo_dir.exists(),
            "repo directory should be removed during cache clear"
        );
    }

    #[tokio::test]
    async fn enforce_retention_prunes_stale_entries() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let manager = CacheManager::from_root(temp_dir.path().join("repo-cache"));
        let repo_dir = manager
            .allocate_repo("owner/repo", 1024)
            .await
            .expect("cache allocation should succeed");
        fs::write(repo_dir.join("artifact.txt"), b"evidence").expect("artifact should be written");
        manager
            .finalize_success("owner/repo", &repo_dir)
            .await
            .expect("cache finalization should succeed");

        {
            let mut state = manager.state.lock().await;
            let entry = state
                .metadata
                .entries
                .get_mut("owner/repo")
                .expect("repo metadata should exist");
            entry.last_accessed =
                Utc::now() - chrono::Duration::hours(manager.policy.retention_hours + 1);
            manager
                .persist_locked(&state)
                .expect("stale metadata should persist");
        }

        let removed = manager
            .enforce_retention()
            .await
            .expect("retention cleanup should succeed");

        assert_eq!(removed, 1);
        assert!(
            !repo_dir.exists(),
            "repo directory should be removed when it exceeds the retention window"
        );
    }
}
