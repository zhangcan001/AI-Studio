# DEV-129-C Archive Export / Import MVP

```text
TASK=DEV-129-C
VERSION=1.3.1_STABLE
SCOPE=AIARCHIVE_PACKAGE_INSPECT_FIRST_RESTORE
EXTEND_PROJECT_BACKUP_SERVICE=YES
NEW_ARCHIVE_TABLES=NO
PARALLEL_ARCHIVE_SERVICE=NO
RAW_SQLITE_COPY=NO
```

## 1. Implementation summary

DEV-129-C packages the existing v19 logical backup as a portable `.aiarchive`
file (ZIP with a stable extension). It **extends** `ProjectBackupService` /
`ProjectBackupRepository` only. It does **not** add `ProjectArchiveService`,
Archive catalog tables, a second queue/task/generation authority, automatic
backup, cloud sync, agents, or a raw SQLite copy.

The roadmap sketch that mentioned `metadata.sqlite` is **not** the portable
format. `project.json` remains the authoritative logical snapshot. Packages
never include a SQLite database file.

## 2. Package layout (v19 write)

Default save name: `AI-Studio-Project.aiarchive`.

Dialog filters (export + inspect): `aiarchive` and `zip`, label `归档 / 备份`.

```text
manifest.json
project.json
provenance.json
history/task_snapshots.json
presets.json
production_queue.json
production_preparation_snapshots.json
assets/<id>/content.<ext>
assets/<id>/thumbnail.<ext>   # optional; stays under assets/, not duplicated
```

Notes:

- Root `manifest.json` + `project.json` stay at the ZIP root so current readers
  continue to work. There is no `snapshot/` relocation in this MVP.
- `provenance.json` is a deterministic sidecar containing
  `generation_tool_usages` + `generation_asset_versions` (sorted by id). The
  same rows remain inside `project.json`; the sidecar is for inspectability and
  package clarity, not a second authority.
- Thumbnails remain under `assets/<id>/thumbnail.*`. A separate `previews/`
  tree is intentionally omitted to avoid duplicating large bytes.
- Historical **v18** `.zip` packages without `provenance.json` or the new
  manifest fields still inspect and restore.

## 3. Manifest extensions (additive)

`ProjectBackupManifest` gains optional fields with `#[serde(default)]`:

| Field | Meaning |
| --- | --- |
| `inventory` | Counts: assets, assetVersions, relations, prompts, models, tools, lineage, tasks |
| `logicalSnapshotChecksum` | SHA-256 of the exact `project.json` bytes in the package |
| `mediaInventory` | Required content files: relative path + size + sha256 |
| `createdBy` | Source app version (unchanged) |

Absent optional fields deserialize as empty/`None`, so v18 (and early v19
without these fields) still parse. When present, inspect validates checksums
and inventory consistency.

## 4. Export flow

```text
repository snapshot
  -> required media existence / size / sha256
  -> temp ZIP (manifest + project.json + provenance + media)
  -> atomic publish to .aiarchive (or .zip if the user chooses)
```

Missing or mismatched required media blocks export. Optional thumbnails are
omitted when unavailable.

## 5. Inspect-first import

1. Open archive (`.aiarchive` or `.zip`) with ZIP safety limits.
2. Validate format/version; validate checksums / inventory when present (v19).
3. Show preview (including v2 counts already on `ProjectBackupPreviewView`).
4. Inspection copies the package into a short-lived inspection cache only.
   It does **not** mutate the database or start processes.
5. Restore only after inspect: new Project ID + new root, explicit ID remap
   from DEV-129-B, never overwrite the source project, never auto-guess
   relations, never resume running tasks.

## 6. Compatibility

| Package | Inspect | Restore |
| --- | --- | --- |
| Historical v18 `.zip` (no provenance / inventory) | Yes | Yes (new project) |
| DEV-129-B v19 without new manifest fields | Yes | Yes |
| DEV-129-C v19 `.aiarchive` | Yes + checksum validation | Yes (new project) |

## 7. Explicitly deferred (DEV-129-D+)

- Persistent Archive / ArchiveVersion catalog tables and local archive browser
- Snapshot path layout migration (`snapshot/…` relocation) if product wants it
- Background / automatic backup
- Cloud sync, multi-user, incremental/deduplicated storage
- Raw SQLite file copy as an alternate portable format
- Production flow, Task state machine, or Generation execution changes
- Auto-install / probe / heal restored tool instances

## 8. Boundary status

```text
BACKUP_FORMAT=ai-studio-project-backup
BACKUP_VERSION=19
PACKAGE_EXTENSION=.aiarchive (+ legacy .zip)
NEW_ARCHIVE_TABLES=NO
PARALLEL_ARCHIVE_SERVICE=NO
RAW_SQLITE_COPY=NO
AUTO_BACKUP=NO
CLOUD_SYNC=NO
DEV_129_C=COMPLETE_AFTER_COMMIT
DEV_129_D_STARTED=NO
```

## 9. Verification

```text
cargo check
cargo test --lib -- v19_write_includes_provenance inspect_accepts_historical_v18 inspect_does_not_write_db restore_from_aiarchive media_checksum_mismatch export_blocks_required_media
cargo test --lib -- project_backup
pnpm test -- ProjectWorkspace
```
