// JWT authentication utilities for API sessions.

use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

use crate::auth::users::User;

const JWT_ISSUER: &str = "github-archiver";
const JWT_AUDIENCE: &str = "github-archiver-api";
const JWT_TTL_HOURS: i64 = 24;
const MIN_JWT_SECRET_LEN: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // subject (user id)
    pub exp: usize,  // expiration timestamp (seconds since epoch)
    pub iat: usize,
    pub nbf: usize,
    pub iss: String,
    pub aud: String,
    pub jti: String,
    pub role: String,
    pub token_version: u64,
}

fn jwt_secret_from_env() -> Result<String> {
    let secret = env::var("JWT_SECRET")
        .context("JWT_SECRET is required; refusing to use static fallback")?;
    validate_jwt_secret(&secret)?;
    Ok(secret)
}

pub fn validate_jwt_secret(secret: &str) -> Result<()> {
    let trimmed = secret.trim();
    let lowered = trimmed.to_ascii_lowercase();

    if trimmed.len() < MIN_JWT_SECRET_LEN {
        return Err(anyhow!(
            "JWT_SECRET must be at least {MIN_JWT_SECRET_LEN} characters long"
        ));
    }

    if lowered.contains("your-secret")
        || lowered.contains("changeme")
        || lowered.contains("default")
        || lowered.contains("github-archive-scraper-jwt-secret-key")
    {
        return Err(anyhow!("JWT_SECRET uses an unsafe configured value"));
    }

    Ok(())
}

fn token_expiration() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::hours(JWT_TTL_HOURS)
}

pub fn token_expiration_rfc3339() -> String {
    token_expiration().to_rfc3339()
}

fn create_token_with_claims(user_id: &str, role: &str, token_version: u64) -> Result<String> {
    let secret = jwt_secret_from_env()?;
    let now = Utc::now();
    let expiration = now + Duration::hours(JWT_TTL_HOURS);
    let claims = Claims {
        sub: user_id.to_string(),
        exp: expiration.timestamp() as usize,
        iat: now.timestamp() as usize,
        nbf: now.timestamp() as usize,
        iss: JWT_ISSUER.to_string(),
        aud: JWT_AUDIENCE.to_string(),
        jti: Uuid::new_v4().to_string(),
        role: role.to_string(),
        token_version,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow!("JWT encode error: {e}"))
}

/// Create a JWT token for a user record.
pub fn create_token_for_user(user: &User) -> Result<String> {
    create_token_with_claims(&user.username, &user.canonical_role(), user.token_version)
}

/// Create a read-only JWT token for a subject. Prefer create_token_for_user in production paths.
pub fn create_token(user_id: &str) -> Result<String> {
    create_token_with_claims(user_id, "read_only", 0)
}

/// Verify a JWT token and return the claims if valid
pub fn verify_token(token: &str) -> Result<Claims> {
    let secret = jwt_secret_from_env()?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&[JWT_AUDIENCE]);
    validation.set_issuer(&[JWT_ISSUER]);
    validation.validate_nbf = true;

    let token_data: TokenData<Claims> = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| anyhow!("JWT decode error: {e}"))?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::users::User;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    #[test]
    fn token_round_trip_preserves_subject_and_future_expiration() {
        let _guard = env_lock();
        std::env::set_var("JWT_SECRET", "test-secret-one-0123456789abcdef");

        let token = create_token("analyst").expect("token");
        let claims = verify_token(&token).expect("claims");

        assert_eq!(claims.sub, "analyst");
        assert_eq!(claims.role, "read_only");
        assert_eq!(claims.iss, JWT_ISSUER);
        assert_eq!(claims.aud, JWT_AUDIENCE);
        assert!(claims.exp > Utc::now().timestamp() as usize);

        std::env::remove_var("JWT_SECRET");
    }

    #[test]
    fn verification_rejects_token_signed_with_different_secret() {
        let _guard = env_lock();
        std::env::set_var("JWT_SECRET", "signing-secret-0123456789abcdef00");
        let token = create_token("analyst").expect("token");

        std::env::set_var("JWT_SECRET", "verification-secret-0123456789abcdef00");
        let error = verify_token(&token).expect_err("wrong secret should fail");

        assert!(error.to_string().contains("JWT decode error"));
        std::env::remove_var("JWT_SECRET");
    }

    #[test]
    fn missing_jwt_secret_is_rejected() {
        let _guard = env_lock();
        std::env::remove_var("JWT_SECRET");

        let error = create_token("analyst").expect_err("secret must be required");

        assert!(error.to_string().contains("JWT_SECRET is required"));
    }

    #[test]
    fn token_for_user_carries_role_and_token_version() {
        let _guard = env_lock();
        std::env::set_var("JWT_SECRET", "role-secret-0123456789abcdef012345");
        let user = User {
            id: "admin-id".to_string(),
            username: "admin".to_string(),
            password_hash: "hash".to_string(),
            role: "admin".to_string(),
            created_at: Utc::now(),
            last_login: None,
            is_active: true,
            token_version: 3,
        };

        let token = create_token_for_user(&user).expect("token");
        let claims = verify_token(&token).expect("claims");

        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.token_version, 3);
        std::env::remove_var("JWT_SECRET");
    }
}
