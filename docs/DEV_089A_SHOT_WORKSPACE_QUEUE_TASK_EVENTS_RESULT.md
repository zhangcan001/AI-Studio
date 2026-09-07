# DEV-089A ShotWorkspace Queue & Task-Event Controller Extraction 验收结果

日期：2026-09-08
基线：`f167a1d`
实现提交：`a0f378c`
Source-only CI：`34156278961`（frontend 与 Rust 均 success）

## 结论

```text
DEV-089A=PASS
SHOT_WORKSPACE_QUEUE_CONTROLLER=PASS
SHOT_TASK_EVENT_CONTROLLER=PASS
SEQUENTIAL_QUEUE_BEHAVIOR=PASS
SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION=0
RUST_IPC_DATABASE_CSS_APP_CHANGE=NO
```

已将 ShotWorkspace 的 production queue、顺序批次启动协调、queue focus/open 状态、task://updated 生命周期与 terminal task event 路由提取到独立 controller/hook；ShotWorkspace 保留 monitor、multi-package、selection 与其他既有工作区边界。

## 已交付

- `useShotQueueController`：队列加载、focus/open、批次启动、取消、暂停、恢复、重排、requeue，以及 project/unmount session invalidation。
- `useShotTaskEvents`：task subscription cleanup、同项目 terminal task 过滤、900ms debounce、multi-package 与 queue route 分流和 monitor refresh。
- `shotQueueState`：顺序批次 reducer、terminal completion/failure 判定、queue busy contract 与默认批次选择逻辑。
- ShotWorkspace 仅通过 controller/hook 接收 queue/task-event 结果，不再直接调用 `subscribeTaskUpdates`。
- 增加 queue state、queue controller、task-event controller focused tests，并将 direct task subscription 与 queue controller 边界加入 `dev088-architecture-guard.mjs`。

## 行为保持

```text
same busy batch -> ACTIVE/current，不重复启动
different busy batch -> queue
PRODUCTION_QUEUE_BUSY -> structured nonfatal notice
clean terminal completion -> sequential auto-advance
FAILED/CANCELLED/SKIPPED -> sequential pause
cancel future -> clear suffix only
resume -> PAUSED only
project switch/unmount -> invalidate async session
requeue -> backend -> queue refresh -> monitor refresh；不自动启动
task events -> terminal SUCCEEDED/FAILED/CANCELLED、同项目、900ms debounce
```

## 变更范围

```text
scripts/dev088-architecture-guard.mjs
src/features/shots/ShotWorkspace.tsx
src/features/shots/hooks/useShotQueueController.ts
src/features/shots/hooks/useShotQueueController.test.tsx
src/features/shots/hooks/useShotTaskEvents.ts
src/features/shots/hooks/useShotTaskEvents.test.tsx
src/features/shots/shotQueueState.ts
src/features/shots/shotQueueState.test.ts
```

ShotWorkspace 源文件由基线的 2726 行降至 2332 行。未修改 Rust、IPC、database、CSS、`src/App.tsx`，也未修改 monitor、multi-package、selection、production structure、references、creation form、GenerationStudio、WorkflowWorkspace 或 AssetVideoBatchWorkspace。

## 验证结果

本地验证：

- focused queue/task-event tests：通过（18 tests）。
- critical ShotWorkspace regression set：通过（47 tests）。
- `pnpm test`：128 test files，602 tests passed。
- `pnpm exec tsc --noEmit`：通过。
- `pnpm run build`：通过；仅保留既有 chunk warning。
- `node scripts/dev088-architecture-guard.mjs`：通过；raw invoke 为 0、272 个 frontend commands parity 通过、ShotWorkspace direct task subscription guard 通过。
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`：通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1`：通过，0 failed。
- `pnpm tauri build`：通过，MSI 与 NSIS artifacts 均生成。
- `git diff --check`：通过。

远端 Source-only CI：

- Frontend source checks：success（tests、TypeScript、build）。
- Rust source checks：success（format、check、all-target tests）。
- CI 运行链接：[34156278961](https://github.com/zhangcan001/AI-Studio/actions/runs/34156278961)

## 约束确认

```text
AI Studio code changed: YES（仅 DEV-089A queue/task-event extraction）
AI Studio Git changed: YES（实现提交 + 本文档提交）
Rust/IPC/database/CSS/App.tsx changed: NO
DEV-089 implemented: NO
```
