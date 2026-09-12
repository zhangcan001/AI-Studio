# DEV-109 — Exact Target Navigation & Focus

```text
TASK=DEV-109
BASELINE_SHA=43843d4160e51ed30acf9f1fa06b41bbebcf1707
FINAL_SHA=PENDING

AI_STUDIO_1_3_STARTED=YES
DEV_109_STARTED=YES

PROJECT_SCOPE_NAVIGATION=PASS
TASK_TARGET=PASS
BATCH_TARGET=PASS
REVIEW_TARGET=PASS
ASSET_TARGET=PASS
SHOT_TARGET=PASS
STALE_FOCUS_REPLACEMENT=PASS
CROSS_PROJECT_ISOLATION=PASS

FRONTEND_TEST=PASS
TSC=PASS
FRONTEND_BUILD=PASS

RUST_CHANGED=NO
NO_RUST_DOMAIN_CHANGE=YES
RUST_TEST=NOT_RUN_NO_RUST_CHANGE

P0=NONE
P1=NONE

DEV_109=PASS
DEV_110_PLANNED=YES
DEV_110_STARTED=NO
DEV_111_STARTED=NO
DEV_112_STARTED=NO
AUTO_NEXT_TASK=NO

COMMIT=feat(navigation): add exact production target focus
PUSHED=PENDING
REMOTE_CI_RUN=PENDING
REMOTE_CI_STATUS=PENDING
```

## 实现边界

DEV-109 复用了现有 `ProjectCommandCenterNavigationRequest`、App workspace state、Studio Store、Shot selection、Production Queue controller 和 Review surface；没有引入 Router、第二套 target model、第二个 Queue/Task/Review authority，也没有修改 Rust、数据库、migration、backup 或 release version。

导航优先级为：

1. Review source 带 `reviewId`/`itemId` 时，进入 `shots/review`，并保留 batch、shot、task、asset context；
2. task target 进入 Task History，直接按 `taskId` 读取详情，不依赖第一页；
3. batch target 进入 `shots/production`，重新读取当前项目 Queue projection，再展开并标记 exact batch；
4. asset target 进入 Asset Library，并按 `assetId` 自动打开 detail；
5. shot target 进入 Shot workspace，并按 `shotId` 选择 exact Shot。

Command Center 顶层会为请求补上所属 `projectId`。App 对显式 project target 做存在性校验；跨项目时先切换 active project，再应用 entity focus。所有 exact navigation 都会替换 task、batch、review、asset、shot focus channels；普通 rail/workspace navigation 会清理旧 focus。

Exact target 不可用时不再静默回退：Task/Asset 使用已有直接 lookup，Shot selection 保留不可用请求并显示错误，Review batch/item 显示不可定位状态而不回退到第一项或 legacy board。Shot 用户主动选择其他镜头后，后续普通列表 reconciliation 仍可按现有行为选择可用镜头。

## Focused regression coverage

- Command Center task target → Task History exact task direct lookup。
- Daily/production batch target → existing Queue drawer exact batch focus and expansion；不启动队列。
- Review Inbox → exact review item、batch、shot、task、stage context。
- Review Inbox final result → exact Asset request。
- Asset Usage → exact Shot ID。
- Cross-project target → switch project before rendering target Shot。
- Invalid review/shot target and stale shot target replacement → clear failure/no silent approximate object。

## Scope guards

```text
NO_NEW_ROUTER_FRAMEWORK=YES
NO_NEW_DOMAIN_MODEL=YES
NO_NEW_QUEUE_MODEL=YES
NO_NEW_TASK_MODEL=YES
NO_NEW_REVIEW_AUTHORITY=YES
NO_SCHEMA_CHANGE=YES
NO_MIGRATION_CHANGE=YES
NO_BACKUP_VERSION_CHANGE=YES
UNRESOLVED_NAVIGATION_ISSUES=NONE_WITHIN_SCOPE
```

Browser-style deep-link persistence and the 500-shot locate/search work remain intentionally outside DEV-109 and are not started here.
