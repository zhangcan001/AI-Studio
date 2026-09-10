# DEV-090 — Explicit Recipe Promotion Foundation Result

```text
TASK=DEV-090
BASELINE_SHA=8a0a142643125f1a26fbe81b5ec96f2d6fa41885
IMPLEMENTATION_SHA=513544cef97891307d204ca548082dc925506b0e
RESULT_SHA=see final documentation commit in git history
DEV_089=CLOSED
DEV_090=PASS
DEV_089_REMOTE_PHASE_ACCEPTANCE=PASS
```

## Scope and authority

DEV-090 adds explicit promotion state for one existing recipe within a workflow
version. Identity is always the exact pair `(workflowVersionId, recipeId)`.
Promotion is owned by `WorkflowRegistryService` and persisted by
`WorkflowRecipePromotionRepository`; Workflow Workspace reads the resulting
metadata from the existing registry read model.

The implementation does not change new-recipe publishing, recipe editing,
duplicate, archive/delete/history behavior, current-version authority, queue
behavior, or GenerationStudio behavior.

## Persistence and semantics

```text
PERSISTENCE_MODEL=SQLite migration 030 workflow_recipe_promotions
PROMOTION_SCOPE=one existing recipe per workflow_version_id
ONE_PROMOTED_PER_WORKFLOW_VERSION=YES
IDEMPOTENT_PROMOTION=YES
ATOMIC_REPLACEMENT=YES
EXACT_MEMBERSHIP_VALIDATION=YES
PROMOTION_METADATA_IN_REGISTRY_READ_MODEL=YES
CLEAR_PROMOTION=DEFERRED
```

Migration 030 uses `workflow_version_id` as the primary key and a composite
foreign key for exact recipe membership. Deleting a workflow version or its
recipe cascades promotion cleanup. Existing databases receive no automatic
promotion and preserve their existing recipe selection behavior.

```text
WORKFLOW_CURRENT_AUTHORITY=PRESERVED
EXPLICIT_RECIPE_REF_AUTHORITY=PRESERVED
HISTORICAL_REFERENCE_IMMUTABILITY=PRESERVED
NEW_RECIPE_PUBLISH_BEHAVIOR=PRESERVED
GENERATION_SELECTION_BEHAVIOR_CHANGE=NO
RECIPE_ARCHIVE_CHANGE=NO
RECIPE_HISTORY_CHANGE=NO
ARCHIVED_WORKFLOW_PROMOTION_POLICY=REJECT
DISABLED_WORKFLOW_PROMOTION_POLICY=ALLOWED_WHEN_NOT_ARCHIVED
BUILTIN_RECIPE_PROMOTION_POLICY=ALLOWED_SEPARATE_FROM_IMMUTABLE_PACKAGE
DELETE_CLEANUP_POLICY=FK_CASCADE_ON_WORKFLOW_VERSION_AND_EXACT_RECIPE
BACKWARD_COMPATIBILITY=YES; old DB gets no promotions; no auto-selection
```

Promotion of a historical workflow version does not change which version is
current. When that version is selected, its promoted recipe is preferred
within that version; otherwise the existing version/recipe fallback remains.

## UI and API

Workflow Workspace exposes a non-optimistic `设为推广配方` action for valid,
non-archived registry recipes. A successful action refreshes the authoritative
workspace and catalog data. A failed action surfaces an error and preserves the
previous state. GenerationStudio and the generation catalog do not consume or
display promotion metadata in this task.

## Validation

```text
PROMOTION_REPOSITORY_TESTS=PASS
PROMOTION_SERVICE_TESTS=PASS
WORKFLOW_LIFECYCLE_TESTS=PASS
WORKFLOW_WORKSPACE_TESTS=PASS
GENERATION_CATALOG_REGRESSION=PASS
GENERATION_STUDIO_REGRESSION=PASS

FULL_FRONTEND=PASS
TSC=PASS
BUILD=PASS
RPC_PARITY=PASS
FRONTEND_NO_RAW_INVOKE=PASS
ARCHITECTURE_GUARD=PASS
RUST_CHECK=PASS
RUST_TESTS=PASS
TAURI_BUILD=PASS
DIFF_CHECK=PASS

REMOTE_CI_REQUIRED=YES
REMOTE_CI_RUN=#115
REMOTE_CI_STATUS=GREEN
REMOTE_CI_HEAD=d8b98aa056d09797e45e0c5834987502bea872b9
```

Focused promotion UAT covers exact identity, idempotent replacement, failed
promotion preservation, archived-version gating, workspace/catalog refresh,
and visible promoted state. The full frontend suite passed with 146 files and
770 tests. The Rust all-target suite passed; existing compiler warnings remain
non-fatal.

Architecture output includes:

```text
WORKFLOW_RECIPE_PROMOTION=PASS
WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER=PASS
WORKFLOW_SMART_IMPORT_CONTROLLER=PASS
GENERATION_PRESET_CONTROLLER=PASS
GENERATION_SUBMISSION_CONTROLLER=PASS
GENERATION_BATCH_CONTROLLER=PASS
GENERATION_EXPERIMENT_CONTROLLER=PASS
GENERATION_PROJECT_TEMPLATE_CONTROLLER=PASS
GENERATION_ASSET_INTENT_CONTROLLER=PASS
GENERATION_WORKFLOW_SELECTION_CONTROLLER=PASS
SHOT_WORKSPACE_* = PASS
ASSET_VIDEO_*_CONTROLLER=PASS
```

## Frozen invariants

```text
NEW_RECIPE_BEHAVIOR=UNCHANGED
EDITOR_BEHAVIOR=UNCHANGED
DUPLICATE_BEHAVIOR=UNCHANGED
ARCHIVE_DELETE_HISTORY_BEHAVIOR=UNCHANGED
CURRENT_VERSION_AUTHORITY=UNCHANGED
QUEUE_BEHAVIOR=UNCHANGED
GENERATIONSTUDIO_BEHAVIOR=UNCHANGED
RUST_SOURCE_CHANGE=YES
IPC_SURFACE_CHANGE=YES
DATABASE_SCHEMA_CHANGE=YES
DATABASE_MIGRATION=030_workflow_recipe_promotions.sql
WORKFLOW_LIFECYCLE_CHANGE=YES; promotion lifecycle only
FRONTEND_CLIENT_CHANGE=YES
QUEUE_BEHAVIOR_CHANGE=NO
EXECUTOR_BEHAVIOR_CHANGE=NO
TASK_MODEL_CHANGE=NO
DATABASE_BEHAVIOR_CHANGE=NO
CSS_CHANGE=NO
```

## Commits

```text
IMPLEMENTATION_COMMIT=513544cef97891307d204ca548082dc925506b0e
IMPLEMENTATION_COMMIT_MESSAGE=feat(workflows): add explicit recipe promotion
RESULT_COMMIT=see final documentation commit in git history
RESULT_COMMIT_TYPE=DOCS_ONLY
```

## Files changed

The implementation is limited to the workflow promotion repository/port,
registry command/read-model wiring, Workflow Workspace action and display,
architecture guard, migration compatibility assertions, and focused UAT:

```text
scripts/dev088-architecture-guard.mjs
src-tauri/migrations/030_workflow_recipe_promotions.sql
src-tauri/src/application/ports/workflow_recipe_promotion_repository.rs
src-tauri/src/application/workflow_registry_service.rs
src-tauri/src/commands/workflow_registry.rs
src-tauri/src/infrastructure/database/repositories/workflow_recipe_promotion.rs
src-tauri/src/lib.rs
src/features/workflows/WorkflowWorkspace.tsx
src/features/workflows/WorkflowWorkspaceList.tsx
src/features/workflows/WorkflowRecipePromotionUat.test.tsx
src/services/tauriClient.ts
src/services/workflowClient.ts
src/types/workflowOnboarding.ts
```

Migration compatibility test fixtures were updated only for the new migration
maximum and cleanup of the promotion table/index in old-schema upgrade tests.

## Deferred and next task

```text
DEFERRED_RECIPE_LIFECYCLE=
clear promotion; recipe archive/history; promotion consumption policy
NEXT_SINGLE_TASK=DEV-091 Recipe Promotion Consumption Policy
```

DEV-091 is only a recommendation. It is not started by this task.

```text
DEV_090_RESULT=PASS
```
