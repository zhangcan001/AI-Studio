# DEV-044 UI C 重构记录（主线程集成记录）

## Baseline

- 产品基线：AI Studio 0.6.1。
- 设计方向：用户批准的 C 方案「融合平衡型」；目标是深色、专业桌面创作工作台，而不是后台 CRUD 页面。
- 子任务 ownership：`ProductionQueueDrawer`、Queue layout tests、功能迁移矩阵、500 Shot Queue UX safety。
- 主线程已完成 `StudioShell`、全局 Rail、Project Structure Tree、Shot Creation Workspace、Shot Inspector 与 `App` / `ShotWorkspace` 接线；backend、schema、数据库、队列语义未改动。

## User-approved C design

Queue 是主工作区底部的单一 Queue Drawer slot：默认只显示 42～48px 的摘要条；展开后控制在约 220～260px，队列内容保持可见但不夺走 Shot 主工作区。完整 Runbook 仍属于 Global Production workspace，Shot 页面只保留相关简要项目。

## Before UX and new information architecture

旧 `ProductionQueuePanel` 同时承担 fetch、持久化队列管理、创建表单、详情、结果预览、对比与复核，导致 Queue 在不同工作区重复出现。Agent D 新增的 Drawer 只承担“当前快照 + 单项动作 + 打开完整队列”的呈现边界：数据与副作用由 host controller 保持。

完整 C Shell 的 Global Rail、Project Structure、Context Workspace、Shot Creation Workspace 与 Inspector 已由主线程接入；生产 Runbook、Project Pipeline 与 Review Board 仅在各自正式上下文渲染，避免同一功能在 Shot 页面重复出现。

## Queue Drawer

### Default / expanded states

- 默认 `expanded = false`；支持 `defaultExpanded` 与 controlled `expanded`。
- `onToggle(nextExpanded)` 只改变 UI 状态，不触发 backend mutation。
- collapsed header 显示“生产队列”以及运行中、等待中、失败三个真实数字。
- 数字优先来自 `overview`，其次来自 `runbook.summary`，再由传入的 `details` / `runbook.rows` 派生；没有数据时显示“—”，不填充演示数字。
- expanded 高度通过独立 CSS 控制在约 260px 上限，完整 Runbook 仍在生产工作区。

### Real rows and actions

- 批次行只来自 `details`、现有 `queues` summary 或 `runbook.rows`；优先顺序固定且不会复制成第二个队列。
- 项目行只来自 `items`，若 host 未单独传入则读取 `details[].items`；500 Shot 场景最多渲染 24 个当前快照行，并提示其余项由完整 Runbook 承载。
- 行展示优先读取现有对象上的 `thumbnailUrl`、`shotName`、`stage`、`resolution`、`duration` 可选槽位；字段不存在时显示“—”或省略，不制造缩略图、分辨率、时长或进度数据。
- `onStart(batchId)`、`onPause(batchId)`、`onRetry(itemId)`、`onOpen(batchId)` 都是单项回调。Drawer 不导入 `tauriClient`，不执行 API，不实现调度。
- 组件没有全局 Start All、Scheduler、Auto Start Next，也没有第二套队列状态。

## Feature migration matrix

完整旧功能清单、旧位置、新位置、backend contract 与 STATUS 见 [DEV_044_UI_C_FUNCTION_MATRIX.md](./DEV_044_UI_C_FUNCTION_MATRIX.md)。矩阵中的 STATUS 仅为 `MOVED`、`PRESERVED`、`MERGED`。

## Performance

- collapsed 状态不渲染批次/项目行，减少 Shot 页面默认 DOM。
- expanded 状态限制 Drawer 自己的可见快照为 24 行，不抓取、不重新 fetch 全部 tree，也不读 full media。
- `onToggle` 不触发 backend mutation；start/pause/retry/open 只通过 host callback 传递单个已有 ID。
- Queue Drawer 没有 polling、scheduler、自动启动链或新的缓存模型。

## Accessibility and UX safety

- Toggle 使用 `aria-expanded`、`aria-controls`，具备键盘 focus ring。
- 操作按钮带有单项英文 action aria-label 与中文可见标签，disabled 状态由 busy action 反馈。
- 状态同时显示文字和语义色，不依赖颜色单独传达运行/失败状态。
- 缩略图使用传入 URL；无 URL 时提供明确的“暂无缩略图”可访问标签。
- `prefers-reduced-motion` 保留静态交互。

## Tests

`src/features/production/ProductionQueueDrawer.test.tsx` 覆盖：

- collapsed 默认状态与真实 overview 数字；
- expanded 状态与 runbook summary 数字；
- details/items 的真实行与可选展示槽位；
- 单项 Start/Pause/Retry/Open action 标记；
- 不渲染全局 Start All、Scheduler、Auto Start Next。

## Main integration gate

- 前端基线门禁：`pnpm test` 通过（78 个 test files / 283 tests passed）；`pnpm build` passed。
- Rust 基线门禁：`cargo fmt --check` passed；`cargo check` passed；DEV-044B 最终 `cargo test -- --test-threads=1` 通过（586 个 lib tests + 43 个 integration tests = 629 passed / 0 failed / 1 ignored）。
- Scope：Product 0.6.1、Migration 021、BACKUP_VERSION 12、Manifest 1 保持不变；未新增 backend command、database table、schema、queue path、tag、release 或 installer。
- Runtime safety：开发版仅用于壳层可操作性确认；未启动真实 GPU / ComfyUI 生产任务。

## Regression and integration notes

`ProductionQueueDrawer` 已接入 `ShotWorkspace` 底部 slot，使用真实 `ProductionBatchRunbookView` 快照；开始与打开动作由现有 host callback 承担。完整 Runbook 仍在收起的生产管理区，不创建第二套队列状态。前端 targeted / full tests、TypeScript build、Rust fmt/check/test、diff gate 与 Git publish 由主线程执行。

## Known limitations

1. 当前 `ProductionBatchDetail` / `ProductionBatchItemView` 没有统一的 thumbnail、resolution、duration、shotName 字段。组件已提供纯 props 展示槽位，但不会通过 Shot fetch、资产 fetch 或假数据补齐。
2. `details` 在不同 host 中可能代表当前选中批次或已加载批次集合；组件按传入顺序显示，不改变选择策略。
3. 只允许单项回调；是否允许 start、pause、retry 以及 admission 校验仍由 host/backend policy 决定。
4. 本地人工验收仍需在用户机器上确认窗口尺寸、项目数据与运行时连接状态；本次代码门禁不启动真实 GPU / ComfyUI 生产任务。

## Final decision

DEV-044 C 方案达到可集成并完成主线程接线：默认收起、真实 props 驱动、单项动作、无调度副作用、独立深色 CSS、带 targeted tests；完整结果以主线程最终回归与本地人工验收为准。

## DEV-044B Final UX Closure

### Navigation and context

- Global Rail 已收敛为明确语义：Project → Command Center / project，Creation → Shots / creation，Assets → Assets / assets，Production → Shots / production，Review → Shots / review，Analysis → Command Center / analysis，Settings → Settings / settings。
- `activeStudioSection` 为 UI-only 状态，由 `App` 传入 `StudioShell.currentSection`；恢复工作区与 Rail 高亮共用同一映射，Shots 默认回到 Creation。
- Shot 上下文路径固定为 `Project > Series > Episode > Scene > Shot`；父级 breadcrumb 可点击回到对应 planner，当前叶子节点保持不可点击。

### Workspace separation

- Creation 只显示 Project Structure、上下文 planner、Shot Creation Workspace / Inspector 与收起的 Queue Drawer；不再显示 Runbook、Pipeline、Review Board 或旧编辑器。
- Production 只显示 `ProductionBatchRunbookPanel`、`ProjectProductionPipeline` 及现有生产摘要/队列控制；不重复显示 Shot Creation editor。
- Review 只显示 `ShotBatchReviewBoard` 与既有 task/audit 入口；不再混入 planner 或旧编辑器。
- 旧 `shot-legacy-panels`、旧 shot grid/list/editor JSX/CSS，以及 `ScalarControl`、`AssetThumb`、`CandidateCard` 等死 helper 已移除；500-shot bounded tree / search 行为保留。

### Preview, inspector, queue and ComfyUI

- Preview 优先级为显式 preview → 当前选中资产 → 当前候选首项；手动点击候选只改变预览，确认动作才提交选择。
- Inspector 将 Recipe、meta、resolution、duration、steps、CFG 作为 common fields；seed、sampler、denoise、guidance、detail、low/high-frequency 等保持 advanced 默认折叠。
- ComfyUI 正常连接状态只保留 Top Bar；offline / capability warning 使用紧凑 banner，完整配置集中到 Settings。
- Queue Drawer 默认收起，只保留 summary 与单项 Start / Pause / Retry / Open；取消 Start All、Scheduler、Auto Next 以及生产页重复完整队列。Shot Creation Workspace 自己承载 shot-level notice/error，避免重复提示。

### Verification and release boundary

- Targeted UI gate：8 个相关 test files / 27 tests passed。
- Full frontend gate：78 个 test files / 283 tests passed，0 failed；`pnpm build` passed；`git diff --check` passed。
- Full Rust gate：`cargo fmt --check`、`cargo check`、`cargo test -- --test-threads=1` 全部通过；总计 629 passed / 0 failed / 1 ignored。历史记录中的 586 仅是 lib 单测 subtotal，另有 43 个 integration tests；一次早期 lifecycle 单测的瞬时失败已由隔离复跑及最终全量复跑确认通过，且 `src-tauri` 无改动。
- 人工 UI gate：开发版已启动，待用户在 1920 / 1440 / 1180 宽度人工确认窗口与导航；本次未触发真实 GPU / ComfyUI 生产任务。
- 版本边界冻结为 0.6.1 / Migration 021 / BACKUP_VERSION 12 / Manifest 1；本任务不创建新版本、tag、release、installer，不修改 backend contract。
