# DEV-089D — AssetVideo Workflow Resolution Controller Result

## Result

```text
Baseline: 7eb8d380189402569ae50d0f66681526e5b938c7
Implementation: 3405573a8b086a5ed03461fe8996dca6c72da455
Source-only CI: 34350621561 GREEN

AssetVideoBatchWorkspace: 2147 -> 2031 lines
DEV-089D=PASS
DEV-089=OPEN
```

## Controller boundary

`useAssetVideoWorkflowController` now owns the video workflow resolution and
project-folder strategy state that previously lived in
`AssetVideoBatchWorkspace`:

- manual video selection;
- project workflow configuration;
- `AUTO` / `MANUAL` project workflow strategy;
- per-mode manual overrides;
- workflow selection notices.

The controller reuses the existing resolution authorities
`resolveProjectVideoWorkflow` and `resolveProjectFolderWorkflow`. It does not
create a second persistence source, store, resolver, or backend path.

## Preserved behavior

- Exact recipe identity remains `workflowVersionId + recipeId`.
- Resolution priority remains manual, project mode, project default,
  recommended, then compatible.
- Generic fallback remains disabled for the H3 video path.
- Stale manual selections are cleared with the existing compatibility notice.
- Stale project bindings remain non-persistent and show the existing notice;
  no automatic binding repair was added.
- Project changes reset manual selection, strategy, overrides, and notices
  before loading the new project configuration.
- `AUTO` ignores temporary manual overrides; `MANUAL` resolves exact per-mode
  overrides and removes undefined entries.
- Project-folder modes remain derived by the Workspace from local inspection;
  local import, batch execution, asset library, and generic workflow UI remain
  in their original owners.

## Validation

```text
Focused controller + AssetVideo tests: PASS (13 tests)
Asset/runtime focused suite: PASS (66 tests)
Full frontend: PASS (133 files, 633 tests)
TypeScript: PASS
Frontend build: PASS
Architecture guard: PASS
Rust check: PASS
Rust tests: PASS (796 passed, 1 ignored)
Tauri build: PASS
Source-only CI #34350621561: GREEN
```

Architecture guard output includes:

```text
FRONTEND_NO_RAW_INVOKE=PASS
RAW_INVOKE_OUTSIDE_TRANSPORT=0
RPC_PARITY=PASS
SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION=PASS
SHOT_WORKSPACE_QUEUE_CONTROLLER=PASS
SHOT_WORKSPACE_MONITOR_CONTROLLER=PASS
SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER=PASS
ASSET_VIDEO_WORKFLOW_CONTROLLER=PASS
```

## Frozen invariants

```text
New state source: NO
New Zustand: NO
Backend: UNCHANGED
IPC: UNCHANGED
Database: UNCHANGED
Migration: NO
Rust change: NO
CSS change: NO
ShotWorkspace change: NO
WorkflowWorkspace change: NO
GenerationStudio change: NO
Production behavior: UNCHANGED
Local import behavior: UNCHANGED
Asset library behavior: UNCHANGED
```

## Commits

```text
Implementation commit: 3405573a8b086a5ed03461fe8996dca6c72da455
Result commit: docs-only; recorded after CI validation
DEV-089D=PASS
DEV-089=OPEN
```
