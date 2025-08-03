use anyhow::{anyhow, Result};
use aws_config::BehaviorVersion;
use aws_sdk_sts::Client as StsClient;
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::time::Duration;
use tracing::{info, warn, error, debug};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use crate::secrets::scanner::{SecretMatch, SecretSeverity};

/// Secret validator for verifying if secrets are active
pub struct SecretValidator {
    http_client: HttpClient,
    aws_config: Option<aws_config::SdkConfig>,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub secret_hash: String,
    pub is_valid: bool,
    pub validation_method: String,
    pub error_message: Option<String>,
    pub additional_info: Option<String>,
    pub validated_at: chrono::DateTime<chrono::Utc>,
}

impl SecretValidator {
    /// Create a new secret validator
    pub async fn new() -> Result<Self> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("GitArchiver-SecretValidator/1.0")
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        // Try to load AWS config (may fail if not configured)
        let aws_config = match aws_config::load_defaults(BehaviorVersion::latest()).await {
            config => Some(config),
        };

        Ok(Self {
            http_client,
            aws_config,
        })
    }

    /// Validate a secret match
    pub async fn validate_secret(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        info!("Validating secret: {}", secret_match.detector_name);

        let result = match secret_match.detector_name.as_str() {
            name if name.contains("AWS") => self.validate_aws_credentials(secret_match).await,
            name if name.contains("GitHub") => self.validate_github_token(secret_match).await,
            name if name.contains("Slack") => self.validate_slack_token(secret_match).await,
            name if name.contains("Discord") => self.validate_discord_token(secret_match).await,
            name if name.contains("Google") => self.validate_google_api_key(secret_match).await,
            name if name.contains("Stripe") => self.validate_stripe_key(secret_match).await,
            name if name.contains("SendGrid") => self.validate_sendgrid_key(secret_match).await,
            name if name.contains("Twilio") => self.validate_twilio_key(secret_match).await,
            name if name.contains("JWT") => self.validate_jwt_token(secret_match).await,
            _ => Ok(ValidationResult {
                secret_hash: secret_match.hash.clone(),
                is_valid: false,
                validation_method: "unsupported".to_string(),
                error_message: Some("Validation not supported for this secret type".to_string()),
                additional_info: None,
                validated_at: chrono::Utc::now(),
            }),
        };

        match result {
            Ok(mut validation_result) => {
                validation_result.secret_hash = secret_match.hash.clone();
                Ok(validation_result)
            }
            Err(e) => {
                error!("Validation failed for {}: {}", secret_match.detector_name, e);
                Ok(ValidationResult {
                    secret_hash: secret_match.hash.clone(),
                    is_valid: false,
                    validation_method: secret_match.detector_name.clone(),
                    error_message: Some(e.to_string()),
                    additional_info: None,
                    validated_at: chrono::Utc::now(),
                })
            }
        }
    }

    /// Validate AWS credentials
    async fn validate_aws_credentials(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        if let Some(_aws_config) = &self.aws_config {
            // For AWS validation, we'd need both access key and secret key
            // This is a simplified version - in practice, you'd extract both from context
            
            if secret_match.detector_name.contains("Access Key") {
                // For access key, we can't validate without secret key
                return Ok(ValidationResult {
                    secret_hash: String::new(),
                    is_valid: false,
                    validation_method: "aws_access_key_check".to_string(),
                    error_message: Some("Cannot validate access key without secret key".to_string()),
                    additional_info: Some("Access key format appears valid".to_string()),
                    validated_at: chrono::Utc::now(),
                });
            }

            // For secret keys, we'd try STS GetCallerIdentity
            // Note: This is dangerous in real scenarios as it could trigger alerts
            warn!("AWS secret validation disabled for security reasons");
            Ok(ValidationResult {
                secret_hash: String::new(),
                is_valid: false,
                validation_method: "aws_sts_disabled".to_string(),
                error_message: Some("AWS validation disabled for security".to_string()),
                additional_info: None,
                validated_at: chrono::Utc::now(),
            })
        } else {
            Ok(ValidationResult {
                secret_hash: String::new(),
                is_valid: false,
                validation_method: "aws_no_config".to_string(),
                error_message: Some("AWS config not available".to_string()),
                additional_info: None,
                validated_at: chrono::Utc::now(),
            })
        }
    }

    /// Validate GitHub token
    async fn validate_github_token(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let token = &secret_match.matched_text;
        
        let response = self
            .http_client
            .get("https://api.github.com/user")
            .header("Authorization", format!("token {}", token))
            .header("User-Agent", "GitArchiver-SecretValidator/1.0")
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let user_info: Result<Value, _> = resp.json().await;
                    let additional_info = match user_info {
                        Ok(user) => {
                            let login = user["login"].as_str().unwrap_or("unknown");
                            let user_type = user["type"].as_str().unwrap_or("User");
                            Some(format!("User: {} ({})", login, user_type))
                        }
                        Err(_) => None,
                    };

                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: true,
                        validation_method: "github_api".to_string(),
                        error_message: None,
                        additional_info,
                        validated_at: chrono::Utc::now(),
                    })
                } else if status == 401 {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "github_api".to_string(),
                        error_message: Some("Token is invalid or expired".to_string()),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                } else {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "github_api".to_string(),
                        error_message: Some(format!("HTTP {}", status)),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                }
            }
            Err(e) => Err(anyhow!("GitHub API request failed: {}", e)),
        }
    }

    /// Validate Slack token
    async fn validate_slack_token(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let token = &secret_match.matched_text;
        
        let response = self
            .http_client
            .post("https://slack.com/api/auth.test")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let slack_response: Result<Value, _> = resp.json().await;
                match slack_response {
                    Ok(data) => {
                        let is_valid = data["ok"].as_bool().unwrap_or(false);
                        let error_msg = data["error"].as_str().map(|s| s.to_string());
                        let additional_info = if is_valid {
                            let team = data["team"].as_str().unwrap_or("unknown");
                            let user = data["user"].as_str().unwrap_or("unknown");
                            Some(format!("Team: {}, User: {}", team, user))
                        } else {
                            None
                        };

                        Ok(ValidationResult {
                            secret_hash: String::new(),
                            is_valid,
                            validation_method: "slack_auth_test".to_string(),
                            error_message: error_msg,
                            additional_info,
                            validated_at: chrono::Utc::now(),
                        })
                    }
                    Err(e) => Err(anyhow!("Failed to parse Slack response: {}", e)),
                }
            }
            Err(e) => Err(anyhow!("Slack API request failed: {}", e)),
        }
    }

    /// Validate Discord token
    async fn validate_discord_token(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let token = &secret_match.matched_text;
        
        let response = self
            .http_client
            .get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", token))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let user_info: Result<Value, _> = resp.json().await;
                    let additional_info = match user_info {
                        Ok(user) => {
                            let username = user["username"].as_str().unwrap_or("unknown");
                            let discriminator = user["discriminator"].as_str().unwrap_or("0000");
                            Some(format!("Bot: {}#{}", username, discriminator))
                        }
                        Err(_) => None,
                    };

                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: true,
                        validation_method: "discord_api".to_string(),
                        error_message: None,
                        additional_info,
                        validated_at: chrono::Utc::now(),
                    })
                } else if status == 401 {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "discord_api".to_string(),
                        error_message: Some("Token is invalid".to_string()),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                } else {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "discord_api".to_string(),
                        error_message: Some(format!("HTTP {}", status)),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                }
            }
            Err(e) => Err(anyhow!("Discord API request failed: {}", e)),
        }
    }

    /// Validate Google API key
    async fn validate_google_api_key(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let api_key = &secret_match.matched_text;
        
        // Use a simple API endpoint that most keys have access to
        let response = self
            .http_client
            .get(&format!("https://www.googleapis.com/discovery/v1/apis?key={}", api_key))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: true,
                        validation_method: "google_discovery_api".to_string(),
                        error_message: None,
                        additional_info: Some("Key has access to Discovery API".to_string()),
                        validated_at: chrono::Utc::now(),
                    })
                } else if status == 403 {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "google_discovery_api".to_string(),
                        error_message: Some("API key is invalid or restricted".to_string()),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                } else {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "google_discovery_api".to_string(),
                        error_message: Some(format!("HTTP {}", status)),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                }
            }
            Err(e) => Err(anyhow!("Google API request failed: {}", e)),
        }
    }

    /// Validate Stripe API key
    async fn validate_stripe_key(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let api_key = &secret_match.matched_text;
        
        let response = self
            .http_client
            .get("https://api.stripe.com/v1/account")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let account_info: Result<Value, _> = resp.json().await;
                    let additional_info = match account_info {
                        Ok(account) => {
                            let country = account["country"].as_str().unwrap_or("unknown");
                            let business_type = account["business_type"].as_str().unwrap_or("unknown");
                            Some(format!("Country: {}, Type: {}", country, business_type))
                        }
                        Err(_) => None,
                    };

                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: true,
                        validation_method: "stripe_account_api".to_string(),
                        error_message: None,
                        additional_info,
                        validated_at: chrono::Utc::now(),
                    })
                } else if status == 401 {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "stripe_account_api".to_string(),
                        error_message: Some("API key is invalid".to_string()),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                } else {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "stripe_account_api".to_string(),
                        error_message: Some(format!("HTTP {}", status)),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                }
            }
            Err(e) => Err(anyhow!("Stripe API request failed: {}", e)),
        }
    }

    /// Validate SendGrid API key
    async fn validate_sendgrid_key(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let api_key = &secret_match.matched_text;
        
        let response = self
            .http_client
            .get("https://api.sendgrid.com/v3/user/account")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: true,
                        validation_method: "sendgrid_account_api".to_string(),
                        error_message: None,
                        additional_info: Some("Key has access to account API".to_string()),
                        validated_at: chrono::Utc::now(),
                    })
                } else if status == 401 || status == 403 {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "sendgrid_account_api".to_string(),
                        error_message: Some("API key is invalid or lacks permissions".to_string()),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                } else {
                    Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "sendgrid_account_api".to_string(),
                        error_message: Some(format!("HTTP {}", status)),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    })
                }
            }
            Err(e) => Err(anyhow!("SendGrid API request failed: {}", e)),
        }
    }

    /// Validate Twilio API key
    async fn validate_twilio_key(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let api_key = &secret_match.matched_text;
        
        // Note: Twilio validation would require account SID as well
        // This is a simplified check
        Ok(ValidationResult {
            secret_hash: String::new(),
            is_valid: false,
            validation_method: "twilio_format_check".to_string(),
            error_message: Some("Twilio validation requires account SID".to_string()),
            additional_info: Some("Key format appears valid".to_string()),
            validated_at: chrono::Utc::now(),
        })
    }

    /// Validate JWT token
    async fn validate_jwt_token(&self, secret_match: &SecretMatch) -> Result<ValidationResult> {
        let token = &secret_match.matched_text;
        
        // Parse JWT without verification (just structure check)
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Ok(ValidationResult {
                secret_hash: String::new(),
                is_valid: false,
                validation_method: "jwt_structure_check".to_string(),
                error_message: Some("Invalid JWT structure".to_string()),
                additional_info: None,
                validated_at: chrono::Utc::now(),
            });
        }

        // Try to decode header and payload
        let header_result = BASE64.decode(parts[0]);
        let payload_result = BASE64.decode(parts[1]);

        match (header_result, payload_result) {
            (Ok(header_bytes), Ok(payload_bytes)) => {
                let header_json: Result<Value, _> = serde_json::from_slice(&header_bytes);
                let payload_json: Result<Value, _> = serde_json::from_slice(&payload_bytes);

                match (header_json, payload_json) {
                    (Ok(header), Ok(payload)) => {
                        let alg = header["alg"].as_str().unwrap_or("unknown");
                        let exp = payload["exp"].as_i64();
                        let iss = payload["iss"].as_str().unwrap_or("unknown");

                        let is_expired = if let Some(exp_timestamp) = exp {
                            chrono::Utc::now().timestamp() > exp_timestamp
                        } else {
                            false
                        };

                        let additional_info = format!(
                            "Algorithm: {}, Issuer: {}, Expired: {}",
                            alg, iss, is_expired
                        );

                        Ok(ValidationResult {
                            secret_hash: String::new(),
                            is_valid: !is_expired,
                            validation_method: "jwt_decode_check".to_string(),
                            error_message: if is_expired { Some("Token is expired".to_string()) } else { None },
                            additional_info: Some(additional_info),
                            validated_at: chrono::Utc::now(),
                        })
                    }
                    _ => Ok(ValidationResult {
                        secret_hash: String::new(),
                        is_valid: false,
                        validation_method: "jwt_decode_check".to_string(),
                        error_message: Some("Invalid JWT JSON structure".to_string()),
                        additional_info: None,
                        validated_at: chrono::Utc::now(),
                    }),
                }
            }
            _ => Ok(ValidationResult {
                secret_hash: String::new(),
                is_valid: false,
                validation_method: "jwt_decode_check".to_string(),
                error_message: Some("Invalid JWT base64 encoding".to_string()),
                additional_info: None,
                validated_at: chrono::Utc::now(),
            }),
        }
    }

    /// Batch validate multiple secrets
    pub async fn validate_secrets_batch(
        &self,
        secrets: &[SecretMatch],
        max_concurrent: usize,
    ) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        
        for chunk in secrets.chunks(max_concurrent) {
            let mut chunk_results = Vec::new();
            
            for secret in chunk {
                match self.validate_secret(secret).await {
                    Ok(result) => chunk_results.push(result),
                    Err(e) => {
                        error!("Validation error for {}: {}", secret.detector_name, e);
                        chunk_results.push(ValidationResult {
                            secret_hash: secret.hash.clone(),
                            is_valid: false,
                            validation_method: "error".to_string(),
                            error_message: Some(e.to_string()),
                            additional_info: None,
                            validated_at: chrono::Utc::now(),
                        });
                    }
                }
                
                // Rate limiting
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            
            results.extend(chunk_results);
        }
        
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::scanner::{SecretCategory, SecretSeverity};

    fn create_test_secret_match(detector_name: &str, matched_text: &str) -> SecretMatch {
        SecretMatch {
            detector_name: detector_name.to_string(),
            matched_text: matched_text.to_string(),
            start_position: 0,
            end_position: matched_text.len(),
            line_number: Some(1),
            filename: Some("test.txt".to_string()),
            entropy: 4.0,
            severity: SecretSeverity::High,
            category: SecretCategory::ApiKey,
            context: "test context".to_string(),
            verified: false,
            hash: "test_hash".to_string(),
        }
    }

    #[tokio::test]
    async fn test_validator_creation() {
        let validator = SecretValidator::new().await;
        assert!(validator.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_validation() {
        let validator = SecretValidator::new().await.unwrap();
        
        // Valid JWT structure (may be expired)
        let jwt_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let secret_match = create_test_secret_match("JWT Token", jwt_token);
        
        let result = validator.validate_secret(&secret_match).await;
        assert!(result.is_ok());
        
        let validation_result = result.unwrap();
        assert_eq!(validation_result.validation_method, "jwt_decode_check");
    }

    #[tokio::test]
    async fn test_invalid_jwt_validation() {
        let validator = SecretValidator::new().await.unwrap();
        
        let invalid_jwt = "not.a.jwt";
        let secret_match = create_test_secret_match("JWT Token", invalid_jwt);
        
        let result = validator.validate_secret(&secret_match).await;
        assert!(result.is_ok());
        
        let validation_result = result.unwrap();
        assert!(!validation_result.is_valid);
        assert!(validation_result.error_message.is_some());
    }

    #[tokio::test]
    async fn test_unsupported_secret_type() {
        let validator = SecretValidator::new().await.unwrap();
        
        let secret_match = create_test_secret_match("Unsupported Secret", "test123");
        
        let result = validator.validate_secret(&secret_match).await;
        assert!(result.is_ok());
        
        let validation_result = result.unwrap();
        assert_eq!(validation_result.validation_method, "unsupported");
        assert!(!validation_result.is_valid);
    }
}
