// API keys management system
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid; // Removed unused error

const DEFAULT_API_SECRETS_FILE: &str = ".gitarchiver/api_secrets.json";
const API_KEY_HASH_PREFIX: &str = "sha256:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key_type: ApiKeyType,
    #[serde(rename = "key_hash", alias = "key_value")]
    pub key_hash: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub last_used: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiKeyType {
    GitHub,
    AWS,
    Database,
    Webhook,
    Scanner,
    Admin,
    ReadOnly,
}

impl std::fmt::Display for ApiKeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiKeyType::GitHub => write!(f, "GitHub"),
            ApiKeyType::AWS => write!(f, "AWS"),
            ApiKeyType::Database => write!(f, "Database"),
            ApiKeyType::Webhook => write!(f, "Webhook"),
            ApiKeyType::Scanner => write!(f, "Scanner"),
            ApiKeyType::Admin => write!(f, "Admin"),
            ApiKeyType::ReadOnly => write!(f, "Read Only"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeysStorage {
    pub keys: HashMap<String, ApiKey>,
    pub version: u32,
    pub last_updated: DateTime<Utc>,
}

impl Default for ApiKeysStorage {
    fn default() -> Self {
        Self {
            keys: HashMap::new(),
            version: 1,
            last_updated: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub key_type: ApiKeyType,
    pub description: Option<String>,
    pub expires_in_days: Option<u32>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key_type: ApiKeyType,
    pub key_value: String, // Only returned on creation
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyListItem {
    pub id: String,
    pub name: String,
    pub key_type: ApiKeyType,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub permissions: Vec<String>,
}

pub struct ApiKeyManager;

impl ApiKeyManager {
    fn storage_path() -> PathBuf {
        std::env::var("GITARCHIVER_API_SECRETS_FILE")
            .or_else(|_| std::env::var("API_SECRETS_FILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_API_SECRETS_FILE))
    }

    fn hash_api_key(key_value: &str) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(key_value.as_bytes());
        format!("{API_KEY_HASH_PREFIX}{}", hex::encode(hasher.finalize()))
    }

    fn is_hashed_key(value: &str) -> bool {
        value.starts_with(API_KEY_HASH_PREFIX)
    }

    fn migrate_legacy_cleartext_keys(storage: &mut ApiKeysStorage) -> bool {
        let mut changed = false;
        for key in storage.keys.values_mut() {
            if !Self::is_hashed_key(&key.key_hash) {
                key.key_hash = Self::hash_api_key(&key.key_hash);
                changed = true;
            }
        }
        if changed {
            storage.version += 1;
            storage.last_updated = Utc::now();
        }
        changed
    }

    #[cfg(unix)]
    fn harden_file_permissions(path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        if path.exists() {
            let metadata = fs::metadata(path)
                .map_err(|e| anyhow!("Failed to read API secrets file metadata: {}", e))?;
            let mode = metadata.permissions().mode();
            if mode & 0o077 != 0 {
                warn!(
                    "API secrets file permissions were too broad; tightening to owner-only access"
                );
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                    .map_err(|e| anyhow!("Failed to harden API secrets file permissions: {}", e))?;
            }
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn harden_file_permissions(_path: &Path) -> Result<()> {
        Ok(())
    }

    /// Load API keys from storage
    pub fn load_keys() -> Result<ApiKeysStorage> {
        let path = Self::storage_path();
        if !path.exists() {
            info!("API secrets file not found, creating new storage");
            let storage = ApiKeysStorage::default();
            Self::save_keys(&storage)?;
            return Ok(storage);
        }

        Self::harden_file_permissions(&path)?;

        let content = fs::read_to_string(&path)
            .map_err(|e| anyhow!("Failed to read API secrets file: {}", e))?;

        let mut storage: ApiKeysStorage = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse API secrets file: {}", e))?;
        if Self::migrate_legacy_cleartext_keys(&mut storage) {
            Self::save_keys(&storage)?;
        }

        info!("Loaded {} API keys from storage", storage.keys.len());
        Ok(storage)
    }

    /// Save API keys to storage
    pub fn save_keys(storage: &ApiKeysStorage) -> Result<()> {
        let content = serde_json::to_string_pretty(storage)
            .map_err(|e| anyhow!("Failed to serialize API keys: {}", e))?;
        let path = Self::storage_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| anyhow!("Failed to create parent directory: {}", e))?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)
                .map_err(|e| anyhow!("Failed to write API secrets file: {}", e))?;
            file.write_all(content.as_bytes())
                .map_err(|e| anyhow!("Failed to write API secrets file: {}", e))?;
        }

        #[cfg(not(unix))]
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .map_err(|e| anyhow!("Failed to write API secrets file: {}", e))?;
            file.write_all(content.as_bytes())
                .map_err(|e| anyhow!("Failed to write API secrets file: {}", e))?;
        }

        Self::harden_file_permissions(&path)?;

        info!("Saved {} API keys to storage", storage.keys.len());
        Ok(())
    }

    /// Generate a new API key
    pub fn generate_api_key(key_type: &ApiKeyType) -> String {
        let prefix = match key_type {
            ApiKeyType::GitHub => "gha",
            ApiKeyType::AWS => "aws",
            ApiKeyType::Database => "db",
            ApiKeyType::Webhook => "wh",
            ApiKeyType::Scanner => "sc",
            ApiKeyType::Admin => "adm",
            ApiKeyType::ReadOnly => "ro",
        };

        let random_part = format!(
            "{}{}",
            Uuid::new_v4().to_string().replace('-', ""),
            Uuid::new_v4().to_string().replace('-', "")
        );
        format!("{}_{}_{}", prefix, &random_part[..32], &random_part[32..64])
    }

    /// Create a new API key
    pub fn create_api_key(
        request: CreateApiKeyRequest,
        created_by: String,
    ) -> Result<ApiKeyResponse> {
        let mut storage = Self::load_keys()?;

        let key_id = Uuid::new_v4().to_string();
        let key_value = Self::generate_api_key(&request.key_type);
        let key_hash = Self::hash_api_key(&key_value);

        let expires_at = request
            .expires_in_days
            .map(|days| Utc::now() + chrono::Duration::days(days as i64));

        let api_key = ApiKey {
            id: key_id.clone(),
            name: request.name.clone(),
            key_type: request.key_type.clone(),
            key_hash,
            description: request.description.clone(),
            created_at: Utc::now(),
            created_by: created_by.clone(),
            last_used: None,
            expires_at,
            is_active: true,
            permissions: request.permissions.clone(),
        };

        storage.keys.insert(key_id.clone(), api_key);
        storage.last_updated = Utc::now();
        storage.version += 1;

        Self::save_keys(&storage)?;

        info!(
            "Created new API key '{}' for user '{}'",
            request.name, created_by
        );

        Ok(ApiKeyResponse {
            id: key_id,
            name: request.name,
            key_type: request.key_type,
            key_value, // Only returned on creation
            description: request.description,
            created_at: Utc::now(),
            expires_at,
            is_active: true,
            permissions: request.permissions,
        })
    }

    /// List all API keys (without revealing the actual key values)
    pub fn list_api_keys() -> Result<Vec<ApiKeyListItem>> {
        let storage = Self::load_keys()?;

        let keys: Vec<ApiKeyListItem> = storage
            .keys
            .values()
            .map(|key| ApiKeyListItem {
                id: key.id.clone(),
                name: key.name.clone(),
                key_type: key.key_type.clone(),
                description: key.description.clone(),
                created_at: key.created_at,
                last_used: key.last_used,
                expires_at: key.expires_at,
                is_active: key.is_active,
                permissions: key.permissions.clone(),
            })
            .collect();

        Ok(keys)
    }

    /// Get API key by ID (without revealing the actual key value)
    pub fn get_api_key(key_id: &str) -> Result<Option<ApiKeyListItem>> {
        let storage = Self::load_keys()?;

        if let Some(key) = storage.keys.get(key_id) {
            Ok(Some(ApiKeyListItem {
                id: key.id.clone(),
                name: key.name.clone(),
                key_type: key.key_type.clone(),
                description: key.description.clone(),
                created_at: key.created_at,
                last_used: key.last_used,
                expires_at: key.expires_at,
                is_active: key.is_active,
                permissions: key.permissions.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Validate an API key and return its information
    pub fn validate_api_key(key_value: &str) -> Result<Option<ApiKey>> {
        let storage = Self::load_keys()?;
        let candidate_hash = Self::hash_api_key(key_value);

        for key in storage.keys.values() {
            if key.key_hash == candidate_hash && key.is_active {
                // Check if key is expired
                if let Some(expires_at) = key.expires_at {
                    if Utc::now() > expires_at {
                        warn!("API key '{}' is expired", key.name);
                        return Ok(None);
                    }
                }
                return Ok(Some(key.clone()));
            }
        }

        Ok(None)
    }

    /// Update last used timestamp for an API key
    pub fn update_last_used(key_value: &str) -> Result<()> {
        let mut storage = Self::load_keys()?;
        let candidate_hash = Self::hash_api_key(key_value);

        for key in storage.keys.values_mut() {
            if key.key_hash == candidate_hash {
                key.last_used = Some(Utc::now());
                storage.last_updated = Utc::now();
                Self::save_keys(&storage)?;
                break;
            }
        }

        Ok(())
    }

    /// Deactivate an API key
    pub fn deactivate_api_key(key_id: &str) -> Result<bool> {
        let mut storage = Self::load_keys()?;

        if let Some(key) = storage.keys.get_mut(key_id) {
            let key_name = key.name.clone(); // Clone the name before moving
            key.is_active = false;
            storage.last_updated = Utc::now();
            storage.version += 1;
            Self::save_keys(&storage)?;

            info!("Deactivated API key '{}'", key_name);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete an API key
    pub fn delete_api_key(key_id: &str) -> Result<bool> {
        let mut storage = Self::load_keys()?;

        if let Some(key) = storage.keys.remove(key_id) {
            storage.last_updated = Utc::now();
            storage.version += 1;
            Self::save_keys(&storage)?;

            info!("Deleted API key '{}'", key.name);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Regenerate an API key
    pub fn regenerate_api_key(key_id: &str) -> Result<Option<String>> {
        let mut storage = Self::load_keys()?;

        if let Some(key) = storage.keys.get_mut(key_id) {
            let new_key_value = Self::generate_api_key(&key.key_type);
            let key_name = key.name.clone(); // Clone the name before moving
            key.key_hash = Self::hash_api_key(&new_key_value);
            key.last_used = None;
            storage.last_updated = Utc::now();
            storage.version += 1;
            Self::save_keys(&storage)?;

            info!("Regenerated API key '{}'", key_name);
            Ok(Some(new_key_value))
        } else {
            Ok(None)
        }
    }

    /// Get API key statistics
    pub fn get_statistics() -> Result<serde_json::Value> {
        let storage = Self::load_keys()?;

        let total_keys = storage.keys.len();
        let active_keys = storage.keys.values().filter(|k| k.is_active).count();
        let expired_keys = storage
            .keys
            .values()
            .filter(|k| {
                if let Some(expires_at) = k.expires_at {
                    Utc::now() > expires_at
                } else {
                    false
                }
            })
            .count();

        let mut by_type: HashMap<String, usize> = HashMap::new();
        for key in storage.keys.values() {
            *by_type.entry(key.key_type.to_string()).or_insert(0) += 1;
        }

        Ok(serde_json::json!({
            "total_keys": total_keys,
            "active_keys": active_keys,
            "expired_keys": expired_keys,
            "inactive_keys": total_keys - active_keys,
            "by_type": by_type,
            "last_updated": storage.last_updated,
            "version": storage.version
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    #[test]
    fn create_api_key_stores_hash_not_cleartext() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("api_secrets.json");
        std::env::set_var("GITARCHIVER_API_SECRETS_FILE", &path);

        let response = ApiKeyManager::create_api_key(
            CreateApiKeyRequest {
                name: "scanner".to_string(),
                key_type: ApiKeyType::Scanner,
                description: None,
                expires_in_days: None,
                permissions: vec!["scan".to_string()],
            },
            "admin".to_string(),
        )
        .expect("api key");

        let stored = fs::read_to_string(&path).expect("stored file");
        assert!(stored.contains("\"key_hash\""));
        assert!(!stored.contains("\"key_value\""));
        assert!(!stored.contains(&response.key_value));

        let valid = ApiKeyManager::validate_api_key(&response.key_value).expect("validate");
        assert!(valid.is_some());
        std::env::remove_var("GITARCHIVER_API_SECRETS_FILE");
    }

    #[test]
    fn legacy_cleartext_key_value_is_migrated_to_hash() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("api_secrets.json");
        std::env::set_var("GITARCHIVER_API_SECRETS_FILE", &path);

        let legacy = serde_json::json!({
            "keys": {
                "legacy-id": {
                    "id": "legacy-id",
                    "name": "legacy",
                    "key_type": "Scanner",
                    "key_value": "legacy-secret-value",
                    "description": null,
                    "created_at": Utc::now(),
                    "created_by": "admin",
                    "last_used": null,
                    "expires_at": null,
                    "is_active": true,
                    "permissions": ["scan"]
                }
            },
            "version": 1,
            "last_updated": Utc::now()
        });
        fs::write(&path, serde_json::to_string_pretty(&legacy).expect("json")).expect("write");

        let storage = ApiKeyManager::load_keys().expect("load legacy");

        let migrated = storage.keys.get("legacy-id").expect("legacy key");
        assert!(migrated.key_hash.starts_with(API_KEY_HASH_PREFIX));
        let stored = fs::read_to_string(&path).expect("stored file");
        assert!(!stored.contains("legacy-secret-value"));
        std::env::remove_var("GITARCHIVER_API_SECRETS_FILE");
    }
}
