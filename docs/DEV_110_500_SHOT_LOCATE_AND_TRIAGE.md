# DEV-110 — 500-Shot Locate & Exception Triage

```text
TASK=DEV-110
BASELINE_SHA=38befc5b7d7e5c833b2a5383ccb0c1023265348a
FINAL_SHA=ed33e8fed2235b09a07d406e2fd2cfe8e44a9b05

AI_STUDIO_1_3_STARTED=YES
DEV_110_STARTED=YES

500_SHOT_SUMMARY=PASS
500_SHOT_PREVIEW_BOUNDED=PASS
500_SHOT_TOTALS_CORRECT=PASS
500_SHOT_VIEW_ALL=PASS
500_SHOT_FILTERED_LOCATE=PASS
500_SHOT_PROJECT_ISOLATION=PASS

FAILED_COLLECTION=PASS
RUNNING_COLLECTION=PASS
READY_COLLECTION=PASS
REVIEW_COLLECTION=PASS
COMPLETED_COLLECTION=PASS
FILTER_REPLACEMENT=PASS
FILTER_VISIBILITY=PASS
PROJECT_SCOPE_ISOLATION=PASS
EXACT_ITEM_CONTINUITY=PASS
COUNT_CONSISTENCY=PASS
NO_FULL_500_ITEM_COMMAND_CENTER_RENDER=PASS

FRONTEND_TEST=PASS
TSC=PASS
FRONTEND_BUILD=PASS

RUST_CHANGED=NO
NO_RUST_DOMAIN_CHANGE=YES
RUST_TEST=NOT_RUN_NO_RUST_CHANGE

NO_NEW_SEARCH_ENGINE=YES
NO_NEW_ANALYTICS_DOMAIN=YES
NO_NEW_QUEUE_MODEL=YES
NO_NEW_TASK_MODEL=YES
NO_NEW_REVIEW_AUTHORITY=YES
NO_SCHEMA_CHANGE=YES
NO_MIGRATION_CHANGE=YES
NO_BACKUP_VERSION_CHANGE=YES

P0=NONE
P1=NONE

DEV_110=PASS
DEV_111_STARTED=NO
DEV_112_STARTED=NO
AUTO_NEXT_TASK=NO

REMOTE_CI_RUN=34699885946
REMOTE_CI_STATUS=PASS (Rust source checks + Frontend source checks)

COMMIT=feat(production): add large-project filtered locate flows
PUSHED=YES
```

## 实现结果

DEV-110 只增加“大项目定位”能力，没有把 Command Center 改造成完整列表页。Command Center 仍使用后端的 bounded daily projection，最多显示 20 项；`查看全部` 只携带 project-scoped collection intent，用户真正进入目标 workspace 后才读取已有完整列表。

新增或补齐的入口如下：

| Summary / bucket | 目标 surface | 初始筛选 |
| --- | --- | --- |
| 失败任务 | Task History | `status=FAILED` |
| 运行中任务 | Task History / Production Monitor | `status=ACTIVE` |
| 待生成镜头 | Shot Workspace → Production | `status=READY` |
| 已完成镜头 | Shot Workspace → Creation | `status=COMPLETED` |
| 未分配镜头（仅当事实口径明确） | Shot Workspace → Creation | `sceneId=UNASSIGNED` |
| 待审核结果 | Shot Workspace → Review 中的现有 Review Inbox | `review state=PENDING`（未审 / 待返工） |
| 可明确区分的 image/video review bucket | Shot Workspace → Review | 对应 `status` + `stage` |

Command Center 还提供紧凑的失败、运行中、Ready、Completed 集合卡片；只有总数大于 20 时才显示 `查看全部 N`，避免与已经完整显示的小集合重复。Daily Production 对 `running`、`needsAttention`、`review` 等可能混合多个事实口径的 bucket 只在 total 与 aggregate authority、reason code 都一致时提供过滤 CTA，无法安全映射时不误导用户。

Waiting Review 复用了已有 `ProductionReviewInbox` 的分页 / load-more 和 total count。Summary 模式在有更多结果时显示 `查看全部 N`，进入 Review workspace 后只显示该项目的完整待处理集合，没有在 Command Center 额外渲染 500 条 review card。

## Filter、Project Scope 与 Exact Target

`ProjectCommandCenterNavigationRequest` 扩展了 navigation-only 的 `collectionFilter` union；它不是新的数据模型或 authority。App 继续先解析 exact review/task/batch/asset/shot target，再解析 collection target，所以从 filtered list 点击具体项时仍保持 DEV-109 的 exact target 连续性。

所有新增集合 CTA 都绑定来源项目的 `projectId`。App 对显式项目做存在性校验；跨项目导航先切换到目标项目，再应用 workspace 和 collection filter。Shot、Task History、Review Inbox 都把项目范围显示给用户，不能通过清除局部筛选跨项目。

摘要入口会替换 contextual filter，而不是叠加旧状态：Shot 导航重置 search、status、scene、page size 和 page；Task 导航重置 status、keyword、workflow、time filter；Review Inbox 使用固定的 pending 状态。进入后仍可通过现有 toolbar / filter UI 手工清除 status、scene 或 search，查看该项目的完整内容。Shot toolbar、Task History header 和 Review Inbox header 都显示当前筛选与项目范围。

## 500-Shot 证据与计数口径

- 现有 Rust 500-shot synthetic fixture 保持不变：500 个镜头、500 个未分配事实、daily preview 20 项、`hasMore=true`；本任务未修改 Rust。
- 新增前端回归以 500 个未分配镜头的 aggregate 计数验证：Command Center 行仍只有 20 个，显示 `500`，并产生带 `sceneId=UNASSIGNED` 和 project scope 的 `查看全部 500` 请求。
- 完整列表继续使用 Shot Workspace 的现有 status / scene / pagination，Review Inbox 继续使用现有 paged query；Summary 不会因为增加 CTA 自动请求全量 500 条。
- aggregate count、collection filter 与目标列表使用同一项目和状态事实；UI 不把 task failure 伪装成 shot failure。目标列表仍显示现有的匹配数 / 总数，Review Inbox 显示其 query 的 total。

## Validation

本地验证：

```text
pnpm test                         PASS — 149 files / 813 tests
pnpm exec tsc --noEmit            PASS
pnpm build                        PASS
git diff --check                  PASS
```

Focused coverage covers bounded 500-shot preview, failed/running/ready/completed collection CTAs, Review Inbox View All, filter replacement and visibility, project-scoped navigation, and exact item precedence. No Rust, schema, migration, backup version, release metadata, queue/executor/task model, search engine, or new review authority was added.

DEV-111（Queue / Review / Rework Continuation）与 DEV-112 均未开始；本任务完成后停止，等待产品负责人确认。
