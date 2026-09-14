# DEV-130 — AI Studio v2 Personal Edition Release Review

```text
TASK=DEV-130
KIND=REVIEW_AND_DOCUMENTATION
BASELINE_SHA=5711ba8fddbc3711ea6fa26de3ef56e732b98daa
BRANCH=feat/dev-129-b-archive-data-foundation
VERSION=1.3.1_STABLE + v2 additive layers 033-036 / backup v19
CODE_CHANGE=NO
PRODUCTION_FLOW_CHANGE=NO
TASK_STATE_MACHINE_CHANGE=NO
GENERATION_EXECUTION_CHANGE=NO
NEW_ARCHIVE_TABLES=NO
DEV_131_STARTED=NO
PUSH=NO
```

This is a review of current HEAD. It does not start DEV-131, does not change
Production Core / Task / Generation execution, and does not add Archive catalog
tables. The v1.3.1 Production Queue remains the only production execution
authority.

## 0. Verdict (read this first)

The v2 Personal Edition **module foundations are in place**. The claimed
end-to-end loops are **not**. Three write-path gaps still prevent an honest
`READY=YES` for the data loop:

1. Interactive generation does not keep Comfy execution serialized with the
   production queue (admission released after spawn; `submission_permit`
   dropped after POST `/prompt`).
2. Provenance tables exist, but generation completion does not write
   `generation_tool_usages` / `generation_asset_versions`.
3. Frontend `createGeneration` and Prompt writes omit `modelVersionId`, so
   Prompt → Model → Generation is not persisted from the product UI.

```text
V2_PERSONAL_EDITION_READY=NO
BLOCKERS=COMFY_SERIAL; PROVENANCE_WRITE_PATH; MODEL_VERSION_ID_UI
ARCHIVE_MVP=YES
LIVE_GATES=GPU_COMFY_AFTER_RESTORE; PRODUCT_SCALE_ARCHIVE; DESKTOP_INSPECT_RESTORE_UX; RESTORED_TOOL_UNKNOWN; MISSING_MEDIA_RELINK
```

`ARCHIVE_MVP=YES` means the 129-C/D inspect-first `.aiarchive` backup path is
real. It does **not** mean a persistent Archive catalog, auto-backup, or
product-scale proof.

---

## 1. Claimed loops vs current authorities

Authorities in this codebase (not the planning vocabulary):

| Claimed name | Actual authority | Notes |
| --- | --- | --- |
| Project | `projects` | Isolation boundary |
| Reference Asset | `assets` + `reference_anchors` / `reference_sets` | Production references stay on existing anchors/sets |
| Asset | `assets` | No second Asset store |
| Prompt | `prompt_entries` / `prompt_versions` | Shot/asset prompt text is production context, not a second library |
| Model | `models` / `model_versions` | Personal registry; optional FKs only |
| Tool | `tools` / `tool_instances` / `tool_versions` / `tool_capabilities` | Metadata; Comfy runtime is still `AppSettings` + `ComfyService` |
| Generation | `tasks` + `generation_snapshots` | No `generations` table. `generation_id` in 036 is `tasks.id` |
| Result | `task_output_assets` + `assets` | No `results` table |
| AssetVersion | `asset_versions` | Schema/service exist; generation does not auto-write |
| Archive | `ProjectBackupService` v19 / `.aiarchive` | No `Archive` / `ArchiveVersion` tables |

---

## 2. Functional loop

`Project → Reference Asset → Prompt → Model → Tool → Generation → Result → AssetVersion → Archive`

| Step | Status | Evidence | Honest meaning |
| --- | --- | --- | --- |
| Project | **PASS** | `src-tauri/src/application/project_service.rs`; `src/features/projects/ProjectWorkspace.tsx` | Local project create/open/isolation works. |
| Reference Asset | **PASS** | `src-tauri/src/application/reference_anchor_service.rs`; `reference_set_service.rs`; Asset Library detail | User can pick project-owned assets and ordered references. |
| Prompt | **PASS** | `prompt_library_service.rs`; `src/features/prompts/PromptStudio.tsx`; `PromptLibraryPanel.tsx` | Reusable Prompt identity + append-only versions exist. Studio is read-mostly; writes stay on the library panel. |
| Model | **PARTIAL** | Registry: `model_service.rs`, `commands/model.rs`, `migrations/034_prompt_studio_model_foundation.sql`. UI: `PromptStudio.tsx` reads `listModels` / `listModelVersions`. Writes: backend `model_create` exists; `tauriClient.ts` has no `createModel`. Prompt/Generation UI does not send `modelVersionId` (see §5). | Registry can store ModelVersion. The product UI does not close Prompt→Model or Generation→Model. |
| Tool | **PARTIAL** | Registry: `tool_service.rs`, `commands/tool.rs`, `migrations/035_local_tool_hub_data_foundation.sql`. UI: `src/features/tools/LocalToolHub.tsx` is list/get only. Comfy execution: `comfy_service.rs` / `generation_service.rs`. | Tool Hub describes tools. It does not select, invoke, or record usage on a run. |
| Generation | **PARTIAL** | Execution: `generation_service.rs`, `production_queue_service.rs`, Queue Start admission. Snapshot: `generation_snapshots.model_version_id` optional. Scheduler classifies `GPU_*_SERIAL` / `max_concurrent=1` in `scheduler.rs` but does not own the gate. | Queue Start still executes production. Interactive create overlaps Comfy (§5). Snapshot ModelVersion is optional and unused by UI. |
| Result | **PASS** | `generation_service.rs` `complete_success` → `asset_import_service.import_outputs`; `task_output_assets`; `assets.source_task_id` | Result Asset identity is the existing production path. |
| AssetVersion | **PARTIAL** | Schema/service: `migrations/033_asset_library_data_layer.sql`, `asset_data_service.rs`. UI list: `asset_versions_list` / `AssetPreview.tsx`. Writers: `insert_asset_version` is used by `AssetDataService` tests and provenance tests, **not** by `complete_success` / `output_collector.rs` / import. | History can be stored. A successful generation does not create an AssetVersion. |
| Archive | **PARTIAL** | Backup MVP: `project_backup_service.rs` v19 + `.aiarchive`; `ProjectWorkspace.tsx` export / inspect / restore. Catalog: none (deferred). Scale: 129-D N=120/120/200. | Personal inspect-first restore works for the backup format. Not a catalog, not product-scale, not a live GPU proof after restore. |

Functional-loop close: **NO**. A person can walk the screens, but Tool usage, ModelVersion, AssetVersion, and Archive catalog are not one automatic production record.

---

## 3. Data loop

`Project → Asset → Prompt → Model → Tool → Generation → AssetVersion → Archive`

| Hop | Status | Evidence |
| --- | --- | --- |
| Project → Asset | **PASS** | `assets.project_id`; `asset_library_service.rs` / `asset_query_service.rs` project-scoped list/detail |
| Asset → Prompt | **PARTIAL** | Prompt Library is project-scoped. PromptVersion has no required Asset FK. References remain `reference_anchors` / `reference_sets`. Prompt Studio empty-states “未记录参考素材关联” when none exist. |
| Prompt → Model | **PARTIAL** | `prompt_versions.model_version_id` (034, optional). Backend `prompt_library_create` / `prompt_library_add_version` accept it. Frontend `PromptLibraryCreateRequest` and `addPromptLibraryVersion()` omit it. |
| Model → Tool | **DEFERRED** | No FK / usage relation. Do not infer from names or capabilities. |
| Tool → Generation | **FAIL** (write path) | 036 `generation_tool_usages` + `ProvenanceLineageService.create_tool_usage` + `generation_tool_usage_create` exist. `generation_service.complete_success` never calls them. Frontend `tauriClient.ts` has list only. |
| Generation → AssetVersion | **FAIL** (write path) | 036 `generation_asset_versions` requires an existing `asset_versions` row **and** `task_output_assets` key. Generation creates the Asset, not the AssetVersion, and does not write the link. |
| AssetVersion → Archive | **PASS** if rows exist | Backup v19 snapshots `asset_versions`, `asset_relations`, referenced models/tools, and lineage (`project_backup.rs` / `project_backup_service.rs`). Restore remaps into a new project. Empty versions/lineage archive as empty. |

Data-loop close: **NO**. Schema can represent the chain. Live UI + generation execution do not write the chain.

---

## 4. Module inspection (this branch)

### 4.1 Production Core — PASS (unchanged v1.3.1 authority)

Prepare → Queue → Start → Monitor → Review → Rework still holds.

- Queue Start: `production_start_admission_service.rs`
- Queue: `production_queue_service.rs` (serial item dispatch; waits on dispatched task terminal status)
- Generation execution: `generation_service.rs`
- 128-B/C and 129-B/C/D documents all record `QUEUE_CHANGE=NO` / `GENERATION_EXECUTION_CHANGE=NO`

v2 modules must not be described as a second executor. They are not.

### 4.2 Asset Library — HARDENED catalog, PARTIAL version write

- Data: 033 + `asset_data_service.rs`
- UI: `src/features/assets/` (DEV-122-C / DEV-123)
- Isolation: list/detail/version/relation are project-scoped
- Generated results land as `assets` via import, not as `asset_versions`
- Read-path fencing remains weaker than delete (see §5); 129-D did not change it

### 4.3 Prompt Studio — HARDENED view, PARTIAL provenance

- Data: existing Prompt Library + 034 Model registry
- UI: `PromptStudio.tsx` (read), `PromptLibraryPanel.tsx` (write)
- History is `taskHistoryPage({ projectId, filter: "ALL" })` — project-wide, not prompt-linked
- UI copy already admits this (`PromptStudio.tsx` “当前数据层没有 Prompt Version → Generation 显式关系”)
- `PROMPT_GENERATION_LINK=DEFER` from DEV-125 is still the recorded decision

### 4.4 Local Tool Hub — MVP metadata, read-only client

- Data: 035 + `tool_service.rs` (create/update/health/version commands exist)
- Typed client: `listTools` / `listToolInstances` / `listToolVersions` / `listToolCapabilities` only (`tauriClient.ts` ~416–430)
- UI: `LocalToolHub.tsx` — no register, probe, launch, or health-record action
- Runtime: Comfy still owned by settings + `ComfyService`, not Tool Hub

### 4.5 Provenance (DEV-128) — READ PATH PASS, WRITE PATH FAIL

| Layer | Status | Path |
| --- | --- | --- |
| Schema | PASS | `src-tauri/migrations/036_provenance_lineage.sql` |
| Service / IPC create | PASS as API | `provenance_lineage_service.rs`; `commands/provenance_lineage.rs` |
| Generation write | FAIL | `complete_success` imports outputs only; no lineage insert |
| Frontend write | FAIL | `tauriClient.ts` exports `listGenerationToolUsages` / `listGenerationAssetVersionLinks` only |
| Frontend read | PASS | `AssetPreview.tsx`, `PromptStudio.tsx`, `TaskHistoryDetail.tsx` |

DEV-128-B said commands were “for future typed-transport consumers only; no
frontend surface or production execution path was changed.” That is still true
for the write path. Visibility without writers means live generations show empty
lineage (the UI empty-states are honest).

### 4.6 Archive (129-A..D on this branch) — MVP PASS, catalog DEFERRED

| Slice | Status | Notes |
| --- | --- | --- |
| 129-A audit | PASS | Existing `ProjectBackupService` is the authority |
| 129-B data foundation | PASS | Backup v18 → **v19**; additive collections for versions, relations, referenced models/tools, lineage |
| 129-C export/import | PASS | `.aiarchive` ZIP; `manifest.json` + `project.json` + `provenance.json`; inspect-first; v18 `.zip` still restores |
| 129-D hardening | PASS at tested N | Truncated ZIP, checksum/inventory/media mismatch, zip-slip, restore path remap, host-path leak closed |
| Archive catalog tables | **DEFERRED** | No `Archive` / `ArchiveVersion` / browser. Intentional. |
| Product-scale | **DEFERRED** | 129-D used Assets=120 / Versions=120 / Relations=200 / 1 project — not 100 / 10k / 50k / 100k |

Frontend: `exportProjectBackup` / `inspectProjectBackup` / `restoreProjectBackup` in
`ProjectWorkspace.tsx`. Restore confirms new project, no overwrite, no auto-generation.

---

## 5. Known open issues (not in 129 scope) — still open at 5711ba8

### 5.1 Interactive generation vs production queue — Comfy not serialized — BLOCKER

Production queue is serial at the **item** level: `run_loop` waits while an item
is `Dispatched` (`production_queue_service.rs`).

Interactive create is not:

1. `commands/generation.rs` `generation_create` acquires
   `acquire_interactive_admission()` then calls `start_generation`.
2. `start_generation` registers the task and `tokio::spawn`s `execute_prepared`
   (`generation_service.rs` ~363–378), then returns.
3. The admission `OwnedMutexGuard` drops when the command returns — **after
   spawn, not after Comfy completion**.
4. Inside execution, `submission_permit` is acquired around POST `/prompt` and
   **explicitly `drop(submission_permit)` after submit** (~816–936), before the
   event loop that waits for Comfy to finish.

`scheduler.rs` still advertises `GPU_STANDARD_SERIAL` / `GPU_HEAVY_SERIAL` with
`max_concurrent: 1`. That is classification metadata, not the gate. Two
interactive (or interactive + production) Comfy executions can overlap after
submit.

### 5.2 Frontend `createGeneration` / Prompt write omit `modelVersionId` — BLOCKER

Backend accepts optional `model_version_id`:

- `commands/generation.rs` `GenerationCreateRequest`
- `CreateGenerationRequest.model_version_id` persisted onto `generation_snapshots`
- `commands/prompt_library.rs` create / add-version

Frontend does not send it:

- `tauriClient.createGeneration` request type: `projectId`, `workflowVersionId`,
  `recipeId`, `values`, optional `submissionIdempotencyKey` only
- Callers: `useGenerationSubmissionController.ts`, `TaskHistoryDetail.tsx`,
  `WorkflowWorkspace.tsx`, `AssetVideoBatchWorkspace.tsx`
- `GenerationBatchItemRequest` also omits it
- `PromptLibraryCreateRequest` (`src/types/prompt.ts`) has no `modelVersionId`
- `addPromptLibraryVersion(projectId, promptId, text)` — text only
- `PromptLibraryPanel.tsx` create/version saves never pass a model

So the 034 columns stay NULL for the product UI path.

### 5.3 Prompt Studio history is project-wide, not prompt-linked — OPEN (deferred)

`PromptStudio.tsx` loads `taskHistoryPage({ projectId, filter: "ALL", ... })`
and labels it contextual. There is still no `prompt_version_id` on
`generation_snapshots`. Shot-stage prompts may carry `prompt_version_id`
(migrations 010/019); that is production-shot context, not Prompt Studio
reverse lookup. DEV-125 `PROMPT_GENERATION_LINK=DEFER` still applies.

### 5.4 Model/Tool typed client still mostly read-only — OPEN

Confirmed against current `tauriClient.ts`:

| Domain | Client reads | Client writes |
| --- | --- | --- |
| Model | `listModels`, `getModel`, `listModelVersions`, `getCurrentModelVersion`, `getModelVersion` | none (`model_create` / `model_update` / `model_version_create` exist in Rust) |
| Tool | `listTools`, `listToolInstances`, `listToolVersions`, `listToolCapabilities` | none (`tool_create`, `tool_instance_create`, `tool_instance_record_health`, `tool_version_create`, `tool_capability_create` exist in Rust) |

`LocalToolHub.tsx` only imports the list functions.

### 5.5 Asset read path fencing weaker than delete — OPEN (129-D did not change it)

| Path | Fence |
| --- | --- |
| Delete | `asset_deletion_service.rs` loads `project_root`; `FileSystemAssetStore::validate_delete_paths` / `validated_delete_path` **canonicalize** and require `canonical.starts_with(root)`; symlinks rejected |
| Image / thumbnail read | `asset_query_service.rs` `read_image` / `read_thumbnail`: project_id membership, then `asset_store.read(storage_path)` — **no project-root canonical fence** (`asset_store.rs` `read` is `fs::read`) |
| Video / audio media | `media_protocol.rs`: project_id + **lexical** `storage_path.starts_with(project_root)` — stronger than image read, weaker than delete canonicalize |

129-D only closed a restore host-path fallback in `project_backup.rs`. It did
not harden asset reads.

---

## 6. Live / product-owner gates (not a code PASS)

Treat like prior GPU live validation (`docs/DEV_023_ORCHESTRATOR_LIVE_VALIDATION.md`).
Unit tests here are not a substitute.

| Gate | Why it stays live |
| --- | --- |
| GPU / ComfyUI generation (including after restore) | Needs local Comfy/GPU. Serial overlap in §5.1 is unproven on hardware. |
| Product-scale archive (~100 projects / 10k assets / 50k versions / 100k relations) | 129-D bounded probe is 120/120/200 / 1 project |
| Desktop inspect → restore UX on a real `.aiarchive` | Dialog filters, inspection TTL, human preview |
| Restored tool instances remain `UNKNOWN` until the owner probes | Restore never auto-heals |
| Missing / offline media relink on moved drives | Beyond package remap |

---

## 7. P2 / P3 debt (not READY blockers by themselves)

The §5 blockers are P1 for the claimed loops. Remaining debt:

### P2

- **UI consistency:** Prompt Studio mixes English section titles (`Used Generations`, `Generated Assets`, `Model Versions`, `Tools`) with Chinese chrome (`PromptStudio.tsx`). Tool Hub / Model registry have no personal write UX. Generated assets often show “未建立版本记录”.
- **Asset read fencing** (§5.5) — security/correctness hygiene, not loop completeness.
- **Archive wayfinding:** backup lives on Project workspace buttons, not a first-run production story (continues BK-113-01).
- **Cross-module search / lineage query cost:** still multiple reads/joins (DATA-INT-001 from DEV-127). Measure real libraries before new indexes.

### P3

- Model/Tool client write wrappers (only after product wants registration UX; do not auto-probe).
- PromptVersion → Generation reverse lookup (still deferred).
- Archive catalog / browser / auto-backup / cloud (explicit non-goals for 129/130).
- Diagnostic vocabulary (FE-113-03) still visible.
- Snapshot path layout `snapshot/…` relocation (deferred in 129-C).

### Migration inventory (001–036)

Additive, backward-compatible. No 037. No Archive tables.

| Range | Role |
| --- | --- |
| 001–032 | v1.x production core: tasks, queue, shots, prompt library, reviews, structure, workflows, recipe archive, handoffs |
| 033 | `asset_versions`, `asset_relations` |
| 034 | `models`, `model_versions`; optional `model_version_id` on `prompt_versions` and `generation_snapshots` |
| 035 | `tools`, `tool_instances`, `tool_versions`, `tool_capabilities` |
| 036 | `generation_tool_usages`, `generation_asset_versions` |

v2 Personal Edition data is 033–036 plus backup format v19. Historical 001–032
remain the Production Core schema. Archive persistence is the logical backup
document, not a new migration.

---

## 8. What this review does **not** claim

- It does not authorize Comfy serial, provenance write-path, or `modelVersionId` UI work (that would be a later, separately approved task — not DEV-131 started here).
- It does not mark GPU/Comfy as PASS.
- It does not treat empty lineage UI as proof that lineage is captured.
- It does not treat backup v19 as an Archive catalog.

---

## 9. Closeout

```text
V2_PERSONAL_EDITION_READY=NO
BLOCKERS=COMFY_SERIAL; PROVENANCE_WRITE_PATH; MODEL_VERSION_ID_UI
ARCHIVE_MVP=YES
LIVE_GATES=GPU_COMFY_AFTER_RESTORE; PRODUCT_SCALE_ARCHIVE; DESKTOP_INSPECT_RESTORE_UX; RESTORED_TOOL_UNKNOWN; MISSING_MEDIA_RELINK
DEV_131_STARTED=NO
PUSHED=NO
```

Reviewed at `5711ba8` (`test(v2): harden archive integrity and recovery`) on
`feat/dev-129-b-archive-data-foundation`.
