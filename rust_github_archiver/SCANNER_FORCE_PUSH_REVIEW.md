# Deleted Commit & Force-Push Scanner Review

## Executive summary
- The current Rust scanner does **not** reproduce the "deleted commit" workflow from the reference script/blog: it never successfully fetches or scans the dangling `before` commits from zero-commit PushEvents, and the detection logic in the real-time monitor is inverted (it treats *existing* commits as dangling and skips actual deleted ones).
- Background workers correctly enqueue PushEvents into `pending_push_scans`, but `perform_repository_scan` ignores the claimed queue items and instead re-queries the last 25 events from `github_events`, which means the commits you intended to process are never fed to TruffleHog.
- Even if the right events were supplied, the Git-based approach (`GitCloner::identify_base_commit`) calls `git fetch <before_sha>` which fails for force-pushed commits (by definition those refs were removed). The code then falls back to scanning `HEAD`, so deleted history is not inspected at all.
- The historical BigQuery path retrieves zero-commit PushEvent metadata, but the commits are not archived locally and the results are neither persisted nor piped into the scanner/queue, so it cannot mirror the reference script that mined GH Archive for dangling commits.

## Expected workflow (per reference script/blog)
From `to-do.md` and the embedded TruffleHog blog summary:
1. Query GH Archive / GitHub Events for PushEvents whose `commits` array is empty (`size == 0`) – these indicate force-pushes.
2. For each candidate, grab the `before` SHA and retrieve the full commit diff (even if it is no longer reachable) via `git cat-file`, `git show`, or archival data (GH Archive keeps the payload, and GitHub will still serve the blob if you fetch `+refs/*`).
3. Run a detector (TruffleHog/secrets-ninja) against that deleted commit payload, then store the findings.
4. Repeat for historical ranges (BigQuery) and in real time (GitHub Events API/webhook) so that no dangling commit escapes scanning.

## What the current code actually does
### Event ingestion & queueing
- `process_push_event` (in `src/realtime/mod.rs`) saves every PushEvent to `github_events` and optionally enqueues it in `pending_push_scans`.
- The queue worker in `src/api/state.rs` periodically claims events and calls `scanning_service.start_scan(.., event_ids)` so the scanner can process each repo incrementally.

### Real-time "dangling" detection
- `process_push_event` calls `check_for_dangling_commit`, which simply invokes `DanglingCommitFetcher::fetch_commit` (GitHub REST API `/repos/{owner}/{repo}/commits/{sha}`) and returns whatever the API returned.
- **Bug:** `fetch_commit` already returns `Ok(None)` when the API says 404, but `process_push_event` treats `Ok(Some(commit))` as "Found dangling commit" and `Ok(None)` as "Commit exists, not dangling". The condition is reversed, so real dangling commits are never scanned.
- The secret scan performed here (`scan_commit_for_secrets`) just concatenates the commit message plus any returned file patches and runs the lightweight regex-based `SecretScanner`, not TruffleHog.

### Scanner execution path
- `start_scan` records the job and spawns `execute_scan`, which eventually calls `perform_repository_scan`.
- `perform_repository_scan` retrieves events via `fetch_commits_from_event_store`, which reads **only** from `github_events` (`Database::get_push_events_for_repository`) and ignores the specific queue entries that triggered the scan. The claimed `event_ids` are merely marked completed/failed; their `before` SHAs are never used.
- For each event returned from `github_events`, the scanner:
  1. Clones the repo (`GitCloner::partial_clone`).
  2. Calls `identify_base_commit(repo_path, before_sha)` → `git fetch origin <before_sha>`.
  3. Runs `trufflehog git --since-commit <base> --branch <head/ref>`.
- When `git fetch <before_sha>` fails (the normal case for deleted commits), the event is skipped and the scanner eventually falls back to `scan_repository(.., "", "HEAD")`, i.e., a full HEAD scan unrelated to the zero-commit event.

### BigQuery / historical mode
- `BigQueryScanner::scan_zero_commit_events` builds the right SQL (filters `JSON_EXTRACT_ARRAY(payload, '$.commits') = []`).
- However, `run_bigquery_scan` and `IntegrationService::scan_organization_historical` merely list the events or try to re-fetch the commit via GitHub's commits API. There is no persistence of these events, no injection into `pending_push_scans`, and no mechanism to download the actual commit objects when GitHub returns 404.

## Gaps vs. the desired workflow
| # | Expectation | Current behavior | Impact |
|---|-------------|------------------|--------|
|1|Detect zero-commit PushEvents by checking `payload.commits == []` / `size == 0`|`process_push_event` never inspects `payload.commits`; it only checks that `before_sha` is non-zero and then queries the commits API|Every forced push is treated the same as a normal push, leading to noisy queue entries and no guarantee that zero-commit events are prioritized|
|2|When a commit is missing (dangling), treat it as the interesting case|`check_for_dangling_commit` treats `Some(commit)` as dangling and `None` as "exists"|Actual deleted commits are skipped entirely; existing commits trigger false alerts|
|3|Use the queued event data (before/head/ref) when invoking TruffleHog|`perform_repository_scan` ignores the queue payload and simply re-queries the last N events from `github_events`, which may not include the claimed events|The committed work of queuing zero-commit events is wasted; scans may never touch the intended commit SHA|
|4|Fetch deleted commits even when GitHub rejects regular fetches (e.g., via `git fetch origin +refs/*`, GH Archive blob download, or cached payload)|`GitCloner::identify_base_commit` relies on `git fetch origin <before_sha>` and aborts when the ref no longer exists|Scanning deleted history never happens; the code falls back to scanning HEAD|
|5|Historical GH Archive / BigQuery path should download and scan commit blobs|`scan_zero_commit_events` only lists metadata, and `scan_organization_historical` still calls the live commits API, which returns 404 for actual dangling commits|Historical dangling commits cannot be reconstructed, so the workflow from the reference script is missing|
|6|Reference script performs per-commit scanning (diff-only) to minimize noise|Current TruffleHog run scans entire branches/HEAD, producing unrelated findings and missing the exact deleted diff|Signal/noise ratio worsens and deleted commits remain unscanned|

## Recommended improvements
1. **Fix dangling detection logic** in `process_push_event`:
   - Treat `Ok(None)` as "dangling" (commit missing) and `Ok(Some(..))` as "exists".
   - Compare `payload.size` / `payload.commits` to verify zero-commit PushEvents before enqueuing.
2. **Use the claimed queue payload** when scanning:
   - Extend `ScanningService` to accept the `EventScanTarget` objects returned by `claim_pending_push_events` (they already contain `before_sha`, `head_sha`, `forced`, etc.).
   - Remove the redundant `fetch_commits_from_event_store` call or at least filter by the exact `event_id`s passed into `start_scan`.
3. **Handle unreachable commits explicitly**:
   - Teach `GitCloner` to create an orphaned ref for the `before_sha` (e.g., `git fetch origin +<before_sha>:refs/temp/<sha>` with `uploadpack.allowReachableSHA1InWant=true`), and when that fails, fall back to the GH Archive payload (store the raw PushEvent JSON so you can reconstruct the diff locally as the reference script does).
   - Alternatively, use the REST API `GET /repos/{owner}/{repo}/git/commits/{sha}` which can still return the object even after force-push, and stream that to TruffleHog without recloning the repo.
4. **Persist and scan GH Archive / BigQuery results**:
   - When `scan_zero_commit_events` returns events, enqueue them (`pending_push_scans`) or store their raw payload so an offline worker can reconstruct the commit diff even if the live repo no longer has it.
   - Record successes/failures so you do not rescan the same historical event repeatedly.
5. **Align detectors with the reference script**:
   - Replace the lightweight regex scan in `scan_commit_for_secrets` with either TruffleHog CLI (pointed at the reconstructed commit) or the same detector set used elsewhere, so real-time detections produce comparable results.
6. **Add regression tests**:
   - Create fixtures for a zero-commit PushEvent payload and ensure the pipeline marks it dangling, enqueues it, and hands the right `before/head` SHAs to the scanner without falling back to HEAD.

## Suggested next steps
1. Patch `check_for_dangling_commit`/`process_push_event` so that missing commits trigger scanning, then add logging/metrics to confirm zero-commit events are detected.
2. Refactor `ScanningService::perform_repository_scan` to take an explicit `Vec<EventCommit>` (supplied by the queue worker) and to fetch commit objects via a method that works for deleted refs.
3. Wire the BigQuery/archival path into `pending_push_scans`, storing the raw payload so that even if GitHub APIs refuse the commit later, you still have the diff to scan (this is what the reference script relied on).
4. Once the data flow works end-to-end, re-run the workflow described in the blog (scan historical zero-commit events, confirm that dangling commits produce detections) and backfill the database.
