# API Compatibility Contract

This document freezes the active production API surface used by the dashboard, CLI-adjacent operators, and automation.

Canonical authenticated roles are `admin`, `operator`, and `read_only`. Legacy stored roles `user` and `viewer` remain accepted as compatibility aliases and normalize to `operator` and `read_only` in API responses.

## Scope

The compatibility surface is defined by route registration in [src/api/routes.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/api/routes.rs:128) and the handler/service boundary behind it.

Active route groups:

- `GET /ping`
- `GET /api/health`
- `POST /api/auth/login`
- `GET /api/auth/verify`
- `GET /api/auth/status`
- `POST /api/auth/logout`
- `GET /api/auth/user`
- `POST /api/auth/change-password`
- `GET /api/auth/users`
- `POST /api/auth/users`
- `DELETE /api/auth/users/:id`
- `GET /api/status`
- `GET /api/system/status`
- `GET /api/system/metrics`
- `GET /api/scraper/status`
- `POST /api/scraper/control`
- `POST /api/start-scraper`
- `POST /api/stop-scraper`
- `POST /api/pause-scraper`
- `POST /api/resume-scraper`
- `POST /api/restart-scraper`
- `GET /api/database/status`
- `GET /api/database/stats`
- `POST /api/database/start`
- `POST /api/database/stop`
- `POST /api/database/restart`
- `GET /api/config`
- `POST /api/scanner/scan`
- `POST /api/scanner/batch-scan`
- `GET /api/scanner/results`
- `GET /api/scanner/statistics`
- `GET /api/scanner/detectors`
- `GET /api/scanner/metrics`
- `GET /api/scanner/export`
- `POST /api/scanner/schedule`
- `GET /api/scanner/schedules`
- `GET /api/monitoring/metrics`
- `GET /api/monitoring/overview`
- `GET /api/monitoring/trends`
- `GET /api/monitoring/logs`
- `GET /api/monitoring/logs/export`
- `GET /api/monitoring/ws`
- `POST /api/keys`
- `GET /api/keys`
- `GET /api/keys/:id`
- `POST /api/keys/:id/deactivate`
- `DELETE /api/keys/:id/delete`
- `POST /api/keys/:id/regenerate`
- `GET /api/keys/statistics`
- `GET /api/keys/types`
- `POST /api/keys/validate`
- `POST /api/tokens/add`
- `GET /api/tokens/stats`
- `GET /api/tokens/details`
- `POST /api/tokens/remove-unhealthy`
- `POST /api/tokens/reset-health`
- `POST /api/webhooks/add`
- `POST /api/webhooks/remove`
- `POST /api/webhooks/update`
- `GET /api/webhooks/list`
- `GET /api/webhooks/stats`
- `GET /api/metrics`
- `GET /api/metrics/report`
- `POST /api/metrics/reset`
- `GET /api/health/extended`
- `GET /api/audit/logs`
- `GET /api/audit/logs/:id`
- `GET /api/audit/stats`
- `GET /api/audit/export`
- `POST /api/audit/cleanup`
- `POST /api/realtime/start`
- `POST /api/realtime/stop`
- `POST /api/realtime/pause`
- `POST /api/realtime/resume`
- `GET /api/realtime/status`
- `GET /api/realtime/events`
- `POST /api/realtime/config`
- `POST /api/realtime/stats/reset`
- `GET /metrics`
- `GET /health`
- `GET /health/live`
- `GET /health/ready`

Versioned aliases now exist for the runtime, scanner, and monitoring contracts used by the dashboard:

- `GET /api/v1/status`
- `GET /api/v1/system/status`
- `GET /api/v1/system/metrics`
- `GET /api/v1/scraper/status`
- `POST /api/v1/scraper/control`
- `GET /api/v1/database/status`
- `GET /api/v1/database/stats`
- `GET /api/v1/scanner/results`
- `GET /api/v1/scanner/statistics`
- `GET /api/v1/scanner/detectors`
- `POST /api/v1/scanner/scan`
- `POST /api/v1/scanner/batch-scan`
- `GET /api/v1/scanner/export`
- `GET /api/v1/scanner/metrics`
- `POST /api/v1/scanner/schedule`
- `GET /api/v1/scanner/schedules`
- `GET /api/v1/monitoring/metrics`
- `GET /api/v1/monitoring/overview`
- `GET /api/v1/monitoring/trends`
- `GET /api/v1/monitoring/logs`
- `GET /api/v1/monitoring/logs/export`
- `GET /api/v1/monitoring/ws`

## Compatibility Rules

- Existing route paths stay stable unless a versioned replacement is added first.
- Response field removals or semantic changes require either a compatibility shim or a new versioned route.
- Unsupported production behavior must return an explicit HTTP error. Production routes must not return placeholder payloads that pretend success.
- Handler modules stay thin. Contract shaping belongs at the API boundary, while execution belongs in service modules.

## Protected Capability Bands

- `read_only`: authenticated observation routes including user/session state, password changes for the current user, scan exports, schedule listing, monitoring overview/trends/logs, metrics reports, and extended health.
- `operator`: runtime control routes including scraper lifecycle, scanner launch/schedule actions, realtime monitor control, and token/webhook read surfaces used by active operations.
- `admin`: privileged control routes including database lifecycle, user management, API key management, token/webhook mutation, audit-log access, and metrics reset.
- Public compatibility routes remain public only where the existing dashboard and health/metrics surface still depends on them.

## Export Contract

`GET /api/scanner/export` returns a JSON export bundle for `format=json` and `format=csv`.
`GET /api/v1/scanner/export` is the versioned alias with the same behavior.

- `format=json`: the response body is the export bundle.
- `format=csv`: the response body is still the export bundle; the dashboard converts `detections` to CSV client-side.
- `format=pdf`: explicitly unsupported and returns `501 Not Implemented`.

## Database Stats Contract

`GET /api/database/stats` and `GET /api/v1/database/stats` now return one of these explicit states:

- `status=ready`: live database statistics are available.
- `status=empty`: the database is reachable but currently has no persisted events/tables to summarize.
- `status=unavailable`: statistics could not be loaded; the route returns `503 Service Unavailable`.

## Change Control

Before changing a route in the frozen surface:

1. Update this document.
2. Update the dashboard/CLI caller if applicable.
3. Add or update request/response tests for the changed contract.
