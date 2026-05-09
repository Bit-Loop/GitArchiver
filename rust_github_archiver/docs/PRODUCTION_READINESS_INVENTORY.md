# Production Readiness Inventory

This inventory tracks the Rust surfaces that are intentionally part of the production build after the production-readiness pass.

## Compiled Production Modules

- `src/main.rs` and `src/operator/` provide the grouped CLI for ingest, scan, triage, and admin database workflows.
- `src/bin/web_server.rs` and `src/api/` provide the Axum dashboard/API boundary, including scanner, scraper, auth, audit, monitoring, AI triage, and maintenance repair handlers.
- `src/core/` provides configuration, PostgreSQL persistence, resource monitoring, and persistence service wrappers.
- `src/scanning/`, `src/secrets/`, and `src/github/` provide scan orchestration, TruffleHog execution, detection models, validation, and dangling-commit collection.
- `src/realtime/`, `src/scraper/`, `src/bigquery/`, `src/performance/`, `src/metrics.rs`, `src/rate_limiter.rs`, `src/security.rs`, `src/health.rs`, `src/logging.rs`, and `src/shutdown.rs` are compiled support modules for ingestion, runtime monitoring, metrics, safety controls, and shutdown behavior.
- `src/gui/` is compiled when the `gui` feature is enabled.
- `src/ai/` is compiled in the main crate and contains heuristic triage plus the local OpenAI-compatible redacted triage client.

## Removed Dormant Skeletons

The following tracked Rust modules were outside the crate graph and contained placeholder or unfinished behavior. They were removed instead of being left as invisible production code:

- `src/query/`
- `src/schema/`
- `src/schema_evolution/`
- `src/sources/`
- `src/tree/`
- empty module stubs: `src/config_management.rs`, `src/deployment_management.rs`, `src/gui_integration.rs`, `src/multi_source_api.rs`, `src/testing_framework.rs`

## Required Gates

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --all-targets --locked`
- `cargo check --manifest-path tauri-app/Cargo.toml --locked`
- `npm run build` in `tauri-app/`
