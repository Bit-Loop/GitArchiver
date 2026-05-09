// JWT authentication utilities for API sessions.

use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // subject (user id)
    pub exp: usize,  // expiration timestamp (seconds since epoch)
}

/// Create a JWT token for a given user id, with expiration (default 24h)
pub fn create_token(user_id: &str) -> Result<String> {
    let secret = env::var("JWT_SECRET")
        .unwrap_or_else(|_| "github-archive-scraper-jwt-secret-key".to_string());
    let expiration = Utc::now() + Duration::hours(24);
    let claims = Claims {
        sub: user_id.to_string(),
        exp: expiration.timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow!("JWT encode error: {e}"))
}

/// Verify a JWT token and return the claims if valid
pub fn verify_token(token: &str) -> Result<Claims> {
    let secret = env::var("JWT_SECRET")
        .unwrap_or_else(|_| "github-archive-scraper-jwt-secret-key".to_string());
    let validation = Validation::default();
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
        std::env::set_var("JWT_SECRET", "test-secret-one");

        let token = create_token("analyst").expect("token");
        let claims = verify_token(&token).expect("claims");

        assert_eq!(claims.sub, "analyst");
        assert!(claims.exp > Utc::now().timestamp() as usize);

        std::env::remove_var("JWT_SECRET");
    }

    #[test]
    fn verification_rejects_token_signed_with_different_secret() {
        let _guard = env_lock();
        std::env::set_var("JWT_SECRET", "signing-secret");
        let token = create_token("analyst").expect("token");

        std::env::set_var("JWT_SECRET", "verification-secret");
        let error = verify_token(&token).expect_err("wrong secret should fail");

        assert!(error.to_string().contains("JWT decode error"));
        std::env::remove_var("JWT_SECRET");
    }
}
