# DEV-089M Generation Experiment Controller Result

```text
TASK=DEV-089M
PARENT_TASK=DEV-089
REPOSITORY=zhangcan001/AI-Studio
BRANCH=master

BASELINE=1bbd4daffe50519dd2be2bac4a47841adbf5520e
IMPLEMENTATION=7e0684210427335e6d64efa09fa196bf8a7d4157
DEV_089M=PASS
DEV_089=OPEN
```

## Scope

The Experiment / Variant Plan lifecycle was extracted from `GenerationStudio` into:

```text
src/features/studio/hooks/useGenerationExperimentController.ts
```

The workspace remains the composition container. Project Template, Asset Intent, workflow selection, prompt library, dashboard, production queue authority, and unrelated Studio state remain outside this controller.

## Ownership

The controller now owns:

- experiment focus batch state;
- experiment batch contexts and cloned base values;
- prompt experiment dimensions;
- legacy experiment-plan queue creation, admission refresh, and start ordering;
- benchmark-created focus coordination;
- exact workflow-version/recipe winner promotion;
- project-boundary reset of experiment state.

`GenerationStudio` continues to own composition, Studio mode, notices, missing-asset presentation, and cross-domain callbacks. Queue creation and starting still use the existing `tauriClient` authority.

## Preservation

```text
EXACT_WORKFLOW_IDENTITY=workflowVersionId+recipeId
VALUES_CLONED=YES
QUEUE_CREATE_START_ORDER=PRESERVED
ADMISSION_GUARD=PRESERVED
ADMISSION_REFRESH_FAILURE=BEST_EFFORT_PRESERVED
EXPERIMENT_CONTEXTS=PRESERVED
FOCUS_BATCH=PRESERVED
WINNER_PROMOTION=PRESERVED
PROJECT_ISOLATION=PRESERVED
NO_AUTO_RETRY=YES
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_STATE_SOURCE=YES
NO_NEW_ZUSTAND=YES
```

Winner promotion still loads the exact catalog recipe into the Studio store, preserves reuse provenance, switches to single mode, reports missing assets, and never auto-submits a generation.

## Size

```text
GenerationStudio.tsx=912 -> 828 lines
OWNERSHIP_CLEAR=YES
```

## Verification

```text
FOCUSED_EXPERIMENT_CONTROLLER=PASS
FOCUSED_TESTS=1 file, 9 passed
STUDIO_EXPERIMENT_PRODUCTION_REGRESSION=29 files, 194 passed
FULL_FRONTEND=142 files, 722 passed
TSC=PASS
BUILD=PASS
ARCHITECTURE_GUARD=PASS
RPC_PARITY=PASS
FRONTEND_NO_RAW_INVOKE=PASS
RAW_INVOKE_OUTSIDE_TRANSPORT=0
RUST_CHECK=PASS
RUST_TEST=796 passed, 0 failed, 1 ignored
TAURI_BUILD=PASS
DIFF_CHECK=PASS
```

Architecture output includes:

```text
GENERATION_EXPERIMENT_CONTROLLER=PASS
SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION=PASS
SHOT_WORKSPACE_QUEUE_CONTROLLER=PASS
SHOT_WORKSPACE_MONITOR_CONTROLLER=PASS
SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER=PASS
ASSET_VIDEO_WORKFLOW_CONTROLLER=PASS
ASSET_VIDEO_LIBRARY_CONTROLLER=PASS
ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER=PASS
```

## Frozen invariants

```text
SMART_IMPORT_BEHAVIOR=UNCHANGED
PROJECT_TEMPLATE_BEHAVIOR=UNCHANGED
ASSET_INTENT_BEHAVIOR=UNCHANGED
PROMPT_LIBRARY_BEHAVIOR=UNCHANGED
PRODUCTION_QUEUE_AUTHORITY=UNCHANGED
RUST_CHANGE=NO
IPC_CHANGE=NO
DATABASE_CHANGE=NO
CSS_CHANGE=NO
VISUAL_CHANGE=NO
```

## Git and CI policy

```text
IMPLEMENTATION_COMMIT=7e0684210427335e6d64efa09fa196bf8a7d4157
IMPLEMENTATION_MESSAGE=refactor(studio): extract experiment controller
IMPLEMENTATION_PUSH=YES
REMOTE_CI_REQUIRED=NO
REMOTE_CI_WAITED=NO
```

The implementation commit was pushed to `origin/master`. Under the active CI-OPT-003 policy, normal master pushes do not require waiting for Source-only CI.

The next commit records this result document only.
