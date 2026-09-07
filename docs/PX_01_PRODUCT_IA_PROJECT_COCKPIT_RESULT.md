# PX-01 — Product IA & Project Cockpit Result

## Delivery

```text
TASK=PX-01
BASELINE_HEAD=e7309270ae66191e4d384632946336f2cdef19ed
IMPLEMENTATION_COMMIT=4a808d8
FRONTEND_ONLY=YES
```

Entry gate was closed before PX-01 changes: the DEV-085 Source-only CI run passed both `Frontend source checks` and `Rust source checks` on the expected baseline.

## Product IA

```text
DEFAULT_GLOBAL_RAIL_ITEMS=7
DUPLICATE_ANALYSIS_ENTRY=REMOVED_FROM_DEFAULT_UI
DEFAULT_GLOBAL_RAIL=项目, 创作, 资产, 生产, 审核, 工作流, 设置
ANALYSIS_ROUTE_COMPATIBILITY=YES
```

The internal `StudioRailItemId = "analysis"` and `studioRouteForSection("analysis")` route remain available for compatibility. The duplicate analysis item is no longer rendered in the default rail.

The user-facing Project Command Center is now presented as `项目总览`. The first-level project view emphasizes project name, overall progress, review workload, issues, runtime status, and the recommended next action. Project ID remains internal and is not shown in the first-screen hero.

## Project Cockpit

```text
PROJECT_COCKPIT=PASS
FIRST_SCREEN_STATUS_LAYERS=PROGRESS, REVIEW, ISSUES, RUNTIME
EMPTY_STATE=PASS
LOADING_STATE=PASS
ERROR_STATE=PASS
500_SHOT_SUMMARY=PASS
```

The existing `ProjectCommandCenterAggregate` remains the single source for aggregate progress, review counts, issue summaries, runtime readiness, scene summaries, recent activity, and recommendations. No dashboard service, state store, or per-shot cockpit card list was added.

An active empty project exposes one primary `开始创作` action. A missing active project keeps the existing `管理项目` entry.

## Continue Work and navigation

```text
CONTINUE_WORK_TARGETED_NAVIGATION=PASS
CONTINUE_WORK_SIDE_EFFECTS=NONE
WORKSPACE_RESUME_COMPATIBLE=YES
```

`ProjectCommandCenterNavigationRequest` carries `destination`, optional `section`, `shotId`, `batchId`, and `actionKind`. `App.tsx` resolves the request through existing `activeStudioSection`, `resumeShotId`, `focusedProductionBatchId`, `navigateToStudioSection`, `navigateToWorkspace`, and `workspaceResumeStore` behavior.

The action matrix is deterministic:

| Action kind | Destination | Focus |
| --- | --- | --- |
| `STRUCTURAL_BLOCKED` | 创作 | project structure route |
| `COMFY_BLOCKED` | 设置 | runtime settings |
| `REVIEW_REQUIRED` | 生产 | review-required batch when available |
| `AUTO_RESUMABLE` | 生产 | auto-resumable batch when available |
| `ACTIVE_PRODUCTION` | 生产 | active batch when available |
| `IMAGE_REVIEW` / `VIDEO_REVIEW` | 审核 | review shot when available |
| `MISSING_CONFIG` / `UNASSIGNED` | 创作 | target shot when available |
| `NO_SHOTS` | 创作 | first-shot entry |
| `READY` | 生产 | `firstReadyShotId` when available |
| `COMPLETE` | 创作 | new creative round |

Navigation does not start, resume, retry, regenerate, enqueue, or mutate production state.

## Verification

```text
FOCUSED_FRONTEND=PASS (4 files, 35 tests)
FULL_FRONTEND=PASS (119 files, 576 tests)
TSC=PASS
FRONTEND_BUILD=PASS
RUST_CHECK=PASS (cargo check --manifest-path src-tauri/Cargo.toml --all-targets)
```

The build emitted only the repository's existing chunk-size and Rust dead-code warnings; no PX-01 failure was reported.

Remote Source-only CI status is recorded after the implementation push.

## Frozen invariants

```text
BACKEND_CHANGE=NO
DATABASE_MIGRATION=NO
BACKUP_SCHEMA_CHANGE=NO
IPC_CHANGE=NO
PRODUCTION_BEHAVIOR_CHANGE=NO
WORKFLOW_LIFECYCLE_CHANGE=NO
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
UNRELATED_DIFF=NO
```

No Rust, database, backup, IPC, production queue, executor, task model, or workflow lifecycle files were changed.
