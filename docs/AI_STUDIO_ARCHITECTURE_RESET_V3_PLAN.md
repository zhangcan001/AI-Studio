# AI Studio Architecture Reset V3 — 目标与渐进迁移方案

## Phase0 gate update — 2026-10-01

用户已批准 Option C / KEEP_ENGINE + REBUILD_CONTROL_PLANE、PROJECT_FIRST、SINGLE_APP_ROUTE、RUN_PROJECTION、ADVANCED_WORKFLOW_LAB；不改core/database。本轮仅补证及契约，未实施。

实际baseline `9f23e22bba73426534eabb080247db9228add91e`，Phase1 CI36841629302 exact head completed/success。B/C/E/F=PARTIAL，D=NOT VERIFIED；PRODUCT_CONTRACT结构已建立，但运行oracle尚有缺口，PRODUCT_CONTRACT_COMPLETE=NO，READY_FOR_PHASE_2=NO。明细见AUDIT的Phase0更新与PRODUCT_CONTRACT，旧Phase1观察不补签本轮gate。

必要设计澄清：当前最强媒体检索/预览入口是Asset Library，目标Runs统一运行状态、失败恢复和来源深链，**不取代媒体库、不合并底层Task/Asset/Artifact/Shot事实**。新Shot历史候选须区分本次生成与项目复用，不造provenance。accepted真实原文并非“生成成功”，需要修的是颜色/技术ID和终态反馈，不把伪引文作为oracle。

进入Phase2前仍需：完整图片queued/running/Review传播；H3同mode配置与历史关联；input修正与transient恢复；SUCCESS/FAILED/SUCCESS仅恢复失败leaf且旧attempt保留；native OCC冲突/draft与旧binding刷新/重启/显式升级；完整Shot History和输入reuse。repository OCC单项通过不能替代native冲突UX。以上仍使用隔离数据、合法UI/service和明确标识fixture，禁止SQL假状态或新GPU昂贵生成。

本轮即使后续gate闭环也不实现Facade；实现阶段另需用户明确指令。

日期 2026-10-01；设计依据 `master@fe00472950c12593892e6d80f97fcf6bf60776e6` 的当前源码与本轮隔离 Native 审计。对应 `AI_STUDIO_ARCHITECTURE_RESET_V3_AUDIT.md`。**这是方案，不是实现或发布验收；当前 READY_FOR_PHASE_2=NO。**

## 1. 明确决定

```ini
FULL_REWRITE_RECOMMENDED=NO
FRONTEND_SHELL_REWRITE=YES
APPLICATION_FACADE_REWRITE=YES
CORE_ENGINE_REWRITE=NO
DATABASE_REWRITE=NO
RECOMMENDED_OPTION=C
TARGET_NORMAL_FACADE_COUNT=52
NORMAL_GENERATION_ENTRY_COUNT=1
BIG_BANG=NO
```

保留现有 Queue、Task、Asset、Shot、Workflow、SQLite 和 repositories，不增加第二套 persistence。重建的是产品控制面：用户知道下一步在哪、提交意味着什么、失败怎么恢复、结果怎么选用；不是靠移动文件或重命名按钮达成重构。

普通目标仅图片+H3视频；质量模式保留精确秒数。快速预览、音频输入、独立音频生成/图音混合不回归普通产品。音轨在视频内保留，旧 audio references 和备份兼容读取，不能以隐藏 UI 为由删历史数据。

## 2. 三方案比较（预算，不冒充实测）

|维度|A 保留9入口整理|B 5区工作台|C Project-first 单工作区（推荐）|
|---|---|---|---|
|A首次使用|保留overview→creation跨页及scope意图修正|项目→创作1跳|项目直接新Shot，0跨页|
|B图片|原多个表单/入口仍竞争|创作主入口可集中|项目Create同Shot一页完成|
|C H3视频|需协调Studio/Shot/Run|创作模式+运行区域|Create stage/mode切换，素材就地选择|
|D失败|仍可能猜Review/Task/Queue|Runs主入口改善|Create failure卡→同项目Run详情，1深链|
|E换生成器|工作流→项目设置残留|设置→生成器→返回创作|Create picker即时作用域选择；高级进入项目/系统设置|
|F历史|多结果destination维持|Library与Runs仍需project上下文|同project Runs主结果、Library搜索投影|
|概念|原内部术语不自然消失|项目/创作/运行/素材/设置|项目与Shot上下文固定，运行/素材是任务视图|
|导航|9global+11workspace|5global+projectselector|2global，项目内4主页+secondary settings|
|技术身份|必须另做隐藏|必须另做隐藏|exact pair内藏，显示名/版本/模式|
|实现复杂度|低壳改动，高长期同步维护|中，需要跨区项目状态|中高，需要统一Route，减少长期状态组合|
|迁移风险|低短期，高长期碎片|中|中，可用routeadapter/feature rollout回退|
|复用engine|YES|YES|YES|
|扩展性|功能增长不断加入口|全局区会继续膨胀|新能力放项目内任务/高级，不扩global|

推荐 C 不是因为漂亮，而是 Project/Shot isolation 是现有真实边界，失败恢复/素材选用也需要保留同一project。A虽便宜不能解决 route双owner、raw identity 与普通CRUD编排；B可作迁移途中shell，不作为另一个产品架构长期并存。

目标路径预算（后续要实测）：A home新Shot≤2语义点击；B已有Shot→填prompt→生成1主动作，候选选用≤2；C同Shot切video→选模式/素材→生成，0跨workspace；D失败通知→运行明细1深链，修正输入明确返回Create，retry不重做成功项；E picker→版本→scope确认≤3，advanced Lab另计；F Runs/Library search→结果≤2。输入键击不算按钮点击，复杂模式素材选择按实际数量另计，不能强行给统一漂亮数字。

## 3. 信息架构与页面模型

```text
Global（2）
  项目切换/项目列表
  系统设置
Project workspace（4主页面）
  Overview（进度/下一步/阻断/最近结果）
  Create（当前Shot；多选为同页模式）
  Runs（排队/运行/失败/完成；结果与恢复）
  Library（图片/视频/角色场景道具/参考集/提示词）
  secondary: 项目设置（生成器默认/结构/备份）
System settings
  Runtime
  Generators
    Advanced Workflow Lab
  Tools（高级）
  Diagnostics
```

TARGET_TOP_LEVEL_COUNT=2 指全局，不把项目内4标签混成6个全局一级。Overview可作为打开项目默认，不占global rail；项目设置是secondary，不与Create抢主路由。Library隐藏本轮排除的audio主动入口；历史audio若存在可在“历史格式”中查看，不能丢失。

### Overview

只有一个明确下一步CTA，来自后端 aggregate推荐；空项目直接新建未归档Shot，不强制Handoff/一致性。复杂项目导入是secondary“批量导入”；blocked action给原因及修复位置。Recent results用同一ResultSummary，不重复构造五套统计。runtime显示“已连接/未检查/可用/阻断”分层。

### Create

围绕当前Shot：素材picker、prompt、generator、必要参数、生成、运行摘要、候选选用同页。当前stage/mode是route或draft显式字段。默认不显示UUID、node、recipe等实现名；时长单位秒、质量说明和输入数量约束清楚。首尾帧/参考视频模式按实际recipe供给显示，不能把不支持的模式伪装成通用Shot能力。

Generate先可定位必填问题，再调用后端权威准入；消息区分“请求已接受”“排队”“执行失败”“结果已生成”。后台失败事件即使已离开Create也更新同project Run projection；不能只打开production mode才订阅。预检不是执行授权，submit仍重新检查snapshot/OCC/runtime generation。

保留 StudioStore 作为既有session draft authority。新controller读同一store或受控adapter，不另建 competingCreationStore；旧Shot persisted stage values是server fact，dirty draft是编辑session，不自动覆盖。离开/切项目提示未保存变化，不在effect中无声清除。

### Runs

普通唯一生产对象为 **Run projection**。排队、运行、失败、完成是列表分组，不重定义Task状态机。Batch COMPLETED 可能含失败；PAUSED必须在“已暂停”而非完成。运行状态/结果/审核/选用四类事实分别表示。失败明细给中文原因、输入定位、“修正并新建运行”和仅临时失败可重试；保留旧失败Task、新attempt关联及成功结果。批次 retry 是原Queue恢复用例，不从UI重造所有item。

### Library

统一媒体浏览/搜索/筛选/来源/标签/收藏、profiles/referenceSets/prompt版本。统一是导航/查询facade，不把Asset/Prompt/Profile/ReferenceSet塞同一新表。每种资源仍有typed discriminator及原ID。使用媒体→Create intent，带projectId/assetId，后端校验usage和类型；Library不能另起GPU执行。大量媒体分页和缩略图，不能一次读全/把bytes复制到Zustand。

### Settings / Workflow Lab

普通generator卡：名称、模式、版本、输入需求、质量、可用性/下一步。project默认/Shot覆盖明确作用域；原exact pair和OCC opaque token照存，不从label反推。Lab保留导入、识别解释、映射、验证、发布、版本生命周期、promotion、历史、benchmark/quick test；diagnostics展示hash/path/node/raw codes。高级跳回Create应保留draft，不能把“测试运行”作为第二正常生成入口。

## 4. Route 与状态契约

建议判别式 AppRoute，单reducer，不必增加router依赖：

```typescript
// 设计示例，非本轮新增代码
// projectId、ids保留原ID；view source有明确判别式。
type AppRoute =
 | { kind: 'project-list' }
 | { kind: 'project'; projectId: string; page: 'overview' }
 | { kind: 'create'; projectId: string; shotId?: string; stage: 'image'|'video' }
 | { kind: 'runs'; projectId: string; run?: RunRef; filter?: string }
 | { kind: 'library'; projectId: string; resource?: ResourceRef; filter?: string }
 | { kind: 'project-settings'; projectId: string; section: string }
 | { kind: 'system-settings'; section: string; returnTo?: AppRoute };
type RunRef =
 | { source: 'production-run'; id: string }
 | { source: 'queue-batch'; id: string }
 | { source: 'task'; id: string };
```

RunRef只是locator，**没有新run表/新状态机/全局生成runId**。同一组合run/queue/task不能在列表重复算三次：投影明确preferred parent，standalone历史task单列；query cursor稳定按时间+source+id，project隔离。run_start仅可用于可启动batch/编排，task source不支持返回typed action denial；不默默submit新任务。

Shell只装配bootstrap、canonicalroute、project上下文、page；event/cache适配器单独生命周期。已有ProjectStore保持项目selection/cache；TaskStore作为server projection adapter；StudioStore session draft；WorkspaceResumeStore仅serialize兼容route，不与route双向争truth。breadcrumbs由route+entity titles派生，禁止父/子两个effect互写。

server-state key包含projectId+source/id+必要runtime generation。事件invalidate query，不在路由组件操纵历史状态机；mutation通过typedIPC。已有缓存requestId/cancellation保留。provider只在有稳定共享生命周期/数据合约时加入，不为架构图而加入。

## 5. Product Facades：52个普通用例预算

这是未来接口草案，不是创建52个Tauri命令的授权。现有可直接复用的aggregate（project_command_center、workflow_workspace_query、assetLibraryPage等）不要重复包同义接口。每个用例须说明输入、project隔离、sourceauthority、atomicity、deny/action结果；真实设计若47/55更合理可调，不为了数字合并不同权限/事务。

|Facade|候选普通用例（总计52）|委派 authority|
|---|---|---|
|Project（10）|list, get, create, update, structureGet, structureUpdate, generatorBindingsGet, generatorBindingSet, generatorBindingRemove, archiveTransfer|Project/Structure/Binding/Backup services|
|Creation（12）|get, createShot, updateShot, deleteShot, selectContext, updateDraft, generatorsList, generatorSelect, referenceSelect, readinessGet, generate, selectResult|Shot/Studio draft/Workflow readmodel/Admission/Queue|
|Run（11）|list, get, start, pause, cancel, retry, recoveryPlan, archive, restore, resultsGet, resultReview|Queue/Task/Orchestrator/Artifact services|
|Library（12）|list, get, importInspect, importCommit, useInCreation, favoriteSet, tagsUpdate, deletionInspect, delete, versionsGet, relationsGet, resourceEdit|Asset/Store/Organization/Prompt/Profile/Reference services|
|System（7）|get, runtimeStatus, runtimeConfigure, runtimeCheck, generatorSupply, environmentProfileApply, resumeGet|Settings/Comfy/Library/Resume adapters|

advanced WorkflowLabFacade / DiagnosticsFacade 不纳入52普通数，不限于“隐藏后移除handler”。普通UseCase注册/typedtransport需要allowlist，features不能直接importrawTauriinvoke，也不能普通controller继续调300低层wrapper。

### 深接口契约，不能泛用万能 mutation

- project.archiveTransfer 分 export/inspect/restore tagged request，只共享归档用例；restore必须明确用户授权、检查结果和同样的backup service补偿边界，不能client拼文件。若三个动作授权/错误模型差异大，应拆分并调整budget，**52不是硬KPI**。
- creation.updateDraft是更新既有sessiondraft的产品动作，frontend controller本地完成可不新增IPC；persist stage config走ShotService。**52包括产品用例，不等于52新增IPC**，最终普通IPC目标40–60经实装计量。
- creation.generate 输入project/shot/stage、exactpair token、values、idempotencykey；验证→持久化→queuecreate/start复用既有锁/事务。返回RunRef和accepted acknowledgement。不得将已经accept但start失败伪装成未创建；返回可恢复batchref、start outcome，使重复请求安全。
- generatorBindingSet携expectedBindingInstanceId+revision，后端沿用现有BindingService；冲突不自动覆盖，返回当前版本和显式重新确认。generatorSelect区分session override和project default，默认不改历史binding。
- run.retry按source委派现有requeue/partialResume/recovery，不抹原FAILED，不重新执行SUCCEEDED；来自task且snapshot不完整时返回修正输入深链，不盲目重建。
- resourceEdit是typedPrompt/Profile/ReferenceSet操作，不是任意JSON写库；大跨事务写入应独立command，不搞万能patch endpoint。
- query DTO含display状态和opaque identity，typed details按需；errors含stablecode、messageargs、field/actionlocation，不把Englishrawmessage当普通呈现。
- typedTauriClient/ipc仍唯一transport；内部服务只通过ports，facade不能直接sqlx写业务表。

## 6. 目标代码结构与规模防腐线

```text
app/App                   shell装配(<300行偏好)
app/routes                AppRoute reducer + legacy locator adapter
app/bootstrap             lifecycle + event bridge
features/project          overview controller/view
features/create           controller + existing StudioStore adapter + panels
features/runs             Run projection controller/results/recovery
features/library          typed resource controllers/media primitives
features/settings         runtime/generators
features/workflow-lab     existing advanced service/client boundary
services/product/*        typed product usecases（复用现有ipc）
application/product/*     usecase facade/readmodel，不新repository authority
application/*existing*    服务authority + 内部模块
application/ports         existing repository/Comfy/FS ports
domain                    original identity/state machines/definitions
infrastructure            SQLx/Comfy/AssetStore/filesystem
```

page偏好<500行，controller<300–400，单CSS<1000–1500；超限记录多职责/接口理由，不机械FAIL。依赖guard：pages→controllers→productclient→typedIPC；productfacade→existingservices→ports；禁止componentrawinvoke/SQL writes/newtaskqueue/newdrafttruth。

先稳定facade和route，再迁内部Rust模块。Backup保留一个恢复协调authority，onboarding保留一个commitauthority，Queue保留一个workerauthority；pure graph/normalizer按算法阶段组织。test模块分开可提阅读性，但不得借此删测试或改fixture语义。

## 7. 完整主要功能迁移矩阵

|CURRENT_LOCATION/功能|TARGET_LOCATION|决定|BACKEND_AUTHORITY|PHASE|
|---|---|---|---|---|
|ProjectCommandCenter overview/nextstep/dailyboard|Overview|REFACTOR|CommandCenter aggregate|2|
|Projects list/create/template|global项目列表|KEEP/MERGE|Project/Organization|2|
|Projects config/backup/manifest|项目设置|MERGE|Binding/Backup/Structure|2/6|
|GenerationStudio single|Create|MERGE|Generation/Queue/StudioStore|3|
|GenerationStudio batch/experiment|Create多选/高级实验|MERGE/HIDE|Queue/Benchmark|3/6|
|Shot structure CRUD|Create tree/项目结构|KEEP|Structure/Shot|3|
|Shot prompt/params/references/stage|Create|REFACTOR|Shot/Prompt/Consistency|3|
|Shot candidates/selectedresults|Create resultpanel|KEEP|Shot/Asset/Task|3|
|Consistency scope/profiles/costumes|Library档案+Create context|MERGE|Consistency repositories|3/5|
|AssetVideoBatchWorkspace|Create多选video|MERGE|Queue/Asset|3|
|DirectGenerationEntry|Create|RETIRE独立表单|Queue|3/7|
|ProductionRun image→video orchestration/templates|Runs组合细节+Create preset|KEEP/MERGE|Orchestrator→Queue|4|
|ProductionQueue controls/itemretry/partialresume|Runs|MERGE|Queue|4|
|Scene/Episode/Series/Project batches|Create多选/高级导入|KEEP/HIDE细节|Preparation/Queue|4|
|ShotBatch plan/bulkprompt/config/import|Create multi/import|KEEP|ShotBulk/ShotBatch|3/4|
|ProductionPackage discovery/inspect/mappings/runbook|Runs高级导入|HIDE|Package/Queue/Runbook|4|
|External Production Handoff dryrun/mappings|项目批量导入|HIDE默认教程|ExternalHandoff/Structure|4|
|H3 Local directory import/segment editor|Library高级导入/Run关联|KEEP/HIDE|H3LocalImport/Asset/Queue|5|
|TaskHistory/detail/inputreuse/recovery|Runs details→Create intent|MERGE|Task/Snapshot|4|
|ArtifactReview board/history/open/reveal|Run/Shot resultreview|MERGE|ArtifactRepository|4|
|Asset library/sourceimport/deletion/versions/relations|Library|KEEP/REFACTOR|Asset/Browse/Store/Usage|5|
|Tags/favorites/referenceanchors/sets|Library|KEEP|Organization/ReferenceSet|5|
|PromptStudio/library/version/model/toolprovenance|Library prompt editor/Create picker|MERGE|Prompt/Model/Tool/Lineage|5|
|WorkflowWorkspace importwizard/maps/validation/publication|Workflow Lab|HIDE普通/REFACTOR|Onboarding/Registry|6|
|Workflow lifecycle/promotion/versiondiff/history|Workflow Lab|KEEP|Lifecycle/Registry/History|6|
|Workflow quickrun/benchmark/quality/recommendation|Lab验证|KEEP高级|Benchmark/Queue|6|
|Project generatorselection/runtimeprofiles/presets|Create picker+项目设置+systemgenerators|MERGE|Binding/Settings/Preset|3/6|
|Runtime connect/capability/environment/profile/freecache|System settings|KEEP高级危险操作|Comfy/Settings|2/6|
|LocalToolHub|System高级工具|HIDE一级|Tool service|6|
|Productionaudit/lineage/integrity/diagnosticsexport|Diagnostics|HIDE普通技术详情|Audit/Lineage|4/6|
|Repairjob status|Diagnostics|KEEP只读|RepairJobRepository|6|
|Workspace resume/search/breadcrumbs|canonicalroute adapter|REWRITE|Route owner+resumeStore|2|
|Audio历史类型/快速预览旧引用|历史只读兼容|HIDE/RETIRE主动入口|existingAsset/definitions|5/6|

“RETIRE”指原入口最终退役，不是删除领域服务或历史数据；所有row要有新destination/invariants证明才进入Phase7。feature CSS随row迁移，不提前整库清理。

## 8. Compatibility / 数据策略

|不变量|迁移方式|证明/退出门槛|
|---|---|---|
|Database42/71，无043|新壳/facade零schema迁移默认|真实旧副本打开；no新增migration；不是空迁移占号|
|Project/Shot IDs与隔离|route/query/mutation必带originalprojectId|两项目交替深链无串数据|
|旧无Shot任务|RunRef task source，不伪造Shot关联|Task历史/参数/asset来源可读|
|旧Workflow/Recipe exactpair|opaqueDTO不改versionIDs|库current切换不迁旧binding；缺版本明确诊断|
|Binding OCC|instance+revision原协议|并发改同binding冲突；repair/backups兼容原逻辑|
|Queue/Task历史|既有state machines/repositories|resume/retry不多执行成功项；PAUSED不terminal|
|Asset IDs/bytes/references|typedprojection，不复制数据表|read/play/use/deleteguard/lineage一致|
|Backup v20|复用现有服务/格式；不bump|旧缺字段可读，新字段留存，冲突不覆盖|
|Runtime package supply|exact selection+capability generation|冷启动/重连/失配状态可解释，原compiler行为不变|
|Provenance/RepairJobs|继续immutable snapshot/checkpoints|新UI只折叠；诊断/失败重试可达|
|Resume11workspaces|legacy→Route纯adapter，保持返回legacy能力|shots section兼容默认明确，新route完整可恢复|
|用户dirtydraft|一个StudioStore/shoteditsowner|切project/route/generator保留或明确确认|

任何后来发现必须迁移schema的改变必须独立说明必要性、compat/rollback和用户授权，不在架构重置顺带增043。UI rollback不能假装数据库 down migration；本计划零迁移使回退可靠。

## 9. Strangler：10个阶段（本轮只Phase1审计文档）

下面编号是**后续迁移工程 Phase0–9**，与本任务标题Phase1不是同一个实施序列。Phase2开工还需补安全UX证据+用户评审授权。每阶段独立可部署checkpoint；不并行改多个authorities。

|阶段|ENTRY_GATE|EXIT_GATE|ROLLBACK_BOUNDARY|LEGACY_PATH_REMOVED?|
|---|---|---|---|---|
|0 Architecture freeze/证据补齐|本两文档、基线、用户确认方向|补B/C/D/E/F正向/恢复证据；确认identity/route/RunRef、资源/测试budget|仅docs/evidence撤回，无数据变动|NO|
|1 Product Facade seam|Phase0通过，scope及typed契约评审|代表3–5完整用例委派旧authority，typederror/idempotency/OCC parity，普通clientallowlist|切回旧typedclient，facade不另存事实|NO|
|2 Canonical Route + shell/Overview|facade代表用例通过|legacy11locator映射、projectresume/back/invaliddeepLink、3viewport；一个routeowner|入口flag回legacyShell，routeadapter保留读|NO|
|3 Create 单入口|shell/route稳定、dirtydraft契约通过|图片/H3 modes/输入/候选/选用/幂等/失败就地；无第二StudioStore|perproject入口回Shot/Studio，draftadapter可读|NO，先只隐藏旧normalentry|
|4 Unified Runs|RunRef去重/投影actioncapability评审|queue/task/orchestrator映射、事件刷新、pause/retry/partialsuccess/恢复；failure→Create link|回旧queue/history/orchestrator，只换UI|NO|
|5 Unified Library|Runresults稳定、资源typedrefs固定|分页/媒体/Prompt/Profile/ReferenceSet/usage/deleteguard旧历史兼容|回旧Asset/Prompt页，原repo事实未变|NO|
|6 Workflow Lab隔离|normalgeneratorpicker+OCC+runtime状态一致|导入映射发布版本历史benchmark工具/diagnostics都可达，普通不露UUID|恢复旧workflow入口；不改版本/绑定|NO|
|7 Legacy workspace退役|功能矩阵逐row100%迁移或明确退休，用户验收|依赖/import/route/handler usage证明；旧deepLinkadapter；删候选逐项审查|可回恢复旧文件checkpoint，不回滚领域历史|YES，仅已证明旧UI路径|
|8 Backend 内部模块分解|productboundary稳定，选单service|调用接口/锁顺序/事务/补偿/fixtures语义一致，targetedparity|revert单service结构提交，既有DB不动|NO domain删除|
|9 Legacy CSS清理|对应旧UI已退出、selectorusage盘点|3viewport/keyboard/focus/statuscontrast回归，无globalleak新增|恢复对应旧styles checkpoint，tokens不变|YES，仅无调用方styles|

顺序重点：先契约后壳、Create后Runs并共享投影，不先清CSS，不在UI重写同时重排Queue锁。Phase8可按独立module分小checkpoint，但不得扩大为第二engine。遇门槛失败保持legacy入口可用，不留下两套都不能用。

## 10. 验证计划、资源预算、完成标准

本轮 docs-only：git diff --check、scope审计，不跑全量CI。后续每个任务先RAM/VRAM/runner检查，最多10核心case，targetname显式选择、串行；额外数量/全量套件必须用户明确授权。compile/check不是无限资源freepass，CI/localwide gate未跑标NOT VERIFIED。不要把“最多10命令、任意case”偷换用户最多10case规则。

Phase0建议6场景共用隔离数据集，不跑GPU：A首次新Shot、B成功图片候选/选用、C H3模式/素材/时长与既有成片、D失败修正及partialsuccess恢复、E binding保存+revision冲突、F旧resultlookup。fixture必须显式标示，不能冒充模型真实生成；真实端到端GPU如确需另获授权。后续阶段按风险选≤10case，跨层改动需更广gate时记录缺口并申请，不擅自全量。

关键验收：normalUI不出现rawIDs/hash/path；Generate accepted不是SUCCEEDED；terminal事件映射和nullable显示正确；切项目不串cache；RunRef去重不丢task；错误能回输入；旧成功asset/旧binding不可变；只有Queue能执行；backupv20可恢复；所有退役入口有替代/明确用户同意。规模预算只是辅助，任一状态/数据不变量失败即不发布。

## 11. 删除候选（本轮全部未删除）

|候选|理由|删除前证据/不能删的部分|
|---|---|---|
|App workspace/section多套routing helpers|双owner/resume缺section|11locatoradapter和backdeepLink通过后；保留compatread|
|Shot production/review专属内嵌panels|create/queue职责混合|Runs/Review功能完整；Queue/Artifact服务留|
|Studio/AssetVideoBatch/DirectGeneration独立normal表单|多生成入口|Create覆盖single/batch/provenance/旧taskreuse后|
|重复runtimewarnings/queue统计/greennotice|当前多处不同更新|同一projection+statusprimitives证明；不能删backendvalidation|
|CommandCenter aggregate镜像/legacyshotsstructure state|serverprojection复制|所有viewfallback调用、tests确认；不是只看setter就删|
|旧TaskHistory主入口/Review一级nav|用户猜结果位置|Runs/Library深链可达；保留diagnostichistory/reviewfacts|
|Workflow普通技术卡/UUID常显|技术泄漏|Lab+诊断可达、exactpair不变；不删IDs|
|legacy App.css无调用selectors|跨代层叠|usage scan+3viewport+focus/hover/errorregression|
|未使用compat UI或testfallback|可能只服务旧测试|namespace/dynamic/importgraph确认；test-onlybridge不先判dead|

不删任何unknown文件，不清用户DB/cache/media，不以“架构简化”清历史migration/runtimepackage。audio/fastpreview仅普通入口退休，历史引用不破坏。

## 12. 风险登记与停止条件

|风险|触发点|控制/停止条件|
|---|---|---|
|数据丢失|旧UI退役、import/delete|repo不换；usageguard/backup；缺restoreproof停止|
|identity corruption|name→pair选择、版本升级|opaqueexactpair+原ID；缺版本不自动替换|
|Queueauthority绕过|facade.generate/Runstart|只能委派QueueService，禁止直接Comfy `/prompt`|
|OCC回归|projectdefaults/Lab/repair|原instance+revision，冲突显式，wholeReplace禁用|
|backupcompat|facadearchive/Library资源整合|v20原format，旧缺字段兼容；不改restore事务|
|旧project/无Shot任务打不开|Create唯一路径|RunRef task source+legacyadapter，不能造Shot对应|
|Task历史丢/终态误判|Runprojection去重|保留childlocator；状态与review分列|
|asset引用破坏|Library合并/删资源|originalIDs/bytes/usage/lineage，先inspect再delete|
|runtimepackage选错|displaynamepicker/cache|exactversiontoken+runtimegeneration、冷启动明确检查|
|导航/草稿丢失|route/resume/controller改写|singleowner+dirtyconfirm+returnTo；记录回退路径|
|readmodel不一致|events/polling/projectswitch|project-scopedkeys、subscription单lifecycle、cancel/requestId|
|重复执行|accepted/start失败/重试|existingidempotency/atomicadmission，成功项不重做|
|Godfacade/万能CRUD|52数字压力|typedusecase/权限/事务边界，宁可拆增数不混域|
|迁移半成品|过早删legacy|每phaseEXIT后再隐藏，Phase7才删，rollback可验证|
|过度基础设施|新增provider/cache库/微模块|复用现有store/queryseams；无证明不加依赖|

当前缺口不是“再修完所有bug才能设计”，而是不能把有限native观察写成完整UX验收。方案方向可评审，实施必须先满足Phase0并获用户确认。本轮不开始任何迁移阶段，不修改生产实现。
