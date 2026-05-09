pub mod ai;
pub mod api;
pub mod audit;
pub mod audit_helpers;
pub mod auth;
pub mod bigquery;
pub mod circuit_breaker;
#[cfg(feature = "experimental")]
pub mod cli;
pub mod core;
pub mod github;
#[cfg(feature = "gui")]
pub mod gui;
pub mod health;
pub mod integration;
pub mod logging;
pub mod metrics;
pub mod operator;
pub mod performance;
pub mod rate_limiter;
pub mod realtime;
pub mod scanning;
pub mod scraper;
pub mod secrets;
pub mod security;
pub mod shutdown;

pub use ai::{
    AITriageAgent, LocalOpenAiTriageClient, LocalOpenAiTriageConfig, RedactedTriageInput,
    TriageContext, TriageResult,
};
pub use bigquery::BigQueryScanner;
pub use github::DanglingCommitFetcher;
#[cfg(feature = "gui")]
pub use gui::SecretsNinjaApp;
pub use integration::{GitHubSecretHunter, HunterConfig};
pub use performance::{PerformanceEngine, SecretDatabase};
pub use realtime::GitHubEventMonitor;
pub use scanning::ScanningService;
pub use secrets::{SecretMatch, SecretScanner, SecretValidator};
