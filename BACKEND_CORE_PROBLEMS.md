# Remaining Core Backend Problems (as of May 13, 2026)

This note summarizes unresolved backend risks observed from repository tracking docs.

## 1) Event ingestion persistence is not fully verified
- The event monitor reports successful fetches, but database insertion confirmation is missing.
- Documented possibilities include missing DB wiring, silent save failures, or log filtering.
- This is currently the highest-priority reliability concern.

## 2) Throughput scaling target remains open
- The 1M+ events/hour scaling objective is still unchecked.
- This indicates the current backend likely has not met intended production throughput goals.

## 3) Rate-limit and auth hardening still incomplete
- Rate-limit-awareness is still an open item.
- Current status notes unauthenticated constraints and credential warnings.

## 4) Pipeline/scanner test coverage needs expansion
- Unit/flow-level tests for pipelines and scanners remain an open TODO.
- This is a material regression risk for ingestion/scanning changes.

## 5) Production observability and resilience gaps
- `cargo-audit` integration and Sentry telemetry are still open.
- Setup-level restart loop with health checks also remains open.

## 6) Documentation hygiene issue impacts backend prioritization
- The TODO file contains pasted long-form external content, creating noise and making backend priorities harder to track.

## Recommended execution order
1. Validate and fix DB persistence path for GitHub events.
2. Add focused integration tests for fetch -> parse -> persist.
3. Enforce robust rate-limit/credential behavior.
4. Add audit + telemetry + process recovery automation.
5. Optimize for throughput targets once correctness and observability are stable.
