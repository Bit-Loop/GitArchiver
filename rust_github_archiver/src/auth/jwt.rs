// JWT authentication placeholder

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, TokenData};
use std::env;
use chrono::{Utc, Duration};


#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // subject (user id)
    pub exp: usize,  // expiration timestamp (seconds since epoch)
}


/// Create a JWT token for a given user id, with expiration (default 24h)
pub fn create_token(user_id: &str) -> Result<String> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "github-archive-scraper-jwt-secret-key".to_string());
    let expiration = Utc::now() + Duration::hours(24);
    let claims = Claims {
        sub: user_id.to_string(),
        exp: expiration.timestamp() as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| anyhow!("JWT encode error: {e}"))
}


/// Verify a JWT token and return the claims if valid
pub fn verify_token(token: &str) -> Result<Claims> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "github-archive-scraper-jwt-secret-key".to_string());
    let validation = Validation::default();
    let token_data: TokenData<Claims> = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map_err(|e| anyhow!("JWT decode error: {e}"))?;
    Ok(token_data.claims)
}
