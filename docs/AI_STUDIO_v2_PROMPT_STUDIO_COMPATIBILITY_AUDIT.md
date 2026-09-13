# AI Studio v2 Prompt Studio Compatibility Audit

```text
TASK=DEV-124-A
STATUS=COMPATIBILITY_AUDIT
BASELINE=AI_STUDIO_v1.3.1_STABLE
CODE_CHANGE=NO
DATABASE_CHANGE=NO
UI_CHANGE=NO
PROMPT_FEATURE_IMPLEMENTATION=NO
```

## 1. Audit conclusion

Prompt Studio can be added incrementally, but it must extend the existing Prompt
Library and connect to the existing production provenance path. The repository
already has authoritative Prompt and Reference capabilities. It does not yet
have a canonical Model registry, a standalone Generation entity, or a standalone
Result entity.

The implementation boundary is therefore:

```text
Existing Prompt Library
        +
Existing Task / GenerationSnapshot / output-asset path
        +
Existing ReferenceAnchor / ReferenceSet / asset bindings
        +
One future canonical model metadata authority, only if MVP requires it
```

No second Prompt system, executor, queue, task model, or reference library
should be introduced.

## 2. Existing Prompt authority

### 2.1 Inventory

| Capability | Current state | Evidence |
| --- | --- | --- |
| Prompt model | Exists | `prompt_entries` and `PromptEntryView` |
| Prompt entry | Exists | `prompt_entries`, project-scoped |
| Template/snippet | Partial but supported | `prompt_entries.kind` is `prompt` or `snippet` |
| Prompt version | Exists | `prompt_versions`, append-version service operation |
| Prompt parameters | Not a Prompt Library entity | Generation recipe fields are stored separately from prompt text |
| Model metadata | Not present in Prompt Library | No model/provider fields on prompt entry/version |
| Prompt references | Not present as Prompt Library records | References are supplied by existing asset/reference bindings |
| Prompt results | Not present as Prompt Library records | Results are represented by task output assets |

### 2.2 Current implementation

The existing authority is the Prompt Library:

- Migration: `src-tauri/migrations/009_prompt_library.sql`
- Domain/application: `src-tauri/src/application/prompt_library_service.rs`
- Repository port: `src-tauri/src/application/ports/prompt_library_repository.rs`
- SQL repository: `src-tauri/src/infrastructure/database/repositories/prompt_library.rs`
- Commands: `src-tauri/src/commands/prompt_library.rs`
- Frontend types: `src/types/prompt.ts`
- Typed client: `src/services/tauriClient.ts`
- Existing UI: `src/features/prompts/PromptLibraryPanel.tsx`

An entry owns project-scoped metadata such as name, kind, and tags. Its text is
stored in `prompt_versions`; creating a new version appends a new version number
instead of overwriting historical text. The service supports list, get, create,
append version, metadata update, and delete operations.

The current implementation is sufficient as the identity and text-version
authority. It is not yet a complete Prompt Studio record because model,
provider-specific parameters, explicit reference roles, and result/provenance
links are not part of the Prompt Library view or schema.

### 2.3 Consumers

The Prompt Library is already consumed by more than one frontend surface:

- `PromptLibraryPanel` inside `GenerationStudio`
- prompt selection and snapshot loading in `ShotInspector` / `ShotWorkspace`
- prompt-library summaries in `CreationDashboard`
- prompt-related counts in project command-center types

DEV-124-B must preserve these consumers and their project scope.

## 3. Existing Generation authority

### 3.1 Inventory

| Capability | Current state | Authority |
| --- | --- | --- |
| Generation record | No standalone `generations` table | Existing Task execution record |
| Input snapshot | Exists | `generation_snapshots` |
| Workflow/recipe identity | Exists | Exact `workflow_version_id` + `recipe_id` on Task/definition |
| Generation status | Exists | Task status, progress, timings, errors |
| Model information | Partial runtime metadata | Workflow/recipe definitions and external runtime data |
| Output/result record | No standalone `results` table | `assets` linked through `task_output_assets` |
| Retry/idempotency lineage | Exists | Task idempotency, attempt, and parent-task fields |

### 3.2 Current implementation

Generation is an execution concern owned by the existing production path:

- `src-tauri/src/application/generation_service.rs`
- `src-tauri/src/application/generation_catalog_service.rs`
- `src-tauri/src/domain/generation_snapshot.rs`
- `src-tauri/src/application/ports/generation_snapshot_repository.rs`
- `src-tauri/src/application/ports/generation_definition_repository.rs`
- `src-tauri/src/commands/generation.rs`
- `src-tauri/src/commands/catalog.rs`

The relevant database authorities are:

- `tasks`: project, exact workflow-version/recipe identity, status, timing,
  errors, provenance, retry, and idempotency information.
- `generation_snapshots`: the workflow, recipe, user-input, and resolved-input
  snapshot for a task.
- `task_output_assets`: the mapping from a completed task to output Assets.
- `assets`: the durable media and metadata records used by the rest of the app.

The existing `GenerationService` prepares typed inputs, including Asset IDs for
image, video, and audio inputs, and records resolved input snapshots. It also
owns the existing queue/execution boundary. Prompt Studio must not create a
parallel generation executor or bypass Queue Start.

The exact workflow identity remains the pair:

```text
workflowVersionId + recipeId
```

Names must not be used to infer that identity.

## 4. Existing Model authority

### 4.1 Inventory

| Capability | Current state |
| --- | --- |
| Model entity | Does not exist |
| Model registry table | Does not exist |
| Provider registry | Does not exist as a general Prompt Studio authority |
| Model version entity | Does not exist |
| Model repository/service/commands | Does not exist |
| Frontend model registry types/views | Does not exist |

Model and provider values may occur in workflow/recipe definitions, compiled
workflow JSON, external ComfyUI/runtime metadata, or error messages. These are
observed execution metadata, not a reusable, canonical Model registry.

`provider_kind`, `provider_model`, and `provider_metadata_json` on the script
draft foundation are scoped to script-draft provider metadata. They do not
constitute a general Model authority and should not be duplicated or treated as
the Prompt Studio registry.

### 4.2 Implication

Model metadata is the largest Prompt Studio foundation gap. If the MVP needs a
reusable model selector, DEV-124-B should design one canonical, project-safe
model metadata authority with additive migration. It should record external
tool/model identity and parameters without claiming to own or execute the
external model. A collection of provider-specific registries would create the
same authority problem the audit is intended to prevent.

## 5. Existing Reference authority

References already have explicit, project-scoped authorities at different
levels of the product:

| Reference capability | Current authority | Purpose |
| --- | --- | --- |
| Named character/scene/prop/style anchor | `reference_anchors` + `reference_anchor_assets` | Reusable named anchor backed by Assets |
| Ordered consistency/reference bundle | `reference_sets` + `reference_set_items` | Grouped Assets with role, order, and primary status |
| Shot reference input | `shot_reference_assets` | Stage-specific shot inputs |
| Generation reference input | `ReferenceManifest` and typed Asset inputs | Explicit Asset IDs passed into generation |
| Asset-to-asset relationship | `asset_relations` | Typed relation between Assets, not a Prompt input registry |

Evidence includes:

- `src-tauri/migrations/020_reference_anchors.sql`
- `src-tauri/migrations/022_consistency_profiles_and_reference_sets.sql`
- `src-tauri/migrations/010_shot_production.sql`
- `src-tauri/migrations/033_asset_library_data_layer.sql`
- `src-tauri/src/domain/reference_anchor.rs`
- `src-tauri/src/application/reference_anchor_service.rs`
- `src-tauri/src/domain/consistency/reference_set.rs`
- `src-tauri/src/application/reference_set_service.rs`
- `src-tauri/src/application/generation_service.rs`

The Reference authority is therefore not a missing generic `references` table.
Prompt Studio should select and preserve existing Asset/AssetVersion IDs and,
where relevant, existing ReferenceAnchor or ReferenceSet IDs. A future Prompt
Studio snapshot may preserve role, order, and checksum context, but it must not
replace the existing anchor/set/shot bindings.

## 6. Database audit

### 6.1 Relevant existing tables

| Existing table | Reuse in Prompt Studio |
| --- | --- |
| `projects` | Project ownership and isolation |
| `workflows`, `workflow_versions`, `recipes` | Exact generation definition identity |
| `prompt_entries`, `prompt_versions` | Prompt identity, metadata, and text history |
| `tasks` | Generation execution/history authority |
| `generation_snapshots` | Immutable task input/provenance snapshot |
| `task_output_assets` | Generation-to-result mapping |
| `assets`, `asset_versions`, `asset_relations` | Media, version, and asset relationship authority |
| `reference_anchors`, `reference_anchor_assets` | Named reference anchors |
| `reference_sets`, `reference_set_items` | Ordered reference collections |
| `shots`, `shot_reference_assets`, `shot_generation_links` | Shot-level references and generation links |

The repository also contains production telemetry, consistency-profile, and
asset-video-prompt tables. They remain valid authorities for their existing
bounded contexts but should not be repurposed as a second Prompt Studio core.

### 6.2 Missing or partial capabilities

The following generic tables/entities were not found:

```text
models
generations
results
references
```

Their absence does not mean all four should be created. The current design can
reuse Task, GenerationSnapshot, Asset, output mappings, ReferenceAnchor, and
ReferenceSet. New schema is justified only for a concrete Prompt Studio gap,
and must be additive, project-scoped, indexed, and migration-safe.

Likely future gaps for DEV-124-B are:

- provider/model metadata that is reusable and queryable;
- provider-specific parameter snapshots associated with a Prompt version or
  generation attempt;
- explicit Prompt-version-to-reference and Prompt-version-to-result links,
  if existing task snapshots and Asset usages cannot provide the required
  explainability;
- a stable provenance snapshot that does not reinterpret historical Task data.

DEV-124-B must first decide whether these are fields on the existing authority
or small additive sidecar entities. It must not introduce generic tables merely
to mirror existing tables.

## 7. Rust backend audit

### 7.1 Existing extension points

**Prompt:**

- `application/prompt_library_service.rs` — validation and Prompt Library use
  cases.
- `application/ports/prompt_library_repository.rs` — persistence port.
- `infrastructure/database/repositories/prompt_library.rs` — SQL queries.
- `commands/prompt_library.rs` — typed Tauri command boundary.

**Generation:**

- `application/generation_service.rs` — admission, input preparation,
  snapshots, execution, output import, and queue boundary.
- `application/generation_catalog_service.rs` — workflow/recipe catalog.
- `domain/generation_snapshot.rs` — input/provenance snapshot validation.
- `application/ports/generation_snapshot_repository.rs` and
  `generation_definition_repository.rs` — repository ports.
- task history/query services — durable execution and output views.

**Reference:**

- reference-anchor domain/service/port/repository/commands;
- consistency reference-set domain/service/port/repository/commands;
- existing generation input preparation and reference manifest handling.

**Model:**

No existing Model domain, repository, service, or command was found. Do not
hide a new registry inside the workflow catalog or Prompt Library without
deciding which one is the canonical authority.

### 7.2 DEV-124-B insertion boundary

The lowest-risk implementation boundary is to extend the Prompt Library
application/repository path only for Prompt Studio metadata that the MVP truly
needs, and to reuse existing services for generation, assets, and references.
Any new model metadata service should be a separate, single authority with a
small repository port rather than a second Prompt or generation service.

Commands should remain typed Tauri commands and frontend calls should continue
through `src/services/tauriClient.ts` and `src/services/ipc.ts`. Persistence
must continue through repository ports; direct database writes from commands or
components are out of bounds.

## 8. Frontend audit

### 8.1 Current surfaces

There is no standalone `prompt-studio` workspace or route. The current Prompt
surface is embedded in the existing Generation Studio:

- `src/features/prompts/PromptLibraryPanel.tsx` — list, filters, detail,
  version history, diff, metadata operations, and apply-to-Studio behavior.
- `src/features/studio/GenerationStudio.tsx` — current host and generation
  integration.
- `src/features/shots/ShotWorkspace.tsx` and `ShotInspector.tsx` — project-
  scoped prompt selection and snapshot loading.
- `src/features/production/CreationDashboard.tsx` — Prompt Library summary.
- `src/app/App.tsx` — workspace routing; no Prompt Studio route today.

The Asset workspace separately owns Asset, reference-anchor, and reference-set
surfaces. Task history and result panels own execution/output display.

### 8.2 Typed transport and types

The current typed path is:

```text
components/features
  -> src/services/tauriClient.ts
  -> src/services/ipc.ts
  -> Tauri commands
```

Relevant frontend types are in:

- `src/types/prompt.ts`
- `src/types/generation.ts`
- `src/types/task.ts`
- `src/types/history.ts`
- `src/types/referenceAnchor.ts`
- `src/types/consistency.ts`

No frontend model registry, generation registry, result registry, or dedicated
Prompt Studio view currently exists. DEV-124-B/C must preserve the typed
transport and existing Prompt Library behavior rather than introduce raw
`invoke` calls or a parallel store.

## 9. Authority decision

The unique authority for each concept is:

```text
Prompt=
  existing prompt_entries + prompt_versions,
  exposed through PromptLibraryService

Generation=
  existing Task + GenerationSnapshot + GenerationService;
  production execution remains behind the existing Queue boundary

Model=
  no canonical registry today;
  current observed values remain workflow/recipe/runtime metadata until one
  future registry is explicitly introduced

Reference=
  existing ReferenceAnchor + ReferenceSet + shot/generation Asset bindings

Result=
  existing Asset + task_output_assets mapping;
  no standalone Result entity today
```

This decision is intentionally asymmetric: existing bounded authorities are
reused, while the missing Model authority is acknowledged rather than silently
duplicated. The audit rejects:

- a second Prompt table or frontend store;
- a Prompt Studio queue or executor;
- a second generic Reference Library;
- a result table that mirrors Assets without a clear ownership boundary;
- model registries hidden separately in each provider integration.

## 10. Prompt Studio MVP boundary

### 10.1 In scope

The first Prompt Studio foundation may cover:

- Prompt metadata: name, kind, tags, project ownership, description if needed;
- reusable template text and immutable historical versions;
- model/provider values recorded as explicit metadata;
- provider-specific parameters recorded as structured, versioned values;
- references selected by existing Asset/AssetVersion IDs and, when applicable,
  ReferenceAnchor/ReferenceSet IDs;
- result links to existing Task, GenerationSnapshot, Asset, and
  `task_output_assets` records;
- provenance sufficient to explain which Prompt version, model metadata,
  parameters, and references were used;
- basic read/record behavior through the existing typed transport.

The UI may later expose these pieces as Prompt metadata, template/version,
model, parameters, references, and results panels. It does not need a complex
visual editor for the foundation.

### 10.2 Explicitly out of scope

- AI prompt optimization or AI tagging;
- automatic generation or automatic import;
- Agent orchestration or auto-decision;
- a new queue, executor, task, retry, or review system;
- replacing ComfyUI, TTS, music, or other external tools;
- cloud synchronization, multi-user access, permissions, or SaaS;
- vector search or semantic search;
- a generic reference database that duplicates Anchors and Reference Sets;
- destructive reinterpretation of historical v1.3.1 data.

### 10.3 Compatibility and migration guardrails

When implementation starts, migration must be additive only:

1. Preserve all existing `Project`, `Shot`, `Task`, Queue, Review, Asset,
   Prompt, and reference IDs.
2. Keep existing `prompt_entries` / `prompt_versions` behavior compatible with
   current Generation Studio, Shot Inspector, and dashboard consumers.
3. Keep project ownership and project-isolation predicates on every new
   project-owned record and query.
4. Keep the exact `workflowVersionId + recipeId` pair for generation identity.
5. Preserve Task and GenerationSnapshot history; do not backfill by guessing
   model or Prompt identity from names.
6. Prefer explicit foreign keys, uniqueness rules, and indexes over flexible
   JSON-only lookup where the relation is authoritative.
7. Treat provider-specific parameters as opaque/versioned metadata until a
   stable cross-provider contract is proven.

## 11. Implementation readiness

```text
PROMPT_STUDIO_READY=YES
IMPLEMENTATION_RISK=MEDIUM
RECOMMEND_NEXT_STEP=DEV-124-B_PROMPT_STUDIO_FOUNDATION
```

`PROMPT_STUDIO_READY=YES` means the existing authorities and the incremental
boundary are clear enough to plan the foundation. It does not mean the Prompt
Studio feature is implemented. Risk is **MEDIUM** because Prompt and Reference
capabilities are reusable, but Model metadata, parameter snapshots, and
Prompt-to-generation/result explainability still require a careful additive
design.

## 12. Audit checklist

```text
EXISTING_AUTHORITY_AUDIT=PASS
DATABASE_AUDIT=PASS
BACKEND_AUDIT=PASS
FRONTEND_AUDIT=PASS
MIGRATION_PREPARATION=READY
DUAL_PROMPT_SYSTEM=NO
SECOND_GENERATION_EXECUTOR=NO
SECOND_REFERENCE_LIBRARY=NO
CODE_CHANGED=NO
DATABASE_CHANGED=NO
UI_CHANGED=NO
```

## 13. Final recommendation

Proceed to DEV-124-B only as a foundation/design-to-implementation step. Start
by specifying the smallest additive representation for model metadata,
parameter snapshots, and Prompt-version provenance links. Keep Prompt Library,
GenerationService/Queue, Asset Library, and ReferenceAnchor/ReferenceSet as the
existing authorities. Do not start a new generation or reference subsystem.
