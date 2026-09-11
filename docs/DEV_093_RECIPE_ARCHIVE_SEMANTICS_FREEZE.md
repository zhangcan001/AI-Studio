# DEV-093 Recipe Archive Semantics Freeze

## 0. Scope and audit baseline

```text
TASK=DEV-093A
REPOSITORY=zhangcan001/AI-Studio
BRANCH=master
BASELINE=ee7ab368a49a179387025f398ecf3906a14334a4

DEV-093A=PASS
PRODUCT_CODE_CHANGE=NO
DATABASE_CHANGE=NO

DEV-093_IMPLEMENTATION=NOT_STARTED
DEV-093_IMPLEMENTATION_REMOTE_CI_REQUIRED=YES
```

This document freezes semantics only. It does not implement Recipe Archive and does not modify product code, migrations, IPC, scripts, workflows, or package files.

Audited paths included:

- `src-tauri/migrations/001_initial.sql`
- `src-tauri/migrations/003_presets.sql`
- `src-tauri/migrations/005_workflow_runtime_state.sql`
- `src-tauri/migrations/006_production_queue.sql`
- `src-tauri/migrations/008_organization.sql`
- `src-tauri/migrations/010_shot_production.sql`
- `src-tauri/migrations/013_workflow_archive_and_package_metadata.sql`
- `src-tauri/migrations/014_workflow_benchmark.sql`
- `src-tauri/migrations/018_production_orchestrator.sql`
- `src-tauri/migrations/024_production_preparation_snapshots.sql`
- `src-tauri/migrations/027_project_workflow_bindings.sql`
- `src-tauri/migrations/028_workflow_registry_v2.sql`
- `src-tauri/migrations/030_workflow_recipe_promotions.sql`
- `src-tauri/src/application/workflow_registry_service.rs`
- `src-tauri/src/application/generation_service.rs`
- `src-tauri/src/application/production_queue_service.rs`
- `src-tauri/src/application/production_start_admission_service.rs`
- `src-tauri/src/application/project_workflow_binding_service.rs`
- `src-tauri/src/application/workflow_lifecycle_service.rs`
- `src-tauri/src/application/workflow_workspace_query_service.rs`
- `src/features/workflows/workflowWorkspaceAdapters.ts`
- `src/features/workflows/WorkflowWorkspace.tsx`
- `src/features/workflows/WorkflowWorkspaceList.tsx`

## 1. Confirmed current persistence model

The current model has three different authorities:

| Data | Current authority | Semantics |
|---|---|---|
| `recipes` | `recipes` in migration 001 | Immutable recipe definition rows: exact ID, version, YAML, schema version, SHA-256, and creation time. |
| Workflow-version availability | `workflow_runtime_states` from migrations 005 and 013 | Mutable version-level `enabled`, `archived`, `archived_at`, and `updated_at`. |
| Promotion | `workflow_recipe_promotions` from migration 030 | Optional exact `(workflow_version_id, recipe_id)` promotion. A missing row means no promotion. It is independent of `workflows.current_version_id`. |

`recipes` currently has no recipe-level archive field. The existing `archived` fields are version-level state. The current `WorkflowRegistryRecipeView` TypeScript type already has optional `archived` and `archivedAt` slots, but the backend does not currently populate recipe-level archive semantics.

The 030 composite unique index on `(recipes.workflow_version_id, recipes.id)` is already present. It is the required exact-key target for a future recipe-state foreign key.

## 2. Exact identity is frozen

Every persisted or historical reference remains the exact pair:

```text
workflowVersionId + recipeId
```

Recipe archive must never rewrite, migrate, guess, or rebind that pair. This applies to:

- Tasks and task history.
- Generation snapshots and their task identity.
- Production queue items and retry lineage.
- Presets.
- In-memory Batch Draft items.
- Project templates and production run templates.
- Benchmark experiments, candidates, runs, and results.
- Project explicit bindings.
- Shot stage configuration and shot generation links.
- Frozen production preparation data.
- Any future history or audit record.

The following invariants are mandatory:

```text
AUTO_REBIND=NO
AUTO_MIGRATION=NO
NAME_GUESSING=NO
HISTORICAL_REFERENCE_IMMUTABILITY=YES
```

Archive changes future availability only. It does not alter definition bytes, identity, values, task provenance, queue values, snapshots, or history.

## 3. Recommended Recipe Archive authority

### Decision: Option A — dedicated recipe runtime-state table

DEV-093 must add a dedicated runtime-state repository backed by a new migration, rather than adding mutable lifecycle columns to `recipes`.

Proposed table shape:

```sql
CREATE TABLE workflow_recipe_runtime_states (
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    archived_at TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workflow_version_id, recipe_id),
    FOREIGN KEY (workflow_version_id, recipe_id)
        REFERENCES recipes(workflow_version_id, id)
        ON DELETE CASCADE
);
```

The exact migration name is `031_workflow_recipe_archive_state.sql`.

Reasons:

1. `recipes` is an immutable definition table. Mutable archive metadata belongs beside the existing version runtime state, not inside the immutable definition row.
2. The composite foreign key prevents a recipe ID from being accidentally interpreted under a different workflow version.
3. Migration 030 already provides the composite unique index required by SQLite for the exact foreign key.
4. Existing databases need no backfill: a missing recipe-state row means `archived=false`, matching the backward-compatible missing-row behavior of `workflow_runtime_states`.
5. Physical recipe deletion, if reached through the existing hard-delete lifecycle, cascades only the state row. Archive itself never deletes the recipe, package, task, snapshot, queue, or history.
6. Restore is an exact state mutation on the same pair and cannot accidentally change workflow currentness or promotion.

Option B—adding `archived` and `archived_at` directly to `recipes`—is rejected because it makes the immutable definition table mutable, couples definition migration to lifecycle state, and weakens the separation already established by `workflow_runtime_states`.

## 4. Single availability authority

`WorkflowRegistryService::is_available(workflow_version_id, recipe_id)` remains the single Registry availability predicate. DEV-093 extends that existing predicate; it does not create a second archive resolver.

The future predicate must require all of the existing facts plus recipe state:

```text
exact workflow version exists
exact recipe belongs to exact workflow version
logical workflow is ACTIVE
workflow version is enabled
workflow version is not archived
recipe is not archived
exact runtime artifact exists and hashes match
runtime package identity matches
at least one workflow version remains active
```

`GenerationService::prepare_task` already calls `NewGenerationAdmission` before creating a task or snapshot. The Recipe Archive implementation must make this existing path reject archived recipes with a stable archive-specific admission code; it must not add a second generation admission rule.

All other paths that can create new work must delegate to the same Registry predicate or an application service that calls it:

- Production queue creation must reject a new batch containing an archived recipe before persistence.
- Production queue start and dispatch must re-check the exact pending pairs through existing start admission and generation admission.
- Production orchestrator run/stage creation must reject a new archived recipe reference.
- New experiment candidates/runs must reject an archived recipe.
- New shot production requests and retries must reject an archived recipe.
- Preset application to a new generation must reject an archived recipe; existing preset rows remain readable.

No path may infer availability from a recipe name, latest version, package name, catalog order, or historical definition existence.

## 5. Future generation policy

```text
ARCHIVED_RECIPE_NEW_GENERATION_ALLOWED=NO
```

The following are blocked for an archived exact pair:

- Direct new generation.
- Implicit generation through the promoted/default/latest recipe path.
- Newly created production batches or production runs.
- New experiment work.
- New shot production or retry work.
- New generation created from an existing preset or draft.

Existing definition lookup remains available for history, inspection, and migration-safe reads. Definition readability must not be confused with new-generation admission.

## 6. Production Queue decision

### Decision: B — reject at execution admission

Current code proves that a production queue item stores `(workflow_version_id, recipe_id)` when the batch is created, but the Task is created later by `ProductionQueueService` through `GenerationService::start_generation_with_task_hook`. The formal `ProductionStartAdmissionService` also inspects the exact pending pairs before committing a batch start.

Therefore, when a pending queue item was created before archive and the recipe is archived before execution:

1. The queue item remains the same exact pair and its frozen values remain unchanged.
2. The formal batch-start admission rejects the exact archived pair before a not-yet-running batch is committed to `RUNNING`.
3. If a batch is already `RUNNING` and a later pending item reaches the dispatch loop, the existing generation admission rejects it before task creation; the existing queue error path records the failure without executing the archived recipe.
4. No queue item is silently rewritten, rebound, or switched to a promoted/latest recipe.

A running Task that passed admission before the archive continues to its existing terminal outcome. Archive is not cancellation and does not interrupt execution already admitted.

This is the required behavior because the Task does not exist at queue-item creation time and the existing execution path already has exact-pair admission. DEV-093 must strengthen those existing gates, not add a second executor or a grandfathered bypass.

## 7. Explicit project bindings

A project binding remains the exact persisted pair after archive:

```text
binding identity = workflowVersionId + recipeId
AUTO_REBIND=NO
```

The binding is retained and returned as stale/unavailable. It is not deleted, rewritten, or replaced with a promotion/default/latest recipe.

New generation through that binding is blocked by the single Registry availability predicate. The UI must show the binding as unavailable and require an explicit user selection of another active exact pair before replacing it.

The existing `ProjectWorkflowBindingService` already preserves stale references and evaluates availability separately. DEV-093 extends that availability result to include recipe state.

## 8. Promotion interaction

### Decision: A — reject archive while promoted

If `(V1, R2)` is promoted, archiving `R2` is rejected with no mutation. The user must explicitly clear the exact promotion first, then issue the archive action.

Reasons:

- No hidden cross-repository mutation.
- Promotion metadata remains internally coherent.
- Existing exact `clear_recipe_promotion` is the explicit transaction boundary for promotion removal.
- Archive failure has no partial state to compensate.
- The current fallback can then select the newest non-archived recipe after the user explicitly clears promotion.

`WorkflowRegistryService::promote_recipe` must also reject an already archived recipe defensively. `clear_recipe_promotion` remains exact and idempotent. Archive must never automatically clear or replace promotion metadata.

## 9. Restore semantics

```text
RESTORE_IS_EXPLICIT=YES
```

Restore:

- Writes `archived=false` for the same exact `(workflow_version_id, recipe_id)`.
- Preserves the same `recipe_id`, definition bytes, version, and SHA-256.
- Does not automatically promote the recipe.
- Does not change `workflows.current_version_id`.
- Does not modify project bindings.
- Does not create a new recipe or version.
- Does not change tasks, snapshots, queue items, presets, experiments, shots, or history.
- Does not modify built-in package files.

A restored recipe becomes eligible for future generation only after the existing exact runtime, enabled, capability, and package admission checks pass.

## 10. Implicit resolution and current recipe

`resolveImplicitWorkflowRecipe()` remains the single frontend implicit-resolution boundary. Its future rule is:

```text
archived recipe -> excluded from implicit resolution
```

No `resolveImplicitActiveRecipe2`, GenerationStudio-specific archive resolver, or project-specific archive resolver may be introduced.

The backend Registry read model must distinguish version state from recipe state:

- `WorkflowRegistryVersionView.archived` continues to mean workflow-version archive.
- `WorkflowRegistryRecipeView.archived` and `archivedAt` become recipe-level runtime metadata.
- Runtime inspection must expose recipe archive separately from version archive; the existing version-level `archived` meaning must not be overloaded.

`currentRecipe` is derived only from active recipes in the selected current version:

1. An exact promoted, non-archived recipe.
2. Otherwise the newest non-archived recipe by existing recipe-version ordering.
3. `null` if no eligible recipe exists, which is a defensive state for legacy/inconsistent data.

An archived recipe is never returned as `currentRecipe`. This prevents a UI row from showing an apparently usable current recipe while generation admission rejects it. The complete `recipes` list still includes archived historical definitions with their archive metadata.

## 11. Last active recipe

### Decision: reject archiving the last active recipe

```text
REJECT_LAST_ACTIVE_RECIPE_ARCHIVE=YES
ALLOW_ZERO_ACTIVE_RECIPES=NO
```

A workflow version must retain at least one non-archived recipe while the version itself remains enabled and non-archived. Attempting to archive the last active recipe is rejected before any repository write.

This keeps `currentRecipe`, Registry readiness, and new-generation admission aligned. A version that has no active recipe is not a useful enabled version and would otherwise create a UI/admission contradiction.

Legacy or externally inconsistent data with zero active recipes is fail-closed: `currentRecipe=null`, readiness is blocked, and new generation is rejected. It is not auto-repaired.

## 12. Built-in and user recipes

```text
USER_RECIPE_ARCHIVE_ALLOWED=YES
BUILTIN_RECIPE_ARCHIVE_ALLOWED=YES
```

Both use the same dedicated runtime-state table and the same promotion/last-active guards. Built-in archive is runtime metadata only:

- No `recipe.yaml` edit.
- No workflow package edit.
- No built-in manifest edit.
- No package directory deletion or quarantine.
- No change to product workflow source files.

The existing product workflow purge prohibition remains unrelated. Recipe archive is reversible availability state; it is not package lifecycle deletion.

## 13. Archive is not Delete/Purge

```text
ARCHIVE=REVERSIBLE_RUNTIME_AVAILABILITY_STATE
DELETE_OR_PURGE=PHYSICAL_OR_LIFECYCLE_DELETION
```

Recipe archive must not:

- Delete a `recipes` row.
- Delete a task or generation snapshot.
- Delete a production batch or queue item.
- Delete a preset, experiment, shot configuration, or history.
- Delete or rewrite a package file.
- Reuse the existing workflow purge path.

The existing workflow purge remains a separate hard-delete lifecycle guarded by reference inspection. DEV-093 must not route recipe archive through purge.

## 14. Required decision matrix

| Action / reference | Archived recipe result |
|---|---|
| Future implicit selection | Excluded by the single `resolveImplicitWorkflowRecipe` boundary. |
| Direct new generation | Rejected by `WorkflowRegistryService::is_available` through existing `NewGenerationAdmission`. |
| Existing project exact binding | Exact pair remains; shown stale/unavailable; no automatic rebind; new generation blocked. |
| Existing queued item | Exact pending item remains; formal start/dispatch admission rejects it before archived execution. |
| Existing running task | Continues unchanged; archive does not cancel or mutate an already admitted task. |
| Completed task/history | Preserved and readable with the original exact pair and snapshot evidence. |
| Preset | Existing row remains readable and exact; applying it to new generation is blocked until an active exact recipe is explicitly selected. |
| Batch draft | Exact draft item and values remain; submit/queue creation is blocked; user must explicitly select an active exact pair. |
| Experiment result | Existing experiment/candidate/run/result remains readable and exact; new work from the archived candidate is blocked. |
| Shot binding | Exact shot configuration remains; new production/retry is blocked; explicit replacement is required. |
| Promoted recipe | Archive is rejected until the user explicitly clears the exact promotion. |
| Restore | Explicitly restores the same exact recipe ID; no promotion, current-version, binding, or new-version side effect. |
| Last active recipe | Archive is rejected. At least one non-archived recipe must remain for an enabled, non-archived version. |
| Built-in recipe | Runtime archive is allowed under the same guards; package and manifest remain immutable. |

No matrix entry is deferred.

## 15. Minimum DEV-093 implementation boundary

The implementation must remain one Recipe Archive boundary and include only the necessary call-site guards:

1. `src-tauri/migrations/031_workflow_recipe_archive_state.sql`.
2. A `WorkflowRecipeRuntimeStateRepository` application port and SQLite implementation with exact composite-key reads/writes.
3. `WorkflowRegistryService` archive/restore methods, exact membership validation, promotion guard, last-active guard, read-model projection, and the extended `is_available` predicate.
4. `WorkflowWorkspaceQueryService` and lifecycle runtime inspection changes that expose recipe-level archive without conflating version-level archive.
5. Typed Rust commands and typed frontend transport for explicit recipe archive/restore.
6. `WorkflowRegistryRecipeView`/workspace adapter/list/action changes for stale state and explicit archive/restore UI.
7. `resolveImplicitWorkflowRecipe` filtering of archived recipes; no second resolver.
8. Narrow new-work admission wiring for queue creation, queue start/dispatch, orchestrator creation, experiment creation, shot production/retry, preset application, and direct generation. All must reuse the single Registry predicate.
9. Focused repository, service, admission, queue, binding, promotion, restore, read-model, and frontend resolver tests.
10. Architecture guard coverage for the single recipe archive authority and absence of duplicate archive resolvers.
11. Result documentation and required remote Source-only CI acceptance.

The implementation must not add a second queue, executor, task model, package format, or archive state source.

## 16. Required future tests

The DEV-093 implementation must test at least:

- Exact recipe-state isolation between two recipes in one workflow version.
- Cross-version same recipe ID cannot address the wrong state.
- Missing state row is active for migration compatibility.
- Archive and restore preserve exact definition identity.
- Archive of a promoted recipe is rejected without clearing promotion.
- Explicit promotion clear and subsequent archive each commit atomically, with no hidden combined mutation.
- Last active recipe archive is rejected.
- Direct generation and implicit resolution reject archived recipes.
- New queue creation rejects archived recipes.
- Existing pending queue item is rejected at start/dispatch after archive.
- Already running task continues and completed history remains readable.
- Project binding remains stale and is not rebound.
- Preset, batch draft, experiment, and shot references remain exact.
- Built-in archive changes runtime metadata only.
- Restore does not promote, change current version, or rewrite bindings.
- `currentRecipe` never points to an archived recipe.

## 17. Risk classification

The future DEV-093 implementation is cross-layer and requires:

```text
DATABASE_SCHEMA_CHANGE=YES
DATABASE_MIGRATION=YES
RUST_SOURCE_CHANGE=YES
IPC_SURFACE_CHANGE=YES
WORKFLOW_LIFECYCLE_CHANGE=YES
REMOTE_CI_REQUIRED=YES
```

This DEV-093A audit is documentation-only:

```text
PRODUCT_CODE_CHANGE=NO
DATABASE_CHANGE_THIS_TASK=NO
REMOTE_CI_REQUIRED_THIS_TASK=NO
```

## 18. Frozen conclusion

```text
RECIPE_ARCHIVE_AUTHORITY=DEDICATED_RECIPE_RUNTIME_STATE_TABLE
ARCHIVED_RECIPE_NEW_GENERATION_ALLOWED=NO
QUEUE_POLICY=ADMISSION_REJECTS_ARCHIVED_EXACT_PAIR
PROMOTION_POLICY=REJECT_ARCHIVE_UNTIL_EXPLICIT_CLEAR
RESTORE_IS_EXPLICIT=YES
REJECT_LAST_ACTIVE_RECIPE_ARCHIVE=YES
BUILTIN_RECIPE_ARCHIVE=RUNTIME_METADATA_ONLY
HISTORICAL_REFERENCE_IMMUTABILITY=YES
ONE_IMPLICIT_RECIPE_RESOLUTION_RULE=YES

DEV-093A=PASS
DEV-093_IMPLEMENTATION=NOT_STARTED
```
