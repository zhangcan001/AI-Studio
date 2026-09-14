# AI Studio v2 Provenance & Lineage Audit

```text
TASK=DEV-128-A
BASELINE_SHA=4363a3d95d68c1cec9e90face708b7a8156e1ecb
VERSION=1.3.1_STABLE
SCOPE=COMPATIBILITY_AUDIT_ONLY
CODE_CHANGE=NO
DATABASE_CHANGE=NO
UI_CHANGE=NO
```

## 1. Audit conclusion

The existing system already has a durable generation anchor, model
provenance, result-to-asset mapping, and immutable asset versions. The missing
facts are relationships around those authorities, not new domain entities:

```text
Existing generation authority = Task + GenerationSnapshot
Existing result authority     = task_output_assets + Asset
Existing model authority      = Model + ModelVersion
Existing tool authority       = Tool + ToolInstance + ToolVersion
```

The minimum safe increment is two relation tables anchored on the existing
Task and output authorities:

```text
generation_tool_usages
generation_asset_versions
```

Do not add a `generations` table, a `results` table, a second Asset system, or
a generic polymorphic provenance table. Keep PromptVersion-to-generation
reverse lookup deferred as decided in DEV-125 until a concrete query need is
demonstrated.

```text
IMPLEMENTATION_READY=YES
IMPLEMENTATION_RISK=MEDIUM
```

`YES` means ready for a narrow, separately approved additive data-layer
implementation. It does not authorize implementation in DEV-128-A.

## 2. Existing generation audit

### 2.1 Task is the generation identity

`tasks` is the production generation record and already carries:

- `id` and `project_id`;
- exact `workflow_id`, `workflow_version_id`, and `recipe_id`;
- status, queue number, lifecycle timestamps, and task errors;
- retry/idempotency identity (`parent_task_id`, submission attempt, and key);
- immutable runtime provenance for the selected workflow/recipe/build;
- optional runtime telemetry, including `generation_execution_id`;
- `prompt_id`, which is the external runtime/Comfy submission prompt ID.

The last field must not be mistaken for a Prompt Library identity. It is
assigned during submission and is not a `prompt_entries.id` or
`prompt_versions.id`.

Task remains owned by Production Core. Queue Start is still the only execution
gate, and the provenance design must not move submission or scheduling into
Asset Library, Prompt Studio, or Tool Hub.

### 2.2 GenerationSnapshot is the immutable input snapshot

`generation_snapshots` has one row per Task and currently stores:

```text
task_id
workflow_json
recipe_yaml
user_inputs_json
resolved_inputs_json
model_version_id (optional)
created_at
```

`model_version_id` is the existing exact ModelVersion provenance added by
DEV-124-B. The snapshot does not currently store a Tool identity, ToolInstance
identity, ToolVersion identity, PromptVersion identity, or output AssetVersion
identity.

The JSON input fields are historical execution evidence, but they are not a
substitute for typed foreign-key relationships when the product must answer
an exact provenance query.

### 2.3 Result is already represented by existing output mappings

There is no standalone `results` table. The existing result path is:

```text
Task
  ↓
task_output_assets(task_id, output_id, ordinal, asset_id)
  ↓
assets(source_task_id, project_id, ...)
```

The Asset import/output collection path creates the project-owned Asset and
preserves the source Task. Review and production output references also reuse
the existing Asset identity. This mapping remains the Result authority.

## 3. Existing Asset lineage audit

### 3.1 Asset identity

The canonical `assets` record has a stable Asset ID, `project_id`, media
metadata, filesystem location/checksum, timestamps, and optional
`source_task_id`. Asset Library queries and detail operations are project
scoped.

`source_task_id` provides a useful generation-to-Asset shortcut, but it only
identifies the parent Task. It does not identify which AssetVersion was
created by that Task when an Asset has multiple revisions.

### 3.2 AssetVersion identity

DEV-122-B added `asset_versions` in the same Asset domain. It stores:

```text
id
project_id
asset_id
version_number
metadata_snapshot
location
checksum
created_at
```

Version numbers are append-only and historical versions are not overwritten.
The current table correctly preserves Asset history, but it has no
`task_id`, output key, or `generation_snapshot_id`.

### 3.3 Current result-to-version gap

The current lineage is therefore:

```text
Task
  ↓ task_output_assets
Result Asset
  ↓ asset_versions
AssetVersion history
```

The joins are understandable, but the exact result version is not a
first-class fact. A later user can create another AssetVersion, and the
database cannot answer from the version row alone which generation output
created it. This is the P1 gap identified in DEV-127.

AssetVersion must remain an extension of Asset, not a new result or media
authority.

## 4. Existing Tool usage audit

### 4.1 Tool Hub authority

The Local Tool Hub provides the canonical personal/local metadata registry:

```text
tools             = stable Tool identity
tool_instances    = local path/endpoint and explicit health observation
tool_versions     = append-only observed versions
tool_capabilities = user-maintained capability labels
```

`ToolHealthStatus` is limited to `AVAILABLE`, `MISSING`, and `UNKNOWN`.
Health records are caller-supplied observations; the Tool Hub does not probe,
start, stop, install, or execute a process.

### 4.2 ComfyUI boundary

`AppSettings` / `JsonSettingsStore` remain the ComfyUI configuration
authority. `ComfyService` and the Comfy adapter remain the runtime, queue,
submission, history, and output-collection authority.

The existing Comfy path does not currently emit a canonical `ToolId`,
`ToolInstanceId`, or `ToolVersionId` into a Task or GenerationSnapshot. A
Tool Hub row describing ComfyUI therefore does not prove that a particular
generation used that row. Inferring usage from an endpoint, display name, or
capability label would be unsafe and would fail for multiple local instances.

### 4.3 Tool-to-generation gap

There is no current relationship between:

```text
Tool / ToolInstance / ToolVersion
        and
Task / GenerationSnapshot
```

This is the second P1 gap. The missing fact is an explicit usage observation,
not a new executor or a Tool Hub ownership change.

There is also no need for a permanent Tool-to-Model registry relationship in
the MVP. A generation can record the exact ModelVersion in the existing
snapshot and the exact executor Tool usage in a separate relation. Those two
facts together explain which registered tool executed which model revision,
without asserting that a model is globally owned by one tool.

## 5. Missing relationship design

### 5.1 Options considered

| Option | Decision | Reason |
| --- | --- | --- |
| Add Tool IDs and output-version IDs as JSON fields on `generation_snapshots` | Reject | Multiple tools and multiple outputs become opaque, difficult to query, and weakly constrained. |
| Add `tool_id` directly to `tasks` | Reject | Task is the Production Core authority; a task may involve more than one tool and Tool Hub metadata is not Task ownership. |
| Create a generic polymorphic `generation_provenance` table | Reject | Nullable entity IDs would weaken foreign-key integrity and create a second provenance vocabulary. |
| Create a new `generations` or `results` entity | Reject | Existing Task, GenerationSnapshot, `task_output_assets`, and Asset authorities are sufficient. |
| Add `generation_tool_usages` | **Recommend** | A typed, append-oriented relation can support one or more explicit local tool observations per existing Task. |
| Add `generation_asset_versions` | **Recommend** | A typed relation can attach one exact AssetVersion to an existing task output key without replacing the Asset or result authority. |
| Add a standalone `PromptGenerationLink` now | Defer | DEV-125 found the current Task/snapshot path sufficient for MVP; exact PromptVersion reverse filtering has not yet been shown to be a required query. |

### 5.2 Recommended `generation_tool_usages`

This is a relation, not a new Generation entity. The proposed additive shape
is:

```text
generation_tool_usages
----------------------
id                    TEXT PRIMARY KEY
task_id               TEXT NOT NULL -> tasks.id
tool_id               TEXT NOT NULL -> tools.id
tool_instance_id      TEXT NULL     -> tool_instances.id
tool_version_id       TEXT NULL     -> tool_versions.id
role                  TEXT NOT NULL
observed_at           TEXT NOT NULL
metadata_json         TEXT NOT NULL
```

Recommended rules:

1. `task_id` is the existing generation anchor. Do not introduce
   `generation_id`.
2. `role` is a small explicit value such as `EXECUTOR`; additional roles must
   be justified by a real workflow rather than added speculatively.
3. `tool_instance_id` and `tool_version_id` are optional for legacy or
   partially observed runs, but when present they must belong to `tool_id`.
   This parent consistency is validated in the Rust service and tested at the
   repository boundary.
4. `observed_at` records when the usage fact was captured. It is not a health
   probe timestamp and does not start or control a process.
5. Do not backfill existing Tasks by guessing from ComfyUI endpoints or
   names. Legacy rows remain valid without a usage relation.
6. Add indexes by `task_id`, then by `tool_id` and observed time. Do not add a
   project column that can disagree with `tasks.project_id`; project scope is
   derived from the Task.

### 5.3 Recommended `generation_asset_versions`

This relation attaches a concrete version to the existing output identity:

```text
generation_asset_versions
-------------------------
id                    TEXT PRIMARY KEY
task_id               TEXT NOT NULL
output_id             TEXT NOT NULL
ordinal               INTEGER NOT NULL
asset_version_id      TEXT NOT NULL -> asset_versions.id
created_at            TEXT NOT NULL

FOREIGN KEY (task_id, output_id, ordinal)
  -> task_output_assets(task_id, output_id, ordinal)
```

Recommended rules:

1. The composite output key must already exist in `task_output_assets`; the
   relation cannot invent a Result.
2. The linked AssetVersion must belong to the `asset_id` in that output row.
   Rust service validation should enforce this cross-table invariant.
3. The linked AssetVersion and Task must belong to the same project. The
   service must verify this before insert; no name-based inference is allowed.
4. The MVP should allow one canonical AssetVersion per task output key. A
   unique constraint on `(task_id, output_id, ordinal)` keeps the result
   version deterministic and avoids a second “current result” concept.
5. Use `ON DELETE RESTRICT` for the provenance relation to avoid silently
   erasing the proof of a generation. Asset deletion inspection must report a
   blocking lineage reference before a future implementation permits removal.
6. Do not copy `asset_id` into this table unless a measured query need proves
   it necessary; derive it through the existing output mapping and validate
   it on write.

### 5.4 Prompt and Model boundary

The existing ModelVersion relationship is already adequate for the minimum
increment:

```text
generation_snapshots.model_version_id -> model_versions.id
```

Prompt remains deliberately bounded:

- reusable Prompt identity remains `prompt_entries` / `prompt_versions`;
- production stage bindings remain `shot_stage_prompts`;
- task submission `prompt_id` remains the external runtime ID;
- no new Prompt table or standalone PromptGenerationLink is added in this
  audit.

If exact PromptVersion-to-generation filtering becomes a real requirement,
extend the existing GenerationSnapshot/Task authority additively in a later
design. Do not infer a PromptVersion from prompt text or from the external
runtime prompt ID.

## 6. Authority review

```text
Asset       = assets + existing Asset services; AssetVersion extends Asset
Prompt      = prompt_entries + prompt_versions; stage text remains contextual
Generation  = tasks + generation_snapshots + existing task history
Result      = task_output_assets + assets
Model       = models + model_versions
Tool        = tools + tool_instances + tool_versions + capabilities
ComfyUI     = AppSettings/JsonSettingsStore + ComfyService/ComfyAdapter
```

The two proposed tables are edges between existing authorities. They must be
implemented behind the existing Rust repository ports and application
services, then exposed through the existing typed transport only if a later
read surface requires them. They must not become a new source of truth for
Task state, results, assets, prompts, models, or ComfyUI execution.

## 7. Additive migration plan

### 7.1 Migration shape

The next approved migration can be numbered after the current migration 035
(for example, migration 036) and should:

1. create `generation_tool_usages` with typed foreign keys and indexes;
2. create `generation_asset_versions` with the existing output composite key
   and `asset_versions` foreign key;
3. add uniqueness/check constraints for role, output identity, and non-empty
   metadata as appropriate;
4. leave every existing table and column intact;
5. run in the existing migration transaction and preserve old databases.

No `ALTER TABLE` is required for the minimum design. In particular, do not
rewrite `tasks`, `generation_snapshots`, `task_output_assets`, or `assets` to
move their ownership.

### 7.2 Compatibility and backfill policy

```text
ADDITIVE_ONLY=YES
DESTRUCTIVE_REWRITE=NO
LEGACY_BACKFILL=NO
GUESS_FROM_ENDPOINT=NO
```

Existing Tasks, snapshots, results, and Assets remain readable after the
migration. New relation rows are written only when the caller has an explicit
stable identity and the service validates its ownership. Historical runs with
unknown Tool or AssetVersion identity remain intentionally unlinked rather
than receiving guessed provenance.

### 7.3 Deletion policy

The implementation must audit existing Asset deletion inspection before
enabling the `ON DELETE RESTRICT` relationship. A referenced AssetVersion
should be reported as a lineage blocker; provenance should never disappear as
an incidental cascade. Task and output deletion behavior must likewise be
reviewed against the existing historical-record policy before any delete path
is changed.

## 8. Project isolation

The proposed relations should not duplicate project ownership. Isolation is
derived and checked as follows:

| Relation | Isolation rule |
| --- | --- |
| `generation_tool_usages` | Resolve `task_id` under the caller's `project_id`; Tool metadata is personal/local, but a usage row is visible only through its project-owned Task. |
| `generation_asset_versions` | Resolve the existing output row and AssetVersion; require Task project, output Asset project, and AssetVersion project to match. |
| Existing Task/Snapshot/Result | Preserve current project-scoped services and exact stable-ID lookups. |

There is no project-to-tool ownership implied by these relations. A Tool may
be used by multiple personal projects, while each usage event remains scoped
by its owning Task.

## 9. MVP implementation boundary for the next phase

### In scope

- additive migration for the two relation tables;
- Rust domain records and repository ports for the relations;
- service validation for Task, Tool, ToolInstance, ToolVersion, output-key,
  Asset, and AssetVersion consistency;
- append/query behavior with project-isolation tests;
- explicit generation-path capture when the caller supplies a Tool identity;
- exact result-version attachment after an output AssetVersion is persisted;
- deletion inspection tests proving provenance is not silently orphaned.

### Out of scope

```text
NEW_GENERATION_ENTITY=NO
NEW_RESULT_ENTITY=NO
NEW_ASSET_SYSTEM=NO
PROMPT_GENERATION_LINK=DEFERRED
QUEUE_REWRITE=NO
EXECUTOR_CHANGE=NO
COMFY_SERVICE_BEHAVIOR_CHANGE=NO
TOOL_PROBING=NO
PROCESS_CONTROL=NO
AUTO_BACKFILL=NO
UI_CHANGE=NO
AI_AGENT=NO
```

The next implementation must preserve the Queue Start gate and must not
pretend that a Tool Hub registration is an execution authorization.

## 10. Verification plan for implementation

The future data-layer implementation should add focused coverage for:

### Migration

- migrate a v1.3.1 database through the new migration;
- preserve Project, Task, GenerationSnapshot, output Asset, Prompt, Model,
  and existing Tool rows;
- verify both new tables and their indexes exist.

### Tool usage relation

- create and query a usage row for a Task;
- round-trip Tool, ToolInstance, and ToolVersion identity;
- reject an instance/version belonging to another Tool;
- reject a Task from another project at a project-scoped service boundary;
- retain a usage row without guessing when optional observation fields are
  absent;
- verify no process probe, start, stop, or Queue submission occurs.

### AssetVersion relation

- create and query the exact Task output-to-AssetVersion link;
- reject a missing output key;
- reject an AssetVersion from another Asset or project;
- reject a second canonical version for the same output key;
- verify deletion inspection reports the relation instead of silently dropping
  lineage;
- preserve all historical Task output and Asset rows.

### Regression

- existing GenerationSnapshot model provenance still round-trips;
- existing Asset Library and Prompt Studio project isolation remains intact;
- existing ComfyService and Queue/Task/Review behavior is unchanged.

## 11. Readiness assessment

```text
GENERATION_AUDIT=PASS_WITH_TOOL_AND_VERSION_GAPS
ASSET_LINEAGE_AUDIT=PASS_WITH_EXPLICIT_VERSION_LINK_GAP
TOOL_USAGE_AUDIT=PASS_WITH_NO_TASK_RELATION
MISSING_RELATIONSHIP=RECOMMEND generation_tool_usages + generation_asset_versions
AUTHORITY_REVIEW=PASS
MIGRATION_PLAN=READY_ADDITIVE_ONLY
IMPLEMENTATION_READY=YES
IMPLEMENTATION_RISK=MEDIUM
```

The medium risk is concentrated in cross-table consistency and deletion
semantics, not in the data model itself. The implementation should remain a
small data-layer change and should not be bundled with Tool Hub process
control, Prompt Studio redesign, Queue changes, or a new generation runtime.

## 12. Evidence reviewed

- `docs/AI_STUDIO_v2_CORE_INTEGRATION_REVIEW.md`
- `docs/DEV_123_ASSET_LIBRARY_HARDENING.md`
- `docs/DEV_125_PROMPT_STUDIO_HARDENING.md`
- `docs/DEV_126_B_TOOL_HUB_DATA_LAYER_IMPLEMENTATION.md`
- `docs/DEV_126_C_TOOL_HUB_FRONTEND_IMPLEMENTATION.md`
- `src-tauri/migrations/001_initial.sql`
- `src-tauri/migrations/004_video_outputs.sql`
- `src-tauri/migrations/009_prompt_library.sql`
- `src-tauri/migrations/019_shot_stage_prompts.sql`
- `src-tauri/migrations/020_reference_anchors.sql`
- `src-tauri/migrations/033_asset_library_data_layer.sql`
- `src-tauri/migrations/034_prompt_studio_model_foundation.sql`
- `src-tauri/migrations/035_local_tool_hub_data_foundation.sql`
- Existing Rust Task, GenerationSnapshot, Asset, Model, Tool, repository,
  service, Comfy adapter, and typed transport implementations.

## 13. Audit status

```text
DOCUMENT_ONLY=YES
CODE_CHANGED=NO
DATABASE_CHANGED=NO
UI_CHANGED=NO
DEV_128_A=COMPLETE_AFTER_COMMIT
DEV_128_B_STARTED=NO
AUTO_NEXT_TASK=NO
```
