# DEV-075 Project Workflow Binding

## Closeout

```text
DEV075_START_SHA=8e4ed13f5f809fc1725f20a35b95dac6e7001750
DEV075_FINAL_SHA=82afb7c99057e95e32c9a00c4aecc2134475623e
BRANCH=master
WORKTREE_START=clean

P0 = NONE
P1 = NONE
P2 = NONE
P3 = NONE
DEV075_PROJECT_WORKFLOW_BINDING = PASS

MIGRATION_BEFORE=026
MIGRATION_AFTER=027
BACKUP_BEFORE=15
BACKUP_AFTER=16

PROJECT_WORKFLOW_SOURCE_OF_TRUTH=DATABASE
LEGACY_LOCAL_STORAGE=MIGRATION_ONLY
IMAGE_DEFAULT=SUPPORTED
VIDEO_DEFAULT=SUPPORTED
VIDEO_MODE_OVERRIDE=SUPPORTED
STALE_BINDING_SILENT_REPLACEMENT=NO
PRODUCTION_PACKAGE_OVERRIDE=NO
EXISTING_TASK_MUTATION=NO
SECOND_QUEUE=NO
SECOND_EXECUTOR=NO
AUTO_START=NO
AUTO_RETRY=NO
```

## Implementation

- Migration `027_project_workflow_bindings.sql` adds the 56th table. The primary key is `(project_id, stage, mode)`, project deletion cascades, and workflow/recipe references remain soft.
- `ProjectWorkflowBindingRepository` replaces a project’s complete binding set inside one transaction. The service validates project, stage, mode, duplicate keys, workflow version, recipe ownership, current version, and runtime availability with stable `PROJECT_WORKFLOW_*` error prefixes.
- `project_workflow_config_get` and `project_workflow_config_replace` are registered in the existing project command surface. `ProjectView` remains basic project metadata.
- The project page provides explicit-save image/video defaults and seven video-mode overrides. Stale references remain visible with their original WorkflowVersion/Recipe IDs and can be reselected or cleared.
- The pure resolver uses `explicit > project_mode > project_default > recommended > compatible`. Generation Studio and video/project-folder paths read the database configuration; transient manual choices do not write project defaults.
- Existing frozen task, batch, queue, and Production Package references are not rewritten or overridden.
- Backup format V16 includes project workflow bindings, remaps the restored project ID, preserves stale soft references, reports missing workflow dependencies, and accepts V15 documents without the new field.

## Verification evidence

Targeted Rust:

```text
cargo test ... project_workflow -- --test-threads=1
9 passed, 0 failed
cargo test ... backup_round_trip_creates_new_project_and_keeps_asset_bytes -- --test-threads=1
1 passed, 0 failed
cargo test ... --test dev055_release_compatibility dev055_backup_16_roundtrip -- --test-threads=1
1 passed, 0 failed
```

Targeted frontend:

```text
pnpm test -- projectWorkflowResolution ProjectWorkflowSettings workflowCapabilities
3 files, 12 tests passed
pnpm exec tsc --noEmit
PASS
```

Current complete serial gate results:

```text
Rust full serial: 709 passed, 0 failed, 1 ignored
Frontend: 103 test files, 454 tests passed
TypeScript: PASS
pnpm build: PASS (216 modules transformed; existing >500 KB chunk warning only)
git diff --check: PASS
pnpm tauri build: PASS
Version: 1.0.0
Migration: 027
Backup format: V16
Tables: 56
```

The explicit V15 compatibility test creates a V15 archive whose project document has no `projectWorkflowBindings` field, then verifies that the current `ProjectBackupService` can inspect and restore it into a new project with zero bindings and no compatibility error.

## UX acceptance scope

The implementation supports database persistence across project-page remounts, image/video defaults, per-mode overrides with fallback, explicit clearing, stale-binding warnings without silent persistence, V16 round-trip restoration, and V15 restore compatibility. No real H3 generation is required for this configuration-only DEV.
