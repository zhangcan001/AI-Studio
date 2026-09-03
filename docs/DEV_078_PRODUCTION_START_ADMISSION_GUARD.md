# DEV-078 Production Start Admission Guard

DEV-078 将正式生产启动的最终判断收口到后端准入服务。`production_queue_start` 仍是唯一公开启动命令；服务在同一个既有 Queue admission mutex 内重新加载 Batch，先执行现有 state/busy 检查，再按 Batch item 冻结的 Workflow Version + Recipe 做当前 Runtime 检查，最后提交 `RUNNING` 并进入既有 dispatch。

## Frozen verification markers

```text
DEV078_START_SHA=eae1a0be742825e0a3ca0522a6fe842264011330
BRANCH=master
WORKTREE_START=clean

MIGRATION_BEFORE=027
MIGRATION_AFTER=027
BACKUP_BEFORE=16
BACKUP_AFTER=16

PRODUCTION_START_ADMISSION=BACKEND_AUTHORITATIVE
BATCH_FROZEN_WORKFLOW_TRUTH=YES
CURRENT_PROJECT_BINDING_USED_FOR_EXISTING_BATCH_START=NO
FRONTEND_READINESS_TRUSTED_FOR_START=NO
BACKEND_RUNTIME_RECHECK_ON_EVERY_START=YES

EXISTING_ADMISSION_MUTEX_REUSED=YES
SECOND_ADMISSION_MUTEX=NO
COMFY_SERVICE_REUSED=YES
WORKFLOW_LIFECYCLE_REUSED=YES
COMFY_PREFLIGHT_SERVICE_DEPENDENCY=NO

PENDING_ITEMS_ONLY_RUNTIME_CHECK=YES
EXACT_RUNTIME_MATCH=WORKFLOW_VERSION_ID_PLUS_RECIPE_ID
DEGRADED_WITH_VALID_CAPABILITY=ALLOW
UNRELATED_RUNTIME_WORKFLOW_BLOCKS_BATCH=NO
START_RUNTIME_ADMISSION_FAIL_CLOSED=YES
PRODUCTION_START_ADMISSION_PERSISTED=NO

NEW_START_COMMAND=NO
EXISTING_PRODUCTION_QUEUE_START_REUSED=YES
AUTO_START_CHANGED=NO
AUTO_RETRY=NO
SECOND_QUEUE=NO
SECOND_EXECUTOR=NO
PREEXISTING_AUTO_START_ON_CREATE_MISMATCH=CONFIRMED_PREEXISTING

DEV078_CODE_SHA=0648a052a22878a439ff7ba3c58fe78921beb442
DEV078_FINAL_SHA=0648a052a22878a439ff7ba3c58fe78921beb442
DEV078_CLOSEOUT_DOC_SHA=RECORDED_IN_GIT_HISTORY
```

## Implementation

- Added `ProductionStartAdmissionService` with only `Arc<ProductionQueueService>`, `Arc<ComfyService>`, and `Arc<WorkflowLifecycleService>` dependencies.
- Reused the existing Queue admission gate. Queue start was split into lock-free `inspect_start_admitted` and `commit_start_admitted`; commit verifies the batch update before spawning.
- Runtime checks are fail-closed for Comfy unavailable/incompatible, capability refresh failure, diagnostics failure, missing/archived/disabled workflow versions, invalid or missing packages, missing exact recipes, non-ready capabilities, missing nodes, incompatible inputs, and blocking diagnostics.
- `DEGRADED` caused only by absent successful-run history remains admissible for the first real run. VRAM thresholds and unrelated workflow status are not used.
- Runtime rejection returns top-level `PRODUCTION_START_ADMISSION_BLOCKED` with structured workflow version, recipe, reason, and missing-node details. Frontend error formatting keeps those identities visible in Chinese without trusting the DEV-077 snapshot.
- H3 local import, review regeneration, production orchestrator, and workflow benchmark auto-start paths now use the same admission service in the formal application composition. Their `cfg(test)` fallback is an explicit legacy test harness only; no production build path uses the unchecked queue helper.

## Start call-site audit

Backend formal start paths are covered by the source-contract test in `dev078_production_start_admission.rs`:

- `commands::production_queue::production_queue_start` delegates to `state.production_start_admission_service.start(...)`.
- H3 local import has two guarded auto-start paths.
- Review regeneration has one guarded auto-start path.
- Production orchestrator has five guarded start paths.
- Workflow benchmark has two guarded auto-start paths.
- `ProductionQueueService::start` is removed. `start_for_test` exists only for pre-existing unit/integration harnesses and is not a formal application entry point.

Frontend start calls remain the existing `startProductionQueue` client wrapper and therefore cross the same Tauri command. No second start command, queue, executor, task model, automatic retry, workflow switch, or batch mutation was added.

`AssetVideoBatchWorkspace` still contains the pre-existing `createBatch() -> startProductionQueue()` flow. DEV-078 does not change that separate create/auto-start mismatch; the call is nevertheless protected by the new backend guard. It is recorded as `CONFIRMED_PREEXISTING`, not introduced by DEV-078.

## Verification

Pure admission tests A1–A13 plus an incompatible-Comfy status case: **14 passed**.

Deterministic SQLite/adapter integration tests B1–B6 plus the existing-gate concurrency test: **7 passed**. They cover valid dispatch, zero-side-effect blocking, pending-only resume, Busy precedence, offline fail-closed behavior, capability refresh failure, and configuration exclusion until commit.

Additional queue lock/update regression tests: **2 passed**. Command and formal backend source boundary tests: **3 passed**.

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check   PASS
cargo check --manifest-path src-tauri/Cargo.toml --all-targets   PASS
cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1
  723 passed, 0 failed, 1 ignored                                PASS
pnpm test
  109 test files, 511 tests                                      PASS
pnpm test -- ProductionMonitor AssetVideoBatchWorkspace MultiPackageProductionBoard
  3 files, 32 tests                                               PASS
pnpm test -- projectWorkflowResolution projectWorkflowPreflight ProjectWorkflowPreflight ProjectWorkflowSettings projectProductionReadiness ProjectProductionReadinessUat
  7 files, 56 tests                                               PASS
pnpm exec tsc --noEmit                                            PASS
pnpm build                                                        PASS
git diff --check                                                  PASS
pnpm tauri build                                                   PASS
```

The Tauri release build produced:

```text
src-tauri/target/release/ai-studio.exe
src-tauri/target/release/bundle/msi/AI Studio_1.0.0_x64_en-US.msi
src-tauri/target/release/bundle/nsis/AI Studio_1.0.0_x64-setup.exe
```

No real MiniMax H3 generation was run for this DEV. The integration suite uses a deterministic Comfy adapter and does not create real production assets.

## Issues and final gate

```text
P0=NONE
P1=NONE
P2=PREEXISTING_AUTO_START_ON_CREATE_MISMATCH_RECORDED_FOR_FOLLOW_UP
P3=NONE
DEV078_PRODUCTION_START_ADMISSION_GUARD=PASS
```

Code was committed and pushed first as:

```text
0648a052a22878a439ff7ba3c58fe78921beb442 feat(production): guard batch start with runtime admission
```

This closeout document is committed separately with the required message `docs: record DEV-078 verification`.
