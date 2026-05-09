# Shadow File Inventory

This inventory records the legacy and backup files that were classified during the refactor cleanup.

## Classified Files

| Path | Classification | Action | Rationale |
| --- | --- | --- | --- |
| `src/api/handlers.rs.backup` | backup snapshot | delete | Active handler surface is `src/api/handlers.rs` plus dedicated modules under `src/api/`. |
| `src/core/database.rs.backup` | backup snapshot | delete | Canonical runtime database abstraction is `src/core/database.rs`. |
| `src/core/database_old.rs` | legacy shadow implementation | delete | Not referenced by the build; replaced by `src/core/database.rs`. |
| `src/core/database_complete.rs` | empty/inactive shadow file | delete | Not referenced by the build and carries no active implementation. |

## Primary Abstraction Decision

- Canonical runtime database abstraction: [src/core/database.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/core/database.rs:1)
- Legacy compatibility wrapper retained only for `MainScraper`: [src/core/enhanced_database.rs](/home/bitloop/Documents/GITHUB/GitArchiver/rust_github_archiver/src/core/enhanced_database.rs:1)

## Outcome

The classified shadow files were removed after this inventory was written so contributors only see the active runtime path in the source tree.
