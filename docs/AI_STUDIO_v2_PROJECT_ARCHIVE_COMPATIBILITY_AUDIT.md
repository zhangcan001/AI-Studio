# DEV-129-A — AI Studio v2 Project Archive Compatibility Audit

## Audit status

| Item | Result |
| --- | --- |
| Audit baseline | `75f357fef2a6154d73f61ec4642e58208a19eaf3` (`master`, v1.3.1 stable baseline) |
| Scope | Project Archive compatibility and local long-term preservation planning |
| Code change | No |
| Database or migration change | No |
| UI change | No |
| Current backup authority | Existing `ProjectBackupService` and `ProjectBackupRepository` |
| DEV-129-A status | Complete after this document is committed and pushed |

This is a documentation-only audit. The repository and current implementation are the source of truth; the earlier v2 planning documents are used as product context only.

## 1. Repository and existing archive boundaries

The archive capability is already present and should be extended rather than replaced:

| Area | Current authority | Finding |
| --- | --- | --- |
| Backup orchestration | `src-tauri/src/application/project_backup_service.rs` | Owns export, ZIP construction, inspection, validation, safe publication, media copying, ID remapping, and application error mapping. |
| Backup persistence boundary | `src-tauri/src/application/ports/project_backup_repository.rs` and `src-tauri/src/infrastructure/database/repositories/project_backup.rs` | Owns the transaction-backed logical project snapshot and atomic restore rows. |
| Project manifest | `src-tauri/src/application/project_manifest_service.rs` | Exports a v2 structure/reference manifest; it is not a full media and history archive. |
| Project commands | `src-tauri/src/commands/project.rs` | Exposes typed export, inspect, restore, and manifest-export commands. |
| Frontend transport | `src/services/tauriClient.ts` and `src/types/project.ts` | Already exposes the project backup and manifest operations; components do not own persistence. |
| Database | `src-tauri/migrations/001_initial.sql` through `036_provenance_lineage.sql` | SQLite schema is owned by Rust/SQLx migrations. |
| Media storage | Project-root filesystem paths recorded by `assets.storage_path` and `thumbnail_path` | Media is outside SQLite; the database stores metadata, checksums, and links. |

The current full backup format is `ai-studio-project-backup`, version `18`. It is a ZIP package containing a transaction-consistent logical JSON snapshot plus copied media. It is **not** a raw copy of the live SQLite database file.

## 2. Project authority audit

### 2.1 Authority matrix

| Domain | Canonical authority | Project relationship | Archive implication |
| --- | --- | --- | --- |
| Project | `projects` | Root isolation boundary; owns name, description, and local `root_path`. | Restore must create a new project/root by default and must not overwrite the source project. |
| Shot | `shots` plus production structure tables from migrations `021` and later | `shots.project_id` scopes the shot; series/episode/scene structure is project-owned. | Preserve shot IDs through an explicit remap and preserve prompt/reference bindings. |
| Task | `tasks` | `tasks.project_id` is the execution-history scope. Queue batches/items reference tasks and remain under the project. | Preserve terminal history and explicitly report any excluded active/incomplete work; never restore a running task as running. |
| Queue | `production_batches`, `production_batch_items`, and queue operation history | Project-scoped through the batch and its items. | Archive history and preparation snapshots; do not create a second queue or resume execution during restore. |
| Review | `production_item_reviews` and its batch-item/task/result references | The review row carries `project_id` and is linked to project-owned production items. | Restore review history after batch/item/task/asset ID remapping; preserve review status and lineage. |
| Asset | `assets`, with `task_output_assets` and existing reference/tag/favorite links | `assets.project_id` is the asset ownership boundary; files live under project storage. | Preserve metadata, output mappings, tags, references, checksums, and copied media. |
| Asset version | `asset_versions` from migration `033` | Has explicit `project_id` and `asset_id`; version history is append-only. | **Current gap:** versions and their relations are not present in `BackupDocument`; future archive format must include them. |
| Asset relation | `asset_relations` from migration `033` | Has project scope and typed source/target asset links. | **Current gap:** relations are not exported; validate that both endpoints remain in the same restored project. |
| Prompt | `prompt_entries` and `prompt_versions` from migration `009` | Prompt entries are project-owned; versions are append-only under the entry. | Current backup includes prompt entries and versions, but v2 model references must be included as well. |
| Generation | Existing `tasks` + one-to-one `generation_snapshots`; output/result identity is represented by `task_output_assets` and `assets` | The task is the generation execution authority; there is no second `generations` table. | Preserve task snapshot and output mappings; do not invent a new Generation entity in the archive. |
| Model | `models` and `model_versions` from migration `034` | Registry metadata is currently global, while prompt/generation rows reference model versions. | Capture referenced immutable model metadata without overwriting or duplicating the canonical registry. |
| Tool | `tools`, `tool_versions`, `tool_capabilities`, and `tool_instances` from migration `035` | Registry/instance metadata is local-machine oriented and not project-owned. | Capture observed context as provenance; do not restore executable state, start processes, or treat a restored instance as healthy automatically. |
| Provenance | `generation_tool_usages` and `generation_asset_versions` from migration `036` | Scope is derived through the task/generation and the referenced project asset version. | **Current gap:** explicit v2 lineage is not in the existing backup document and must be added without creating a second authority. |

### 2.2 Relationship conclusions

The existing model already represents the production chain without a parallel domain:

```text
Project
  ├─ Shot ── PromptEntry/PromptVersion ── ModelVersion
  ├─ QueueBatch/QueueItem ── Task ── GenerationSnapshot
  ├─ Task ── TaskOutputAsset ── Asset ── AssetVersion
  └─ Review ── QueueItem / Task / Result Asset

Task / Generation
  ├─ GenerationToolUsage ── ToolInstance / ToolVersion ── Tool
  └─ GenerationAssetVersion ── AssetVersion
```

The archive must serialize and remap these existing relationships. It must not introduce another Project, Asset, Prompt, Generation, Result, Queue, or Task authority.

One compatibility detail is important: some newer relation tables carry a project column while their foreign keys point to entities that carry project identity separately. The archive validator and repository service must enforce the cross-table project invariant explicitly; SQLite foreign keys alone are not sufficient for every project-isolation check.

## 3. Storage audit

### 3.1 SQLite and metadata

- SQLite persistence is owned by the Rust application layer through SQLx migrations.
- The current backup repository opens a short transaction for metadata reads, loads the project snapshot, and commits before streaming potentially large files.
- Current metadata includes Project, Task, TaskEvent, GenerationSnapshot, Asset, output mappings, prompt entries/versions, queue/preparation history, production structure, reviews, reference data, workflow references, and other historical project records.
- The current mechanism is a **logical SQLite snapshot**: repository-selected rows serialized into the backup document. It is not a live-file copy of `ai-studio.sqlite` and does not depend on copying SQLite journal/WAL files.
- For v2, the logical snapshot should remain the default. A raw database file should not be added merely to satisfy the word “snapshot”; it would be less portable, could capture machine-local state, and would bypass the existing ID-remap and validation boundary.

### 3.2 Media and path handling

- Images, videos, audio, and other asset content remain in the filesystem. The `assets` table stores `storage_path`, optional `thumbnail_path`, file size, MIME information, dimensions/duration, and SHA-256 metadata.
- During export, the service converts local paths into safe package-relative entries such as `assets/<asset-id>/content.<ext>` and optional thumbnails. Absolute host paths are not the archive's portable content identity.
- Required asset content is checked for existence, regular-file status, expected size, and SHA-256 while streaming into the ZIP. A missing or changed required file blocks export rather than creating a silently incomplete archive.
- Thumbnails are optional in the current implementation; an unavailable optional thumbnail is omitted and represented by the absence of a thumbnail entry.
- If a file was moved outside the application's knowledge, the stored path becomes stale. The current boundary does not guess a replacement by filename or search the machine automatically; export reports the content/checksum failure.
- Restore stages files under a temporary project root, verifies their size and checksum, and publishes them into a new project root. The restored database points to the new local paths.
- Tool `path` and `endpoint` values are machine-local observations, not portable media paths. They must be preserved as descriptive metadata only and must never trigger process start, probing, installation, or automatic repair.

### 3.3 Storage compatibility conclusion

The filesystem-plus-SQLite-metadata split is suitable for a personal archive. The missing requirement is not a new storage engine; it is a versioned package contract that includes all v2 logical rows and makes media/checksum behavior explicit.

## 4. Export capability audit

### 4.1 Existing full backup

The current typed export flow is:

```text
project_backup_export
  -> transaction-backed ProjectBackupRepository snapshot
  -> required media checksum/size validation
  -> temporary ZIP
  -> atomic publish
```

The current v18 package writes:

```text
manifest.json
project.json
history/task_snapshots.json
presets.json
production_queue.json
production_preparation_snapshots.json
assets/<asset-id>/content.<ext>
assets/<asset-id>/thumbnail.<ext>  (when available)
```

The `project.json` document currently contains the Project, task and event history, assets, output mappings, generation snapshots, prompts, queue/preparation records, workflow references/registry, tags/favorites, references, production structure, script/consistency data, reviews, benchmarks, production runs, shots, and handoff records covered by the current backup implementation.

The service also provides:

- inspect-before-restore with ZIP safety limits, manifest/document consistency checks, and missing-workflow reporting;
- restore into a newly named project/root with explicit ID remapping;
- atomic database restoration after media staging;
- a separate `project_manifest_export` command for structure and reference data.

### 4.2 Current export gaps for v2

| Capability | Current status | Compatibility finding |
| --- | --- | --- |
| Project metadata | Covered | `manifest.json` and `project.json` agree on project identity. |
| Asset metadata and media inventory | Covered for `assets` | Media is copied and verified; asset metadata is in the logical document. |
| Asset version history | Missing | `asset_versions` is not in `BackupDocument`. |
| Asset relations | Missing | `asset_relations` is not in `BackupDocument`. |
| Prompt entries and basic versions | Covered | Existing `prompt_entries` and `prompt_versions` are serialized. |
| Prompt-to-model-version data | Partial/missing in archive | Migration `034` added `prompt_versions.model_version_id`, but the current backup structs/queries do not yet carry the v2 model registry/reference set. |
| Generation snapshots and output mappings | Covered for existing authority | Task snapshots and `task_output_assets` mappings are included. |
| Model and model-version registry | Missing | `models` and `model_versions` are not included in the current backup document. |
| Tool registry/instances/capabilities/versions | Missing | The Local Tool Hub tables are not included. |
| Explicit provenance lineage | Missing | `generation_tool_usages` and `generation_asset_versions` are not included. |
| Structure/reference manifest | Covered separately | `ProjectManifest` v2 is useful for structure validation but is not a complete archive package. |

Therefore the current backup is a strong v1/older-v2 archive boundary, but it is not yet a complete v2 Personal Edition archive. The gap is additive and identifiable; no replacement system is required.

## 5. Backup strategy plan

### 5.1 Recommended direction

Extend `ProjectBackupService`, `ProjectBackupRepository`, and the existing ZIP format. Do not create a parallel `ProjectArchiveService` with a second serialization and restore path.

The next package format should use a new explicit backup format version (for example, the next version after v18; the exact number belongs to implementation planning). Existing readers must continue to accept supported historical versions, and the new reader must distinguish absent optional v2 collections from malformed required data.

### 5.2 Project Archive Package

The planned local package should contain:

```text
manifest.json
snapshot/project.json                 # logical project/database snapshot
snapshot/history/task_snapshots.json
snapshot/presets.json
snapshot/production_queue.json
snapshot/production_preparation_snapshots.json
snapshot/v2/asset_versions.json
snapshot/v2/asset_relations.json
snapshot/v2/models.json
snapshot/v2/model_versions.json
snapshot/v2/tools.json
snapshot/v2/tool_versions.json
snapshot/v2/tool_capabilities.json
snapshot/v2/tool_instances.json
snapshot/v2/generation_tool_usages.json
snapshot/v2/generation_asset_versions.json
assets/<stable-archive-relative-id>/content.<ext>
assets/<stable-archive-relative-id>/thumbnail.<ext>
```

The exact split between `project.json` and separate deterministic JSON files is an implementation detail. The important invariants are:

1. the logical snapshot is captured consistently through the repository boundary;
2. every included row has an explicit schema/package version;
3. every media entry has a safe relative path, size, and checksum in the manifest or asset record;
4. references are explicit IDs, never filename/name guesses;
5. absent optional v2 data is distinguishable from corrupt data;
6. package ordering and JSON serialization are deterministic enough for repeatable validation.

### 5.3 Manifest responsibilities

The archive manifest should be a small, stable envelope containing:

- archive format and package version;
- generator/application version and source migration/schema version;
- source project identity and export time;
- included domain/feature flags;
- counts for assets, versions, relations, prompts, generations, models, tools, and lineage rows;
- archive-relative media paths, sizes, and checksums;
- task exclusion counts and warnings such as missing workflows;
- logical snapshot checksum and package inventory checksum.

The existing `ProjectBackupManifest` is currently minimal, while `ProjectManifest` v2 is structure-oriented. The implementation should evolve these existing concepts or compose them into the archive envelope; it should not create two competing manifest meanings.

### 5.4 Snapshot semantics

For this MVP, “SQLite snapshot” means a transaction-consistent logical snapshot of the project rows. The archive should record the source migration version and snapshot checksum. A raw SQLite file copy may be evaluated separately, but it is not required and should not become the primary portable format without a clear WAL, locking, path, and migration policy.

Backup remains explicit and user initiated. No automatic backup, background watcher, or cloud upload is part of this plan.

## 6. Restore strategy plan

### 6.1 Inspect before restore

The restore flow should remain inspect-first:

1. Open the archive and enforce entry count, compressed/uncompressed size, safe-path, and symlink limits.
2. Validate manifest format/version, source migration support, inventory counts, and document checksums.
3. Validate every referenced Project, Shot, Task, Prompt, ModelVersion, ToolVersion, Asset, AssetVersion, output mapping, relation, and lineage endpoint.
4. Validate project ownership across relation rows, especially where a relation table's foreign keys do not encode a composite project constraint.
5. Validate required media entries and checksums; report missing workflows and non-portable tool instances as warnings or explicit blockers according to the package contract.
6. Show a preview and require the user to choose restore; inspection alone must not mutate the database or start a process.

### 6.2 Restore into a new project

The current safe default should be retained:

- create a new Project ID and new project root;
- preserve the source project and its files;
- build one complete old-ID to new-ID remap for every project-owned entity;
- stage and checksum-verify media before publishing the new root;
- restore metadata in dependency order through one repository transaction;
- commit only after all rows satisfy referential and project-isolation checks;
- remove staged/final output on failure and never leave a partially restored project advertised as complete.

### 6.3 v2 and global-registry handling

Asset versions, relations, prompts, tasks, snapshots, output mappings, reviews, and lineage rows are restored with the new project IDs. Model/tool registry rows require a different policy because `models` and `tools` are global/local-machine metadata rather than project-owned rows:

- include the immutable model/model-version and observed tool/tool-version/capability descriptors referenced by the archive;
- reuse an already matching canonical registry record when the implementation can prove identity;
- do not overwrite an existing registry record or create a hidden duplicate authority;
- if a referenced model/tool cannot be resolved safely, restore the historical reference as unresolved and report it rather than guessing by name;
- restore ToolInstance path/endpoint as observed metadata only, with a non-healthy state until explicitly verified by a future user action; no process start, installation, probing, or auto-linking is allowed;
- preserve provenance even when the referenced local tool is no longer available.

Existing historical links must remain historical. There is no automatic backfill based on a path, asset name, or prompt text, and no active task is resumed by restore.

### 6.4 Post-restore validation

After the transaction commits, the service should validate:

- project-owned rows all point to the new Project ID;
- every included output, AssetVersion, AssetRelation, PromptVersion, GenerationToolUsage, and GenerationAssetVersion points to an included/remapped endpoint;
- media paths exist under the new project root and match recorded checksums;
- archive inventory counts match the restored rows;
- unresolved workflows, models, tools, and missing optional files are visible in the restore result;
- Task/Generation history remains historical and no queue execution was triggered.

## 7. Archive entity design

### 7.1 Reuse existing archive authority

The first implementation should treat the archive package as the durable entity and the existing `ProjectBackupService` as the application authority. Adding four new database tables immediately would create a catalog to maintain before there is a user need for catalog search.

### 7.2 Planned logical entities

If a later archive catalog is needed, the following meanings are recommended:

| Entity | Meaning | MVP persistence recommendation |
| --- | --- | --- |
| `Archive` | One local archive package associated with a source project, including path, format, size, checksum, and status. | Do not add a table yet; the package and its manifest are authoritative. |
| `ArchiveVersion` | Format/schema revision of an archive package, including compatibility and source migration version. | Represent in `manifest.json`; do not model as a second backup-history table yet. |
| `Snapshot` | One transaction-consistent logical capture of project rows, with capture time, included domains, exclusions, and checksum. | Represent in the package snapshot and manifest. |
| `Manifest` | Stable package envelope containing identity, inventory, entry paths, checksums, capabilities, and warnings. | Extend/compose the existing backup manifest; do not duplicate `ProjectManifest` semantics. |

If persistent records are introduced later, they must be a local archive catalog only. They must not replace `projects`, `assets`, `prompt_entries`, `tasks`, `generation_snapshots`, `models`, or `tools` as domain authorities.

## 8. MVP boundary

### Included

- Local, user-initiated project snapshot.
- Export to a versioned archive package.
- Manifest and asset inventory.
- Metadata, media, checksums, and explicit v2 relations.
- Import inspection and validation before mutation.
- Restore into a new project with ID remapping and atomic database writes.
- Preservation of Project, Shot, Task, Review, Asset, AssetVersion, Prompt, Generation, Model/Tool provenance, and historical warnings where safely supported.
- Clear reporting of excluded active/incomplete work, missing workflows, missing media, and unresolved machine-local tools.

### Explicitly excluded

- Cloud synchronization.
- Multi-user collaboration or permissions.
- Automatic/background backup.
- Live incremental synchronization or deduplicated remote storage.
- Filename/name-based relation inference.
- Automatic model/tool installation, process execution, probing, or health repair.
- A second Project/Asset/Prompt/Generation/Result/Queue/Task authority.
- A raw SQLite file copy as the default portable format.

## 9. Risk and readiness assessment

### Technical debt / compatibility risks

| Priority | Item | Impact | Recommended handling |
| --- | --- | --- | --- |
| P1 | Current v18 backup omits `asset_versions` and `asset_relations`. | Asset Library history and typed relations would be lost on restore. | Add versioned collections and ID remapping before declaring the v2 archive complete. |
| P1 | Current v18 backup omits model/tool registries and both provenance-lineage tables. | Prompt/model/tool history cannot be made portable or fully explainable. | Include referenced immutable records and explicit lineage edges; never infer missing links. |
| P1 | Model/tool rows are global or machine-local while the archive is project-scoped. | Naive restore can overwrite, duplicate, or falsely mark a local tool as available. | Define identity/conflict/unresolved-reference policy during DEV-129-B. |
| P2 | Full backup and structure manifest have different scopes and formats. | Users may mistake a structure manifest for a restorable archive. | Keep names and manifest capabilities explicit; document which artifact is complete. |
| P2 | Large media dominates package size and restore time. | Long exports and partial filesystem failure are possible. | Continue streaming, size caps, checksums, staging, and atomic publish; defer incremental storage. |
| P2 | Some cross-project invariants are service-level rather than composite foreign keys. | Invalid relation rows could pass basic SQLite FK checks. | Add archive validation for project ownership and endpoint closure. |
| P3 | No persistent archive catalog exists. | Finding many local packages is manual. | Defer `Archive` catalog tables until package export/import is proven useful. |

### Final assessment

```text
IMPLEMENTATION_READY=YES
IMPLEMENTATION_SCOPE=DEV-129-B archive data model and compatibility design, followed by a separately approved implementation
IMPLEMENTATION_RISK=MEDIUM-HIGH
```

The existing backup boundary is mature enough to extend. The risk is medium-high because adding v2 rows requires coordinated package versioning, validation, global-registry conflict handling, media integrity, and restore ID remapping. It is not a reason to replace the current architecture.

## 10. Audit conclusion

AI Studio v2 can support a durable local Project Archive without changing the Production Core or introducing a new execution system. The correct next step is an additive archive data-model/compatibility design that extends the existing v18 backup authority to include AssetVersion, AssetRelation, Model/ModelVersion, Tool metadata, and explicit provenance lineage.

No implementation work is started by this audit. DEV-129-B remains the next planned phase and is not started here.

## 11. Verification record

The audit was limited to repository/source inspection and this document. No Rust, React, migration, configuration, queue, task, review, or production-flow files were changed.

```text
CODE_CHANGED=NO
DATABASE_CHANGED=NO
MIGRATION_CHANGED=NO
UI_CHANGED=NO
```
