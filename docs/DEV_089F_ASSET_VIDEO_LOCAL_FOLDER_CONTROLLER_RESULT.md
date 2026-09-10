# DEV-089F — AssetVideoBatch Local Project Folder Session Controller Result

## Result

```text
TASK=DEV-089F
DEV-089F=PASS
DEV-089=OPEN
BASELINE=9a69a1dea1fe19d7d0aed357c974fd767fc5216e
IMPLEMENTATION=0057d2d2553e63e9bc800d685485977c33ff8c8a
IMPLEMENTATION_CI=GREEN
CI_RUN=https://github.com/zhangcan001/AI-Studio/actions/runs/34426977946
```

`AssetVideoBatchWorkspace.tsx` changed from `1907` to `1835` lines. The Workspace remains the owner of shared busy/notice state, project-mode composition, selection, prompts, workflow resolution, and production commit orchestration.

## Controller ownership

Added `src/features/assets/hooks/useAssetVideoLocalImportController.ts` and moved the Local Project Folder session lifecycle into it:

- local inspection and project segment forms
- batch name, auto-start preference, and expanded segment ordinal
- directory selection and rescan
- segment form updates, save, and reset
- session cleanup after commit success or failure
- existing backend inspection authority and exact transport payloads

The controller continues to use the existing `pickH3LocalImportDirectory`, `rescanH3LocalImport`, and `updateH3ProjectSegmentDraft` transport wrappers. `commitH3LocalImport` preserves the existing composition inputs, eligibility policy, batch identity, recipe, quality, reference, and auto-start semantics. Shared busy/notice and production admission remain composed by the Workspace through callbacks.

## Validation

```text
Focused controller + workspace + local-import tests: PASS (3 files, 13 tests)
Full frontend tests: PASS (135 files, 646 tests)
TypeScript: PASS
Frontend build: PASS
Architecture guard: PASS
Rust check: PASS
Rust tests: PASS
Tauri build: PASS
Implementation CI: GREEN (run 34426977946)
```

Architecture guard sentinels include:

```text
FRONTEND_NO_RAW_INVOKE=PASS
RPC_PARITY=PASS
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
LOCAL_INSPECTION_OWNERSHIP=EXTRACTED
PROJECT_SEGMENT_FORM_OWNERSHIP=EXTRACTED
PICK_DIRECTORY=PRESERVED
RESCAN=PRESERVED
SEGMENT_EDIT=PRESERVED
SEGMENT_SAVE=PRESERVED
SEGMENT_RESET=PRESERVED
BACKEND_INSPECTION_AUTHORITY=PRESERVED
LOCAL_IMPORT_COMMIT_PAYLOAD=PRESERVED
COMMIT_ORCHESTRATION=WORKSPACE_COMPOSED
SHARED_BUSY_NOTICE=WORKSPACE_OWNED
NO_AUTO_SAVE=YES
NO_AUTO_RESCAN=YES
NO_AUTO_COMMIT=YES
NO_NEW_STATE_SOURCE=YES
NO_NEW_ZUSTAND=YES
WORKFLOW_BEHAVIOR=UNCHANGED
ASSET_LIBRARY_BEHAVIOR=UNCHANGED
PROMPT_BEHAVIOR=UNCHANGED
PRODUCTION_BEHAVIOR=UNCHANGED
RUST_CHANGE=NO
IPC_CHANGE=NO
DATABASE_CHANGE=NO
DATABASE_MIGRATION=NO
CSS_CHANGE=NO
```

## Commits

```text
IMPLEMENTATION_COMMIT=0057d2d2553e63e9bc800d685485977c33ff8c8a
RESULT_COMMIT=MARKDOWN_ONLY
```
