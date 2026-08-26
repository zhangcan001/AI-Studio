# DEV-052 — Scene Production Preparation + ComfyUI Generation Admission

状态：实现与验收记录

## 1. 目标

DEV-052 将既有 `ResolvedShotContext`、Shot Readiness/ComfyUI Preflight、Consistency Asset Management 接入
Scene Production 和既有 Production Queue。流程严格分为：

```text
Scene / Shot
  → Resolve Context
  → Live Preflight
  → ShotProductionPlan
  → 用户选择 READY 镜头
  → 明确点击“加入生产”
  → 冻结 Production Preparation Snapshot
  → 创建现有 ProductionBatch
  → 用户前往 Queue / Runbook
  → 用户手动 Start
  → ComfyUI 执行
```

Preparation 不等于 Generation，Admission 不等于 Start。准备页不提交 ComfyUI、不创建第二个 Queue 或 Executor，
也不提供 Start All、auto-next、auto-select 或 unattended generation。

## 2. 版本与迁移

- Product version：`0.6.2`，不升级为 `0.7.0`。
- Migration：`001–024`；只新增 `024_production_preparation_snapshots.sql`，不修改 `001–023`。
- Backup：`14`；Backup 13 无快照时恢复为空列表，Backup 12 兼容逻辑保持不变。
- Manifest：`2`，不创建 Manifest 3。Preparation Snapshot 属于生产历史/冻结证据，不属于项目语义定义。

## 3. Preparation Snapshot

`production_preparation_snapshots` 以 project、shot、stage、context hash 和 ProductionBatchItem 建立历史关联，
并使用 project/shot/batch/item 外键与幂等、批次、item 查询索引。Batch 或 BatchItem 删除可以级联快照；Profile、
ReferenceSet、Asset 不被快照反向删除。

Snapshot JSON 的稳定版本为 `schemaVersion=1`，至少包含：

- project/shot/stage、contextHash、resolvedAt、preparedAt、structure；
- profiles、profile revisions、reference sets、ordered reference assets（assetId、sha256、role、ordinal）；
- prompt（renderedText、negativePrompt、orderedSegments）；
- workflowVersionId、recipeId、outputSpec、stageInput；
- frozenGenerationValues；
- readiness status、score、gates、evaluatedAt；
- 最小 Comfy capability evidence，不保存全量 object_info 或节点缓存。

快照 JSON 是 immutable historical evidence。备份恢复时外层实时 FK 会 remap projectId、shotId、batchId、itemId；
JSON 内部 identity 不被当成新的实时 FK 使用，contextHash 保持原值。

## 4. 计划与准入

`ShotProductionPlan` 提供完整 context/readiness/hash、workflow/recipe、当前阶段状态、已有/匹配/过期准备记录、
blockers 和 warnings。Scene 批量页使用不重复返回完整 context 的 `ShotProductionPlanSummary` 与
`ScenePreparationView`，详情通过 `shot_production_plan_detail` 按需获取。

`scene_production_preflight` 是只读接口：只进行一次结构 scope load、Resolver batch、Workflow workspace load 和
Live Comfy preflight，不创建 Batch、Item、Task 或 Generation。`scene_production_admit` 只接收 projectId、sceneId、
stage、shotIds、allowPartial；后端重新解析和 live preflight，只允许 READY 项进入准入流程。

Admission 在一个 SQLite transaction 中创建 Batch、BatchItems、shot bindings 和 Snapshots。相同 project/shot/stage/
contextHash 的活动准备幂等复用；contextHash 变化创建新准备，旧 values 和旧 snapshot 保持不变，并在计划中标记
stalePreparedBatchIds。单次 Admission 最多 100 个镜头；Resolver/Readiness 的批量读取仍可支持 500 个镜头。

- `allowPartial=false`：任何 selected 项非 READY 即整体拒绝。
- `allowPartial=true`：只创建 READY，明确返回 skippedIncomplete、skippedBlocked、alreadyPreparedCount。
- WARNING 在 overall READY 时可以准入；INCOMPLETE/BLOCKED 不得降级为 warning。

## 5. Generation 与 ComfyUI 边界

写入 `production_batch_items.values_json` 的值必须从 `ResolvedShotContext` 生成并冻结，包含最终 prompt、ordered
references、I2V stage input、workflow scalar 和 output/recipe 输入。实现复用 GenerationInputPreparer、Recipe、
WorkflowCompiler、ordered reference binding 与现有 ShotBatchService/freeze_shot_batch_values，不创建第二套 compiler。

I2V 快照同时保存 selected image assetId 与 sha256；REF2VA 按 resolved context 的稳定顺序冻结 references。ComfyUI
offline 产生 COMFY_CAPABILITY blocker；Runtime Busy 仍是 warning，其他 gate 通过时可以 READY。真正执行继续走：

```text
ProductionQueueService → GenerationService → WorkflowCompiler → ComfyUI
```

Admission 不调用 GenerationService、Queue start 或 Comfy prompt submit。真正 Start 继续只存在于现有 Queue/Runbook，
由用户明确操作。

## 6. 兼容性与 UI

旧 `scene_production_plan`、`scene_production_prepare` 签名继续保留；Scene preset、Prompt Template bulk apply 仍是
配置编辑工具，不替代 Preparation。没有 Profile/ReferenceSet 的 0.6.2 legacy shot，只要 legacy prompt、stage config
和 legacy reference assets 完整，仍可以 READY 并准入。

准备页布局为左侧状态列表、中间 Compact Production Grid、右侧 Readiness/Context Inspector。卡片显示 shot、缩略图、
角色/场景摘要、reference 数、状态、score、阶段状态、alreadyPrepared/stale。只有 READY 且未默认勾选 alreadyPrepared
的镜头可选，选择全部 READY 最多 100 项。点击“加入生产”只显示加入队列结果与“前往生产队列”，不会自动 Start。

## 7. 验收重点

验收覆盖 dry preflight 无 batch/task/generation、READY/WARNING 准入、INCOMPLETE/BLOCKED 拒绝、幂等与 stale、
allowPartial、100 上限、I2V/REF2VA 冻结、legacy fallback、500-shot 无 N+1、一次 Comfy preflight、Snapshot 原子写入、
Backup 14 roundtrip、Backup 13/12 兼容、既有 Queue/Orchestrator/Runbook/Manual Review 回归，以及前端“加入生产”
不调用 startProductionQueue 的强制测试。

## Closure Verification

本节是 DEV-052 Closure Fix 的验收证据入口。产品与数据契约保持不变：Product `0.6.2`、Migration `024`、
Backup `14`、Manifest `2`；`Preparation ≠ Generation`、`Admission ≠ Start` 仍是硬约束。本次前端验收没有修改
生产组件，只扩展了两个 Vitest/React regression 文件。

### Closure 起点与已确认修复口径

- `DEV052_CLOSURE_START_SHA = 627647aac8c0a8581985c327435733cf6ad11bd5`
- GenerationDefinition N+1：`ProductionPreparationService.evaluate_many()` 应先收集并去重
  `(workflowVersionId, recipeId)`，再通过 `find_many` bulk read；正式 SQLite repository 不得在 context loop 内逐 key
  `find()`。
- Prepared Batch validation：在同一 transaction 内，以 set-based query 一次校验 Shot membership、一次校验已有
  `shot_generation_links`，然后继续 Batch / BatchItem / binding / Snapshot 的原子写入。
- Snapshot 运行期只允许 INSERT / READ；重新准备产生新 context 时，旧 Snapshot 与旧冻结值保持 immutable。

### Runtime integration test 说明

正式 closure 证据必须来自真实运行代码，而不是只检查源码字符串：使用 `tempdir()` 与 in-process SQLite，执行
`001 → 024` migrations，创建真实 Project / Scene / Shot / Workflow / Recipe，调用 Service / Repository / Domain，
并用 Fake Comfy capability / submit counter 验证 preflight 与 admission 不生成。测试需要实际查询并断言
Batch、BatchItem、Shot binding、PreparationSnapshot、Task 的前后状态，覆盖 dry preflight、READY admission、
rollback、幂等、context changed、冻结值、I2V、REF2VA、non-ready 与 100 limit。Source-contract tests 不能替代这组
runtime integration tests；最终真实运行文件为
`src-tauri/tests/dev052_runtime_integration.rs`，由 Main 接管完成并通过下述 targeted 与 full run。

### Negative prompt execution mapping audit

审计全部正式 `src-tauri/runtime_packages/*/recipe.yaml` 后，当前发布 recipe 只有语义明确的 `prompt` TextArea，
没有 `negative_prompt` / `negativePrompt` 输入。因此当前正式 recipe 的执行映射结论为：
`NEGATIVE_PROMPT_EXECUTION_MAPPING = NOT_PRESENT`。准备层不会把第二个任意 TextArea 猜作 negative prompt；只有
明确命名的 negative 输入才映射 `ResolvedShotContext.prompt_context.negative_prompt`，多个未明确语义的 TextArea 会返回
Preparation ERROR。显式 negative 输入与歧义 TextArea 均有 Rust regression test 覆盖。

### Frontend regression evidence

本次 Agent D focused run：

```text
pnpm test -- src/features/shots/SceneProductionPreparation.test.tsx src/features/shots/ShotReadinessInspector.test.tsx
Test Files: 2 passed
Tests: 8 passed, 0 failed
```

覆盖首屏 READY / INCOMPLETE / BLOCKED / 已准备统计、非 READY 禁选、105 个 READY 的 100 上限、组件事件链
`Select all READY → 加入生产 → 前往生产队列`、七 Gate、Profile / ReferenceSet 来源、context hash、alreadyPrepared、
stale、ComfyUI offline blocker、Legacy Shot，以及加入生产路径中 `startProductionQueue` 调用次数为 `0`。

### Main final closure statistics

以下为 Main 在最终 targeted/full 命令中取得的实际结果；500-shot 的 definition 数字来自真实 counting unit test 与
SQLite bounded-chunk test，runtime integration 同时验证真实 500-shot preflight 不写入队列且不提交 Comfy。

```text
RUST_PASSED = 776 (lib 640 + integration 136)
RUST_FAILED = 0
RUST_IGNORED = 1 (existing live ComfyUI test)

FRONTEND_FILES = 88
FRONTEND_TESTS = 307
FRONTEND_FAILED = 0
FRONTEND_TODO = 0

PNPM_BUILD = PASS (tsc + Vite; 191 modules)
MIGRATION_FRESH_001_TO_024 = PASS (1 targeted test)
MIGRATION_EXISTING_023_TO_024 = PASS (1 targeted test)
BACKUP_14_ROUNDTRIP = PASS (1 targeted round-trip test)
BACKUP_13_RESTORE = PASS (1 fixed v5-v9/v12/v13 fixture test)
BACKUP_12_RESTORE = PASS (1 fixed v5-v9/v12/v13 fixture test)

500_SHOT_RESOLVER_BATCH_CALLS = 1
500_SHOT_COMFY_CURRENT_CALLS = 1
500_SHOT_WORKFLOW_WORKSPACE_CALLS = 1
500_SHOT_DEFINITION_FIND_CALLS = 0
500_SHOT_DEFINITION_FIND_MANY_CALLS = 1
500_SHOT_DEFINITION_SQL_CHUNKS = 3 (SQLite chunk size 200)

ATOMIC_ROLLBACK_TEST = PASS
I2V_TEST = PASS (selected image assetId + sha256 frozen)
REF2VA_TEST = PASS (persisted reference order frozen)
IDEMPOTENCY_TEST = PASS
COMFY_SUBMIT_COUNT = 0
```

Targeted evidence：`dev052_production_preparation` 30 passed，`dev052_runtime_integration` 6 passed，
`production_queue` 35 passed；SceneProductionPreparation targeted 4 passed，Agent D 双文件 focused run 8 passed。
Full evidence：Rust 776 passed / 0 failed / 1 ignored，frontend 88 files / 307 tests，build PASS，diff-check PASS。
Full Rust run 同时覆盖 ProductionQueue、ShotBatch、Scene/Episode/Series Production、Runbook、
ProductionOrchestrator、Manual Candidate Review，以及 DEV-049/050/051/052 regression suites。
