# DEV-129-D Archive Hardening

```text
TASK=DEV-129-D
VERSION=1.3.1_STABLE
SCOPE=ARCHIVE_INTEGRITY_CORRUPTION_RECOVERY_PATH_CHANGE
EXTEND_PROJECT_BACKUP_SERVICE=YES
NEW_ARCHIVE_TABLES=NO
PARALLEL_ARCHIVE_SERVICE=NO
RAW_SQLITE_COPY=NO
AUTO_BACKUP=NO
CLOUD_SYNC=NO
PUSH=NO
```

## 1. Implementation summary

DEV-129-D hardens the v19 / `.aiarchive` path delivered by DEV-129-B/C. It
extends focused Rust coverage around export/import integrity, corruption
recovery, and restore path remapping. It does **not** add Archive catalog
tables, automatic backup, cloud sync, raw SQLite packaging, a second
queue/task/generation authority, or production/task/generation behavior
changes.

## 2. What was proven in this environment

### Integrity / corruption (inspect fails, no DB mutation)

| Behavior | Proof |
| --- | --- |
| Truncated/corrupt ZIP | `truncated_corrupt_zip_fails_inspect_without_db_mutation` — inspect returns `BACKUP_INVALID`; project/task counts unchanged |
| `logicalSnapshotChecksum` mismatch | `logical_snapshot_checksum_mismatch_fails_inspect` |
| Inventory counts vs package rows | `inventory_count_mismatch_fails_inspect` when `inventory` is present |
| Required media sha256 mismatch | `media_checksum_mismatch_fails_inspect` (+ export block) |
| Required media size mismatch | `media_size_mismatch_fails_inspect` + `export_blocks_required_media_size_mismatch` |
| ZIP path traversal | Existing `zip_slip_archive_is_rejected_before_restore` kept; `zip_path_traversal_regression_still_rejected` added |
| v18 compat | Existing `inspect_accepts_historical_v18_zip_without_provenance` kept; `v18_zip_compat_regression_still_inspects` added |

### Path change / ID remap (restore)

| Behavior | Proof |
| --- | --- |
| Media lands under the **new** project root | `restore_writes_media_under_new_project_root_not_archived_host_paths` |
| Archived absolute host paths are not reused on `assets.storage_path` / `asset_versions.location` | same test + scale old-path leak assertion |
| `asset_versions` / `asset_relations` / lineage endpoints remap into the new project; no leftover old `project_id` | `restore_id_remap_keeps_versions_relations_lineage_in_new_project` |

### Small targeted correctness fix

`restore_asset_versions_and_relations` previously fell back to the archived
`version.location` when a remapped media path was missing. That could reuse an
absolute host path from the package. Restore now **requires** the remapped
storage path under the new project root and fails closed otherwise
(`src-tauri/src/infrastructure/database/repositories/project_backup.rs`).

## 3. Bounded synthetic scale (actual N)

Product planning language referenced roughly:

```text
Projects: 100
Assets: 10_000
Versions: 50_000
Relations: 100_000
```

This task does **not** create 100 live projects or full product-scale fixtures.
The in-process probe used:

```text
N_ASSETS     = 120
N_VERSIONS   = 120
N_RELATIONS  = 200
Projects     = 1 (single archive round-trip)
```

Why this N:

- Exercises inventory checksums, media inventory, restore remapping, and
  SQLite inserts for versions/relations without multi-minute fixtures.
- Keeps unique `(project_id, source, target, relation_type)` pairs within the
  schema UNIQUE constraint.
- Tiny synthetic media bytes avoid I/O dominating the run.
- Full product-scale (10k/50k/100k) and multi-project stress remain a
  **live / product-owner gate** (see §5), not a CI unit-test obligation.

Test: `bounded_synthetic_scale_archive_integrity_round_trip`.

## 4. Explicitly out of scope (unchanged)

- Persistent Archive / ArchiveVersion catalog tables and local archive browser
- Snapshot path layout migration (`snapshot/…`) if product wants it
- Background / automatic backup
- Cloud sync, multi-user, incremental/deduplicated storage
- Raw SQLite file copy as an alternate portable format
- Production flow, Task state machine, or Generation execution changes
- Auto-install / probe / heal restored tool instances

## 5. Remaining live / product-owner gates (DEV-130+)

Treat these like prior GPU live validation: code/tests here are not a substitute
for an owner-controlled live pass.

| Gate | Why it remains live |
| --- | --- |
| Full product-scale archive stress (≈100 projects / 10k assets / 50k versions / 100k relations, or measured personal workload) | Too large / slow for unit fixtures; needs disk, time, and representative media |
| Desktop inspect → restore UX on real `.aiarchive` chosen by the user | Dialog filters, inspection TTL, and human preview copy |
| Restored tool instances remain `UNKNOWN` until owner probes on a real machine | Path/endpoint are metadata only; no auto heal |
| GPU / ComfyUI generation after restore (if DEV-130 covers runtime continuity) | Requires local ComfyUI/GPU; not represented as a code-gate PASS |
| Missing/offline media relinking on moved drives | Path-stability product behavior beyond package remap |
| Archive catalog / browser / auto-backup / cloud | Explicitly deferred past 129-D |

## 6. Boundary status

```text
BACKUP_FORMAT=ai-studio-project-backup
BACKUP_VERSION=19
PACKAGE_EXTENSION=.aiarchive (+ legacy .zip)
NEW_ARCHIVE_TABLES=NO
PARALLEL_ARCHIVE_SERVICE=NO
RAW_SQLITE_COPY=NO
AUTO_BACKUP=NO
CLOUD_SYNC=NO
DEV_129_D=COMPLETE_AFTER_COMMIT
DEV_130_STARTED=NO
PUSHED=NO
```

## 7. Verification

```text
cargo check --lib
cargo test --lib -- project_backup_service::tests::
  => 50 passed (includes DEV-129-D hardening + bounded scale)
```

Focused names added/extended under
`src-tauri/src/application/project_backup_service.rs` tests:

- `truncated_corrupt_zip_fails_inspect_without_db_mutation`
- `logical_snapshot_checksum_mismatch_fails_inspect`
- `inventory_count_mismatch_fails_inspect`
- `media_size_mismatch_fails_inspect`
- `export_blocks_required_media_size_mismatch`
- `zip_path_traversal_regression_still_rejected`
- `restore_writes_media_under_new_project_root_not_archived_host_paths`
- `restore_id_remap_keeps_versions_relations_lineage_in_new_project`
- `bounded_synthetic_scale_archive_integrity_round_trip` (N=120/120/200)
- `v18_zip_compat_regression_still_inspects`
