# Architecture Index

This index points to the active production execution paths so contributors can navigate the codebase without reading legacy or backup files.

## Web Server Path

1. Binary entrypoint: [src/bin/web_server.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/bin/web_server.rs:1)
2. Server bootstrap: [src/api/server.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/server.rs:1)
3. Shared state construction: [src/api/state.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/state.rs:18)
4. Route registration: [src/api/routes.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/routes.rs:128)
5. Handler boundary: [src/api/handlers.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/handlers.rs:1), [src/api/scanner_handlers.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/scanner_handlers.rs:1), [src/api/monitoring_handlers.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/monitoring_handlers.rs:1)

## CLI Path

1. CLI entrypoint and command dispatch: [src/main.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/main.rs:1)
2. Grouped operator command model: [src/operator/cli.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/operator/cli.rs:1)
3. Shared operator command services: [src/operator/service.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/operator/service.rs:1)
4. Legacy CLI module: [src/cli.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/cli.rs:1) behind the `experimental` feature only
5. Shared scanning runtime used by CLI flows: [src/scanning/mod.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scanning/mod.rs:1)

## Scraper Runtime Path

1. Lifecycle action normalization: [src/api/scraper_control.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/scraper_control.rs:1)
2. Runtime orchestration service: [src/operator/runtime.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/operator/runtime.rs:1)
3. Runtime ownership and worker startup: [src/api/state.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/state.rs:18)
4. Active scraper implementation: [src/scraper/archive_scraper.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scraper/archive_scraper.rs:1)
5. Lower-level compatibility loop retained for non-web flows: [src/scraper/main_scraper.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scraper/main_scraper.rs:1)
6. Scraper state machine: [src/scraper/state.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scraper/state.rs:1)

## Auth And Operator Control Path

1. Route capability bands: [src/api/routes.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/routes.rs:128)
2. Authentication and minimum-role enforcement: [src/auth/middleware.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/auth/middleware.rs:1)
3. Canonical role model and compatibility aliases: [src/auth/roles.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/auth/roles.rs:1)
4. User storage and role normalization: [src/auth/users.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/auth/users.rs:1)

## Scan Request Path

1. HTTP handler boundary: [src/api/scanner_handlers.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/scanner_handlers.rs:139)
2. Request acceptance and orchestration: [src/api/scanner_service.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/scanner_service.rs:85)
3. Scan execution, queueing, and pause/shutdown control: [src/scanning/mod.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scanning/mod.rs:24)
4. Stable scan provenance and evidence domain types: [src/scanning/domain.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scanning/domain.rs:1)
5. TruffleHog adapter and clone policy: [src/scanning/trufflehog.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scanning/trufflehog.rs:1)

## Persistence Path

1. Canonical runtime database abstraction: [src/core/database.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/core/database.rs:1)
2. Shared persistence boundary used by handlers, workers, realtime monitor, and scanner: [src/core/persistence_service.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/core/persistence_service.rs:1)
3. Event-queue claim/start path: [src/api/state.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/state.rs:121)
4. Stable scan provenance model: [src/scanning/domain.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scanning/domain.rs:1)
5. Scan artifact persistence adapter: [src/scanning/persistence.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/scanning/persistence.rs:1)

## Active Abstraction Choices

- Primary runtime database abstraction: `src/core/database.rs::Database`
- Primary active persistence entrypoint for runtime and API flows: `src/core/persistence_service.rs::PersistenceService`
- Compatibility wrapper kept only for the legacy main scraper: `src/core/enhanced_database.rs::DatabaseManager`
- Active runtime lifecycle orchestration: `src/operator/runtime.rs::ScraperRuntimeService`
- Active grouped CLI surface: `src/operator/cli.rs`
- Active CLI command execution layer: `src/operator/service.rs`
- Legacy CLI code is no longer part of the default crate surface; it is feature-gated behind `experimental`
- Canonical operator roles are `admin`, `operator`, and `read_only`; legacy `user`/`viewer` values are compatibility aliases only
- Public API handlers should delegate to service modules instead of embedding orchestration logic
