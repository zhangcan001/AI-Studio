# DEV-089E — AssetVideoBatch Asset Library Query Controller Result

## Result

```text
TASK=DEV-089E
DEV-089E=PASS
DEV-089=OPEN
BASELINE=e70b78004f9b200a708fa1bdd3d09539c459da8b
IMPLEMENTATION=3d1f35b2dfdf6bda84d3858fe11b47b2cf5a5fbb
IMPLEMENTATION_CI=GREEN
CI_RUN=https://github.com/zhangcan001/AI-Studio/actions/runs/34359008756
```

`AssetVideoBatchWorkspace.tsx` changed from `2032` to `1977` lines. The workspace remains the owner of asset selection, first/last-frame selection, prompts, local import, workflow resolution, and batch execution.

## Controller ownership

Added `src/features/assets/hooks/useAssetVideoLibraryController.ts` and moved the Asset Library query lifecycle into it:

- available assets and ID-based page merging
- keyword input and the existing 300ms debounce
- media filter, cursor, loading, and user-facing error state
- request-version stale success/error protection
- source-mode enable/disable lifecycle and project reset
- refresh and guarded pagination
- authoritative full-default-page reconciliation through the Workspace callback

The controller continues to call the existing `assetLibraryPage` transport with the unchanged query contract: `category=ALL`, trimmed keyword, selected media type, `sourceKind=ALL`, `createdOrder=NEWEST`, requested cursor, and `limit=30`.

Selected assets remain preserved across filtered and paginated queries. Only a reset request with an empty keyword, `mediaType=ALL`, and no `nextCursor` replaces the library list and invokes the Workspace reconciliation callback.

## Validation

```text
Focused controller + workspace tests: PASS (2 files, 13 tests)
Full frontend tests: PASS (134 files, 641 tests)
TypeScript: PASS
Frontend build: PASS
Architecture guard: PASS
Rust check: PASS
Rust tests: PASS (796 passed, 1 ignored)
Tauri build: PASS
Implementation CI #103: GREEN
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
```

## Frozen invariants

```text
KEYWORD_DEBOUNCE_300MS=PRESERVED
QUERY_CONTRACT=PRESERVED
REQUEST_VERSION_GUARD=PASS
STALE_SUCCESS_IGNORED=YES
STALE_ERROR_IGNORED=YES
PAGINATION=PRESERVED
ASSET_ID_DEDUP=PRESERVED
SELECTED_ASSET_QUERY_PRESERVATION=YES
FULL_DEFAULT_PAGE_RECONCILIATION=PRESERVED
FILTERED_QUERY_DOES_NOT_PRUNE_SELECTION=YES
PROJECT_SWITCH_RESET=PRESERVED
NO_NEW_POLLING=YES
NO_NEW_CACHE=YES
NO_NEW_STATE_SOURCE=YES
NO_NEW_ZUSTAND=YES
WORKFLOW_BEHAVIOR=UNCHANGED
LOCAL_IMPORT_BEHAVIOR=UNCHANGED
PROMPT_BEHAVIOR=UNCHANGED
BATCH_BEHAVIOR=UNCHANGED
RUST_CHANGE=NO
IPC_CHANGE=NO
DATABASE_CHANGE=NO
DATABASE_MIGRATION=NO
CSS_CHANGE=NO
```

## Commits

```text
IMPLEMENTATION_COMMIT=3d1f35b2dfdf6bda84d3858fe11b47b2cf5a5fbb
RESULT_COMMIT=MARKDOWN_ONLY
```
