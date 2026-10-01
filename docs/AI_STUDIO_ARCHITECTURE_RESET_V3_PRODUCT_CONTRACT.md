# Architecture Reset V3 — Product Contract

日期：2026-10-01；Phase0B证据基线 `397118ffb91c7dd623ec4ff70f1e563811c09e43`。指定CI36846060015 exact head completed/success。本文件作为已冻结迁移oracle，Phase0B **PASS**；实现入口资格已具备，但本轮不实施Facade。PASS只覆盖本轮收窄gate，不等于旧UI完美、全平台UAT或新GPU验收；完整实测边界见AUDIT的Phase0B权威节。

## Frozen classification（所有下述domain字段按此解释）

### PRESERVE

- Exact WorkflowVersion + Recipe identity、项目隔离；名称和current version不决定身份。
- Production Queue唯一执行authority；Studio Store唯一创作state authority；Facade仅协调现有service/repository ports，不新建executor/task model。
- Task/attempt历史不可变；原失败保留，新运行/attempt独立id及合法lineage；Asset媒体/来源/历史保留。
- Shot selected result持久化；选用不删除其他candidate；Task状态、Asset存在、Artifact Review、Shot选用互不等价，Review独立。
- OCC revision + binding instance保护，包括ABA；冲突不静默覆盖，保留draft并可见serverlatest。
- partial success只恢复可恢复失败leaf；成功项不得重新执行，其Task/Asset/SHA保持。
- old generator binding不随catalog/library刷新/restart自动升级，仅用户明确select/save改变exact pair。
- Backup v20兼容、历史运行/媒体引用；route locator/resume保留project/Shot/Task/Asset身份及dirty-draft意图。

### REPLACE

- workspace + section竞争式routing → single AppRoute；普通生成入口收敛到单Create上下文。
- 分裂的结果查找/失败状态投影 → Runs统一执行事实及恢复导航，链接而非吞并Library/Shot。
- 普通UI原始技术ID → 友好Generator/模式/版本/输入；exact identity仍内部使用。
- 当前input-error缺直接编辑CTA、OCC必须去另一readiness面板比server值 → 明确修正/冲突确认路径；不改变底层immutable snapshot/OCC。

### LEGACY_DEFECT

- accepted/started绿色通知可像成功；原文没有“生成成功”，禁止伪引文；Create缺完整独立queued/running反馈。
- 同一failed Task的Create/Shot History状态滞后、queue strip失败计数不同、Overview只聚合且返回入口需猜测。
- nullms；Production英文raw error/code、WFV/Recipe/asset UUID及内部mode泄漏。
- 新Shot显示项目历史候选却缺明确来源范围；未保存参数可能被其它子表单保存重载复位；保存动作需清晰。
- 以上是迁移排除项，不先修旧UI作为本轮开工门槛，不描述成本轮已修复。

### ADVANCED_ONLY

WorkflowVersion ID、Recipe ID、Task/batch/execution诊断ID、Node ID、hash、package path、mapping evidence、repair internals、runtime provenance/telemetry/raw diagnostics。保留真实可达技术详情，不作为普通使用的唯一解释；旧Task无Shot也可诊断，不造假关联。

下方每节的PRESERVE属于上述保留域；DO_NOT_PRESERVE是REPLACE/LEGACY_DEFECT的历史用语，技术详情迁至ADVANCED_ONLY。其余规范性句子描述替代产品目标，不自动等于已实现。

## 1. Product mental model

用户在项目内准备图片/视频、启动运行、查找结果、选用结果。普通产品使用 Generator；Workflow、Recipe、节点与运行包属于高级诊断/Workflow Lab。用户已批准 KEEP_ENGINE / REBUILD_CONTROL_PLANE、PROJECT_FIRST、SINGLE_APP_ROUTE、RUN_PROJECTION、ADVANCED_WORKFLOW_LAB。

PRESERVE：项目隔离、既有身份和历史引用。DO_NOT_PRESERVE：迫使用户在多个工作区猜测任务是否完成。Run 是既有执行事实的投影，不是新 executor 或新任务数据库。

## 2. Create contract

PRESERVE：输入、参考素材、生成器选择、参数与来源进入既有 typed submission 路径；真正执行经 Production Queue。准备/创建批次不等于执行；accepted 不等于 succeeded。

普通用户必须能区分未提交、已接受、待执行、执行中、成功、失败/取消。只有实际终态成功才显示成功；提交中、排队中不得允许无意重复提交。必填输入错误应指出可修正字段，修正后产生新运行，不能重试同一错误 snapshot。

已观察：隔离 Shot 使用合法 UI 提交到临时 Comfy 边界，两个 Task 经正常执行路径 SUCCEEDED 并各生成两个 Asset，均为 **FIXTURE**。绿色文案原文是“已加入普通生产队列 pbt_… 并开始处理，镜头候选仍需手动选择。”，**没有字面写“生成成功”**；颜色/批次 UUID 与缺少完整状态反馈仍不应保留。Queued/Running 全部 UI 文案未完整采集。

## 3. Run contract

PRESERVE：既有 batch/item/task/execution identity、幂等 admission、不可变输入、历史 attempts、取消及恢复边界。PAUSED 不是终态；Task SUCCEEDED/FAILED/CANCELLED、Batch COMPLETED、Item SUCCEEDED/FAILED/CANCELLED/SKIPPED 必须按各自域解释。

投影需覆盖 Create、Production、Task、Shot History、Overview，不向 UI 复制一套执行 authority。状态订阅/刷新必须项目作用域；切换项目不能把前项目响应写回新项目。

核心恢复 oracle：SUCCESS/FAILED/SUCCESS 批次只恢复失败 leaf；成功项 1/3 不再提交；旧失败 attempt 保留并关联新 attempt。**Phase0B已用正式dev061b服务fixture扩展验证**：A/C item、Task与Asset/SHA不变；旧B失败保留，仅新增B child/Task并成功。COMFY_OFFLINE先安全暂停，需显式继续剩余C后进入三叶partial场景。集成证据不是完整Native/GPU/admission UAT。

## 4. Result contract

四种事实分列：Task status、Asset existence、Artifact review state、Shot selected result。彼此不得等价。SUCCEEDED 不是 Review 自动通过；选择候选不是删除其他候选。

PRESERVE：同项目/类型校验、来源 Task、原媒体和历史引用；结果选用持久化。正常 `ShotService.select_result` 允许同项目既有媒体作为候选，非 linked-task 选用不能伪造来源或补造 Task。

已观察：新 Shot 图片页在生成之前已经展示同项目历史图片；随后显示新 fixture 输出。合法确认后 selectedImageAssetId 为 `ast_74f7051a-c289-4e3a-a198-ce6238d91eff`（**HISTORICAL_REAL_OUTPUT**）。视频页可展示、播放并确认既有 H3 首尾帧 quality 2.1.1 的一秒成片 `ast_dd7f218a-facc-4be5-a9d1-9287d2cfdc75`。原 Task 与其他 Asset 仍存在。

必须明确区分“该 Shot 生成的结果”与“同项目可复用媒体”，不能让候选存在被误读为本 Shot 已完成。当前素材预览是最佳媒体查找入口，Task 是溯源诊断入口；目标 Runs 应统一运行状态并链接 Library/Shot，**不是取代 Library 媒体库**。

## 5. Generator selection contract

PRESERVE：exact `workflowVersionId + recipeId`，不能按名字或 current version 推断。保存仅影响用户明确选择的项目 slot，不自动启动生成。

OCC 必须携带 binding_instance_id + revision；陈旧 update/remove 不能覆盖服务端新值，删除后重建也不能 ABA 覆盖。冲突时显示服务端最新选择、保留本地 draft，并提供重新选择/确认路径。

旧 binding 在 catalog/library refresh、app restart 后保持 exact pair；只有显式选择并保存才升级。不可用旧版本显示不可用及原因，不自动替换。Backup v20 兼容不因 UI 改造改变。

上一轮Phase0A native 普通保存/reload：MiniMax H3 高质量首尾帧视频，WFV `wfv_ed7bd1b1-52ab-41b8-93de-f6138c1122bd` + Recipe `rcp_18d5fe30-a428-4be8-a62a-f19c7812322b`，VIDEO/DEFAULT revision=1。显示完整 UUID，选择器不显示友好版本号，属于 DO_NOT_PRESERVE。

Phase0B用Native stale form + 独立实际service writer完成OCC：中文“项目工作流绑定已被其他操作修改，请确认最新绑定后重试。”、draft保留、server不覆盖；最新server选择经“检查开工条件”可见，非内联比较（legacy）。I2V2.0 old pair经catalog/library刷新及app restart保持revision3/instance；Native显式选择I2V2.1、Save/reload得到 `wfv_cf180eea-ff88-4938-bb7b-6bee4a65633a` + `rcp_201e838e-1880-40ab-8a3c-fc6bc1fccd93`，revision4，同instance。SAVE_RELOAD/OCC/EXPLICIT_UPGRADE=PASS，AUTO_UPGRADE/SILENT_OVERWRITE=NO。普通picker友好version仍缺，exactpairUUID泄漏不应保留。

## 6. Failure/recovery contract

输入错误：返回 Create → 修正字段 → 新运行。Transient 执行错误：仅在正式可恢复判定及 frozen input 支持时 retry/requeue；submission outcome uncertain 不得盲目重复提交。

恢复必须保留旧 Task/错误详情/原 exact pair 与 lineage，成功 leaf 不重做。用户能看到失败事实、原因与有效操作；不能仅导航到 Production 才刷新失败。

Phase0B Native同镜头width=0创建 FAILED `tsk_56d50ee8-b565-48e8-baeb-222c104201b5` / INPUT_OUT_OF_RANGE；Task提示中文、transient retry禁用。手工回Create镜头定位编辑width=64，新batch `pbt_4b44267a1e74447896dcd862f9ada9db` / Task `tsk_518f0919-4384-4e13-9f39-501a64fc0581` SUCCEEDED（媒体FIXTURE），旧失败完整行未变。D2原正式offline recovery用例PASS，D3三叶fixture仅B新attempt，A/C不重做。输入修正可用但缺直接edit CTA；Create/History投影不同步仍legacy，不改业务代码。旧轮prompt_id不一致incident仍只算夹具错误，不充当本轮recovery证据。

## 7. Route/resume contract

PRESERVE：项目、Shot/Task/Asset locator、返回来源、有效 dirty draft。单 AppRoute 承担导航状态；持久化 resume、deep link/back 与页面状态不得各维护竞争 owner。

旧 Task 即使没有 Shot 也必须可打开；不能为路由便利造 Shot。已观察 Library“查看生成任务”打开真实历史 Task；“用于创作”按钮存在，但本轮完整 reuse 操作未执行。

## 8. Advanced Workflow boundary

普通 Create 暴露友好 Generator 名称、模式/版本、参考素材、提示词和参数。UUID、node target、package/path/hash、内部 mode enum、batch/task identity 移到可达的高级诊断，不删除底层身份。

保留 Workflow Lab 导入、分析、发布、版本、生命周期能力及旧任务 diagnostics；不通过普通入口退役去破坏历史资源。

## 9. Engine invariants

PRESERVE：Production Queue 单执行 authority；Studio Store authority；repository ports + SQLx persistence；project isolation；不可变 workflow/recipe/task snapshots；精确版本引用；原 idempotency/OCC/lifecycle boundaries；Task/Asset/Artifact/Shot 分离；Backup v20 和历史媒体。

禁止第二 queue/executor/task model、Facade 直接 POST /prompt、名称猜身份、whole-replace 绕过 slot OCC、手写 SQL 假历史。UI facade 只能协调既有服务，Run projection 只读派生事实。

## 10. Legacy defects intentionally NOT preserved

|ID|Observed / required evidence|User impact / authority|Expected V3 / regression oracle|
|---|---|---|---|
|L-ACCEPT|原文 accepted/started，绿色提示含 batch UUID；完整 queued/running 文案缺口|提交提示不等于 Task 终态；Queue/Task authority|分别呈现 accepted/queued/running/succeeded；延迟成功/失败场景|
|L-FAIL-PROJECTION|Phase0B同一INPUT_OUT_OF_RANGE Task：Create strip 0失败、History已排队，Production/Task失败、Overview聚合失败；AUDIT记录五surface|同一事实的错误可见性依赖入口和重开；不是底层Task丢失|五surface project-scoped Run投影一致；毫秒传播SLA未测|
|L-IDENTITY|项目 picker 完整 WFV/Recipe UUID；Shot recipe UUID、内部 ref2va；Task 包/目标/ID|普通用户理解负担；底层 identity 正确保留|普通友好标签，Advanced 显示诊断；exact-pair parity|
|L-RESULT-SCOPE|新 Shot 在未生成前已有项目历史候选|可能误认本 Shot 已有结果；Asset 与 selected Shot 分离|明确本次输出/历史复用；选用不造 provenance|
|L-NULL-TIME|Phase1 已观察 nullms；本轮不重复复现|缺失时长被当有效数字|缺值显示未知；零值与缺值分开|
|L-MULTI-ENTRY|Library→Task 可达，Create 候选/选择又是另入口|找结果需猜入口|Runs 统一状态/来源导航 + Library 媒体检索，不删除任一事实|

以上是排除的 UX 缺陷，不是本轮修复记录。不得将规范性要求标为已经实机通过。

## Evidence closure gate（Phase0B权威）

|Journey|Current evidence|Gate|
|---|---|---|
|B|正常fixture成功Task/Assets与候选、历史图片合法选用/持久化及Task/Asset保留；缺独立queued/running和部分投影滞后已列legacy|PASS_WITH_KNOWN_LEGACY_DEFECTS|
|C|镜头03同Create Video：正式I2V2.1 + reference + prompt编辑应用 + duration_seconds=1并保存，Generate可见但未点击；历史真实quality2.1.1播放沿用既有证据，不混称同一输出|PASS|
|D|Native确定性validation修正后新Task成功、旧失败完整保留；原transient测试 + 三叶partial集成PASS；同失败Task五surface差异明确记录|PASS|
|E|普通Save/reload、Native冲突渲染/草稿保留/serverlatest、不覆盖；旧pair刷新/restart不升级；显式2.1升级持久化|PASS|
|F|用户已收窄并接受角色：Runs执行状态/恢复，Library持久媒体发现/复用，Shot创作/选用；不追加完整reuseUAT|PASS|

核心domain invariants有实测证据 + legacy defects明确 + replacement contract冻结，是当前entry gate。旧UI无需先完美。详细case身份、fixture适用范围、失败重跑与10项预算见AUDIT；全平台/完整Library reuse/传播时延SLA仍NOT VERIFIED。

```ini
PRESERVE_SECTION=YES
REPLACE_SECTION=YES
LEGACY_DEFECT_SECTION=YES
ADVANCED_ONLY_SECTION=YES
DOMAIN_INVARIANTS_FROZEN=YES
PRODUCT_CONTRACT_COMPLETE=YES
PHASE0_EVIDENCE_COMPLETE=YES
ARCHITECTURE_CONTRACT_FROZEN=YES
READY_FOR_IMPLEMENTATION_PHASE_1=YES
ARCHITECTURE_IMPLEMENTATION_STARTED=NO
```

到此停止；不自动开始Facade、Route或Run实现。后续重构必须用本契约回归，不把暂存fixture媒体充作新真实GPU结果。
