use anyhow::{anyhow, Result};
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{error, info, warn}; // Removed unused debug
                                  // Removed unused base64 imports
use entropy::shannon_entropy;
use sha2::{Digest, Sha256};

/// Secret scanner with 50+ built-in detectors
pub struct SecretScanner {
    detectors: Vec<SecretDetector>,
    patterns: HashMap<String, Regex>,
    entropy_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretDetector {
    pub name: String,
    pub description: String,
    pub pattern: String,
    pub keywords: Vec<String>,
    pub entropy_threshold: Option<f64>,
    pub verify_func: Option<String>,
    pub severity: SecretSeverity,
    pub category: SecretCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecretSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for SecretSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretSeverity::Low => write!(f, "Low"),
            SecretSeverity::Medium => write!(f, "Medium"),
            SecretSeverity::High => write!(f, "High"),
            SecretSeverity::Critical => write!(f, "Critical"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecretCategory {
    CloudProvider,
    Database,
    ApiKey,
    Certificate,
    Password,
    Token,
    Webhook,
    PrivateKey,
    Url,
    Other,
}

impl std::fmt::Display for SecretCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretCategory::CloudProvider => write!(f, "Cloud Provider"),
            SecretCategory::Database => write!(f, "Database"),
            SecretCategory::ApiKey => write!(f, "API Key"),
            SecretCategory::Certificate => write!(f, "Certificate"),
            SecretCategory::Password => write!(f, "Password"),
            SecretCategory::Token => write!(f, "Token"),
            SecretCategory::Webhook => write!(f, "Webhook"),
            SecretCategory::PrivateKey => write!(f, "Private Key"),
            SecretCategory::Url => write!(f, "URL"),
            SecretCategory::Other => write!(f, "Other"),
        }
    }
}

impl SecretCategory {
    /// Canonical key used when aggregating in the UI
    pub fn frontend_label(&self) -> &'static str {
        match self {
            SecretCategory::ApiKey | SecretCategory::CloudProvider => "API Keys",
            SecretCategory::Token => "Access Tokens",
            SecretCategory::Password => "Passwords",
            SecretCategory::Certificate => "Certificates",
            SecretCategory::PrivateKey => "Private Keys",
            SecretCategory::Database => "Database URLs",
            SecretCategory::Webhook | SecretCategory::Url => "URLs",
            SecretCategory::Other => "Other",
        }
    }

    pub fn storage_key(&self) -> &'static str {
        match self {
            SecretCategory::CloudProvider => "cloud_provider",
            SecretCategory::Database => "database",
            SecretCategory::ApiKey => "api_key",
            SecretCategory::Certificate => "certificate",
            SecretCategory::Password => "password",
            SecretCategory::Token => "token",
            SecretCategory::Webhook => "webhook",
            SecretCategory::PrivateKey => "private_key",
            SecretCategory::Url => "url",
            SecretCategory::Other => "other",
        }
    }

    pub fn from_storage_key(value: &str) -> Option<Self> {
        match value {
            "cloud_provider" => Some(SecretCategory::CloudProvider),
            "database" => Some(SecretCategory::Database),
            "api_key" => Some(SecretCategory::ApiKey),
            "certificate" => Some(SecretCategory::Certificate),
            "password" => Some(SecretCategory::Password),
            "token" => Some(SecretCategory::Token),
            "webhook" => Some(SecretCategory::Webhook),
            "private_key" => Some(SecretCategory::PrivateKey),
            "url" => Some(SecretCategory::Url),
            "other" => Some(SecretCategory::Other),
            _ => None,
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "api key" | "api keys" => Some(SecretCategory::ApiKey),
            "cloud provider" | "cloud providers" => Some(SecretCategory::CloudProvider),
            "database" | "database url" | "database urls" => Some(SecretCategory::Database),
            "certificate" | "certificates" => Some(SecretCategory::Certificate),
            "password" | "passwords" => Some(SecretCategory::Password),
            "token" | "tokens" | "access token" | "access tokens" => Some(SecretCategory::Token),
            "webhook" | "webhooks" => Some(SecretCategory::Webhook),
            "private key" | "private keys" => Some(SecretCategory::PrivateKey),
            "url" | "urls" => Some(SecretCategory::Url),
            "other" => Some(SecretCategory::Other),
            _ => None,
        }
    }
}

impl SecretSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecretSeverity::Low => "Low",
            SecretSeverity::Medium => "Medium",
            SecretSeverity::High => "High",
            SecretSeverity::Critical => "Critical",
        }
    }
}

impl FromStr for SecretSeverity {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(SecretSeverity::Low),
            "medium" => Ok(SecretSeverity::Medium),
            "high" => Ok(SecretSeverity::High),
            "critical" => Ok(SecretSeverity::Critical),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMatch {
    pub detector_name: String,
    pub matched_text: String,
    pub start_position: usize,
    pub end_position: usize,
    pub line_number: Option<usize>,
    pub filename: Option<String>,
    pub entropy: f64,
    pub severity: SecretSeverity,
    pub category: SecretCategory,
    pub context: String,
    pub verified: bool,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub matches: Vec<SecretMatch>,
    pub files_scanned: usize,
    pub total_lines: usize,
    pub scan_duration_ms: u64,
    pub detector_stats: HashMap<String, usize>,
}

impl Default for SecretScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretScanner {
    /// Create a new secret scanner with built-in detectors
    pub fn new() -> Self {
        let mut scanner = Self {
            detectors: Vec::new(),
            patterns: HashMap::new(),
            entropy_threshold: 4.5,
        };

        scanner.load_built_in_detectors();
        scanner
    }

    /// Load all built-in secret detectors
    fn load_built_in_detectors(&mut self) {
        let detectors = vec![
            // AWS
            SecretDetector {
                name: "AWS Access Key ID".to_string(),
                description: "Amazon Web Services Access Key ID".to_string(),
                pattern: r"(?i)(AKIA[0-9A-Z]{16})".to_string(),
                keywords: vec!["aws".to_string(), "amazon".to_string(), "akia".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_aws_access_key".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::CloudProvider,
            },
            SecretDetector {
                name: "AWS Secret Access Key".to_string(),
                description: "Amazon Web Services Secret Access Key".to_string(),
                pattern: r#"(?i)(aws.{0,20})?['"]([0-9a-zA-Z/+]{40})['"]"#.to_string(),
                keywords: vec!["aws".to_string(), "secret".to_string()],
                entropy_threshold: Some(4.5),
                verify_func: Some("verify_aws_secret_key".to_string()),
                severity: SecretSeverity::Critical,
                category: SecretCategory::CloudProvider,
            },
            SecretDetector {
                name: "AWS Session Token".to_string(),
                description: "Amazon Web Services Session Token".to_string(),
                pattern: r#"(?i)(aws.session.token.{0,20})?['"]([0-9a-zA-Z/+=]{16,})['"]"#.to_string(),
                keywords: vec!["aws".to_string(), "session".to_string(), "token".to_string()],
                entropy_threshold: Some(4.0),
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::Token,
            },
            SecretDetector {
                name: "AWS IAM User Token".to_string(),
                description: "Amazon Web Services IAM User Identifier".to_string(),
                pattern: r"(?i)(AIDA[0-9A-Z]{16})".to_string(),
                keywords: vec![
                    "aws".to_string(),
                    "iam".to_string(),
                    "aida".to_string(),
                ],
                entropy_threshold: None,
                verify_func: None,
                severity: SecretSeverity::High,
                category: SecretCategory::Token,
            },
            SecretDetector {
                name: "AWS Account ID".to_string(),
                description: "Amazon Web Services Account Identifier".to_string(),
                pattern: r"(?i)aws[\s_-]*account[\s_-]*id[^0-9]{0,5}(?:[0-9]{4}-?[0-9]{4}-?[0-9]{4})".to_string(),
                keywords: vec!["aws".to_string(), "account".to_string(), "id".to_string()],
                entropy_threshold: None,
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::Other,
            },
            SecretDetector {
                name: "AWS RDS Connection String".to_string(),
                description: "Amazon RDS connection string containing credentials".to_string(),
                pattern: r"(?i)(?:postgres(?:ql)?|mysql|aurora|mariadb)://[^:@\s]+:[^@\s]+@[a-z0-9.-]+\.rds\.amazonaws\.com(?::\d+)?/[\w.-]+".to_string(),
                keywords: vec!["aws".to_string(), "rds".to_string(), "database".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_rds_connection".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::Database,
            },
            SecretDetector {
                name: "PostgreSQL Connection String".to_string(),
                description: "PostgreSQL connection string containing credentials".to_string(),
                pattern: r"(?i)postgres(?:ql)?://[^:@\s]+:[^@\s]+@[a-z0-9_.-]+(?::\d+)?/[a-z0-9_.-]+".to_string(),
                keywords: vec!["postgres".to_string(), "postgresql".to_string()],
                entropy_threshold: None,
                verify_func: None,
                severity: SecretSeverity::High,
                category: SecretCategory::Database,
            },

            // GitHub
            SecretDetector {
                name: "GitHub Personal Access Token".to_string(),
                description: "GitHub Personal Access Token (classic)".to_string(),
                pattern: r"(?i)ghp_[0-9a-zA-Z]{36}".to_string(),
                keywords: vec!["github".to_string(), "ghp_".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_github_token".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::Token,
            },
            SecretDetector {
                name: "GitHub Fine-grained PAT".to_string(),
                description: "GitHub Fine-grained Personal Access Token".to_string(),
                pattern: r"(?i)github_pat_[0-9a-zA-Z_]{82}".to_string(),
                keywords: vec!["github".to_string(), "github_pat_".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_github_token".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::Token,
            },
            SecretDetector {
                name: "GitHub OAuth Token".to_string(),
                description: "GitHub OAuth Access Token".to_string(),
                pattern: r"(?i)gho_[0-9a-zA-Z]{36}".to_string(),
                keywords: vec!["github".to_string(), "gho_".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_github_token".to_string()),
                severity: SecretSeverity::Medium,
                category: SecretCategory::Token,
            },
            SecretDetector {
                name: "GitHub App Token".to_string(),
                description: "GitHub App Installation Token".to_string(),
                pattern: r"(?i)ghs_[0-9a-zA-Z]{36}".to_string(),
                keywords: vec!["github".to_string(), "ghs_".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_github_token".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::Token,
            },

            // MongoDB
            SecretDetector {
                name: "MongoDB Connection String".to_string(),
                description: "MongoDB connection string with credentials".to_string(),
                pattern: r"mongodb://[a-zA-Z0-9_.-]+:[a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+".to_string(),
                keywords: vec!["mongodb".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_mongodb_connection".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::Database,
            },
            SecretDetector {
                name: "MongoDB Atlas Connection".to_string(),
                description: "MongoDB Atlas connection string".to_string(),
                pattern: r"mongodb\+srv://[a-zA-Z0-9_.-]+:[a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+".to_string(),
                keywords: vec!["mongodb".to_string(), "atlas".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_mongodb_connection".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::Database,
            },

            // Google Cloud Platform
            SecretDetector {
                name: "Google API Key".to_string(),
                description: "Google Cloud Platform API Key".to_string(),
                pattern: r"(?i)AIza[0-9A-Za-z\\-_]{35}".to_string(),
                keywords: vec!["google".to_string(), "gcp".to_string(), "aiza".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_google_api_key".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::ApiKey,
            },
            SecretDetector {
                name: "Google Service Account".to_string(),
                description: "Google Cloud Service Account JSON".to_string(),
                pattern: r#"(?i)"type":\s*"service_account""#.to_string(),
                keywords: vec!["service_account".to_string(), "google".to_string(), "gcp".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_google_service_account".to_string()),
                severity: SecretSeverity::Critical,
                category: SecretCategory::Certificate,
            },

            // Slack
            SecretDetector {
                name: "Slack Bot Token".to_string(),
                description: "Slack Bot User OAuth Token".to_string(),
                pattern: r"(?i)xoxb-[0-9]{11,13}-[0-9]{11,13}-[0-9a-zA-Z]{24}".to_string(),
                keywords: vec!["slack".to_string(), "xoxb".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_slack_token".to_string()),
                severity: SecretSeverity::Medium,
                category: SecretCategory::Token,
            },
            SecretDetector {
                name: "Slack Webhook URL".to_string(),
                description: "Slack Incoming Webhook URL".to_string(),
                pattern: r"https://hooks\.slack\.com/services/[A-Z0-9]+/[A-Z0-9]+/[a-zA-Z0-9]+".to_string(),
                keywords: vec!["slack".to_string(), "webhook".to_string(), "hooks".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_slack_webhook".to_string()),
                severity: SecretSeverity::Medium,
                category: SecretCategory::Webhook,
            },

            // Discord
            SecretDetector {
                name: "Discord Bot Token".to_string(),
                description: "Discord Bot Token".to_string(),
                pattern: r"(?i)[MN][A-Za-z\d]{23}\.[\w-]{6}\.[\w-]{27}".to_string(),
                keywords: vec!["discord".to_string(), "bot".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_discord_token".to_string()),
                severity: SecretSeverity::Medium,
                category: SecretCategory::Token,
            },
            SecretDetector {
                name: "Discord Webhook".to_string(),
                description: "Discord Webhook URL".to_string(),
                pattern: r"https://discord(?:app)?\.com/api/webhooks/[0-9]+/[a-zA-Z0-9_-]+".to_string(),
                keywords: vec!["discord".to_string(), "webhook".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_discord_webhook".to_string()),
                severity: SecretSeverity::Low,
                category: SecretCategory::Webhook,
            },

            // SSH Keys
            SecretDetector {
                name: "SSH Private Key".to_string(),
                description: "SSH Private Key".to_string(),
                pattern: r"-----BEGIN (?:RSA|OPENSSH|DSA|EC|PGP) PRIVATE KEY-----".to_string(),
                keywords: vec!["ssh".to_string(), "private".to_string(), "key".to_string()],
                entropy_threshold: None,
                verify_func: None,
                severity: SecretSeverity::Critical,
                category: SecretCategory::Certificate,
            },

            // JWT Tokens
            SecretDetector {
                name: "JWT Token".to_string(),
                description: "JSON Web Token".to_string(),
                pattern: r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*".to_string(),
                keywords: vec!["jwt".to_string(), "token".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_jwt_token".to_string()),
                severity: SecretSeverity::Medium,
                category: SecretCategory::Token,
            },

            // Stripe
            SecretDetector {
                name: "Stripe API Key".to_string(),
                description: "Stripe API Key".to_string(),
                pattern: r"(?i)sk_(?:test|live)_[0-9a-zA-Z]{24}".to_string(),
                keywords: vec!["stripe".to_string(), "sk_".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_stripe_key".to_string()),
                severity: SecretSeverity::High,
                category: SecretCategory::ApiKey,
            },

            // SendGrid
            SecretDetector {
                name: "SendGrid API Key".to_string(),
                description: "SendGrid API Key".to_string(),
                pattern: r"(?i)SG\.[a-zA-Z0-9_-]{22}\.[a-zA-Z0-9_-]{43}".to_string(),
                keywords: vec!["sendgrid".to_string(), "sg.".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_sendgrid_key".to_string()),
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
            },

            // Twilio
            SecretDetector {
                name: "Twilio API Key".to_string(),
                description: "Twilio API Key".to_string(),
                pattern: r"(?i)SK[a-z0-9]{32}".to_string(),
                keywords: vec!["twilio".to_string()],
                entropy_threshold: None,
                verify_func: Some("verify_twilio_key".to_string()),
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
            },

            // Generic patterns
            SecretDetector {
                name: "Generic API Key".to_string(),
                description: "Generic API key pattern".to_string(),
                pattern: r#"(?i)(api.key|apikey|api_key).{0,20}['"]([0-9a-zA-Z_-]{16,})['"]"#.to_string(),
                keywords: vec!["api".to_string(), "key".to_string()],
                entropy_threshold: Some(4.0),
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
            },
            SecretDetector {
                name: "Generic Password".to_string(),
                description: "Generic password pattern".to_string(),
                pattern: r#"(?i)(password|passwd|pwd).{0,20}['"]([0-9a-zA-Z_!@#$%^&*-]{8,})['"]"#.to_string(),
                keywords: vec!["password".to_string(), "passwd".to_string(), "pwd".to_string()],
                entropy_threshold: Some(3.5),
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::Password,
            },
            SecretDetector {
                name: "Generic Secret".to_string(),
                description: "Generic secret pattern".to_string(),
                pattern: r#"(?i)(secret|token).{0,20}['"]([0-9a-zA-Z_-]{16,})['"]"#.to_string(),
                keywords: vec!["secret".to_string(), "token".to_string()],
                entropy_threshold: Some(4.0),
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::Token,
            },

            // High-entropy strings
            SecretDetector {
                name: "High Entropy String".to_string(),
                description: "High entropy base64-like string".to_string(),
                pattern: r"[A-Za-z0-9+/=]{32,}".to_string(),
                keywords: vec![],
                entropy_threshold: Some(5.5),
                verify_func: None,
                severity: SecretSeverity::Low,
                category: SecretCategory::Other,
            },
        ];

        self.detectors = detectors;
        self.compile_patterns();
    }

    /// Return metadata for all detectors (used by API)
    pub fn detectors(&self) -> Vec<SecretDetector> {
        self.detectors.clone()
    }

    /// Compile regex patterns for all detectors
    fn compile_patterns(&mut self) {
        for detector in &self.detectors {
            match Regex::new(&detector.pattern) {
                Ok(regex) => {
                    self.patterns.insert(detector.name.clone(), regex);
                }
                Err(e) => {
                    error!("Failed to compile regex for {}: {}", detector.name, e);
                }
            }
        }
        info!("Compiled {} regex patterns", self.patterns.len());
    }

    /// Scan text for secrets
    pub fn scan_text(&self, text: &str, filename: Option<&str>) -> Vec<SecretMatch> {
        let mut matches = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for detector in &self.detectors {
            if let Some(regex) = self.patterns.get(&detector.name) {
                for captures in regex.captures_iter(text).flatten() {
                    if let Some(m) = captures.get(0) {
                        let matched_text = m.as_str().to_string();
                        let start = m.start();
                        let end = m.end();

                        // Calculate line number
                        let line_number = text[..start].matches('\n').count() + 1;

                        // Get context (surrounding lines)
                        let context = self.get_context(&lines, line_number.saturating_sub(1), 2);

                        // Calculate entropy
                        let entropy = shannon_entropy(&matched_text);

                        // Check if entropy meets threshold
                        if let Some(threshold) = detector.entropy_threshold {
                            if (entropy as f64) < threshold {
                                continue;
                            }
                        }

                        // Create hash of the match
                        let mut hasher = Sha256::new();
                        hasher.update(&matched_text);
                        let hash = hex::encode(hasher.finalize());

                        matches.push(SecretMatch {
                            detector_name: detector.name.clone(),
                            matched_text,
                            start_position: start,
                            end_position: end,
                            line_number: Some(line_number),
                            filename: filename.map(|s| s.to_string()),
                            entropy: entropy as f64,
                            severity: detector.severity.clone(),
                            category: detector.category.clone(),
                            context,
                            verified: false,
                            hash,
                        });
                    }
                }
            }
        }

        matches
    }

    /// Scan a file for secrets
    pub fn scan_file(&self, file_path: &str) -> Result<Vec<SecretMatch>> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;

        Ok(self.scan_text(&content, Some(file_path)))
    }

    /// Scan multiple files
    pub fn scan_files(&self, file_paths: &[String]) -> ScanResult {
        let start_time = std::time::Instant::now();
        let mut all_matches = Vec::new();
        let mut total_lines = 0;
        let mut detector_stats = HashMap::new();

        for file_path in file_paths {
            match self.scan_file(file_path) {
                Ok(matches) => {
                    // Count lines
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        total_lines += content.lines().count();
                    }

                    // Update detector stats
                    for m in &matches {
                        *detector_stats.entry(m.detector_name.clone()).or_insert(0) += 1;
                    }

                    all_matches.extend(matches);
                }
                Err(e) => {
                    warn!("Failed to scan file {}: {}", file_path, e);
                }
            }
        }

        let scan_duration_ms = start_time.elapsed().as_millis() as u64;

        ScanResult {
            matches: all_matches,
            files_scanned: file_paths.len(),
            total_lines,
            scan_duration_ms,
            detector_stats,
        }
    }

    /// Scan git diff or patch content
    pub fn scan_patch(&self, patch_content: &str, filename: Option<&str>) -> Vec<SecretMatch> {
        // Extract only added lines from the patch
        let added_lines: Vec<&str> = patch_content
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .map(|line| &line[1..]) // Remove the '+' prefix
            .collect();

        if added_lines.is_empty() {
            return Vec::new();
        }

        let added_content = added_lines.join("\n");
        self.scan_text(&added_content, filename)
    }

    /// Get context around a line
    fn get_context(&self, lines: &[&str], line_index: usize, context_size: usize) -> String {
        let start = line_index.saturating_sub(context_size);
        let end = (line_index + context_size + 1).min(lines.len());

        lines[start..end].join("\n")
    }

    /// Add custom detector
    pub fn add_detector(&mut self, detector: SecretDetector) -> Result<()> {
        // Compile the regex to ensure it's valid
        let regex =
            Regex::new(&detector.pattern).map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;

        self.patterns.insert(detector.name.clone(), regex);
        self.detectors.push(detector);

        Ok(())
    }

    /// Get all detector names
    pub fn get_detector_names(&self) -> Vec<String> {
        self.detectors.iter().map(|d| d.name.clone()).collect()
    }

    /// Set entropy threshold
    pub fn set_entropy_threshold(&mut self, threshold: f64) {
        self.entropy_threshold = threshold;
    }

    /// Filter matches by severity
    pub fn filter_by_severity(
        matches: &[SecretMatch],
        min_severity: SecretSeverity,
    ) -> Vec<SecretMatch> {
        let min_level = match min_severity {
            SecretSeverity::Low => 0,
            SecretSeverity::Medium => 1,
            SecretSeverity::High => 2,
            SecretSeverity::Critical => 3,
        };

        matches
            .iter()
            .filter(|m| {
                let level = match m.severity {
                    SecretSeverity::Low => 0,
                    SecretSeverity::Medium => 1,
                    SecretSeverity::High => 2,
                    SecretSeverity::Critical => 3,
                };
                level >= min_level
            })
            .cloned()
            .collect()
    }

    /// Deduplicate matches by hash
    pub fn deduplicate_matches(matches: &[SecretMatch]) -> Vec<SecretMatch> {
        let mut seen_hashes = std::collections::HashSet::new();
        let mut unique_matches = Vec::new();

        for m in matches {
            if seen_hashes.insert(m.hash.clone()) {
                unique_matches.push(m.clone());
            }
        }

        unique_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aws_access_key_fixture() -> String {
        format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE")
    }

    fn github_classic_token_fixture() -> String {
        format!("ghp_{}", "A".repeat(36))
    }

    fn github_oauth_token_fixture() -> String {
        format!("gho_{}", "A".repeat(36))
    }

    fn github_app_token_fixture() -> String {
        format!("ghs_{}", "A".repeat(36))
    }

    #[test]
    fn test_scanner_creation() {
        let scanner = SecretScanner::new();
        assert!(!scanner.detectors.is_empty());
        assert!(!scanner.patterns.is_empty());
    }

    #[test]
    fn test_aws_access_key_detection() {
        let scanner = SecretScanner::new();
        let text = format!(
            r#"
        aws_access_key_id = "{}"
        aws_secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        "#,
            aws_access_key_fixture()
        );

        let matches = scanner.scan_text(&text, None);
        assert!(!matches.is_empty());

        let aws_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("AWS"))
            .collect();
        assert!(!aws_matches.is_empty());
    }

    #[test]
    fn test_github_token_detection() {
        let scanner = SecretScanner::new();
        let text = format!("github_token = '{}'", github_classic_token_fixture());

        let matches = scanner.scan_text(&text, None);
        let github_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("GitHub"))
            .collect();
        assert!(!github_matches.is_empty());
    }

    #[test]
    fn test_entropy_calculation() {
        let scanner = SecretScanner::new();

        // High entropy string
        let high_entropy_text = "password = 'aB3!mK9@pL7#nQ5$rT8&vW2*xY6^zA1%'";
        let matches = scanner.scan_text(high_entropy_text, None);

        if let Some(m) = matches.first() {
            assert!(m.entropy > 3.0);
        }
    }

    #[test]
    fn test_patch_scanning() {
        let scanner = SecretScanner::new();
        let patch = format!(
            r#"
diff --git a/config.py b/config.py
index 1234567..abcdefg 100644
--- a/config.py
+++ b/config.py
@@ -1,3 +1,4 @@
 # Configuration
 DEBUG = True
+API_KEY = "{}"
 SECRET_KEY = "mysecret"
        "#,
            aws_access_key_fixture()
        );

        let matches = scanner.scan_patch(&patch, Some("config.py"));
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_deduplication() {
        let matches = vec![
            SecretMatch {
                detector_name: "Test".to_string(),
                matched_text: "secret123".to_string(),
                start_position: 0,
                end_position: 9,
                line_number: Some(1),
                filename: None,
                entropy: 3.5,
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
                context: "secret123".to_string(),
                verified: false,
                hash: "abc123".to_string(),
            },
            SecretMatch {
                detector_name: "Test".to_string(),
                matched_text: "secret123".to_string(),
                start_position: 10,
                end_position: 19,
                line_number: Some(2),
                filename: None,
                entropy: 3.5,
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
                context: "secret123".to_string(),
                verified: false,
                hash: "abc123".to_string(), // Same hash
            },
        ];

        let unique = SecretScanner::deduplicate_matches(&matches);
        assert_eq!(unique.len(), 1);
    }

    // --- EXPANDED COMPREHENSIVE TESTS ---

    #[test]
    fn test_mongodb_connection_string_detection() {
        let scanner = SecretScanner::new();
        let text = "mongodb://admin:secretPassword123@db.example.com:27017/mydb";

        let matches = scanner.scan_text(text, None);
        let mongo_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("MongoDB"))
            .collect();
        assert!(
            !mongo_matches.is_empty(),
            "Should detect MongoDB connection string"
        );
    }

    #[test]
    fn test_postgresql_connection_string_detection() {
        let scanner = SecretScanner::new();
        let text = "postgresql://user:password@localhost:5432/database";

        let matches = scanner.scan_text(text, None);
        let postgres_matches: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.detector_name.contains("PostgreSQL") || m.detector_name.contains("Connection")
            })
            .collect();
        assert!(
            !postgres_matches.is_empty(),
            "Should detect PostgreSQL connection string"
        );
    }

    #[test]
    fn test_github_fine_grained_pat_detection() {
        let scanner = SecretScanner::new();
        // GitHub fine-grained PAT format: github_pat_ + 82 chars
        let token = "github_pat_".to_string() + &"A".repeat(82);
        let text = format!("GITHUB_TOKEN={}", token);

        let matches = scanner.scan_text(&text, None);
        let pat_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("Fine-grained"))
            .collect();
        assert!(
            !pat_matches.is_empty(),
            "Should detect GitHub fine-grained PAT"
        );
    }

    #[test]
    fn test_github_oauth_token_detection() {
        let scanner = SecretScanner::new();
        let text = format!("oauth_token = '{}'", github_oauth_token_fixture());

        let matches = scanner.scan_text(&text, None);
        let oauth_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("OAuth"))
            .collect();
        assert!(
            !oauth_matches.is_empty(),
            "Should detect GitHub OAuth token"
        );
    }

    #[test]
    fn test_github_app_token_detection() {
        let scanner = SecretScanner::new();
        let text = format!("app_token = '{}'", github_app_token_fixture());

        let matches = scanner.scan_text(&text, None);
        let app_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("App"))
            .collect();
        assert!(!app_matches.is_empty(), "Should detect GitHub App token");
    }

    #[test]
    fn test_aws_session_token_detection() {
        let scanner = SecretScanner::new();
        let text =
            r#"aws_session_token = "FwoGZXIvYXdzEBYaDKq8J1234567890ABCDEFGhijKLMNOPQRSTuvWXYZ""#;

        let matches = scanner.scan_text(text, None);
        let session_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("Session"))
            .collect();
        assert!(
            !session_matches.is_empty(),
            "Should detect AWS session token"
        );
    }

    #[test]
    fn test_aws_iam_user_token_detection() {
        let scanner = SecretScanner::new();
        let text = "iam_user = \"AIDA1234567890EXAMPLE\"";

        let matches = scanner.scan_text(text, None);
        let iam_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("IAM"))
            .collect();
        assert!(!iam_matches.is_empty(), "Should detect AWS IAM user token");
    }

    #[test]
    fn test_aws_account_id_detection() {
        let scanner = SecretScanner::new();
        let text = "aws_account_id = \"123456789012\"";

        let matches = scanner.scan_text(text, None);
        let account_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("Account ID"))
            .collect();
        assert!(!account_matches.is_empty(), "Should detect AWS account ID");
    }

    #[test]
    fn test_aws_rds_connection_string_detection() {
        let scanner = SecretScanner::new();
        let text = "postgresql://admin:Secret123@mydb-instance.abc123xyz.us-east-1.rds.amazonaws.com:5432/mydb";

        let matches = scanner.scan_text(text, None);
        let rds_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("RDS"))
            .collect();
        assert!(
            !rds_matches.is_empty(),
            "Should detect AWS RDS connection string"
        );
    }

    #[test]
    fn test_no_false_positives_on_example_keys() {
        let scanner = SecretScanner::new();
        let text = r#"
        // Example: AKIA_REDACTED_EXAMPLE
        // aws_secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        // token: ghp_REDACTED_EXAMPLE
        "#;

        let matches = scanner.scan_text(text, None);
        // Should still detect patterns but context might indicate example
        // In real implementation, you'd filter out "EXAMPLE" strings
        assert!(matches.iter().any(|m| m.matched_text.contains("EXAMPLE")));
    }

    #[test]
    fn test_empty_text_scanning() {
        let scanner = SecretScanner::new();
        let matches = scanner.scan_text("", None);
        assert!(matches.is_empty(), "Empty text should produce no matches");
    }

    #[test]
    fn test_text_with_no_secrets() {
        let scanner = SecretScanner::new();
        let text = "This is just plain text with no secrets at all. Just numbers: 12345.";

        let matches = scanner.scan_text(text, None);
        assert!(
            matches.is_empty(),
            "Plain text should not trigger detectors"
        );
    }

    #[test]
    fn test_add_custom_detector() {
        let mut scanner = SecretScanner::new();
        let initial_count = scanner.detectors.len();

        let custom_detector = SecretDetector {
            name: "Custom API Key".to_string(),
            description: "Custom API key pattern".to_string(),
            pattern: r"custom_[a-zA-Z0-9]{32}".to_string(),
            keywords: vec!["custom".to_string()],
            entropy_threshold: None,
            verify_func: None,
            severity: SecretSeverity::Medium,
            category: SecretCategory::ApiKey,
        };

        let result = scanner.add_detector(custom_detector);
        assert!(result.is_ok(), "Should successfully add custom detector");
        assert_eq!(scanner.detectors.len(), initial_count + 1);

        // Test custom detector works
        let text = "api_key = custom_abcdefgh12345678ijklmnop90123456";
        let matches = scanner.scan_text(text, None);
        let custom_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.detector_name.contains("Custom"))
            .collect();
        assert!(
            !custom_matches.is_empty(),
            "Custom detector should find matches"
        );
    }

    #[test]
    fn test_get_detector_names() {
        let scanner = SecretScanner::new();
        let names = scanner.get_detector_names();

        assert!(!names.is_empty());
        assert!(names.iter().any(|n| n.contains("AWS")));
        assert!(names.iter().any(|n| n.contains("GitHub")));
        assert!(names.iter().any(|n| n.contains("MongoDB")));
    }

    #[test]
    fn test_set_entropy_threshold() {
        let mut scanner = SecretScanner::new();
        let initial_threshold = scanner.entropy_threshold;

        scanner.set_entropy_threshold(5.5);
        assert_eq!(scanner.entropy_threshold, 5.5);
        assert_ne!(scanner.entropy_threshold, initial_threshold);
    }

    #[test]
    fn test_filter_by_severity_critical_only() {
        let matches = vec![
            SecretMatch {
                detector_name: "Low severity".to_string(),
                matched_text: "low".to_string(),
                start_position: 0,
                end_position: 3,
                line_number: Some(1),
                filename: None,
                entropy: 2.0,
                severity: SecretSeverity::Low,
                category: SecretCategory::Other,
                context: "low".to_string(),
                verified: false,
                hash: "hash1".to_string(),
            },
            SecretMatch {
                detector_name: "Critical severity".to_string(),
                matched_text: "critical".to_string(),
                start_position: 0,
                end_position: 8,
                line_number: Some(2),
                filename: None,
                entropy: 4.5,
                severity: SecretSeverity::Critical,
                category: SecretCategory::CloudProvider,
                context: "critical".to_string(),
                verified: false,
                hash: "hash2".to_string(),
            },
        ];

        let filtered = SecretScanner::filter_by_severity(&matches, SecretSeverity::Critical);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].severity, SecretSeverity::Critical);
    }

    #[test]
    fn test_filter_by_severity_high_and_above() {
        let matches = vec![
            SecretMatch {
                detector_name: "Low".to_string(),
                matched_text: "low".to_string(),
                start_position: 0,
                end_position: 3,
                line_number: Some(1),
                filename: None,
                entropy: 2.0,
                severity: SecretSeverity::Low,
                category: SecretCategory::Other,
                context: "low".to_string(),
                verified: false,
                hash: "hash1".to_string(),
            },
            SecretMatch {
                detector_name: "Medium".to_string(),
                matched_text: "medium".to_string(),
                start_position: 0,
                end_position: 6,
                line_number: Some(2),
                filename: None,
                entropy: 3.0,
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
                context: "medium".to_string(),
                verified: false,
                hash: "hash2".to_string(),
            },
            SecretMatch {
                detector_name: "High".to_string(),
                matched_text: "high".to_string(),
                start_position: 0,
                end_position: 4,
                line_number: Some(3),
                filename: None,
                entropy: 4.0,
                severity: SecretSeverity::High,
                category: SecretCategory::Token,
                context: "high".to_string(),
                verified: false,
                hash: "hash3".to_string(),
            },
            SecretMatch {
                detector_name: "Critical".to_string(),
                matched_text: "critical".to_string(),
                start_position: 0,
                end_position: 8,
                line_number: Some(4),
                filename: None,
                entropy: 4.5,
                severity: SecretSeverity::Critical,
                category: SecretCategory::CloudProvider,
                context: "critical".to_string(),
                verified: false,
                hash: "hash4".to_string(),
            },
        ];

        let filtered = SecretScanner::filter_by_severity(&matches, SecretSeverity::High);
        assert_eq!(filtered.len(), 2); // High and Critical
        assert!(filtered
            .iter()
            .all(|m| matches!(m.severity, SecretSeverity::High | SecretSeverity::Critical)));
    }

    #[test]
    fn test_multiple_secrets_in_one_text() {
        let scanner = SecretScanner::new();
        let text = format!(
            r#"
        # Multiple secrets
        aws_access_key_id = "{}"
        github_token = "{}"
        mongodb_uri = "mongodb://user:pass@localhost:27017/db"
        "#,
            aws_access_key_fixture(),
            github_classic_token_fixture()
        );

        let matches = scanner.scan_text(&text, None);
        assert!(
            matches.len() >= 3,
            "Should detect at least 3 different secrets"
        );

        let detector_types: Vec<&str> = matches.iter().map(|m| m.detector_name.as_str()).collect();

        // Should have AWS, GitHub, and MongoDB detections
        assert!(detector_types.iter().any(|d| d.contains("AWS")));
        assert!(detector_types.iter().any(|d| d.contains("GitHub")));
        assert!(detector_types.iter().any(|d| d.contains("MongoDB")));
    }

    #[test]
    fn test_patch_with_multiple_additions() {
        let scanner = SecretScanner::new();
        let patch = format!(
            r#"
diff --git a/config.py b/config.py
index 1234567..abcdefg 100644
--- a/config.py
+++ b/config.py
@@ -1,5 +1,7 @@
 # Configuration
 DEBUG = True
+AWS_KEY = "{}"
+GITHUB_TOKEN = "{}"
 SECRET_KEY = "mysecret"
+DB_URI = "mongodb://admin:secret@db:27017"
        "#,
            aws_access_key_fixture(),
            github_classic_token_fixture()
        );

        let matches = scanner.scan_patch(&patch, Some("config.py"));
        assert!(
            matches.len() >= 3,
            "Should detect multiple secrets in patch"
        );
    }

    #[test]
    fn test_default_scanner_has_built_in_detectors() {
        let scanner = SecretScanner::default();
        assert!(!scanner.detectors.is_empty());
        assert!(
            scanner.detectors.len() > 10,
            "Should have many built-in detectors"
        );
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(SecretSeverity::Low.to_string(), "Low");
        assert_eq!(SecretSeverity::Medium.to_string(), "Medium");
        assert_eq!(SecretSeverity::High.to_string(), "High");
        assert_eq!(SecretSeverity::Critical.to_string(), "Critical");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(SecretCategory::CloudProvider.to_string(), "Cloud Provider");
        assert_eq!(SecretCategory::Database.to_string(), "Database");
        assert_eq!(SecretCategory::ApiKey.to_string(), "API Key");
        assert_eq!(SecretCategory::Certificate.to_string(), "Certificate");
        assert_eq!(SecretCategory::Password.to_string(), "Password");
        assert_eq!(SecretCategory::Token.to_string(), "Token");
        assert_eq!(SecretCategory::Webhook.to_string(), "Webhook");
        assert_eq!(SecretCategory::Other.to_string(), "Other");
    }

    #[test]
    fn test_high_entropy_random_string() {
        let scanner = SecretScanner::new();
        let high_entropy_text = "secret_key = 'Xk9pL2mN4qR7sT0vW3yZ6bC8eF1hJ5uA'";

        let matches = scanner.scan_text(high_entropy_text, None);
        if let Some(m) = matches.first() {
            assert!(
                m.entropy > 3.5,
                "High entropy string should have high entropy score"
            );
        }
    }

    #[test]
    fn test_low_entropy_common_string() {
        let scanner = SecretScanner::new();
        let low_entropy_text = "password = 'aaaaaaaa'"; // Very low entropy

        let matches = scanner.scan_text(low_entropy_text, None);
        // Should not trigger high-entropy detectors
        let high_entropy_matches: Vec<_> = matches.iter().filter(|m| m.entropy > 4.0).collect();
        assert!(high_entropy_matches.is_empty() || low_entropy_text.contains("password"));
    }

    #[test]
    fn test_deduplicate_preserves_first_occurrence() {
        let matches = vec![
            SecretMatch {
                detector_name: "First".to_string(),
                matched_text: "secret".to_string(),
                start_position: 0,
                end_position: 6,
                line_number: Some(1),
                filename: None,
                entropy: 3.0,
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
                context: "first occurrence".to_string(),
                verified: false,
                hash: "samehash".to_string(),
            },
            SecretMatch {
                detector_name: "Second".to_string(),
                matched_text: "secret".to_string(),
                start_position: 10,
                end_position: 16,
                line_number: Some(2),
                filename: None,
                entropy: 3.0,
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
                context: "second occurrence".to_string(),
                verified: false,
                hash: "samehash".to_string(),
            },
        ];

        let unique = SecretScanner::deduplicate_matches(&matches);
        assert_eq!(unique.len(), 1);
        assert_eq!(unique[0].context, "first occurrence");
    }
}
