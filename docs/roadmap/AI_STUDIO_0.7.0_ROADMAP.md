# AI Studio 0.7.0 Roadmap：AI 漫剧生产闭环 V1

状态：Architecture Freeze（DEV-046，规划文档，不实现功能）

内部代号：Narrative Production V1
基线版本：0.6.2
DEV-046_START_SHA：60385e09c4ed7ad066e153ad52170b06ad763f0b
0.6.2 官方源 RC：542524feff8d4d65e8657125e6907898ca835bef

本文件冻结 0.7.0 的产品范围、数据边界、服务边界和交付顺序。DEV-046 不新增迁移、不修改现有命令、不修改生产执行器、队列语义、审核语义或正式 UI；实现从 DEV-047 开始。

DEV-054 落地状态：DEV-047 至 DEV-053 已接入同一条 Narrative Production V1 路径。当前产品兼容版本仍为 0.6.2，migration 最大版本仍为 024，正式执行器仍为现有 Production Queue → GenerationService → WorkflowCompiler → ComfyUI；0.7.0 version bump 留给 DEV-055 Release Gate。详细实现记录见 docs/DEV_054_NARRATIVE_PRODUCTION_INTEGRATION.md。

## 1. 产品愿景

0.7.0 的目标是把 AI Studio 从“能逐镜头生成”推进到“可控、可复用、可审核、可追踪、适合批量生产”的 AI 漫剧生产闭环 V1。

闭环不是一键自动导演，也不是把创作者从流程中移除，而是让人负责创意和关键选择，让系统负责一致性资产复用、上下文解析、就绪度检查、批次准备和生产证据留存：

1. 人建立角色、场景、道具和风格的语义档案。
2. 人为镜头绑定角色、服装、场景、道具和参考集。
3. 系统沿项目结构继承配置，生成稳定的镜头上下文。
4. 系统在提交前说明缺什么、冲突在哪里、当前运行环境是否可用。
5. 人确认 READY 镜头加入现有生产队列。
6. 现有 Krea2 图片、MiniMax H3 视频、Production Batch、Production Queue、Production Run、审核和审计继续完成执行闭环。

## 2. 0.6.2 能力审计

审计依据是实际 Rust、TypeScript、SQLite schema 和已注册 Tauri command，而不是目录名称。

| 能力 | 状态 | 现有证据 | 0.7.0 缺口 |
| --- | --- | --- | --- |
| Project | EXISTS | ProjectService、projects 表、ProjectCommandCenter | 需要成为上下文根，但不改旧项目模型 |
| Series | EXISTS | production_series、ProductionStructureService | 缺 profile 继承和批次级上下文 |
| Episode | EXISTS | production_episodes、EpisodeProductionService | 缺 profile 继承和准备结果 |
| Scene | EXISTS | production_scenes、SceneProductionService | 缺 SceneProfile 和统一 readiness |
| Shot | EXISTS | shots、ShotService、ShotWorkspace | 缺 Shot Reference Pack 和 profile binding |
| Project Manifest | EXISTS | ProjectManifestService、manifest version 1 | 需保留 anchors 和旧 shot 字段，未来再扩展 profiles |
| Reference Anchor | EXISTS | migration 020、ReferenceAnchorService、reference_anchor_* commands | 只有按 kind 的素材集合，缺语义 profile、revision 和可组合 ReferenceSet |
| Prompt Template | EXISTS | PromptTemplateService、PromptTemplatePanel、prompt_template_* commands | 已有结构上下文但没有统一 ResolvedShotContext |
| Project Context | PARTIAL | PromptTemplateContext、项目/集/场景/镜头结构、ProjectCommandCenter | 缺单一解析入口，Krea2/H3 仍从各自输入读取 |
| Asset | EXISTS | assets、AssetLibrary、AssetQueryService、标签/收藏/删除检查 | 缺 profile/reference/shot 使用反向查询 |
| Workflow | EXISTS | workflow library、recipe、capability/readiness、WorkflowWorkspace | 只表达运行时能力，不表达镜头语义就绪 |
| Krea2 Image | EXISTS | product runtime scope、shot batch、ProductionRun image stage | 缺统一上下文适配器 |
| MiniMax H3 Video | EXISTS | I2V/REF2VA 规则、ordered reference binding、H3 runtime | 缺 profile/reference set 解析后的稳定输入 |
| Production Run | EXISTS | production_runs、ProductionOrchestratorService、production_run_* commands | 0.7.0 不新建第二种 run |
| Production Queue | EXISTS | production_batches/items、ProductionQueueService、queue commands | 只接受已准备的 generation values |
| Production Batch | EXISTS | migration 006、ShotBatchService、scene/episode/series prepare | 需接受 ShotProductionPlan 的已冻结值 |
| Scene Production | EXISTS | scene_production_plan/prepare、SceneProductionPanel | 现有 eligibility 不是完整 readiness |
| Episode Production | EXISTS | episode_production_plan/prepare、EpisodeProductionPanel | 同上，需要统一 context |
| Series Production | EXISTS | series_production_plan/prepare、SeriesProductionPanel | 同上，需要统一 context |
| Batch Runbook | EXISTS | ProductionBatchRunbookService、ProductionBatchRunbookPanel | 继续只读派生批次，并保持手动一次启动一个 |
| Manual Candidate Review | EXISTS | ProductionItemReviewService、ProductionBatchReviewWorkspace、ProductionRun 手动选图 | 0.7.0 只补 compare 生产力，不重做审核后端 |
| Production Audit Center | EXISTS | ProductionAuditService、ProductionAuditCenter | 增加 profile/context 快照的可追踪性，仍只读 |
| Project Command Center | EXISTS | ProjectCommandCenterService、ProjectCommandCenter | 增加准备/阻塞摘要，不改全局聚合契约 |
| C UI Workspace | EXISTS | StudioShell、StudioGlobalRail、App workspace routing | 在当前 shell 内增加子页，不做全局壳重构 |
| Creation / Production / Review | EXISTS | Studio rail、ShotWorkspace modes、ProductionQueue/Review surfaces | 需将 Preparation 明确成 Production 子工作区 |
| Zoom / Pan | EXISTS | ZoomableImagePreview 的 fit/zoom/pointer pan | 保留并用于 compare/context 预览 |
| 中文 UI | EXISTS | current labels、i18n/statusLabels、中文 UI tests | 新增 Profile/Readiness 文案必须继续中文化 |
| Script Import V1 | MISSING | 未发现 TXT/Markdown/JSON script-to-draft command | 仅列为 SHOULD，不能覆盖正式 Shot |
| Auto Storyboard | MISSING | 未发现 storyboard draft domain | P2，仅生成草稿，必须人工确认 |

结论：0.7.0 首要问题不是再造一条生产通道，而是在现有结构、资产和队列之上增加一致性语义层与准备层。

## 3. 生产工作流

~~~text
Project / Series / Episode / Scene
              │
       Profile + Revision
              │
       ReferenceSet + Asset
              │
       Shot Reference Pack
              │
   ResolvedShotContext（动态解析）
              │
 Readiness + Preflight（后端权威）
              │
      ShotProductionPlan
              │
 人工确认：加入现有 ProductionBatch
              │
 ProductionQueue / ProductionOrchestratorService
              │
 Krea2 Image → 人工候选选择 → MiniMax H3 Video
              │
   Manual Review → Audit / Snapshot
~~~

准备和执行是两个明确动作。打开准备页、解析上下文、运行预检、生成计划都不启动生成；只有用户确认加入已有队列后，才调用现有 Production Queue 或 Production Orchestrator。

## 4. 用户痛点与优先级

评分范围为 1–5，5 表示对闭环影响最大或发生最频繁。

| ID | 痛点 | Impact | Frequency | 当前 workaround | Proposed solution | Priority |
| --- | --- | ---: | ---: | --- | --- | ---: |
| A | 角色在不同镜头中脸型、发型、服装漂移 | 5 | 5 | 手工复制参考图和提示词 | CharacterProfile + CostumeVariant + ReferenceSet + shot binding | 5 |
| B | 场景/道具参考容易丢失或顺序不稳定 | 5 | 4 | 在 shot references 中逐张选择 | SceneProfile/PropProfile + reusable ReferenceSet | 5 |
| C | 每个镜头重复写角色、场景和风格提示词 | 4 | 5 | 复制 prompt 后手改 | ResolvedShotContext + deterministic Prompt Context Builder | 5 |
| D | 生产前不知道哪些镜头缺工作流、素材或输出规格 | 5 | 4 | 先加入批次，失败后返工 | Shot Readiness + seven-gate Preflight | 5 |
| E | 批量准备结果不透明，阻塞镜头和可生产镜头混在一起 | 5 | 4 | 依赖 scene/episode plan 文本 | ShotProductionPlan、READY/INCOMPLETE/BLOCKED 分层 | 5 |
| F | 素材不知道被哪些档案、参考集、镜头使用 | 4 | 4 | 搜索文件名和历史任务 | Asset Usage 查询和反向关系面板 | 4 |
| G | 场景/集/系列只看生成状态，不看语义准备状态 | 4 | 4 | 逐级打开 shot | 场景生产网格、summary counters、readiness 聚合 | 4 |
| H | 审核时比较多个候选和重生成版本成本高 | 4 | 3 | 手工打开素材、记版本 | A/B compare UX、上一项/下一项、保留手动 gate | 3 |
| I | Profile 修改会不会影响旧镜头不明确 | 5 | 3 | 复制旧配置或不再修改 | 动态 draft resolve + prepare/start 时 Production Snapshot | 5 |
| J | 当前 ComfyUI/Recipe 可用性与镜头语义就绪分开 | 4 | 4 | 分别看 Settings 和 Shot | Context resolver 组合 Workflow/Comfy capability gate | 4 |
| K | 脚本到镜头结构仍需人工重复录入 | 3 | 2 | 手工 bulk import | TXT/Markdown/JSON draft import，人工确认后落正式结构 | 2 |
| L | 生产证据散落在批次、任务和素材记录中 | 4 | 3 | 打开 Audit Center 逐项查 | 在 snapshot/audit 中保留 resolved context identity，不重做审计链 | 3 |

## 5. 策略与优先级

默认结论：Consistency Asset System 是第一核心能力，选择 YES。

### P0：必须形成闭环

1. P0-1 Character / Scene / Prop consistency asset system
2. P0-2 Shot Reference Pack
3. P0-3 Shot Readiness / Preflight
4. P0-4 Batch Shot Preparation

### P1：提高日常生产效率

1. P1-1 Asset Library 2.0
2. P1-2 Scene Batch Production UX
3. P1-3 Review Compare UX

### P2：只做低风险草稿

1. P2-1 Auto Storyboard（draft only）
2. P2-2 Script Import V1（TXT / Markdown / JSON）

## 6. 核心架构冻结

### 6.1 分层

1. 现有内容层：Project、Series、Episode、Scene、Shot、Asset、Workflow、Prompt Template。
2. 一致性语义层：CharacterProfile、CostumeVariant、SceneProfile、PropProfile、StyleProfile、ReferenceSet。
3. 解析与准备层：ShotReferencePack、ResolvedShotContext、ShotReadiness、Preflight、ShotProductionPlan。
4. 既有执行层：ProductionQueueService、ProductionOrchestratorService、ProductionBatch、ProductionRun、GenerationService、Review、Audit。

解析层不得让 Krea2 或 H3 直接读取多张 Profile/Reference/Structure 表。它们只接受统一的 ResolvedShotContext 或由其生成的既有 GenerationValues。

### 6.2 Profile、Asset、ReferenceAnchor、ReferenceSet

| 概念 | 定义 | 不承担的职责 |
| --- | --- | --- |
| Asset | 物理媒体文件及其 checksum、存储路径、尺寸、来源任务 | 不代表“某个角色”或“某个场景” |
| Profile | 可复用的语义实体、文本规则、默认关系和版本 | 不保存物理文件本身 |
| ReferenceAnchor | 0.6.2 已存在的低层 kind → ordered image assets 关系 | 不在 0.7.0 被删除或强行改名 |
| ReferenceSet | 可命名、可复用、可排序的参考集合，指向具体 Asset | 不复制 Asset 文件，不替代 Profile |

0.7.0 保留 ReferenceAnchor 作为兼容层；新 Profile/ReferenceSet 可以从 Anchor 导入或引用，但不能让旧项目必须人工重建。

### 6.3 继承

逻辑层级固定为：

Project → Series → Episode → Scene → Shot

规则：

- 标量字段：最近一层显式值覆盖祖先；未设置则继承。
- 集合字段：默认继承并按稳定 ID 去重；明确的 replace/remove 操作可以停止继承。
- null/clear 是显式停止继承，不等于未设置。
- 同一层同一 role 出现冲突绑定时，Resolver 返回 BLOCKER，不静默选择。
- Shot 的显式绑定只覆盖祖先同 role 绑定；多个不同角色实例使用 role + ordinal，允许同一 Profile 在一镜多次出现。
- ReferenceSet 在解析时展开成当前可用的具体 Asset 顺序；一旦准备，顺序和 checksum 进入 Production Snapshot。

### 6.4 动态解析、快照和可复现性

- 编辑、预览、就绪度计算：动态解析当前 Profile/ReferenceSet/结构状态。
- Prepare 或 Start：冻结 Profile revision、ReferenceSet revision、具体 asset IDs/checksums、最终 prompt、workflow version、recipe、output spec、Comfy capability identity。
- 已创建 ProductionBatch item、ProductionRun stage 或 Generation Snapshot 的旧生产输入不随 Profile 编辑变化。
- 尚未准备的旧 Shot 使用最新有效 Profile；没有 Reference Pack 的旧 Shot 保持兼容的 legacy path，不能被强制改成 BLOCKED。

## 7. 数据模型摘要

逻辑表和字段见 companion 文档 docs/architecture/AI_STUDIO_0.7.0_ERD.md。核心实体如下：

- CharacterProfile：identity、canonical prompt、default style/reference set。
- CostumeVariant：角色服装变体、服装 prompt fragment、可选参考集。
- SceneProfile：环境、空间、时间、镜头可继承描述。
- PropProfile：道具 identity、材质、尺度和参考集。
- StyleProfile：画风、色彩、线条、负面提示和输出偏好。
- ReferenceSet：可复用集合的名字、用途、revision 和 ordered items。
- ShotProfileBinding：shot 与 profile、role、ordinal、costume variant 的关系。
- ShotReferenceSetBinding：shot 与 reusable ReferenceSet 的关系。
- ProfileRevision：最小可复现版本，不做复杂 event sourcing。

Asset Usage Graph 不使用图数据库。用 SQLite 现有关系加新关系查询：

1. Asset → ReferenceSetItem。
2. ReferenceSet → ShotReferenceSetBinding。
3. Shot → ShotProfileBinding / shot_reference_assets / generation links。
4. Profile → ShotProfileBinding / CostumeVariant / ReferenceSet。
5. 旧 ReferenceAnchor → reference_anchor_assets。

通过 set-based SQL 或有限次批量查询生成使用者列表，避免每个资产再发一条查询。

## 8. 后端服务边界

0.7.0 规划的独立服务：

| 服务 | 责任 | 是否独立 |
| --- | --- | --- |
| ConsistencyProfileService | Profile CRUD、继承字段校验、revision 最小能力 | 是 |
| ReferenceSetService | ReferenceSet CRUD、顺序、资产归属和使用关系 | 是 |
| ShotContextResolver | 结构继承、binding 合并、prompt context、stage adapter | 是 |
| ShotReadinessService | 后端权威 readiness、gate checks、批量查询 | 是 |
| ProductionPreparationService | 将 resolved/readiness 组合为 ShotProductionPlan，并调用现有 batch preparation | 是 |

不新增：

- NewProductionExecutor
- SecondQueue
- SchedulerService
- Graph database

现有 ProductionOrchestratorService、ProductionQueueService、ProductionBatch、ProductionRun 继续作为唯一执行边界。Preparation 只产生计划或显式创建既有 Batch，不直接 start。

### 8.1 规划中的 command signatures

这里只冻结接口形状，不实现：

~~~text
character_profile_list(project_id, profile_kind?) -> CharacterProfileSummary[]
character_profile_get(project_id, profile_id) -> ProfileDetail
character_profile_create(request) -> ProfileDetail
reference_set_list(project_id, profile_kind?, profile_id?) -> ReferenceSetSummary[]
shot_reference_pack_get(project_id, shot_id) -> ShotReferencePack
shot_reference_pack_update(request) -> ShotReferencePack
shot_resolve_context(project_id, shot_id, stage, mode) -> ResolvedShotContext
shot_preflight(project_id, shot_id, stage) -> ShotReadiness
scene_prepare_production(request) -> ScenePreparationView
~~~

所有 request/response 使用 camelCase DTO；后端是唯一事实来源。

## 9. 前端与 UI IA

不重做 StudioShell、StudioGlobalRail 或全局路由。沿用当前 C UI Workspace：

- Assets：Assets / Profiles / Reference Sets。
- Creation：Scene / Shot / Storyboard（Storyboard 只显示 P2 draft）。
- Production：Preparation / Queue / Runbook。
- Review：Image / Video。

### 9.1 Scene Production UX

- 左侧：当前项目的 scene → shot 列表，可按 readiness/status 筛选。
- 中央：Shot Production Card 网格。
- 右侧：Readiness、Reference Pack、Profile binding 和 blocker inspector。
- 顶部：总镜头、READY、INCOMPLETE、BLOCKED、已准备、已入队计数。

Compact Shot Production Card 至少显示：

thumbnail · shot ID · characters · scene · readiness · image status · video status

动作只有“打开”“准备”“加入生产”；加入生产后进入现有 Queue，不自动生成。

### 9.2 Batch UX

用户选择多个 READY Shot 后点击“加入现有生产队列”。系统：

1. 再次在后端确认 readiness。
2. 生成或复用已有 ProductionBatch。
3. 返回 batch detail 和 blocked/warning summary。
4. 不自动 Start All、不自动选择候选、不自动切换下一镜。

### 9.3 Review Compare UX

只规划前端效率层，不重设计审核后端：

- A/B candidate compare。
- 上一个 / 下一个。
- Confirm / Reject / Regenerate。
- 仍由用户选择首帧、REF2VA 顺序和审核状态。
- 当前 review regeneration 的显式用户动作可以保持，但不扩展成 unattended pipeline。

### 9.4 状态架构

新增三个轻量 Zustand 状态边界：

- Profile Store：当前 project 的 profile list、选中项、编辑草稿和 invalidation。
- Asset Library State：分页、筛选、标签、usage query 状态。
- Shot Preparation State：当前 scene、stage、selected shot IDs、plan/readiness、busy/error。

它们只缓存页面和请求状态；Profile、Asset、Shot、Queue 的事实仍来自 backend，不在前端复制整库。

## 10. 性能目标

基准项目规模：

- 1 project
- 20 episodes
- 100 scenes
- 500 shots
- 1000 assets
- 100 profiles

要求：

1. Scene 页面不能出现逐 shot、逐 profile、逐 asset 的 N+1 查询。
2. shot_resolve_context 支持单镜头；scene/episode/series 提供批量 resolve/readiness API。
3. Asset Library 继续分页；usage 查询按 project、profile、reference set、shot 过滤。
4. 一次页面加载返回 card 所需摘要，不把完整 profile、完整 asset metadata 重复嵌入每张卡。
5. 解析缓存 key 至少包含 project、shot、stage、profile revision、reference set revision 和 source updated_at。
6. 500-shot 项目中，准备页应能先显示结构和摘要，再渐进加载详情；不得为每张卡串行调用 Tauri。

## 11. 0.6.2 兼容性与 Migration 022 草案

Migration 022 只在本规划中草拟，不创建 sql 文件。

原则：

- 0.6.2 的 Project、Shot、Asset、ReferenceAnchor、Prompt、Queue、Run、Review 数据原样保留。
- 新表全部新增，不改旧表的必填字段，不重写旧 shot。
- 旧 Shot 的 Reference Pack 可以为 null；legacy shot 仍按现有 prompt/reference/stage config 运行。
- 打开旧项目不要求用户手动迁移；新表为空也能读写项目。
- 新表允许 nullable binding/revision，用于逐步采用；未来 migration 必须幂等、可回滚备份。
- Project Manifest 继续输出原有 reference_anchors，未来 manifest version 才增加 profiles/reference_sets。
- Project backup version 12 继续可读；0.7.0 后再明确 backup version 13 的追加字段策略。

ERD 和字段分类见 docs/architecture/AI_STUDIO_0.7.0_ERD.md。

## 12. Scope Freeze

### MUST

- Character Profile。
- Scene Profile。
- Prop Profile。
- Style Profile 的最小全局上下文。
- Reference Set 及 ordered asset items。
- Shot Reference Pack 和 shot profile/reference-set binding。
- ResolvedShotContext 和 deterministic prompt builder。
- Shot Readiness、七类 Preflight gate、批量 readiness。
- Scene Batch Preparation，READY 才能进入现有 queue。
- Asset Usage 查询（SQLite relations + query）。
- 0.6.2 legacy path 和 old Project open 兼容。

### SHOULD

- Profile Revision / Snapshot 的最小实现。
- Review Compare UX。
- Script Import V1：TXT、Markdown、JSON 生成 draft。
- CostumeVariant 的轻量扩展。

### DEFER

- 自动生成完整 storyboard。
- unattended generation、Scheduler、Auto-next、Auto-select。
- cloud sync、登录、多人协作、在线项目。
- plugin marketplace。
- AI/model training、LoRA training。
- autonomous director。
- second executor、second queue。
- complex timeline editing、audio DAW。
- DOCX/PDF/复杂脚本解析。

## 13. DEV Roadmap

| DEV | 主题 | 交付边界 | 依赖 |
| --- | --- | --- | --- |
| DEV-047 | Consistency Data Foundation | 新逻辑实体、ID/校验、repository contract、无正式 UI/执行接入 | DEV-046 |
| DEV-048 | Profiles + Reference Sets | Profile CRUD、ReferenceSet CRUD、asset validation、legacy anchor adapter | 047 |
| DEV-049 | Context Resolver | hierarchy、binding、prompt builder、ResolvedShotContext、动态解析 | 047、048 |
| DEV-050 | Readiness + Preflight | 七 gate、READY/INCOMPLETE/BLOCKED、批量结果、Comfy capability 合并 | 049 |
| DEV-051 | Asset Library 2.0 | usage query、Profiles/Reference Sets IA、搜索筛选反向使用 | 048、050 |
| DEV-052 | Scene Production Preparation | ShotProductionPlan、scene prepare、复用现有 ProductionBatch/Queue | 049、050 |
| DEV-053 | Review Productivity | compare、A/B、next/previous、手动 gate；不改审核后端模型 | 052 |
| DEV-054 | Integration | Creation/Production/Review IA、legacy fallback、backup/manifest read compatibility | 047–053 |
| DEV-055 | Release Gate | 测试、500-shot performance、upgrade/backup、0.7.0 release gate | 054 |

推荐 9 个任务；如果需要压缩，可将 DEV-051 与 DEV-053 延后为同一 P1 迭代，但不可跳过 047–050。

## 14. 依赖图

~~~mermaid
flowchart TD
  D046[DEV-046 Architecture Freeze] --> D047[DEV-047 Data Foundation]
  D047 --> D048[DEV-048 Profiles and Reference Sets]
  D048 --> D049[DEV-049 Context Resolver]
  D049 --> D050[DEV-050 Readiness and Preflight]
  D048 --> D051[DEV-051 Asset Library 2.0]
  D050 --> D051
  D050 --> D052[DEV-052 Scene Preparation]
  D049 --> D052
  D052 --> D053[DEV-053 Review Productivity]
  D051 --> D054[DEV-054 Integration]
  D053 --> D054
  D052 --> D054
  D054 --> D055[DEV-055 Release Gate]
~~~

硬依赖：047 → 048 → 049 → 050 → 052。
软依赖：051、053 可以在核心闭环稳定后并行，但不能改变执行层边界。

## 15. 风险与缓解

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| schema complexity | 迁移和查询变复杂 | 先做最小 profile/reference/set 表；不存冗余 context |
| duplicate ReferenceAnchor model | 两套“参考”语义漂移 | Anchor 保留为低层兼容；ReferenceSet 是新组合层，明确 adapter |
| Profile vs Asset confusion | 用户把语义档案当成文件 | UI 和 API 明确 Asset=physical、Profile=semantic |
| prompt overgrowth | prompt 过长、顺序不稳定 | 固定 builder 顺序、长度检查、每段来源可见 |
| N+1 | 500-shot 项目卡顿 | 批量 resolve/readiness、set-based SQL、分页摘要 |
| UI complexity | C UI 出现过多面板 | 仅增加当前 workspace 子页，Scene page 三栏固定信息密度 |
| old compatibility | 旧项目打不开或被强制重建 | 新字段 nullable，legacy path，不做人工 backfill |
| queue coupling | 准备层误启动或绕过 gate | Preparation 只 plan/add；唯一 start 仍是现有 Queue/Run command |
| reproducibility | Profile 修改改变历史结果 | Prepare/Start 冻结 profile/reference/prompt/workflow snapshot |
| profile version drift | 同一 Profile 在不同 shot 语义不一致 | revision identity 进入 snapshot，旧 prepared item 永不动态重算 |

## 16. 验证策略

0.7.0 实现阶段至少覆盖：

- service unit：profile validation、inheritance、binding conflict、prompt ordering。
- repository：CRUD、ordered ReferenceSet、asset project/type boundary、usage query。
- migration upgrade：021 → 022、fresh database、0.6.2 fixture preserve。
- context resolution：project/series/episode/scene/shot override、multiple roles、clear inheritance。
- profile revision：old shot snapshot stable，new shot uses new revision。
- reference deletion：被 ReferenceSet/shot/anchor 使用时给出 blocker/warning，不产生悬空静默关系。
- readiness：每个 gate 的 blocker、warning、incomplete、score。
- batch prepare：只创建现有 batch，不启动；blocked 排除，READY 可加入。
- compatibility：旧 project、manifest、backup、legacy shot 可打开。
- performance：500 shots / 1000 assets / 100 profiles 的 scene page 无 N+1。

不引入 microservices、GraphQL、Redis、Kafka、cloud database、event sourcing 或 CQRS。运行形态仍是 Tauri + Rust + SQLite。

## 17. Acceptance Criteria

0.7.0 功能实现完成时必须可验证：

1. 可创建 Character Profile。
2. 可创建 ReferenceSet 并绑定 ordered assets。
3. Scene 可绑定 SceneProfile。
4. Shot 可绑定多个 CharacterProfile、PropProfile，并为角色选择 CostumeVariant。
5. Shot 可绑定 ReferenceSet ID，系统能解析出具体 Asset。
6. 系统能为镜头生成 ResolvedShotContext。
7. 系统能计算 readiness status、score、checks、blockers、warnings。
8. Scene Batch Preparation 能一次返回所有 shot 的 plan。
9. BLOCKED/INCOMPLETE 镜头不会进入 ready batch。
10. READY 镜头可以加入现有 ProductionBatch/ProductionQueue。
11. 加入队列不自动生成；候选选择和审核仍为手动。
12. 0.6.2 Project 可以直接打开，旧 Shot 没有 Reference Pack 也能运行 legacy path。
13. 500-shot 项目在目标规模下可用，无 scene 页面 N+1。
14. Project Manifest、backup、Production Audit 的历史数据仍可读。

## 18. Non-goals

0.7.0 不承诺：

- 云端项目、登录、在线同步、多用户协作。
- 插件市场、模型训练、LoRA 训练。
- AI 自动导演、无人值守生产、自动选择候选。
- 第二执行器、第二队列、Scheduler 驱动的全自动生产。
- 复杂时间线剪辑、音频 DAW。
- DOCX/PDF 或复杂自然语言脚本解析。
- 自动覆盖用户已经确认的正式 Shot。

最终冻结结论：0.7.0 的价值在于“准备正确、输入可解释、资产可复用、执行可追踪”，而不是增加一个更大的自动化按钮。

## 19. DEV-054 已落地集成事实

- Creation workspace 在既有 shell 内增加 Project/Series/Episode/Scene/Shot 的一致性编辑；Project/Series/Episode/Scene 只展示 binding/inheritance truth，只有 Shot 页面展示最终 ResolvedShotContext、stage prompt 与 context hash；没有新增全局 consistency rail。
- Scope/Shot binding 已通过统一 pack command、后端校验和 SQLite combined transaction 开放给普通用户。
- shot_context_draft_get 复用现有单例 ShotContextResolver；不运行 Comfy live preflight，不创建第二 resolver/cache。
- Preparation snapshot 是用户明确准入时的历史证据；Profile/ReferenceSet 后续变化不会重算已冻结 prompt、asset order、context hash 或 stage input。
- Project Command Center 与 Production Audit 现在可读 consistency/preparation 摘要和 snapshot lineage；读取路径不启动生产。
- 无新 binding 的旧 Shot 保留 prompt/reference/stage-config legacy fallback。
- Manual Gate 仍保留：候选、关键帧、视频、审核、加入队列和 Queue Start 都由用户触发。
- DEV-054 不新增 migration 025、第二执行器、第二 queue、Scheduler、auto-start、auto-select 或 unattended generation。

下一任务：DEV-055 — AI Studio 0.7.0 Release Gate。
