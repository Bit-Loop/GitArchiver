use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::database::EventScanTarget;
use crate::secrets::DetectionSource;

/// Canonical scan-domain finding type. Detector-specific raw material still lives in
/// `SecretMatch`, but the scan pipeline refers to those matches as findings.
pub type ScanFinding = crate::secrets::SecretMatch;

/// Identifies who launched a scan so API, workers, and persistence share the
/// same operator/runtime contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanInitiator {
    Manual { username: String },
    Scheduled { username: String },
    Worker { worker_id: String },
    Realtime { source_reference: String },
}

impl ScanInitiator {
    pub fn manual(username: impl Into<String>) -> Self {
        Self::Manual {
            username: username.into(),
        }
    }

    pub fn scheduled(username: impl Into<String>) -> Self {
        Self::Scheduled {
            username: username.into(),
        }
    }

    pub fn worker(worker_id: impl Into<String>) -> Self {
        Self::Worker {
            worker_id: worker_id.into(),
        }
    }

    pub fn realtime(source_reference: impl Into<String>) -> Self {
        Self::Realtime {
            source_reference: source_reference.into(),
        }
    }

    pub fn created_by_label(&self) -> String {
        match self {
            Self::Manual { username } => username.clone(),
            Self::Scheduled { username } => format!("scheduled:{}", username),
            Self::Worker { worker_id } => format!("worker:{}", worker_id),
            Self::Realtime { source_reference } => format!("realtime:{}", source_reference),
        }
    }

    pub fn detection_source(&self) -> DetectionSource {
        match self {
            Self::Worker { .. } | Self::Realtime { .. } => DetectionSource::RealTime,
            Self::Manual { .. } | Self::Scheduled { .. } => DetectionSource::ManualScan,
        }
    }

    pub fn source_reference(&self) -> Option<&str> {
        match self {
            Self::Realtime { source_reference } => Some(source_reference.as_str()),
            Self::Worker { worker_id } => Some(worker_id.as_str()),
            Self::Manual { .. } | Self::Scheduled { .. } => None,
        }
    }
}

/// Stable provenance captured for every source event that drives a scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEventProvenance {
    pub event_id: i64,
    pub repository_full_name: String,
    pub repository_url: Option<String>,
    pub before_sha: String,
    pub head_sha: Option<String>,
    pub reference: Option<String>,
    pub forced: bool,
    pub commit_count: usize,
    pub event_created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryRef {
    pub full_name: String,
    pub repository_url: Option<String>,
}

impl RepositoryRef {
    pub fn new(full_name: impl Into<String>, repository_url: Option<String>) -> Self {
        Self {
            full_name: full_name.into(),
            repository_url,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitRef {
    pub event_id: Option<i64>,
    pub before_sha: String,
    pub head_sha: Option<String>,
    pub reference: Option<String>,
    pub forced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceArtifact {
    pub filename: Option<String>,
    pub line_number: Option<usize>,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindingRecord {
    pub detector_name: String,
    pub verified: bool,
    pub match_hash: String,
    pub repository: RepositoryRef,
    pub artifact: EvidenceArtifact,
    pub source_event_ids: Vec<i64>,
    pub source_reference: Option<String>,
}

impl From<EventScanTarget> for SourceEventProvenance {
    fn from(value: EventScanTarget) -> Self {
        Self {
            event_id: value.event_id,
            repository_full_name: value.repository_full_name,
            repository_url: value.repository_url,
            before_sha: value.before_sha,
            head_sha: value.head_sha,
            reference: value.reference,
            forced: value.forced,
            commit_count: value.commit_count.max(1) as usize,
            event_created_at: value.event_created_at,
        }
    }
}

impl From<&EventScanTarget> for SourceEventProvenance {
    fn from(value: &EventScanTarget) -> Self {
        Self {
            event_id: value.event_id,
            repository_full_name: value.repository_full_name.clone(),
            repository_url: value.repository_url.clone(),
            before_sha: value.before_sha.clone(),
            head_sha: value.head_sha.clone(),
            reference: value.reference.clone(),
            forced: value.forced,
            commit_count: value.commit_count.max(1) as usize,
            event_created_at: value.event_created_at,
        }
    }
}

impl From<&SourceEventProvenance> for RepositoryRef {
    fn from(value: &SourceEventProvenance) -> Self {
        Self {
            full_name: value.repository_full_name.clone(),
            repository_url: value.repository_url.clone(),
        }
    }
}

impl From<&SourceEventProvenance> for CommitRef {
    fn from(value: &SourceEventProvenance) -> Self {
        Self {
            event_id: Some(value.event_id),
            before_sha: value.before_sha.clone(),
            head_sha: value.head_sha.clone(),
            reference: value.reference.clone(),
            forced: value.forced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_initiator_exposes_stable_runtime_metadata() {
        let manual = ScanInitiator::manual("analyst");
        let worker = ScanInitiator::worker("worker-1");
        let realtime = ScanInitiator::realtime("push-hook");

        assert_eq!(manual.created_by_label(), "analyst");
        assert_eq!(worker.created_by_label(), "worker:worker-1");
        assert_eq!(realtime.source_reference(), Some("push-hook"));
        assert_eq!(worker.detection_source().as_str(), "realtime");
        assert_eq!(manual.detection_source().as_str(), "manual_scan");
    }

    #[test]
    fn source_event_maps_to_repository_and_commit_domain_types() {
        let event = SourceEventProvenance {
            event_id: 42,
            repository_full_name: "owner/repo".to_string(),
            repository_url: Some("https://github.com/owner/repo".to_string()),
            before_sha: "abc123".to_string(),
            head_sha: Some("def456".to_string()),
            reference: Some("refs/heads/main".to_string()),
            forced: false,
            commit_count: 1,
            event_created_at: Utc::now(),
        };

        let repository = RepositoryRef::from(&event);
        let commit = CommitRef::from(&event);

        assert_eq!(repository.full_name, "owner/repo");
        assert_eq!(commit.event_id, Some(42));
        assert_eq!(commit.head_sha.as_deref(), Some("def456"));
    }
}
