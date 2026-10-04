# AI Studio 2.1 聚焦路线图

日期：2026-10-04。状态：PROPOSED / NOT IMPLEMENTED。
依据：[r4 产品审计](AI_STUDIO_POST_R4_PRODUCT_AUDIT.md)，源码基线 `8a503f6ab45bedb2b7f272d1f755abe915d7b70a`。
稳定发布仍是 `v2.0.0-personal-r4`；本轮不 bump version、不创建 tag/release。

## 1. Objective

让个人用户更容易从“软件已安装”走到“环境准备好”，完成图片/H3 创作、解释失败、找回与复用已有结果。复用已验证权威，不再做 Architecture Reset，不引入自动制作/自动决定系统。

```text
THEME_COUNT=4
ORDER=M1 → M2 → M3 → M4
FIRST_IMPLEMENTATION=Create readiness 的可操作错误去向
FIRST_IMPLEMENTATION_SIZE=1–3 logical commits
IMPLEMENTATION_STARTED=NO
```

四主题是范围上限，不是批准同时开发。M3 历史分页、M4 写入维护必须另过 contract Gate；发现成本超预算先缩小功能，不扩大架构。工程量是单开发者有效工作日的粗估，不含等待反馈/CI，不是交付承诺。

## 2. 候选价值评分

公式：`(USER_VALUE × FREQUENCY × PAIN × CONFIDENCE) / (EFFORT × RISK)`。
每项 1–5。全部是当前证据下的产品判断，Frequency 不是遥测。分数只辅助，数据安全/依赖/首用价值优先。

| 候选 | Value | Frequency | Pain | Confidence | Effort | Risk | Score | 决定 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 首个 readiness 动作补齐 | 5 | 4 | 5 | 5 | 2 | 2 | 125 | FIRST / M1 |
| 连贯 first-run / environment summary | 5 | 4 | 4 | 4 | 3 | 2 | 53.3 | M1 后续 |
| 日常恢复/复用解释 | 5 | 5 | 4 | 4 | 3 | 2 | 66.7 | M2 |
| Library 模块查找/视觉找回 | 5 | 5 | 4 | 4 | 3 | 2 | 66.7 | M3 |
| 混合 Runs 完整历史分页 | 4 | 4 | 4 | 4 | 4 | 3 | 21.3 | M3 contract 后，先现有完整历史入口 |
| 只读媒体检查 | 4 | 2 | 4 | 3 | 3 | 3 | 10.7 | M4 小范围 |
| 自动 safe tool discovery | 3 | 2 | 2 | 3 | 4 | 3 | 3 | DEFER；登记解释合入 M1 |
| 精确 Prompt usage 聚合 | 2 | 2 | 2 | 2 | 3 | 2 | 2.7 | DEFER；先复用体验 |
| 未测大库专项优化 | 3 | 2 | 2 | 2 | 4 | 3 | 2 | MEASURE_BEFORE_OPTIMIZE |

首项高分且有明确 S 根因，小范围立即可见收益，最适合 1–3 提交；M2/M3 日常价值高，但不能让未配置用户先面对更多选择器。

## 3. M1 — First-run / Runtime Readiness 与可操作阻断

**Problem / findings**：F01、F02、F08 说明部分、F09。四种状态（连接、包合法、依赖、输入）容易被混淆，Create 阻断按钮可能无去向。

**User value**：不用理解 Recipe/Node/内部 ID 就能知道“现在不能做什么、应该去哪里做什么、怎么再验证”。

**Scope**：
1. 首个 bounded checkpoint：修 Create 的 readiness action presentation。
2. Overview/Settings 的轻量准备摘要：连接 → 已有生成器 → 已知节点/模型依赖 → 本次输入；每个失败/未知有合法目的地。
3. 环境操作与诊断摘要区分，保留现有 endpoint 测试、保存、重连、refresh/preflight；说明测试草稿不等于已应用环境。
4. Tool Hub 空态解释元数据登记与现有 Comfy 连接、健康记录更新时间；不做发现引擎。

**Non-goals**：自动安装模型/节点、自动启动/停止 Comfy、后台扫描全部磁盘、保存第二份 readiness authority、自动运行任务、新向导框架、工作流引擎重写、统一 tool registry 重构。

**Architecture constraints**：现有 product facade 和 typed IPC；前端只展示后端事实、合法 route/field action。UNKNOWN 不能伪造 READY。endpoint/global 与 project generator binding 分开；精确 workflowVersionId+recipeId 不变；binding instance/revision/OCC 不变。导航必须遵守现有 dirty-draft/returnTo 行为。

**Acceptance**：
- MISSING_INPUT/OUT_OF_RANGE 有合法字段时聚焦该字段；运行环境类 issue 指向已有 System Settings，带 returnTo；生成器不可用指向合法选择/设置入口。
- 未知 action/未知 field 使用明确安全 fallback，不静默 no-op、不打开猜测节点、不泄露 raw payload。
- 输入/历史候选保留，切项目不展示其他项目状态；没有任何按钮间接启动 Queue。
- 连接成功、包存在、依赖失败分别显示，不用“5 个可用包”宣称当前可以生成。
- 在不生成的 isolated Native 中核对 offline、未检查、缺模型/节点已有契约或安全模拟、准备正常几种路径；不声称只读检查等于真实 GPU 生成。

**Effort / dependencies / risk**：首项 1–2 天、1–3 提交；整体 M1 3–5 天。依赖现有 errors/readiness/actions/routes，不需 migration。风险低–中：draft 返回和异步项目切换需回归。

**Checkpoints（仅规划）**：
- DISCOVERY：对照实际 issue codes、field/action 和合法目的地，确认 fallback；不再全库扫描。
- CONTRACT：确定显示动作映射及 returnTo，缺事实先 UNKNOWN，不添猜测状态。
- IMPLEMENTATION：一个展示/路由逻辑单元；需要时补已有 typed details，不创建 executor。
- TARGETED_TESTS：字段定位、runtime 跳转、未知 fallback、draft/project isolation、raw invoke/IPC guard（若触及 transport）。
- NATIVE_ACCEPTANCE：一次隔离 session，旧输入/候选保留，无提交/重试 side effect。
- CI：逻辑 checkpoint 按项目 policy 跑相关 local gates；M1 最终跨层/重要 checkpoint exact HEAD CI，不使用 r4 旧 CI 证明新代码。

## 4. M2 — Daily Create / Runs 的恢复与复用

**Problem / findings**：F03、F07 的覆盖与去向、F10–11。用户需要理解“重试原快照、新草稿、新运行、选用、审核”多个概念；失败原因到动作还不够直达。

**User value**：失败后不丢输入/结果，知道能否重试、该改哪里；提示词与素材能在正确项目/明确输入槽位复用。

**Scope**：
- 以 typed error / recoverability 为源的一句原因+主动作，技术信息折叠保留；识别不了的原因诚实显示未知。
- 失败/部分成功明确旧结果保留，重试旧快照与编辑后新生成的差别；权限/能力不满足解释原因。
- 提示词/素材近期 picker 说明覆盖范围、从现有 Library 找回入口；搜索细节在 M3。
- 只做影响理解的 polish：名称展示一致、按实际字段帮助、删除不适用于图片/H3 的音频输入暗示；不删除历史/imported audio 支持。

**Non-goals**：自动重试、自动审核/选用、修正旧 snapshot、按正文猜 Prompt identity、改 Queue/Task terminal status、恢复已退役快速预览/音频输入生产、独立 Prompt statistics。

**Architecture constraints**：availableActions/recoverability 继续决定能力；Queue Start 是唯一执行入口；retry 使用原显式 lineage，edit/new generation 走原提交路径。保留结果/镜头选用状态和项目隔离，Studio Store 仍唯一草稿 authority。

**Acceptance**：
- queued/running/paused/failed/partial/completed 的动作语义明确；PAUSED 不当 completed。
- 跨项目旧响应不覆盖新项目；draft 更改不被后台刷新丢失。
- 输入问题可去对应草稿；retry 不偷偷换输入；无可恢复失败项时不提供假 retry。
- 对几类固定原因做“原因/可重试/应改什么/去哪里/旧结果保留”十秒理解核对；这是未来小型实测，不是本轮已经达到。
- 第 21 条旧 Prompt / 第 101 项素材可经明确 Library identity 复用，不推断名称/时间/正文关系；多输入槽位由用户明确选择。

**Effort / dependencies / risk**：3–5 天，分恢复说明与 picker/文案两个 checkpoint；依赖 M1 action 契约。风险中：新运行与原重试混淆、Prompt version/context、既有选用/审核状态。

**Checkpoints**：DISCOVERY 复用现有失败 fixture/契约 → CONTRACT 恢复动作与快照区别 → IMPLEMENTATION 展示/controller adapter → TARGETED_TESTS 失败/部分成功/旧结果/明确复用/OCC → NATIVE_ACCEPTANCE 有历史结果的 owned fixture 一次会话（不需要新 GPU） → CI 相关本地 Gate 和最终 exact-head authority。

## 5. M3 — Search & Findability，先模块、后规模

**Problem / findings**：F04、F05、F07 查找部分、F13。普通 Library 与 Create 查找浅，Runs 完整旧历史在普通入口之外。

**User value**：快速找之前那张图、那条 Prompt、失败运行，不要求用户记自动文件名或知道高级工作区。

**Scope**：
1. 以已有 Library 类别/名称/分页为基础，提供已有收藏/标签能力的正常入口和有界可见缩略图；高级编辑仍保留。
2. Prompt 查找复用现有名称/标签/版本服务；Create picker 有明确近期/完整入口，不擅自认为项目近期任务等于选中 Prompt usage。
3. Runs 首先明确“近期 / 完整历史”的去向，利用现有高级历史入口；模块搜索/分页合同成熟后逐步上移，不一次造跨域全局搜索。
4. 单独测量个人版 1k/10k 库、1/5/20 页和快速输入；50k/100k 不纳入默认发布承诺。

**Non-goals**：全局 Search Platform、Elasticsearch、FTS 新平台、跨项目泄露、无界全量读取/缩略图、默认全文 Prompt 索引、性能 cache authority、因为理论循环就重构全部 SQL。

**Architecture constraints**：现有 SQLite repository、typed facade；scope/cursor 包含 project/category/query；稳定排序去重，不能 name/path 推断 relation。All 近期摘要不能冒充 complete。数据记录多不意味着必须加 schema；只有真实持久域需求才审批 migration。

**Acceptance**：
- 不止搜已加载页：媒体/Prompt 查询经过当前项目 repository；分类游标在 search/filter/project 改变时安全失效。
- 收藏/标签结果与已有 Advanced Assets 语义一致；完整使用位置和删除保护保持。
- 缩略图仅可见有界请求，缺图有占位，Blob URL 释放；列表 detail 不读取全部媒体 bytes。
- Runs 不丢父级归并/去重/归档事实，不把混合运行表分页当成简单单表 offset。
- Baseline 先记录 p50/p95、IPC/bytes、DOM、内存和首预览；建议回归阈值在 CONTRACT 基于测量确定，不能事后调整到“刚好 PASS”。
- 大集合优化只在具体 budget/guard 失败后改一个 seam，再测，保留 Phase12 boundary 与受审 successor。

**Effort / dependencies / risk**：5–8 天，不含未批准的完整混合 Runs 分页；后者额外 2–4 天且需 contract 评估，必要时留后续版本。依赖 M2 reuse entry、现有资产 tag/favorite ports。风险中：分页一致性、UI DOM 累积与新增预览资源、历史结果覆盖。

**Checkpoints**：DISCOVERY 小型 owned SQLite/library 真实分布基准 → CONTRACT 查询范围/稳定 cursor/预算 → IMPLEMENTATION Library filters/visible previews、Prompt finder、Runs 明确入口分别逻辑 checkpoint → TARGETED_TESTS 非首页匹配/过滤失效/项目隔离/缺预览/归并 → NATIVE_ACCEPTANCE 样本库查找复用与关系检查 → CI checkpoint 相关本地 Gate、完成后 exact-head CI。

## 6. M4 — Local Media Maintenance，只读先行

**Problem / findings**：F06。无法预览时用户不能区分储存缺失、不可读、hash 不符和 codec。

**User value**：保持对项目数据的信任；找到安全处理方法，不用删除记录试错。

**Scope**：
- 第一版仅用户显式触发、项目限定、可取消/有界分批的 scan/verify。
- 明确区分存在、可读、校验不符、预览解码、未检查；显示现有关系/保留结果说明。
- 可以提供定位受管理数据目录或重新导入说明，但不自动登记 replacement/path。
- relink 默认不在 M4 首版；如果用户确有需求，独立批准 contract：对明确 assetId + 用户选定文件验证内容、项目归属、版本/关系保持和事务性。

**Non-goals**：自动 orphan delete、全磁盘扫描、按文件名/path/time 猜 lineage/provenance、替换不相同 bytes 仍保留原 hash、自动覆盖引用、新 maintenance DB。

**Architecture constraints**：repository/application/file-store 已有边界，媒体验证只读；外部原始文件移动不应自动判定为应用导入副本损坏。校验大型视频必须流式，串行/低并发，不与生产 GPU/build 抢资源。诊断输出仍最小隐私，不导出私密路径/媒体。

**Acceptance**：
- Owned fixture 下分别测试缺文件、hash 不符、无读权限/不可读可移植模拟、codec/预览失败；不能把同类 UI 错误原因合并成文件丢失。
- scan 前后 DB/关系/选用/Task/媒体 bytes 不变；取消不留下业务状态改写。
- 拒绝跨项目/path traversal/超范围定位；历史 task 和正式关系保留。
- 不支持修复时也给明确下一步，不伪造“已修好”。

**Effort / dependencies / risk**：3–5 天，必须先确认本地受管理媒体读取契约；依赖 M3 Library 错误/操作入口。风险中–高（若写入则高），因此只读切片优先。若仍无用户真实需求且 discovery 显示代价过高，DEFER 主题实施，不用为了凑四主题开发大维护系统。

**Checkpoints**：DISCOVERY 文件储存/导入副本/预览错误分类 → CONTRACT 只读有界输出与取消 → IMPLEMENTATION existing service adapter → TARGETED_TESTS 文件安全/校验/无写入/项目隔离/大文件流式 → NATIVE_ACCEPTANCE copied fixture 中缺媒体、关系仍可查、没有自动恢复 → CI 本地相关完整数据/privacy Gate + exact-head CI。

## 7. 统一 regression oracle 与交付规则

所有未来改动继续保护：

- Production Queue Start 唯一执行 authority；现有 Task state machine。
- 精确 workflowVersionId + recipeId、binding_instance_id / revision / OCC；repair/backup 不绕过绑定写入契约。
- 项目隔离、Asset/provenance/显式关系、旧 snapshot 与结果。
- Backup v20 及有效 v1–v20 compatibility，不无必要 bump 格式。
- Phase12 performance、Phase13 diagnostics/privacy、已冻结 release successor 边界；hash 更新必须有 legal diff/untouched aggregate/chain proof。
- typed tauriClient/ipc、repository ports；不创建第二套 queue/store/router/DB/cache authority。

测试不设数量上限；真实 bug 配回归，targeted first；跨层风险/发布前按实际任务执行 full local/frontend/Rust/Tauri Gate。按项目 CI policy 调用远端，重要阶段以其 exact final code HEAD 结果为准；不要重复 unchanged HEAD suite。每次 compile/test 前检查 RAM/VRAM、已有 runner，安全释放仅本任务资源；不关用户应用/ComfyUI。

沿项目既定 master 模式，逻辑单元验证后 commit、fetch/divergence 后普通 push；不 force、不自动合并/重写历史。减少同一 checkpoint 的多次 CI churn，但不堆巨大未提交改动。Native 复用 owned fixture，每主题尽量一次 session，新 GPU 不作为一般 UX Gate。

本轮只需文档 consistency + diff --check；**r4 CI 证明 r4 源码，不证明文档提交后的新功能**。master push 可能被现有 workflow 自动触发 CI；本轮不手动 dispatch/rerun、不为了只改文档额外等待全量结果，也不改 CI 来规避触发。

## 8. 明确不做 / 延后

不做：cloud sync、多用户/SaaS、AI Agent、自动 AI 决策、Prompt 自动评分/优化、自动最佳 Prompt、第二 Queue/Task state machine、工作流引擎重写、新 DB 平台、remote telemetry、恢复快速预览和音频输入/单独音频/图音频生成。

延后：通用安全 tool discovery（仅明确目录/endpoint、用户确认登记、无启停）；Prompt 精确统计（先核对显式使用记录覆盖，recent project history 不做 usage proxy）；未证实的大库优化；写入 relink；纯审美重构。

有价值的 Prompt stats 若以后批准，优先 usage count/recent use，并标明 coverage；generator distribution/successful generations 只能由显式使用关系关联，不按文本猜测。统计结果不能变成业务 authority。

第一实施任务虽然已经选择，**仍需下一条用户授权才开始**。本轮结束于两份文档 commit/push，不自动开始 M1，不创建新 tag/release。
