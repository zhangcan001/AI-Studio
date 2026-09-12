# DEV-108 — AI Studio 1.3 Roadmap

~~~text
TASK=DEV-108
CURRENT_RELEASE=1.2.0
AUDIT_TASK=DEV-108
P0=NONE
P1=NONE
P1_RELEASE_BLOCKER=NONE
TOTAL_P2=8
TOTAL_P3=2
AI_STUDIO_1_3_RECOMMENDED_FIRST_THEME=Production Wayfinding & Action Continuity
DEV_109=planned, not started
DEV_110=planned, not started
DEV_111=planned, not started
DEV_112=planned, not started
AI_STUDIO_1_3_STARTED=NO
DEV_109_STARTED=NO
AUTO_NEXT_TASK=NO
~~~

## Roadmap stance

1.2.0 是当前已发布版本。本文件是基于 DEV-108 产品审计的规划，不是 1.3 开发启动令，也不包含自动执行 DEV-109。当前建议先收敛用户从“我在什么上下文”到“下一步应做什么”的连续性，再考虑交付面和更大规模的增强。

## Candidate themes

候选主题来自当前仓库和完整生产旅程，不是机械照抄示例：

- **A. Production Wayfinding & Action Continuity**：从 Command Center、Shot、Preparation、Queue、Review、Rework 到精确 Task/Asset 的目标、焦点和下一步提示。
- **B. Large Project Locate & Exception Triage**：从 500-shot daily bucket、Shot/Asset usage 和多项目入口中快速查看全部异常和关系。
- **C. Final Result / Deliverable Visibility**：让 selected result、最终状态、文件位置、成品清单在现有事实之上形成一致的最后一公里入口。
- **D. Workflow / Recipe Operational UX**：在保留 workflowVersionId + recipeId 精确身份的前提下，改善人类名称、版本、缺失和可解释性。
- **E. Review Throughput & Decision Workspace**：让 Review Inbox、Batch review、A/B 选择、备注和 Rework 之间的处理连续性更适合高审核量。

## Theme score matrix

评分采用 1–5 的方向性分值。Complexity 与 Risk 越低越好；为了让“低成本、低风险”在综合分中体现，计算为：

Score = User Impact + Frequency + (6 - Complexity) + (6 - Risk) + Architecture Fit

最大 25 分。分数用于比较，不代表虚假的精确估计。

| Theme | User Impact | Frequency | Complexity | Risk | Architecture Fit | Score |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A. Production Wayfinding & Action Continuity | 5 | 5 | 2 | 2 | 5 | **23/25** |
| B. Large Project Locate & Exception Triage | 5 | 4 | 3 | 2 | 5 | 21/25 |
| C. Final Result / Deliverable Visibility | 5 | 4 | 3 | 2 | 5 | 21/25 |
| D. Workflow / Recipe Operational UX | 4 | 4 | 2 | 2 | 5 | 21/25 |
| E. Review Throughput & Decision Workspace | 4 | 3 | 3 | 3 | 4 | 17/25 |

### Why the recommended theme wins

A 得分最高不是因为它要“做所有 UX”，而是因为它覆盖当前最常见的跨 workspace friction，同时可以拆成小的导航、CTA、copy 和 focus 合同变化。它不要求新的 domain model，也不改变 Queue、Review、Asset 或 Task 的 authority。

- B、C、D、E 都是有效方向，但先有统一 target/focus/next-action contract，后续各主题才不会分别发明入口。
- C 的最终交付可见性很重要，但在没有明确 selected-result/file-location continuity 时直接设计新的 deliverable surface 容易扩大范围。
- B 的 500-shot locate 价值高，但“查看全部”动作应建立在稳定目标导航上，避免打开错误上下文。
- D 的 Workflow/Recipe UX 需要保持 exact pair，先统一路由和 action feedback 可降低后续误解。
- E 会涉及更高审核 throughput 和更多状态交互，适合在 Inbox → focused review → rework continuation 基础稳固后处理。

## Recommended first theme

~~~text
AI_STUDIO_1_3_RECOMMENDED_FIRST_THEME=Production Wayfinding & Action Continuity
WHY_NOW=1.2 的核心 authority 已稳定，但准备完成、异常定位、审核和最终结果之间仍需要用户记住 mode、drawer、ID 和下一步；这是最常见且不需要 schema 的剩余摩擦。
USER_VALUE=让用户从摘要或当前结果在少量点击内到达正确的 Shot、Batch、Task 或 Asset，并明确知道下一步是否只是准备、是否会启动 GPU、以及如何继续。
WHY_NOT_OTHER_THEMES=B/C/D/E 都依赖更稳定的 target/focus/next-action 约定；先做它们会把相同的连续性问题分散到多个 workspace，各自形成不一致的局部方案。
EXPECTED_SCOPE=复用现有 App navigation、Studio Store、project command center read model、Shot/Asset/Queue/Review/Task surfaces，补齐精确 target、focus、CTA、状态 copy 和 focused regression；小步提交，可独立验证。
EXPECTED_NON_GOALS=不新增 schema、migration、backup version、queue/executor/task/retry/scheduler/domain model；不做 bulk review mutation、不做完整 deliverable model redesign、不做 live ComfyUI scale production。
~~~

## Planned delivery slices — planning only

### DEV-109 — Exact target navigation and focus

~~~text
Goal=让项目中心、生产监控、Review Inbox、Shot、Asset 之间的每个核心链接都打开正确 workspace，并聚焦 exact project/series/episode/scene/shot/batch/task/asset/review target。
Scope=统一现有 navigation request precedence 和 focus 参数；修复 task-only daily target；为 Queue、Review、Rework 成功 CTA 保留 exact batch/review focus；补充最小 route regression。
Non-goals=不引入新 router、不改持久化模型、不复制 Store authority、不添加第二个 queue 或 task model。
Acceptance=task target 打开 Task History；batch/shot target 打开对应生产上下文；asset/review target 保持项目隔离并聚焦目标；现有导航和 focused tests 全部通过。
~~~

### DEV-110 — 500-shot locate and exception triage

~~~text
Goal=让大项目用户从 Command Center 的摘要和异常计数进入完整、带过滤的 Shot/Task/Review/Asset 列表，而不是停留在前 20 项预览。
Scope=在现有 DAILY_PRODUCTION_ITEM_LIMIT 预览之上增加“查看全部/带筛选打开”目标；复用现有 Shot search/status/scene/pagination、Review Inbox load-more 和 Asset usage 查询。
Non-goals=不取消首屏 bounded preview；不新增统计 domain model；不做未经 profile 证明必要的虚拟化或全仓库性能重构；不改变 100-shot admission 规则。
Acceptance=500-shot synthetic fixture 中摘要仍快速返回，前 20 项保持可读，并可从每个 relevant bucket 打开完整过滤结果；项目隔离、计数和现有 target identity 不变。
~~~

### DEV-111 — Queue / Review / Rework continuation

~~~text
Goal=把准备、审片、返工成功态统一连接到现有 Queue，并让用户明确看到“待启动”而不是误以为已经运行。
Scope=在 preparation、Project Command Center、Review Inbox、Batch Review、Rework 和 Queue drawer 之间复用现有 READY batch/task/review facts，增加一致的 next-action CTA、notice 和 exact batch focus。
Non-goals=不自动 Start；不新增 executor、scheduler、retry engine 或 review mutation authority；不把 Review Inbox 变成第二个 Queue。
Acceptance=准备或返工后可直接打开对应 READY batch；Review 后可回到同一批次并明确点击 Queue Start；任何非 Queue surface 都不会提交 Task/Comfy；原有 lineage 和历史保持。
~~~

### DEV-112 — Production state language and action feedback

~~~text
Goal=统一“准备、待启动、运行、完成、失败、返工”在各 workspace 的人类可读状态和按钮说明，降低技术 ID 和模式切换带来的推理。
Scope=建立共享 copy/state presentation contract；对 Workflow/Recipe 显示人类名称+版本、精确 ID 置于详情；在 Start 附近固定说明会创建/提交 Task 并启动 GPU，在 prepare/rework 附近固定说明不会启动。
Non-goals=不改底层状态枚举、不把名称当 identity、不修改 workflow/recipe 绑定、不做新国际化框架或大规模组件重写。
Acceptance=同一 READY/RUNNING/COMPLETED/FAILED/REWORK 状态在 Project、Shot、Queue、Review、Task surfaces 使用一致业务词；用户可区分 prepare 与 Start；精确 identity 仍可审计，现有 schema/IPC/authority guards 通过。
~~~

上述四个 DEV 只是建议切片，每项均可单独评审、测试和提交；本 DEV-108 不启动其中任何一项。

## Architecture boundary review

| Boundary | DEV-108 verdict | 1.3 guardrail |
| --- | --- | --- |
| Project scoped data | PASS | 所有 target、query、focus 必须携带并验证 project scope。 |
| Shot authority | PASS | 复用现有 Shot workspace/store，不另造镜头事实源。 |
| Asset authority | PASS | 复用 Asset Library/Asset Usage 和 exact asset navigation。 |
| Workflow + Recipe identity | PASS | 永远使用 exact workflowVersionId + recipeId pair；名称只用于显示。 |
| Preparation authority | PASS | prepare/create 继续只创建 READY batch，不代表运行。 |
| Production Queue explicit execution authority | PASS | 只有 Queue 的显式 Start 创建/提交真实 Task；任何 CTA 只能导航或提示。 |
| Task execution authority | PASS | 复用现有 Task/Production Monitor；不新增执行器或 task model。 |
| Review authority | PASS | Inbox 仍是 read projection；mutation 继续在现有 review surface。 |
| Backup compatibility | PASS | 不改 backup version、历史 references 或 remap 规则。 |
| External Agent handoff boundary | PASS | Handoff 仍只写正式结构，经预检和显式确认，不自动排队或执行。 |

## Test coverage gaps

~~~text
TEST_COVERAGE_GAPS
~~~

只记录缺口，不在 DEV-108 大量补测试：

1. 没有一个从 App 启动到 Project、Handoff、Preparation、Queue Start、Task、Review、Rework、Final Asset 的单一真实 App+Rust end-to-end journey test。
2. 有 focused UI/service tests，但没有把 daily target 的 task-only navigation、destination 与 section precedence 作为回归合同固定下来。
3. 有 500-shot admission/backend fixture，但没有 500-shot desktop UI 的渲染、滚动、键盘和时间预算测试。
4. Review Inbox 的列表 projection 和 Batch Review mutation 分别有覆盖，但没有对“从 Inbox 打开后默认聚焦正确 review item/candidate”做完整集成断言。
5. 没有跨入口的 final selected result + file location assertion，无法自动保证 Project Center、Shot、Asset、Production Monitor 的最后一步表达一致。
6. 键盘焦点、topbar search focus、drawer 可发现性和视觉信息层级没有专门的 accessibility/visual regression 套件。
7. Handoff 错误 path、长 JSON 和复杂 references 有 focused coverage，但没有真实 500-shot 错误修复工作流的桌面可用性回归。

## CI and ComfyUI boundary

~~~text
CI_ACCEPTABLE=YES
SOURCE_ONLY_FRONTEND_APPROX=3 min
SOURCE_ONLY_RUST_APPROX=14 min
REAL_COMFY_RUNTIME_AVAILABLE=NO
REAL_COMFY_SMOKE=NOT_RUN_ENVIRONMENT_UNAVAILABLE
~~~

当前 Source-only CI 的时长对全栈桌面仓库是可接受的。DEV-108 不为几分钟引入复杂矩阵、脆弱缓存或并行化改造。当前机器没有 127.0.0.1:8188，因此不宣称 live generation；已审计 capability detection、connection status、offline/unavailable state、错误展示和 recovery affordance。ComfyUI 不可用是环境边界，不是本次 P0/P1。

## Explicit not-first-phase list

以下事项不放入 1.3 第一阶段：

- 新的 deliverable domain model、schema 或迁移。
- 新 queue、executor、task model、scheduler 或 retry engine。
- 因为 Queue 不够显眼而让其他页面直接 Start。
- Review Inbox 承担第二套 review mutation。
- 未经真实 profile 证明的大规模虚拟化、缓存或 CI 矩阵重构。
- 先做完整 Workflow/Recipe 重绑定机制或按名称猜测 identity。
- live ComfyUI 大规模生产压力测试。

## Roadmap decision

~~~text
AI_STUDIO_1_3_FIRST_PHASE=Production Wayfinding & Action Continuity
AI_STUDIO_1_3_STARTED=NO
DEV_109_STARTED=NO
AUTO_NEXT_TASK=NO
~~~