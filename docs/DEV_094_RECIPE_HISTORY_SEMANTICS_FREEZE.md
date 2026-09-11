# DEV-094 Recipe History & Lineage Semantics Freeze

## 0. Scope and audit baseline

~~~text
TASK=DEV-094A
REPOSITORY=zhangcan001/AI-Studio
BRANCH=master
BASELINE=ff1ac3326f6d4eac41d8619c833eb57859abc54f

DEV-093=CLOSED
DEV-094A=PASS
PRODUCT_CODE_CHANGE=NO
DATABASE_CHANGE=NO
NEW_HISTORY_TABLE=NO
DATABASE_MIGRATION=NO

DEV-094_IMPLEMENTATION=NOT_STARTED
REMOTE_CI_REQUIRED_THIS_TASK=NO
AUTO_NEXT_TASK=NO
~~~

This document freezes the semantics and information architecture for a future Recipe History implementation. It does not implement a history UI, add IPC, change repositories, add migrations, or modify any product behavior.

The working tree contained only the pre-existing untracked .serena/ directory. It is local Serena metadata and is not part of this task.

## 1. Audit method

The audit used the repository and current source as the authority. The following schema and code were inspected:

- src-tauri/migrations/001_initial.sql
- src-tauri/migrations/003_presets.sql
- src-tauri/migrations/006_production_queue.sql
- src-tauri/migrations/008_organization.sql
- src-tauri/migrations/010_shot_production.sql
- src-tauri/migrations/014_workflow_benchmark.sql
- src-tauri/migrations/018_production_orchestrator.sql
- src-tauri/migrations/024_production_preparation_snapshots.sql
- src-tauri/migrations/027_project_workflow_bindings.sql
- src-tauri/migrations/028_workflow_registry_v2.sql
- src-tauri/migrations/030_workflow_recipe_promotions.sql
- src-tauri/migrations/031_workflow_recipe_archive_state.sql
- src-tauri/src/infrastructure/database/repositories/workflow_runtime.rs
- src-tauri/src/infrastructure/database/repositories/task_history.rs
- src-tauri/src/infrastructure/database/repositories/task.rs
- src-tauri/src/infrastructure/database/repositories/generation_snapshot.rs
- src-tauri/src/infrastructure/database/repositories/production_queue.rs
- src-tauri/src/infrastructure/database/repositories/preset.rs
- src-tauri/src/infrastructure/database/repositories/workflow_benchmark.rs
- src-tauri/src/infrastructure/database/repositories/shot.rs
- src-tauri/src/infrastructure/database/repositories/organization.rs
- src-tauri/src/infrastructure/database/repositories/project_workflow_binding.rs
- src-tauri/src/infrastructure/database/repositories/production_orchestrator.rs
- src-tauri/src/application/workflow_registry_service.rs
- src-tauri/src/application/workflow_workspace_query_service.rs
- src-tauri/src/application/task_history_service.rs
- src-tauri/src/application/task_query_service.rs
- src-tauri/src/application/workflow_benchmark_service.rs
- src-tauri/src/application/production_queue_service.rs
- src-tauri/src/application/production_orchestrator_service.rs
- src-tauri/src/application/project_workflow_binding_service.rs
- src-tauri/src/application/production_batch_runbook_service.rs

The prior decisions in docs/DEV_093_RECIPE_ARCHIVE_SEMANTICS_FREEZE.md and the implementation facts in docs/DEV_093_RECIPE_ARCHIVE_RESULT.md were used only as context; the persistence and query conclusions below are based on the current code and schema.

## 2. Frozen identity and three separate concepts

Every Recipe History reference is keyed by the exact composite identity:

~~~text
workflowVersionId + recipeId
~~~

A recipe ID without its workflow-version ID is not a valid historical identity.

Recipe History has three distinct layers:

| Layer | Authority | Meaning |
|---|---|---|
| A. Recipe definition lineage | workflow_versions + immutable recipes rows | Which immutable recipe definitions belong to a workflow version, their recipe version, YAML, schema version, SHA-256, and creation time. |
| B. Recipe lifecycle state | workflow_recipe_promotions + workflow_recipe_runtime_states | Whether the exact pair is promoted or archived at the current time. |
| C. Recipe usage history | Existing task, snapshot, queue, project, shot, benchmark, and production records | What was configured, queued, attempted, executed, or completed with the exact pair. |

These layers must never be collapsed into one mutable history state.

~~~text
RECIPE_HISTORY=DERIVED_EXISTING_FACTS
SECOND_HISTORY_STATE_SOURCE=NO
HISTORICAL_REFERENCE_IMMUTABILITY=YES
AUTO_REBIND=NO
NAME_GUESSING=NO
~~~

## 3. Persistence audit matrix

YES in the exact-pair column means both workflow_version_id and recipe_id are persisted by that source. INDIRECT means the pair is recovered through an existing immutable relation such as task_id, shot_id, experiment_id, run_id, or production_batch_item_id.

| Source | Exact workflow version ID? | Exact recipe ID? | Created / updated time | Project ID? | Task ID? | Status? | User-visible history value? |
|---|---:|---:|---|---:|---:|---:|---|
| recipes | YES | YES | created_at | NO | NO | NO | YES — immutable definition lineage |
| tasks | YES | YES | created_at, queued_at, started_at, finished_at | YES | SELF | YES | YES — authoritative execution attempts |
| generation_snapshots | INDIRECT via task_id | INDIRECT via task_id | created_at | INDIRECT via task | YES | NO | YES — frozen execution evidence, joined to task |
| production_batch_items | YES | YES | created_at, updated_at | INDIRECT via batch | Optional | YES | YES — planned/queued usage and batch status |
| presets | YES | YES | created_at, updated_at | YES | NO | No lifecycle status | YES — exact preset usage |
| project_templates | YES | YES | created_at, updated_at | NO — global table | NO | Derived availability | YES — exact reusable template reference |
| project_workflow_bindings | YES | YES | created_at, updated_at | YES | NO | No status column; availability is derived | YES — current/stale project binding |
| shot_stage_configs | YES | YES | updated_at | INDIRECT via shot | NO | No status | YES — configured shot stage |
| shot_generation_links | INDIRECT via linked task/item | INDIRECT via linked task/item | created_at | INDIRECT via shot/batch | YES or indirect via item | No link status | YES — shot/stage navigation evidence |
| benchmark_candidates | YES | YES | created_at | INDIRECT via experiment | Optional | No candidate status | YES — experiment participation |
| benchmark_runs | INDIRECT via candidate | INDIRECT via candidate | created_at, updated_at | INDIRECT via experiment | Optional | YES | YES — benchmark execution evidence |
| production_stages | Nullable YES | Nullable YES | created_at, updated_at, start/finish times | INDIRECT via run | NO | YES | YES — production run/stage reference |
| production_stage_items | INDIRECT via stage | INDIRECT via stage | created_at, updated_at | INDIRECT via run | YES or stage/batch item | YES | YES — stage-item execution navigation |
| production_preparation_snapshots | INDIRECT via batch item / frozen snapshot | INDIRECT via batch item / frozen snapshot | created_at | YES | Indirect | No top-level status | YES — frozen preparation evidence |
| production_run_templates | YES for Krea2 and/or H3 fields | YES for Krea2 and/or H3 fields | created_at, updated_at | YES | NO | No status | YES — reusable production configuration |
| workflow_runtime_artifacts | YES | YES | created_at | NO | NO | No lifecycle status | Technical lineage only |
| workflow_recipe_promotions | YES | YES | promoted_at | NO | NO | Promoted/not promoted | YES — current preference |
| workflow_recipe_runtime_states | YES | YES | updated_at, archived_at | NO | NO | Archived/active | YES — current availability |

The schema already has enough exact identity to derive the requested history. The sources do not justify a recipe_history table or an event-sourcing table.

## 4. Recipe definition lineage

The recipes table is immutable definition storage:

~~~text
recipe_id
workflow_version_id
version
schema_version
recipe_yaml
recipe_sha256
created_at
~~~

~~~text
RECIPE_DEFINITION_IMMUTABLE=YES
EDIT_OLD_RECIPE_HISTORY=NO
REWRITE_RECIPE_VERSION=NO
REWRITE_RECIPE_SHA=NO
~~~

A new Recipe created by Workflow Parameter Exposure is a new immutable definition row. It enters the lineage as a new exact recipe identity; it does not edit the old row.

The current schema has no explicit parent-recipe column. Therefore lineage means:

1. Same workflow_version_id.
2. Ordered immutable recipe definitions by the existing recipe-version comparator.
3. Exact recipe_id, version, and recipe_sha256 retained for every row.

“Previous recipe” means the immediately preceding recipe in that ordered lineage. It must never be inferred from a name, package, latest row, or approximate YAML similarity.

## 5. Current lifecycle labels

The future history projection must display these facts independently:

| Label | Authority | Meaning |
|---|---|---|
| Current version | workflows.current_version_id | The workflow version selected as current. |
| Current recipe | Existing Registry derivation | A non-archived promoted recipe in the current version, otherwise the newest non-archived recipe. |
| Promoted | workflow_recipe_promotions exact pair | The recipe selected as implicit preference for its version. |
| Archived | workflow_recipe_runtime_states exact pair | Runtime availability state; missing state row means active. |
| Historical | Any persisted usage/reference row | A past or configured fact, independent of current availability. |

currentRecipe is a current selectable projection. It is not a substitute for the promoted, archived, or historical labels.

The current Registry already keeps recipe rows and lifecycle metadata separate. Recipe History must preserve that separation.

## 6. Existing coarse metrics are not recipe history

The audit found two existing Registry metrics that must not be reused as recipe-level history:

- WorkflowRegistryService::project_usage_count is computed from distinct project IDs in project_workflow_bindings across all versions of a logical workflow.
- WorkflowRegistryService::history_count sums version-wide deletion/reference counts from tasks, production_batch_items, presets, project_templates, shot_stage_configs, and benchmark_candidates.

These are workflow-level purge/reference indicators, not exact-pair history. They include non-executed configuration references and may aggregate multiple recipes.

WorkflowRuntimeRepository likewise calculates active_tasks, total_tasks, successful-task existence, and latest success/failure timestamps by workflow_version_id, not by recipe_id.

~~~text
EXISTING_REGISTRY_COUNTS_ARE_RECIPE_HISTORY=NO
EXISTING_VERSION_TASK_METRICS_ARE_RECIPE_HISTORY=NO
~~~

The future exact-pair projection must query both identity columns.

## 7. Product question decisions

The following table freezes the minimum answer for every required product question.

| # | Product question | Decision |
|---:|---|---|
| 1 | Which Workflow Version owns this Recipe? | SUPPORTED_BY_EXISTING_DATA / DERIVABLE from recipes.workflow_version_id and workflow_versions. |
| 2 | What is the Recipe version? | SUPPORTED_BY_EXISTING_DATA from recipes.version. |
| 3 | Is it currently recommended/promoted? | SUPPORTED_BY_EXISTING_DATA from exact workflow_recipe_promotions; absent row means not promoted. |
| 4 | Is it archived? | SUPPORTED_BY_EXISTING_DATA from exact workflow_recipe_runtime_states; missing row means active. |
| 5 | How many times was it run? | DERIVABLE as exact-pair Task count. Display as task attempts, not queue-item count. |
| 6 | How many succeeded? | DERIVABLE as exact-pair Tasks with status SUCCEEDED. |
| 7 | How many failed? | DERIVABLE as exact-pair Tasks with status FAILED. |
| 8 | What was the latest run time? | DERIVABLE from exact-pair Task timestamps: latest terminal finished_at; expose latest attempt separately when no terminal task exists. |
| 9 | Which projects used it? | DERIVABLE from exact-pair project-scoped references. Minimum summary uses distinct project IDs from Tasks, queue batches, project bindings, production runs, and production run templates; distinguish executed projects from merely configured/queued projects. |
| 10 | Is there a current Project Binding? | SUPPORTED_BY_EXISTING_DATA from exact project_workflow_bindings; retain and show stale/unavailable bindings. |
| 11 | Is there a Preset? | SUPPORTED_BY_EXISTING_DATA from exact presets; show count and an on-demand bounded list of name, project, and updated time. |
| 12 | Is it in a Production Queue or historical batch? | SUPPORTED_BY_EXISTING_DATA from exact production_batch_items joined to production_batches; show batch/item status and archived batch metadata. |
| 13 | Was it used for Shot Production? | DERIVABLE from exact shot_stage_configs, then existing shot and generation-link relations; show episode/scene/shot/stage/task navigation when available. |
| 14 | Is there Benchmark/Experiment evidence? | SUPPORTED_BY_EXISTING_DATA from exact benchmark_candidates and related benchmark_runs; show experiment/candidate/run navigation. |
| 15 | Can it be compared with the previous Recipe? | DERIVABLE as a pure comparison of immutable YAML/definitions, but NOT_WORTH_IMPLEMENTING in the initial DEV-094 history slice; defer UI diff to a separate comparison boundary. |

No product question depends on new mutable history state.

## 8. Execution history authority

The authoritative execution-attempt record is tasks:

- It stores exact workflow_version_id and recipe_id.
- It stores project identity and lifecycle status.
- It stores created_at, queue/start/finish timestamps, errors, and runtime provenance.
- TaskStatus distinguishes non-terminal states, SUCCEEDED, FAILED, and CANCELLED.

generation_snapshots is immutable frozen execution evidence keyed by task_id. It does not duplicate the pair, so the future query joins through the Task. It must not become a second task history table.

The task-history read service intentionally keeps raw runtime/snapshot payloads out of the default serialized history view. Recipe History should follow the same privacy boundary: show status, timestamps, exact IDs/SHA in technical details, and navigation to the existing task; do not copy raw workflow JSON or prompt payloads.

~~~text
EXECUTION_HISTORY_AUTHORITY=tasks
FROZEN_EXECUTION_EVIDENCE=generation_snapshots_via_task_id
NEW_RECIPE_TASK_HISTORY_TABLE=NO
~~~

## 9. Queue and production semantics

### 9.1 Queue usage is not executed usage

production_batch_items are persisted at queue/batch creation and carry:

~~~text
workflow_version_id
recipe_id
values_json
status
task_id (nullable)
created_at
updated_at
~~~

A queue item with no task_id is planned or queued usage only. It must not increment successful runs or be presented as an executed Task.

When task_id exists, the history UI may navigate to the Task and use the Task as execution authority. The queue item remains useful for batch/item status and planned values.

### 9.2 Status display

The read-only history projection may display:

- Batch status: READY, RUNNING, PAUSED, COMPLETED.
- Item status: PENDING, DISPATCHING, DISPATCHED, SUCCEEDED, FAILED, CANCELLED, SKIPPED.
- archived_at for an archived batch.

A queue item with SUCCEEDED is a production-item result, but the Recipe “successful run” metric remains Task-based to avoid counting planned queue entries and Task execution twice. A queue item without a Task remains visible as queued/planned history.

Already admitted running Tasks continue under the existing lifecycle rules. Recipe History is read-only and must not cancel, retry, requeue, or rewrite them.

### 9.3 Production Run / Stage

production_stages persist nullable exact workflow-version and recipe fields and are linked to a project through production_runs. production_stage_items and preparation snapshots provide existing navigation to task, batch item, and frozen preparation evidence.

These rows are production references, not a replacement for Task execution counts.

## 10. Project, preset, template, shot, and experiment usage

### Project usage

The product must distinguish:

~~~text
EXECUTED_PROJECTS = distinct tasks.project_id for the exact pair
REFERENCED_PROJECTS = distinct project IDs from tasks, queue batches, bindings, production runs, and production run templates
~~~

The minimum summary can show referencedProjectCount with an “executed” breakdown. It must not relabel the existing workflow-wide project_usage_count as a recipe count.

project_templates are global in the current schema and have no project_id; they contribute to template usage, not project usage. production_run_templates are project-scoped and can carry separate Krea2 and H3 exact pairs.

### Presets and templates

The minimum useful read-only display is:

- Preset count, plus bounded entries with preset name, project, and updated_at.
- Project-template count, plus bounded entries with name and updated_at.
- Production-run-template count, plus bounded entries with project, name, and updated_at.

No preset or template schema changes are needed.

### Shot usage

A shot-stage configuration directly stores the exact pair. Its project is recovered through shots.project_id. A generation link then connects the shot/stage to an existing Task or production batch item. History can therefore navigate:

~~~text
Episode/Scene (existing project structure when available)
→ Shot
→ Stage
→ Task or Production Batch Item
~~~

No separate shot-history state is required.

### Experiment usage

A benchmark candidate directly stores the exact pair and belongs to an experiment with a project ID. Benchmark runs belong to candidates and can resolve Task, snapshot, output, and production-item evidence through existing links.

The history projection should show experiment/candidate/run counts and bounded navigation references, not copy benchmark result payloads.

## 11. Archive and removed-workflow behavior

### Archived Recipe

~~~text
HISTORY_READABLE=YES
ARCHIVE_HIDES_HISTORY=NO
~~~

Archive changes future availability only. It does not delete or rewrite:

- Recipe definition bytes or SHA.
- Tasks or generation snapshots.
- Queue items, batches, or frozen values.
- Presets, templates, experiments, shots, or production records.
- Existing exact project bindings.

The history UI must continue to show an archived label alongside historical usage.

### Soft-removed Workflow

Migration 028 stores logical workflow removal as workflows.library_state=REMOVED with removed_at. While the definition and referenced records remain, the history query continues to return exact recipe lineage and usage. Removed status is shown separately from recipe archive.

### Legal hard purge

The existing purge path remains authoritative. It inspects references and may physically remove definitions/artifacts only when its existing invariants permit it. After a legal hard purge, Recipe History must not manufacture a tombstone or fake definition row. If the exact definition no longer exists, no history entry may pretend that it can still display the deleted definition.

~~~text
SOFT_REMOVE_HISTORY=READABLE
HARD_PURGE_HISTORY=NO_SYNTHETIC_TOMBSTONE
PURGE_BEHAVIOR=UNCHANGED
~~~

## 12. Ordering

Recipe lineage ordering must reuse the existing WorkflowRegistryService::compare_versions semantics:

1. Compare up to the first three dot-separated numeric components.
2. Treat missing/invalid numeric components as zero for those comparisons.
3. Use the original version string as the deterministic fallback when numeric components tie.

The same numeric-first comparator exists in WorkflowWorkspaceQueryService. SQL MAX(version), insertion order, and plain lexicographic ordering are not lineage authorities; for example, 1.10 must not sort before 1.9.

For equal recipe-version strings, history list output uses exact recipe_id as the final stable display tie-breaker. That tie-break does not change current/promotion semantics.

~~~text
RECIPE_ORDERING=EXISTING_NUMERIC_FIRST_COMPARATOR
INSERTION_ORDER_AS_AUTHORITY=NO
LEXICAL_VERSION_ORDER_AS_AUTHORITY=NO
NEW_VERSION_COMPARATOR=NO
~~~

## 13. Minimum future read model

The future implementation should expose a read-only exact-pair projection similar to:

~~~text
RecipeHistorySummary {
  workflowId
  workflowVersionId
  recipeId
  workflowVersion
  recipeVersion
  recipeSha256

  isCurrentVersion
  isPromoted
  archived
  archivedAt

  taskCount
  activeTaskCount
  succeededTaskCount
  failedTaskCount
  cancelledTaskCount
  lastFinishedAt
  lastAttemptAt

  executedProjectCount
  referencedProjectCount
  presetCount
  projectTemplateCount
  productionRunTemplateCount
  queueItemCount
  shotStageCount
  experimentCount
  benchmarkRunCount
}
~~~

The exact implementation may omit a field only if the corresponding source is absent or the field cannot be made exact. It must not replace exact fields with workflow-wide counts.

A bounded detail projection may include:

- Task history entries with exact task ID, project, status, created/started/finished timestamps, and navigation target.
- Queue/batch references with batch/item IDs, project, status, and task_id presence.
- Preset/template references with business names and updated timestamps.
- Shot/stage and experiment/candidate/run navigation references.
- Promotion/archive metadata and exact technical IDs in a collapsed detail section.

It must not include raw Recipe YAML, raw workflow JSON, storage paths, prompt IDs, or internal database IDs in the default UI.

## 14. Query authority

### Decision: dedicated RecipeHistoryQueryService

Future Recipe History belongs in a dedicated read-only application service:

~~~text
RecipeHistoryQueryService
        ↓
read-only exact-pair repository queries
        ↓
existing SQLite facts
~~~

It is not owned by:

- WorkflowRegistryService mutation methods.
- WorkflowWorkspaceQueryService initial registry/runtime snapshot.
- Task, queue, benchmark, shot, or project mutation services.
- A frontend store or a new persistence source.

Reasons:

1. WorkflowWorkspaceQueryService currently builds registry/runtime rows and capability evidence; its task metrics are version-wide and its initial query loops over every version/recipe.
2. Adding all history fan-out there would make first Workspace load proportional to every recipe's history sources.
3. A dedicated service makes exact-pair filtering and read-only aggregation explicit without changing existing mutation authorities.
4. Existing repositories remain source-specific authorities; the new service only orchestrates reads and derives a projection.

The future repository boundary is a read-only exact-pair query port with set-based methods. It must not add mutation methods or a second lifecycle authority.

~~~text
QUERY_AUTHORITY=RecipeHistoryQueryService
MUTATION_AUTHORITY=EXISTING_DOMAIN_SERVICES
READ_ONLY_HISTORY_SERVICE=YES
~~~

## 15. IPC and UI information architecture

### IPC decision

Use an independent, typed, read-only, on-demand command:

~~~text
workflow_recipe_history_get(
  workflowVersionId,
  recipeId,
  taskCursor?,
  taskLimit?
)
~~~

The command must validate the exact pair and return the summary plus bounded detail sections. It must not accept a workflow name, recipe name, “latest” selector, or implicit fallback.

Task entries use keyset pagination ordered by created_at DESC, id DESC, matching the existing Task History repository. Related usage sections are counts plus bounded navigation references; they are not an unbounded cross-source timeline.

~~~text
IPC_READ_ONLY=YES
IPC_EXACT_IDENTITY=YES
IPC_MUTATION_SIDE_EFFECTS=NO
IPC_NAME_GUESSING=NO
~~~

This command is deliberately separate from the existing registry payload so the initial Workflow Workspace query remains lightweight.

### UI placement

Enhance the existing Workflow Workspace hierarchy only:

~~~text
Workflow
  └─ Version
      └─ Recipe
          ├─ version / SHA
          ├─ current / promoted / archived labels
          ├─ lightweight usage summary
          └─ 查看历史 / View history
~~~

“View history” opens a read-only detail panel or existing detail surface in the Workflow Workspace. It does not create a new Workspace, route, store, or mutation toolbar.

Allowed navigation targets are existing Task, Project, Shot, Production, and Experiment records. Any restore, promote, archive, rebind, rerun, retry, or generation action remains in its current explicit business entry point.

## 16. Pagination and performance

### Pagination

~~~text
TASK_HISTORY_PAGINATION=KEYSET
TASK_HISTORY_ORDER=created_at DESC, id DESC
RELATED_USAGE=COUNT_PLUS_BOUNDED_REFERENCES
UNIFIED_CROSS_SOURCE_TIMELINE=NO
~~~

A single merged timeline would imply comparable chronology across task, queue, shot, benchmark, and template records that currently have different timestamps and lifecycle meanings. Grouped sections are more truthful and cheaper.

### Performance strategy

~~~text
INITIAL_WORKSPACE_HISTORY_FANOUT=NO
HISTORY_DETAILS=ON_DEMAND
N_RECIPES_X_N_REPOSITORIES=NO
SET_BASED_EXACT_PAIR_QUERIES=YES
UNBOUNDED_DETAIL_LOAD=NO
~~~

The implementation must:

1. Keep the initial Workflow Workspace registry/runtime query unchanged.
2. Fetch one exact pair only after the user opens history.
3. Use set-based SQL aggregation, not one query per source row.
4. Bound task and related-reference lists.
5. Reuse existing keyset ordering and repository limits.
6. Run SQLite query-plan/performance checks against realistic task, queue, shot, benchmark, and template volumes.

Existing indexes already cover several supporting paths, including:

- idx_tasks_created_at and task status.
- idx_presets_project_recipe.
- idx_production_batch_items_batch_ordinal and task links.
- idx_benchmark_candidates_workflow.
- idx_benchmark_runs_experiment_candidate and task links.
- idx_production_stages_run_status and batch links.
- idx_production_run_templates_project.
- idx_project_workflow_bindings_project.
- idx_workflow_runtime_artifacts_version_recipe.
- idx_workflow_recipe_promotions_recipe.

Some exact-pair paths are reached through existing joins rather than a dedicated pair index, especially Tasks, Shot stage configuration, Queue items, and Production Stages. That is a performance consideration, not a missing-history fact.

~~~text
DEV_094A_NEW_INDEX=NO
DEV_094A_PERFORMANCE_MIGRATION=NO
FUTURE_INDEXES=MEASURED_OPTIMIZATION_ONLY
~~~

If realistic measurements later show a dedicated pair index is necessary, it must be a separately reviewed performance migration. It must not become a history state table or be smuggled into the semantics implementation.

## 17. Privacy and technical detail

Default history UI must not expose:

~~~text
raw Recipe YAML
raw workflow JSON
storage paths
prompt IDs
internal database IDs
~~~

Technical disclosure may include, in a collapsed section:

~~~text
workflowVersionId
recipeId
workflowSha256
recipeSha256
~~~

Business-facing labels, status, timestamps, counts, and navigation are the default. History reads must not copy sensitive payloads merely to make the page look complete.

## 18. Future DEV-094 implementation boundary

The future implementation is frozen to:

1. A read-only exact-pair repository query port and SQLite set-based queries.
2. RecipeHistoryQueryService aggregation and projection.
3. One typed read-only workflow_recipe_history_get command.
4. Existing Workflow Workspace recipe-row entry point and read-only detail surface.
5. Keyset task pagination and bounded related usage references.
6. Focused exact-pair, status, archive, soft-remove, queue-vs-task, project, shot, benchmark, and pagination tests.
7. No mutation, no new history table, no new store, no new queue/executor/task model, and no changes to archive/promotion/purge authority.

~~~text
WORKFLOW_MUTATION_CHANGE=NO
QUEUE_CHANGE=NO
EXECUTOR_CHANGE=NO
TASK_MODEL_CHANGE=NO
HISTORY_TABLE=NO
NEW_ZUSTAND=NO
~~~

## 19. Required future tests

At minimum, DEV-094 implementation tests must prove:

- Two recipes in one workflow version do not share counts or entries.
- The same recipe ID under different workflow versions cannot cross-match.
- Recipe definition, version, and SHA remain immutable in the projection.
- Promotion and archive labels are independent and exact.
- Missing recipe runtime-state row is treated as active.
- Archived recipes remain readable in history.
- Task counts include only the exact pair; success/failure counts use Task status.
- finished_at and fallback latest-attempt timestamps are derived deterministically.
- A queue item without task_id is visible as planned usage, not an executed run.
- Queue items with a Task navigate to the exact Task without double-counting.
- Project counts distinguish executed projects from referenced/configured projects.
- Preset, template, shot, benchmark, and production references use existing exact joins.
- Soft-removed workflows remain readable while definitions remain.
- Hard-purged definitions do not produce synthetic history.
- Keyset pagination is stable at equal timestamps.
- History reads perform no mutation and do not expose raw payloads by default.

## 20. Risk classification

The audit itself is documentation-only:

~~~text
PRODUCT_CODE_CHANGE_THIS_TASK=NO
DATABASE_CHANGE_THIS_TASK=NO
IPC_CHANGE_THIS_TASK=NO
REMOTE_CI_REQUIRED_THIS_TASK=NO
~~~

The future DEV-094 implementation is expected to be cross-layer but read-only:

~~~text
DATABASE_SCHEMA_CHANGE=NO
DATABASE_MIGRATION=NO
RUST_SOURCE_CHANGE=YES
IPC_SURFACE_CHANGE=YES
FRONTEND_SOURCE_CHANGE=YES

WORKFLOW_MUTATION_CHANGE=NO
QUEUE_CHANGE=NO
TASK_MODEL_CHANGE=NO
ARCHIVE_PROMOTION_PURGE_CHANGE=NO
NEW_HISTORY_TABLE=NO
NEW_HISTORY_STATE_SOURCE=NO

REMOTE_CI_REQUIRED=YES
~~~

The remote CI requirement follows the existing project policy: a new Rust query service and typed IPC surface are cross-layer source changes even though they do not change the schema.

## 21. Required decision matrix

| Question | Authority / Decision |
|---|---|
| Recipe definition lineage | Immutable recipes rows scoped by exact workflow_version_id; ordered by the existing numeric-first recipe-version comparator. |
| Recipe lifecycle state | Exact promotion row plus exact recipe runtime-state row; missing archive state means active. |
| Executed task history | Exact-pair tasks rows; generation_snapshots are frozen evidence through task_id. |
| Queue usage | Exact-pair production_batch_items joined to batches; no task_id means planned/queued, not executed. |
| Project usage | Derived distinct project IDs; separate executed projects from referenced/configured projects. Do not reuse workflow-wide Registry counts. |
| Preset usage | Exact presets rows; count plus bounded name/project/updated-time references. |
| Shot usage | Exact shot_stage_configs plus existing shot and generation-link joins. |
| Experiment usage | Exact benchmark_candidates plus related benchmark runs/results. |
| Current/promoted/archive state | Current version from workflow registry; promoted from promotion table; archived from recipe runtime state; historical is separate. |
| Historical exact identity | Always workflowVersionId + recipeId; no rebinding or name guessing. |
| Ordering | Existing compare_versions numeric-first semantics, exact recipe ID tie-break for display only. |
| Archived history readability | YES; archive blocks future availability, not reads of retained definitions or references. |
| Removed workflow behavior | Soft removal remains readable; legal hard purge has no synthetic tombstone. |
| Query authority | Dedicated read-only RecipeHistoryQueryService, backed by exact-pair set-based repository reads. |
| Database migration needed | NO for semantics or the initial read model; no history table. |
| IPC shape | Typed on-demand workflow_recipe_history_get(workflowVersionId, recipeId, taskCursor?, taskLimit?); read-only exact pair. |
| UI placement | Existing Workflow Workspace Version → Recipe row with a read-only history detail surface. |
| Pagination | Keyset task pagination; grouped related-reference counts and bounded lists; no merged cross-source timeline. |
| Performance strategy | On-demand detail, set-based exact-pair aggregation, bounded lists, no initial N-recipes fan-out, measured indexes only as a later optimization. |

No decision in this matrix is TBD.

## 22. Frozen conclusion

~~~text
RECIPE_HISTORY=DERIVED_EXISTING_FACTS
RECIPE_HISTORY_AUTHORITY=DEDICATED_READ_ONLY_QUERY_SERVICE
HISTORY_EXECUTION_AUTHORITY=TASKS
HISTORY_FROZEN_EVIDENCE=GENERATION_SNAPSHOTS_VIA_TASK_ID

HISTORICAL_EXACT_IDENTITY=workflowVersionId+recipeId
HISTORICAL_REFERENCE_IMMUTABILITY=YES
ARCHIVED_HISTORY_READABLE=YES
SOFT_REMOVED_HISTORY_READABLE=YES
HARD_PURGE_SYNTHETIC_HISTORY=NO

QUEUE_ITEM_WITHOUT_TASK=PLANNED_USAGE_ONLY
SUCCESS_FAILURE_COUNTS=EXACT_TASK_STATUS
PROJECT_USAGE=EXECUTED_AND_REFERENCED_COUNTS_DISTINGUISHED

RECIPE_COMPARE=DEFERRED
NEW_HISTORY_TABLE=NO
DATABASE_MIGRATION=NO
NEW_HISTORY_STATE_SOURCE=NO
NEW_INDEX_THIS_TASK=NO

IPC_STRATEGY=TYPED_ON_DEMAND_READ_ONLY_EXACT_PAIR
UI_PLACEMENT=EXISTING_WORKFLOW_WORKSPACE_RECIPE_DETAIL
PAGINATION=KEYSET_TASK_HISTORY
PERFORMANCE_STRATEGY=ON_DEMAND_SET_BASED_BOUNDED_READS

WORKFLOW_MUTATION_CHANGE=NO
QUEUE_CHANGE=NO
TASK_MODEL_CHANGE=NO
REMOTE_CI_REQUIRED_FOR_FUTURE_DEV_094=YES

DEV-094A=PASS
DEV-094_IMPLEMENTATION=NOT_STARTED
~~~
