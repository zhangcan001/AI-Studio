# DEV-108 — Product Friction Inventory

~~~text
TASK=DEV-108
MODE=PRODUCT_AUDIT
CURRENT_RELEASE=1.2.0
P0=NONE
P1=NONE
P1_RELEASE_BLOCKER=NONE
P2=8
P3=2
INVENTORY_RESULT=COMPLETE
~~~

## 目的与边界

本清单只记录从真实仓库审计中确认、值得进入后续产品决策的摩擦点。它不等同于所有页面缺口，也不把每一个可改进的文案或边界情况都升级为 1.3 工作项。

- P0：数据损坏、项目隔离破坏、完全无法生产；当前无。
- P1：核心生产流程严重受阻或存在静默危险行为；当前无。
- P2：会反复降低生产效率、可发现性或连续性，但有可用 workaround；当前 8 项。
- P3：规模化、可访问性、验证或 polish 缺口；当前 2 项。
- REQUIRES_SCHEMA_CHANGE=NO 和 REQUIRES_NEW_DOMAIN_MODEL=NO 是本清单的默认结论；优先复用现有 read model、导航状态和 authority。

## Prioritized inventory

### PX-108-001

~~~text
ID=PX-108-001
AREA=HOME / TOPBAR
TYPE=DISCOVERABILITY
SEVERITY=P2
USER_SYMPTOM=用户点击“搜索镜头 / 场景”后只是进入创作区，搜索输入框没有被聚焦，也没有明确显示搜索已启动。
ROOT_CAUSE=App.tsx 的 onSearch 只调用 creation 导航，未把搜索意图作为 focus 状态传入 Shot workspace。
CURRENT_WORKAROUND=进入创作区后手动找到 Shot 搜索框并输入。
RECOMMENDATION=让同一个入口同时切换到创作区并聚焦现有搜索控件；复用现有搜索，不增加全局搜索系统。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=SMALL
IMPACT=中等；第一次定位镜头时会产生额外认知和点击成本。
FREQUENCY=中高；入口位于全局 topbar，任何需要快速找 Shot 的用户都可能遇到。
COST=低；主要是导航 intent 和 focus 的小范围补齐。
RISK=低；不触碰生产 authority 或持久化。
~~~

### PX-108-002

~~~text
ID=PX-108-002
AREA=COMMAND_CENTER
TYPE=LARGE_PROJECT_USABILITY
SEVERITY=P2
USER_SYMPTOM=500-shot 项目在每日生产卡片中只能看到每桶前 20 项，用户看不到完整异常集合，也没有直接的“查看全部并带过滤打开”动作。
ROOT_CAUSE=project_command_center_service.rs 使用 DAILY_PRODUCTION_ITEM_LIMIT=20 对派生 bucket 做 bounded list，前端只展示该投影。
CURRENT_WORKAROUND=根据计数再去 Shot、Task 或 Review 页面手动搜索和筛选。
RECOMMENDATION=在不改变派生事实的前提下，增加“查看全部/打开带筛选列表”的目标动作，并保留首屏 20 项预览。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=MEDIUM
IMPACT=高；大项目异常处理会从摘要退化为人工找记录。
FREQUENCY=中；小项目不明显，500-shot 或批量生产时稳定出现。
COST=中；需要对现有 navigation target 和列表筛选参数做一致性设计。
RISK=低到中；主要风险是目标过滤不一致，不是数据写入风险。
~~~

### PX-108-003

~~~text
ID=PX-108-003
AREA=COMMAND_CENTER / NAVIGATION
TYPE=NAVIGATION
SEVERITY=P2
USER_SYMPTOM=从每日生产卡片点击 task-only 项目时，目标可能进入 Shot production mode，而不是直接打开 Task History 的任务详情。
ROOT_CAUSE=ProjectCommandCenter 的 dailyProductionNavigation 同时给出 destination=tasks 和 section=production；App.tsx 优先使用 section 映射到 ShotWorkspace，而 ShotWorkspace 没有 focusTaskId。
CURRENT_WORKAROUND=从生产监控或 Task History 的任务列表重新按关键词/ID查找。
RECOMMENDATION=把 task target 解析为 Task History 并传入精确 task focus；Batch/Shot target 才进入生产上下文。补充 route regression。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=SMALL
IMPACT=中等；用户会怀疑点击没有生效，尤其是在异常排查时。
FREQUENCY=中；只在 task-only daily target 出现。
COST=低到中；需要统一 navigation request precedence 和 focus contract。
RISK=低；有替代入口，且不会执行错误任务。
~~~

### PX-108-004

~~~text
ID=PX-108-004
AREA=PRODUCTION_QUEUE
TYPE=DISCOVERABILITY / ACTION_CLARITY
SEVERITY=P2
USER_SYMPTOM=用户完成准备后首先看到生产包、Runbook 或其他 production mode 内容，Queue drawer 和显式 Start 不是最强的首屏下一步。
ROOT_CAUSE=Queue 作为唯一执行 authority 被放在 ShotWorkspace production mode 的次级 drawer/组合视图中，而准备成功与 Queue Start 的连续提示不够统一。
CURRENT_WORKAROUND=根据 notice、生产 rail 或手动展开 drawer 找到 READY 批次并点击 Start。
RECOMMENDATION=在现有 production mode 首屏强化“待启动批次 → 打开队列 → 显式 Start”路径；不复制 Queue，也不改变 Start 的唯一 authority。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=MEDIUM
IMPACT=高；这是准备到真实生产之间最容易卡住的步骤。
FREQUENCY=高；每次新批次或返工批次准备后都可能发生。
COST=中；涉及几个现有 workspace 的 CTA、notice 和 focus 组合。
RISK=低；强化提示不能自动启动，反而能保持安全边界。
~~~

### PX-108-005

~~~text
ID=PX-108-005
AREA=PREPARATION / QUEUE
TYPE=ACTION_CLARITY
SEVERITY=P2
USER_SYMPTOM=“准备图片生产”“准备场景生产”“加入生产”“已加入生产队列”“待启动”并存，用户需要自行判断当前是否已经创建 batch、是否会消耗 GPU。
ROOT_CAUSE=多个历史入口分别形成文案，prepare/create/start 的语义虽在实现上分离，但没有统一到一套用户语言。
CURRENT_WORKAROUND=阅读 READY、running 状态和说明文字，确认后再去 Queue。
RECOMMENDATION=建立可见的状态词汇表：准备=创建 READY batch，启动=Queue 的显式 GPU action，运行=Task 已提交；所有成功态统一给“打开队列”。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=SMALL
IMPACT=中高；影响新用户对安全边界和下一步的理解。
FREQUENCY=高；批量准备、单批准备和返工均会经过这些文案。
COST=低；主要为 copy、状态标签和 success CTA 统一。
RISK=低；不改状态机，仅改善表达。
~~~

### PX-108-006

~~~text
ID=PX-108-006
AREA=ASSET / FINAL_RESULT
TYPE=INFORMATION_HIERARCHY
SEVERITY=P2
USER_SYMPTOM=从 Asset Preview 进入时，用户不一定能立即看出该资产是否是 Shot 的最终选定结果，也不一定能从这里打开文件位置。
ROOT_CAUSE=最终选定标记和打开成品位置能力分布在 Shot、Project Center、Production Monitor 等不同 surface，AssetPreview 没有统一呈现。
CURRENT_WORKAROUND=先回 Shot 或 Project Center 判断 selected result，再从 Production Monitor 打开成品文件夹。
RECOMMENDATION=在现有 Asset/Shot/production facts 上提供统一 business-language summary 和现有文件位置动作；不新增 deliverable 表。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=MEDIUM
IMPACT=高；直接影响最终交付的最后一步。
FREQUENCY=中高；每次需要确认和交付成品时出现。
COST=中；需要统一 selected-result 和 output-location 的现有读取路径。
RISK=低到中；需避免把“有 output”误报为“已选最终结果”。
~~~

### PX-108-007

~~~text
ID=PX-108-007
AREA=ASSET_USAGE
TYPE=LARGE_PROJECT_USABILITY
SEVERITY=P2
USER_SYMPTOM=Asset Usage 每个关系 bucket 最多展示 10 项，用户面对一个被大量 Shot 复用的资产时无法在原地看全使用范围。
ROOT_CAUSE=AssetUsagePanel 为保持可读性对每个 usage bucket 做了前 10 项 bounded display，只显示“另有 N 项关系”。
CURRENT_WORKAROUND=按项目、Shot 或关系类型手动搜索，逐个核对。
RECOMMENDATION=保留摘要前 10 项，同时提供带关系类型、项目隔离和分页的“查看全部关系”。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=MEDIUM
IMPACT=中等；会增加参考连续性和影响评估成本。
FREQUENCY=中；共享角色/参考资产和大项目时明显。
COST=中；需复用现有 usage 查询和分页，不宜复制关联 authority。
RISK=低；只读展示。
~~~

### PX-108-008

~~~text
ID=PX-108-008
AREA=REVIEW_INBOX
TYPE=NAVIGATION / DECISION_CONTINUITY
SEVERITY=P2
USER_SYMPTOM=Review Inbox 能告诉用户有多少待审，但本身是只读 projection；用户需要再进入 Shot review 或 Batch review 才能选择、通过或发起返工。
ROOT_CAUSE=Inbox 被设计为 project-level read model，Review mutation authority 保留在既有 review workspace，入口之间的 focus 连续性不足。
CURRENT_WORKAROUND=点击 Shot/Batch/Task 入口，再手动确认当前候选和 review item。
RECOMMENDATION=保留单一 review authority，为 Inbox 的打开动作增加默认 focus 到具体审片项、候选和 batch，不在 Inbox 复制 mutation。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=MEDIUM
IMPACT=中高；审核量越大，重复跳转越明显。
FREQUENCY=中高；每个需要审核或返工的输出都会经过。
COST=中；需要稳定的 read-projection target contract。
RISK=低；只优化导航，不改变审核规则。
~~~

### PX-108-009

~~~text
ID=PX-108-009
AREA=PROJECT_ENTRY
TYPE=LARGE_PROJECT_USABILITY
SEVERITY=P3
USER_SYMPTOM=项目数量较多时，项目列表缺少统一的搜索、状态筛选和最近生产动作，用户需要滚动或记忆项目名称。
ROOT_CAUSE=项目入口适合少量本地项目，当前首屏主要是名称和管理动作，没有面向多项目运营的 locate surface。
CURRENT_WORKAROUND=使用项目名称、顶栏选择器或系统文件路径逐个查找。
RECOMMENDATION=未来以项目级搜索/最近活动为独立小切片评估；不把它混入 1.3 第一主题。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=MEDIUM
IMPACT=中等；影响多项目用户而非单项目生产链路。
FREQUENCY=低到中；取决于本地项目数量。
COST=中；需要定义项目级状态摘要和排序。
RISK=低。
~~~

### PX-108-010

~~~text
ID=PX-108-010
AREA=TEST / PERFORMANCE
TYPE=VALIDATION_GAP
SEVERITY=P3
USER_SYMPTOM=当前有 500-shot admission/backend 证据，但没有真实桌面 UI 的 500-shot 渲染、键盘操作、滚动和时间预算 profile，用户规模体验只能从静态代码推断。
ROOT_CAUSE=DEV-108 机器无 ComfyUI，现有测试主要是 focused UI/service tests，没有完整 live desktop performance harness。
CURRENT_WORKAROUND=依赖现有合成 500-shot 规则测试、列表分页实现和人工观察。
RECOMMENDATION=后续建立可重复的桌面性能/可访问性 profile，仅在真实规模证据显示问题时再优化；不以此为当前 release blocker。
REQUIRES_SCHEMA_CHANGE=NO
REQUIRES_NEW_DOMAIN_MODEL=NO
ESTIMATED_SCOPE=MEDIUM
IMPACT=中等；影响规模信心，不阻断现有生产。
FREQUENCY=低；只有大项目和设备性能较弱时明显。
COST=中到高；需要稳定 fixture、采样指标和桌面运行环境。
RISK=低。
~~~

## Priority triage

| Priority | Count | Meaning for 1.3 |
| --- | ---: | --- |
| P0 | 0 | 不存在立即修复项 |
| P1 | 0 | 不存在 release blocker |
| P2 | 8 | 进入主题评分和切片规划，按连续性优先 |
| P3 | 2 | 记录；不应挤占 1.3 第一阶段 |

### Selection dimensions

- **Impact**：是否让用户无法完成、误解或重复操作核心链路。
- **Frequency**：是否在每次生产/审核或常见入口触发。
- **Cost**：是否可在现有 authority 上小范围收敛。
- **Risk**：是否可能误导启动、跨项目或破坏历史数据。

P2 不是“必须马上开发”；它表示值得产品计划处理。当前没有理由越权修复 P0/P1，因为审计未发现 P0/P1。