use anyhow::Result;
use serde_json::json;
use tracing::warn;
use uuid::Uuid;

use crate::core::database::Database;
use crate::scanning::domain::{EvidenceArtifact, FindingRecord, RepositoryRef, ScanFinding};
use crate::scanning::CompletedScan;
use crate::secrets::{FindingDetectionRecord, FindingScanRecord};

pub struct ScanPersistenceAdapter<'a> {
    database: &'a Database,
}

impl<'a> ScanPersistenceAdapter<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub async fn persist_scan(
        &self,
        scan: &CompletedScan,
        failure_reason: Option<&str>,
    ) -> Result<()> {
        let scan_uuid = parse_scan_uuid(&scan.id);
        let scan_record = build_scan_record(scan, scan_uuid, failure_reason);

        self.database.insert_secret_scan(&scan_record).await?;

        let detection_records = build_detection_records(scan, scan_uuid);
        if !detection_records.is_empty() {
            self.database
                .insert_secret_detections(&detection_records)
                .await?;
        }

        Ok(())
    }
}

fn parse_scan_uuid(scan_id: &str) -> Uuid {
    match Uuid::parse_str(scan_id) {
        Ok(uuid) => uuid,
        Err(error) => {
            warn!(
                "Scan {} had invalid UUID ({}), generating replacement",
                scan_id, error
            );
            Uuid::new_v4()
        }
    }
}

fn build_scan_record(
    scan: &CompletedScan,
    scan_uuid: Uuid,
    failure_reason: Option<&str>,
) -> FindingScanRecord {
    let repository = scan_repository(scan);
    let source_reference = scan
        .initiator
        .source_reference()
        .map(std::string::ToString::to_string);

    let scan_metadata = json!({
        "repository": repository,
        "severity_breakdown": scan.results.severity_breakdown.clone(),
        "category_breakdown": scan.results.category_breakdown.clone(),
        "detector_stats": scan.results.detector_stats.clone(),
        "false_positives": scan.results.false_positives,
        "verified_findings": scan.results.verified_findings,
        "verified_secrets": scan.results.verified_findings,
        "source_reference": source_reference,
        "created_by": scan.created_by.clone(),
        "failure_reason": failure_reason,
        "initiator": scan.initiator,
        "source_events": scan.source_events,
    });

    FindingScanRecord {
        scan_id: scan_uuid,
        repository: if scan.repository.is_empty() {
            None
        } else {
            Some(scan.repository.clone())
        },
        scan_type: scan.scan_type.as_str().to_string(),
        status: scan.status.as_str().to_string(),
        source: scan.initiator.detection_source(),
        started_at: scan.started_at,
        completed_at: Some(scan.completed_at),
        duration_ms: Some(scan.duration_ms as i64),
        files_scanned: Some(scan.results.files_scanned as i64),
        secrets_found: scan.results.findings.len() as i64,
        created_by: scan.created_by.clone(),
        metadata: scan_metadata,
    }
}

fn build_detection_records(scan: &CompletedScan, scan_uuid: Uuid) -> Vec<FindingDetectionRecord> {
    let repository = scan_repository(scan);
    let source_reference = scan
        .initiator
        .source_reference()
        .map(std::string::ToString::to_string);
    let source_event_ids: Vec<i64> = scan
        .source_events
        .iter()
        .map(|event| event.event_id)
        .collect();

    scan.results
        .findings
        .iter()
        .map(|finding| {
            let event_id = infer_event_id(finding.context.as_str(), &source_event_ids);
            let finding_record = build_finding_record(
                finding,
                repository.clone(),
                &source_event_ids,
                source_reference.clone(),
            );
            let metadata = json!({
                "context": finding.context.clone(),
                "entropy": finding.entropy,
                "match_length": finding.matched_text.len(),
                "source_reference": source_reference.clone(),
                "detector": finding.detector_name.clone(),
                "source_event_ids": source_event_ids,
                "finding": finding_record,
            });

            FindingDetectionRecord::from_finding_match(
                finding,
                &scan.repository,
                event_id,
                Some(scan_uuid),
                scan.completed_at,
                scan.initiator.detection_source(),
                metadata,
            )
        })
        .collect()
}

fn scan_repository(scan: &CompletedScan) -> RepositoryRef {
    scan.source_events
        .first()
        .map(RepositoryRef::from)
        .unwrap_or_else(|| RepositoryRef::new(scan.repository.clone(), None))
}

fn build_finding_record(
    finding: &ScanFinding,
    repository: RepositoryRef,
    source_event_ids: &[i64],
    source_reference: Option<String>,
) -> FindingRecord {
    FindingRecord {
        detector_name: finding.detector_name.clone(),
        verified: finding.verified,
        match_hash: finding.hash.clone(),
        repository,
        artifact: EvidenceArtifact {
            filename: finding.filename.clone(),
            line_number: finding.line_number,
            commit_sha: infer_commit_sha(finding.context.as_str()),
        },
        source_event_ids: source_event_ids.to_vec(),
        source_reference,
    }
}

fn infer_event_id(context: &str, source_event_ids: &[i64]) -> Option<i64> {
    if source_event_ids.len() == 1 {
        return source_event_ids.first().copied();
    }

    context
        .strip_prefix("Event ")
        .and_then(|remaining| remaining.split_once(' '))
        .and_then(|(event_id, _)| event_id.parse::<i64>().ok())
}

fn infer_commit_sha(context: &str) -> Option<String> {
    context
        .split("Commit ")
        .nth(1)
        .and_then(|remaining| remaining.split([' ', '•']).next())
        .map(str::trim)
        .filter(|commit| !commit.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::scanning::domain::ScanInitiator;
    use crate::scanning::{CompletedScan, ScanResults, ScanStatus, ScanType};
    use crate::secrets::{SecretCategory, SecretMatch, SecretSeverity};

    fn sample_scan() -> CompletedScan {
        CompletedScan {
            id: Uuid::new_v4().to_string(),
            repository: "octo/repo".to_string(),
            scan_type: ScanType::Incremental,
            status: ScanStatus::Completed,
            started_at: Utc::now(),
            completed_at: Utc::now(),
            duration_ms: 25,
            results: ScanResults {
                findings: vec![SecretMatch {
                    detector_name: "Test Detector".to_string(),
                    matched_text: "ghp_REDACTED_EXAMPLE".to_string(),
                    start_position: 0,
                    end_position: 22,
                    line_number: Some(7),
                    filename: Some("secrets.env".to_string()),
                    entropy: 5.0,
                    severity: SecretSeverity::High,
                    category: SecretCategory::ApiKey,
                    context: "Event 42 @ 2024-01-01T00:00:00Z • Commit abc123 • Ref: refs/heads/main • Forced: false".to_string(),
                    verified: true,
                    hash: "hash123".to_string(),
                }],
                files_scanned: 1,
                total_lines: 7,
                scan_duration_ms: 25,
                severity_breakdown: Default::default(),
                category_breakdown: Default::default(),
                detector_stats: Default::default(),
                false_positives: 0,
                verified_findings: 1,
            },
            created_by: "worker:test-worker".to_string(),
            initiator: ScanInitiator::worker("test-worker"),
            source_events: vec![crate::scanning::domain::SourceEventProvenance {
                event_id: 42,
                repository_full_name: "octo/repo".to_string(),
                repository_url: Some("https://github.com/octo/repo".to_string()),
                before_sha: "abc123".to_string(),
                head_sha: Some("def456".to_string()),
                reference: Some("refs/heads/main".to_string()),
                forced: false,
                commit_count: 1,
                event_created_at: Utc::now(),
            }],
        }
    }

    #[test]
    fn infer_event_id_uses_context_for_multi_event_scans() {
        assert_eq!(infer_event_id("Event 55 @ ts", &[1, 2, 3]), Some(55));
        assert_eq!(infer_event_id("Commit HEAD", &[1, 2, 3]), None);
    }

    #[test]
    fn build_detection_records_carries_event_traceability() {
        let scan = sample_scan();
        let records = build_detection_records(&scan, Uuid::new_v4());

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_id, Some(42));
        assert_eq!(records[0].source.as_str(), "realtime");
        assert_eq!(
            records[0].metadata.get("source_event_ids"),
            Some(&json!([42]))
        );
        assert_eq!(
            records[0]
                .metadata
                .get("finding")
                .and_then(|finding| finding.get("artifact"))
                .and_then(|artifact| artifact.get("commit_sha")),
            Some(&json!("abc123"))
        );
    }
}
