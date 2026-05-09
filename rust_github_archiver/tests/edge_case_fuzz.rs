use std::str::FromStr;

use github_archiver::realtime::usable_github_token;
use github_archiver::secrets::{SecretCategory, SecretScanner, SecretSeverity};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

fn bounded_text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..512).prop_map(|chars| chars.into_iter().collect())
}

fn bounded_patch_line() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..160).prop_map(|chars| chars.into_iter().collect())
}

fn maybe_filename() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        prop::collection::vec(any::<char>(), 0..64)
            .prop_map(|chars| Some(chars.into_iter().collect()))
    ]
}

fn assert_finding_shape(
    finding: &github_archiver::secrets::SecretMatch,
    source_text: &str,
    filename: Option<&str>,
) -> Result<(), TestCaseError> {
    prop_assert!(finding.end_position >= finding.start_position);
    prop_assert!(finding.end_position <= source_text.len());
    prop_assert!(!finding.matched_text.is_empty());
    prop_assert_eq!(
        &source_text[finding.start_position..finding.end_position],
        finding.matched_text.as_str()
    );
    prop_assert!(finding.entropy.is_finite());
    prop_assert!(finding.entropy >= 0.0);
    prop_assert!(!finding.hash.is_empty());
    prop_assert_eq!(finding.filename.as_deref(), filename);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(96),
        max_shrink_iters: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn secret_scanner_handles_arbitrary_text_without_invalid_spans(
        text in bounded_text(),
        filename in maybe_filename(),
    ) {
        let scanner = SecretScanner::new();
        let findings = scanner.scan_text(&text, filename.as_deref());

        for finding in &findings {
            assert_finding_shape(finding, &text, filename.as_deref())?;
        }
    }

    #[test]
    fn secret_scanner_finds_known_secret_inside_generated_noise(
        prefix in bounded_text(),
        suffix in bounded_text(),
    ) {
        let token = format!("ghp_{}", "A".repeat(36));
        let text = format!("{prefix}\nGITHUB_TOKEN={token}\n{suffix}");
        let scanner = SecretScanner::new();
        let findings = scanner.scan_text(&text, Some("generated.env"));

        prop_assert!(
            findings.iter().any(|finding| finding.matched_text == token),
            "scanner should retain the inserted GitHub token finding"
        );
        for finding in &findings {
            assert_finding_shape(finding, &text, Some("generated.env"))?;
        }
    }

    #[test]
    fn patch_scanner_handles_generated_added_removed_and_metadata_lines(
        added in prop::collection::vec(bounded_patch_line(), 0..24),
        removed in prop::collection::vec(bounded_patch_line(), 0..24),
        metadata in prop::collection::vec(bounded_patch_line(), 0..8),
    ) {
        let mut patch = String::from("diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n");
        for line in metadata {
            patch.push_str("@@ ");
            patch.push_str(&line.replace('\n', " "));
            patch.push_str(" @@\n");
        }
        for line in removed {
            patch.push('-');
            patch.push_str(&line.replace('\n', " "));
            patch.push('\n');
        }
        for line in added {
            patch.push('+');
            patch.push_str(&line.replace('\n', " "));
            patch.push('\n');
        }

        let expected_scanned_text = patch
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .map(|line| &line[1..])
            .collect::<Vec<_>>()
            .join("\n");

        let scanner = SecretScanner::new();
        let findings = scanner.scan_patch(&patch, Some("file.txt"));

        for finding in &findings {
            assert_finding_shape(finding, &expected_scanned_text, Some("file.txt"))?;
        }
    }

    #[test]
    fn token_filter_trims_or_rejects_generated_inputs(raw in bounded_text()) {
        let result = usable_github_token(&raw);

        if let Some(token) = result {
            prop_assert_eq!(token.as_str(), raw.trim());
            prop_assert!(!token.is_empty());

            let lowered = token.to_ascii_lowercase();
            for marker in [
                "redacted",
                "example",
                "sample",
                "replace",
                "change_me",
                "changeme",
                "your_github_token",
                "your-token",
            ] {
                prop_assert!(!lowered.contains(marker));
            }
            prop_assert!(!(lowered.contains("place") && lowered.contains("holder")));
        }
    }

    #[test]
    fn category_and_severity_parsers_handle_generated_labels(label in bounded_text()) {
        let _ = SecretCategory::from_storage_key(&label);
        let _ = SecretCategory::from_label(&label);
        let _ = SecretSeverity::from_str(&label);
    }
}

#[test]
fn category_and_severity_parsers_accept_canonical_edge_labels() {
    assert_eq!(
        SecretCategory::from_label(" API KEYS "),
        Some(SecretCategory::ApiKey)
    );
    assert_eq!(
        SecretCategory::from_label("Access Tokens"),
        Some(SecretCategory::Token)
    );
    assert_eq!(
        SecretCategory::from_storage_key("private_key"),
        Some(SecretCategory::PrivateKey)
    );
    assert_eq!(
        SecretSeverity::from_str(" CRITICAL "),
        Ok(SecretSeverity::Critical)
    );
    assert!(SecretSeverity::from_str("critical/high").is_err());
}
