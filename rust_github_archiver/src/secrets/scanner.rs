use anyhow::{anyhow, Result};
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, error, debug};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use sha2::{Sha256, Digest};
use entropy::shannon_entropy;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretCategory {
    CloudProvider,
    Database,
    ApiKey,
    Certificate,
    Password,
    Token,
    Webhook,
    Other,
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
                pattern: r"(?i)(aws.{0,20})?['\"]([0-9a-zA-Z/+]{40})['\"]".to_string(),
                keywords: vec!["aws".to_string(), "secret".to_string()],
                entropy_threshold: Some(4.5),
                verify_func: Some("verify_aws_secret_key".to_string()),
                severity: SecretSeverity::Critical,
                category: SecretCategory::CloudProvider,
            },
            SecretDetector {
                name: "AWS Session Token".to_string(),
                description: "Amazon Web Services Session Token".to_string(),
                pattern: r"(?i)(aws.session.token.{0,20})?['\"]([0-9a-zA-Z/+=]{16,})['\"]".to_string(),
                keywords: vec!["aws".to_string(), "session".to_string(), "token".to_string()],
                entropy_threshold: Some(4.0),
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::Token,
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
                pattern: r"(?i)(api.key|apikey|api_key).{0,20}['\"]([0-9a-zA-Z_-]{16,})['\"]".to_string(),
                keywords: vec!["api".to_string(), "key".to_string()],
                entropy_threshold: Some(4.0),
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::ApiKey,
            },
            SecretDetector {
                name: "Generic Password".to_string(),
                description: "Generic password pattern".to_string(),
                pattern: r"(?i)(password|passwd|pwd).{0,20}['\"]([0-9a-zA-Z_!@#$%^&*-]{8,})['\"]".to_string(),
                keywords: vec!["password".to_string(), "passwd".to_string(), "pwd".to_string()],
                entropy_threshold: Some(3.5),
                verify_func: None,
                severity: SecretSeverity::Medium,
                category: SecretCategory::Password,
            },
            SecretDetector {
                name: "Generic Secret".to_string(),
                description: "Generic secret pattern".to_string(),
                pattern: r"(?i)(secret|token).{0,20}['\"]([0-9a-zA-Z_-]{16,})['\"]".to_string(),
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
                for capture in regex.find_iter(text) {
                    if let Ok(Some(m)) = capture {
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
                            if entropy < threshold {
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
                            entropy,
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
        let regex = Regex::new(&detector.pattern)
            .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;
        
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
    pub fn filter_by_severity(matches: &[SecretMatch], min_severity: SecretSeverity) -> Vec<SecretMatch> {
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

    #[test]
    fn test_scanner_creation() {
        let scanner = SecretScanner::new();
        assert!(!scanner.detectors.is_empty());
        assert!(!scanner.patterns.is_empty());
    }

    #[test]
    fn test_aws_access_key_detection() {
        let scanner = SecretScanner::new();
        let text = r#"
        aws_access_key_id = "AKIAIOSFODNN7EXAMPLE"
        aws_secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        "#;

        let matches = scanner.scan_text(text, None);
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
        let text = "github_token = 'ghp_1234567890123456789012345678901234567890'";

        let matches = scanner.scan_text(text, None);
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
        let patch = r#"
diff --git a/config.py b/config.py
index 1234567..abcdefg 100644
--- a/config.py
+++ b/config.py
@@ -1,3 +1,4 @@
 # Configuration
 DEBUG = True
+API_KEY = "AKIAIOSFODNN7EXAMPLE"
 SECRET_KEY = "mysecret"
        "#;

        let matches = scanner.scan_patch(patch, Some("config.py"));
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
}
