# AI Studio v2 Asset Library MVP Implementation Plan

```text
TASK=DEV-121
STATUS=IMPLEMENTATION_PLANNING_ONLY
BASELINE=AI_STUDIO_v1.3.1_STABLE
CODE_CHANGE=NO
DATABASE_MIGRATION=NO
SCHEMA_IMPLEMENTATION=NO
UI_IMPLEMENTATION=NO
API_CHANGE=NO
IMPLEMENTATION_PLAN_READY=YES
ESTIMATED_COMPLEXITY=HIGH
RECOMMEND_START_DEV=YES
NEXT_STEP=DEV-122_ASSET_LIBRARY_MVP_IMPLEMENTATION
```

## 1. Purpose and planning boundary

DEV-121 defines the engineering execution plan for the Asset Library MVP. It is a planning and handoff document only. It does not create tables, add migrations, change Rust or React code, add commands, or alter the v1.3.1 production workflow.

The implementation must start as a separate DEV-122 task. The existing Production Queue remains the only production execution authority, and the Studio Store remains the authoritative frontend state boundary. Asset Library work must not create a second queue, executor, task model, or production lifecycle.

## 2. MVP objective

The MVP makes personal creative assets findable, inspectable, traceable, and reusable without trying to become a full media editor. The first release must provide:

- **Asset Metadata** — project ownership, kind/category, display name, original filename, media metadata, storage status, timestamps, and source information.
- **Asset Version** — a non-destructive history of file or metadata snapshots, with a stable asset identity and an explicit current version.
- **Tags** — project-scoped, reusable labels with assignment, removal, and basic filtering.
- **Preview** — image, video poster, audio metadata, and existing thumbnail/media URL behavior where supported.
- **Relations** — explicit typed links such as source, variant, derived output, reference, or replacement.
- **Provenance** — whether an asset was imported, generated, attached to a task/shot, or derived from another asset/prompt.
- **Basic Search** — text search over names and original filenames plus project, kind/category, tag, and storage-status filters.

The MVP is successful when a person can import or locate an asset, understand what it is and where it came from, see its versions and relations, and use the existing production surfaces without changing the production execution gate.

### MVP invariants

1. Every project-owned asset remains isolated by `project_id`; cross-project reads and relations are rejected or excluded.
2. An asset ID is stable even when its media file is replaced, moved, or restored.
3. A new file is represented by a new version; existing historical versions are not silently overwritten.
4. The asset catalog stores metadata and references, not large media blobs.
5. A missing or moved file is visible and recoverable; it is never silently treated as a valid preview.
6. Existing Project, Shot, Task, Queue, Review, prompt, and generation references remain valid.
7. Asset operations do not start production work. Queue Start remains the only execution gate.

## 3. Existing baseline and reuse-first map

The repository already contains a substantial v1 asset system. DEV-122 must begin with a compatibility audit and extend the existing system rather than introduce parallel concepts.

### Existing backend and persistence to reuse

- `src-tauri/src/domain/asset.rs` already defines `AssetId`, asset types, categories, and the v1 asset metadata shape.
- `src-tauri/src/application/asset_library_service.rs`, `asset_query_service.rs`, `asset_import_service.rs`, `asset_usage_service.rs`, and `asset_deletion_service.rs` already contain asset use cases that must be reused or carefully evolved.
- Existing asset repository ports and `AssetStore` provide the persistence and filesystem boundaries. New behavior must continue through repository abstractions and the existing storage abstraction.
- `src-tauri/src/commands/asset.rs` is the natural command boundary. New commands, if needed in DEV-122, must have typed frontend transport parity.
- Existing migrations already contain `assets`, browse indexes, tags and tag links, prompt entries and versions, task/shot asset links, generation snapshots, runtime provenance, and telemetry records.
- Existing production-output and task-reference records may already cover portions of the logical `results` and `asset_usages` concepts. Their exact grain and retention must be audited before adding any physical table.

### Existing frontend to reuse

The first frontend implementation should build on `src/features/assets/`, including `AssetLibrary.tsx`, `AssetWorkspace.tsx`, `AssetCard.tsx`, `AssetGrid.tsx`, `AssetPreview.tsx`, `AssetUsagePanel.tsx`, and `assetLibraryState.ts`. Existing prompt-library surfaces should remain the presentation of the existing prompt authority.

The transport must remain `src/services/tauriClient.ts` and `src/services/ipc.ts`. Feature components must not call raw Tauri `invoke`. Existing media URL, thumbnail, usage, tag, favorite, and project-scoped browse behavior should be preserved.

## 4. Logical data architecture

The following are logical entities for the MVP. A logical entity does not automatically mean a new physical table: DEV-122 must first map it to the current schema and only add an additive migration for a genuine missing capability.

### 4.1 Entity responsibilities and compatibility mapping

| Logical entity | MVP responsibility | v1.3.1 reuse and implementation note |
| --- | --- | --- |
| `assets` | Stable project-owned identity, descriptive metadata, current status, and catalog entry. | Reuse the existing `assets` table and `Asset` domain model. Do not create a second asset table. Any extension must be additive and backward-compatible. |
| `asset_versions` | Immutable file/metadata snapshots, version sequence, checksum, media metadata, provenance, and current-version selection. | Candidate additive table after schema audit. Existing assets can receive an explicit baseline version only through a reviewed, idempotent backfill; old asset IDs and references remain unchanged. |
| `asset_tags` | Tag vocabulary and asset-to-tag assignment. | The logical concept is already split across existing `asset_tags` and `asset_tag_links`. Reuse that system; do not create another tag-link mechanism. |
| `asset_relations` | Typed directed links between assets, including source, variant, derived, reference, replacement, and related. | Candidate additive capability. Existing reference sets, anchors, shot references, and generation links must be mapped first so the new relation layer does not duplicate a stronger existing relation. |
| `asset_usages` | Explain where an asset is used: project, shot, task, production item, prompt/generation context, or export. | Candidate logical read model. Existing task output, shot reference, production, usage, and provenance records must be queried or adapted before adding a new physical table. |
| `prompts` | Prompt identity, prompt versions, provider/model context, and reusable prompt linkage. | Reuse/align with existing `prompt_entries` and `prompt_versions`. A facade or adapter is preferred over a parallel `prompts` table. |
| `generations` | A generation attempt and its input/configuration/provenance, linked to outputs. | Reuse existing generation snapshots, task/generation records, runtime provenance, and telemetry where they already provide the required grain. Do not create a second generation authority. |
| `results` | Generated or imported output records that can become asset versions or asset links. | Treat as a logical result projection over existing task output and generation records unless an audited gap requires an additive table. Queue/task lifecycle remains authoritative. |

### 4.2 Conceptual relationships

```text
Project 1 ──── * Asset
Asset   1 ──── * AssetVersion
Asset   * ──── * Tag
Asset   1 ──── * AssetRelation (from/to another project-owned Asset)
Asset   1 ──── * AssetUsage (Shot/Task/Production/Export context)
Prompt  1 ──── * PromptVersion
Generation * ── 1 PromptVersion (optional input)
Generation 1 ── * Result
Result     ──── 1 AssetVersion or creates an AssetVersion
```

Relations must use stable IDs and explicit relation types. Names are display data only and must never be used to infer identity. Workflow references continue to use the exact `workflowVersionId` plus `recipeId` pair wherever they are already required.

### 4.3 Conceptual records, not a schema commitment

DEV-122 may refine fields, indexes, and constraints after the audit. At planning level, the records need to answer:

- **Asset:** which project owns it, what kind it is, how it is named, and which version is current.
- **Asset version:** which asset it belongs to, which sequence or revision it represents, where the file is, how it was checked, and which metadata/provenance was captured at that time.
- **Tag/link:** which project-scoped vocabulary item is assigned to which asset.
- **Relation:** source asset, target asset, relation type, optional context, and creation time.
- **Usage:** asset, project context, owning object type/ID, usage role, and active/historical status.
- **Prompt/prompt version:** reusable text and parameters with an explicit revision.
- **Generation/result:** request context, model/tool, parameters, references, status, output identity, and links to the existing task/queue records.

All new relationships need foreign-key and project-isolation rules appropriate to the existing SQLite conventions. Exact SQL belongs to DEV-122, not this plan.

## 5. Migration strategy

### 5.1 Additive-only policy

```text
MIGRATION_STRATEGY=ADDITIVE_ONLY
```

The transition from v1.3.1 to v2 Asset Library must be additive only:

- no dropping, renaming, truncating, or destructive rewriting of v1 tables;
- no change to existing Project, Shot, Task, Queue, Review, production, or workflow identity semantics;
- no change to historical task/output references;
- no replacement of existing prompt, generation, tag, or storage authorities without an explicit compatibility adapter;
- new nullable metadata or new tables only where the audit demonstrates a real gap;
- existing IDs remain valid and readable after every migration step.

### 5.2 Execution sequence for DEV-122

1. **Backup and inventory.** Take a database backup, record the migration version, count existing assets and links, and identify all foreign-key and repository consumers.
2. **Compatibility audit.** Compare each logical entity with existing tables and services. Confirm whether version, relation, usage, and result data already exists under another name or grain.
3. **Additive migration design.** Write the smallest migration set. Prefer extending `assets` or exposing a compatibility query over creating a duplicate entity.
4. **Idempotent baseline handling.** If `asset_versions` is needed, create a deterministic baseline version for legacy assets inside a transaction, preserving the existing asset ID, path, checksum, and timestamps. Record counts and make reruns safe.
5. **Dual-read or adapter period.** Let repositories read both legacy records and new MVP records while the mapping is validated. Do not add dual writes to competing authorities unless the write owner and consistency rule are explicit.
6. **Validation.** Run foreign-key checks, before/after count comparisons, project-isolation tests, representative historical task/shot queries, and migration rollback/backup-restore checks in a disposable copy.
7. **Release gate.** Only after the compatibility checks pass may the new command and frontend surfaces consume the expanded catalog.

### 5.3 Preservation requirements

The following must remain queryable and behaviorally unchanged:

- existing projects and their ownership boundaries;
- shots and reference/output links;
- tasks and their historical status/output relationships;
- Queue Start and the existing production execution path;
- Review, rework, and production-item history;
- existing prompt and generation provenance;
- existing asset files, paths, checksums, thumbnails, and media URLs.

If a legacy record cannot be mapped losslessly, it must remain available through the legacy path and be marked as unmapped for later review; it must not be deleted or guessed into a new relation.

## 6. Rust backend implementation plan

DEV-122 should retain the current layered architecture and use the following logical boundaries:

```text
src-tauri/src/
  domain/asset.rs
  application/
    asset_library_service.rs
    asset_query_service.rs
    asset_import_service.rs
    asset_usage_service.rs
    ports/
      asset_repository.rs
      asset_browse_repository.rs
      asset_usage_repository.rs
      asset_store.rs
  commands/asset.rs
  infrastructure/...
```

### Responsibilities

- **Domain/model:** stable IDs, asset/version/relation value objects, allowed kinds and statuses, project ownership, and validation rules. Extend the current `Asset` model only when the compatibility audit proves the field is needed.
- **Repository ports:** query and persistence contracts for catalog, versions, tags, relations, and usage. Ports must hide SQLx and remain testable with focused fakes.
- **Application services:** coordinate use cases such as list/search, get detail, update safe metadata, create a new version, list version history, assign tags, query relations, and inspect provenance.
- **Asset store:** retain filesystem operations, checksum calculation, thumbnail/poster handling, missing-file checks, and safe import staging. Do not write media blobs through SQLite.
- **Commands:** expose only the approved typed boundary to the frontend. Commands must enforce project scope and return actionable, structured errors.
- **Infrastructure repositories:** implement the ports with the existing SQLx/migration conventions and preserve historical rows. Any new repository must not bypass repository abstractions with direct writes from application code.

### Backend contract rules

- Reads and writes must carry the project context explicitly.
- Metadata updates must not mutate historical version snapshots.
- Version creation must be atomic with its catalog update and must not start a production task.
- Relation creation must validate both assets belong to the permitted project scope.
- Missing media must return a typed state, not a generic success with an empty preview.
- All frontend-facing commands need matching entries in `src/services/ipc.ts` and `src/services/tauriClient.ts`, plus raw-invoke guard coverage.

## 7. Frontend implementation plan

The MVP should extend the current assets feature rather than create a separate application shell.

```text
Asset Library
├── Asset List
├── Filters
├── Asset Detail
└── Version History
```

### Asset List

Reuse the existing asset workspace, grid, card, pagination, project scope, thumbnail, favorite, tag, and usage patterns. Add basic search and filters for text, kind/category, tags, status, and provenance. Loading, empty, error, and missing-file states must be explicit.

### Filters

Filters should be deterministic and server-backed where the existing browse API supports it. The selected project is always part of the query. Filter state should remain in the existing feature state boundary and must not become a second global store.

### Asset Detail

The detail surface should show metadata, preview, checksum/path status, tags, provenance, relations, and usage references. It may offer basic management—rename/update safe metadata, tag assignment, favorite, and a controlled new-version action—but it is not a media editor.

### Version History

Show current and historical versions in descending order with file metadata, checksum, creation source, and preview availability. Historical versions are read-only. Replacing a file creates a new version rather than silently rewriting the previous one.

### Frontend constraints

- Use `src/services/tauriClient.ts` and `src/services/ipc.ts` for all backend communication.
- Reuse `AssetPreview.tsx`, `AssetUsagePanel.tsx`, media URL helpers, and existing tag controls where possible.
- Keep accessibility for keyboard navigation, focus, labels, and media alternatives.
- Keep the first version read-mostly and operationally simple; no complex editor, drag-and-drop graph, or speculative workspace abstraction.

## 8. File storage plan

### Source of truth split

- **Filesystem:** original media, generated media, thumbnails/posters, and other preview derivatives.
- **SQLite:** asset identity, version metadata, relative storage reference, checksum, MIME type, dimensions/duration, size, provenance, status, and relations.

The existing `AssetStore` and its app-managed storage conventions remain the storage boundary. Persist a stable relative storage reference where possible; do not make an absolute machine-specific path the asset identity.

### Required path and file behavior

- **Path:** resolve through the app storage root and normalize safely. The database reference must survive a workspace/app relocation.
- **Checksum:** calculate while importing or verifying; use it for integrity, duplicate detection hints, and moved-file recovery. A checksum match is evidence, not permission to merge distinct asset identities automatically.
- **Missing file:** retain metadata, mark the version unavailable, show a clear state, and preserve all historical links. Preview commands must return a typed missing result.
- **Moved file:** do not silently guess or rewrite. Provide a future controlled repair/relink operation that verifies the selected file, records the new path and checksum, and preserves the old path as audit information.
- **Large media:** stream copy and checksum calculation, stage outside the final catalog location, and avoid loading full files into memory or SQLite.
- **Atomic save:** write to a temporary location, verify size/checksum, move into the managed store, then commit metadata. A failed copy must not leave a catalog row that looks complete.
- **Preview:** generate or reuse thumbnails/posters through the current store behavior; preview failure must not destroy the original asset.

## 9. Import implementation plan

The future import flow is deliberately confirmable and non-destructive:

```text
Select File
    ↓
Metadata Scan
    ↓
Asset Candidate
    ↓
Confirm
    ↓
Save
```

1. **Select File:** use the existing typed picker/import boundary and accept only supported media types for the MVP.
2. **Metadata Scan:** inspect filename, MIME type, size, dimensions/duration, checksum, and preview eligibility without creating a production task.
3. **Asset Candidate:** show the proposed project, name, kind/category, tags, storage destination, duplicate hints, and provenance. No catalog mutation is required for cancel.
4. **Confirm:** validate project scope, supported type, storage availability, and user edits. Re-check the source file before committing.
5. **Save:** stage and verify the file, persist the asset/version metadata transactionally, create preview derivatives where applicable, and return the stable asset ID.

Existing image, video, and audio import paths must be reconciled into this flow in DEV-122. The result must be one import authority, not separate legacy and v2 import implementations.

## 10. Testing and verification plan

### Backend

- create asset with project isolation and required metadata;
- update allowed metadata without mutating historical versions;
- create a new version with checksum/path/media metadata;
- list version history in stable order;
- assign, remove, and filter tags;
- create and query typed asset relations;
- query usage/provenance across existing shot/task/production references;
- return deterministic missing-file and checksum-mismatch states;
- prevent cross-project asset reads and relation creation;
- verify failed import cleanup and no partial catalog row;
- confirm asset actions do not bypass Queue Start or create a second task/executor.

### Frontend

- Asset List renders loading, empty, error, and populated states;
- basic search and filter combinations update the displayed results;
- Asset Detail displays metadata, preview state, tags, relations, usage, and provenance;
- Version History renders current and historical versions and keeps history read-only;
- missing and moved-file states are actionable and not represented as blank successful previews;
- typed transport calls are used and raw `invoke` guard tests remain green;
- keyboard/focus/accessibility behavior is preserved for list, filter, detail, and preview controls.

### Migration and compatibility

- migrate a representative v1.3.1 database on a disposable copy;
- verify existing projects are preserved;
- verify existing shots, tasks, queue records, review history, and production references remain queryable;
- compare asset, tag, prompt, generation, and output counts before and after;
- run foreign-key and project-isolation checks;
- verify legacy media URLs, thumbnails, paths, and checksums still resolve;
- rerun any baseline backfill to prove idempotence;
- test backup/restore before enabling the new catalog paths.

### Required implementation gates

DEV-122 should run focused Rust tests and `cargo check` for backend changes, focused frontend tests plus TypeScript/build for frontend changes, and the full cross-layer gates when migration, command parity, or shared transport changes are involved. The current CI-OPT-003 rule remains in force.

## 11. Development order

No phase below is executed by DEV-121. It is the ordered handoff for DEV-122.

### Phase 0 — Compatibility audit and contract freeze

- inventory current assets, tags, prompts, generations, output links, usage records, and storage paths;
- confirm which logical entities are already represented;
- document repository/command/transport contracts and migration invariants;
- establish backup, fixture, and before/after count checks.

**Exit:** no duplicate authority is planned, and every proposed new field/table has a documented compatibility reason.

### Phase 1 — Data layer

- implement only the smallest additive migration approved by the audit;
- extend domain models and repository ports;
- add SQLx repository implementations and legacy adapters where required;
- add baseline version handling only if it is lossless and idempotent;
- add data-layer tests for project isolation, versions, tags, relations, paths, and migration preservation.

**Exit:** existing v1.3.1 data and production references pass compatibility tests.

### Phase 2 — Backend API

- implement catalog/search/detail/version/relation/provenance use cases;
- reconcile existing import services into the candidate/confirm/save flow;
- expose the minimum typed Tauri commands;
- update IPC/transport parity and structured error handling;
- verify no queue/task lifecycle behavior changed.

**Exit:** backend tests and command/transport guards pass with a disposable migrated database.

### Phase 3 — Frontend

- extend existing Asset List, Filters, Asset Detail, and Version History surfaces;
- reuse preview, tags, usage, media URL, pagination, and feature-state patterns;
- add only basic management controls and explicit loading/error/missing states;
- keep all data access through the typed transport.

**Exit:** list/filter/detail/version flows are usable, accessible, and covered by focused frontend tests.

### Phase 4 — Integration and release gate

- run import-to-catalog-to-preview flow;
- run version/relation/provenance flow against real repositories;
- verify migrated projects and historical production flows;
- run cross-layer test, TypeScript, build, Rust, and required CI gates;
- document known limitations before release.

**Exit:** Asset Library MVP is ready for a separately approved release decision; no automatic follow-on work is started.

## 12. Non-goals

The MVP explicitly does not include:

- **AI Tagging**;
- **Vector Search** or embeddings;
- **Cloud Sync** or remote storage orchestration;
- **Auto Classification**;
- **AI Agent** or autonomous decisions;
- SaaS, multi-user accounts, permissions, billing, or enterprise administration;
- replacing ComfyUI, IndexTTS, ACE-Step, Agnes Creator, VRBoxPlayer, or other local tools;
- a new production queue, executor, task model, workflow engine, or review system;
- a full image/video/audio editor;
- speculative taxonomy expansion for Character/Scene/Prop as a breaking change to current asset types.

## 13. Risk control

| Risk | Control |
| --- | --- |
| Data migration risk | Additive-only migrations, backup, disposable-copy rehearsal, before/after counts, foreign-key checks, idempotent baseline handling, and legacy fallback. |
| File path risk | Stable asset IDs, app-managed relative references, checksum verification, explicit missing/moved states, staged atomic writes, and no silent relinking. |
| Large media risk | Filesystem-only media, streaming copy/checksum, bounded metadata scans, asynchronous preview work where already supported, and no SQLite BLOB storage. |
| UI complexity risk | Reuse the existing assets feature, deliver list/filter/detail/history only, keep basic management, and avoid a graph editor or media editor. |
| Existing-system duplication risk | Compatibility audit first; reuse current assets, tags, prompts, generation, usage, provenance, commands, and `AssetStore`; add a physical entity only for a proven gap. |
| Project-isolation risk | Carry project context through every repository/service/command query and test cross-project rejection explicitly. |
| Production regression risk | Keep Queue Start as the only execution gate, preserve task/shot/review references, and add regression tests before integration. |

## 14. Implementation readiness and DEV-122 handoff

```text
IMPLEMENTATION_PLAN_READY=YES
ESTIMATED_COMPLEXITY=HIGH
RECOMMEND_START_DEV=YES
```

The complexity is **HIGH** for implementation—not because the MVP needs a large new UI, but because the repository already has overlapping asset, prompt, generation, provenance, import, usage, and production-reference capabilities. Compatibility and authority boundaries are the primary engineering work.

DEV-122 may start only after the implementer confirms:

- the v1.3.1 stable baseline and clean tracked worktree;
- the compatibility inventory and mapping of all eight logical entities;
- the smallest additive migration, if any, and its backup/restore rehearsal;
- repository/command/typed-transport contracts;
- project isolation and Queue/Task/Review preservation tests;
- the file-store behavior for checksum, missing, moved, and large files;
- the frontend reuse plan and explicit MVP cut line.

This document intentionally records a plan, not an implementation. DEV-121 makes no product-code, database, schema, UI, or API change.

