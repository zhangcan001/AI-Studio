# DEV-072 — Explicit Sequential Batch Start V1

状态：`DEV-072 CODE GATE = PASS — DEV-072 = PASS`

本 DEV 在 AI Studio 0.9.0 中增加“用户显式点选后按顺序连续启动 Batch”的前端编排。DEV-071 的 `AUTO_NEXT = NO` 历史结论保持有效；本次新增的是用户明确 armed 的 sequence，不是隐式自动接续。

## 1. Frozen product contract

```text
AUTO_START_ON_CREATE                  = NO
IMPLICIT_AUTO_NEXT                    = NO
EXPLICIT_USER_ARMED_SEQUENTIAL_NEXT  = YES
EXPLICIT_SEQUENTIAL_NEXT              = YES
AUTO_RETRY                            = NO
START_ALL                             = NO
SECOND_QUEUE                          = NO
SECOND_EXECUTOR                       = NO
SECOND_TASK_MODEL                     = NO
DIRECT_COMFY                          = NO
MAX_CONCURRENT_BATCH                  = 1
FORMAL_EXECUTOR                       = COMFYUI
SEQUENCE_RESTART_RESUME               = NO
```

只有用户逐个点击【开始】的 Batch 才会进入 sequence。Bulk Create 不会创建 sequence，也不会创建 Task 或提交 ComfyUI；没有点击过【开始】的 READY Batch 不会被自动启动。

## 2. Baseline and scope

```text
DEV072_START_SHA = 8d0b33f7cff387a66b667dce1de00958fa5a299a
PRODUCT_VERSION  = 0.9.0
MIGRATION        = 026
MIGRATION027     = ABSENT
ROOT_CAUSE       = USER_START_INTENT_NOT_QUEUED
```

原问题是 `ProductionQueueDrawer` 每个 Batch 独立调用 `onStart(batchId)`，Host 直接调用现有 `startProductionQueue(projectId, batchId)`。当全局 admission 已被其它 Batch 占用时，用户“稍后运行”的意图没有被保存，因此前一批完成后没有逻辑知道应该启动哪一批。

本 DEV 只在 `ShotWorkspace` 增加 session-local user-armed orchestration，并在 `ProductionQueueDrawer` 增加展示与回调；没有修改 `src-tauri/src/application/production_queue_service.rs`、repository、migration 或 `ProductionBatchStatus`。

## 3. Session-local sequence state

```ts
interface SequentialBatchStartState {
  status: "IDLE" | "ACTIVE" | "PAUSED";
  currentBatchId?: string;
  queuedBatchIds: string[];
  pauseReason?: string;
}
```

状态只存在当前 App session、当前 Project 的 React state/ref 中，不写 SQLite 或 localStorage。App unmount 或切换 Project 会清除 sequence intent；重启后不会因为旧意图突然启动 GPU 任务。

用户点击顺序严格写入 `queuedBatchIds`，并做唯一化：重复点击不会产生 `[B, B]`。当前 Batch 仍使用现有 global admission gate；sequence 不创建第二个队列或执行器。

## 4. Runtime behavior

### Explicit start

- admission free：立即调用 `startProductionQueue(projectId, batchId)`，成功后设为 `currentBatchId` / `ACTIVE`。
- 已有其它 Batch 占用 admission：保留用户意图，按点击顺序加入等待队列，不提交该 Batch。
- start 调用发生 busy race：结构化 `PRODUCTION_QUEUE_BUSY` 保留队列并等待现有刷新路径；其它错误保留 Batch、暂停 sequence 并显示实际错误。

### Queue and header UI

- 等待 Batch 显示 `等待自动开始 #1`、`等待自动开始 #2` 等本地 intent overlay；数据库 Batch status 仍是 `READY`，没有新增 `QUEUED` 枚举。
- 等待项的【开始】变为【取消等待】，只移除该项，不影响当前运行 Batch。
- Header 显示 `连续运行`、当前 Batch、等待数量以及【取消后续连续运行】；取消只清空 queued suffix，并提示“当前任务会继续完成”。
- 当前 Batch 终止异常时显示 `连续运行已暂停`、原因、【继续后续】和【取消后续连续运行】。
- Drawer 仍是 presentation boundary，不读取 DB、不判断 admission、不调用 Tauri。

### Single advance path

`maybeAdvanceSequentialBatchStart()` 是唯一自动推进入口，带 in-flight guard。它按以下顺序工作：

1. sequence 为 `PAUSED` 或没有 queued Batch 时直接返回。
2. 复用 `getProductionAdmissionStatus()`；busy 时保留队列并返回，不循环提交。
3. admission free 后复用 `getProductionQueue(projectId, currentBatchId)` 读取真实 Batch detail，不根据单个 Task event 猜完成。
4. 只有下列全部满足才算 clean completion：

   ```text
   status    = COMPLETED
   running   = 0
   pending   = 0
   failed    = 0
   cancelled = 0
   skipped   = 0
   succeeded = total
   ```

5. clean completion 后只取 `queuedBatchIds[0]`，调用现有 `startProductionQueue()`；调用成功前不从队列移除。
6. 成功后更新 current/queue，并复用既有 Queue、Board、Monitor refresh/convergence 路径。

Task terminal event 仍只触发现有约 900ms truth refresh；没有在 `TASK_SUCCEEDED` 事件中直接启动下一批，也没有新增第二个高频 timer。现有 visibility resume、Queue refresh 和 Board refresh 到达新的 Batch truth 后都会再次尝试推进。

## 5. Failure and pause semantics

- 当前 Batch 为 `PAUSED`：sequence 也为 `PAUSED`，不能跳过当前 Batch 启动后续。
- 当前 Batch terminal 但 `failed > 0`、`cancelled > 0` 或 `skipped > 0`：sequence 暂停，保留剩余 queue。
- 用户点击【继续后续】：明确接受上一批异常，清除 current 并启动下一 queued Batch；不会 retry 上一批，也不会自动重试失败 item。
- 下一 Batch 的 start 失败：Batch 仍保留在 queue，sequence 为 `PAUSED`，真实错误可见。
- 用户切换 Project 或 App 关闭：sequence intent 清除，不做 restart resume。

## 6. Regression evidence

严格串行执行，目标测试结果如下：

```text
pnpm test -- ProductionQueueDrawer                  PASS (7 tests)
pnpm test -- ShotWorkspace.sequentialBatchStart    PASS (7 tests)
pnpm test -- ShotWorkspace.production               PASS (17 tests)
pnpm test -- ShotWorkspace.multiPackagePolling      PASS (6 tests)
pnpm exec tsc --noEmit                              PASS
pnpm build                                          PASS

cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check  PASS
cargo check --manifest-path src-tauri/Cargo.toml --all-targets  PASS
cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1  PASS
```

新增 `ShotWorkspace.sequentialBatchStart.test.tsx` 覆盖：

```text
MULTI_BATCH_SEQUENCE = PASS       # A -> B -> C，保持点击顺序
MAX_ACTIVE_BATCH     = 1          # admission busy 时不启动下一批
MULTI_ITEM_GATE      = PASS       # 7/8 不推进，8/8 才推进
UNARMED_BATCH        = PASS       # D 未点击，不会自动启动
DUPLICATE_INTENT     = PASS       # 重复点击保持唯一
CANCEL_WAITING       = PASS
CANCEL_SEQUENCE      = PASS
FAILURE_PAUSE        = PASS
MANUAL_CONTINUE      = PASS       # 不 retry 上一批
START_FAILURE_RETAIN = PASS
VISIBILITY_RESUME    = PASS
CREATE_SAFETY        = PASS
```

Rust 全量串行结果：`700 passed; 0 failed; 1 ignored`（库测试），所有 integration test suites 通过；ignored 测试是既有需要真实运行时的 live gate。

前端全量回归结果：

```text
FRONTEND_FILES = 101
FRONTEND_TESTS = 444
FRONTEND_FAILED = 0
TSC = PASS
BUILD = PASS
```

## 7. Human UAT boundary

代码门禁和 Source-only CI 通过后，才进入真人 UAT；本 DEV 代码阶段不自动操作真实桌面生产。真人 UAT 使用 repo 外全新隔离 Project 和四个各含 1 个 H3 I2V、5 sec、960×544 item 的 Production Package：

```text
A -> B -> C 为用户明确点击的 sequence
B_STARTED_AT >= A_FINISHED_AT
C_STARTED_AT >= B_FINISHED_AT
BATCH_OVERLAP = NO
D（从未点击 Start）仍 READY/PENDING，Task = NONE
```

在真人 UAT 完成前，本文件状态保持：

```text
DEV-072 CODE GATE = PASS
DEV-072 = PENDING HUMAN UAT
```

提交 hash、Source-only CI run 和真人 UAT 结果在任务最终报告中记录。

## 8. DEV-072A terminal lifecycle closure

```text
DEV072A_START_SHA = 34fe62bd62953be9c2d24cc0bd19ba8a158e4519
TERMINAL_STATE_LEAK = YES
SAME_SESSION_REUSE_GAP = YES
```

DEV-072A 修复了 `maybeAdvanceSequentialBatchStart()` 的终态收口：只有 sequence `PAUSED` 保持 early return；无 current 且无 suffix 时归一化为 `IDLE`；有 current 时继续读取真实 Batch truth，再区分 `RUNNING`、当前 Batch `PAUSED`、clean `COMPLETED` 和 terminal failure。最终 clean completion 或无 suffix 的 terminal failure/cancel/skip 会清除 current 并回到 `IDLE`；带 suffix 的 terminal failure 仍保持 `PAUSED`，必须由用户明确【继续后续】。因此同一 App session 中 A→B→C 完成后点击 D 会立即启动 D，不会遗留 C 或“等待：0 个”。

DEV-072A fix gates：

```text
FINAL_BATCH_STATE_CLEANUP        = PASS
SAME_SESSION_SEQUENCE_REUSE      = PASS
TERMINAL_FAILURE_NO_SUFFIX_CLEANUP = PASS
FAILURE_WITH_SUFFIX_PAUSE        = PASS
```

新增回归：

```text
returns the sequential state to idle after the final armed batch completes       PASS
starts a new batch immediately after the previous sequence has fully completed    PASS
clears terminal failed state when there are no armed suffix batches               PASS
```

DEV-072A 严格串行验证结果：

```text
pnpm test -- ShotWorkspace.sequentialBatchStart    PASS (10 tests)
pnpm test -- ProductionQueueDrawer                PASS (7 tests)
pnpm test -- ShotWorkspace.production              PASS (17 tests)
pnpm test -- ShotWorkspace.multiPackagePolling     PASS (6 tests)
pnpm test                                         PASS (101 files, 447 tests)
pnpm exec tsc --noEmit                            PASS
pnpm build                                        PASS
git diff --check                                  PASS

cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check PASS
cargo check --manifest-path src-tauri/Cargo.toml --all-targets PASS
cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1 PASS
Rust library result: 700 passed; 0 failed; 1 ignored; all integration suites passed
```

Version、数据库和安全边界保持不变：`0.9.0`、Migration `026`、Migration027 不存在；没有修改 backend queue service、repository、command、migration、`ProductionBatchStatus`，没有新增 queue、executor、task model、retry 或 restart resume。最终状态仍为：

```text
DEV-072 CODE GATE = PASS
DEV-072 = PASS
```

## 9. DEV-072 FINAL HUMAN UAT

真人 UAT 使用最新本地构建 `src-tauri/target/release/ai-studio.exe`，repo 外 fixture：

```text
UAT_ROOT = C:\Users\ADMIN\Desktop\AI_Studio_DEV072_UAT
PACKAGE_IDS = DEV072-UAT-A, DEV072-UAT-B, DEV072-UAT-C, DEV072-UAT-D
ITEM = SH001
MODE = I2V
DURATION = 5 sec
SIZE = 960×544
```

实现与基线：

```text
IMPLEMENTATION_SHA = 34fe62bd62953be9c2d24cc0bd19ba8a158e4519
TERMINAL_FIX_SHA   = 3923e77923578da2c73b093ca35e9bb2f633782c
FIX_CI             = 33580093365 success
VERSION            = 0.9.0
MIGRATION          = 026
MIGRATION027       = ABSENT
```

批次与真实 Task 时间（Asia/Shanghai，`+08:00`）：

| Package | Batch ID | Task started | Task finished | Final |
| --- | --- | --- | --- | --- |
| A | `pbt_b56fedd2f49a4edd831e20846981c5f5` | `2026-09-02T11:46:36.999272+08:00` | `2026-09-02T11:48:39.948982100+08:00` | `COMPLETED`, `1/1`, `100%` |
| B | `pbt_84a35c209e064bb584889982685ce7c9` | `2026-09-02T11:48:43.139913100+08:00` | `2026-09-02T11:50:36.681667600+08:00` | `COMPLETED`, `1/1`, `100%` |
| C | `pbt_2eb67a841ca449d3a8eb34d1aaf38ea5` | `2026-09-02T11:50:39.897547400+08:00` | `2026-09-02T11:52:18.317051400+08:00` | `COMPLETED`, `1/1`, `100%` |
| D | `pbt_acb0dd066dcc4e908b46d15c61de6103` | `2026-09-02T11:54:46.805181200+08:00` | `2026-09-02T11:56:25.432223000+08:00` | `COMPLETED`, `1/1`, `100%` |

最终真人验收结果：

```text
AUTO_START_ON_CREATE              = NO
A_RUNNING                         = PASS
B_AUTO_START                      = PASS
C_AUTO_START                      = PASS
BATCH_OVERLAP                     = NO
FINAL_SEQUENCE_STATE              = IDLE
UNARMED_D_AUTO_START              = NO
D_IMMEDIATE_START_AFTER_SEQUENCE  = PASS
A_FINAL                           = COMPLETED
B_FINAL                           = COMPLETED
C_FINAL                           = COMPLETED
D_FINAL                           = COMPLETED
MAX_CONCURRENT_BATCH              = 1
COMFY_QUEUE_FINAL                 = running 0, pending 0
```

操作偏差记录：初始 A 运行期间用户误点 D，D 短暂显示为“等待自动开始 #3”，但没有创建 D Task 或提交 ComfyUI；随后 D 恢复为 `READY/PENDING`，并在同一 App session 中按要求显式启动后成功完成。该偏差不隐去，按真人 UAT owner 决定作为非产品 P2 操作偏差记录；产品行为验收判定为 `PASS`。

```text
P0 = NONE
P1 = NONE
P2 = OPERATOR_DEVIATION_ONLY
P3 = NONE
FAILURE_PAUSE       = AUTOMATED_PASS
MANUAL_CONTINUE     = AUTOMATED_PASS
CANCEL_WAITING      = AUTOMATED_PASS
CANCEL_SEQUENCE     = AUTOMATED_PASS
DUPLICATE_INTENT    = AUTOMATED_PASS
VISIBILITY_RESUME   = AUTOMATED_PASS
AUTO_RETRY          = NO
```
```
