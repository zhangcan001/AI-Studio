# Architecture Reset V3 — Product Contract

日期：2026-10-01；源码基线 `9f23e22bba73426534eabb080247db9228add91e`。本文件是迁移 oracle，不是实现方案、完整验收声明或 Phase2 开工授权。Phase0 **PARTIAL**；未验证项不能由源码推断替代。

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

核心恢复 oracle：SUCCESS/FAILED/SUCCESS 批次只恢复失败 leaf；成功项 1/3 不再提交；旧失败 attempt 保留并关联新 attempt。**本轮该三项运行 fixture 未执行，不能宣称已验证。**

## 4. Result contract

四种事实分列：Task status、Asset existence、Artifact review state、Shot selected result。彼此不得等价。SUCCEEDED 不是 Review 自动通过；选择候选不是删除其他候选。

PRESERVE：同项目/类型校验、来源 Task、原媒体和历史引用；结果选用持久化。正常 `ShotService.select_result` 允许同项目既有媒体作为候选，非 linked-task 选用不能伪造来源或补造 Task。

已观察：新 Shot 图片页在生成之前已经展示同项目历史图片；随后显示新 fixture 输出。合法确认后 selectedImageAssetId 为 `ast_74f7051a-c289-4e3a-a198-ce6238d91eff`（**HISTORICAL_REAL_OUTPUT**）。视频页可展示、播放并确认既有 H3 首尾帧 quality 2.1.1 的一秒成片 `ast_dd7f218a-facc-4be5-a9d1-9287d2cfdc75`。原 Task 与其他 Asset 仍存在。

必须明确区分“该 Shot 生成的结果”与“同项目可复用媒体”，不能让候选存在被误读为本 Shot 已完成。当前素材预览是最佳媒体查找入口，Task 是溯源诊断入口；目标 Runs 应统一运行状态并链接 Library/Shot，**不是取代 Library 媒体库**。

## 5. Generator selection contract

PRESERVE：exact `workflowVersionId + recipeId`，不能按名字或 current version 推断。保存仅影响用户明确选择的项目 slot，不自动启动生成。

OCC 必须携带 binding_instance_id + revision；陈旧 update/remove 不能覆盖服务端新值，删除后重建也不能 ABA 覆盖。冲突时显示服务端最新选择、保留本地 draft，并提供重新选择/确认路径。

旧 binding 在 catalog/library refresh、app restart 后保持 exact pair；只有显式选择并保存才升级。不可用旧版本显示不可用及原因，不自动替换。Backup v20 兼容不因 UI 改造改变。

本轮 native 普通保存/reload：MiniMax H3 高质量首尾帧视频，WFV `wfv_ed7bd1b1-52ab-41b8-93de-f6138c1122bd` + Recipe `rcp_18d5fe30-a428-4be8-a62a-f19c7812322b`，VIDEO/DEFAULT revision=1。显示完整 UUID，选择器不显示友好版本号，属于 DO_NOT_PRESERVE。

既有 repository OCC 单项测试通过；它不是 service/native 双窗口 UAT。Native conflict rendering、本地草稿保留和本轮旧版本刷新/重启/显式升级仍 **NOT VERIFIED**。

## 6. Failure/recovery contract

输入错误：返回 Create → 修正字段 → 新运行。Transient 执行错误：仅在正式可恢复判定及 frozen input 支持时 retry/requeue；submission outcome uncertain 不得盲目重复提交。

恢复必须保留旧 Task/错误详情/原 exact pair 与 lineage，成功 leaf 不重做。用户能看到失败事实、原因与有效操作；不能仅导航到 Production 才刷新失败。

临时夹具初版错误返回不同 prompt_id，真实 backend 拒绝为 SUBMISSION_STATE_UNCERTAIN；修正夹具后新提交成功。该 incident **不是 input-error recovery 或 transient retry 的替代证据**。未改业务代码，也未伪造状态。

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
|L-FAIL-PROJECTION|Create 队列统计 0 failed，Production 显示 failed；新夹具失败及旧失败混合，不据此估算单一失败延迟|错误可见性依赖入口；不能当五 surface 单事实闭环|同一 Task 五 surface project-scoped projection；完整延迟测量待补|
|L-IDENTITY|项目 picker 完整 WFV/Recipe UUID；Shot recipe UUID、内部 ref2va；Task 包/目标/ID|普通用户理解负担；底层 identity 正确保留|普通友好标签，Advanced 显示诊断；exact-pair parity|
|L-RESULT-SCOPE|新 Shot 在未生成前已有项目历史候选|可能误认本 Shot 已有结果；Asset 与 selected Shot 分离|明确本次输出/历史复用；选用不造 provenance|
|L-NULL-TIME|Phase1 已观察 nullms；本轮不重复复现|缺失时长被当有效数字|缺值显示未知；零值与缺值分开|
|L-MULTI-ENTRY|Library→Task 可达，Create 候选/选择又是另入口|找结果需猜入口|Runs 统一状态/来源导航 + Library 媒体检索，不删除任一事实|

以上是排除的 UX 缺陷，不是本轮修复记录。不得将规范性要求标为已经实机通过。

## Evidence closure gate

|Journey|Current evidence|Gate|
|---|---|---|
|B|正常 fixture Task 成功、Asset/candidate、历史图片选用；完整 queued/running、Review 与全 surface 同时性未闭环|PARTIAL|
|C|历史真实 H3 quality 首尾帧2.1.1一秒视频在 Shot/Library 展示、播放、合法关联；当前 Shot form 是 ref2va，不宣称为该历史任务输入|PARTIAL|
|D|失败拒绝/新成功任务可观察；输入修正、正式 transient retry、三项 partial resume、五 surface propagation 未闭环|NOT VERIFIED|
|E|native exact pair 保存/reload + repository OCC test；native conflict/localdraft/old-binding cycle 未闭环|PARTIAL|
|F|Shot 候选、Library 预览和来源 Task 跳转；Shot History/open/select、输入 reuse 完整路径未闭环|PARTIAL|

PRODUCT_CONTRACT_STRUCTURE_COMPLETE=YES；PRODUCT_CONTRACT_COMPLETE=NO（未验证 oracle 仍在）；PHASE0_EVIDENCE_COMPLETE=NO；READY_FOR_PHASE_2=NO。
