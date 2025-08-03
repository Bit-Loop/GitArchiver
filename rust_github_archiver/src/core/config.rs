use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use tracing::{error, info, warn};

/// Database configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub password: String,
    pub min_connections: u32,
    pub max_connections: u32,
    pub command_timeout: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            host: env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("DB_PORT")
                .unwrap_or_else(|_| "5432".to_string())
                .parse()
                .unwrap_or(5432),
            name: env::var("DB_NAME").unwrap_or_else(|_| "gharchive".to_string()),
            user: env::var("DB_USER").unwrap_or_else(|_| "gharchive".to_string()),
            password: env::var("DB_PASSWORD").unwrap_or_else(|_| "gharchive_password".to_string()),
            min_connections: env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
            max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .unwrap_or(20),
            command_timeout: env::var("DB_COMMAND_TIMEOUT")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
        }
    }
}

impl DatabaseConfig {
    /// Get PostgreSQL connection string
    pub fn connection_string(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.name
        )
    }
}

/// GitHub API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub token: String,
    pub username: String,
    pub api_base_url: String,
    pub unauthenticated_rate_limit: u32,
    pub authenticated_rate_limit: u32,
    pub rate_limit_buffer: u32,
    pub rate_limit_reset_buffer: u32,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            token: env::var("GITHUB_TOKEN").unwrap_or_default(),
            username: env::var("GITHUB_USERNAME").unwrap_or_default(),
            api_base_url: "https://api.github.com".to_string(),
            unauthenticated_rate_limit: 60,
            authenticated_rate_limit: 5000,
            rate_limit_buffer: 5,
            rate_limit_reset_buffer: 300,
        }
    }
}

impl GitHubConfig {
    /// Check if GitHub token is configured
    pub fn is_authenticated(&self) -> bool {
        !self.token.is_empty()
    }

    /// Get effective rate limit based on authentication
    pub fn effective_rate_limit(&self) -> u32 {
        if self.is_authenticated() {
            self.authenticated_rate_limit
        } else {
            self.unauthenticated_rate_limit
        }
    }
}

/// Download and processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub base_url: String,
    pub s3_list_url: String,
    pub download_dir: PathBuf,
    pub max_concurrent_downloads: u32,
    pub chunk_size: usize,
    pub request_timeout: u64,
    pub max_retries: u32,
    pub retry_delay: f64,
    pub batch_size: u32,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        let download_dir = PathBuf::from(
            env::var("DOWNLOAD_DIR").unwrap_or_else(|_| "./gharchive_data".to_string()),
        );

        // Create download directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&download_dir) {
            warn!("Failed to create download directory: {}", e);
        }

        Self {
            base_url: "https://data.gharchive.org/".to_string(),
            s3_list_url: "https://data.gharchive.org/?list-type=2".to_string(),
            download_dir,
            max_concurrent_downloads: env::var("MAX_CONCURRENT")
                .unwrap_or_else(|_| "6".to_string())
                .parse()
                .unwrap_or(6),
            chunk_size: env::var("CHUNK_SIZE")
                .unwrap_or_else(|_| "4096".to_string())
                .parse()
                .unwrap_or(4096),
            request_timeout: env::var("REQUEST_TIMEOUT")
                .unwrap_or_else(|_| "180".to_string())
                .parse()
                .unwrap_or(180),
            max_retries: env::var("MAX_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3),
            retry_delay: env::var("RETRY_DELAY")
                .unwrap_or_else(|_| "2.0".to_string())
                .parse()
                .unwrap_or(2.0),
            batch_size: env::var("BATCH_SIZE")
                .unwrap_or_else(|_| "500".to_string())
                .parse()
                .unwrap_or(500),
        }
    }
}

/// Resource monitoring and limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub memory_limit_gb: f64,
    pub disk_limit_gb: f64,
    pub cpu_limit_percent: f64,
    pub memory_warning_threshold: f64,
    pub disk_warning_threshold: f64,
    pub cpu_warning_threshold: f64,
    pub emergency_cleanup_threshold: f64,
    pub monitoring_interval_seconds: u64,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            memory_limit_gb: env::var("MEMORY_LIMIT_GB")
                .unwrap_or_else(|_| "18.0".to_string())
                .parse()
                .unwrap_or(18.0),
            disk_limit_gb: env::var("DISK_LIMIT_GB")
                .unwrap_or_else(|_| "40.0".to_string())
                .parse()
                .unwrap_or(40.0),
            cpu_limit_percent: env::var("CPU_LIMIT_PERCENT")
                .unwrap_or_else(|_| "80.0".to_string())
                .parse()
                .unwrap_or(80.0),
            memory_warning_threshold: 0.8,
            disk_warning_threshold: 0.8,
            cpu_warning_threshold: 0.7,
            emergency_cleanup_threshold: 0.9,
            monitoring_interval_seconds: 30,
        }
    }
}

impl ResourceConfig {
    /// Get memory limit in bytes
    pub fn memory_limit_bytes(&self) -> u64 {
        (self.memory_limit_gb * 1024.0 * 1024.0 * 1024.0) as u64
    }

    /// Get disk limit in bytes
    pub fn disk_limit_bytes(&self) -> u64 {
        (self.disk_limit_gb * 1024.0 * 1024.0 * 1024.0) as u64
    }

    /// Get memory warning threshold in bytes
    pub fn memory_warning_bytes(&self) -> u64 {
        (self.memory_limit_bytes() as f64 * self.memory_warning_threshold) as u64
    }

    /// Get disk warning threshold in bytes
    pub fn disk_warning_bytes(&self) -> u64 {
        (self.disk_limit_bytes() as f64 * self.disk_warning_threshold) as u64
    }

    /// Get CPU warning threshold in percent
    pub fn cpu_warning_percent(&self) -> f64 {
        self.cpu_limit_percent * self.cpu_warning_threshold
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub log_dir: PathBuf,
    pub main_log_file: String,
    pub api_log_file: String,
    pub audit_log_file: String,
    pub max_file_size_mb: u64,
    pub backup_count: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        let log_dir = PathBuf::from(env::var("LOG_DIR").unwrap_or_else(|_| "./logs".to_string()));

        // Create log directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            warn!("Failed to create log directory: {}", e);
        }

        Self {
            level: env::var("LOG_LEVEL").unwrap_or_else(|_| "INFO".to_string()),
            log_dir,
            main_log_file: "scraper.log".to_string(),
            api_log_file: "api.log".to_string(),
            audit_log_file: "audit.log".to_string(),
            max_file_size_mb: 50,
            backup_count: 5,
        }
    }
}

impl LoggingConfig {
    /// Get main log file path
    pub fn main_log_path(&self) -> PathBuf {
        self.log_dir.join(&self.main_log_file)
    }

    /// Get API log file path
    pub fn api_log_path(&self) -> PathBuf {
        self.log_dir.join(&self.api_log_file)
    }

    /// Get audit log file path
    pub fn audit_log_path(&self) -> PathBuf {
        self.log_dir.join(&self.audit_log_file)
    }
}

/// Web API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub host: String,
    pub port: u16,
    pub debug: bool,
    pub cors_origins: Vec<String>,
    pub max_request_size: usize,
    pub request_timeout: u64,
}

impl Default for WebConfig {
    fn default() -> Self {
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "*".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        Self {
            host: env::var("WEB_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("WEB_PORT")
                .unwrap_or_else(|_| "8081".to_string())
                .parse()
                .unwrap_or(8081),
            debug: env::var("WEB_DEBUG")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
            cors_origins,
            max_request_size: env::var("MAX_REQUEST_SIZE")
                .unwrap_or_else(|_| "16777216".to_string())
                .parse()
                .unwrap_or(16777216),
            request_timeout: env::var("WEB_REQUEST_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
        }
    }
}

impl WebConfig {
    /// Get base URL for the web interface
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub admin_password: String,
    pub secret_key: String,
    pub jwt_secret: String,
    pub session_duration_hours: u64,
    pub max_failed_attempts: u32,
    pub lockout_duration_minutes: u64,
    pub require_2fa: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        use uuid::Uuid;

        Self {
            admin_password: env::var("ADMIN_PASSWORD")
                .unwrap_or_else(|_| "admin123".to_string()),
            secret_key: env::var("SECRET_KEY")
                .unwrap_or_else(|_| Uuid::new_v4().to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "github-archive-scraper-jwt-secret-key".to_string()),
            session_duration_hours: env::var("SESSION_DURATION_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .unwrap_or(24),
            max_failed_attempts: env::var("MAX_FAILED_ATTEMPTS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
            lockout_duration_minutes: env::var("LOCKOUT_DURATION_MINUTES")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            require_2fa: env::var("REQUIRE_2FA")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
        }
    }
}

/// Professional configuration manager that consolidates all settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub github: GitHubConfig,
    pub download: DownloadConfig,
    pub resources: ResourceConfig,
    pub logging: LoggingConfig,
    pub web: WebConfig,
    pub security: SecurityConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            github: GitHubConfig::default(),
            download: DownloadConfig::default(),
            resources: ResourceConfig::default(),
            logging: LoggingConfig::default(),
            web: WebConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Config {
    /// Create a new configuration from environment variables and optional config file
    pub fn new(config_file: Option<&str>) -> Result<Self> {
        let mut config = Config::default();

        // Load from file if provided
        if let Some(file_path) = config_file {
            config = config.load_from_file(file_path)?;
        }

        // Validate configuration
        config.validate()?;

        info!("Configuration loaded successfully");
        Ok(config)
    }

    /// Load configuration from JSON file
    pub fn load_from_file(self, config_file: &str) -> Result<Self> {
        let builder = ConfigBuilder::builder()
            .add_source(File::with_name(config_file).required(false))
            .add_source(Environment::with_prefix(""))
            .build()
            .context("Failed to build configuration")?;

        let config: Config = builder
            .try_deserialize()
            .context("Failed to deserialize configuration")?;

        info!("Configuration loaded from file: {}", config_file);
        Ok(config)
    }

    /// Save current configuration to JSON file
    pub fn save_to_file(&self, config_file: &str) -> Result<()> {
        let config_json = serde_json::to_string_pretty(self)
            .context("Failed to serialize configuration")?;

        std::fs::write(config_file, config_json)
            .context("Failed to write configuration file")?;

        info!("Configuration saved to: {}", config_file);
        Ok(())
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();

        // Validate database configuration
        if !self.validate_database_connection() {
            errors.push("Invalid database configuration");
        }

        // Validate resource limits
        if self.resources.memory_limit_gb <= 0.0 {
            errors.push("Memory limit must be positive");
        }
        if self.resources.disk_limit_gb <= 0.0 {
            errors.push("Disk limit must be positive");
        }
        if self.resources.cpu_limit_percent <= 0.0 || self.resources.cpu_limit_percent > 100.0 {
            errors.push("CPU limit must be between 0 and 100");
        }

        // Validate web configuration
        if self.web.port == 0 || self.web.port > 65535 {
            errors.push("Invalid web port");
        }

        // Validate download configuration
        if self.download.max_concurrent_downloads == 0 {
            errors.push("Max concurrent downloads must be positive");
        }
        if self.download.batch_size == 0 {
            errors.push("Batch size must be positive");
        }

        if !errors.is_empty() {
            let error_msg = format!("Configuration validation failed: {}", errors.join(", "));
            error!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }

        Ok(())
    }

    /// Validate database connection parameters
    pub fn validate_database_connection(&self) -> bool {
        if self.database.host.is_empty()
            || self.database.name.is_empty()
            || self.database.user.is_empty()
            || self.database.password.is_empty()
        {
            error!("Database configuration is incomplete");
            return false;
        }

        if self.database.port == 0 || self.database.port > 65535 {
            error!("Invalid database port: {}", self.database.port);
            return false;
        }

        true
    }

    /// Get resource limits for monitoring
    pub fn get_resource_limits(&self) -> std::collections::HashMap<String, f64> {
        let mut limits = std::collections::HashMap::new();
        limits.insert("memory_bytes".to_string(), self.resources.memory_limit_bytes() as f64);
        limits.insert("disk_bytes".to_string(), self.resources.disk_limit_bytes() as f64);
        limits.insert("cpu_percent".to_string(), self.resources.cpu_limit_percent);
        limits.insert("memory_warning_bytes".to_string(), self.resources.memory_warning_bytes() as f64);
        limits.insert("disk_warning_bytes".to_string(), self.resources.disk_warning_bytes() as f64);
        limits.insert("cpu_warning_percent".to_string(), self.resources.cpu_warning_percent());
        limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_creation() {
        let config = Config::default();
        assert_eq!(config.database.host, "localhost");
        assert_eq!(config.web.port, 8081);
    }

    #[test]
    fn test_database_connection_string() {
        let config = DatabaseConfig::default();
        let conn_str = config.connection_string();
        assert!(conn_str.contains("postgresql://"));
        assert!(conn_str.contains("gharchive"));
    }

    #[test]
    fn test_github_authentication() {
        let mut config = GitHubConfig::default();
        assert!(!config.is_authenticated());

        config.token = "test_token".to_string();
        assert!(config.is_authenticated());
    }

    #[test]
    fn test_resource_limits() {
        let config = ResourceConfig::default();
        assert!(config.memory_limit_bytes() > 0);
        assert!(config.disk_limit_bytes() > 0);
        assert!(config.cpu_warning_percent() > 0.0);
    }

    #[test]
    fn test_config_save_load() -> Result<()> {
        let config = Config::default();
        let temp_file = NamedTempFile::new()?;
        let file_path = temp_file.path().to_str().unwrap();

        // Save configuration
        config.save_to_file(file_path)?;

        // Load configuration
        let loaded_config = Config::new(Some(file_path))?;

        assert_eq!(config.database.host, loaded_config.database.host);
        assert_eq!(config.web.port, loaded_config.web.port);

        Ok(())
    }
}
