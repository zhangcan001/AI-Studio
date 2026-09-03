# DEV-076 Project Workflow Preflight

## Closeout

```text
DEV076_START_SHA=82afb7c99057e95e32c9a00c4aecc2134475623e
DEV076_CLOSEOUT_START_SHA=29998dd36406cd37e519cd8d95a91db1e3f02f21
DEV076_IMPLEMENTATION_SHA=d22c1087cd686a46681d42156844934b2c4b4fe6
DEV076_CLOSEOUT_CODE_SHA=4851cb745ab966240d5912ab62423c54cfa2da3c
DEV076_FINAL_SHA=4851cb745ab966240d5912ab62423c54cfa2da3c
DEV076_CLOSEOUT_DOC_SHA=RECORDED_IN_GIT_HISTORY
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

STRICT_H3_MODE_RESOLUTION=YES
H3_GENERIC_VIDEO_FALLBACK=NO
CUSTOM_VIDEO_GENERIC_PATH=SUPPORTED
PROJECT_PREFLIGHT_MATCHES_H3_PRODUCTION=YES
ASSET_VIDEO_BATCH_WORKSPACE_STRICT=YES
PROJECT_FOLDER_STRICT=YES
GENERIC_VIDEO_FALLBACK_PRESERVED=YES

UAT_METHOD=DETERMINISTIC_UI_INTEGRATION
DETERMINISTIC_UI_UAT=PASS
CASE_A=PASS
CASE_B=PASS
CASE_C=PASS
CASE_D=PASS
CASE_E=PASS
CASE_F=PASS
CASE_G=PASS
MANUAL_WEBVIEW_AUTOMATION=UNAVAILABLE
MANUAL_INTERACTIVE_NOT_RUN=CLOSED_BY_DETERMINISTIC_UI_UAT
SETTINGS_CONFIG_CALLBACK=PASS
PREFLIGHT_REFRESH=PASS

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

The implementation commit is `d22c1087cd686a46681d42156844934b2c4b4fe6`. The closeout-code and final SHAs are `4851cb745ab966240d5912ab62423c54cfa2da3c`; the documentation-closeout SHA remains `RECORDED_IN_GIT_HISTORY` to avoid a self-referential value inside the document.

## Implementation

- Added a pure `preflightProjectWorkflow` model for `IMAGE` plus the seven H3 modes: `FL2VA_TEXT_TO_VIDEO`, `FL2VA_IMAGE_TO_VIDEO`, `FL2VA_FIRST_LAST`, `REF2VA_IMAGE`, `REF2VA_AUDIO`, `REF2VA_IMAGE_AUDIO`, and `REF2VA_VIDEO_IMAGE`.
- Reused `filterImageRecipes`, `filterVideoRecipes`, `recipesForVideoMode`, `videoRecipeCapability`, `h3RecipeForMode`, the current H3 quality profile, and the existing image recommendation contract.
- Every item reports the resolved recipe, source, original configured reference, stale-binding state, fallback state, and a human-readable message. Overall status follows `READY` / `PARTIAL` / `BLOCKED` without treating a warning as a project failure.
- Added the `生产可用性` panel to the project page. It shows all eight paths, Chinese source labels, workflow display names, WorkflowVersion/Recipe identifiers, stale-binding warnings, blocked paths, and `重新检查`.
- `ProjectWorkflowSettings` sends the loaded and saved config to `ProjectWorkspace`, so a successful save updates the panel immediately. `重新检查` rereads the current project binding and recomputes against the current catalog without a new backend command.
- Video resolution now supports a strict mode-only path for preflight and H3 production views. Existing callers retain the previous generic-video fallback behavior unless they opt into strict mode resolution.
- Added an explicit V15 archive test through `ProjectBackupService.inspect` and `restore`; the restored project is new and has zero project workflow bindings. DEV-075 documentation remains unchanged and records its existing final SHA and P0-P3 closure.

No preflight result is persisted. No migration, backup format, project manifest, queue, executor, task schema, package schema, or runtime-start architecture was added or changed. Auto-start and auto-retry remain unchanged, and ComfyUI health, GPU, media-input, and prompt checks remain out of scope.

One workflow-selection rule was intentionally tightened: explicit H3 modes now require an exact compatible recipe and no longer fall back to an arbitrary generic `CUSTOM_VIDEO` recipe. Generic `CUSTOM_VIDEO` remains supported for generic video flows.

## Resolution consistency

The project preflight passes `explicit = undefined` but calls the same project resolvers used by production paths. The priority remains:

```text
explicit > project_mode > project_default > recommended > compatible
```

Generation Studio continues to resolve image defaults from the same project configuration. `AssetVideoBatchWorkspace` now uses strict H3 mode candidates and the same project video resolver, so a mode-specific project override cannot be replaced by a generic video recipe at production time. Project-folder mode resolution remains strict through its existing `projectFolderModes` filtering.

## Verification evidence

```text
Baseline preflight model/UI: 2 files, 11 tests passed (8 pure model tests, 3 UI component tests)
Deterministic closeout UAT: ProjectWorkflowUat 7 tests passed (Cases A-G)
Focused closeout UI/runtime: 5 files, 33 tests passed
  ProjectWorkflowPreflight 4; ProjectWorkflowSettings 6; projectWorkflowPreflight 9; projectWorkflowResolution 7
AssetVideoBatchWorkspace/workflowCapabilities: 2 files, 10 tests passed

V15 explicit restore: 1 passed, 0 failed
Project workflow Rust tests: 9 passed, 0 failed
DEV-055 backup roundtrip: 1 passed, 0 failed

Rust full serial: library 709 passed, 0 failed, 1 ignored; all integration targets passed
Frontend full suite: 106 test files, 480 tests passed
cargo fmt: PASS
cargo check --all-targets: PASS
TypeScript: PASS
pnpm build: PASS (218 modules transformed)
git diff --check: PASS
pnpm tauri build: PASS (release exe, MSI, and NSIS bundles)
```

The build emitted only the existing minified chunk-size warning (`index-CoJo_oLV.js` is about 1.08 MB) and existing Rust dead-code warnings.

The closeout UAT uses the dedicated `ProjectWorkflowUat.test.tsx` harness, which mounts the real `ProjectWorkflowSettings` and `ProjectWorkflowPreflight` components together and mocks only the Tauri client boundary. Cases A-G therefore exercise the live settings callback, save path, preflight panel, strict resolver, stale-binding display, and no-remount update behavior. All seven cases passed in the full frontend suite.

Generated release artifacts:

- `src-tauri/target/release/ai-studio.exe`
- `src-tauri/target/release/bundle/msi/AI Studio_1.0.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/AI Studio_1.0.0_x64-setup.exe`

## Manual UX

Formal AI Studio 1.0.0 startup was checked successfully. The project workspace, existing project content, and `ComfyUI 已连接` state were readable. This remains a startup/readability check only; the closeout A-G gate is recorded through deterministic UI integration UAT.

The available Windows automation can read the embedded WebView accessibility tree, but clicking its project-navigation controls consistently failed with `SetIsBorderRequired failed: 不支持此接口 (0x80004002)` / unknown click outcome. Manual WebView interaction was therefore unavailable; the required equivalent was executed as deterministic React UI integration UAT with real Settings/Preflight components and only Tauri APIs mocked:

```text
Case A = PASS — empty config with compatible catalog is not BLOCKED
Case B = PASS — saving image default A updates the panel
Case C = PASS — video default applies only to supported modes
Case D = PASS — mode override wins over video default
Case E = PASS — stale override warns, shows IDs, falls back visibly, and does not persist
Case F = PASS — strict H3 mode blocks despite generic CUSTOM_VIDEO availability
Case G = PASS — image default A to B updates without remount
```

Manual WebView automation remains an environment limitation, not a product defect. The deterministic UI integration UAT is the recorded closeout equivalent for this gate.

## Git

```text
DEV076_IMPLEMENTATION_SHA=d22c1087cd686a46681d42156844934b2c4b4fe6
DEV076_CLOSEOUT_CODE_SHA=4851cb745ab966240d5912ab62423c54cfa2da3c
DEV076_FINAL_SHA=4851cb745ab966240d5912ab62423c54cfa2da3c
DEV076_CLOSEOUT_DOC_SHA=RECORDED_IN_GIT_HISTORY
CODE_COMMIT=fix(projects): freeze strict H3 workflow resolution
DOC_COMMIT=docs: close DEV-076 workflow preflight
PUSH=origin/master
WORKTREE_END=clean
```

The implementation SHA is recorded from the supplied baseline. `DEV076_FINAL_SHA` intentionally points to the closeout-code commit rather than this document's later commit; the actual documentation-closeout SHA is reported from Git history after this commit.

## Issues

```text
P0=NONE
P1=NONE
P2=NONE
P3=NONE
P0=P1=P2=P3=NONE
```

## Final

```text
DEV076_PROJECT_WORKFLOW_PREFLIGHT=PASS
```

The DEV-076 closeout is PASS. Strict H3 mode resolution is frozen, generic `CUSTOM_VIDEO` remains available only through the generic/legacy path, the deterministic UI integration equivalent passed Cases A-G, all regression gates passed, and no P0-P3 issue remains.
