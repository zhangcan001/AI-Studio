# DEV-129-B Archive Data Foundation

```text
TASK=DEV-129-B
VERSION=1.3.1_STABLE
SCOPE=ARCHIVE_DATA_FOUNDATION_BACKUP_V19
ADDITIVE_BACKUP_ONLY=YES
NEW_ARCHIVE_TABLES=NO
PARALLEL_ARCHIVE_SERVICE=NO
FRONTEND_ARCHIVE_UI=NO
```

## 1. Implementation summary

DEV-129-B extends the existing `ProjectBackupService` /
`ProjectBackupRepository` authority so v2 rows can be snapshotted and restored.
It does **not** introduce `Archive` / `ArchiveVersion` / `ArchiveManifest` /
`ArchiveEntry` tables, a second queue/task/generation authority, automatic
backup, cloud sync, agents, or `.aiarchive` packaging (those remain DEV-129-C).

The backup format stays `ai-studio-project-backup`. Package version advances
from **18 → 19**. New collections are additive with `#[serde(default)]` so
historical v18 packages continue to inspect and restore. Absent optional v2
data deserializes to empty vectors; corrupt required historical fields still
fail validation.

## 2. Entities included in BackupDocument v19

| Collection | Source | Scope |
| --- | --- | --- |
| `asset_versions` | `asset_versions` | Project-owned; filtered by `project_id` |
| `asset_relations` | `asset_relations` | Project-owned; both endpoints must stay in-project |
| `models` / `model_versions` | registry | Only rows referenced by this project's prompts/snapshots |
| `tools` / `tool_versions` / `tool_capabilities` / `tool_instances` | Local Tool Hub | Only rows referenced by this project's lineage |
| `generation_tool_usages` / `generation_asset_versions` | provenance | Through included terminal tasks / remapped outputs |

Also carried when present:

- `prompt_versions.model_version_id` (optional)
- `snapshots.model_version_id` (optional; generation snapshot authority remains `tasks` + `generation_snapshots`)

## 3. Identity / conflict policy (P1)

### Models

1. Reuse when the same primary key already exists **and** immutable identity
   fields match (`provider`, `name`, and `type` for models; remapped
   `model_id` + `version` for versions).
2. Else reuse when `UNIQUE(provider, name)` (and version uniqueness under that
   model) proves the same registry record; remap dependent FKs to the existing
   canonical id. Do **not** overwrite the existing row.
3. Else insert the backup row with its original id when absent.
4. If the same primary key exists with conflicting identity fields, leave the
   historical reference **unresolved** (dependent optional FKs become `NULL`),
   report the id, and never guess by display name alone.

### Tools

1. Reuse only when the same primary key exists and immutable identity fields
   match (`name`, `type` for tools; `tool_id` + `version` for versions).
2. There is no safe global UNIQUE name key; name-only matching is forbidden.
3. Else insert when absent; on PK identity conflict, leave unresolved and
   report.
4. Restored `tool_instances` always land as status `UNKNOWN` with
   `last_checked = NULL`. Path/endpoint are preserved as observed metadata
   only. Restore never starts processes, probes, installs, or marks
   `AVAILABLE`.

### Provenance

Lineage rows remap `generation_id` onto restored task ids and
`asset_version_id` onto restored asset-version ids. Tool/model endpoints use
the resolved registry map when available. Provenance is preserved even when a
local tool is missing, as long as observed registry metadata can be inserted
or safely reused; unresolved conflicts are reported rather than invented.

## 4. Restore remapping

Project-owned ids are remapped onto the new project:

- `asset_versions` → new ids, new `project_id`, remapped `asset_id`; `location`
  is rewritten when it equals the restored asset storage path
- `asset_relations` → new ids; both endpoints must resolve inside the restored
  project or restore fails validation
- `generation_tool_usages` / `generation_asset_versions` → new ids; endpoints
  must exist after remap

Inspect/preview surfaces optional counts for the new collections on
`ProjectBackupPreviewView` without a second inspect API.

## 5. Explicitly deferred (DEV-129-C+)

- `.aiarchive` packaging / separate snapshot file layout
- Persistent Archive catalog tables
- Automatic/background backup
- Cloud sync, agents, multi-user
- Raw SQLite file copy
- Archive UI beyond existing backup inspect/restore
- Production flow, Task state machine, or Generation execution changes

## 6. Boundary status

```text
BACKUP_FORMAT=ai-studio-project-backup
BACKUP_VERSION=19
NEW_ARCHIVE_TABLES=NO
PARALLEL_ARCHIVE_SERVICE=NO
PRODUCTION_FLOW_CHANGE=NO
TASK_STATE_MACHINE_CHANGE=NO
GENERATION_EXECUTION_CHANGE=NO
AUTO_BACKUP=NO
CLOUD_SYNC=NO
RAW_SQLITE_COPY=NO
DEV_129_B=COMPLETE_AFTER_COMMIT
DEV_129_C_STARTED=NO
```

## 7. Verification

```text
cargo check       = PASS
cargo test --lib -- backup_v19 v18_backup_without v19_backup_document = PASS (3)
cargo test --lib -- project_backup ... = PASS (34)
```

Identity-policy decisions recorded in §3 were implemented in
`restore_model_registry` / `restore_tool_registry` inside
`project_backup.rs`.
