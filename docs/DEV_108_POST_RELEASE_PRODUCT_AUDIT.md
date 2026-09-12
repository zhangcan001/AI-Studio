# DEV-108 — AI Studio 1.2 Post-Release Product Audit

```text
TASK=DEV-108
MODE=PRODUCT_AUDIT
CURRENT_RELEASE=1.2.0
TAG=v1.2.0
FEATURE_DEVELOPMENT=NO
LARGE_REFACTOR=NO
NEW_MIGRATION=NO
BACKUP_VERSION_CHANGE=NO
P0=NONE
P1=NONE
P1_RELEASE_BLOCKER=NONE
AUDIT_RESULT=COMPLETE
```

## Audit method and evidence boundary

本审计基于当前 master 的真实 UI 路由、组件行为、Rust read model、已有 UAT/回归测试和 DEV-099～DEV-107 发布证据完成。审计按普通生产者视角走完整链路，而不是只按数据库关系判断“存在即合格”。

本机 ComfyUI `127.0.0.1:8188` 不可用，因此 Task 执行和真实生成输出不宣称为 live smoke；生产前后的 UI、状态、导航、错误与安全边界按现有 mock/synthetic UAT 和代码调用边界审计。已有 DEV-106/107 证据确认：migration 032、Backup V18、Production Queue 单一显式 Start authority、Handoff/Preparation/Rework 不自动执行。

主要证据入口：

- `src/app/App.tsx`、`src/app/studioNavigation.ts`、`src/app/StudioShell.tsx`
- `src/features/projects/ProjectCommandCenter.tsx`
- `src/features/projects/ProjectImportDryRunWorkspace.tsx`、`src/features/projects/ExternalAgentHandoffPanel.tsx`
- `src/features/shots/ShotWorkspace.tsx`、`src/features/shots/ShotListToolbar.tsx`
- `src/features/shots/ProjectProductionPipeline.tsx`、`src/features/shots/ShotBulkConfigPanel.tsx`
- `src/features/production/ProductionReviewInbox.tsx`、`src/features/studio/ProductionQueuePanel.tsx`、`src/features/production/ProductionMonitor.tsx`
- `src/features/assets/AssetLibrary.tsx`、`src/features/assets/AssetPreview.tsx`、`src/features/assets/AssetUsagePanel.tsx`
- `src/features/tasks/TaskHistory.tsx`、`src/features/tasks/TaskHistoryDetail.tsx`
- `src-tauri/src/application/project_command_center_service.rs`、`src-tauri/src/infrastructure/database/repositories/project_command_center.rs`
- 目标相关测试：Project Command Center、Handoff、Import、Shot production、Queue、Review、Asset usage、Task detail

## End-to-end journey audit

| Step | User goal | Current route | Friction | Severity | Recommendation |
| --- | --- | --- | --- | --- | --- |
| 1. 打开 AI Studio | 进入可工作的生产环境 | 启动检查 → 项目中心 | 启动页能说明 ComfyUI 离线仍可进入，但没有“我现在可以做什么”的生产导向 | P2 | 保持离线可进入；增加一条面向新用户的首个动作说明，不改变启动边界 |
| 2. 进入/创建 Project | 找到已有项目或创建新项目 | 顶栏项目选择器 / 项目 rail / 项目工作区 | 选择器只呈现名称；项目列表承担创建、备份、恢复等管理动作，生产状态不在列表首屏 | P2 | 保留项目选择器；在项目入口增加最近状态/最近动作摘要 |
| 3. 查看 Project Command Center | 知道项目现状和下一步 | 项目 rail → 项目总览 | 总体进度、待审核、问题、运行环境和推荐下一步清楚；但 daily board 每桶只显示前 20 项，500 Shot 时不能直接定位全部异常 | P2 | 复用现有派生事实，提供“查看全部/带筛选打开镜头列表”的定位动作 |
| 4. 找到外部 Handoff Import | 把外部生产资料交给 AI Studio | 项目总览“批量导入预检” → “导入外部生产数据” | 入口是两级命名，用户先看到的是通用批量导入预检，不一定知道它支持外部 Agent | P2 | 在项目入口并列说明“外部 Agent 交接”；不新增独立 domain |
| 5. 导入 Handoff JSON | 载入外部结构 | Handoff 工作区：文件选择或粘贴 | 文件/粘贴、示例、schemaVersion 1 和当前项目 ID 说明完整；技术词较多但目标用户是外部 Agent 生产流程 | P2 | 首屏先显示业务说明，schema 字段放进可展开技术说明 |
| 6. 预检并理解错误 | 修复输入后再提交 | “运行交接预检” → Preview | 错误 code、field path、message、写入计数清楚；长 path 在 500-shot/复杂引用时仍需要用户自行回到 JSON 编辑器 | P2 | 继续保留 path；未来增加“复制错误路径/按路径定位” |
| 7. 确认并创建正式结构 | 明白确认会写什么 | Preview → “明确确认并写入” → “打开项目结构” | confirm 明确写入 Series/Episode/Scene/Shot 且不会创建 Task；成功后用户仍需手动点击打开结构 | P2 | 成功态保留两个动作：继续查看结构、返回项目中心；不要自动执行 |
| 8. 确认 Workflow / Recipe / References | 确认生产配置没有被猜测替换 | 结构树 → Shot workspace → Inspector | 精确 `workflowVersionId + recipeId`、参考顺序和缺失状态有显示；普通用户容易把版本/配方 ID 当成必须理解的技术字段 | P2 | 显示人类名称和版本为主，精确 ID 收进详情；继续禁止按名称猜测 |
| 9. 创建正式 production hierarchy | 建立可生产组织 | 结构管理 / Series / Episode / Scene / Shot | 结构层级真实存在，支持选择节点；结构管理通过“更多结构操作”进入，入口较隐蔽 | P2 | 对空项目直接提供“创建系列/集/场景”的首屏 CTA |
| 10. 打开 Shot | 处理一个镜头 | 创作 rail → Shot workspace | Shot 已是 production workspace：身份、Prompt、参考、配置、候选、进度和结果都可见；但左右结构/主区/Inspector/生产 tab 信息密度高 | P2 | 默认突出当前阶段和下一步，技术参数保持可折叠 |
| 11. 查看 Asset / references | 知道参考来源和使用关系 | Shot Inspector → Asset Library / Usage | Asset → Shot 反向使用、ReferenceSet/Profile、历史生产关系和精确打开 Shot 已具备；usage bucket 单桶最多显示 10 项 | P2 | 在同一 authority 下提供分页或“查看全部关系” |
| 12. 批量准备生产 | 选择可生产镜头并生成 READY batch | Shot workspace → 项目生产管线 / 场景/集/系列规划 | READY、阻塞、100-shot 限制、partial mode 和“不会启动 GPU”表达充分；不同入口使用“准备/加入生产队列/启动”多套词汇 | P2 | 建立统一状态词汇表；成功后统一显示“已创建待启动批次 → 打开队列” |
| 13. 进入 Production Queue | 找到待启动批次 | 生产 rail → Shot workspace production mode → Queue drawer / Runbook | Queue 是显式 authority，状态完整；但主页面先显示 Runbook/生产包/项目生产，Queue drawer 默认是次级区域，普通用户可能不把“准备完成”与“要启动队列”连接起来 | P2 | 让待启动批次在生产模式首屏有更强的 Start/下一步提示，不改变 Start authority |
| 14. 显式 Start | 确认真实生产即将发生 | Queue drawer/Runbook 的“开始/启动” | 只有这里会进入 Task/Comfy；启动前 Comfy connection guard 清楚；多批次连续运行、暂停和取消等待存在，但术语需要熟悉 | P2 | 在 Start 按钮旁固定显示“将创建/提交 Task，启动 GPU”；保持显式点击 |
| 15. Task 执行 | 看见运行、失败、恢复 | Queue item / Production Monitor / Task History | Running/Completed/Failed/Cancelled/Skipped、重试、暂停、恢复和事件刷新完整；task-only daily card 的 section 路由需要补充回归，避免落到生产上下文而非 Task History | P2 | 修正/测试目标解析：Task target 必须打开 Task History，Batch/Shot target 才打开生产上下文 |
| 16. 产生输出 | 看见输出是否可用 | Production Monitor / Queue item / Task detail | 输出资产、播放、打开文件位置、导出成品清单在 Production Monitor 可用；Asset/Task 之间可导航 | P2 | 统一输出资产的“已生成/可用/记录不可用”表达，并把文件位置能力带到最终资产入口 |
| 17. Review Inbox | 集中处理待审结果 | 项目中心“待审核结果”或 Review rail → Shot review | 项目级 Inbox 有总数、未审、待返工、分页和 Shot/Task/Asset/最终结果入口；Inbox 本身是只读投影，真正状态操作在批次审片工作区，用户要二次进入 | P2 | 保留 read projection；增加“打开后默认聚焦审片项”的连续性，不复制 review authority |
| 18. 选择结果 | 选择最终图片/视频 | Shot review / Batch review / Shot workspace | 明确按钮“设为关键帧/设为最终视频”，不会自动选第一张；A/B 比较和状态过滤可用 | P2 | 选中后展示统一业务结果摘要：Shot、阶段、采用资产、后续动作 |
| 19. Rework | 说明原因并重新准备 | Review → “编辑并重生成” → READY rework batch | 返工确认明确不会自动开始，创建后有“打开生产队列”；原任务/审核历史保留；用户仍需记住回 Queue Start | P2 | 将“返工已准备，下一步启动队列”做成统一成功 CTA |
| 20. 再次进入 Queue | 执行返工而不是误执行旧项 | READY rework batch → Queue Start | lineage/原失败记录/新 batch 保留，安全边界正确；重返入口依赖 notice 或 Queue 列表 | P2 | 在 Review、Shot、Project Center 之间保留精确 batch focus |
| 21. 找到最终选定结果 | 确认哪个结果被采用 | Project Center Complete → exact Asset；Shot selected output → Asset Library | 能到 exact Asset，且 Production Monitor 可打开成品文件夹/导出清单；但 Asset Preview 本身没有始终可见的“最终选定”标记或“打开文件位置”动作 | P2 | 建立现有 Asset/Shot facts 上的统一最终结果入口；不新增 deliverable 表 |
| 22. 找到文件/成品 | 在 10 秒内使用或交付 | Production Monitor → 打开成品文件夹；Asset Library → Asset Preview | 从批次监控可用；从项目中心/Asset 入口不总能直接打开文件位置，最终交付路径不一致 | P2 | 统一“查看成品/打开文件位置/导出清单”动作，优先复用现有文件系统能力 |

## Workspace scores

```text
HOME_SCORE=7.0/10
PROJECT_ENTRY_SCORE=7.2/10
COMMAND_CENTER_SCORE=7.6/10
HANDOFF_SCORE=8.0/10
SHOT_WORKSPACE_SCORE=7.2/10
ASSET_LIBRARY_SCORE=7.8/10
PREPARATION_SCORE=7.8/10
QUEUE_SCORE=7.7/10
REVIEW_SCORE=7.1/10
REWORK_SCORE=7.6/10
FINAL_RESULT_SCORE=6.4/10
OVERALL_PRODUCT_SCORE=7.3/10
```

### 1. App Entry / Home — 7.0/10

当前没有独立营销式 Home；启动后进入项目上下文，项目中心是实际产品首页。空项目可以进入“开始创作”，ComfyUI 离线状态不阻止查看和整理项目。弱点是新用户得到的是工作区导航而不是一条极短的生产向导，项目列表与项目中心之间也没有统一的“最近生产动作”概览。

### 2. Project List / Project Entry — 7.2/10

项目创建、打开、编辑、模板、备份导出、恢复和删除边界清楚，项目选择器可跨项目切换。项目列表适合项目数量有限的本地工具；对于很多项目，缺少搜索、筛选和状态摘要，但这不是当前 500-shot 生产链路的 blocker。

### 3. Project Command Center — 7.6/10

这是当前最接近普通生产者首页的部分：总体进度、待审核、需要处理、运行环境、每日五桶、最近活动和推荐下一步都由现有事实派生。问题是 500-shot 下每桶只显示 20 项，不能直接“查看全部异常”，而且部分 target label 仍暴露 ID。

### 4. External Agent Handoff — 8.0/10

Handoff 入口、文件/粘贴、schema 说明、示例复制、只读预检、错误 path、SHA、写入计划、幂等状态、显式确认、历史和映射均具备。主要成本是它被藏在通用批量导入入口后，且 JSON 技术语言偏多；这属于 P2，不是导入阻塞。

### 5. Shot Workspace — 7.2/10

Shot 已经不是单纯技术记录：有结构选择、搜索/筛选/分页、Prompt、Reference、Workflow/Recipe、阶段状态、候选结果、Production progress、Review 和精确 Asset/Task 导航。问题是同一页面承载创作、生产、审核、结构管理和 queue drawer，普通用户需要理解多个上下文切换。

### 6. Asset Library — 7.8/10

Asset Library 已有搜索、来源、标签、收藏、类型、排序、分类、分页、批量整理、A/B 对比和 Usage。Asset usage 能回到 Shot/Task，项目隔离和 exact ID navigation 正确。缺口是最终选定状态在 Asset 页面不够突出，且单个 usage bucket 只显示前 10 条关系。

### 7. Bulk Production Preparation — 7.8/10

准备、加入、启动严格分离；preflight、READY、阻塞、partial mode、100-shot admission、500-shot project plan 和“不会启动 GPU”均可见。安全性强于易学性：多个层级入口和不同按钮文案会让用户重复确认“现在只是准备还是已经入队”。

### 8. Production Queue — 7.7/10

Queue 是可靠的执行控制面：READY/RUNNING/PAUSED/COMPLETED、失败、取消、跳过、重排、重试、恢复、事件刷新和 exact Task navigation 都存在。主要问题不是执行语义，而是可发现性和布局：Queue 在 Shot production mode 中以 drawer/Runbook 组合出现，常规用户可能先看到大量计划信息再找到真正的 Start。

### 9. Review Inbox — 7.1/10

项目中心 Inbox 有计数、分页、刷新和 Shot/Task/Asset/最终结果入口；Batch review 有候选预览、A/B、通过、标星、待重生成、废弃和备注。它更像一个安全的 decision workspace，但项目 Inbox 是只读 projection，状态操作分散到 Shot review/Batch review，review throughput 仍有额外跳转。

### 10. Rework — 7.6/10

返工原因、提示词覆盖、时长、分辨率、Seed 和确认操作清楚；后端创建 READY rework batch，绝不自动提交。成功后的 queue CTA 已存在，但“返工结束”与“下一步回 Queue Start”的连续感仍可加强。

### 11. Final Result / Deliverable — 6.4/10

这是最低分区。Project Center 可以定位 exact selected Asset，Production Monitor 可以查看全部成品、打开成品文件夹和导出清单，因而不存在数据断裂；但普通用户从 Asset/Shot 进入后不一定能在同一处看到“这是最终选定结果”以及文件位置。最终生产事实存在，最终交付体验还没有成为一个统一的结果入口。

## Audit dimensions summary

| Workspace | Discoverability | Navigation | Information hierarchy | Action clarity | Safety | Large project |
| --- | --- | --- | --- | --- | --- | --- |
| Entry / Project | MINOR_FRICTION | CLEAR | MINOR_FRICTION | CLEAR | CLEAR | OCCASIONAL |
| Command Center | CLEAR | MINOR_FRICTION | MINOR_FRICTION | CLEAR | CLEAR | CONFUSING at 500 |
| Handoff | MINOR_FRICTION | CLEAR after import | MINOR_FRICTION | CLEAR | CLEAR | CLEAR within 500 limit |
| Shot | CLEAR for experienced user | MINOR_FRICTION | CONFUSING | MINOR_FRICTION | CLEAR | MINOR_FRICTION |
| Asset | CLEAR | CLEAR | MINOR_FRICTION | CLEAR | CLEAR | MINOR_FRICTION |
| Preparation | CLEAR | MINOR_FRICTION | MINOR_FRICTION | CLEAR | CLEAR | CLEAR within 100 admission |
| Queue | MINOR_FRICTION | MINOR_FRICTION | MINOR_FRICTION | CLEAR | CLEAR | MINOR_FRICTION |
| Review / Rework | MINOR_FRICTION | MINOR_FRICTION | MINOR_FRICTION | CLEAR | CLEAR | CONFUSING for throughput |
| Final result | MINOR_FRICTION | MINOR_FRICTION | CONFUSING | MINOR_FRICTION | CLEAR | MINOR_FRICTION |

## Safety and architecture verdict

```text
HANDOFF_AUTO_START=NO
PREPARATION_AUTO_START=NO
REWORK_AUTO_START=NO
QUEUE_EXPLICIT_START=YES
PROJECT_ISOLATION=PASS
WORKFLOW_RECIPE_EXACT_PAIR=PASS
ASSET_AUTHORITY=PASS
REVIEW_AUTHORITY=PASS
BACKUP_COMPATIBILITY=PASS
SECOND_QUEUE=NO
SECOND_EXECUTOR=NO
SECOND_TASK_MODEL=NO
NEW_MIGRATION=NO
```

没有发现数据损坏、项目串数据、静默替换 workflow/recipe、自动执行或 authority 分裂问题。当前主要问题是体验连续性和信息表达，不应通过新 domain model 解决。

## Product maturity answer

```text
PRODUCT_MATURITY=USABLE_PRODUCTION_APP
```

AI Studio 1.2 已经不是纯开发者工具：它有项目上下文、生产结构、外部交接、资产连续性、批量准备、持久化队列、显式执行、审核、返工、备份和交付文件夹。它还不是 `MATURE_PRODUCTION_APP`，因为普通用户仍需在多个 mode、drawer、技术 ID 和入口之间完成推理，最终选定结果与文件位置也没有统一的最后一公里入口。

### 最强的三个地方

1. **安全的生产边界**：准备、返工、重试和 Handoff 都不会偷偷启动；只有现有 Queue Start 执行真实生产。
2. **项目级连续性**：Command Center、Shot、Asset、Task、Review 已通过 exact IDs 互相连接，且状态主要由现有事实派生。
3. **外部输入和规模约束**：Handoff 有 schema/path/preview/confirm/历史，500-shot 计划与 100-shot admission 防止一次操作失控。

### 最弱的三个地方

1. **最终交付可见性**：选定 Asset、最终状态和文件位置分布在 Shot、Project Center、Production Monitor、Asset Library 多处。
2. **生产入口的层级感**：Queue 是核心执行面，但在生产模式中与 Runbook、生产包和项目管线并列，Start 需要用户发现。
3. **大项目异常定位**：后台聚合可处理 500 Shot，但 Command Center 的桶只显示前 20 项，缺少从异常摘要直接打开完整过滤结果的动作。

### 最影响普通用户效率的三个问题

1. 从项目问题/每日生产卡片跳到正确的 Shot、Batch 或 Task 还不够稳定、统一。
2. 准备完成后，用户必须理解“READY ≠ Running”并再次找 Queue Start。
3. 完成后要确认“哪个结果被采用、文件在哪里”，不同入口的答案不一致。

## Audit outcome

```text
P0=NONE
P0=NONE
P1=NONE
P1_RELEASE_BLOCKER=NONE
P2=8 product frictions documented
P3=2 polish/debt items documented
CI_ACCEPTABLE=YES
REAL_COMFY_RUNTIME_AVAILABLE=NO
REAL_COMFY_SMOKE=NOT_RUN_ENVIRONMENT_UNAVAILABLE
AI_STUDIO_1_2_CURRENT_RELEASE=YES
AI_STUDIO_1_3_PLANNED=YES
AI_STUDIO_1_3_STARTED=NO
DEV_109_STARTED=NO
AUTO_NEXT_TASK=NO
```

旅程表按用户步骤标记局部摩擦；同一根因在多个步骤重复出现，因此最终 inventory 去重为 8 个 P2 根因和 2 个 P3 验证/规模项。P2 问题会进入优先级矩阵；本任务不修复业务功能、不改 schema、不改 Backup、不改 Queue/Review/Handoff authority。

## Focused verification evidence

DEV-108 期间只运行与审计结论直接相关的最小 UI regression 集合，没有借机补大批测试或改业务代码：

~~~text
FOCUSED_UI_TEST_FILES=13
FOCUSED_UI_TESTS_PASSED=88
FOCUSED_UI_TESTS_FAILED=0
FOCUSED_UI_TEST_COMMAND=pnpm test --run src/app/studioNavigation.test.ts src/app/StudioShell.test.tsx src/features/projects/ProjectCommandCenter.test.tsx src/features/projects/ExternalAgentHandoffPanel.test.tsx src/features/projects/ProjectImportDryRunWorkspace.test.tsx src/features/shots/ShotWorkspace.test.ts src/features/shots/ShotWorkspace.production.test.tsx src/features/shots/ShotListToolbar.test.tsx src/features/assets/AssetLibrary.test.tsx src/features/assets/AssetUsagePanel.test.tsx src/features/production/ProductionQueueDrawer.test.tsx src/features/production/ProductionReviewInbox.test.tsx src/features/studio/ProductionQueuePanel.test.tsx src/features/studio/ProductionBatchReviewWorkspace.test.tsx src/features/tasks/TaskHistoryDetail.test.tsx
~~~

Vitest 输出包含一个既有的 jsdom 提示：`Not implemented: navigation to another Document`；对应测试仍然通过，没有形成 DEV-108 blocker。DEV-107 已提供完整 1.2 本地 gate、Rust 测试、migration/backup/installer 和 exact-head Source-only CI 证据，本次只引用这些已发布证据，不重复触发 release gate。
