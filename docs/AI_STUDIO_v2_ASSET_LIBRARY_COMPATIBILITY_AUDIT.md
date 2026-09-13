# AI Studio v2 Asset Library Compatibility Audit

```text
TASK=DEV-122-A
STATUS=COMPATIBILITY_AUDIT_COMPLETE
BASELINE_VERSION=AI_STUDIO_v1.3.1_STABLE
BASELINE_SHA=d9db9eeb2caf9f8a6e9967ae9ae31aefbd406f2e
CODE_CHANGE=NO
DATABASE_CHANGE=NO
SCHEMA_CHANGE=NO
UI_CHANGE=NO
EXISTING_AUTHORITY_AUDIT=COMPLETE
DATABASE_AUDIT=COMPLETE
BACKEND_AUDIT=COMPLETE
FRONTEND_AUDIT=COMPLETE
MIGRATION_PREPARATION=COMPLETE
DEV_122_B_STARTED=NO
```

## 1. Audit purpose and boundary

DEV-122-A audits the v1.3.1 stable repository before Asset Library MVP implementation. The purpose is to identify the existing authorities, data relationships, storage boundaries, and safe extension points so DEV-122-B does not create a duplicate asset, prompt, generation, result, tag, reference, or usage system.

This document is an audit and implementation handoff only. It does not modify Rust, React, SQLite, migrations, commands, routes, stores, or user-facing behavior. DEV-122-B must remain a separate task.

## 2. Baseline verification

The required repository checks were run at the start of the audit:

```text
BRANCH=master
HEAD=d9db9eeb2caf9f8a6e9967ae9ae31aefbd406f2e
REMOTE=origin/master
REMOTE_STATUS=up_to_date
TRACKED_WORKTREE=clean
UNTRACKED_LOCAL_METADATA=.serena/
```

The v1.3.1 release baseline remains documented as:

```text
VERSION=1.3.1
STABLE_SOURCE_SHA=81b204566605c73f840b231b7b981f2c377813cd
NEXT_MAJOR_DIRECTION=AI_STUDIO_v2_PERSONAL_EDITION
```

The current checkout includes the planning commits through DEV-121. `.serena/` is local project metadata and remains intentionally untracked.

## 3. Repository structure audit

| Area | Existing location | Finding |
| --- | --- | --- |
| Rust/Tauri application entry | `src-tauri/src/lib.rs`, `src-tauri/src/app_state.rs` | Application composition, state wiring, command registration, and repository construction already exist. |
| Rust domain | `src-tauri/src/domain/` | Asset, task, generation snapshot, reference, consistency, project, and production domain types already exist. |
| Rust application layer | `src-tauri/src/application/` | Asset query/import/usage/deletion/library services, prompt services, generation services, reference services, and queue services already exist. |
| Rust command layer | `src-tauri/src/commands/` | Asset, prompt-library, generation, reference, production, task, and review commands already exist. |
| SQLite migrations | `src-tauri/migrations/001_initial.sql` through `032_external_production_handoffs.sql` | SQLite schema is migration-managed and currently reaches migration 032. |
| SQLite repositories | `src-tauri/src/infrastructure/database/repositories/` | SQLx repository implementations exist for assets, browsing, usage, deletion, prompts, generations, references, production, tasks, and projects. |
| Filesystem storage | `src-tauri/src/application/ports/asset_store.rs`, `src-tauri/src/infrastructure/filesystem/asset_store.rs` | Asset media and preview files use an `AssetStore` port and a filesystem implementation. |
| React application shell | `src/app/` | Workspace navigation, lazy loading, project context, and asset workspace routing already exist. |
| React asset feature | `src/features/assets/` | Asset Library, cards, grid, preview, usage, tags, favorites, deletion, imports, references, comparisons, and video workflows already exist. |
| React prompt feature | `src/features/prompts/` | Prompt library and prompt-version interactions already exist. |
| Frontend transport | `src/services/tauriClient.ts`, `src/services/ipc.ts` | Typed command transport and centralized IPC error normalization already exist. |
| Frontend types | `src/types/asset.ts`, `src/types/prompt.ts`, `src/types/consistency.ts` | Existing asset, prompt, usage, reference, and generation-facing DTOs already exist. |
| Frontend state | `src/features/assets/assetLibraryState.ts`, `src/stores/` | Asset list state is feature-local; Studio, project, task, and workspace resume stores remain separate authorities. |

### Asset-related file inventory

Backend asset entry points include:

- `src-tauri/src/domain/asset.rs`;
- `src-tauri/src/application/asset_library_service.rs`;
- `src-tauri/src/application/asset_query_service.rs`;
- `src-tauri/src/application/asset_import_service.rs`;
- `src-tauri/src/application/source_asset_import_service.rs`;
- `src-tauri/src/application/asset_usage_service.rs`;
- `src-tauri/src/application/asset_deletion_service.rs`;
- `src-tauri/src/application/ports/asset_repository.rs`;
- `src-tauri/src/application/ports/asset_browse_repository.rs`;
- `src-tauri/src/application/ports/asset_usage_repository.rs`;
- `src-tauri/src/application/ports/asset_store.rs`;
- `src-tauri/src/infrastructure/database/repositories/asset.rs`;
- `src-tauri/src/infrastructure/database/repositories/asset_browse.rs`;
- `src-tauri/src/infrastructure/database/repositories/asset_usage.rs`;
- `src-tauri/src/infrastructure/filesystem/asset_store.rs`; and
- `src-tauri/src/commands/asset.rs` and `src-tauri/src/commands/consistency_assets.rs`.

Frontend asset entry points include `AssetWorkspace.tsx`, `AssetLibrary.tsx`, `AssetGrid.tsx`, `AssetCard.tsx`, `AssetPreview.tsx`, `AssetUsagePanel.tsx`, `AssetDeleteDialog.tsx`, `TagManagerDialog.tsx`, `assetLibraryState.ts`, and the reference/profile surfaces under `src/features/assets/`.

## 4. Existing domain audit

Status meanings:

- **EXISTS:** a usable authority and implementation already exist;
- **PARTIAL:** the concept exists through one or more narrower records or projections, but no single general-purpose entity exists;
- **ABSENT:** no explicit reusable authority was found.

| Concept | Status | Evidence | Compatibility conclusion |
| --- | --- | --- | --- |
| Asset | EXISTS | `AssetId`, `AssetType`, `Asset`, asset repositories, query/library services, commands, and frontend `AssetView`. | Existing `assets`/`Asset` is the asset authority. Extend only additively; never create a second asset table or ID system. |
| AssetStore | EXISTS | `AssetStore` port and `FileSystemAssetStore` implementation provide image/video/audio writes, thumbnails/posters, reads, streaming, and safe path boundaries. | Reuse the current storage boundary and its project-root conventions. |
| Prompt | EXISTS | `prompt_entries`, `prompt_versions`, `PromptLibraryRepository`, `PromptLibraryService`, prompt commands, and `PromptLibraryPanel`. | Existing prompt library remains authoritative. Do not add a parallel `prompts` table for MVP. |
| Generation | PARTIAL | Generation commands/services, `generation_snapshots`, task generation fields, production orchestrator records, provenance, and telemetry exist. | Generation is currently task/workflow-oriented rather than an independent Asset Library entity. Reuse existing generation authority and expose an adapter/projection only if needed. |
| Result | PARTIAL | `task_output_assets`, `production_item_reviews.result_asset_id`, `production_stage_items.asset_id`, and output collection code link results to assets. | Result is already represented by production/task output links. Audit the required read grain before considering any new result entity. |
| Import | EXISTS | `AssetImportService`, `SourceAssetImportService`, image/video/audio picker commands, signature/size validation, SHA-256, atomic writes, and source import DTOs exist. | Reconcile existing import paths into a candidate/confirm/save experience; do not add another importer. |
| Tag | EXISTS | `asset_tags`, `asset_tag_links`, `asset_favorites`, organization repository/service, tag commands, and tag UI exist. | Existing project-scoped organization system is authoritative. |
| Reference | EXISTS | `shot_reference_assets`, reference anchors/assets, reference sets/items, consistency profiles, scoped bindings, reference services, and reference UI exist. | Preserve typed reference systems and add a generic relation only when a real gap is demonstrated. |
| Asset Version | ABSENT | No explicit `asset_versions` table, domain record, repository, command, or frontend history surface was found. | Candidate for a single additive entity in DEV-122-B, subject to the migration design and legacy baseline strategy. |
| Asset Relation | ABSENT as a generic entity | Multiple specific relations exist, but no generic `asset_relations` table/service/command was found. | Candidate additive entity only if existing links cannot cover the approved MVP relation types. |
| Asset Usage | PARTIAL | `AssetUsageRepository`, `AssetUsageService`, `asset_usage_get`, and aggregate usage DTOs exist without a dedicated `asset_usages` table. | Reuse the current reverse-usage query. Do not create an `asset_usages` table before proving the aggregate is insufficient. |

### Current Asset domain boundary

`src-tauri/src/domain/asset.rs` currently provides:

- stable `ast_...` asset IDs;
- `AssetType::Image`, `AssetType::Video`, and `AssetType::Audio`;
- source and generated categories for image/video/audio;
- project ID, name/original name, storage and thumbnail paths;
- SHA-256, MIME type, dimensions, duration, file size, source task ID, JSON metadata, and timestamps; and
- domain validation and database conversion rules.

The current asset type is intentionally media-oriented. Character, scene, prop, voice, music, and prompt concepts must not be forced into a breaking expansion of `AssetType` during the audit. DEV-122-B should first preserve existing media behavior and introduce taxonomy changes only as an explicitly reviewed compatibility decision.

## 5. Database audit

### 5.1 SQLite runtime

SQLite is initialized through `src-tauri/src/infrastructure/database/pool.rs` with:

- SQLx `Migrator` loading `./migrations`;
- `create_if_missing=true`;
- foreign keys enabled;
- WAL journal mode;
- a five-second busy timeout; and
- a pool maximum of five connections.

The migration test currently expects migration version 32 and 59 application tables. The next migration must be the next sequential migration after 032 and must follow the existing SQLx naming and testing conventions.

### 5.2 Existing tables and fields relevant to Asset Library

#### Core asset and generation records

`001_initial.sql` creates:

- `projects` — project identity and ownership root;
- `workflows`, `workflow_versions`, and `recipes` — workflow/recipe definitions;
- `tasks` — task lifecycle and production input/output context;
- `assets` — the current physical asset catalog;
- `generation_snapshots` — immutable task-bound workflow/recipe and resolved-input snapshot; and
- `task_events` — task event history.

The existing `assets` columns are:

```text
id
project_id
type
category
name
original_name
storage_path
thumbnail_path
sha256
mime_type
width
height
duration_ms
file_size
source_task_id
metadata_json
created_at
updated_at
```

Existing browse indexes cover project, creation time, and category. `source_task_id` preserves a generated asset's task provenance.

`004_video_outputs.sql` creates `task_output_assets`, which maps a task/output/ordinal to an asset and preserves generated result ordering. This is a key existing result relationship.

#### Organization and prompt records

`008_organization.sql` creates:

- `asset_tags` — project-scoped tag vocabulary with normalized-name uniqueness;
- `asset_tag_links` — project-scoped asset/tag many-to-many links; and
- `asset_favorites` — project-scoped favorite state.

`009_prompt_library.sql` creates:

- `prompt_entries` — project-scoped prompt/snippet identity, kind, name, and tag JSON; and
- `prompt_versions` — immutable numbered prompt text revisions.

`011_asset_video_prompt.sql` and `019_shot_stage_prompts.sql` add asset/shot-specific prompt references without replacing the prompt library.

#### Shot, production, review, and reference records

`010_shot_production.sql` creates `shots`, `shot_stage_configs`, `shot_reference_assets`, and `shot_generation_links`. These connect project shots to selected assets, ordered references, prompt versions, tasks, and production batch items.

`012_production_item_review.sql` creates `production_item_reviews`. Its `result_asset_id` points to `assets`, while review status, lineage, notes, task, batch, and parent-item history remain owned by the review system.

`018_production_orchestrator.sql` creates production runs/stages/items. `production_stage_items.asset_id` and `source_asset_id` reference `assets`; task and batch references remain part of the production orchestrator authority.

`020_reference_anchors.sql`, `022_consistency_profiles_and_reference_sets.sql`, and `023_consistency_scope_bindings.sql` create reference anchors, ordered reference sets, profile records, and scoped bindings. These are existing typed reference authorities, not generic asset relations.

Later migrations add preparation snapshots, package bindings, workflow bindings, registry state, and external handoff records. They must remain untouched by Asset Library data-layer work except for read-only compatibility verification.

### 5.3 Existing relationships

The audited foreign-key relationships include:

```text
assets.project_id                 -> projects.id
assets.source_task_id             -> tasks.id
task_output_assets.task_id        -> tasks.id
task_output_assets.asset_id       -> assets.id
asset_tag_links.asset_id          -> assets.id
asset_tag_links.tag_id            -> asset_tags.id
asset_tag_links.project_id        -> projects.id
shots.selected_*_asset_id         -> assets.id
shot_reference_assets.asset_id    -> assets.id
shot_generation_links.task_id     -> tasks.id
production_item_reviews.result_asset_id -> assets.id
production_stage_items.asset_id   -> assets.id
production_stage_items.source_asset_id -> assets.id
reference_anchor_assets.asset_id  -> assets.id
reference_set_items.asset_id      -> assets.id
shot_stage_prompts.*              -> prompt_entries/prompt_versions
generation_snapshots.task_id      -> tasks.id
```

These relations already provide a substantial provenance and usage graph. They must be preserved rather than copied into a new generalized graph without a migration justification.

### 5.4 Physical entity findings

| Proposed v2 entity/table | Current physical status | Reuse decision |
| --- | --- | --- |
| `assets` | EXISTS | Reuse existing table and repository. Add only fields that are proven necessary. |
| `asset_versions` | NOT FOUND | Candidate additive table for independent file/version history. Requires a reviewed legacy baseline mapping. |
| `asset_tags` | EXISTS | Reuse existing `asset_tags` plus `asset_tag_links`; no duplicate tag system. |
| `asset_relations` | NOT FOUND generically | Candidate only after mapping anchors, sets, shot refs, source/output links, and profile bindings. |
| `asset_usages` | NOT FOUND as a table | Reuse `AssetUsageRepository` aggregate query and existing relation tables first. |
| `prompts` | NOT FOUND by that name | Reuse `prompt_entries` and `prompt_versions`. |
| `generations` | NOT FOUND by that name | Reuse task, generation snapshot, orchestrator, provenance, and telemetry records. |
| `results` | NOT FOUND by that name | Reuse `task_output_assets`, review result references, and production stage outputs. |

## 6. Rust backend audit

### 6.1 Existing layers

| Layer | Existing authority | Audit finding |
| --- | --- | --- |
| Model | `domain/asset.rs`, `domain/generation_snapshot.rs`, task, reference, and consistency modules | Asset identity and current metadata are already domain-owned. Version/relation value objects are the likely missing pieces. |
| Repository ports | `application/ports/asset_repository.rs`, `asset_browse_repository.rs`, `asset_usage_repository.rs`, `asset_store.rs` | Persistence and filesystem boundaries are already abstracted and testable. New data must use ports. |
| SQLx repositories | `infrastructure/database/repositories/asset.rs`, `asset_browse.rs`, `asset_usage.rs`, `prompt_library.rs`, `generation_snapshot.rs`, `reference_anchor.rs`, `reference_set.rs` | Existing implementations should be extended narrowly. Do not write SQL directly from command or component code. |
| Services | `asset_library_service.rs`, `asset_query_service.rs`, `asset_import_service.rs`, `source_asset_import_service.rs`, `asset_usage_service.rs`, `asset_deletion_service.rs` | Existing asset use cases cover browse, detail, import, usage, deletion, and safe reference checks. Version/relation use cases are missing or narrow. |
| Commands | `commands/asset.rs`, `commands/consistency_assets.rs`, `commands/prompt_library.rs`, `commands/generation.rs` | Existing typed Tauri command boundaries exist. DEV-122-B should not add user-facing commands as part of data-layer work. |
| Composition | `src-tauri/src/lib.rs`, `src-tauri/src/app_state.rs` | Repository/service construction and command registration are centralized. Any later service wiring belongs to the appropriate follow-up task. |

### 6.2 Existing backend behavior to preserve

- `AssetQueryService` validates project IDs, parses stable asset IDs, loads assets, and rejects cross-project access as not found.
- `AssetBrowseRepository` carries `project_id`, category, keyword, media type, source kind, favorite, tag, order, cursor, and limit in the browse query.
- `AssetUsageService` validates project and entity IDs and returns project-scoped reverse-usage summaries.
- Asset import validates supported media, source signatures/sizes where applicable, calculates SHA-256, and persists through `AssetStore` and repositories.
- Asset deletion inspects task, production, review, profile, reference-set, and snapshot references before cleanup; task history is preserved.
- `FileSystemAssetStore` enforces project-rooted paths, uses temporary files and rename publication, supports streaming writes/reads, and keeps thumbnail/poster operations behind the port.
- Prompt and generation flows use existing repositories and task/workflow identities rather than an Asset Library-owned executor.

### 6.3 DEV-122-B modification locations

If the compatibility gate approves a data-layer extension, DEV-122-B should be limited to these locations:

1. `src-tauri/migrations/` — the next sequential additive migration only, if an explicit version/relation gap is confirmed;
2. `src-tauri/src/domain/asset.rs` or narrowly scoped new domain modules — version/relation records and validation, without breaking current media types;
3. `src-tauri/src/application/ports/` — repository contracts for approved new records;
4. `src-tauri/src/infrastructure/database/repositories/` — SQLx implementations and mapping tests;
5. existing asset repository/service wiring in `src-tauri/src/lib.rs` and `src-tauri/src/app_state.rs`, only if a new repository is actually required; and
6. focused Rust data-layer tests and migration fixtures.

DEV-122-B must not change frontend components, routes, stores, production queue behavior, task lifecycle, review transitions, workflow identity, or external tool execution.

## 7. Frontend audit

### 7.1 Navigation and route authority

The app uses workspace navigation rather than a separate route package. `src/app/App.tsx`:

- defines `assets` as an existing workspace;
- lazy-loads `AssetWorkspace` and `AssetVideoBatchWorkspace`;
- resolves asset targets with `assetId` into the `assets` workspace;
- preserves review/task/batch/shot authorities when multiple IDs are present; and
- keeps project context in navigation requests.

No new top-level route is required for the MVP. The existing Asset Workspace is the extension point.

### 7.2 Existing asset surfaces

`src/features/assets/AssetWorkspace.tsx` already provides tabs for asset-related surfaces. `AssetLibrary.tsx` already supports:

- project-scoped asset browsing;
- keyword input with debounced search;
- category, media type, source, favorite, tag, and created-order filters;
- keyset pagination;
- selection and bulk operations;
- import feedback and partial import failure reporting;
- asset detail/preview selection;
- usage display;
- tags and favorites;
- deletion inspection; and
- video/reference workflows.

`AssetCard.tsx`, `AssetGrid.tsx`, `AssetPreview.tsx`, `AssetUsagePanel.tsx`, and `assetLibraryState.ts` provide reusable rendering, media, usage, and pagination behavior. They are the correct extension points for DEV-122-C; a parallel Asset Library shell is not needed.

### 7.3 Existing frontend types and transport

`src/types/asset.ts` already exposes `AssetView`, import batches/failures, library query filters, pagination cursors, and deletion inspection/result DTOs. The current `AssetView` intentionally exposes safe metadata and tags but not raw storage paths or SHA-256 values.

`src/services/tauriClient.ts` already wraps commands for:

- project-scoped asset browse/detail/recent/task queries;
- image/video/audio/source imports;
- image/thumbnail reads and media URLs;
- asset usage;
- tags, favorites, and bulk organization;
- deletion inspection and deletion;
- asset video prompts; and
- reference anchors and reference sets.

`src/services/ipc.ts` owns the typed `invokeCommand` boundary and error normalization. New frontend behavior must continue through these services; raw `invoke` must not appear in feature components.

### 7.4 DEV-122-C modification locations

After DEV-122-B is complete, the frontend implementation task should primarily modify:

- `src/types/asset.ts` for approved version/relation/provenance DTOs;
- `src/services/tauriClient.ts` and `src/services/ipc.ts` for typed transport parity;
- `src/features/assets/AssetLibrary.tsx`, `AssetPreview.tsx`, and `AssetWorkspace.tsx` for MVP read/detail/version surfaces;
- new narrowly scoped components under `src/features/assets/` only where existing components cannot be extended; and
- `src/features/assets/*.test.tsx` and state tests for list/filter/detail/version behavior.

`src/app/App.tsx` should change only if an existing workspace target cannot carry the detail/version state. A new global asset store is not justified by this audit; keep feature-local asset state and preserve `StudioStore` authority for generation inputs.

## 8. Authority decision

The following decisions are binding for the next implementation task:

| Domain | Unique authority | Status | Prohibited duplicate |
| --- | --- | --- | --- |
| Asset identity and catalog | Existing `assets` table, `Asset` domain, `AssetRepository`, `AssetBrowseRepository`, and asset services | EXISTING | New parallel `assets` table or alternate asset ID. |
| Asset media files | `AssetStore` / `FileSystemAssetStore` | EXISTING | Direct filesystem writes from UI, commands, or arbitrary services. |
| Prompt | `prompt_entries`, `prompt_versions`, prompt repository/service/commands | EXISTING | New `prompts` table or second prompt library. |
| Generation request and execution context | Existing task, generation snapshot, generation definition, orchestrator, provenance, and telemetry authorities | EXISTING/PARTIAL | New Asset Library executor or competing `generations` lifecycle. |
| Generated/imported result mapping | `task_output_assets`, production stage/output records, and review result references | EXISTING/PARTIAL | New result lifecycle that bypasses tasks or Queue Start. |
| Tags and favorites | Existing organization repository and `asset_tags`/`asset_tag_links`/`asset_favorites` | EXISTING | New tag vocabulary or tag-link table. |
| Typed references | Existing shot references, anchors, reference sets, profile bindings, and services | EXISTING | Re-encoding all reference records into a generic relation graph. |
| Reverse usage | Existing `AssetUsageRepository`, `AssetUsageService`, and `asset_usage_get` | EXISTING/PARTIAL | New `asset_usages` table before query-grain proof. |
| Asset version history | No current authority | NEW, if approved | Multiple version tables or mutable history in `assets`. |
| Generic asset-to-asset relations | No current generic authority | NEW, if approved | Several competing relation tables with overlapping semantics. |
| Search/browse | Existing asset browse repository and SQLite indexes | EXISTING | A second client-only catalog/search source. |
| Production execution | Existing Production Queue and Queue Start gate | FROZEN | Any Asset Library queue, executor, or auto-start behavior. |

The rule is **one authority per domain**. When an existing specific relationship is sufficient, Asset Library reads should project it rather than copy it. A new physical entity is justified only by a documented gap, a defined owner, and a migration/test plan.

## 9. Migration preparation

### 9.1 Proposed migration posture

```text
MIGRATION_MODE=ADDITIVE_ONLY
CURRENT_MIGRATION=032
NEXT_MIGRATION=033_SEQUENTIAL_IF_APPROVED
DESTRUCTIVE_CHANGE=NO
LEGACY_ID_REWRITE=NO
QUEUE_TASK_REVIEW_REWRITE=NO
```

No migration is created or executed by DEV-122-A. The first implementation step in DEV-122-B is a disposable-database compatibility rehearsal, not an immediate production migration.

### 9.2 Candidate mapping for the next migration

| Need | Initial preparation decision |
| --- | --- |
| Existing assets | Reuse `assets` unchanged unless a specific additive column is approved. Preserve all IDs, paths, checksums, task links, and timestamps. |
| Version history | Add one `asset_versions` entity only if independent historical file/version records cannot be represented safely by existing data. Legacy assets may receive one deterministic baseline record in a transaction. |
| Tags | No migration expected; reuse existing organization tables and repository. |
| Generic relations | Add one `asset_relations` entity only after existing anchors, sets, shot refs, source/output links, and profile bindings are mapped and proven insufficient. |
| Usage | No migration expected initially; extend the existing usage query/projection. |
| Prompts | No migration expected; reuse `prompt_entries` and `prompt_versions`. |
| Generations | No migration expected; reuse task/snapshot/orchestrator/provenance records. |
| Results | No migration expected; reuse output mappings and production/review result references. |

### 9.3 Required migration safeguards

Before applying an approved migration, DEV-122-B must:

1. back up the local database and record the migration version;
2. inventory project, asset, task, output, tag, prompt, generation, reference, and review counts;
3. rehearse the migration on a disposable copy with foreign keys enabled;
4. avoid drop, rename, truncate, or destructive rewrite operations;
5. keep old IDs and historical references queryable;
6. make any baseline backfill deterministic and idempotent;
7. compare before/after counts and representative relationships;
8. run SQLite foreign-key and project-isolation checks; and
9. verify that Queue, Task, Shot, Review, prompt, generation, thumbnail, and media reads remain valid.

If a legacy record cannot be mapped losslessly, keep it on the legacy path and report it as unmapped. Never infer a relation from a display name or silently discard the record.

## 10. Implementation boundary

### DEV-122-B — Asset Library Data Layer Implementation: IN SCOPE

- perform the final compatibility mapping against a disposable v1.3.1 database;
- implement only the smallest additive migration for a proven version/relation gap;
- add approved domain records and validation rules;
- extend repository ports and SQLx repositories through existing abstractions;
- preserve existing assets, tags, prompts, generation snapshots, output mappings, usage queries, references, and production history;
- implement deterministic legacy baseline handling only if required by the approved version model;
- add focused Rust repository/domain/migration tests; and
- keep project isolation and stable IDs explicit in every new query.

### DEV-122-B: OUT OF SCOPE

- React components, routes, CSS, stores, or Asset Library UI;
- new frontend-facing Tauri commands or transport DTOs;
- import dialog/candidate UX;
- AI tagging, vector search, cloud sync, auto classification, or agents;
- prompt-library replacement or a new generation/result authority;
- media editing, media transcoding, or a background asset worker;
- Production Queue, Queue Start, Task, Shot, Review, workflow, or recipe lifecycle changes;
- destructive migration, ID rewrite, file relocation, or silent relinking; and
- taxonomy expansion that breaks current image/video/audio behavior.

### Later frontend boundary

DEV-122-C may consume the stable data-layer contract to extend the existing Asset Workspace. It should not start until DEV-122-B has established the repository/DTO behavior and migration compatibility evidence.

## 11. Audit acceptance criteria

```text
EXISTING_AUTHORITY_AUDIT=PASS
DATABASE_AUDIT=PASS
BACKEND_AUDIT=PASS
FRONTEND_AUDIT=PASS
MIGRATION_PREPARED=YES
PARALLEL_ASSET_SYSTEM=PROHIBITED
PRODUCTION_QUEUE_AUTHORITY=PRESERVED
PROJECT_ISOLATION=PRESERVED
```

The audit is complete because every requested domain concept has an explicit status, existing physical tables and relationships have been mapped, reuse versus candidate additions has been decided, backend and frontend extension points are identified, and DEV-122-B has a bounded data-layer scope.

## 12. Final status

```text
DEV_122_A=COMPLETE
DEV_122_B_STARTED=NO
AUTO_NEXT_TASK=NO
NEXT_STEP=DEV_122_B_ASSET_LIBRARY_DATA_LAYER_IMPLEMENTATION
```
