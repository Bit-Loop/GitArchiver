# TODO.md

## 🛠️ Integration and Implementation Tasks

### 1. BigQuery Scanning Pipeline
- [X] Use `gcp-bigquery-client` crate.
  - Source: https://crates.io/crates/gcp-bigquery-client
- [X] Query GH Archive for zero-commit PushEvents since 2011,, use this to catch the DB up witout buring out the API.
- [X] Filter by org/user.
- [X] Extract `before` commit hashes.

### 2. Dangling Commit Fetcher
- [X] Use `octocrab` crate to hit `/repos/{org}/{repo}/commits/{hash}`.
- [X] Handle GitHub API rate limits (exponential backoff).
- [X] Add Redis caching (`redis` crate).

### 3. Secret Scanner
- [X] Wrap TruffleHog CLI or re-implement with `regex` crate.
- [X] Implement 50+ secret detectors (AWS, GitHub PATs, MongoDB, etc.).
- [X] Scan entire commit diffs and patches.

### 4. Secrets Validation/Investigation
- [X] Port Secrets Ninja to Rust GUI using `iced` or `druid`.
  - Reference: https://github.com/NikhilPanwar/secrets-ninja
- [X] Validate secrets (e.g., AWS STS call).
- [X] Query owners and permissions via APIs.
- [X] Triage: corporate emails, file types (.env), time-series leak graphs.

### 5. AI Triage Agent
- [X] Integrate local LLM using `llm` crate + LLaMA model.
- [X] Auto-score impact, detect bounties, suggest revocation.
- [X] Offline: deduplicate DB, optimize queries.
- [X] Manage and optimize secret wordlists using AI.

### 6. Real-Time Enhancements
- [X] Poll GitHub Events API.
- [X] Add webhook server via `actix-web`.
- [X] Live zero-commit detection on PushEvents.

### 7. Expanded Event Support
- [X] Add support for `pull_request`, `issue_comment`, and `release` events.
- [X] Scan for secrets in descriptions, comments, and assets.
- [X] Dynamically update schema with event relationships.

### 8. Performance & Scaling
- [X] Parallel processing with `rayon`.
- [X] Distribute using Docker + Kubernetes.
- [X] Caching layer: `rusqlite` + `lru-cache`.
- [X] Create DB schema with relations and indexes (events, commits, secrets).

### 9. UI / Dashboard
- [ ] Build GUI with Tauri.
- [ ] Implement lava lamp–style dynamic background:
  - Green: healthy
  - Yellow: warnings
  - Red: critical
  - Add 3D-like lighting and shading for visual cues.

### 10. Automation / Output
- [ ] Auto-generate bounty reports (PDF via `printpdf`).
- [ ] Export data to JSON and CSV.
- [ ] `setup.sh`: Add restart loop with health checks.

---

## ⚙️ Overall Improvements

- [ ] Scale to 1M+ events/hour using batching.
- [ ] Brute-force partial commit hashes.
- [ ] Scan non-dangling commits.
- [ ] Integrate TruffleHog Analyze for deeper secret enumeration.
- [ ] Implement Force Push Scanner per:
  - https://trufflesecurity.com/blog/guest-post-how-i-scanned-all-of-github-s-oops-commits-for-leaked-secrets

---

## 🔐 Security & Ethics

- [ ] Add responsible disclosure mode.
- [ ] Enforce rate-limit awareness.

---

## 🧪 Testing & Deployment

- [ ] Unit tests for pipelines and scanners.
- [ ] Fuzzing for secret detectors.
- [ ] GitHub Actions CI/CD pipeline.
- [ ] Containerize for deployment across orgs.

---

## 📦 Code Quality & Documentation

- [ ] Integrate `cargo-audit` for vuln scanning.
- [ ] Use `sentry` crate for logging and telemetry.
- [ ] Optimize bottlenecks with `flamegraph`.
- [ ] Document via `mdBook`.
- [ ] Add API references and usage examples.

---

## 🤝 OSS Contribution Ready

- [ ] Add `LICENSE`
- [ ] Add `CODE_OF_CONDUCT.md`
- [ ] Add issue and PR templates

---

## 🚀 10x Competitive Advantages

- [ ] Implement full offline scanning pipeline (no dependency on external APIs).
- [ ] Prebuilt curated wordlists for specific orgs (e.g., internal naming schemes, service prefixes).
- [ ] AI-enhanced secret detection with NLP/LLM context parsing (e.g., differentiate real vs. decoy keys).
- [ ] Automatically correlate leaked keys with known public infrastructure (Shodan, Censys integration).
- [ ] Build internal "zero-day" scanner to detect org-specific secrets before they are leaked (proactive detection).
- [ ] Visualize secret propagation across forks and clones.
- [ ] Add honeypot alerting (detect if attacker uses a fake leaked key).
- [ ] Include Rust static analysis tools to find embedded secrets at compile time.
- [ ] Embed CVE detection in GitHub Action workflows to catch exposed libraries early.
- [ ] Offer org-wide GitHub repo scanning as a packaged CLI or GUI utility.
- [ ] Support export/import of analysis sessions for collaboration and team-based bounty hunting.
- [ ] Use local embeddings + LLM to match leaks to real-world services for impact estimation.
- [ ] Support SSH/PGP/PEM format secret detection + entropy heuristics to reduce false positives.
- [ ] Implement a "secret radar" that continuously monitors GitHub for new leaks in real-time.
















### TRUFFLE HOG BLOGPOST:
You're right. Here's the entire markdown output, correctly enclosed in a fenced code block:

````markdown
# Learn how AI coding assistants can introduce security risks — Register for the webinar

---

# The Dig

### Guest Post: How I Scanned all of GitHub’s “Oops Commits” for Leaked Secrets  
**By Sharon Brizinov**

---

## TL;DR

- GitHub Archive logs every public commit — even deleted ones.
- Force pushes may erase them from history, but GitHub keeps "dangling" commits forever.
- These show up as **zero-commit PushEvents**.
- I scanned all zero-commit events since 2020 — found **$25k+** in bug bounty secrets.
- Tool open-sourced with **Truffle Security** to scan your own org's commits: [Try it here](https://github.com/NikhilPanwar/secrets-ninja)

---

## Overview

### What’s in this post:

- What it means to delete a commit  
- Using the GitHub Event API  
- Finding all deleted commits  
- Building automation  
- Hunting for secrets  
- Case study: Massive supply-chain compromise  
- Summary

---

## Background

I'm **Sharon Brizinov** — white-hat hacker focused on OT/IoT vulnerabilities. I also hunt bugs.

Previously:  
**How I Made 64k From Deleted Files** using TruffleHog.

Now:  
Accessing **100% of deleted commits** on GitHub.

I compiled tools and ideas:

- GitHub Event API  
- [Neodyme: Hidden GitHub Commits](https://neodyme.io)  
- [TruffleHog’s deleted commit scanning posts](https://trufflesecurity.com)  
- gharchive.org  

My approach:  
Scan all **Zero-Commit PushEvents** from GH Archive with automation to hunt secrets.

---

## What Does it Mean to Delete a Commit?

You might think:

```bash
git reset --hard HEAD~1
git push --force
```

...makes a secret vanish. Wrong.

Even after deleting a commit:

- GitHub still **retains access via the commit hash**
- Even 4 hex digits (e.g., `9eef`) may suffice

Use `git cat-file`, `git rev-list`, and `git ls-tree` to inspect tree/blob structures.

---

## Why It Happens

GitHub retains all commit objects (for pull requests, forks, audit logs, etc).  
Examples:

```bash
git -c "remote.origin.fetch=+refs/*:refs/remotes/origin/*" fetch origin
```

This ensures nothing is *truly* deleted.

---

## GitHub Event API

**No API token required.**

```bash
curl http://api.github.com/events
```

Use it to monitor push events.

**But for past data?**  
Use **[GH Archive](https://gharchive.org)** — example:

```
https://data.gharchive.org/2015-01-01-15.json.gz
```

---

## Finding Force-Pushed Deleted Commits

Search GH Archive for **PushEvent** entries with `commits: []`.

These represent **force pushes** that deleted commit(s).

Example deleted commit still live after 10 years:  
🔗 [grapefruit623/gcloud-python@e9c3d312](https://github.com/grapefruit623/gcloud-python/commit/e9c3d31212847723aec86ef96aba0a77f9387493)

---

## Scanning for Secrets

**Minimal repo clone** + **TruffleHog**:

```bash
git clone --filter=blob:none --no-checkout https://github.com/<ORG>/<REPO>
cd <REPO>
git fetch origin <COMMIT_HASH>
trufflehog git file://. --branch=<COMMIT_HASH>
```

---

## GitHub API

Query deleted commit:

```bash
https://api.github.com/repos/<ORG>/<REPO>/commits/<HASH>
```

Example:

```bash
https://api.github.com/repos/github/gitignore/commits/e9552d855c356b062ed82b83fcaacd230821a6eb
```

---

## Web Access to Commits

```plaintext
https://github.com/<ORG>/<REPO>/commit/<HASH>
https://github.com/<ORG>/<REPO>/commit/<HASH>.patch
https://github.com/<ORG>/<REPO>/commit/<HASH>.diff
```

---

## Building the Automation

Use GH Archive + GitHub Event API + TruffleHog.  
You don’t need to build it — we open-sourced the tool:

```bash
python force_push_scanner.py --db-file /path/to/force_push_commits.sqlite3 --scan <github_org/user>
```

Scans all GHArchive PushEvent data for `zero-commit` events and checks `before` commit for secrets.

---

## Hunting for Impactful Secrets

### 1. Manual Search  
- Focus on non-generic emails  
- Used **secrets-ninja** and **TruffleHog Analyze**  
- Reported in-scope secrets via Bug Bounty

### 2. Vibe-Coded Triage Platform  
- Built a no-backend front-end tool for quick filtering  
- Graphs showed:
  - Most secrets leaked in recent years
  - **MongoDB**, GitHub PATs, and AWS keys most common
  - `.env` most common leak source

---

## Everything AI

Goal: Use LLMs to **auto-evaluate secrets** for impact.

Future idea:  
Offline agent + open-source analysis → Pass metadata to LLAMA-based agent → Output risk score.

Work in progress...

---

## Case Study — Preventing a Massive Supply-Chain Compromise

- Found a GitHub PAT in a deleted commit
- It had **admin access** to all of **Istio’s** repositories  
- Could have triggered:
  - Code injections
  - Release tampering
  - Total repo wipe

**Istio (36k stars, 8k forks)** is critical infrastructure used by Google, IBM, Red Hat, etc.  
They responded swiftly and revoked the token.

---

## Summary

- Deleted ≠ Gone.
- Once committed, secrets must be revoked.
- GH Archive + GitHub Event API + TruffleHog = powerful combo.
- $25k+ earned. Thousands of secrets discovered.
- Platform built. Secrets triaged. Impact measured.

---

# The Dig

Insights, research, and field reports from Truffle Security Co.

**Jul 18, 2025** — GCP CloudQuarry: Searching for Secrets in Public GCP Images  
**Jul 11, 2025** — How to Scan Force Pushed Commits for Secrets

---

**STAY STRONG. DIG DEEP.**  
#trufflehog-community  
#SecretScanning  

© 2025 Truffle Security Co.  
[Privacy Policy](#) | [Terms](#) | [Data Processing](#) | [AUP](#)
````








### rust cargo - gcp-bigquery-client documentation:
````markdown
# GCP BigQuery Client (Rust)

> An ergonomic Rust async client library for interacting with Google BigQuery.

---

## 📦 Crate Links

- [Crates.io](https://crates.io/crates/gcp-bigquery-client)  
- [GitHub](https://github.com/lquerel/gcp-bigquery-client)  
- [Docs.rs](https://docs.rs/gcp-bigquery-client)

---

## 🔧 Features

- ✅ Support for **all BigQuery API endpoints**  
- ✅ Compatible with **Service Account Keys**, **Workload Identity**, **Installed Flow**, and other [`yup-oauth2`](https://docs.rs/yup-oauth2) authentication methods  
- ✅ Async API (uses `tokio`)  
- ✅ Serialize & Deserialize via `serde`  
- ✅ Full support for **nested/record JSON types**  
- ✅ Builder-pattern interface for datasets, tables, schemas, jobs  
- ✅ Partial support for **BigQuery Storage Write API**  
- ✅ BigQuery Emulator support

### TLS Options

- `rustls-tls` (default): Uses `rustls`  
- `native-tls`: Uses `OpenSSL`

---

## ✅ Supported BigQuery Resources

| Resource        | Status       | Notes               |
|-----------------|--------------|---------------------|
| Dataset         | ✅ Full       |                     |
| Table           | ✅ Full       |                     |
| Tabledata       | ✅ Full       | Streaming insert    |
| Job             | ✅ Full       |                     |
| Model           | ✅ Full       | Untested            |
| Routine         | ✅ Full       | Untested            |
| Project         | ✅ Partial    | Untested            |
| Storage Write   | 🟡 Partial    |                     |

---

## 🧪 Example Workflow

This example:

1. Loads environment variables:  
   `$PROJECT_ID`, `$DATASET_ID`, `$TABLE_ID`, `$GOOGLE_APPLICATION_CREDENTIALS`

2. Initializes the BigQuery client  
3. Deletes existing dataset (if any)  
4. Creates new dataset & table  
5. Inserts rows using streaming API  
6. Executes `SELECT COUNT(*)` query  
7. Drops table & dataset

```rust
// 1. Init client from service account key file
let client = gcp_bigquery_client::Client::from_service_account_key_file(gcp_sa_key).await?;

// 2. Remove old dataset
let _ = client.dataset().delete(project_id, dataset_id, true).await;

// 3. Create new dataset
let dataset = client.dataset().create(
    Dataset::new(project_id, dataset_id)
        .location("US")
        .friendly_name("Just a demo dataset")
        .label("owner", "me")
        .label("env", "prod"),
).await?;

// 4. Create a nested table
let table = dataset.create_table(
    &client,
    Table::from_dataset(
        &dataset,
        table_id,
        TableSchema::new(vec![
            TableFieldSchema::timestamp("ts"),
            TableFieldSchema::integer("int_value"),
            TableFieldSchema::float("float_value"),
            TableFieldSchema::bool("bool_value"),
            TableFieldSchema::string("string_value"),
            TableFieldSchema::record(
                "record_value",
                vec![
                    TableFieldSchema::integer("int_value"),
                    TableFieldSchema::string("string_value"),
                    TableFieldSchema::record(
                        "record_value",
                        vec![
                            TableFieldSchema::integer("int_value"),
                            TableFieldSchema::string("string_value"),
                        ],
                    ),
                ],
            ),
        ]),
    )
    .friendly_name("Demo table")
    .description("A nice description for this table")
    .label("owner", "me")
    .label("env", "prod")
    .expiration_time(SystemTime::now() + Duration::from_secs(3600))
    .time_partitioning(
        TimePartitioning::per_day()
            .expiration_ms(Duration::from_secs(3600 * 24 * 7))
            .field("ts"),
    ),
).await?;

// 5. Insert streaming data
let mut insert_request = TableDataInsertAllRequest::new();
insert_request.add_row(None, MyRow { /* fill fields */ })?;
client.tabledata()
    .insert_all(project_id, dataset_id, table_id, insert_request)
    .await?;

// 6. Query table
let mut response = client.job().query(
    project_id,
    QueryRequest::new(format!(
        "SELECT COUNT(*) AS c FROM `{}.{}.{}`",
        project_id, dataset_id, table_id
    )),
).await?;

let mut rs = ResultSet::new_from_query_response(response);
while rs.next_row() {
    println!("Number of rows inserted: {}", rs.get_i64_by_name("c")?.unwrap());
}

// 7. Drop table and dataset
client.table().delete(project_id, dataset_id, table_id).await?;
client.dataset().delete(project_id, dataset_id, true).await?;
```

---

## 🧰 Examples

- Full end-to-end usage in `examples/load.rs`
- Showcases:
  - Streaming insert
  - Nested records
  - Serde struct mapping
  - Custom schema definitions

---

## 📜 License

Dual licensed under:

- MIT License  
- Apache 2.0 License

---

## 🙋‍♀️ Contributing

- Feature requests, issues, and PRs welcome
- Use the [GitHub discussion section](https://github.com/lquerel/gcp-bigquery-client/discussions) for suggestions

---
```
````
