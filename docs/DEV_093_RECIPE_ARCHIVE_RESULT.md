# DEV-093 Recipe Archive Result

TASK=DEV-093
BASELINE_SHA=0703e4a3690669ea08a9bbc582ca18ab00ad97a8
IMPLEMENTATION_SHA=3277c4d78d6756ac6d37c8d53dc6ff131a905298
REMOTE_CI_RUN=#117
REMOTE_CI_RUN_ID=34560064669
REMOTE_CI_HEAD=3277c4d78d6756ac6d37c8d53dc6ff131a905298
REMOTE_CI_STATUS=GREEN

## Scope

Recipe archive/restore was implemented without mutating immutable recipe rows. Runtime archive state is stored separately by the exact (workflow_version_id, recipe_id) identity.

## Persistence and authority

- Migration 031_workflow_recipe_archive_state.sql adds workflow_recipe_runtime_states.
- The composite primary key and foreign key enforce exact workflow-version/recipe membership.
- Missing runtime-state rows mean active; explicit archived state is the only archive marker.
- WorkflowRecipeRuntimeStateRepository is the sole persistence port for this state.
- WorkflowRegistryService::is_available remains the availability authority and includes recipe archive state.
- Application production code does not add direct SQLx access.

## Archive and restore semantics

- Archive and restore are exposed through typed workflow_archive_recipe and workflow_restore_recipe IPC commands.
- Archive and restore require the exact workflow version and recipe pair.
- A promoted recipe cannot be archived until its promotion is explicitly cleared.
- The last active recipe of a workflow version cannot be archived.
- Restore reactivates only the exact recipe; it does not promote it, make it current, restore bindings, change history, or change runtime packages.
- Repeated archive/restore operations remain explicit and safe.
- Structured IPC errors preserve stable recipe lifecycle error codes and exact IDs in details; the frontend does not parse error strings.

## Registry and resolution

- Registry recipe views expose archived and archivedAt.
- Historical recipe lists retain archived recipes.
- currentRecipe selects only a promoted, non-archived recipe, then the newest non-archived recipe.
- Implicit frontend resolution excludes archived recipes and preserves exact workflow version plus recipe identity.
- Built-in and imported workflow metadata remain immutable; archive state is runtime-only.

## Execution and admission safety

- Direct generation rejects archived recipes through the existing generation admission path.
- Production queue creation and queue start reject archived recipes before persistence or execution.
- Existing queue, executor, task model, production binding, preset, batch, experiment, shot, and project-workflow semantics remain unchanged.
- No automatic rebind, promotion, queue start, executor, or task creation was added.
- No new queue, executor, task model, or competing state source was introduced.

## IPC and frontend

- Added typed archiveWorkflowRecipe and restoreWorkflowRecipe transport methods.
- Workflow Workspace displays archived state, disables promotion for archived recipes, and provides archive/restore actions.
- Existing typed transport and structured error localization remain the only frontend boundary.

## Verification

Local gates:

- pnpm test: PASS — 146 files, 788 tests.
- pnpm exec tsc --noEmit: PASS.
- pnpm build: PASS.
- cargo fmt --manifest-path src-tauri/Cargo.toml -- --check: PASS.
- cargo check --manifest-path src-tauri/Cargo.toml --all-targets: PASS.
- cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1: PASS — 805 passed, 0 failed, 1 ignored.
- pnpm tauri build: PASS — MSI and NSIS bundles produced.
- node scripts/dev088-architecture-guard.mjs: PASS.
- git diff --check: PASS.
- Focused Workflow Workspace and recipe archive/structured IPC tests: PASS.

Architecture sentinels include:

~~~text
STRUCTURED_IPC_ERROR=PASS
RECIPE_ARCHIVE_AUTHORITY=PASS
ONE_RECIPE_ARCHIVE_STATE_SOURCE=PASS
ONE_IMPLICIT_RECIPE_RESOLVER=PASS
APPLICATION_DIRECT_SQLX_NEW_USAGE=0
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
WORKFLOW_SMART_IMPORT_CONTROLLER=PASS
WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER=PASS
~~~

Remote Source-only CI #117:

- Rust source checks: PASS.
- Frontend source checks: PASS.
- Overall remote CI: GREEN.

## Frozen invariants

~~~text
EXACT_RECIPE_IDENTITY=workflowVersionId+recipeId
IMMUTABLE_RECIPE_ROWS=PRESERVED
ARCHIVE_STATE=DEDICATED_RUNTIME_STATE
MISSING_STATE=ACTIVE
PROMOTED_ARCHIVE_GUARD=PRESERVED
LAST_ACTIVE_RECIPE_GUARD=PRESERVED
RESTORE_DOES_NOT_PROMOTE=YES
IMPLICIT_RESOLUTION_EXCLUDES_ARCHIVED=YES
DIRECT_GENERATION_ARCHIVED_GUARD=PASS
QUEUE_ARCHIVED_GUARD=PASS
STRUCTURED_IPC_ERROR=YES
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
NO_AUTO_REBIND=YES
NO_AUTO_PROMOTION=YES
NO_AUTO_START=YES
RUST_SOURCE_CHANGE=YES
IPC_SURFACE_CHANGE=YES
DATABASE_SCHEMA_CHANGE=YES
DATABASE_MIGRATION=031_workflow_recipe_archive_state.sql
VISUAL_CHANGE=NO
~~~

DEV-093=PASS
DEV-094=NOT_STARTED
AUTO_NEXT_TASK=NO
AUTO_HANDOFF=NO
