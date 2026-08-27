# Narrative Preproduction V2

状态：DEV-057 Data Foundation 已落地；Import/Match/Storyboard/Review/Promote 尚未实现。

DEV-057 仅落地 Script/Draft 的领域 contract、Migration 025、不可变 revision、校验和 Backup 15；产品版本继续为 0.7.0，Manifest 继续为 2。本文下游的正式生产链和后续交付边界保持不变。

Narrative Preproduction V2 的职责是把“文本输入和创作建议”安全地接到 0.7.0 已验证的生产准备链。它不是新的生产执行系统，也不把 AI 草稿变成无人值守流水线。

## 1. 架构目标

```text
Script Source
    ↓
ScriptDocument / source provenance
    ↓
DraftStructure / StoryboardDraft
    ↓ 人工编辑、匹配、差异审阅
Formal Production Structure
    ↓ 人工确认 binding
ResolvedShotContext
    ↓
Readiness / Comfy Preflight
    ↓
ProductionPreparation / immutable Snapshot
    ↓ 用户手动加入现有 Queue
ProductionQueue → GenerationService → WorkflowCompiler → ComfyUI
```

V2 只增加上游的文本草稿和确认边界；下游仍然只有一条正式执行链。

## 2. 0.7.0 复用矩阵

| 0.7.0 能力 | V2 复用方式 | V2 不得做的事 |
| --- | --- | --- |
| `ProductionStructureService` | Confirm promote 的正式 Series/Episode/Scene/Shot 写入协作者 | Draft 编辑阶段直接调用正式 CRUD |
| `ShotBulkService` | 复用校验、原子 bulk 经验；只有正式确认后才能使用 | 把现有 Shot TSV/JSON 直接当 Script Import |
| `ConsistencyProfileService` | 为角色/场景/道具 Match 提供 project-scoped candidates | 让 LLM 直接写 Profile ID 或自动创建 Profile |
| `ReferenceSetService` | 用户确认 Profile 后选择已存在的 ReferenceSet | 从文本猜 Asset、ReferenceSet、顺序或 checksum |
| `ShotContextResolver` | 正式 Shot 生成后唯一的 context resolve 入口 | 给 Draft 伪造 ResolvedShotContext |
| `PromptContextBuilder` | 生成正式 stage-specific Prompt | 直接把 Storyboard prompt draft 当最终 Prompt |
| `ShotReadinessService` | 用户进入准备/预检时对正式 Shot 批量检查 | Import/Storyboard 阶段运行生产 preflight |
| `ProductionPreparationService` | 服务端 re-resolve、re-preflight、冻结 Snapshot、准入现有 batch | Draft 直接创建 Snapshot 或自动 start |
| `ProductionQueueService` | 继续唯一的手动队列启动和 generation loop | 新建 ScriptProductionQueue/Scheduler |
| `ProductionAuditService` | 记录 source/draft revision 到正式 provenance 的引用 | 把整篇原文复制到每个 Shot |

## 3. 分层责任

### 3.1 Source 层

Source 层只负责原文 identity、格式、checksum、存储引用、source spans、导入诊断和 parser/provider metadata。它不拥有任何生产 ID。

### 3.2 Draft 层

Draft 层负责 Episode/Scene/Shot 候选树、Storyboard 字段、Profile Match 候选、AI/human 值、revision、diff 和操作 provenance。它可以长期保存编辑事实，但仍不拥有正式生产状态。

### 3.3 Formal Structure 层

只有显式 promote 后，正式结构层才拥有正式 Series/Episode/Scene/Shot identity。Promote 后用户仍要单独确认 consistency binding；“结构确认”不等于“Profile 确认”。

### 3.4 Production 层

正式 Shot 的 Context、Readiness、Preparation、Snapshot 和 Queue 由 0.7.0 服务继续拥有。V2 不能复制这些模型，也不能把 Draft status 解释为生产 readiness。

## 4. 关键接口 seam

下面是后续能力的架构形状，不是 DEV-057 的实现 API。DEV-057 不增加 command 或 AppState wiring。

```text
ScriptImportPort
  preview(source) -> ScriptDocumentPreview
  parse(source, options) -> DraftRevision

DraftStore
  get(sourceId, revision, page) -> DraftPage
  apply_patch(draftId, revision, patch) -> DraftRevision
  diff(draftId, left, right) -> DraftDiff

ProfileMatchPort
  match(projectId, mentions) -> MatchView[]

ProductionPromotionPort
  preview(projectId, draftId, revision, target) -> PromotePreview
  confirm(projectId, draftId, revision, target) -> PromoteResult

ExistingProductionPorts
  resolve_draft(formalProjectId, formalShotId, stage)
  readiness / preflight
  preparation plan / admit
  queue start
```

`ProductionPromotionPort` 是唯一跨 Draft/Formal 的写入 seam。它不持有 Queue 或 Comfy adapter，也不能为了“方便”在事务中自动调用 Readiness 或生成。

## 5. 正式流程与副作用矩阵

| 用户动作 | 允许写入 | 不允许副作用 |
| --- | --- | --- |
| 选择文件/预览 | 无，或仅临时 source preview | 不写正式表、不调用 LLM 以外的执行服务 |
| 解析/生成 Draft | Script/Draft revision | 不写 formal structure、Profile、binding、Batch、Task、Comfy |
| Accept/Reject/Edit/Merge/Split/Reorder | Draft revision | 不创建正式 Shot、不改变旧 Shot |
| Profile Match | Draft match candidate/selection | 不自动 binding、不自动创建 Profile |
| Promote preview | 临时校验结果 | 不写 formal structure |
| Confirm promote | 一次事务写正式 structure + mapping + provenance | 不绑定 Profile、不跑 Readiness、不创建 Batch/Task |
| Confirm binding | 既有 consistency binding 路径 | 不自动加入 Queue、不自动生成 |
| Readiness/Preparation | 既有 readiness/preparation/snapshot | 不因打开页面自动 Start |
| Queue Start | 既有 ProductionQueue/Generation/Comfy | 不调用第二执行器/第二队列 |

## 6. Profile Match 与 Context 复用

Match engine 必须以 `projectId + entityType + normalizedMention` 为最小查询边界，并返回：

```text
mention
entityType
normalizedMention
status: EXACT | LIKELY | NO_MATCH | AMBIGUOUS
candidateProfileIds[]
evidence[]
selectedProfileId?
confirmed
```

只有 `confirmed=true` 的用户选择才可以进入 binding request。`EXACT` 也不跳过人工确认；`LIKELY` 和 `AMBIGUOUS` 必须 fail-closed；`NO_MATCH` 只显示“建议创建新 Profile”，不自动写 Profile。

Confirm promote 后，系统不应因为 Draft 有 prompt 就认为 Context 已解决。只有正式 Shot 和已确认 binding 存在时，才调用现有 Resolver。ReferenceSet 和 Asset 仍由用户确认，Reference asset 的顺序/checksum 在 Preparation Snapshot 时冻结。

## 7. LLM / Offline Provider Strategy

### 7.1 只冻结抽象

```text
trait DraftTextAnalyzer {
  analyze(source_blocks, parser_contract, options) -> DraftPatch
}
```

这个 port 的输出是不可信的文本 Patch。它必须经过 schema validation、source span validation、长度限制、重复节点检查和安全过滤，再进入 Draft。

### 7.2 推荐 provider 层次

1. Offline deterministic parser：默认路径，保证无网络可预览和编辑。
2. Local LLM：未来可选，显示模型/版本，所有结果仍需人工确认。
3. OpenAI-compatible API：未来可选的适配器，用户配置 endpoint；不在 DEV-056 真实接入。
4. User-configured endpoint：未来可选，必须有超时、取消、错误和 secret storage policy。

Provider 只负责文本分析、结构建议、候选别名和 Prompt draft。它不能调用 ComfyUI、提交 Queue、创建正式 Profile、绑定 Asset、执行图片/视频生成或决定人工 gate。

### 7.3 离线与失败行为

- 无 Provider 配置时，deterministic parser 仍能产生 Draft。
- Provider timeout/error 时显示诊断，保留已有 Draft，不自动重试到无限循环。
- Provider 返回非法 JSON、越权字段或超长文本时 fail-closed，只保留错误信息。
- Provider secret、原文全文和响应不复制到正式 Shot；审计只保留必要的 provider/model/checksum metadata。

## 8. 最小持久化策略

### DEV-057 已落地模型

DEV-057 实际使用两个核心表：

```text
script_sources
  source identity / project / format / checksum / original filename / source text / timestamps

script_import_drafts
  draft id / source id / revision / status / parser/provider metadata
  draft checksum / summary / payload_json / previous revision / timestamps
```

`payload_json` 承载当前 DraftStructure、diagnostics、source spans 和后续可扩展的 Draft 文档字段。原始文本只存于 `script_sources.source_text`；不复制正式表列，也不建立 draft_episodes/draft_scenes/draft_shots/draft_entity_matches/draft_node_index 镜像表。Migration 025、Backup 15 和 Manifest 2 exclusion 已完成。

服务端 append 在 SQLite transaction 内校验 latest、expected revision 和 previous link；revision 只 INSERT、不 UPDATE。默认列表页为 50、上限 200，list/history 只返回 metadata/summary，不加载完整 payload。

对于 5000 Draft Shot，DEV-057 以 document-oriented payload 做真实 serialize/validate/hash/SQLite roundtrip benchmark；若 payload 超过 64 MiB 或 load+deserialize 超过 2000 ms，测试直接报告 `DEV-057 BLOCKED`，不在本 DEV 偷增 `draft_node_index`。通过时记录 `DRAFT_NODE_INDEX=NOT_NEEDED_V1`；UI virtualization/pagination 留给 DEV-061。

### 为什么不直接使用候选表

`draft_episodes`、`draft_scenes`、`draft_shots`、`draft_entity_matches` 看似方便，但会复制正式结构语义，制造两套 ordinal/identity/status 规则，并迫使每次 Draft 字段变化都走 schema migration。V1 的核心是可版本化文档和人工差异审阅，不是第二套生产数据库。

## 9. Revision 与可复现性

```text
source checksum A
    → Draft revision 1
    → human edits / match decisions
source checksum B
    → Draft revision 2
    → added / removed / changed / moved / uncertain diff
    → user chooses what to promote
```

规则：

- Revision 不原地覆盖；新的 parse/reparse 产生新 revision。
- source checksum、parser version、provider/model metadata 和 prompt contract version 进入 revision identity。
- 用户编辑保留原始 suggestion、当前值、editor 和 operation order。
- Diff 对节点使用 source span、node type、邻近父级和稳定 draft identity；不只按数组下标匹配。
- 已 Confirm 的正式 Shot 永不被 reparse 静默覆盖；新版本只能产生影响预览或新的 Draft。
- Confirm mapping 可追溯到 `sourceId + draftRevision + draftNodeId`，但不复制完整原文。

## 10. Audit / Provenance

正式 Shot 至少需要能回答：

- 来源是 manual、Script Import 还是 Storyboard Draft。
- 对应哪个 source checksum、draftId、revision 和 draftNodeId。
- 哪些字段来自 AI suggestion，哪些由用户编辑/确认。
- 哪个 Profile Match 被确认，何时变成 binding。
- 何时通过 Resolver/Readiness/Preparation，哪个 Snapshot 冻结最终输入。

Provenance 使用关系和摘要，不把完整 ScriptDocument 写入每个 Shot。Audit Center 继续只读，不能因为查看来源而运行解析、预检或生成。

## 11. 大剧本性能与前端状态

目标规模：100 Episodes、1000 Scenes、5000 Draft Shots。规划要求：

- Source 读取/解析有进度、取消、byte/node/depth 上限和清晰诊断。
- Draft API 按 revision、Episode、Scene、状态和搜索分页，返回稳定 cursor、total、hasMore。
- Tree 采用 virtualization 或层级懒加载；原文按 source blocks/viewport 窗口化。
- Inspector 只请求当前 node 详情；不把完整 Draft 重复嵌入每张卡。
- 页面状态缓存 `sourceId/draftId/revision/selection/filter`，事实仍以 backend Draft Store 为准。
- 5000 Draft Shot 不等于 5000 正式生产任务；现有 Production caps 和 batch limits 保持不变。

建议新增一个 Creation 下的 `ScriptImportWorkspaceState`，而不是污染 `ShotWorkspace` 的生产状态。它应与现有 `workspaceResume` 明确区分：是否恢复 draftId/revision 由 DEV-061 选择，不能隐式把未确认 Draft 当成正式 Shot。

## 12. 0.7.0 兼容性

- 没有 ScriptDocument 的 0.7.0 项目直接打开并继续生产。
- 旧 Shot 没有 Draft/Reference Pack 时继续走现有 legacy fallback，不因为缺少 Script 而 BLOCKED。
- Product 仍为 0.7.0；DEV-057 新增 Migration 025，Backup 由 14 升为 15，Manifest 保持 2。
- Backup 15 包含 Script source/draft revision 并支持 roundtrip；Backup 14/13/12 继续兼容且 Script/Draft 为空。Manifest 2 export/import contract 不包含 Script/Draft/source text/payload。
- 0.7.0 项目无 Script source 仍可打开和生产。
- 不把 Draft 状态映射成 Readiness status，不把 Storyboard prompt 映射成已冻结 Prompt Snapshot。

## 13. 分阶段实现边界

| 阶段 | 交付 | 退出条件 |
| --- | --- | --- |
| Data Foundation | Source/Draft contract、revision、最小存储决策 | schema、project isolation、版本不可变 |
| Import | TXT/Markdown/JSON parser 和小说候选 | 三格式、diagnostics、source spans、零 formal writes |
| Match | 四状态 Profile candidates | project/type scoped、stable order、人工确认 |
| Storyboard | 每镜建议和 prompt draft | 字段完整、DRAFT 标记、无执行副作用 |
| Review | 三栏工作区和 diff | Accept/Reject/Edit/Merge/Split/Reorder、规模门禁 |
| Promote | 单一显式正式写入 | all-or-nothing、idempotent、旧 Shot 不覆盖 |
| Integration | 复用现有 Context/Readiness/Preparation/Queue | snapshot lineage、手动 gate、0.7 兼容 |
| Release | 完整回归与发布 | no P0/P1、性能/升级/安全/发布证据 |

## 14. 硬禁止清单

0.8.0 V2 不得引入：

- `StoryboardExecutor`、`ScriptProductionQueue`、`AIProductionScheduler`、`AutoGenerationPipeline`。
- 第二 ProductionRun、第二 Comfy adapter 或另一套 Prompt/Context Resolver。
- LLM 自动创建/绑定 Profile、Asset、ReferenceSet、Workflow 或正式 Shot。
- Draft 自动进入 Readiness、Preparation、Queue、ComfyUI 或 Review。
- 自动选择图片、自动批准视频、自动 Start Queue、Auto-next 或无人值守循环。
- 对已经人工编辑或已确认的正式 Shot 做静默 reparse 覆盖。

这些属于 0.9+ 重新立项范围。
