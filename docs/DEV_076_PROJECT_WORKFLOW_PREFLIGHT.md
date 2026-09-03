# DEV-076 Project Workflow Preflight

## Closeout

```text
DEV076_START_SHA=82afb7c99057e95e32c9a00c4aecc2134475623e
DEV076_FINAL_SHA=d22c1087cd686a46681d42156844934b2c4b4fe6
BRANCH=master
WORKTREE_START=clean

MIGRATION_BEFORE=027
MIGRATION_AFTER=027
BACKUP_BEFORE=16
BACKUP_AFTER=16

PREFLIGHT_PERSISTED=NO
PREFLIGHT_SOURCE=DERIVED_RUNTIME_STATE

PRODUCTION_PATH_COUNT=8
IMAGE_PATH=SUPPORTED
VIDEO_MODE_COUNT=7

OVERALL_READY=SUPPORTED
OVERALL_PARTIAL=SUPPORTED
OVERALL_BLOCKED=SUPPORTED

STALE_BINDING_WARNING=SUPPORTED
FALLBACK_SOURCE_VISIBLE=YES

COMFY_RUNTIME_CHECK=OUT_OF_SCOPE
MEDIA_INPUT_CHECK=OUT_OF_SCOPE
PROMPT_CHECK=OUT_OF_SCOPE

SECOND_QUEUE=NO
SECOND_EXECUTOR=NO
AUTO_START_CHANGE=NO
AUTO_RETRY_CHANGE=NO

DEV075_P2_V15_TEST=CLOSED
DEV075_P2_DOCUMENTATION=CLOSED
```

The implementation commit is `d22c1087cd686a46681d42156844934b2c4b4fe6`. The final documentation commit is recorded in the Git section below.

## Implementation

- Added a pure `preflightProjectWorkflow` model for `IMAGE` plus the seven H3 modes: `FL2VA_TEXT_TO_VIDEO`, `FL2VA_IMAGE_TO_VIDEO`, `FL2VA_FIRST_LAST`, `REF2VA_IMAGE`, `REF2VA_AUDIO`, `REF2VA_IMAGE_AUDIO`, and `REF2VA_VIDEO_IMAGE`.
- Reused `filterImageRecipes`, `filterVideoRecipes`, `recipesForVideoMode`, `videoRecipeCapability`, `h3RecipeForMode`, the current H3 quality profile, and the existing image recommendation contract.
- Every item reports the resolved recipe, source, original configured reference, stale-binding state, fallback state, and a human-readable message. Overall status follows `READY` / `PARTIAL` / `BLOCKED` without treating a warning as a project failure.
- Added the `生产可用性` panel to the project page. It shows all eight paths, Chinese source labels, workflow display names, WorkflowVersion/Recipe identifiers, stale-binding warnings, blocked paths, and `重新检查`.
- `ProjectWorkflowSettings` sends the loaded and saved config to `ProjectWorkspace`, so a successful save updates the panel immediately. `重新检查` rereads the current project binding and recomputes against the current catalog without a new backend command.
- Video resolution now supports a strict mode-only path for preflight and H3 production views. Existing callers retain the previous generic-video fallback behavior unless they opt into strict mode resolution.
- Added an explicit V15 archive test through `ProjectBackupService.inspect` and `restore`; the restored project is new and has zero project workflow bindings. DEV-075 documentation now records the actual final SHA and all P0-P3 values.

No preflight result is persisted. No migration, backup format, project manifest, queue, executor, task schema, package schema, auto-start, retry, or ComfyUI runtime behavior was added or changed.

## Resolution consistency

The project preflight passes `explicit = undefined` but calls the same project resolvers used by production paths. The priority remains:

```text
explicit > project_mode > project_default > recommended > compatible
```

Generation Studio continues to resolve image defaults from the same project configuration. `AssetVideoBatchWorkspace` now uses strict H3 mode candidates and the same project video resolver, so a mode-specific project override cannot be replaced by a generic video recipe at production time.

## Verification evidence

```text
Targeted preflight: 2 files, 11 tests passed
Targeted project workflow set: 4 files, 20 tests passed
ProjectWorkflowSettings: 5 tests passed
projectWorkflowResolution: 4 tests passed
AssetVideoBatchWorkspace/workflowCapabilities: 2 files, 10 tests passed

V15 explicit restore: 1 passed, 0 failed
Project workflow Rust tests: 9 passed, 0 failed
DEV-055 backup roundtrip: 1 passed, 0 failed

Rust full serial: 709 passed, 0 failed, 1 ignored
Frontend full suite: 105 test files, 467 tests passed
cargo fmt: PASS
cargo check --all-targets: PASS
TypeScript: PASS
pnpm build: PASS (218 modules transformed)
git diff --check: PASS
pnpm tauri build: PASS
```

The build emitted only the existing minified chunk-size warning (`index-BdFPOWdg.js` is about 1.08 MB) and existing Rust dead-code warnings.

Generated release artifacts:

- `src-tauri/target/release/ai-studio.exe`
- `src-tauri/target/release/bundle/msi/AI Studio_1.0.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/AI Studio_1.0.0_x64-setup.exe`

## Manual UX

Formal AI Studio 1.0.0 startup was checked successfully. The project workspace, existing project content, and `ComfyUI 已连接` state were readable.

Cases A-G were not claimed as passed. The available Windows automation can read the embedded WebView accessibility tree, but clicking its project-navigation controls consistently failed with `SetIsBorderRequired failed: 不支持此接口 (0x80004002)` / unknown click outcome. Therefore the following remain for a human operator in the running app:

```text
Case A = NOT_RUN — interactive WebView input unavailable
Case B = NOT_RUN — interactive WebView input unavailable
Case C = NOT_RUN — interactive WebView input unavailable
Case D = NOT_RUN — interactive WebView input unavailable
Case E = NOT_RUN — interactive WebView input unavailable
Case F = NOT_RUN — interactive WebView input unavailable
Case G = NOT_RUN — interactive WebView input unavailable
```

This is an environment-only P3 limitation, not a product defect inferred from the automated tests.

## Git

```text
DEV076_FINAL_SHA=d22c1087cd686a46681d42156844934b2c4b4fe6
COMMIT=feat(projects): add workflow production preflight
PUSH=origin/master
WORKTREE_END=clean after closeout documentation commit
```

## Issues

```text
P0=NONE
P1=NONE
P2=NONE
P3=MANUAL_INTERACTIVE_NOT_RUN
```

## Final

```text
DEV076_PROJECT_WORKFLOW_PREFLIGHT=BLOCKED
```

The implementation and automated gates are complete. The final DEV gate remains blocked only because the mandatory human Cases A-G could not be honestly completed through the available WebView automation; no product failure was observed in the startup check.
