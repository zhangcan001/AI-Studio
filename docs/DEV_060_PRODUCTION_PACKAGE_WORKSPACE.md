# DEV-060 — Production Package Workspace

状态：DEV-060 PASS

## 基线与目标

- DEV-059 External Production Package V1 已通过，基线为 `679d2fedab54cf632a9d972f5d0de1454c3ea48c`，CI Source-only 已成功。
- AI Studio 0.8.0 的生产入口是：外部 Production Package 文件夹 → 检查 → 预览 → 选择 → 复用现有 ProductionBatch/ProductionQueue → 用户手动 Start。
- DEV-060 不创建第二队列、第二执行器或第二生成服务，不直接调用 ComfyUI，不调用 LLM，不接入 Shot、Script Import、Profile 或 Storyboard。

## 工作区体验

既有生产 rail 增加本地 tabs，默认显示“生产包”，并保留“项目生产”旧面板。生产包工作区覆盖 EMPTY、INSPECTING、READY、PARTIAL、BLOCKED、CREATING_BATCHES、CREATED、ERROR 状态：

1. 用户点击“选择文件夹”，通过 Tauri 原生 `production_package_pick_root` 选择目录。
2. 选择后自动调用 `production_package_inspect`；检查只读，不写数据库。
3. 页面显示包名、总数、READY、WARNING 和 BLOCKED 摘要，以及可滚动的预览表。
4. READY 默认勾选；WARNING 默认不勾选但可人工选择；BLOCKED 禁用并说明原因。
5. 支持 50/100 分页、跨页选择、筛选不改变选择、全局选择 READY 和清空选择；500 项保持可用。
6. 只有用户明确点击创建后，才以 `inspectionId + selectedItemIds` 调用 `production_package_create_batches`。创建中禁用重复提交，成功后不自动打开队列、不自动 Start，仅提供“前往生产队列”手动入口。

预览列包含选择、状态、外部 ID、名称、模式、首帧/尾帧、引用数、Prompt、时长、分辨率和问题摘要；Prompt 长度在界面上截断，不改变后端原值。文件夹路径为只读展示，不能借此绕过原生选择器输入任意路径。

## 安全与失效处理

- 生产包仍遵循 DEV-059 的路径越界、URL、symlink/junction、类型和容量校验。
- 创建会话失效或包内容变化时显示 `PACKAGE_MEDIA_CHANGED`、`PACKAGE_PROMPT_CHANGED` 或 `PACKAGE_MODE_CHANGED`，提示重新检查；不静默创建旧批次。
- 页面没有“开始生产”、自动排队、自动生成或直接 ComfyUI 请求；开始动作仍由既有队列工作台承担并由用户手动触发。
- 交互提供 tab/checkbox/table 语义、命名按钮、加载 `aria-busy`、错误 `role=alert` 和摘要 `role=status`，1200×800 与 1000×700 不产生整体横向溢出。

## 验收证据

- Rust：DEV-059 production package contract/inspection/create tests 保持通过。
- Frontend：ProductionPackageWorkspace、ProductionPackagePreview、ShotWorkspace 定向测试覆盖分页、跨页选择、READY/WARNING/BLOCKED、37/150/500 项、首尾项、变更重检、无自动打开队列和无 Start/Generate。
- 最终门禁按串行顺序执行：`cargo fmt --check`、`cargo check`、`cargo test -- --test-threads=1`、`pnpm test`、`pnpm exec tsc --noEmit`、`pnpm build`、`git diff --check`。
- 本任务使用四个独占文件范围的子任务协作；`MULTI_AGENT=CONFIRMED`，`ACTIVE_SUBAGENTS=0`，`MULTITHREAD_USED=NO`。所有最终测试由主线串行执行。

## 版本边界与下一步

DEV-060 不新增 Migration 026，继续使用 Product 0.7.0、Migration 025、Backup 15、Manifest 2。下一步为 **DEV-061 — Bulk Production Hardening**。
