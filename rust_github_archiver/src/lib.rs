pub mod api;
pub mod auth;
pub mod bigquery;
pub mod cli;
pub mod core;
pub mod github;
#[cfg(feature = "gui")]
pub mod gui;
pub mod scraper;
pub mod secrets;
#[cfg(feature = "ai")]
pub mod ai;
pub mod realtime;
pub mod performance;
pub mod integration;

pub use bigquery::BigQueryScanner;
pub use github::DanglingCommitFetcher;
pub use secrets::{SecretScanner, SecretValidator, SecretMatch};
#[cfg(feature = "gui")]
pub use gui::SecretsNinjaApp;
#[cfg(feature = "ai")]
pub use ai::{AITriageAgent, TriageResult, TriageContext};
pub use realtime::GitHubEventMonitor;
pub use performance::{PerformanceEngine, SecretDatabase};
pub use integration::{GitHubSecretHunter, HunterConfig};
