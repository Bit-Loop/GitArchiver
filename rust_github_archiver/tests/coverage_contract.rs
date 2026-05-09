use std::fs;
use std::path::{Path, PathBuf};

fn repo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn git_root() -> PathBuf {
    repo_dir()
        .parent()
        .expect("crate should live under repository root")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.as_ref().display()))
}

#[test]
fn critical_flows_fsm_and_orchestration_modules_have_local_test_suites() {
    let modules = [
        "src/api/ai_handlers.rs",
        "src/api/handlers.rs",
        "src/api/scanner_service.rs",
        "src/api/status_service.rs",
        "src/auth/jwt.rs",
        "src/auth/middleware.rs",
        "src/auth/users.rs",
        "src/circuit_breaker.rs",
        "src/core/database.rs",
        "src/core/resource_monitor.rs",
        "src/operator/runtime.rs",
        "src/rate_limiter.rs",
        "src/realtime/rate_limiter.rs",
        "src/realtime/token_pool.rs",
        "src/realtime/webhook.rs",
        "src/scanning/domain.rs",
        "src/scanning/mod.rs",
        "src/scanning/persistence.rs",
        "src/scanning/trufflehog.rs",
        "src/scraper/archive_scraper.rs",
        "src/scraper/downloader.rs",
        "src/scraper/file_processor.rs",
        "src/scraper/main_scraper.rs",
        "src/scraper/state.rs",
        "src/secrets/models.rs",
        "src/secrets/scanner.rs",
        "src/secrets/validator.rs",
    ];

    let mut missing = Vec::new();
    for module in modules {
        let source = read(repo_dir().join(module));
        if !source.contains("#[cfg(test)]") || !source.contains("mod tests") {
            missing.push(module);
        }
    }

    assert!(
        missing.is_empty(),
        "critical production modules missing local tests: {missing:?}"
    );
}

#[test]
fn state_machine_transition_tests_are_explicitly_named() {
    let required_markers = [
        (
            "src/scraper/state.rs",
            "scraper_fsm_accepts_valid_lifecycle_transitions",
        ),
        (
            "src/scraper/state.rs",
            "scraper_fsm_rejects_invalid_lifecycle_transitions",
        ),
        ("src/scanning/mod.rs", "pause_gate_waits_until_resumed"),
        ("src/scanning/mod.rs", "shutdown_gate_cancels_active_scan"),
        (
            "src/circuit_breaker.rs",
            "test_circuit_breaker_opens_after_failures",
        ),
        (
            "src/circuit_breaker.rs",
            "test_circuit_breaker_half_open_recovery",
        ),
        ("src/realtime/rate_limiter.rs", "test_auto_adjust"),
        ("src/realtime/rate_limiter.rs", "test_clear_pause"),
        ("src/realtime/token_pool.rs", "test_token_pool_round_robin"),
        (
            "src/realtime/token_pool.rs",
            "test_token_recovery_after_failures",
        ),
        (
            "src/operator/runtime.rs",
            "runtime_service_starts_and_stops_without_compatibility_flag",
        ),
        (
            "src/operator/runtime.rs",
            "operator_workflow_smoke_covers_login_runtime_and_findings_review",
        ),
    ];

    let mut missing = Vec::new();
    for (module, marker) in required_markers {
        let source = read(repo_dir().join(module));
        if !source.contains(marker) {
            missing.push(format!("{module}::{marker}"));
        }
    }

    assert!(
        missing.is_empty(),
        "required FSM/orchestration tests missing: {missing:?}"
    );
}

#[test]
fn production_source_has_no_skeleton_markers() {
    let forbidden = [
        "todo!(",
        "unimplemented!(",
        "TODO",
        "FIXME",
        "Mock data",
        "mock data",
        "Would be calculated",
        "Placeholder",
        "placeholder",
        "Temporarily disabled",
        "Simple implementation",
        "in reality",
        "This would",
    ];
    let roots = [repo_dir().join("src"), repo_dir().join("tauri-app/src")];
    let mut violations = Vec::new();

    for root in roots {
        for entry in walk_rs_ts_files(&root) {
            let source = read(&entry);
            for marker in forbidden {
                if source.contains(marker) {
                    violations.push(format!("{} contains {marker}", entry.display()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "skeleton markers found in production source: {violations:?}"
    );
}

#[test]
fn github_actions_runs_full_rust_tauri_ui_and_static_gates() {
    let root_workflow = read(git_root().join(".github/workflows/rust.yml"));

    for required in [
        "cargo fmt --all -- --check",
        "cargo check --all-targets --all-features --locked",
        "cargo clippy --all-targets --all-features --locked -- -D warnings",
        "cargo test --all-targets --all-features --locked",
        "cargo llvm-cov --all-targets --all-features --locked",
        "npm ci",
        "npm run build",
        "cargo check --locked",
        "Static skeleton marker scan",
    ] {
        assert!(
            root_workflow.contains(required),
            "root GitHub Actions workflow does not contain required gate: {required}"
        );
    }
}

fn walk_rs_ts_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_rs_ts_files_inner(root, &mut files);
    files
}

fn walk_rs_ts_files_inner(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_file() {
        let include = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext, "rs" | "ts" | "tsx"))
            .unwrap_or(false);
        if include {
            files.push(path.to_path_buf());
        }
        return;
    }

    if !metadata.is_dir() {
        return;
    }

    for entry in fs::read_dir(path).expect("read source directory") {
        let entry = entry.expect("directory entry");
        let entry_path = entry.path();
        if entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| matches!(name, "target" | "node_modules"))
        {
            continue;
        }
        walk_rs_ts_files_inner(&entry_path, files);
    }
}
