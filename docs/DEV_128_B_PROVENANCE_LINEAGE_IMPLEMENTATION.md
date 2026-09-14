# DEV-128-B Provenance Lineage Data Layer Implementation

```text
TASK=DEV-128-B
VERSION=1.3.1_STABLE
SCOPE=PROVENANCE_LINEAGE_DATA_LAYER
ADDITIVE_MIGRATION_ONLY=YES
FRONTEND_CHANGED=NO
```

## 1. Implementation summary

The provenance gaps identified by DEV-128-A are now represented by two
explicit, append-oriented relations. They extend the existing authorities and
do not introduce a `Generation`, `Result`, or alternate `Asset` domain:

```text
existing Task + GenerationSnapshot
        + generation_tool_usages
        + generation_asset_versions
```

`generation_id` is the public relation field name required by this phase, but
its value is an existing `tasks.id` (`TaskId`). No new generation identity was
created.

## 2. Migration

Migration `036_provenance_lineage.sql` creates:

### `generation_tool_usages`

| Column | Purpose |
| --- | --- |
| `id` | Stable relation identity (`gtu_…`) |
| `generation_id` | Existing `tasks.id` foreign key |
| `tool_instance_id` | Canonical local `tool_instances.id` |
| `tool_version_id` | Optional canonical observed `tool_versions.id` |
| `metadata_json` | Explicit caller-supplied JSON context |
| `created_at` | Capture time |

The write boundary checks that the Task and ToolInstance exist and, when a
ToolVersion is supplied, that it belongs to the ToolInstance's Tool. Queries
derive project scope through the referenced Task rather than duplicating a
project column.

### `generation_asset_versions`

| Column | Purpose |
| --- | --- |
| `id` | Stable relation identity (`gav_…`) |
| `generation_id` | Existing `tasks.id` foreign key |
| `output_id` | Existing output identity |
| `ordinal` | Existing output ordering key |
| `asset_version_id` | Canonical `asset_versions.id` |
| `relation_type` | Typed `OUTPUT` or `DERIVED` relationship |
| `created_at` | Capture time |

The composite foreign key `(generation_id, output_id, ordinal)` must already
exist in `task_output_assets`. A unique constraint permits one canonical
AssetVersion link per existing output key. The repository also checks that the
AssetVersion belongs to the output Asset and the Task project before writing.

All parent references use `ON DELETE RESTRICT` so provenance cannot disappear
as an incidental cascade. Existing migrations and tables are unchanged, and
there is no historical backfill.

## 3. Rust implementation

### Domain

`src-tauri/src/domain/provenance_lineage.rs` adds:

- `GenerationToolUsage` and `GenerationToolUsageId`;
- `GenerationAssetVersion` and `GenerationAssetVersionId`;
- `GenerationAssetVersionRelationType`;
- validation for non-null metadata, non-empty output identity, and typed
  relation values.

The domain stores existing `TaskId`, `ToolInstanceId`, `ToolVersionId`, and
`AssetVersionId` values. It does not define a `Generation` struct.

### Repository and service

`ProvenanceLineageRepository` and
`SqliteProvenanceLineageRepository` provide explicit create/list operations for
both relations. `ProvenanceLineageService` validates:

- Task existence and project ownership;
- ToolInstance and optional ToolVersion ownership;
- exact output key existence;
- AssetVersion, output Asset, and Task project consistency.

Commands are exposed for future typed-transport consumers only; no frontend
surface or production execution path was changed. Queue Start remains the
single execution gate, and no process is probed, started, stopped, or
automatically associated.

## 4. History protection

The migration only creates empty tables. Existing Tasks, GenerationSnapshots,
output mappings, Assets, AssetVersions, Tools, and Tool history remain
unchanged. A relation is written only when the caller supplies stable IDs and
the repository/service can validate the exact parent records. No relationship
is inferred from a file path, display name, endpoint, or Prompt text.

## 5. Tests

Coverage added or updated includes:

- domain validation and relation type parsing;
- fresh migration table creation and preservation of an existing Task;
- Tool usage create/query round trip;
- AssetVersion lineage create/query through an exact output key;
- rejection of a missing output key;
- project-scoped query isolation;
- service-level Task/tool/output/project validation;
- migration regression expectations updated to migration 036 while preserving
  existing Project, Asset, Shot, Task, and Review rows.

## 6. Verification

```text
cargo fmt --check = PASS
cargo check       = PASS
cargo test        = PASS (774 passed, 1 ignored)
```

The existing compiler warnings are unrelated pre-existing dead-code and test
fixture warnings. No frontend files were changed.

## 7. Boundary status

```text
NEW_GENERATION_SYSTEM=NO
NEW_RESULT_SYSTEM=NO
NEW_ASSET_SYSTEM=NO
PROMPT_EXECUTION_CHANGE=NO
QUEUE_CHANGE=NO
TASK_FLOW_CHANGE=NO
AUTO_BACKFILL=NO
FRONTEND_CHANGED=NO
DEV_128_B=COMPLETE_AFTER_COMMIT
DEV_128_C_STARTED=NO
AUTO_NEXT_TASK=NO
```
