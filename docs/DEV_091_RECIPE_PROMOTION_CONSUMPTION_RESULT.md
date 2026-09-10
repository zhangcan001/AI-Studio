# DEV-091 Recipe Promotion Consumption Result

```text
TASK=DEV-091
BASELINE_SHA=8fd1594cdd5e4a88b0f596d6db588f4827c7b205
IMPLEMENTATION_SHA=79258eebaade6cfd739d4166864b4f81486eff9b

PROMOTION_IS_DEFAULT_PREFERENCE=YES
EXPLICIT_RECIPE_AUTHORITY=PRESERVED
PROJECT_EXPLICIT_BINDING_PRESERVED=YES
CURRENT_VERSION_AUTHORITY=PRESERVED
HISTORICAL_REFERENCE_IMMUTABILITY=PRESERVED
NO_NAME_GUESSING=YES
NO_AUTO_REBIND=YES
NO_AUTO_MIGRATION=YES

PROMOTION_AUTHORITY=WORKFLOW_REGISTRY
ONE_PROMOTION_CONSUMPTION_RULE=YES
WORKFLOW_RECIPE_PROMOTION_CONSUMPTION=PASS
```

## Implementation

`resolveImplicitWorkflowRecipe` is the single frontend fallback boundary for
Workflow Workspace actions. It consumes the Registry's exact promoted
`workflowVersionId + recipeId` only after the current workflow version is
resolved, then preserves the existing catalog semver fallback. Archived or
removed workflows, stale promotion metadata, cross-version metadata, and
missing catalog recipes remain safely unavailable or fall back without name
guessing.

The Workflow Workspace list and quick test both use this boundary. Explicit
project bindings, historical records, tasks, batches, presets, experiments,
and GenerationStudio selection remain exact-identity consumers and are not
rewritten.

## Verification

```text
FOCUSED_WORKFLOW_WORKSPACE_TESTS=PASS (21 tests)
FRONTEND_FULL_TESTS=PASS (146 files, 784 tests)
TSC=PASS
BUILD=PASS
ARCHITECTURE_GUARD=PASS
RUST_CHECK=PASS
RUST_TESTS=PASS (799 passed, 1 ignored)
TAURI_BUILD=PASS
DIFF_CHECK=PASS

RUST_SOURCE_CHANGE=NO
IPC_SURFACE_CHANGE=NO
DATABASE_CHANGE=NO
MIGRATION_CHANGE=NO
VISUAL_CHANGE=NO
REMOTE_CI_REQUIRED=NO
REMOTE_CI_RUN=NO
```

The architecture guard continues to pass all existing controller sentinels,
including `WORKFLOW_RECIPE_PROMOTION=PASS`, and now emits
`WORKFLOW_RECIPE_PROMOTION_CONSUMPTION=PASS`. GenerationStudio has no second
promotion state or resolution rule.

```text
DEV_091=PASS
DEV_092=NOT_STARTED
AUTO_NEXT_TASK=NO
AUTO_HANDOFF=NO
STOP=YES
```
