# DEV-132-A — Production Direct Entry Audit

审计日期：2026-09-15
审计基线：`b348c9904929be7dc2828da058ff5ef98173d69a` (`master`)

## 审计结论

当前问题是 **Production 工作区的入口与信息架构问题，不是 Generation 后端能力缺失**。

- Production 侧默认打开 `ShotWorkspace` 的“生产包”页，外部 Production Package 流程确实以文件夹和批次为入口。
- 同一应用已经存在单次 Generation 能力：`创作 → 单次创作` 使用 `generation_create`；镜头制作上下文也有选中 Shot 后调用 `shot_generate` 的路径。
- Production 工作区的 `生产包 / 批量生产包 / 项目生产` 当前没有明确的“单目标生产”按钮，因此用户会得到“只能选择文件夹包批量生产”的体验。
- 不需要第二套 Queue、Generation、Task 或 Result。最小修复是增加清晰的单目标入口，并复用现有单次 Generation/Shot 命令；若产品要求单目标也必须出现在正式生产批次中，则仅需适配现有 Queue 为一项批次，不应创建新的执行系统。

## 审计范围与事实来源

本次只阅读代码和现有架构，不修改产品代码、数据库、迁移或 UI。

主要事实来源：

- `src/app/App.tsx`
- `src/features/shots/ShotWorkspace.tsx`
- `src/features/production/ProductionPackageWorkspace.tsx`
- `src/features/production/ProductionBatchRunbookPanel.tsx`
- `src/features/production/MultiPackageProductionBoard.tsx`
- `src/features/studio/GenerationStudio.tsx`
- `src/features/studio/StudioModeTabs.tsx`
- `src/features/studio/hooks/useGenerationSubmissionController.ts`
- `src/services/tauriClient.ts`
- `src-tauri/src/commands/generation.rs`
- `src-tauri/src/commands/shot.rs`
- `src-tauri/src/commands/production_queue.rs`
- `src-tauri/src/application/generation_service.rs`
- `src-tauri/src/application/shot_service.rs`
- `src-tauri/src/application/production_queue_service.rs`
- `src-tauri/src/application/provenance_lineage_service.rs`

## 1. Repository / Frontend Entry Audit

### Production 导航路径

`StudioShell` 的 Production 导航调用 `openProductionQueue()`。在没有已聚焦批次时，该函数将路由到 `shots / production`；`App.tsx` 随后以 `mode="production"` 挂载 `ShotWorkspace`。

因此，Production 侧不是直接挂载 `GenerationStudio`，而是进入镜头生产工作区。

### Production 工作区现有入口

`ShotWorkspace` 的 Production tab 类型为：

```text
package
project
multi-package
```

对应界面为：

```text
生产包
批量生产包
项目生产
```

其中 `multi-package` 在需要时出现；默认 tab 为 `package`。

当前没有名为“单目标生产”“单次生产”或等价的 Production tab / action。

### 已存在但位于其他上下文的单次入口

`GenerationStudio` 已有四种模式：

```text
单次创作
批量生产
生产运行
基准实验室
```

`单次创作` 会渲染同一套动态表单和 `GenerationActionBar`，点击生成后调用 `useGenerationSubmissionController.generate()`。

`ShotWorkspace` 的 creation mode 也有选中 Shot 后的 `generate()`，调用 `generateShot({ projectId, shotId, stage, values })`。该入口在镜头制作上下文存在，但不会在 Production mode 的批次 tab 中显示为单目标操作。

## 2. Current Production Flow

### 外部 Production Package 流程

```text
用户点击 Production
    ↓
App.openProductionQueue()
    ↓
ShotWorkspace(mode=production)
    ↓
ProductionModeTabs（默认“生产包”）
    ↓
选择/拖入 Production Package 文件夹
    ↓
检查 production-package.json 与包内项目
    ↓
选择可创建项目
    ↓
createProductionPackageBatches(inspectionId, itemIds)
    ↓
创建现有待启动 ProductionBatch
    ↓
打开现有 Production Queue
    ↓
用户明确点击“开始生产”
    ↓
production_queue_start
    ↓
现有 Queue 调度 Task / Generation
    ↓
Comfy 执行、结果导入、AssetVersion、Provenance
```

Production Package 组件明确区分“创建待启动批次”和“真实生产”；创建或打开队列都不会自动开始执行。

### 项目生产流程

```text
ShotWorkspace(mode=production)
    ↓
项目生产
    ↓
项目/集/场景范围内的既有 Shot 与批次执行清单
    ↓
准备或创建现有 ProductionBatch
    ↓
开始生产
    ↓
现有 Queue / Task / Generation 路径
```

该页偏向项目级和批次级运行清单，没有单独的目标选择后立即创建一个 Generation 的操作。

### 非 Production 的单次 Generation 流程

```text
创作
    ↓
单次创作
    ↓
DynamicFormRenderer + GenerationActionBar
    ↓
useGenerationSubmissionController.generate()
    ↓
typed tauriClient.createGeneration()
    ↓
IPC generation_create
    ↓
GenerationService.start_generation()
    ↓
创建一个 Task
    ↓
GenerationExecutionLease + Comfy 执行/监控
    ↓
结果导入 → AssetVersion / Provenance
```

## 3. Required Result Fields

```text
CURRENT_FLOW=
  Production rail → App.openProductionQueue → ShotWorkspace(mode=production)
  → 生产包/批量生产包/项目生产 → existing ProductionBatch
  → production_queue_start → Queue dispatch → Task/Generation → Result/AssetVersion/Provenance

FOLDER_BATCH_DEPENDENCY=
  YES，仅对默认的外部 Production Package 入口；NO，不是后端 Generation 的全局依赖。

SINGLE_GENERATION_SUPPORT=
  YES，已有 generation_create 单请求路径与 shot_generate 单 Shot 路径。

FRONTEND_ENTRY_STATUS=
  PARTIAL：创作工作区已有“单次创作”，Production 工作区没有明确的“单目标生产”入口。

BACKEND_CAPABILITY_STATUS=
  READY：已有 canonical CreateGenerationRequest、GenerationService、Task 创建和 Comfy 执行路径。

QUEUE_COMPATIBILITY=
  PASS：正式 Production Batch 仍由 production_queue_start 作为唯一开始闸门；现有 Queue 接受 1..100 个 item，结构上支持一项批次。

PROVENANCE_COMPATIBILITY=
  PASS（后端基础能力）；PARTIAL（当前单次 UI 上下文的显式 provenance 传递）。

ROOT_CAUSE=
  Production 工作区入口的信息架构/可发现性缺口，而非 Generation 架构缺失。

RECOMMENDED_SOLUTION=
  方案 A：在 Production 相关位置提供清晰的单目标入口，复用现有单次 Generation 或 Shot 生成控制器；
  不新增 Queue、Generation、Task、Result 或执行器。若要求单目标必须进入正式批次，再用现有 Queue 创建一项 batch。

IMPLEMENTATION_SCOPE=
  下一阶段仅增加入口/导航与必要的目标选择适配，复用 typed transport、现有 admission、Task、Generation、Queue 和 provenance。

RISK_LEVEL=
  LOW（仅复用单次入口）；MEDIUM（若在 Production mode 新增 Shot/阶段选择并要求正式队列语义）。
```

## 4. Backend Capability Audit

### `generation_create`：单次 Generation 已存在

`src-tauri/src/commands/generation.rs` 的 `GenerationCreateRequest` 是一个单请求，包含：

```text
project_id
workflow_version_id
recipe_id
values
model_version_id?
prompt_version_id?
tool_instance_id?
tool_version_id?
submission_idempotency_key?
```

命令调用：

```text
generation_create
    ↓
acquire_interactive_admission
    ↓
GenerationService.start_generation
    ↓
prepare_task / TaskRepository.create
    ↓
background execute_prepared
```

这证明后端并非只能接受文件夹或批量包。

### `shot_generate`：单个 Shot 已存在

`src-tauri/src/commands/shot.rs` 提供 `shot_generate`。它接收明确的：

```text
project_id
shot_id
stage
values?
production_batch_item_id?
retry_task_id?
```

`ShotService::generate` 会校验当前项目和 Shot，读取该阶段已保存的 workflow/recipe 配置，构造现有 `CreateGenerationRequest`，并通过 `start_generation_with_task_hook` 创建 Task，同时把 Task 链到该 Shot。该路径不是文件夹批量导入。

### Queue：一项批次在结构上可行

`ProductionQueueService::create_with_provenance` 只要求 item 数量在 `1..=100`，所以现有 Production Queue 可以承载一个 item。它仍然是一个 `ProductionBatch`，并且必须经过 `production_queue_start` 才会进入正式生产运行。

这提供了一个可选的统一队列语义，但不等于当前已经存在“单目标 Production UI”入口。

## 5. Prompt / Model / Tool Input Audit

### API 与 Rust 请求能力

现有 typed transport `src/services/tauriClient.ts` 的 `createGeneration` 已声明可选：

```text
modelVersionId
promptVersionId
toolInstanceId
toolVersionId
```

Rust command 会把这些字段原样转换到 canonical `CreateGenerationRequest`。`GenerationService::prepare_task` 会校验明确提供的 ModelVersion 和 ToolVersion/ToolInstance 组合；不会从名称、路径或 Prompt 文本推断身份。

### 当前 UI 的覆盖范围

`useGenerationSubmissionController` 当前单次提交主要传递：

```text
projectId
workflowVersionId
recipeId
values
submissionIdempotencyKey
```

也就是说，后端具备 Prompt/Model/Tool 的显式上下文输入，但当前 `GenerationStudio → 单次创作` 控制器没有从 Production 入口的目标选择中自动填充这些可选 ID。后续如需要完整 provenance，必须让用户选择或由已保存的精确上下文提供 ID；不得根据路径、名字、Prompt 文本或时间猜测。

`shot_generate` 当前在 `ShotService::generate` 中构造请求时，将 `model_version_id`、`prompt_version_id`、`tool_instance_id`、`tool_version_id` 设为 `None`。因此该路径可以安全执行单个 Shot，但要展示完整 Prompt/Model/Tool provenance，后续需增加显式上下文适配，而不是隐式补全历史关系。

## 6. Provenance Compatibility Audit

### 已有链路

GenerationService 在执行成功路径中：

```text
Task / Generation
    ↓
GenerationProvenanceContext（只接收调用方显式提供的 ID）
    ↓
Comfy output collection
    ↓
AssetImportService.import_outputs
    ↓
现有 output mapping / AssetVersion
    ↓
ProvenanceLineageService.capture_successful_generation
    ↓
GenerationToolUsage / GenerationAssetVersion
```

`GenerationProvenanceContext` 的注释明确禁止从文件名、endpoint 或 Prompt 文本填充缺失字段。成功捕获使用当前输出收集阶段产生的精确 `(output_id, ordinal)`，不会批量模糊绑定。

### 对单目标入口的结论

单目标入口可以复用这条链路，不需要新表或新 Generation domain：

- 使用 `createGeneration` 时，按需传递用户明确选择的 PromptVersion/ModelVersion/ToolInstance/ToolVersion。
- 使用 `generateShot` 时，保持 Shot 的精确 `projectId + shotId + stage`；若目标是完整 provenance，需要另行传入明确的上下文 ID。
- 没有上下文时保持空值，并在界面显示“未记录”，而不是猜测。
- 输出 AssetVersion 仍由既有成功导入和 lineage 捕获负责。

## 7. Architecture Decision

### 方案 A：UI 入口缺失（推荐）

在 Production 相关位置增加清晰的“单目标”入口或引导，直接复用：

```text
已存在的 GenerationStudio 单次模式
或
已存在的 ShotWorkspace 单 Shot generateShot 路径
```

优点：

- 变更最小；
- 不新增后端 API、表、迁移或执行器；
- 继续使用现有 admission、Task、Generation 和成功 provenance；
- 解决 Production 默认落在“生产包”导致的可发现性问题。

需要明确的产品语义：这条路径属于“交互式单次生成”，不要伪装成一个绕过 Queue 的新正式批量生产系统。

### 方案 B：轻量 Direct Generation Request Adapter（条件使用）

若 Production 页必须提供“选一个 Shot/阶段后生成”的专用上下文，可增加很薄的入口适配层，把目标选择转换成已有 `shot_generate` 或 `generation_create` 请求。

适配层只负责：

- 验证当前项目和目标 Shot；
- 使用已保存的精确 workflow/recipe/stage 配置；
- 传递明确存在的 provenance ID；
- 调用现有 typed transport。

它不能创建新的 Generation、Result、Task 或 Queue。

### 正式队列语义的备选

如果产品定义“生产”必须全部可见于 Production Queue，则可将单个目标映射为现有 `ProductionQueueService` 的一项 batch，再由用户点击现有 `production_queue_start`。这是复用现有 Queue 的语义适配，不是新 Queue；但相较方案 A 会增加批次命名、队列可见性和用户确认步骤，不是当前问题的最小修复。

### 方案 C：大型架构重构（拒绝）

没有证据表明需要新执行引擎、新 Generation 表、新 Result 表或新的生产系统。方案 C 会破坏现有单一权威和 Queue 边界，应拒绝。

## 8. DEV-132-B Boundary

下一阶段最多包含：

1. 在 Production 工作区提供可发现的“单目标”入口/导航；
2. 选择已有项目、Shot、阶段或现有单次创作上下文；
3. 调用 `createGeneration` 或 `generateShot`，不直接访问 SQLite；
4. 复用现有 interactive admission、`GenerationExecutionLease`、Task 状态和错误处理；
5. 只传递用户明确选择或现有记录明确提供的 provenance ID；
6. 增加入口、项目隔离、单目标提交和“不自动启动批次”的聚焦测试；
7. 若产品确认必须进入正式队列，再复用现有一项 `ProductionBatch`，仍要求手动点击 Queue 的“开始生产”。

明确不包含：

```text
新 Queue
新 Generation / Result / Task 模型
数据库迁移
文件夹解析规则重构
自动生成或自动关联历史 provenance
AI Agent
修改 Queue Start 闸门
```

## 9. Risk and Guardrails

| 风险 | 等级 | 控制措施 |
| --- | --- | --- |
| 用户仍找不到单目标入口 | Medium | 在 Production 入口显式命名，不只依赖“创作”页的隐藏 tab |
| 单目标与正式批次语义混淆 | Medium | 明确标记交互式生成；若需 Queue 可见性则使用现有一项 batch |
| 跨项目读取 Shot/Asset | High | 所有请求保留 `projectId`，后端继续校验项目归属 |
| provenance 被错误补全 | High | 只传递精确 ID；缺失时保持空值并显示未记录 |
| 产生第二执行系统 | High | 仅调用 `generation_create`、`shot_generate` 或现有 Queue API |

## 10. Acceptance Decisions

```text
NO_ARCHITECTURE_BREAKING_CHANGE=PASS
NO_SECOND_EXECUTION_SYSTEM=PASS
QUEUE_AUTHORITY_PRESERVED=PASS
```

本审计结论为：**后端具备单次 Generation 能力，当前主要缺少 Production 工作区的单目标可发现入口；建议 DEV-132-B 以 UI/薄适配层为边界，继续复用现有权威。**

## Validation

```text
CODE_CHANGED=NO
DATABASE_CHANGED=NO
MIGRATION=NO
UI_MODIFICATION=NO
VALIDATION=DOCS_ONLY
```
