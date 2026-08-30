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

- 状态：`DEV-067 CLOSURE = PENDING INTERACTIVE UAT`；本次只修复 Quick Flow 成功状态文案与 Queue open failure footer，不新增功能。
- `COPY_CONTRADICTION = FIXED`：成功状态统一为“生产批次已创建并已打开生产队列；不会自动开始生成。”；失败状态明确批次已创建且不会重复创建。
- 本地目标测试：ProductionPackageWorkspace `11/11`、ProductionQueueDrawer `6/6`、ShotWorkspace `8/8`；TypeScript、Frontend build、`git diff --check` 均通过。
- 本地完整门禁：Rust all-targets `869 passed / 0 failed / 1 ignored`；Frontend `97 files / 386 tests passed`；Rust format/check、TypeScript、Frontend build 均通过。
- Release build：`pnpm tauri build` 成功，版本仍为 `0.8.1`，未创建新版本、tag 或 GitHub Release。
- Installed-app smoke：release executable 已启动并显示 `ComfyUI 已连接`。当前 Computer Use 环境的 WebView2 截图/输入几何不可用（`SetIsBorderRequired: 0x80004002`、`coordinate input geometry is unavailable`），因此 `FOLDER_PICK`、`AUTO_INSPECT`、`QUICK_CREATE_OPEN`、`MANUAL_START`、`REAL_H3`、`VIDEO_ASSET`、`NEXT_PACKAGE` 与 `RESTART_QUEUE_TRUTH` 未执行，不能记为 PASS。
- `DRAG_DROP = ENV_UNVERIFIED`：同一 WebView2 输入限制影响拖放验证；不将其归类为产品失败。CI 结果以本次 closure 提交对应的 Source-only CI 为准。
