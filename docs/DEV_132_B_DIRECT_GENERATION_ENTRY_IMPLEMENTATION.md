# DEV-132-B — Direct Generation Entry Implementation

实施日期：2026-09-15
基线：`e974edfb51b4caba96430a45157cab92031da7d4`（DEV-132-A 审计完成）

## 结论

已在 Production 工作区加入“单次生成”入口。该入口不是新的执行系统，而是把一个明确的直接生成请求适配为**一个现有 Production Queue 批次项**。创建后批次保持 `READY`，只有用户在现有生产队列中明确点击“开始生产”时，系统才创建规范的 Task/Generation 并提交执行。

因此：

```text
DIRECT_GENERATION_READY=YES
QUEUE_AUTHORITY_PRESERVED=YES
PROVENANCE_NOT_GUESSED=YES
NO_NEW_QUEUE=YES
NO_SECOND_EXECUTION_FLOW=YES
```

## 入口设计

Production 页签现在按以下顺序展示：

```text
单次生成
生产包
批量生产包
项目生产
```

单次生成面板支持：

- 当前项目固定绑定 `projectId`；
- 选择一个明确的既有 Shot，或选择项目级“无 Shot”目标；
- Shot 目标选择 `image` / `video` 阶段；
- 选择已登记的 Prompt Version、Model Version、Tool Instance、Tool Version；
- 使用既有 Recipe 和动态参数表单；
- 空目录、加载中、部分元数据失败、参数校验失败和创建失败均有可见状态；
- 创建成功后可打开既有生产队列，但不会自动调用 Queue Start 或 Comfy。

## 数据流

```text
Production / 单次生成
        ↓
typed transport: createProductionQueue
        ↓
production_queue_create
        ↓
ProductionQueueService::create_direct_generation
        ↓
一个 READY ProductionBatch + 一个 Pending ProductionBatchItem
        ↓  用户明确点击 Queue Start
现有 Queue dispatch
        ↓
现有 GenerationService::start_generation_with_task_hook
        ↓
规范 Task / Generation、Result、AssetVersion、Provenance
```

无 Shot 请求直接复用现有 queue repository。带 Shot 请求复用既有 `ShotBatchRepository::insert_batch_with_bindings`，因此 Shot 与队列项的关系在执行前就以明确 ID 保存，并在 dispatch 时复用现有 task linkage。

## Direct Generation Context 与显式 ID

没有新增数据库表或迁移。直接入口的可选上下文被序列化到现有 `ProductionBatchItem.values_json` 的保留键：

```text
__ai_studio_direct_generation_context
```

其中只保存调用方明确提供的：

```text
shotId
stage
promptVersionId
modelVersionId
toolInstanceId
toolVersionId
```

Queue dispatch 会读取该 envelope，并把这些 ID 传入现有 `CreateGenerationRequest`。Recipe 参数解析会过滤保留键，因此内部上下文不会变成用户生成参数。没有选择的来源保持 `None`，不会依据文件名、路径、Prompt 文本、模型名称或时间猜测关系；损坏的 envelope 会 fail closed 并将队列项标记为 `QUEUE_VALUES_INVALID`。

Tool Version 只有在同时提供明确 Tool Instance 时才接受。Shot 与 stage 必须成对出现，且 stage 只接受既有 `image` / `video` 枚举。重试沿用既有冻结的 `values_json`，所以直接入口上下文不会被重建或猜测。

## Queue 与批量生产兼容性

- 仍只有 `production_queue_create` / `ProductionQueueService` 负责正式生产队列；
- 单次入口只允许一个 queue item；
- 点击“创建单次生成”后状态为 `READY` / item `PENDING`，不会是 `RUNNING`；
- Queue Start、admission、GenerationService、Comfy 生命周期保持原路径；
- 原有生产包、批量生产包、项目生产使用不带 direct 字段的旧请求形状，继续走原有创建路径；
- 没有创建新的 Queue、Task、Generation、Result 或数据库结构。

## 实现文件

- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src/features/production/DirectGenerationEntry.tsx` — 单次生成入口与状态处理；
- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src/features/production/DirectGenerationEntry.css` — 入口面板样式；
- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src/features/production/DirectGenerationEntry.test.tsx` — 单次、Shot、无 Shot、来源 ID 和 Queue gate 测试；
- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src/features/shots/ShotWorkspace.tsx` — Production tab 接入；
- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src/features/shots/hooks/useShotTaskEvents.ts` — direct tab 使用既有队列刷新路径；
- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src/services/tauriClient.ts` — typed transport 请求字段；
- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src-tauri/src/commands/production_queue.rs` — direct request IPC adapter；
- `C:/Users/ADMIN/Documents/ChatGPT/AI Studio/src-tauri/src/application/production_queue_service.rs` — queue item envelope、Shot binding 与 dispatch provenance 适配。

## 测试与验证

```text
Frontend unit tests: PASS — 156 files, 852 tests
DirectGenerationEntry focused tests: PASS — 3 tests
ShotWorkspace production regression: PASS — 20 tests
TypeScript: PASS — pnpm exec tsc --noEmit
Frontend build: PASS — pnpm build
Rust format: PASS — cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
Rust check: PASS — cargo check
Rust tests: PASS — 806 passed, 0 failed, 1 ignored (all integration suites passed)
```

Remote Source-only CI 未由本任务触发：仓库 workflow 的 push 触发器仅监听 `v*` tag，且本任务未要求手动 workflow dispatch。

## 已知限制

- 单次入口只负责准备队列项，不绕过 Queue Start 直接执行；
- Prompt / Model / Tool 选择器当前读取已登记目录，目录加载失败时保留可见诊断而不伪造选项；
- 没有来源 ID 时，生成记录由既有 Generation/Provenance 逻辑保留未知或未记录状态；
- 本任务不包含自动导入、AI 优化、自动执行或批量生产流程改造。
