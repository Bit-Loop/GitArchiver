#!/usr/bin/env python3
"""
Professional Authentication Module for GitHub Archive Scraper
Consolidated authentication, session management, and security features.
"""

import os
import hashlib
import secrets
import time
import json
import logging
from typing import Optional, Dict, Any, Set, List
from dataclasses import dataclass, asdict
from pathlib import Path
from datetime import datetime, timedelta
import jwt
from cryptography.fernet import Fernet


@dataclass
class UserSession:
    """Represents an authenticated user session"""
    session_id: str
    user_id: str
    username: str
    permissions: Set[str]
    created_at: float
    expires_at: float
    last_activity: float
    ip_address: Optional[str] = None
    user_agent: Optional[str] = None
    
    @property
    def is_expired(self) -> bool:
        """Check if session has expired"""
        return time.time() > self.expires_at
    
    @property
    def is_valid(self) -> bool:
        """Check if session is valid and not expired"""
        return not self.is_expired
    
    @property
    def time_remaining(self) -> int:
        """Get remaining session time in seconds"""
        return max(0, int(self.expires_at - time.time()))
    
    def refresh(self, duration_hours: int = 24) -> None:
        """Refresh session expiry time"""
        self.expires_at = time.time() + (duration_hours * 3600)
        self.last_activity = time.time()
    
    def has_permission(self, permission: str) -> bool:
        """Check if session has specific permission"""
        return permission in self.permissions or 'admin' in self.permissions


@dataclass 
class AuthConfig:
    """Authentication configuration"""
    secret_key: str
    session_duration_hours: int = 24
    max_failed_attempts: int = 5
    lockout_duration_minutes: int = 30
    require_2fa: bool = False
    password_min_length: int = 8
    password_require_special: bool = True
    jwt_algorithm: str = 'HS256'


class AuthenticationError(Exception):
    """Authentication related errors"""
    pass


class AuthManager:
    """
    Professional authentication manager with security features:
    - Secure password hashing
    - Session management with JWT tokens
    - Rate limiting and brute force protection
    - Permission-based access control
    - Audit logging
    """
    
    def __init__(self, config_dir: str = "./auth"):
        self.config_dir = Path(config_dir)
        self.config_dir.mkdir(exist_ok=True)
        
        self.logger = logging.getLogger(__name__)
        
        # Configuration files
        self.auth_config_file = self.config_dir / "auth_config.json"
        self.users_file = self.config_dir / "users.json"
        self.sessions_file = self.config_dir / "sessions.json"
        self.audit_log_file = self.config_dir / "audit.log"
        
        # In-memory storage
        self.active_sessions: Dict[str, UserSession] = {}
        self.failed_attempts: Dict[str, Dict[str, Any]] = {}
        self.locked_accounts: Dict[str, float] = {}
        
        # Load configuration
        self.config = self._load_auth_config()
        self.encryption_key = self._get_or_create_encryption_key()
        self.cipher = Fernet(self.encryption_key)
        
        # Load existing data
        self._load_users()
        self._load_sessions()
        
        # Set up audit logging
        self._setup_audit_logging()
        
        # Default permissions
        self.default_permissions = {
            'admin': {'view', 'edit', 'delete', 'admin', 'scraper_control'},
            'user': {'view'},
            'readonly': {'view'}
        }
    
    def create_admin_user(self, username: str, password: str) -> bool:
        """
        Create the initial admin user.
        
        Args:
            username: Admin username
            password: Admin password
            
        Returns:
            True if user created successfully
        """
        try:
            if self._user_exists(username):
                self.logger.warning(f"Admin user {username} already exists")
                return False
            
            if not self._validate_password(password):
                raise AuthenticationError("Password does not meet security requirements")
            
            user_data = {
                'username': username,
                'password_hash': self._hash_password(password),
                'permissions': list(self.default_permissions['admin']),
                'created_at': time.time(),
                'last_login': None,
                'is_active': True,
                'is_admin': True,
                'failed_attempts': 0,
                'locked_until': None
            }
            
            users = self._load_users_data()
            users[username] = user_data
            self._save_users_data(users)
            
            self._audit_log('USER_CREATED', username, 'Admin user created')
            self.logger.info(f"Admin user {username} created successfully")
            return True
            
        except Exception as e:
            self.logger.error(f"Failed to create admin user: {e}")
            return False
    
    def authenticate(self, username: str, password: str, ip_address: str = None, 
                    user_agent: str = None) -> Optional[str]:
        """
        Authenticate user and create session.
        
        Args:
            username: User's username
            password: User's password
            ip_address: Client IP address
            user_agent: Client user agent
            
        Returns:
            Session token if authentication successful, None otherwise
        """
        try:
            # Check if account is locked
            if self._is_account_locked(username):
                self._audit_log('LOGIN_ATTEMPT_LOCKED', username, f'Login attempted on locked account from {ip_address}')
                raise AuthenticationError("Account is temporarily locked")
            
            # Load user data
            users = self._load_users_data()
            if username not in users:
                self._record_failed_attempt(username, ip_address)
                self._audit_log('LOGIN_FAILED', username, f'Unknown username from {ip_address}')
                raise AuthenticationError("Invalid credentials")
            
            user_data = users[username]
            
            # Check if user is active
            if not user_data.get('is_active', True):
                self._audit_log('LOGIN_FAILED', username, f'Inactive account login attempt from {ip_address}')
                raise AuthenticationError("Account is disabled")
            
            # Verify password
            if not self._verify_password(password, user_data['password_hash']):
                self._record_failed_attempt(username, ip_address)
                self._audit_log('LOGIN_FAILED', username, f'Invalid password from {ip_address}')
                raise AuthenticationError("Invalid credentials")
            
            # Clear failed attempts on successful login
            self._clear_failed_attempts(username)
            
            # Update user login time
            user_data['last_login'] = time.time()
            users[username] = user_data
            self._save_users_data(users)
            
            # Create session
            session = self._create_session(
                username, 
                user_data['permissions'], 
                ip_address, 
                user_agent
            )
            
            self._audit_log('LOGIN_SUCCESS', username, f'Successful login from {ip_address}')
            self.logger.info(f"User {username} authenticated successfully from {ip_address}")
            
            return session.session_id
            
        except AuthenticationError:
            raise
        except Exception as e:
            self.logger.error(f"Authentication error: {e}")
            raise AuthenticationError("Authentication failed")
    
    def validate_session(self, session_token: str) -> Optional[UserSession]:
        """
        Validate session token and return session if valid.
        
        Args:
            session_token: Session token to validate
            
        Returns:
            UserSession if valid, None otherwise
        """
        try:
            if not session_token:
                return None
            
            # Check in-memory sessions first
            if session_token in self.active_sessions:
                session = self.active_sessions[session_token]
                if session.is_valid:
                    session.last_activity = time.time()
                    return session
                else:
                    # Remove expired session
                    del self.active_sessions[session_token]
                    return None
            
            # Try to decode JWT token
            try:
                payload = jwt.decode(
                    session_token, 
                    self.config.secret_key, 
                    algorithms=[self.config.jwt_algorithm]
                )
                
                # Reconstruct session from JWT payload
                session = UserSession(
                    session_id=session_token,
                    user_id=payload['user_id'],
                    username=payload['username'],
                    permissions=set(payload['permissions']),
                    created_at=payload['created_at'],
                    expires_at=payload['expires_at'],
                    last_activity=time.time(),
                    ip_address=payload.get('ip_address'),
                    user_agent=payload.get('user_agent')
                )
                
                if session.is_valid:
                    self.active_sessions[session_token] = session
                    return session
                    
            except jwt.InvalidTokenError:
                pass
                
            return None
            
        except Exception as e:
            self.logger.error(f"Session validation error: {e}")
            return None
    
    def logout(self, session_token: str) -> bool:
        """
        Logout user and invalidate session.
        
        Args:
            session_token: Session token to invalidate
            
        Returns:
            True if logout successful
        """
        try:
            session = self.validate_session(session_token)
            if session:
                # Remove from active sessions
                if session_token in self.active_sessions:
                    del self.active_sessions[session_token]
                
                self._audit_log('LOGOUT', session.username, f'User logged out from {session.ip_address}')
                self.logger.info(f"User {session.username} logged out")
                return True
            
            return False
            
        except Exception as e:
            self.logger.error(f"Logout error: {e}")
            return False
    
    def change_password(self, username: str, old_password: str, new_password: str) -> bool:
        """
        Change user password.
        
        Args:
            username: Username
            old_password: Current password
            new_password: New password
            
        Returns:
            True if password changed successfully
        """
        try:
            users = self._load_users_data()
            if username not in users:
                raise AuthenticationError("User not found")
            
            user_data = users[username]
            
            # Verify old password
            if not self._verify_password(old_password, user_data['password_hash']):
                self._audit_log('PASSWORD_CHANGE_FAILED', username, 'Invalid old password')
                raise AuthenticationError("Invalid current password")
            
            # Validate new password
            if not self._validate_password(new_password):
                raise AuthenticationError("New password does not meet security requirements")
            
            # Update password
            user_data['password_hash'] = self._hash_password(new_password)
            users[username] = user_data
            self._save_users_data(users)
            
            # Invalidate all sessions for this user
            self._invalidate_user_sessions(username)
            
            self._audit_log('PASSWORD_CHANGED', username, 'Password changed successfully')
            self.logger.info(f"Password changed for user {username}")
            return True
            
        except AuthenticationError:
            raise
        except Exception as e:
            self.logger.error(f"Password change error: {e}")
            return False
    
    def list_active_sessions(self) -> List[Dict[str, Any]]:
        """Get list of active sessions for admin interface."""
        sessions = []
        current_time = time.time()
        
        for session_id, session in list(self.active_sessions.items()):
            if session.is_valid:
                sessions.append({
                    'session_id': session_id[:8] + '...',  # Truncated for security
                    'username': session.username,
                    'ip_address': session.ip_address,
                    'created_at': datetime.fromtimestamp(session.created_at).isoformat(),
                    'last_activity': datetime.fromtimestamp(session.last_activity).isoformat(),
                    'time_remaining': session.time_remaining,
                    'permissions': list(session.permissions)
                })
            else:
                # Clean up expired session
                del self.active_sessions[session_id]
        
        return sessions
    
    def cleanup_expired_sessions(self) -> int:
        """Remove expired sessions and return count of removed sessions."""
        removed_count = 0
        expired_sessions = []
        
        for session_id, session in self.active_sessions.items():
            if session.is_expired:
                expired_sessions.append(session_id)
        
        for session_id in expired_sessions:
            del self.active_sessions[session_id]
            removed_count += 1
        
        if removed_count > 0:
            self.logger.info(f"Cleaned up {removed_count} expired sessions")
        
        return removed_count
    
    # Private methods
    def _load_auth_config(self) -> AuthConfig:
        """Load authentication configuration."""
        if self.auth_config_file.exists():
            try:
                with open(self.auth_config_file, 'r') as f:
                    config_data = json.load(f)
                return AuthConfig(**config_data)
            except Exception as e:
                self.logger.error(f"Failed to load auth config: {e}")
        
        # Create default config
        secret_key = os.getenv('AUTH_SECRET_KEY', secrets.token_urlsafe(32))
        config = AuthConfig(secret_key=secret_key)
        
        # Save default config
        try:
            with open(self.auth_config_file, 'w') as f:
                json.dump(asdict(config), f, indent=2)
        except Exception as e:
            self.logger.error(f"Failed to save auth config: {e}")
        
        return config
    
    def _get_or_create_encryption_key(self) -> bytes:
        """Get or create encryption key for sensitive data."""
        key_file = self.config_dir / "encryption.key"
        
        if key_file.exists():
            try:
                return key_file.read_bytes()
            except Exception as e:
                self.logger.error(f"Failed to read encryption key: {e}")
        
        # Generate new key
        key = Fernet.generate_key()
        try:
            key_file.write_bytes(key)
            key_file.chmod(0o600)  # Restrict permissions
        except Exception as e:
            self.logger.error(f"Failed to save encryption key: {e}")
        
        return key
    
    def _load_users_data(self) -> Dict[str, Any]:
        """Load users data from file."""
        if not self.users_file.exists():
            return {}
        
        try:
            with open(self.users_file, 'r') as f:
                encrypted_data = f.read()
            
            if encrypted_data:
                decrypted_data = self.cipher.decrypt(encrypted_data.encode())
                return json.loads(decrypted_data)
            return {}
            
        except Exception as e:
            self.logger.error(f"Failed to load users data: {e}")
            return {}
    
    def _save_users_data(self, users: Dict[str, Any]) -> None:
        """Save users data to encrypted file."""
        try:
            data = json.dumps(users, indent=2)
            encrypted_data = self.cipher.encrypt(data.encode())
            
            with open(self.users_file, 'w') as f:
                f.write(encrypted_data.decode())
                
        except Exception as e:
            self.logger.error(f"Failed to save users data: {e}")
    
    def _load_users(self) -> None:
        """Load users into memory (placeholder for now)."""
        pass
    
    def _load_sessions(self) -> None:
        """Load active sessions (placeholder for now)."""
        pass
    
    def _setup_audit_logging(self) -> None:
        """Set up audit logging."""
        audit_logger = logging.getLogger('audit')
        audit_logger.setLevel(logging.INFO)
        
        if not audit_logger.handlers:
            handler = logging.FileHandler(self.audit_log_file)
            formatter = logging.Formatter(
                '%(asctime)s - %(levelname)s - %(message)s'
            )
            handler.setFormatter(formatter)
            audit_logger.addHandler(handler)
    
    def _audit_log(self, action: str, username: str, details: str) -> None:
        """Log security-relevant events."""
        audit_logger = logging.getLogger('audit')
        message = f"ACTION={action} USER={username} DETAILS={details}"
        audit_logger.info(message)
    
    def _hash_password(self, password: str) -> str:
        """Hash password using secure algorithm."""
        salt = secrets.token_hex(16)
        hashed = hashlib.pbkdf2_hmac('sha256', password.encode(), salt.encode(), 100000)
        return f"{salt}:{hashed.hex()}"
    
    def _verify_password(self, password: str, hash_string: str) -> bool:
        """Verify password against hash."""
        try:
            salt, hashed = hash_string.split(':')
            return hashlib.pbkdf2_hmac('sha256', password.encode(), salt.encode(), 100000).hex() == hashed
        except Exception:
            return False
    
    def _validate_password(self, password: str) -> bool:
        """Validate password meets security requirements."""
        if len(password) < self.config.password_min_length:
            return False
        
        if self.config.password_require_special:
            special_chars = "!@#$%^&*()_+-=[]{}|;:,.<>?"
            if not any(c in special_chars for c in password):
                return False
        
        return True
    
    def _user_exists(self, username: str) -> bool:
        """Check if user exists."""
        users = self._load_users_data()
        return username in users
    
    def _is_account_locked(self, username: str) -> bool:
        """Check if account is locked due to failed attempts."""
        if username in self.locked_accounts:
            unlock_time = self.locked_accounts[username]
            if time.time() < unlock_time:
                return True
            else:
                # Remove expired lock
                del self.locked_accounts[username]
        return False
    
    def _record_failed_attempt(self, username: str, ip_address: str) -> None:
        """Record failed login attempt and lock account if necessary."""
        current_time = time.time()
        
        if username not in self.failed_attempts:
            self.failed_attempts[username] = {
                'count': 0,
                'first_attempt': current_time,
                'last_attempt': current_time
            }
        
        attempt_data = self.failed_attempts[username]
        attempt_data['count'] += 1
        attempt_data['last_attempt'] = current_time
        
        # Reset counter if first attempt was too long ago
        if current_time - attempt_data['first_attempt'] > 3600:  # 1 hour
            attempt_data['count'] = 1
            attempt_data['first_attempt'] = current_time
        
        # Lock account if too many failed attempts
        if attempt_data['count'] >= self.config.max_failed_attempts:
            lock_duration = self.config.lockout_duration_minutes * 60
            self.locked_accounts[username] = current_time + lock_duration
            self._audit_log('ACCOUNT_LOCKED', username, f'Account locked after {attempt_data["count"]} failed attempts')
    
    def _clear_failed_attempts(self, username: str) -> None:
        """Clear failed attempts record for successful login."""
        if username in self.failed_attempts:
            del self.failed_attempts[username]
        if username in self.locked_accounts:
            del self.locked_accounts[username]
    
    def _create_session(self, username: str, permissions: List[str], ip_address: str, 
                       user_agent: str) -> UserSession:
        """Create new user session with JWT token."""
        current_time = time.time()
        expires_at = current_time + (self.config.session_duration_hours * 3600)
        
        # Create JWT payload
        payload = {
            'user_id': username,  # Using username as user_id for simplicity
            'username': username,
            'permissions': permissions,
            'created_at': current_time,
            'expires_at': expires_at,
            'ip_address': ip_address,
            'user_agent': user_agent,
            'iat': current_time,
            'exp': expires_at
        }
        
        # Generate JWT token
        session_token = jwt.encode(payload, self.config.secret_key, algorithm=self.config.jwt_algorithm)
        
        # Create session object
        session = UserSession(
            session_id=session_token,
            user_id=username,
            username=username,
            permissions=set(permissions),
            created_at=current_time,
            expires_at=expires_at,
            last_activity=current_time,
            ip_address=ip_address,
            user_agent=user_agent
        )
        
        # Store in active sessions
        self.active_sessions[session_token] = session
        
        return session
    
    def _invalidate_user_sessions(self, username: str) -> None:
        """Invalidate all sessions for a specific user."""
        sessions_to_remove = []
        for session_id, session in self.active_sessions.items():
            if session.username == username:
                sessions_to_remove.append(session_id)
        
        for session_id in sessions_to_remove:
            del self.active_sessions[session_id]
