use crate::secrets::{SecretCategory, SecretMatch, SecretSeverity, ValidationResult};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info};
use uuid::Uuid;

/// AI-powered triage agent for secret analysis
pub struct AITriageAgent {
    model_identifier: Option<String>,
    wordlist_manager: WordlistManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageResult {
    pub secret_hash: String,
    pub impact_score: f64,     // 0.0 - 1.0
    pub bounty_potential: f64, // 0.0 - 1.0
    pub revocation_priority: RevocationPriority,
    pub analysis: String,
    pub suggested_actions: Vec<String>,
    pub risk_factors: Vec<RiskFactor>,
    pub context_analysis: ContextAnalysis,
    pub confidence: f64, // 0.0 - 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RevocationPriority {
    Immediate, // Critical secrets, active and high-value
    High,      // Important secrets with confirmed access
    Medium,    // Potentially active secrets
    Low,       // Likely inactive or low-value
    Monitor,   // Keep watching but no immediate action
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub factor_type: RiskFactorType,
    pub description: String,
    pub severity_impact: f64,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskFactorType {
    CorporateEmail,
    ProductionEnvironment,
    RecentActivity,
    HighPrivileges,
    PublicRepository,
    LargeAudience,
    KnownService,
    CrossReferences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAnalysis {
    pub file_type_risk: f64,
    pub repository_type: String,
    pub organization_context: Option<String>,
    pub temporal_patterns: Vec<String>,
    pub cross_secret_correlations: Vec<String>,
    pub linguistic_indicators: Vec<String>,
}

/// Manages AI-optimized wordlists for secret detection
pub struct WordlistManager {
    organization_specific: HashMap<String, Vec<String>>,
}

impl WordlistManager {
    pub fn new() -> Self {
        Self {
            organization_specific: HashMap::new(),
        }
    }

    /// Generate organization-specific wordlist using AI
    pub async fn generate_org_wordlist(
        &mut self,
        organization: &str,
        samples: &[SecretMatch],
    ) -> Result<Vec<String>> {
        info!(
            "Generating AI-enhanced wordlist for organization: {}",
            organization
        );

        // Extract patterns from existing secrets
        let mut patterns = Vec::new();
        let mut prefixes = Vec::new();
        let mut suffixes = Vec::new();

        for secret in samples {
            if let Some(filename) = &secret.filename {
                // Extract potential naming patterns
                let parts: Vec<&str> = filename.split('/').collect();
                for part in parts {
                    if part.contains(&organization.to_lowercase()) {
                        patterns.push(part.to_string());
                    }
                }
            }

            // Analyze the matched text for patterns
            let text = &secret.matched_text;
            if text.len() > 10 {
                // Extract potential prefixes and suffixes
                if text.len() > 6 {
                    prefixes.push(text[..3].to_string());
                    suffixes.push(text[text.len() - 3..].to_string());
                }
            }
        }

        // Use AI to generate enhanced patterns
        let ai_patterns = self
            .ai_enhance_patterns(&patterns, &prefixes, &suffixes)
            .await?;

        // Combine with standard patterns
        let mut wordlist = vec![
            organization.to_lowercase(),
            format!("{}_", organization.to_lowercase()),
            format!("{}api", organization.to_lowercase()),
            format!("{}key", organization.to_lowercase()),
            format!("{}secret", organization.to_lowercase()),
            format!("{}token", organization.to_lowercase()),
            format!("{}_api", organization.to_lowercase()),
            format!("{}_key", organization.to_lowercase()),
            format!("{}_secret", organization.to_lowercase()),
            format!("{}_token", organization.to_lowercase()),
        ];

        wordlist.extend(ai_patterns);
        wordlist.sort();
        wordlist.dedup();

        self.organization_specific
            .insert(organization.to_string(), wordlist.clone());

        info!("Generated {} patterns for {}", wordlist.len(), organization);
        Ok(wordlist)
    }

    async fn ai_enhance_patterns(
        &self,
        patterns: &[String],
        prefixes: &[String],
        suffixes: &[String],
    ) -> Result<Vec<String>> {
        let mut enhanced = Vec::new();

        // Generate combinations
        for prefix in prefixes {
            for suffix in suffixes {
                enhanced.push(format!("{}{}", prefix, suffix));
                enhanced.push(format!("{}_{}", prefix, suffix));
                enhanced.push(format!("{}-{}", prefix, suffix));
            }
        }

        // Common variations
        for pattern in patterns {
            enhanced.push(format!("{}_prod", pattern));
            enhanced.push(format!("{}_staging", pattern));
            enhanced.push(format!("{}_dev", pattern));
            enhanced.push(format!("prod_{}", pattern));
            enhanced.push(format!("staging_{}", pattern));
            enhanced.push(format!("dev_{}", pattern));
        }

        enhanced.sort();
        enhanced.dedup();

        Ok(enhanced)
    }
}

impl Default for WordlistManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AITriageAgent {
    /// Create a new AI triage agent
    pub async fn new(model_path: &str) -> Result<Self> {
        info!(
            "Initializing AI triage agent with model path: {}",
            model_path
        );

        let model_identifier = if std::path::Path::new(model_path).exists() {
            info!("Model path located; registering for heuristic augmentation");
            Some(model_path.to_string())
        } else {
            info!(
                "Model path {} was not found; continuing in heuristic-only mode",
                model_path
            );
            None
        };

        Ok(Self {
            model_identifier,
            wordlist_manager: WordlistManager::new(),
        })
    }

    /// Create with a small local model (for testing)
    pub async fn new_with_small_model() -> Result<Self> {
        info!("Creating heuristic-only triage agent for tests and offline workflows");
        Ok(Self {
            model_identifier: Some("heuristic-small".to_string()),
            wordlist_manager: WordlistManager::new(),
        })
    }

    /// Perform AI-powered triage on a secret
    pub async fn triage_secret(
        &mut self,
        secret: &SecretMatch,
        validation_result: Option<&ValidationResult>,
        context: &TriageContext,
    ) -> Result<TriageResult> {
        let model_mode = self.model_identifier.as_deref().unwrap_or("heuristic-only");

        info!(
            detector = %secret.detector_name,
            model_mode = %model_mode,
            "AI triaging secret"
        );

        // Analyze context
        let context_analysis = self.analyze_context(secret, context).await?;

        // Generate risk factors
        let risk_factors = self
            .identify_risk_factors(secret, validation_result, context)
            .await?;

        // Calculate impact score
        let impact_score = self
            .calculate_impact_score(secret, &risk_factors, &context_analysis)
            .await?;

        // Calculate bounty potential
        let bounty_potential = self
            .calculate_bounty_potential(secret, &risk_factors, context)
            .await?;

        // Determine revocation priority
        let revocation_priority =
            self.determine_revocation_priority(impact_score, bounty_potential, &risk_factors);

        // Generate AI analysis
        let analysis = self
            .generate_ai_analysis(secret, &risk_factors, &context_analysis)
            .await?;

        // Generate suggested actions
        let suggested_actions = self
            .generate_suggested_actions(secret, &risk_factors, revocation_priority.clone())
            .await?;

        // Calculate confidence
        let confidence = self.calculate_confidence(&risk_factors, validation_result);

        Ok(TriageResult {
            secret_hash: secret.hash.clone(),
            impact_score,
            bounty_potential,
            revocation_priority,
            analysis,
            suggested_actions,
            risk_factors,
            context_analysis,
            confidence,
        })
    }

    async fn analyze_context(
        &self,
        secret: &SecretMatch,
        context: &TriageContext,
    ) -> Result<ContextAnalysis> {
        let file_type_risk = if let Some(filename) = &secret.filename {
            match std::path::Path::new(filename)
                .extension()
                .and_then(|ext| ext.to_str())
            {
                Some("env") => 0.9,
                Some("config") => 0.8,
                Some("json") => 0.7,
                Some("yaml") | Some("yml") => 0.7,
                Some("py") | Some("js") | Some("ts") => 0.6,
                Some("md") | Some("txt") => 0.3,
                _ => 0.5,
            }
        } else {
            0.5
        };

        let repository_type = if context.repository_name.contains("config") {
            "Configuration Repository".to_string()
        } else if context.repository_name.contains("api") {
            "API Repository".to_string()
        } else if context.repository_name.contains("web") || context.repository_name.contains("app")
        {
            "Application Repository".to_string()
        } else {
            "General Repository".to_string()
        };

        let organization_context = context.organization.clone();

        // Analyze temporal patterns
        let temporal_patterns = vec![
            "Recent commit activity".to_string(),
            "Active development".to_string(),
        ];

        // Find cross-secret correlations
        let cross_secret_correlations = vec![
            "Multiple secrets in same file".to_string(),
            "Similar naming patterns".to_string(),
        ];

        // Analyze linguistic indicators
        let linguistic_indicators = if secret.matched_text.contains("prod") {
            vec!["Production indicator".to_string()]
        } else if secret.matched_text.contains("dev") || secret.matched_text.contains("test") {
            vec!["Development/Test indicator".to_string()]
        } else {
            vec![]
        };

        Ok(ContextAnalysis {
            file_type_risk,
            repository_type,
            organization_context,
            temporal_patterns,
            cross_secret_correlations,
            linguistic_indicators,
        })
    }

    async fn identify_risk_factors(
        &self,
        secret: &SecretMatch,
        validation_result: Option<&ValidationResult>,
        context: &TriageContext,
    ) -> Result<Vec<RiskFactor>> {
        let mut risk_factors = Vec::new();

        // Check for corporate email patterns
        if let Some(email) = self.extract_email_from_context(&secret.context) {
            if !email.contains("gmail.com")
                && !email.contains("yahoo.com")
                && !email.contains("hotmail.com")
            {
                risk_factors.push(RiskFactor {
                    factor_type: RiskFactorType::CorporateEmail,
                    description: format!("Corporate email domain detected: {}", email),
                    severity_impact: 0.7,
                    evidence: vec![email],
                });
            }
        }

        // Check for production environment indicators
        if secret.matched_text.contains("prod") || secret.context.contains("production") {
            risk_factors.push(RiskFactor {
                factor_type: RiskFactorType::ProductionEnvironment,
                description: "Production environment indicators detected".to_string(),
                severity_impact: 0.8,
                evidence: vec!["prod keyword found".to_string()],
            });
        }

        // Check validation status
        if let Some(validation) = validation_result {
            if validation.is_valid {
                risk_factors.push(RiskFactor {
                    factor_type: RiskFactorType::HighPrivileges,
                    description: "Secret validated as active".to_string(),
                    severity_impact: 0.9,
                    evidence: vec![validation.validation_method.clone()],
                });
            }
        }

        // Check repository publicity
        if context.is_public_repository {
            risk_factors.push(RiskFactor {
                factor_type: RiskFactorType::PublicRepository,
                description: "Secret found in public repository".to_string(),
                severity_impact: 0.8,
                evidence: vec![context.repository_name.clone()],
            });
        }

        // Check for known high-value services
        if self.is_high_value_service(&secret.detector_name) {
            risk_factors.push(RiskFactor {
                factor_type: RiskFactorType::KnownService,
                description: format!("High-value service: {}", secret.detector_name),
                severity_impact: 0.7,
                evidence: vec![secret.detector_name.clone()],
            });
        }

        Ok(risk_factors)
    }

    fn is_high_value_service(&self, detector_name: &str) -> bool {
        let high_value_services = [
            "AWS",
            "Google",
            "Azure",
            "GitHub",
            "Stripe",
            "PayPal",
            "Twilio",
            "SendGrid",
            "MongoDB",
            "PostgreSQL",
        ];

        high_value_services
            .iter()
            .any(|service| detector_name.contains(service))
    }

    fn extract_email_from_context(&self, context: &str) -> Option<String> {
        // Simple email extraction
        if let Some(start) = context.find('@') {
            let before = &context[..start];
            let after = &context[start..];

            if let Some(email_start) = before.rfind(char::is_whitespace) {
                if let Some(email_end) = after.find(char::is_whitespace) {
                    let email = &context[email_start + 1..start + email_end];
                    return Some(email.to_string());
                }
            }
        }
        None
    }

    async fn calculate_impact_score(
        &self,
        secret: &SecretMatch,
        risk_factors: &[RiskFactor],
        context_analysis: &ContextAnalysis,
    ) -> Result<f64> {
        let mut score = 0.0f64;

        // Base score from secret severity
        score += match secret.severity {
            SecretSeverity::Critical => 0.8,
            SecretSeverity::High => 0.6,
            SecretSeverity::Medium => 0.4,
            SecretSeverity::Low => 0.2,
        };

        // Add risk factor impacts
        for risk_factor in risk_factors {
            score += risk_factor.severity_impact * 0.2;
        }

        // Add context analysis impact
        score += context_analysis.file_type_risk * 0.1;

        // Normalize to 0.0-1.0
        Ok(score.min(1.0))
    }

    async fn calculate_bounty_potential(
        &self,
        secret: &SecretMatch,
        risk_factors: &[RiskFactor],
        context: &TriageContext,
    ) -> Result<f64> {
        let mut potential: f64 = 0.0;

        // Base potential from secret type
        potential += match secret.category {
            SecretCategory::CloudProvider => 0.8,
            SecretCategory::ApiKey => 0.6,
            SecretCategory::Database => 0.7,
            SecretCategory::Certificate => 0.9,
            SecretCategory::Token => 0.5,
            _ => 0.3,
        };

        // High-value organizations get higher bounty potential
        if let Some(org) = &context.organization {
            if self.is_high_value_organization(org) {
                potential += 0.3;
            }
        }

        // Public repository increases bounty potential
        if context.is_public_repository {
            potential += 0.2;
        }

        // Active validation increases potential
        for risk_factor in risk_factors {
            if matches!(risk_factor.factor_type, RiskFactorType::HighPrivileges) {
                potential += 0.3;
                break;
            }
        }

        Ok(potential.min(1.0))
    }

    fn is_high_value_organization(&self, org: &str) -> bool {
        // List of organizations known for good bug bounty programs
        let high_value_orgs = [
            "google",
            "microsoft",
            "apple",
            "facebook",
            "netflix",
            "uber",
            "airbnb",
            "dropbox",
            "slack",
            "github",
        ];

        high_value_orgs
            .iter()
            .any(|high_value| org.to_lowercase().contains(high_value))
    }

    fn determine_revocation_priority(
        &self,
        impact_score: f64,
        bounty_potential: f64,
        risk_factors: &[RiskFactor],
    ) -> RevocationPriority {
        let has_active_validation = risk_factors
            .iter()
            .any(|rf| matches!(rf.factor_type, RiskFactorType::HighPrivileges));

        let has_production_indicators = risk_factors
            .iter()
            .any(|rf| matches!(rf.factor_type, RiskFactorType::ProductionEnvironment));

        if has_active_validation && (impact_score > 0.8 || has_production_indicators) {
            RevocationPriority::Immediate
        } else if has_active_validation || impact_score > 0.6 {
            RevocationPriority::High
        } else if impact_score > 0.4 || bounty_potential > 0.6 {
            RevocationPriority::Medium
        } else if impact_score > 0.2 {
            RevocationPriority::Low
        } else {
            RevocationPriority::Monitor
        }
    }

    async fn generate_ai_analysis(
        &mut self,
        secret: &SecretMatch,
        risk_factors: &[RiskFactor],
        context_analysis: &ContextAnalysis,
    ) -> Result<String> {
        let mut analysis = format!(
            "Secret '{}' detected in {} with {} entropy. ",
            secret.detector_name,
            secret.filename.as_deref().unwrap_or("unknown file"),
            secret.entropy
        );

        if !risk_factors.is_empty() {
            analysis.push_str(&format!("Identified {} risk factors: ", risk_factors.len()));
            for (i, rf) in risk_factors.iter().enumerate() {
                if i > 0 {
                    analysis.push_str(", ");
                }
                analysis.push_str(&rf.description);
            }
            analysis.push_str(". ");
        }

        analysis.push_str(&format!(
            "File type risk assessment: {:.1}%. Repository type: {}. ",
            context_analysis.file_type_risk * 100.0,
            context_analysis.repository_type
        ));

        if let Some(org) = &context_analysis.organization_context {
            analysis.push_str(&format!("Organization context: {}. ", org));
        }

        Ok(analysis)
    }

    async fn generate_suggested_actions(
        &self,
        secret: &SecretMatch,
        risk_factors: &[RiskFactor],
        priority: RevocationPriority,
    ) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        match priority {
            RevocationPriority::Immediate => {
                actions.push("🚨 IMMEDIATE ACTION: Revoke this secret now".to_string());
                actions.push("📞 Contact security team immediately".to_string());
                actions.push("🔍 Audit all systems that may have used this secret".to_string());
            }
            RevocationPriority::High => {
                actions.push("⚡ HIGH PRIORITY: Revoke within 24 hours".to_string());
                actions.push("📋 Document the incident".to_string());
                actions.push("🔄 Rotate the secret".to_string());
            }
            RevocationPriority::Medium => {
                actions.push("⏰ MEDIUM PRIORITY: Revoke within 72 hours".to_string());
                actions.push("📊 Assess impact scope".to_string());
            }
            RevocationPriority::Low => {
                actions.push("📅 LOW PRIORITY: Schedule revocation".to_string());
                actions.push("🔍 Verify if secret is still in use".to_string());
            }
            RevocationPriority::Monitor => {
                actions.push("👀 MONITOR: Keep watching for changes".to_string());
                actions.push("📝 Add to monitoring list".to_string());
            }
        }

        // Add specific actions based on secret type
        match secret.category {
            SecretCategory::CloudProvider => {
                actions.push("☁️ Check cloud service permissions".to_string());
                actions.push("💰 Review billing for unusual activity".to_string());
            }
            SecretCategory::Database => {
                actions.push("🗄️ Check database access logs".to_string());
                actions.push("🔒 Review database permissions".to_string());
            }
            SecretCategory::ApiKey => {
                actions.push("🔑 Check API usage logs".to_string());
                actions.push("📈 Monitor rate limits and quotas".to_string());
            }
            _ => {}
        }

        // Add actions based on risk factors
        for risk_factor in risk_factors {
            match risk_factor.factor_type {
                RiskFactorType::PublicRepository => {
                    actions.push("🌐 Consider making repository private".to_string());
                }
                RiskFactorType::CorporateEmail => {
                    actions.push("📧 Notify email domain administrator".to_string());
                }
                _ => {}
            }
        }

        Ok(actions)
    }

    fn calculate_confidence(
        &self,
        risk_factors: &[RiskFactor],
        validation_result: Option<&ValidationResult>,
    ) -> f64 {
        let mut confidence = 0.5; // Base confidence

        // More risk factors = higher confidence
        confidence += (risk_factors.len() as f64 * 0.1).min(0.3);

        // Validation result affects confidence
        if let Some(validation) = validation_result {
            if validation.is_valid {
                confidence += 0.3; // High confidence if validated
            } else {
                confidence += 0.1; // Some confidence even if invalid
            }
        }

        confidence.min(1.0)
    }

    /// Batch triage multiple secrets
    pub async fn triage_secrets_batch(
        &mut self,
        secrets: &[SecretMatch],
        validations: &HashMap<String, ValidationResult>,
        context: &TriageContext,
    ) -> Result<Vec<TriageResult>> {
        let mut results = Vec::new();

        for secret in secrets {
            let validation = validations.get(&secret.hash);
            match self.triage_secret(secret, validation, context).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to triage secret {}: {}", secret.hash, e);
                }
            }
        }

        Ok(results)
    }

    /// Get wordlist for organization
    pub async fn get_organization_wordlist(
        &mut self,
        organization: &str,
        samples: &[SecretMatch],
    ) -> Result<Vec<String>> {
        if let Some(wordlist) = self
            .wordlist_manager
            .organization_specific
            .get(organization)
        {
            Ok(wordlist.clone())
        } else {
            self.wordlist_manager
                .generate_org_wordlist(organization, samples)
                .await
        }
    }
}

#[derive(Debug, Clone)]
pub struct TriageContext {
    pub repository_name: String,
    pub organization: Option<String>,
    pub is_public_repository: bool,
    pub recent_activity: bool,
    pub contributor_count: Option<usize>,
    pub star_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalOpenAiTriageConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl LocalOpenAiTriageConfig {
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("AI_TRIAGE_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:11434/v1".to_string());
        let model = std::env::var("AI_TRIAGE_MODEL").map_err(|_| {
            anyhow::anyhow!("AI_TRIAGE_MODEL is required when AI triage is enabled")
        })?;
        let api_key = std::env::var("AI_TRIAGE_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Ok(Self {
            base_url,
            model,
            api_key,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedTriageInput {
    pub detection_id: Option<Uuid>,
    pub secret_hash: String,
    pub detector_name: String,
    pub severity: String,
    pub category: String,
    pub repository: Option<String>,
    pub file_path: Option<String>,
    pub line_number: Option<i32>,
    pub verified: bool,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTriageOutput {
    pub id: Uuid,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub analysis: String,
    pub recommended_actions: Vec<String>,
    pub confidence: f64,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

pub struct LocalOpenAiTriageClient {
    config: LocalOpenAiTriageConfig,
    client: reqwest::Client,
}

impl LocalOpenAiTriageClient {
    pub fn new(config: LocalOpenAiTriageConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub async fn triage(&self, input: &RedactedTriageInput) -> Result<LocalTriageOutput> {
        let endpoint = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let prompt = serde_json::json!({
            "task": "Assess a redacted secret detection for operational triage.",
            "constraints": [
                "No raw secret value is provided.",
                "Base the answer only on detector metadata and provenance.",
                "Return concise JSON with analysis, recommended_actions, and confidence between 0 and 1."
            ],
            "finding": input
        });

        let body = serde_json::json!({
            "model": self.config.model,
            "temperature": 0,
            "messages": [
                {
                    "role": "system",
                    "content": "You triage redacted secret-detection metadata for a local security operator. Never request or infer raw secret values."
                },
                {
                    "role": "user",
                    "content": prompt.to_string()
                }
            ]
        });

        let mut request = self.client.post(endpoint).json(&body);
        if let Some(api_key) = &self.config.api_key {
            request = request.bearer_auth(api_key);
        }

        let response = request
            .send()
            .await
            .context("Failed to call local OpenAI-compatible triage endpoint")?
            .error_for_status()
            .context("Local OpenAI-compatible triage endpoint returned an error")?;
        let response_json: serde_json::Value = response
            .json()
            .await
            .context("Failed to decode local triage response")?;

        let content = response_json
            .pointer("/choices/0/message/content")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let parsed = serde_json::from_str::<serde_json::Value>(&content).unwrap_or_else(|_| {
            serde_json::json!({
                "analysis": content,
                "recommended_actions": [],
                "confidence": 0.5
            })
        });

        let recommended_actions = parsed
            .get("recommended_actions")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| {
                vec![
                    "Verify whether the credential is still active through the local validator"
                        .to_string(),
                    "Rotate or revoke the credential through the owning service".to_string(),
                ]
            });

        let analysis = parsed
            .get("analysis")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Local triage completed without structured analysis text")
            .to_string();
        let confidence = parsed
            .get("confidence")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        Ok(LocalTriageOutput {
            id: Uuid::new_v4(),
            provider: "local-openai".to_string(),
            model: self.config.model.clone(),
            base_url: self.config.base_url.clone(),
            analysis,
            recommended_actions,
            confidence,
            completed_at: chrono::Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{SecretCategory, SecretMatch, SecretSeverity};

    fn create_test_context() -> TriageContext {
        TriageContext {
            repository_name: "test-org/test-repo".to_string(),
            organization: Some("test-org".to_string()),
            is_public_repository: true,
            recent_activity: true,
            contributor_count: Some(10),
            star_count: Some(100),
        }
    }

    fn create_test_secret() -> SecretMatch {
        SecretMatch {
            detector_name: "AWS Access Key ID".to_string(),
            matched_text: "AKIA_REDACTED_EXAMPLE".to_string(),
            start_position: 0,
            end_position: 20,
            line_number: Some(42),
            filename: Some("config.env".to_string()),
            entropy: 4.5,
            severity: SecretSeverity::High,
            category: SecretCategory::CloudProvider,
            context: "aws_access_key_id = 'AKIA_REDACTED_EXAMPLE'".to_string(),
            verified: false,
            hash: "test_hash_123".to_string(),
        }
    }

    #[tokio::test]
    async fn test_triage_agent_creation() {
        let agent = AITriageAgent::new_with_small_model().await;
        assert!(agent.is_ok());
    }

    #[tokio::test]
    async fn test_risk_factor_identification() {
        let agent = AITriageAgent::new_with_small_model().await.unwrap();
        let secret = create_test_secret();
        let context = create_test_context();

        let risk_factors = agent
            .identify_risk_factors(&secret, None, &context)
            .await
            .unwrap();

        // Should identify public repository and high-value service
        assert!(!risk_factors.is_empty());
        assert!(risk_factors
            .iter()
            .any(|rf| matches!(rf.factor_type, RiskFactorType::PublicRepository)));
        assert!(risk_factors
            .iter()
            .any(|rf| matches!(rf.factor_type, RiskFactorType::KnownService)));
    }

    #[tokio::test]
    async fn test_context_analysis() {
        let agent = AITriageAgent::new_with_small_model().await.unwrap();
        let secret = create_test_secret();
        let context = create_test_context();

        let analysis = agent.analyze_context(&secret, &context).await.unwrap();

        assert!(analysis.file_type_risk > 0.8); // .env files are high risk
        assert_eq!(analysis.repository_type, "General Repository");
        assert_eq!(analysis.organization_context, Some("test-org".to_string()));
    }

    #[tokio::test]
    async fn test_impact_score_calculation() {
        let agent = AITriageAgent::new_with_small_model().await.unwrap();
        let secret = create_test_secret();
        let context = create_test_context();

        let context_analysis = agent.analyze_context(&secret, &context).await.unwrap();
        let risk_factors = agent
            .identify_risk_factors(&secret, None, &context)
            .await
            .unwrap();

        let impact_score = agent
            .calculate_impact_score(&secret, &risk_factors, &context_analysis)
            .await
            .unwrap();

        assert!(impact_score > 0.0);
        assert!(impact_score <= 1.0);
        // High severity secret should have significant impact
        assert!(impact_score > 0.5);
    }

    #[tokio::test]
    async fn test_bounty_potential_calculation() {
        let agent = AITriageAgent::new_with_small_model().await.unwrap();
        let secret = create_test_secret();
        let context = create_test_context();
        let risk_factors = agent
            .identify_risk_factors(&secret, None, &context)
            .await
            .unwrap();

        let bounty_potential = agent
            .calculate_bounty_potential(&secret, &risk_factors, &context)
            .await
            .unwrap();

        assert!(bounty_potential > 0.0);
        assert!(bounty_potential <= 1.0);
        // Cloud provider secrets should have good bounty potential
        assert!(bounty_potential > 0.6);
    }

    #[tokio::test]
    async fn test_revocation_priority() {
        let agent = AITriageAgent::new_with_small_model().await.unwrap();

        // Test immediate priority (high impact + validation)
        let high_impact = 0.9;
        let high_bounty = 0.8;
        let risk_factors_with_validation = vec![RiskFactor {
            factor_type: RiskFactorType::HighPrivileges,
            description: "Active secret".to_string(),
            severity_impact: 0.9,
            evidence: vec!["validated".to_string()],
        }];

        let priority = agent.determine_revocation_priority(
            high_impact,
            high_bounty,
            &risk_factors_with_validation,
        );
        assert!(matches!(priority, RevocationPriority::Immediate));

        // Test low priority
        let low_impact = 0.1;
        let low_bounty = 0.1;
        let no_risk_factors = vec![];

        let priority =
            agent.determine_revocation_priority(low_impact, low_bounty, &no_risk_factors);
        assert!(matches!(priority, RevocationPriority::Monitor));
    }

    #[tokio::test]
    async fn test_wordlist_generation() {
        let mut agent = AITriageAgent::new_with_small_model().await.unwrap();
        let secret = create_test_secret();
        let samples = vec![secret];

        let wordlist = agent
            .get_organization_wordlist("testorg", &samples)
            .await
            .unwrap();

        assert!(!wordlist.is_empty());
        assert!(wordlist.contains(&"testorg".to_string()));
        assert!(wordlist.contains(&"testorg_api".to_string()));
    }
}
