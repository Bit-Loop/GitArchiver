#!/usr/bin/env python3
"""
Authentication Manager for GitHub Archive Scraper
Handles secure authentication for public web interface
"""

import os
import hashlib
import secrets
import time
import json
from typing import Optional, Dict, Any
from dataclasses import dataclass
from pathlib import Path


@dataclass
class AuthSession:
    """Authentication session"""
    token: str
    user_id: str
    github_token: Optional[str]
    created_at: float
    expires_at: float
    permissions: Dict[str, bool]
    
    @property
    def is_expired(self) -> bool:
        """Check if session is expired"""
        return time.time() > self.expires_at
    
    @property
    def is_valid(self) -> bool:
        """Check if session is valid"""
        return not self.is_expired


class AuthManager:
    """Manages authentication for the web interface"""
    
    def __init__(self, config_dir: str = "./auth"):
        self.config_dir = Path(config_dir)
        self.config_dir.mkdir(exist_ok=True)
        
        self.sessions: Dict[str, AuthSession] = {}
        self.auth_config_file = self.config_dir / "auth_config.json"
        self.sessions_file = self.config_dir / "sessions.json"
        
        # Default permissions
        self.default_permissions = {
            'read_data': True,
            'search_data': True,
            'view_stats': True,
            'download_archives': False,  # Requires auth
            'manage_repositories': False,  # Requires auth
            'admin_operations': False,  # Requires admin auth
            'rate_limited_operations': False  # Requires GitHub token
        }
        
        self._load_config()
        self._load_sessions()
        
    def _load_config(self):
        """Load authentication configuration"""
        try:
            if self.auth_config_file.exists():
                with open(self.auth_config_file, 'r') as f:
                    self.auth_config = json.load(f)
            else:
                self.auth_config = {
                    'admin_password_hash': None,
                    'session_timeout_hours': 24,
                    'require_auth_for_downloads': True,
                    'allow_github_token_config': True
                }
                self._save_config()
        except Exception as e:
            print(f"Error loading auth config: {e}")
            self.auth_config = {}
            
    def _save_config(self):
        """Save authentication configuration"""
        try:
            with open(self.auth_config_file, 'w') as f:
                json.dump(self.auth_config, f, indent=2)
        except Exception as e:
            print(f"Error saving auth config: {e}")
            
    def _load_sessions(self):
        """Load active sessions"""
        try:
            if self.sessions_file.exists():
                with open(self.sessions_file, 'r') as f:
                    session_data = json.load(f)
                    
                # Convert to AuthSession objects and filter expired
                current_time = time.time()
                for token, data in session_data.items():
                    if data['expires_at'] > current_time:
                        self.sessions[token] = AuthSession(**data)
        except Exception as e:
            print(f"Error loading sessions: {e}")
            
    def _save_sessions(self):
        """Save active sessions"""
        try:
            # Convert to serializable format
            session_data = {}
            for token, session in self.sessions.items():
                if session.is_valid:
                    session_data[token] = {
                        'token': session.token,
                        'user_id': session.user_id,
                        'github_token': session.github_token,
                        'created_at': session.created_at,
                        'expires_at': session.expires_at,
                        'permissions': session.permissions
                    }
                    
            with open(self.sessions_file, 'w') as f:
                json.dump(session_data, f, indent=2)
        except Exception as e:
            print(f"Error saving sessions: {e}")
            
    def set_admin_password(self, password: str) -> bool:
        """Set admin password"""
        try:
            # Generate salt and hash
            salt = secrets.token_hex(32)
            password_hash = hashlib.pbkdf2_hmac('sha256', 
                                               password.encode('utf-8'), 
                                               salt.encode('utf-8'), 
                                               100000)
            
            self.auth_config['admin_password_hash'] = {
                'hash': password_hash.hex(),
                'salt': salt
            }
            self._save_config()
            return True
        except Exception as e:
            print(f"Error setting admin password: {e}")
            return False
            
    def verify_admin_password(self, password: str) -> bool:
        """Verify admin password"""
        try:
            if not self.auth_config.get('admin_password_hash'):
                return False
                
            stored = self.auth_config['admin_password_hash']
            password_hash = hashlib.pbkdf2_hmac('sha256',
                                               password.encode('utf-8'),
                                               stored['salt'].encode('utf-8'),
                                               100000)
            
            return password_hash.hex() == stored['hash']
        except Exception as e:
            print(f"Error verifying admin password: {e}")
            return False
            
    def create_session(self, user_id: str, github_token: Optional[str] = None, 
                      is_admin: bool = False) -> str:
        """Create a new authentication session"""
        try:
            # Generate secure session token
            session_token = secrets.token_urlsafe(32)
            
            # Set permissions based on auth level
            permissions = self.default_permissions.copy()
            if github_token:
                permissions.update({
                    'download_archives': True,
                    'rate_limited_operations': True
                })
            if is_admin:
                permissions.update({
                    'download_archives': True,
                    'manage_repositories': True,
                    'admin_operations': True,
                    'rate_limited_operations': True
                })
                
            # Create session
            current_time = time.time()
            timeout_hours = self.auth_config.get('session_timeout_hours', 24)
            expires_at = current_time + (timeout_hours * 3600)
            
            session = AuthSession(
                token=session_token,
                user_id=user_id,
                github_token=github_token,
                created_at=current_time,
                expires_at=expires_at,
                permissions=permissions
            )
            
            self.sessions[session_token] = session
            self._save_sessions()
            
            return session_token
        except Exception as e:
            print(f"Error creating session: {e}")
            return ""
            
    def get_session(self, token: str) -> Optional[AuthSession]:
        """Get session by token"""
        session = self.sessions.get(token)
        if session and session.is_valid:
            return session
        elif session:
            # Remove expired session
            del self.sessions[token]
            self._save_sessions()
        return None
        
    def revoke_session(self, token: str) -> bool:
        """Revoke a session"""
        if token in self.sessions:
            del self.sessions[token]
            self._save_sessions()
            return True
        return False
        
    def cleanup_expired_sessions(self):
        """Remove expired sessions"""
        expired_tokens = [token for token, session in self.sessions.items() 
                         if session.is_expired]
        for token in expired_tokens:
            del self.sessions[token]
            
        if expired_tokens:
            self._save_sessions()
            
    def get_permissions(self, token: Optional[str]) -> Dict[str, bool]:
        """Get permissions for a session token"""
        if not token:
            return self.default_permissions.copy()
            
        session = self.get_session(token)
        if session:
            return session.permissions.copy()
        else:
            return self.default_permissions.copy()
            
    def has_permission(self, token: Optional[str], permission: str) -> bool:
        """Check if session has specific permission"""
        permissions = self.get_permissions(token)
        return permissions.get(permission, False)
        
    def get_github_token(self, session_token: Optional[str]) -> Optional[str]:
        """Get GitHub token for session"""
        if not session_token:
            return None
            
        session = self.get_session(session_token)
        return session.github_token if session else None
        
    def get_status(self) -> Dict[str, Any]:
        """Get authentication status"""
        self.cleanup_expired_sessions()
        
        return {
            'admin_configured': bool(self.auth_config.get('admin_password_hash')),
            'active_sessions': len(self.sessions),
            'session_timeout_hours': self.auth_config.get('session_timeout_hours', 24),
            'auth_required_for_downloads': self.auth_config.get('require_auth_for_downloads', True),
            'github_token_config_allowed': self.auth_config.get('allow_github_token_config', True)
        }
