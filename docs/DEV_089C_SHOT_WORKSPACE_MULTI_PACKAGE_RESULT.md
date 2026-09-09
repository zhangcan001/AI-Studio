# DEV-089C — ShotWorkspace Multi-package Controller Result

## Result

```text
TASK=DEV-089C
PARENT_TASK=DEV-089
DEV-089C=PASS
DEV-089=OPEN
```

## Commits

```text
Baseline=e29d3b8298e9af9163b752c3f1d3f21cb657b4bd
Implementation=1e865e81b8d357c2d33841890a4745917ef1ac9b
Implementation message=refactor(shots): extract multi-package controller
Source-only CI=34319390971 GREEN
```

## Boundary

`useShotMultiPackageController` now owns the Multi-package production state and async lifecycle:

- root selection, discovery, per-package inspection, inspection progress, and discovery run-id guards;
- binding and batch-detail refresh with partial detail-error tolerance;
- refresh in-flight/pending coalescing;
- batch creation, pre-create reinspection, manifest identity validation, safety gates, bound-item exclusion, and partial/failure stop behavior;
- reinspection and live `boardPackages` projection;
- `multiPackageRunId`, `multiPackageRefreshInFlight`, `multiPackageRefreshPending`, and `multiPackageMounted` refs.

Pure projection and safety helpers moved to `shotMultiPackageModel.ts`.
`ShotWorkspace` remains the composition container for navigation and Queue/Monitor integration.

```text
ShotWorkspace lines: 2246 -> 1819
```

## Preserved behavior

```text
DISCOVERY_RUN_ID_GUARD=PASS
REFRESH_COALESCING=PASS
PARTIAL_DETAIL_TOLERANCE=PASS
MANIFEST_HASH_GUARD=PRESERVED
WARNING_SAFETY_GATE=PRESERVED
BLOCKED_SAFETY_GATE=PRESERVED
BOUND_ITEM_DEDUP=PRESERVED
PARTIAL_STOP=PRESERVED
FAILURE_STOP=PRESERVED
NO_AUTO_RETRY=YES
NO_AUTO_CREATE=YES
NO_AUTO_START=YES
BOARD_POLLING_OWNER=UNCHANGED
BOARD_POLL_INTERVAL=5000MS
TASK_EVENT_MULTI_PACKAGE_REFRESH=PRESERVED
QUEUE_CONTROLLER=UNCHANGED
MONITOR_CONTROLLER=UNCHANGED
```

The board remains the sole owner of the 5000ms timer, visibility guard, and polling coalescing. No second controller poller was introduced.

## Validation

```text
Focused frontend: PASS (58 tests)
Full frontend: PASS (132 files, 625 tests)
TypeScript: PASS
Frontend build: PASS
Architecture guard: PASS
Rust cargo check --all-targets: PASS
Rust cargo test --all-targets -- --test-threads=1: PASS
Tauri build: PASS
Implementation CI: GREEN
```

Architecture guard output includes:

```text
FRONTEND_NO_RAW_INVOKE=PASS
RPC_PARITY=PASS
SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION=PASS
SHOT_WORKSPACE_QUEUE_CONTROLLER=PASS
SHOT_WORKSPACE_MONITOR_CONTROLLER=PASS
SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER=PASS
```

## Frozen invariants

```text
RUST_CHANGE=NO
IPC_CHANGE=NO
DATABASE_CHANGE=NO
DATABASE_MIGRATION=NO
CSS_CHANGE=NO
NEW_STATE_SOURCE=NO
NEW_ZUSTAND=NO
NEW_QUEUE=NO
NEW_EXECUTOR=NO
NEW_TASK_MODEL=NO
QUEUE_REDESIGN=NO
MONITOR_REDESIGN=NO
VISUAL_CHANGE=NO
```

## Result status

```text
MULTI_PACKAGE_CONTROLLER=YES
MULTI_PACKAGE_STATE_OWNERSHIP=EXTRACTED
DEV-089C=PASS
DEV-089=OPEN
```
