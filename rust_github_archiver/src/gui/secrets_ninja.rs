use crate::secrets::{SecretMatch, ValidationResult};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretsNinjaApp {
    pub findings_count: usize,
    pub validations_count: usize,
    pub launch_target: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiLaunchError {
    message: &'static str,
}

impl fmt::Display for GuiLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for GuiLaunchError {}

pub fn launch_secrets_ninja() -> Result<(), GuiLaunchError> {
    Err(GuiLaunchError {
        message: "the legacy Iced GUI has been removed; use the browser dashboard or Tauri app",
    })
}

pub fn load_secrets_data(
    secrets: Vec<SecretMatch>,
    validation_results: Vec<ValidationResult>,
) -> SecretsNinjaApp {
    SecretsNinjaApp {
        findings_count: secrets.len(),
        validations_count: validation_results.len(),
        launch_target: "browser-dashboard",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::scanner::{SecretCategory, SecretMatch, SecretSeverity};

    fn create_test_secret() -> SecretMatch {
        SecretMatch {
            detector_name: "GitHub Token".to_string(),
            matched_text: "ghp_REDACTED_EXAMPLE".to_string(),
            start_position: 1,
            end_position: 10,
            line_number: Some(42),
            filename: Some("test.rs".to_string()),
            verified: false,
            entropy: 4.5,
            severity: SecretSeverity::High,
            category: SecretCategory::ApiKey,
            hash: "test_hash".to_string(),
            context: "context".to_string(),
        }
    }

    #[test]
    fn launch_returns_actionable_legacy_gui_error() {
        let error = launch_secrets_ninja().expect_err("legacy GUI should not launch");
        assert!(error.to_string().contains("browser dashboard"));
    }

    #[test]
    fn load_secrets_data_preserves_summary_counts() {
        let app = load_secrets_data(vec![create_test_secret()], vec![]);
        assert_eq!(app.findings_count, 1);
        assert_eq!(app.validations_count, 0);
        assert_eq!(app.launch_target, "browser-dashboard");
    }
}
