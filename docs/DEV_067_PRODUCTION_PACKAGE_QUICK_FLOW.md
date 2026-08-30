# DEV-067 — Production Package Quick Flow V1

DEV-067 把已发布的 AI Studio 0.8.1 Production Package 路径收敛为三次明确操作：选择或拖入生产包、创建并打开生产队列、点击目标批次的“开始”。这是前端工作区优化，不增加新的队列、执行器、调度器或 ComfyUI 旁路。

## 用户流程

1. 在 Production → 生产包中拖入一个包含 `production-package.json` 的文件夹，或点击“选择生产包文件夹”。选择器和合法拖放都会自动调用 Inspect。
2. Inspect 完成后优先显示项目总数、READY、WARNING、BLOCKED 摘要。READY 默认选中；WARNING 必须人工确认；BLOCKED 不可选择。项目明细在超过 10 项时默认折叠，仍按 50/page 查看。
3. 点击“创建并打开生产队列（N 项）”。后端返回的完整创建结果会直接传给既有 Queue host；Queue reload 后自动展开，并以真实的 `createdResult.batches[0].batchId` 聚焦第一批。多个新批次仍全部可见，并标记“刚刚创建”。
4. 只有用户点击具体批次的“开始”才会进入正式生产；创建、打开和聚焦阶段不创建 Task、不提交 ComfyUI Prompt。
5. 完成一个包后可点击“选择下一个生产包”。该操作只清理当前工作区状态，不删除已创建批次、不停止运行批次、不重建旧批次。

非法拖放（多个路径、普通文件、`production-package.json` 单文件或 URL）只显示提示，不会调用 Inspect，也不会偷偷改用父目录。若 Tauri WebView 的拖放 API 不可用，界面明确显示 `API_UNAVAILABLE` 语义并保留文件夹选择器。

## 异常与安全

- 创建成功而 Queue reload/open 失败时显示“生产批次已创建，但生产队列暂时无法打开”，保留创建结果，仅允许“重新打开生产队列”；不会再次调用 create。
- PARTIAL 结果同时显示已加入项目和 `remainingItemIds`，可打开已创建队列或重新检查剩余项目。
- Queue Drawer 不提供 Start All、Auto Next、Scheduler 或第二套执行器。
- 调用链保持 `ProductionPackageService → H3LocalImportService → SourceAssetImportService → ProductionQueueService → GenerationService → WorkflowCompiler → ComfyUI`。

## 版本边界

- Product: `0.8.1`
- Migration: `025`; Migration 026 absent
- Backup: `15`
- Manifest: `2`
- GitHub Release `v0.8.1` numeric ID: `379150356`
- DEV-067 不创建新版本、tag 或 GitHub Release。

## 实现与验证记录

本 DEV 复用既有 `ProductionPackageWorkspace`、`ShotWorkspace` 和 `ProductionQueueDrawer`，不改变 Production Package V1 JSON schema 或后端生产链。Tauri V2 的拖放实现使用已安装 `@tauri-apps/api/webview` 的 `getCurrentWebview().onDragDropEvent()`；若运行环境无法注册，则状态为 `DRAG_DROP = API_UNAVAILABLE` 并继续使用选择器。

## DEV-067 CLOSURE 验证记录

- 状态：`DEV-067 = PASS`；DEV-067A 已完成最新 release build 的安装版回归与真实 H3 验收。
- `COPY_CONTRADICTION = FIXED`：成功状态统一为“生产批次已创建并已打开生产队列；不会自动开始生成。”；失败状态明确批次已创建且不会重复创建。
- 本地目标测试：ProductionPackageWorkspace `11/11`、ProductionQueueDrawer `7/7`、ShotWorkspace production `2/2`；TypeScript、Frontend build、`git diff --check` 均通过。
- 本地完整门禁：Rust all-targets `697 passed / 0 failed / 1 ignored`；Frontend `97 files / 388 tests passed`；Rust format/check、TypeScript、Frontend build 均通过。
- Release build：基于 tested SHA `a49d516d1aab20cd9a7240866ab3b3707218a5c6` 的 `pnpm tauri build` 成功，版本仍为 `0.8.1`，未创建新版本、tag 或 GitHub Release。
- Installed-app UAT：`QUEUE_BEFORE_CREATE = COLLAPSED`；3-item Production Package 的 `FOLDER_PICK`、`AUTO_INSPECT`、`READY_SUMMARY (3 READY)`、`QUEUE_AUTO_OPEN`、created Batch 可见、Start 可见全部 PASS。创建前 Task `253`、Comfy queue `running=0/pending=0/history=9`；创建后 Task 仍为 `253`、Comfy queue/history 无变化，`AUTO_START = NO`。
- Manual Start 与真实生产：新 Batch `pbt_dc9d2538a82141c5bc91c81668554b00` 手动启动后首个 Task `SUCCEEDED`，对应 MP4 Asset 存在且为 `1,001,766` bytes；随后 3 个 Batch items 与 3 个视频 Assets 均 `SUCCEEDED`/存在。`MANUAL_START = PASS`、`REAL_H3 = PASS`、`VIDEO_ASSET = PASS`。
- Next Package / Restart：选择下一个生产包后工作区清空且 Batch 保留；重启 release 后 Batch 仍存在。`NEXT_PACKAGE = PASS`、`RESTART_QUEUE_TRUTH = PASS`。
- `DRAG_DROP = ENV_UNVERIFIED`：同一 WebView2 输入限制影响拖放验证；不将其归类为产品失败。CI 结果以本次 closure 提交对应的 Source-only CI 为准。

## DEV-067A 自动展开回归修复记录

- 基线：`a6be8b60ed5ec1af54ef00afa6d2bdfbec93c300`；此前正式桌面 UAT 的真实结论为 `QUEUE_AUTO_OPEN = PRODUCT_FAIL`。创建批次成功，但生产队列没有自动展开，因此 UAT 在 Manual Start 前停止；此前记录不改写为 PASS。
- `OLD_TEST_FALSE_POSITIVE_RISK = YES`：原有 ShotWorkspace 集成测试直接从 production mode 渲染，而 production mode effect 会先把队列设为展开，未覆盖“先手动折叠、再 Quick Create”的回归路径。
- `EXPANDED_STATE_WRITERS =` production mode effect（`true`）、Quick Flow `openProductionQueue`（修复后在 reload 成功后写 `true`）、focused batch handler（`true`）、ProductionQueueDrawer 的受控 toggle（用户显式输入）；审计未发现创建后将其写为 `false` 的生产路径。
- `ROOT_CAUSE =` Quick Flow 的 queue-open 语义没有绑定到 reload 后的真实队列快照：`reloadProductionQueues` 原先返回 `void`，且展开请求发生在异步 reload 之前，没有确认创建批次已经投影到队列；旧测试因此无法暴露实际桌面回归。
- `OUTER_CALLBACK_EFFECT =` App 的 `onOpenProductionQueue` 仅导航到 Production section；已在 Production section 时为 no-op，不负责展开 Queue，也没有发现会主动折叠 Queue 的 callback。
- `RELOAD_EFFECT =` reload 只更新队列/overview state；修复后返回同一份 `{ queues, overview }` 快照，Quick Flow 先校验创建批次可见，再设置 `expanded = true`，最后执行外层 callback。
- 修复文件：`src/features/shots/ShotWorkspace.tsx`、`src/features/shots/ShotWorkspace.production.test.tsx`；受控 Drawer 行为由 `src/features/production/ProductionQueueDrawer.test.tsx` 覆盖。`SETTIMEOUT_USED = NO`、`DOM_CLICK_HACK = NO`、`BACKEND_CHANGED = NO`。
- 回归测试已覆盖真实 `ShotWorkspace → ProductionPackageWorkspace → ProductionQueueDrawer` 链路的 `COLLAPSED → CREATE → EXPANDED`，并断言 created batch 聚焦、`刚刚创建`、Start 可见且未调用 start。修复提交为 `ae9509dcbb8806f06b98a319ad8c0e2a0c084ee5`，最终测试提交为 `a49d516d1aab20cd9a7240866ab3b3707218a5c6`；Source-only CI `33293998261 = success`。

## DEV-067A FINAL INSTALLED UAT

- `QUEUE_BEFORE_CREATE = COLLAPSED`
- `FOLDER_PICK = PASS`
- `AUTO_INSPECT = PASS`
- `READY_SUMMARY = PASS (3 READY)`
- `QUEUE_AUTO_OPEN = PASS`
- `CREATED_BATCH_VISIBLE = PASS`
- `START_BUTTON_VISIBLE = PASS`
- `TASK_AFTER_CREATE_OPEN = 0`
- `COMFY_AFTER_CREATE_OPEN = 0`
- `AUTO_START = NO`
- `MANUAL_START = PASS`
- `REAL_H3 = PASS`
- `VIDEO_ASSET = PASS`
- `NEXT_PACKAGE = PASS`
- `RESTART_QUEUE_TRUTH = PASS`
- `P0 = NONE`、`P1 = NONE`；focused/recent label 未单独作为人工回复项记录，代码回归测试已覆盖，按任务规则不阻塞主 Gate（`P2 = NOTE`）。
