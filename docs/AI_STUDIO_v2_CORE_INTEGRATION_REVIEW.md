# AI Studio v2 Core Integration Review

```text
TASK=DEV-127
BASELINE_SHA=6054ee10d248d0b3579e71474cc3bedc6e61e335
VERSION=1.3.1_STABLE
SCOPE=DOCUMENTATION_ONLY
CODE_CHANGE=NO
DATABASE_CHANGE=NO
SCHEMA_CHANGE=NO
UI_CHANGE=NO
```

## 1. Review conclusion

The three v2 modules have usable individual boundaries:

```text
Production Core   = validated execution and review boundary
Asset Library     = project-scoped asset catalog and history
Prompt Studio     = project-scoped prompt/model provenance view
Local Tool Hub    = personal/local tool metadata registry
```

The modules can be used together for an explicit personal workflow, but the
full `Tool -> Model -> Prompt -> Generation -> Result Asset` chain is not yet
persisted as one complete, queryable provenance chain. In particular, the
current Tool Hub records observations about local tools but does not record
which tool instance/version was used by a specific generation. Prompt-to-
generation reverse lookup and direct generation-to-asset-version lineage are
also not first-class relationships.

Therefore the module foundations are ready, while the system-level v2 core
integration gate remains open:

```text
V2_CORE_READY=NO
OVERALL_SCORE=8.0/10
NEXT_MAJOR_MODULE=Cross-module Generation Provenance & Lineage Hardening
```

This is an integration finding, not an authorization to change Queue, Task,
Review, schema, or UI in DEV-127.

## 2. Overall architecture

| Module | Current authority | Scope and responsibility | Integration status |
| --- | --- | --- | --- |
| Production Core | Existing Project, Shot, Task, Queue, Review services and repositories | Owns production preparation, Queue Start admission, execution monitoring, review, and rework | Stable; Queue Start remains the only production execution gate |
| Asset Library | Existing `assets` authority plus the additive `asset_versions` and `asset_relations` data layer | Owns project assets, tags, previews, immutable asset history, and explicit asset-to-asset relations | Hardened and project-isolated |
| Prompt Studio | Existing `prompt_entries` / `prompt_versions` Prompt Library plus `models` / `model_versions` | Owns reusable prompt identity, prompt history, model metadata, model versions, and read-only provenance presentation | Hardened; no second Prompt or Generation system |
| Local Tool Hub | Additive `tools`, `tool_instances`, `tool_versions`, and `tool_capabilities` registry | Owns personal/local tool inventory and explicit last-known health observations | MVP; metadata only, not a runtime or executor |

The boundaries are intentionally asymmetric:

1. Production Core remains the only place that admits and executes production
   work. Asset Library, Prompt Studio, and Tool Hub cannot submit work or
   create a competing queue.
2. Asset Library catalogs the durable result and reference assets. It does not
   replace the existing task output or reference authorities.
3. Prompt Studio provides reusable prompt and model context. It does not
   rewrite prompts, optimize them with AI, or auto-generate a task.
4. Tool Hub describes local tools. `AppSettings` / `JsonSettingsStore` remain
   the ComfyUI configuration authority and `ComfyService` / its adapter remain
   the ComfyUI runtime and execution authority.
5. Tool Hub is personal/local inventory and is intentionally not a
   project-owned table. A future project-to-tool use must use an explicit
   stable Tool or ToolInstance identity without changing execution ownership.

## 3. Provenance chain review

### 3.1 Authority and hop inventory

| Chain element or hop | Existing evidence | Result |
| --- | --- | --- |
| Tool | `tools`, `tool_instances`, `tool_versions`, `tool_capabilities`; Rust `ToolService` and typed Tool Hub reads | **EXISTS as metadata**. There is no persisted usage record tying a ToolInstance or ToolVersion to a generation. |
| Tool -> Model | Tool capabilities and Model metadata are maintained separately; no typed foreign key or usage relation | **MISSING**. Do not infer this link from names, providers, or free-form metadata. |
| Model | `models` and `model_versions`; Rust `ModelService` and repositories | **EXISTS as canonical model registry**. ModelVersion is append-oriented and non-destructive. |
| Model -> Prompt | `prompt_versions.model_version_id` references `model_versions.id` | **EXISTS** as an optional exact PromptVersion-to-ModelVersion link. |
| Prompt | `prompt_entries` and `prompt_versions`; `PromptLibraryService` and Prompt Studio | **EXISTS** as the canonical reusable Prompt authority. Production-specific prompt text remains in its owning historical context. |
| Prompt -> Generation | Shot stage prompt bindings and PromptVersion model provenance exist; no `prompt_version_id` on `generation_snapshots` and no standalone PromptGenerationLink | **PARTIAL / DEFERRED**. Current Task and snapshot history is usable, but exact PromptVersion reverse filtering is not first-class. |
| Generation | Existing `tasks`, one-per-task `generation_snapshots`, GenerationService, and task history | **EXISTS** as the production generation history authority; there is no standalone replacement `generations` table. |
| Generation -> Result Asset | `task_output_assets` maps task outputs to existing `assets`; `assets.source_task_id` also preserves source-task context | **EXISTS** for the Result Asset identity through the existing Production Core path. |
| Result Asset -> Asset Version | `asset_versions` preserves immutable asset revisions, location, checksum, and metadata snapshots | **INDIRECT**. The version belongs to the result Asset, but the schema has no explicit generation snapshot/task-output reference on each AssetVersion. |

### 3.2 Chain assessment

The practical chain is currently:

```text
Tool metadata (manual observation only)
        ?
ModelVersion
        ↓
PromptVersion (optional model link)
        ?
Task + GenerationSnapshot
        ↓
task_output_assets
        ↓
Asset
        ↓
AssetVersion history
```

The question marks are deliberate boundaries rather than runtime failures:

- A registered Tool is not proof that a generation used that tool. The
  existing ComfyUI service may execute work while the generic Tool Hub only
  stores user-maintained metadata.
- A PromptVersion can be selected in production context, but the current
  generation snapshot does not carry a canonical PromptVersion foreign key.
  DEV-125 correctly deferred a standalone `PromptGenerationLink` until an
  exact filtering requirement is demonstrated.
- A result Asset is mapped to a Task, and its AssetVersion history is
  available, but a direct “this generation created this exact AssetVersion”
  link is not yet a first-class query relation.

The chain is therefore **explicitly composable and partially closed**, not a
fully automatic or fully persisted end-to-end lineage graph. This distinction
must remain visible in product claims and in any future release gate.

## 4. Data authority review

The following authority decisions prevent parallel domain systems:

| Domain | Unique authority | Boundary decision |
| --- | --- | --- |
| Asset | Existing `assets` plus the existing Asset repositories/services; `asset_versions` and `asset_relations` extend that same domain | No new Asset table or alternate Asset store. Large files remain filesystem-owned and metadata remains Rust/SQLite-owned. |
| Prompt | Existing `prompt_entries` / `prompt_versions` and `PromptLibraryService` | `shot_stage_prompts` and `asset_video_prompts` remain contextual or historical production text; they are not a second reusable Prompt Library. |
| Generation | Existing Production Core `Task` + `generation_snapshots` + `task_output_assets` and task history | No standalone Generation execution authority. Generation definitions/catalog records remain supporting input, not a replacement for task history. |
| Model | `models` + `model_versions` and `ModelService` | ModelVersion is the exact immutable model revision referenced by PromptVersion and GenerationSnapshot where available. |
| Tool | Generic personal registry `tools` + tool child tables for inventory; `AppSettings` / `ComfyService` for ComfyUI runtime behavior | These are separate responsibilities, not duplicate Tool systems: the registry describes tools, while the existing service configures and executes ComfyUI. |

The frontend uses the typed transport through `src/services/tauriClient.ts`
and existing feature services. Components do not access SQLite directly. Rust
persistence continues through repository ports and application services.

## 5. Project isolation review

| Data area | Isolation mechanism | Review result |
| --- | --- | --- |
| Asset | `assets.project_id`; Asset Library list/detail/version/relation operations carry `project_id`; version and relation services validate same-project endpoints | **PASS** |
| Prompt | `prompt_entries.project_id`; Prompt Library list/detail/version queries are project-scoped; reference anchors/sets validate project ownership | **PASS** |
| Generation | `tasks.project_id` is the production boundary; `generation_snapshots` belong to a Task; task history and output queries retain the Task project | **PASS** |
| Result Asset | Result rows are existing project-owned Assets reached through `task_output_assets`; project-scoped asset services reject cross-project access | **PASS** |
| Tool Hub | Tool inventory is intentionally personal/local rather than project-owned | **PASS by design**, provided any future use relation is explicit and does not widen project reads |

The existing DEV-123 Asset hardening and DEV-125 Prompt hardening records
provide the regression evidence for cross-project list, detail, version,
relation/reference, and result access. This review does not add or rerun tests.

No reviewed module creates a second project boundary or allows a display name
to substitute for a stable ID. The one intentional non-project-scoped area is
the local Tool Hub inventory, which is not production data.

## 6. Personal workflow review

The following AI video scenario is possible today without changing the
Production Core:

| Step | Existing authority and action | Closure assessment |
| --- | --- | --- |
| Project | Select the active Project; all project-owned reads use its stable ID | **Closed** |
| Reference Asset | Choose an Asset and, where applicable, a ReferenceAnchor/ReferenceSet or AssetVersion in that Project | **Closed**; reference identity and ordering remain with existing reference authorities |
| Prompt | Select a PromptEntry and exact PromptVersion from Prompt Studio; optionally retain its ModelVersion | **Closed for reuse**; direct generation reverse link is not yet first-class |
| Tool | Inspect the local Tool/ToolInstance, observed version, capability, and health state in Tool Hub | **Manual only**; Tool Hub does not select or invoke the tool |
| Generation | Prepare and submit through the existing Queue Start gate; Task and GenerationSnapshot capture the production record | **Closed for execution**; Tool and exact PromptVersion usage are not fully captured on the snapshot |
| Result | Existing task output mapping records the resulting Asset and its project | **Closed** |
| Asset Version | Append an immutable AssetVersion with metadata snapshot, location, checksum, and creation time | **Closed for history**; direct generation-to-version provenance remains indirect |

The workflow is thus suitable for personal use when the user is willing to
make the Tool and Prompt selections explicit outside the automatic execution
path. It is not yet suitable to claim that AI Studio can reconstruct every
historical generation's exact ToolInstance, ToolVersion, PromptVersion, and
AssetVersion without additional provenance work.

## 7. Technical debt and integration gaps

### P0 — Critical

```text
P0=NONE
```

No critical data-loss, cross-project access, second-executor, or Queue Start
boundary issue was identified in the reviewed code and completion records.

### P1 — Blocks the full integration gate

| ID | Gap | Impact | Recommended disposition |
| --- | --- | --- | --- |
| GEN-INT-001 | No first-class Tool / ToolInstance / ToolVersion usage provenance on a generation Task or GenerationSnapshot | A result cannot be proven to have been produced by the registered local tool instance and observed version | Address in a separately approved provenance-hardening phase; preserve Queue as the executor |
| GEN-INT-002 | AssetVersion has no explicit generation snapshot or task-output provenance field | Asset history can be inspected, but exact “generation produced this version” lineage requires indirect joins or external knowledge | Define the required query/read grain first, then extend the existing authorities additively if needed |

### P2 — Important, but not a release blocker for the current MVPs

| ID | Gap | Impact | Recommended disposition |
| --- | --- | --- | --- |
| PRM-INT-001 | PromptVersion-to-generation reverse lookup is deferred; no standalone `PromptGenerationLink` | Exact historical PromptVersion filtering is less direct | Keep deferred until a real personal workflow requires it; do not create a parallel provenance authority |
| TOOL-INT-001 | Tool Hub has metadata and read-only health observations but no explicit generation adapters or usage capture | Tool capability/version data can become stale and does not yet explain a completed run | Add small explicit adapters only with a future provenance scope; no probing, auto-run, or installer |
| DATA-INT-001 | Cross-module search and lineage queries are not optimized as one catalog | Larger personal libraries may require multiple reads and joins | Measure real collections before adding indexes or a search subsystem |
| STORE-INT-001 | Local media paths and observed tool locations can become unavailable or move | Metadata may outlive the path or external runtime | Keep missing/offline states, checksums, and explicit relink/maintenance operations in a later storage/archive scope |

### P3 — Quality-of-life follow-up

- Add cross-module navigation and a compact lineage view after the underlying
  provenance is authoritative.
- Improve Tool Hub registration and maintenance UX without adding process
  control.
- Add archive/export integration once the Project Archive module is approved.
- Keep cloud sync, multi-user permissions, SaaS, autonomous agents, and
  auto-decision behavior classified as **non-goals**, not technical debt.

## 8. V2 completion assessment

### Module-level assessment

```text
PRODUCTION_CORE=STABLE
ASSET_LIBRARY=HARDENED
PROMPT_STUDIO=HARDENED
LOCAL_TOOL_HUB=MVP
PROJECT_ISOLATION=PASS
AUTHORITY_BOUNDARIES=PASS
END_TO_END_PROVENANCE=PARTIAL
```

### Gate decision

```text
V2_CORE_READY=NO
OVERALL_SCORE=8.0/10
NEXT_MAJOR_MODULE=Cross-module Generation Provenance & Lineage Hardening
```

The score reflects strong individual module foundations and preserved safety
boundaries, reduced for the two P1 lineage gaps and the metadata-only Tool Hub
boundary. The next phase should be a narrow integration review and design
approval, not a new executor, workflow engine, or broad feature expansion.

The system-level gate can be reconsidered after there is an explicit,
backward-compatible answer to:

1. Which exact Tool/ToolInstance/ToolVersion was used for a generation?
2. Which exact PromptVersion and ModelVersion were used?
3. Which result Asset and AssetVersion were produced?
4. Can all of those facts be queried without guessing from names or rewriting
   historical Task records?

## 9. Evidence reviewed

- `docs/AI_STUDIO_v1.3.1_STABLE_BASELINE.md`
- `docs/AI_STUDIO_v2_ARCHITECTURE_PLAN.md`
- `docs/AI_STUDIO_v2_DATA_FOUNDATION_DESIGN.md`
- `docs/AI_STUDIO_v2_ASSET_LIBRARY_COMPATIBILITY_AUDIT.md`
- `docs/DEV_123_ASSET_LIBRARY_HARDENING.md`
- `docs/AI_STUDIO_v2_PROMPT_STUDIO_COMPATIBILITY_AUDIT.md`
- `docs/DEV_125_PROMPT_STUDIO_HARDENING.md`
- `docs/AI_STUDIO_v2_LOCAL_TOOL_HUB_COMPATIBILITY_AUDIT.md`
- `docs/DEV_126_B_TOOL_HUB_DATA_LAYER_IMPLEMENTATION.md`
- `docs/DEV_126_C_TOOL_HUB_FRONTEND_IMPLEMENTATION.md`
- `src-tauri/migrations/033_asset_library_data_layer.sql`
- `src-tauri/migrations/034_prompt_studio_model_foundation.sql`
- `src-tauri/migrations/035_local_tool_hub_data_foundation.sql`
- Existing Rust domain, repository, service, command, and typed frontend
  transport boundaries for Asset, Prompt, Model, GenerationSnapshot, and Tool.

## 10. Review status

```text
DOCUMENT_ONLY=YES
CODE_CHANGED=NO
DATABASE_CHANGED=NO
UI_CHANGED=NO
DEV_127=COMPLETE_AFTER_COMMIT
DEV_128_STARTED=NO
AUTO_NEXT_TASK=NO
```
