#!/usr/bin/env python3
"""
Professional Configuration Management for GitHub Archive Scraper
Centralized configuration with environment variable support, validation, and type safety.
"""

import os
import json
import logging
from typing import Optional, Dict, Any, List, Union
from dataclasses import dataclass, field
from pathlib import Path
from dotenv import load_dotenv


# Load environment variables from .env file
load_dotenv()


@dataclass
class DatabaseConfig:
    """Database configuration settings"""
    host: str = field(default_factory=lambda: os.getenv('DB_HOST', 'localhost'))
    port: int = field(default_factory=lambda: int(os.getenv('DB_PORT', '5432')))
    name: str = field(default_factory=lambda: os.getenv('DB_NAME', 'gharchive'))
    user: str = field(default_factory=lambda: os.getenv('DB_USER', 'gharchive'))
    password: str = field(default_factory=lambda: os.getenv('DB_PASSWORD', 'gharchive_password'))
    min_connections: int = field(default_factory=lambda: int(os.getenv('DB_MIN_CONNECTIONS', '5')))
    max_connections: int = field(default_factory=lambda: int(os.getenv('DB_MAX_CONNECTIONS', '20')))
    command_timeout: int = field(default_factory=lambda: int(os.getenv('DB_COMMAND_TIMEOUT', '60')))
    
    @property
    def connection_string(self) -> str:
        """Get PostgreSQL connection string"""
        return f"postgresql://{self.user}:{self.password}@{self.host}:{self.port}/{self.name}"


@dataclass
class GitHubConfig:
    """GitHub API configuration"""
    token: str = field(default_factory=lambda: os.getenv('GITHUB_TOKEN', ''))
    username: str = field(default_factory=lambda: os.getenv('GITHUB_USERNAME', ''))
    api_base_url: str = 'https://api.github.com'
    unauthenticated_rate_limit: int = 60  # requests per hour
    authenticated_rate_limit: int = 5000  # requests per hour
    rate_limit_buffer: int = 5  # keep buffer requests
    rate_limit_reset_buffer: int = 300  # 5 minutes buffer before reset
    
    @property
    def is_authenticated(self) -> bool:
        """Check if GitHub token is configured"""
        return bool(self.token)
    
    @property
    def effective_rate_limit(self) -> int:
        """Get effective rate limit based on authentication"""
        return self.authenticated_rate_limit if self.is_authenticated else self.unauthenticated_rate_limit


@dataclass
class DownloadConfig:
    """Download and processing configuration"""
    base_url: str = 'https://data.gharchive.org/'
    s3_list_url: str = 'https://data.gharchive.org/?list-type=2'
    download_dir: Path = field(default_factory=lambda: Path(os.getenv('DOWNLOAD_DIR', './gharchive_data')))
    max_concurrent_downloads: int = field(default_factory=lambda: int(os.getenv('MAX_CONCURRENT', '6')))
    chunk_size: int = field(default_factory=lambda: int(os.getenv('CHUNK_SIZE', '4096')))
    request_timeout: int = field(default_factory=lambda: int(os.getenv('REQUEST_TIMEOUT', '180')))
    max_retries: int = field(default_factory=lambda: int(os.getenv('MAX_RETRIES', '3')))
    retry_delay: float = field(default_factory=lambda: float(os.getenv('RETRY_DELAY', '2.0')))
    batch_size: int = field(default_factory=lambda: int(os.getenv('BATCH_SIZE', '500')))
    
    def __post_init__(self):
        """Ensure download directory is a Path object"""
        if isinstance(self.download_dir, str):
            self.download_dir = Path(self.download_dir)
        self.download_dir.mkdir(parents=True, exist_ok=True)


@dataclass
class ResourceConfig:
    """Resource monitoring and limits"""
    memory_limit_gb: float = field(default_factory=lambda: float(os.getenv('MEMORY_LIMIT_GB', '18.0')))
    disk_limit_gb: float = field(default_factory=lambda: float(os.getenv('DISK_LIMIT_GB', '40.0')))
    cpu_limit_percent: float = field(default_factory=lambda: float(os.getenv('CPU_LIMIT_PERCENT', '80.0')))
    memory_warning_threshold: float = 0.8  # 80% of limit
    disk_warning_threshold: float = 0.8  # 80% of limit
    cpu_warning_threshold: float = 0.7  # 70% of limit
    emergency_cleanup_threshold: float = 0.9  # 90% of limit
    monitoring_interval_seconds: int = 30
    
    @property
    def memory_limit_bytes(self) -> int:
        """Get memory limit in bytes"""
        return int(self.memory_limit_gb * 1024 * 1024 * 1024)
    
    @property
    def disk_limit_bytes(self) -> int:
        """Get disk limit in bytes"""
        return int(self.disk_limit_gb * 1024 * 1024 * 1024)


@dataclass
class LoggingConfig:
    """Logging configuration"""
    level: str = field(default_factory=lambda: os.getenv('LOG_LEVEL', 'INFO'))
    log_dir: Path = field(default_factory=lambda: Path(os.getenv('LOG_DIR', './logs')))
    main_log_file: str = 'scraper.log'
    api_log_file: str = 'api.log'
    audit_log_file: str = 'audit.log'
    max_file_size_mb: int = 50
    backup_count: int = 5
    
    def __post_init__(self):
        """Ensure log directory exists"""
        if isinstance(self.log_dir, str):
            self.log_dir = Path(self.log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)
    
    @property
    def main_log_path(self) -> Path:
        """Get main log file path"""
        return self.log_dir / self.main_log_file
    
    @property
    def api_log_path(self) -> Path:
        """Get API log file path"""
        return self.log_dir / self.api_log_file
    
    @property
    def audit_log_path(self) -> Path:
        """Get audit log file path"""
        return self.log_dir / self.audit_log_file


@dataclass
class WebConfig:
    """Web API configuration"""
    host: str = field(default_factory=lambda: os.getenv('WEB_HOST', '0.0.0.0'))
    port: int = field(default_factory=lambda: int(os.getenv('WEB_PORT', '8080')))
    debug: bool = field(default_factory=lambda: os.getenv('WEB_DEBUG', 'False').lower() == 'true')
    cors_origins: List[str] = field(default_factory=lambda: os.getenv('CORS_ORIGINS', '*').split(','))
    max_request_size: int = field(default_factory=lambda: int(os.getenv('MAX_REQUEST_SIZE', '16777216')))  # 16MB
    request_timeout: int = field(default_factory=lambda: int(os.getenv('WEB_REQUEST_TIMEOUT', '30')))
    
    @property
    def base_url(self) -> str:
        """Get base URL for the web interface"""
        return f"http://{self.host}:{self.port}"


@dataclass
class SecurityConfig:
    """Security configuration"""
    admin_password: str = field(default_factory=lambda: os.getenv('ADMIN_PASSWORD', 'admin123'))
    secret_key: str = field(default_factory=lambda: os.getenv('SECRET_KEY', ''))
    jwt_secret: str = field(default_factory=lambda: os.getenv('JWT_SECRET', 'github-archive-scraper-jwt-secret-key'))
    session_duration_hours: int = field(default_factory=lambda: int(os.getenv('SESSION_DURATION_HOURS', '24')))
    max_failed_attempts: int = field(default_factory=lambda: int(os.getenv('MAX_FAILED_ATTEMPTS', '5')))
    lockout_duration_minutes: int = field(default_factory=lambda: int(os.getenv('LOCKOUT_DURATION_MINUTES', '30')))
    require_2fa: bool = field(default_factory=lambda: os.getenv('REQUIRE_2FA', 'False').lower() == 'true')
    
    def __post_init__(self):
        """Generate secret key if not provided"""
        if not self.secret_key:
            import secrets
            self.secret_key = secrets.token_urlsafe(32)


class ConfigurationError(Exception):
    """Configuration related errors"""
    pass


class Config:
    """
    Professional configuration manager that consolidates all settings.
    Provides validation, environment variable support, and easy access patterns.
    """
    
    def __init__(self, config_file: Optional[str] = None):
        """
        Initialize configuration from environment variables and optional config file.
        
        Args:
            config_file: Optional JSON configuration file to load
        """
        self.logger = logging.getLogger(__name__)
        
        # Load configuration sections
        self.database = DatabaseConfig()
        self.github = GitHubConfig()
        self.download = DownloadConfig()
        self.resources = ResourceConfig()
        self.logging = LoggingConfig()
        self.web = WebConfig()
        self.security = SecurityConfig()
        
        # Load from file if provided
        if config_file:
            self._load_from_file(config_file)
        
        # Validate configuration
        self._validate_config()
        
        # Set up logging based on configuration
        self._setup_logging()
    
    def save_to_file(self, config_file: str) -> None:
        """
        Save current configuration to JSON file.
        
        Args:
            config_file: Path to save configuration file
        """
        try:
            config_data = {
                'database': self._dataclass_to_dict(self.database),
                'github': self._dataclass_to_dict(self.github),
                'download': self._dataclass_to_dict(self.download),
                'resources': self._dataclass_to_dict(self.resources),
                'logging': self._dataclass_to_dict(self.logging),
                'web': self._dataclass_to_dict(self.web),
                'security': self._dataclass_to_dict(self.security, exclude_sensitive=True)
            }
            
            with open(config_file, 'w') as f:
                json.dump(config_data, f, indent=2, default=str)
            
            self.logger.info(f"Configuration saved to {config_file}")
            
        except Exception as e:
            self.logger.error(f"Failed to save configuration: {e}")
            raise ConfigurationError(f"Failed to save configuration: {e}")
    
    def get_environment_info(self) -> Dict[str, Any]:
        """Get current environment information for debugging."""
        return {
            'python_version': os.sys.version,
            'platform': os.name,
            'environment_variables': {
                key: value for key, value in os.environ.items() 
                if key.startswith(('DB_', 'GITHUB_', 'LOG_', 'WEB_', 'DOWNLOAD_', 'RESOURCE_'))
            },
            'config_status': {
                'database_configured': bool(self.database.password),
                'github_authenticated': self.github.is_authenticated,
                'download_dir_exists': self.download.download_dir.exists(),
                'log_dir_exists': self.logging.log_dir.exists(),
            }
        }
    
    def validate_database_connection(self) -> bool:
        """
        Validate database connection parameters.
        
        Returns:
            True if configuration appears valid
        """
        required_fields = ['host', 'port', 'name', 'user', 'password']
        for field in required_fields:
            value = getattr(self.database, field)
            if not value:
                self.logger.error(f"Database {field} is not configured")
                return False
        
        if not (1 <= self.database.port <= 65535):
            self.logger.error(f"Invalid database port: {self.database.port}")
            return False
        
        return True
    
    def get_resource_limits(self) -> Dict[str, Union[int, float]]:
        """Get resource limits for monitoring."""
        return {
            'memory_bytes': self.resources.memory_limit_bytes,
            'disk_bytes': self.resources.disk_limit_bytes,
            'cpu_percent': self.resources.cpu_limit_percent,
            'memory_warning_bytes': int(self.resources.memory_limit_bytes * self.resources.memory_warning_threshold),
            'disk_warning_bytes': int(self.resources.disk_limit_bytes * self.resources.disk_warning_threshold),
            'cpu_warning_percent': self.resources.cpu_limit_percent * self.resources.cpu_warning_threshold
        }
    
    # Legacy property access for backward compatibility
    @property
    def DB_HOST(self) -> str:
        return self.database.host
    
    @property
    def DB_PORT(self) -> int:
        return self.database.port
    
    @property
    def DB_NAME(self) -> str:
        return self.database.name
    
    @property
    def DB_USER(self) -> str:
        return self.database.user
    
    @property
    def DB_PASSWORD(self) -> str:
        return self.database.password
    
    @property
    def DB_MIN_CONNECTIONS(self) -> int:
        return self.database.min_connections
    
    @property
    def DB_MAX_CONNECTIONS(self) -> int:
        return self.database.max_connections
    
    @property
    def GITHUB_TOKEN(self) -> str:
        return self.github.token
    
    @property
    def BASE_URL(self) -> str:
        return self.download.base_url
    
    @property
    def S3_LIST_URL(self) -> str:
        return self.download.s3_list_url
    
    @property
    def DOWNLOAD_DIR(self) -> Path:
        return self.download.download_dir
    
    @property
    def MAX_CONCURRENT_DOWNLOADS(self) -> int:
        return self.download.max_concurrent_downloads
    
    @property
    def BATCH_SIZE(self) -> int:
        return self.download.batch_size
    
    @property
    def MAX_RETRIES(self) -> int:
        return self.download.max_retries
    
    @property
    def RETRY_DELAY(self) -> float:
        return self.download.retry_delay
    
    @property
    def REQUEST_TIMEOUT(self) -> int:
        return self.download.request_timeout
    
    @property
    def LOG_FILE(self) -> Path:
        return self.logging.main_log_path
    
    # Private methods
    def _load_from_file(self, config_file: str) -> None:
        """Load configuration from JSON file."""
        try:
            with open(config_file, 'r') as f:
                config_data = json.load(f)
            
            # Update configuration sections
            if 'database' in config_data:
                self._update_dataclass(self.database, config_data['database'])
            if 'github' in config_data:
                self._update_dataclass(self.github, config_data['github'])
            if 'download' in config_data:
                self._update_dataclass(self.download, config_data['download'])
            if 'resources' in config_data:
                self._update_dataclass(self.resources, config_data['resources'])
            if 'logging' in config_data:
                self._update_dataclass(self.logging, config_data['logging'])
            if 'web' in config_data:
                self._update_dataclass(self.web, config_data['web'])
            if 'security' in config_data:
                self._update_dataclass(self.security, config_data['security'])
            
            self.logger.info(f"Configuration loaded from {config_file}")
            
        except FileNotFoundError:
            self.logger.warning(f"Configuration file {config_file} not found, using defaults")
        except Exception as e:
            self.logger.error(f"Failed to load configuration from {config_file}: {e}")
            raise ConfigurationError(f"Failed to load configuration: {e}")
    
    def _validate_config(self) -> None:
        """Validate configuration values."""
        errors = []
        
        # Validate database configuration
        if not self.validate_database_connection():
            errors.append("Invalid database configuration")
        
        # Validate resource limits
        if self.resources.memory_limit_gb <= 0:
            errors.append("Memory limit must be positive")
        if self.resources.disk_limit_gb <= 0:
            errors.append("Disk limit must be positive")
        if not (0 < self.resources.cpu_limit_percent <= 100):
            errors.append("CPU limit must be between 0 and 100")
        
        # Validate web configuration
        if not (1 <= self.web.port <= 65535):
            errors.append(f"Invalid web port: {self.web.port}")
        
        # Validate download configuration
        if self.download.max_concurrent_downloads <= 0:
            errors.append("Max concurrent downloads must be positive")
        if self.download.batch_size <= 0:
            errors.append("Batch size must be positive")
        
        if errors:
            error_msg = "Configuration validation failed: " + ", ".join(errors)
            self.logger.error(error_msg)
            raise ConfigurationError(error_msg)
    
    def _setup_logging(self) -> None:
        """Set up logging based on configuration."""
        log_level = getattr(logging, self.logging.level.upper(), logging.INFO)
        
        # Configure root logger
        logging.basicConfig(
            level=log_level,
            format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
            handlers=[
                logging.FileHandler(self.logging.main_log_path),
                logging.StreamHandler()
            ]
        )
        
        self.logger.info(f"Logging configured at {self.logging.level} level")
    
    def _dataclass_to_dict(self, obj: Any, exclude_sensitive: bool = False) -> Dict[str, Any]:
        """Convert dataclass to dictionary, optionally excluding sensitive data."""
        result = {}
        for field_name in obj.__dataclass_fields__:
            value = getattr(obj, field_name)
            
            # Skip sensitive fields if requested
            if exclude_sensitive and field_name in ('password', 'token', 'secret_key', 'admin_password'):
                result[field_name] = '***REDACTED***'
            elif isinstance(value, Path):
                result[field_name] = str(value)
            else:
                result[field_name] = value
                
        return result
    
    def _update_dataclass(self, obj: Any, data: Dict[str, Any]) -> None:
        """Update dataclass fields from dictionary."""
        for key, value in data.items():
            if hasattr(obj, key):
                # Handle Path objects
                if isinstance(getattr(obj, key), Path):
                    value = Path(value)
                setattr(obj, key, value)
