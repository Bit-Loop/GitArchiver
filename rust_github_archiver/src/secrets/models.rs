use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{SecretCategory, SecretMatch, SecretSeverity};

/// Represents the origin of a detection so that we can segment metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectionSource {
    #[serde(rename = "realtime")]
    RealTime,
    #[serde(rename = "manual_scan")]
    ManualScan,
    #[serde(rename = "backfill")]
    Backfill,
}

impl DetectionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            DetectionSource::RealTime => "realtime",
            DetectionSource::ManualScan => "manual_scan",
            DetectionSource::Backfill => "backfill",
        }
    }
}

/// Metadata captured whenever we execute a scan (manual, realtime, or backfill)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingScanRecord {
    pub scan_id: Uuid,
    pub repository: Option<String>,
    pub scan_type: String,
    pub status: String,
    pub source: DetectionSource,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub files_scanned: Option<i64>,
    pub secrets_found: i64,
    pub created_by: String,
    pub metadata: Value,
}

pub type SecretScanRecord = FindingScanRecord;

impl FindingScanRecord {
    pub fn mark_completed(mut self, completed_at: DateTime<Utc>, duration_ms: i64) -> Self {
        self.status = "completed".to_string();
        self.completed_at = Some(completed_at);
        self.duration_ms = Some(duration_ms);
        self
    }
}

/// Persisted detection row derived from a scan-domain finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingDetectionRecord {
    pub detection_id: Uuid,
    pub scan_id: Option<Uuid>,
    pub event_id: Option<i64>,
    pub repository: String,
    pub file_path: Option<String>,
    pub detector_name: String,
    pub severity: SecretSeverity,
    pub category: SecretCategory,
    pub matched_text_hash: String,
    pub matched_text_preview: String,
    pub line_number: Option<i32>,
    pub verified: bool,
    pub detected_at: DateTime<Utc>,
    pub source: DetectionSource,
    pub metadata: Value,
}

pub type SecretDetectionRecord = FindingDetectionRecord;

impl FindingDetectionRecord {
    pub fn from_finding_match(
        finding: &SecretMatch,
        repository: &str,
        event_id: Option<i64>,
        scan_id: Option<Uuid>,
        detected_at: DateTime<Utc>,
        source: DetectionSource,
        metadata: Value,
    ) -> Self {
        let preview = redacted_preview(&finding.matched_text);

        Self {
            detection_id: Uuid::new_v4(),
            scan_id,
            event_id,
            repository: repository.to_string(),
            file_path: finding.filename.clone(),
            detector_name: finding.detector_name.clone(),
            severity: finding.severity.clone(),
            category: finding.category.clone(),
            matched_text_hash: finding.hash.clone(),
            matched_text_preview: preview,
            line_number: finding.line_number.map(|v| v as i32),
            verified: finding.verified,
            detected_at,
            source,
            metadata,
        }
    }

    pub fn from_match(
        secret_match: &SecretMatch,
        repository: &str,
        event_id: Option<i64>,
        scan_id: Option<Uuid>,
        detected_at: DateTime<Utc>,
        source: DetectionSource,
        metadata: Value,
    ) -> Self {
        Self::from_finding_match(
            secret_match,
            repository,
            event_id,
            scan_id,
            detected_at,
            source,
            metadata,
        )
    }
}

pub fn redacted_preview(value: &str) -> String {
    let char_count = value.chars().count();
    if char_count == 0 {
        return "[redacted:0]".to_string();
    }

    let visible_prefix: String = value.chars().take(4).collect();
    format!("{}…[redacted:{}]", visible_prefix, char_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detection_preview_redacts_secret_material() {
        let secret_match = SecretMatch {
            detector_name: "Test Detector".to_string(),
            matched_text: "ghp_REDACTED_EXAMPLE".to_string(),
            start_position: 0,
            end_position: 22,
            line_number: Some(3),
            filename: Some("secrets.env".to_string()),
            entropy: 5.0,
            severity: SecretSeverity::High,
            category: SecretCategory::ApiKey,
            context: "Commit HEAD".to_string(),
            verified: false,
            hash: "hash123".to_string(),
        };

        let record = FindingDetectionRecord::from_finding_match(
            &secret_match,
            "octo/repo",
            None,
            None,
            Utc::now(),
            DetectionSource::ManualScan,
            json!({}),
        );

        let expected_len = secret_match.matched_text.chars().count();
        assert_eq!(
            record.matched_text_preview,
            format!("ghp_…[redacted:{}]", expected_len)
        );
        assert_ne!(record.matched_text_preview, secret_match.matched_text);
    }
}
