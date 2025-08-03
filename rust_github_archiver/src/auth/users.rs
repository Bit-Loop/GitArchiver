// User management implementation
use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
}

pub struct UserManager {
    users: Arc<RwLock<HashMap<String, User>>>,
}

impl UserManager {
    pub fn new() -> Self {
        let mut users = HashMap::new();
        
        // Create default admin user
        let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin123".to_string());
        if let Ok(admin_user) = Self::create_user("admin", &admin_password, "admin") {
            users.insert("admin".to_string(), admin_user);
        }
        
        Self {
            users: Arc::new(RwLock::new(users)),
        }
    }

    /// Authenticate a user with username and password
    pub async fn authenticate(&self, username: &str, password: &str) -> Option<User> {
        let users = self.users.read().await;
        if let Some(user) = users.get(username) {
            if user.is_active && self.verify_password(password, &user.password_hash).unwrap_or(false) {
                return Some(user.clone());
            }
        }
        None
    }

    /// Get user by username
    pub async fn get_user(&self, username: &str) -> Option<User> {
        let users = self.users.read().await;
        users.get(username).cloned()
    }

    /// Update user's last login time
    pub async fn update_last_login(&self, username: &str) -> Result<()> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(username) {
            user.last_login = Some(chrono::Utc::now());
        }
        Ok(())
    }

    /// Create a new user with hashed password
    fn create_user(username: &str, password: &str, role: &str) -> Result<User> {
        let password_hash = Self::hash_password(password)?;
        Ok(User {
            id: uuid::Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_hash,
            role: role.to_string(),
            created_at: chrono::Utc::now(),
            last_login: None,
            is_active: true,
        })
    }

    /// Hash a password using Argon2
    fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Password hashing failed: {}", e))?;
        Ok(password_hash.to_string())
    }

    /// Verify a password against a hash
    fn verify_password(&self, password: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| anyhow::anyhow!("Invalid password hash: {}", e))?;
        let argon2 = Argon2::default();
        Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
    }
}
