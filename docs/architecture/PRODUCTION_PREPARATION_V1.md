# Production Preparation V1

状态：DEV-052 实现基线
适用版本：AI Studio 0.7.0 / Narrative Production V1

## 1. 目的

Production Preparation V1 是“生成前的准备和确认层”。它把 Shot Context、Readiness、Workflow/Comfy capability 和现有批次能力接起来，但不执行生成。

用户要能回答三个问题：

1. 哪些镜头现在可以生产？
2. 哪些镜头为什么不能生产？
3. 确认后会进入哪个现有批次，输入是否已经冻结？

## 2. 状态模型

### Shot Readiness

- READY：required context、reference、prompt、workflow、output 和 capability 均通过。
- INCOMPLETE：缺少可以由用户补齐的信息，没有不可恢复的技术冲突。
- BLOCKED：存在 workflow/runtime 不可用、跨项目关系、冲突 binding、无效 asset 或硬约束失败。

### Preflight Gate

每个 gate 使用 PASS、WARNING、INCOMPLETE、BLOCKER。聚合到 Shot Readiness：

| Gate 结果 | 聚合影响 |
| --- | --- |
| BLOCKER | Readiness = BLOCKED |
| INCOMPLETE 且没有 BLOCKER | Readiness = INCOMPLETE |
| 全部 PASS，允许 WARNING | Readiness = READY |

Warning 不可绕过 BLOCKER；allow_partial 只表示一次准备请求允许其他 READY 镜头继续，不表示把 blocked 镜头塞进队列。

## 3. ShotProductionPlan DTO

准备页和 scene/episode/series summary 统一使用以下逻辑 DTO：

~~~text
ShotProductionPlan {
  shotId
  ordinal
  name
  sceneId?
  resolvedContext
  imageStage: {
    workflowVersionId?
    recipeId?
    values?
    status
  }
  videoStage: {
    workflowVersionId?
    recipeId?
    values?
    status
  }
  readiness: {
    status
    score
    checks[]
    checkedAt
  }
  blockers[]
  warnings[]
  existingBatchIds[]
  preparedAt?
  snapshotIdentity?
}
~~~

批量页面可以用 ShotProductionPlanSummary，只返回 card 所需字段；点击详情再取完整 ResolvedShotContext。

## 4. Prepare 流程

~~~text
选择 Scene / Stage / Shot IDs
          │
          ▼
后端批量加载结构、Profile、ReferenceSet、Asset、Workflow
          │
          ▼
ShotContextResolver
          │
          ▼
ShotReadinessService + ComfyPreflightService
          │
          ▼
ShotProductionPlan[]
          │
          ├── BLOCKED / INCOMPLETE：显示原因，不进入 ready batch
          │
          └── READY：用户勾选并点击“加入现有生产队列”
                              │
                              ▼
                    复用 ShotBatchService / ProductionQueueService
                              │
                              ▼
                    返回已有 ProductionBatch detail + Preparation Snapshot
~~~

“打开准备页”“刷新 readiness”“查看 context”“生成 plan”都不能触发 generation。

## 5. 现有服务复用

### 保留并复用

- ProductionStructureService：读取 series/episode/scene/shot。
- ShotBatchService：继续负责现有 stage eligibility 和 batch item 构建。
- SceneProductionService：保留 scene scope 和 prepare gate，在内部接入新 plan。
- EpisodeProductionService、SeriesProductionService：继续做 scope 聚合，不创建第二套 batch 逻辑。
- ProductionQueueService：唯一普通批次的创建、start、pause、retry、resume。
- ProductionOrchestratorService：唯一跨图片→人工选图→视频的 Production Run。
- ComfyPreflightService：提供 runtime、node、workflow capability。
- ProductionItemReviewService、ProductionAuditService：继续负责审核和审计。

### 新增服务

#### ProductionPreparationService

负责：

1. 接收 scene/shot scope。
2. 调用 ShotContextResolver。
3. 调用 ShotReadinessService。
4. 生成 ShotProductionPlan。
5. 在显式确认时从 ResolvedShotContext 生成冻结 values，并映射为现有 ProductionBatch 输入。
6. 在同一 SQLite transaction 中写入 Batch、BatchItem、binding 与 Production Preparation Snapshot。
7. 返回 blocked、warning、already prepared、stale、created batch summary。

它不持有自己的执行队列，也不启动 task。

## 6. Batch semantics

### 6.1 Preparation 不默认创建 ProductionRun

决定：Batch Preparation 不创建 ProductionRun。

默认路径：

- Prepare 只返回 plan。
- 用户确认后创建现有 ProductionBatch。
- 用户仍在现有 Queue/Runbook 中手动启动。

当用户明确使用已有 Production Run 的 Krea2→候选选择→H3 流程时，才调用 ProductionOrchestratorService；这不是 Preparation 的第二 executor。

### 6.2 幂等

相同 project + stage + shot + contextHash 已有未结束 batch item 时：

- 不重复创建。
- 返回 existingBatchIds 和 alreadyPrepared。
- 如果 Profile/ReferenceSet revision 改变，新的 contextHash 可以产生新的准备尝试；旧 item 保持冻结。

快照 JSON 的 `schemaVersion` 当前为 `1`。它记录 resolved context、profile revisions、ReferenceSet/asset
checksums、最终 prompt、ordered references、workflow/recipe、output、stage input、frozen generation values、
readiness gates/score 与最小 Comfy capability evidence。快照是历史 immutable evidence；备份恢复时外层
`projectId/shotId/productionBatchId/productionBatchItemId` 会 remap，快照 JSON 内部的历史 identity 不作为实时 FK。

### 6.3 blocked 排除

- BLOCKED 永远不能写入新的 ready batch。
- INCOMPLETE 默认不能加入；未来如果某个 gate 被定义为可确认 warning，也必须在后端显式允许，不能由前端绕过。
- READY 在创建 batch 前重新验证一次，防止页面缓存过期。

## 7. Commands

保持现有 `scene_production_plan` / `scene_production_prepare` 签名与返回兼容；准备页使用以下只读/准入接口：

~~~text
shot_preflight(project_id, shot_id, stage)
  -> ShotReadiness

shot_resolve_context(project_id, shot_id, stage, mode)
  -> ResolvedShotContext

scene_production_preflight({
  projectId,
  sceneId,
  stage
})
  -> ScenePreparationView

scene_production_admit({
  projectId,
  sceneId,
  stage,
  shotIds,
  allowPartial
})
  -> AdmissionResult {
       readyCount,
       incompleteCount,
       blockedCount,
       createdCount,
       skippedIncomplete,
       skippedBlocked,
       existingBatchIds[]
     }

shot_production_plan_detail({ projectId, shotId, stage })
  -> resolved context + readiness + snapshot/admission status
~~~

`scene_production_preflight` 只读且不创建 Batch、Task 或 Generation。`scene_production_admit` 只接受 shot IDs，
后端重新 resolve + live preflight 后，只为 READY 镜头写入现有 batch；它不 start queue、不提交 ComfyUI。

## 8. Scene Production UX

### 布局

- 左栏：场景镜头列表，支持按 ordinal、readiness、image/video status 筛选。
- 中栏：Compact Shot Production Card 网格。
- 右栏：Readiness、Reference Pack、Profile 来源链、Workflow/Output 检查器。
- 顶部：shot total、READY、INCOMPLETE、BLOCKED、已准备、已入队。

### Card

每张卡最少显示：

- 缩略图或空状态。
- Shot ID 和名称。
- Characters 摘要。
- Scene Profile 摘要。
- READY / INCOMPLETE / BLOCKED。
- image status、video status。
- 打开、准备、加入生产。

### 批量交互

1. 默认只允许勾选 READY。
2. 用户可以查看 blocked/incomplete，但不能默认选中。
3. 选择多个 READY 后显示预计 batch item 数。
4. 点击加入队列后显示已创建/已复用 batch。
5. 跳转 Queue 或 Runbook，由用户决定 Start。

## 9. Episode/Series 聚合

EpisodeProductionService 和 SeriesProductionService 当前已有 scene/episode plan。0.7.0 将它们的 eligibility 解释扩展为新 readiness summary，但保留旧字段兼容：

- total
- ready
- incomplete
- blocked
- prepared
- done
- existingBatchIds

聚合原则：

- child BLOCKED 不自动让整个 scope 不能显示 READY child。
- allowPartial 只允许 READY child 创建 batch。
- 全量 prepare 仍受现有 scope size limits。
- Series/episode action 仍是“准备/加入批次”，不是“自动运行全部”。

## 10. Queue boundary

明确禁止在 0.7.0 新增：

- Scheduler 驱动的生产。
- Start All。
- Auto-next。
- Auto-select candidate。
- unattended production。
- Preparation 内部启动 queue/task。

现有行为继续：

- ProductionBatchRunbookPanel 只读派生现有 batch，并手动启动一个 batch。
- ProductionQueuePanel 负责查看、start、pause、cancel、retry、resume。
- ProductionRunPanel 负责明确的阶段按钮和人工候选选择。
- Review regeneration 的 autoStart 只能作为用户点击“返工”后的明确动作，不扩展成全局自动链。

Preparation ≠ Generation，Admission ≠ Start。ComfyUI 仍是唯一正式生成引擎；Queue/Runbook 中的 Start 仍由用户
明确操作。

## 11. Review 与 Compare

后端不新增 review 状态模型。前端规划：

- 当前 item 的候选 A/B。
- 上一项、下一项。
- Confirm、Reject、Regenerate。
- 显示 prompt、seed、workflow、contextHash、ReferenceSet 摘要。
- 选图、审核和进入下一项都必须由用户触发。

Production Audit Center 继续只读展示 run、stage、batch、item、task、snapshot、asset lineage。

## 12. Storyboard / Script Import 约束

### Auto Storyboard

P2 draft only：

- 输入 script text 或已存在的 draft。
- 输出 episode/scene/shot draft。
- 用户预览和确认后才能落正式结构。
- 不覆盖已有 Shot，不创建 ProductionBatch，不触发生成。

### Script Import V1

只考虑：

- TXT。
- Markdown。
- JSON。

输出结构化 draft：

Episode → Scene → Shot。
不做 DOCX、PDF、复杂自然语言解析、自动角色推断或自动覆盖。

## 13. 状态和性能

Frontend Shot Preparation Store 只保存：

- current scene/stage。
- selected shot IDs。
- plan summary。
- readiness summary。
- loading/error/notice。

后端提供 scene batch resolve/readiness；不允许 React 为 500 shots 逐个调用 context。

目标规模：

- 500 shots。
- 1000 assets。
- 100 profiles。

要求：

- 首屏先加载结构和 summary。
- 卡片使用批量摘要。
- 详情按需加载。
- 所有新增查询带 project scope 和适当 index。

## 14. 验收和测试

- dryRun 不创建 batch、不创建 task。
- READY shot 能加入现有 ProductionBatch。
- BLOCKED/INCOMPLETE 不进入 batch。
- 重复 prepare 返回 alreadyPrepared，不重复 batch item。
- contextHash 变化能产生新准备版本，旧 item 不变。
- scene/episode/series 的 partial prepare 统计正确。
- queue start 仍是手动 command。
- 500-shot scene 页面没有 N+1。
- 0.6.2 legacy shot 没有 pack 时仍走旧 stage config/reference path。

## 15. DEV-054 narrative integration

DEV-054 将一致性 binding、ShotContextResolver、Readiness、Preparation、现有 ProductionBatch/Queue、Review 和 Audit 接入同一条读取与准入路径。Preparation 仍是 admission，不是 generation：创建 batch、创建 task、Queue Start 和 ComfyUI submit 都不会由一致性预览或只读 Command Center/Audit 自动触发。

Production Audit 读取 preparation snapshot 的历史 context、contextHash、reference order、stage input 和 lineage；有 snapshot 时不重新解析当前 Profile，legacy batch 没有 snapshot 时保留兼容展示。正式生成仍只经由现有 ProductionQueueService → GenerationService → WorkflowCompiler → ComfyUI，人工候选选择、审核和手动 Queue Start 保持不变。
