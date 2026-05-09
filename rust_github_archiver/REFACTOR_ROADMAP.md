# GitHub Archiver Refactor Roadmap

This roadmap turns the current codebase into a professional research platform for offensive security analysis workflows: stable ingestion, reproducible scanning, evidence-grade persistence, and predictable operator controls.

The goal is not to bolt on more features. The goal is to remove ambiguity, duplicate control planes, placeholder behavior, and hidden coupling so the system becomes reliable under sustained research workloads.

## North Star

The refactored system should have:

- One runtime orchestration layer for scraper, scanner, realtime monitor, and shutdown.
- One canonical scan domain model shared by API, CLI, workers, and persistence.
- Thin handlers and commands that delegate to service modules instead of embedding business logic.
- Versioned API contracts with no placeholder payloads in production routes.
- Evidence-first persistence: detections, source events, commits, artifacts, provenance, and audit data remain traceable.
- Research-safe defaults: deterministic concurrency, bounded resources, redacted secrets in logs, and explicit operator intent.

## Current Structural Problems

- [x] Runtime control is spread across handlers, `AppState`, `MainScraper`, and public fallback endpoints.
- [ ] API/CLI/business logic are mixed together in large files (`src/api/handlers.rs`, `src/main.rs`, `src/scanning/mod.rs`).
- [x] Several placeholder or legacy files are still inside the active source tree (`*.backup`, `database_old.rs`, placeholder middleware/modules).
- [x] There are multiple near-duplicate health/status concepts and partially overlapping database abstractions.
- [x] Frontend and backend contracts are not fully normalized; some endpoints still imply placeholder or transitional behavior.
- [x] Module boundaries do not clearly separate ingestion, scan execution, evidence persistence, auth, and operator control.

## Refactor Phases

### Phase 0: Baseline and Guardrails

- [x] Freeze the public API surface in a short compatibility document.
- [x] Add mandatory `fmt`, `clippy --all-targets -D warnings`, lib tests, and integration tests to the default CI path.
- [x] Inventory all placeholder, backup, old, and duplicate files; classify each as migrate, delete, or archive.
- [x] Add a “no placeholder responses on production routes” rule.
- [x] Add an architecture index describing the active execution path for web server, CLI, scraper, and scanner.

Acceptance criteria:

- [x] A new contributor can identify the production code path without reading backup/legacy files.
- [x] The default engineering gate is deterministic and green.

### Phase 1: Runtime Orchestration and Control Plane

- [x] Introduce a dedicated runtime/service layer for scraper lifecycle operations.
- [x] Make API control endpoints and generic control actions share one code path.
- [x] Remove split ownership of lifecycle state between handlers and runtime objects.
- [x] Normalize status reporting so health, scraper state, and database readiness come from one source of truth.
- [x] Centralize startup rollback rules when runtime initialization fails.

Acceptance criteria:

- [ ] A cold boot, start, pause, resume, stop, and restart sequence behaves identically from UI, API, and CLI.
- [x] No handler directly reimplements scraper lifecycle behavior.

### Phase 2: Scan Pipeline and Queueing

- [x] Split DTO parsing from domain scan config creation.
- [x] Separate scan request acceptance, queueing, execution, and persistence into explicit modules.
- [x] Make concurrency limits request-aware and testable.
- [x] Separate detector behavior from verification behavior and remove config-field overloading.
- [x] Introduce clearer worker contracts for event-driven scans vs. manual repository scans.

Acceptance criteria:

- [ ] The same scan config yields the same execution behavior across manual, batch, scheduled, and worker-triggered scans.
- [x] Queue processing can be reasoned about without reading handler code.

### Phase 3: Canonical Domain and Persistence Model

- [x] Consolidate overlapping database abstractions and define one canonical health/status model.
- [x] Define stable domain types for repositories, source events, commits, scans, findings, and evidence artifacts.
- [x] Move persistence mapping out of runtime loops and into dedicated repositories/adapters.
- [x] Store provenance and failure reasons consistently for every scan path.
- [x] Normalize naming so “secret”, “finding”, “detection”, and “claim” are not used interchangeably without intent.

Acceptance criteria:

- [x] A finding can be traced from API response back to source event and persistence row without guessing.
- [x] Database access logic is not spread across handlers, runtime loops, and model modules.

### Phase 4: API and CLI Unification

- [x] Move command execution into service modules shared with the web API where appropriate.
- [x] Gate optional dependencies only on commands that truly require them.
- [x] Define operator-focused command groups: ingest, scan, research, triage, admin.
- [x] Version scanner and monitoring endpoints and validate payloads at the boundary.
- [x] Remove or quarantine experimental commands from the primary operator path.

Acceptance criteria:

- [ ] API and CLI produce equivalent behavior for equivalent operations.
- [x] Non-scan administrative commands do not fail on missing scan-only dependencies.

### Phase 5: Frontend Contract Cleanup

- [x] Replace placeholder dashboard behavior with explicit loading, empty, degraded, and unavailable states.
- [x] Generate or formalize a typed frontend API contract for critical endpoints.
- [x] Remove hardcoded assumptions about legacy routes and duplicate status sources.
- [x] Break dashboard script logic into composable modules instead of a monolithic page script.
- [x] Add a minimal operator workflow smoke test: login, start runtime, inspect queue, trigger scan, review findings.

Acceptance criteria:

- [x] The dashboard reflects real backend state without fallback guesswork.
- [x] UI failures can be mapped to one API contract, not multiple inferred payload shapes.

### Phase 6: Research Platform Hardening

- [ ] Redact raw secret material from logs, metrics labels, and operator-visible incidental traces.
- [x] Enforce capability boundaries for admin, operator, and read-only roles.
- [x] Add bounded caches and artifact retention rules for large-scale repository analysis.
- [x] Add structured audit events for research-critical actions: scan launch, schedule create/update, export, cleanup, token changes.
- [x] Add deterministic shutdown and resume behavior for long-running scans.

Acceptance criteria:

- [ ] The platform is safe to run continuously in a research environment without leaking sensitive values into logs or unstable temp state.
- [ ] Operator actions remain attributable and reproducible.

### Phase 7: Dead Code Removal and Packaging

- [x] Remove `*.backup`, `*_old.rs`, `database_complete.rs`, and similar shadow files after migration.
- [x] Move experimental subsystems behind features or into a dedicated `experimental/` area.
- [x] Collapse placeholder modules or delete them if unused.
- [x] Publish a smaller, documented top-level crate/module surface.
- [x] Update README, testing guide, and operator runbooks to match the refactored architecture.

Acceptance criteria:

- [ ] The active source tree only contains code that is compiled, tested, or intentionally feature-gated.
- [ ] New maintainers can navigate the codebase without tribal knowledge.

## Execution Checklist

### Completed in this changeset

- [x] Fixed scraper cold-start and lock/lifecycle regressions in the runtime path.
- [x] Restored batch scan `max_concurrent` behavior and stopped misusing `include_private` as a verification flag.
- [x] Limited TruffleHog CLI gating to commands that actually require scan execution.
- [x] Introduced a shared scraper control service module so lifecycle actions no longer live in multiple handler implementations.
- [x] Switched the explicit scraper endpoints and the generic `/api/scraper/control` endpoint onto the same lifecycle execution path.
- [x] Re-established green local engineering gates with `fmt`, `check`, `clippy`, and test coverage.
- [x] Extracted scanner acceptance/orchestration out of `src/api/scanner_handlers.rs` into `src/api/scanner_service.rs`.
- [x] Replaced the placeholder API CORS middleware with a tested compatibility wrapper over the production security middleware.
- [x] Fixed the `require_api_key` usage doctest so the default Rust test path stays green.
- [x] Unified active database health/status reporting around `src/core/database.rs::DatabaseHealth` and documented `Database` as the primary runtime abstraction.
- [x] Added direct auth middleware unit tests and removed the remaining placeholder TODO block from `src/api/auth_middleware.rs`.
- [x] Replaced the last active placeholder scanner export response with an explicit compatibility contract for `json`/`csv` and `501` for `pdf`.
- [x] Added `docs/API_COMPATIBILITY.md`, `docs/ARCHITECTURE_INDEX.md`, and `docs/SHADOW_FILE_INVENTORY.md`.
- [x] Removed inactive shadow files: `src/api/handlers.rs.backup`, `src/core/database.rs.backup`, `src/core/database_old.rs`, and `src/core/database_complete.rs`.
- [x] Introduced `src/scanning/domain.rs` and `src/scanning/persistence.rs` so scan provenance, worker origin, and evidence persistence no longer live in the main execution loop.
- [x] Versioned the active scanner, monitoring, runtime, and database status contracts under `/api/v1/...` and added request-boundary validation for scanner entrypoints.
- [x] Removed production-route sample/demo database stats responses and switched the dashboard to explicit `ready`, `empty`, and `unavailable` database states.
- [x] Split dashboard scanner/runtime/export logic into `dashboard_assets/` modules and switched the critical dashboard flows onto the typed `/api/v1/...` contracts.
- [x] Redacted persisted secret previews and added structured audit events for scan launch, schedule creation, export, emergency cleanup, and token-pool mutations.
- [x] Feature-gated the legacy `src/cli.rs` module behind `experimental` and updated the architecture/testing docs to match the active crate surface.
- [x] Introduced canonical `admin`/`operator`/`read_only` roles with compatibility aliases for legacy `user`/`viewer` records and enforced minimum-role route boundaries for read-only, operator, and admin APIs.
- [x] Removed the hardcoded auth verification identity payload and updated the compatibility, README, testing, and operations docs to reflect the active capability model.
- [x] Moved active runtime lifecycle ownership into `src/operator/runtime.rs`, removed the `AppState` compatibility initialization flag, and routed runtime shutdown through the shared service layer.
- [x] Extracted CLI command execution helpers into `src/operator/service.rs` so `src/main.rs` delegates to shared operator modules instead of embedding command bodies.
- [x] Added stable repository, commit, finding, and evidence artifact domain types in `src/scanning/domain.rs` and persisted them through `src/scanning/persistence.rs`.
- [x] Added explicit scan execution state control for pause, resume, and shutdown in `src/scanning/mod.rs` and covered it with runtime/operator smoke tests.
- [x] Renamed the active scan-domain result model around findings while keeping persisted detection rows explicit and compatibility aliases in the secrets model.
- [x] Moved the primary CLI surface into grouped operator commands under `src/operator/cli.rs` and reduced `src/main.rs` to a thin bootstrap.
- [x] Added repository-cache retention, bounded cache cleanup, and configurable in-memory performance cache expiry for long-running research workloads.
- [x] Replaced the remaining active GUI and repository-scan placeholder behavior with explicit runtime execution or explicit non-active-path errors.
- [x] Introduced `src/core/persistence_service.rs` and routed active API, worker, realtime-monitor, and scan-persistence database interactions through one shared persistence boundary.
- [x] Hardened the sustained-load path with pooled database connection lifecycle settings, composite hot-path indexes for queue/repository scans, cheaper overview aggregation queries, and reduced dashboard background polling pressure.

### Next execution slices

- [ ] Finish converging the remaining large mixed modules, especially `src/api/handlers.rs` and `src/scanning/mod.rs`, into thinner boundary/orchestration layers.
- [ ] Make cold boot, start, pause, resume, stop, and restart behavior equivalent across the web UI, API, and CLI paths.
- [ ] Align manual, batch, scheduled, and worker-triggered scan execution so the same config yields the same behavior across every entrypoint.
- [x] Reduce the remaining spread of database access logic across handlers, runtime loops, and model layers.
