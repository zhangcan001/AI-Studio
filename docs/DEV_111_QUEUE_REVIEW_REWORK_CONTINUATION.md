# DEV-111 — Queue / Review / Rework Continuation

```text
TASK=DEV-111
BASELINE_SHA=b767d5c7e53d3630aecc4cd67b68f6ed1cb95a2b
FINAL_SHA=dc404bdb14070b2b2484ca30a3a41b13dbe63144

AI_STUDIO_1_3_STARTED=YES
DEV_111_STARTED=YES

PREPARATION_CREATES_READY_ONLY=PASS
PREPARATION_TO_QUEUE=PASS
BULK_PREPARATION_TO_QUEUE=PASS

REWORK_CREATES_READY_ONLY=PASS
REVIEW_TO_REWORK=PASS
REWORK_TO_QUEUE=PASS
REVIEW_INBOX_CONTINUITY=PASS

QUEUE_EXPLICIT_START=PASS
NO_AUTO_START_PREPARATION=PASS
NO_AUTO_START_REWORK=PASS
NO_AUTO_START_REVIEW=PASS

EXACT_BATCH_CONTINUATION=PASS
PROJECT_SCOPE_ISOLATION=PASS
STALE_CONTINUATION_REPLACEMENT=PASS
FAILURE_NO_QUEUE_CTA=PASS
QUEUE_START_FEEDBACK=PASS
REVIEW_SELECTION_CONTINUATION=PASS

FRONTEND_TEST=PASS
TSC=PASS
FRONTEND_BUILD=PASS

RUST_CHANGED=NO
NO_RUST_DOMAIN_CHANGE=YES
RUST_TEST=NOT_RUN_NO_RUST_CHANGE

NO_SCHEMA_CHANGE=PASS
NO_MIGRATION_CHANGE=PASS
NO_BACKUP_VERSION_CHANGE=PASS
NO_VERSION_BUMP=PASS
NO_NEW_QUEUE_MODEL=PASS
NO_NEW_EXECUTOR=PASS
NO_NEW_TASK_MODEL=PASS
NO_NEW_REVIEW_AUTHORITY=PASS

P0=NONE
P1=NONE

DEV_111=COMPLETE
DEV_112_PLANNED=YES
DEV_112_STARTED=NO
AUTO_NEXT_TASK=NO

REMOTE_CI_RUN=34723844389
REMOTE_CI_STATUS=SUCCESS

COMMIT=feat(production): connect preparation review and queue flows
PUSHED=YES
```

## 实现结果

- Preparation、Scene、Episode、Series 和 Bulk Preparation 成功后明确显示“已准备完成 / READY / 尚未启动”，并提供打开生产队列的 CTA。已有 `batchId` 优先传递；单批次结果进入 Queue 后精确聚焦该批次，多批次保留现有项目范围 Queue surface。
- Project Command Center 的 READY Continue Work 在 bounded ready projection 只有一个唯一批次时携带该 `batchId`；多个批次不猜测身份，继续使用现有项目范围入口。
- Review 选择结果后保留现有选择与审片状态 authority，并提供精确 `Shot` / `Asset` continuation。Review Inbox 仍是只读投影，继续进入现有精确审片 surface。
- Video Rework 复用现有 `regenerateProductionItem` 返回的 batch detail，成功后显示 READY / 尚未启动并提供 exact rework batch Queue CTA；不自动打开或启动 Queue。已有 Batch Review Workspace 的返工 CTA 同样继续使用返回的 batch ID。
- Queue Start 仍是唯一真实执行入口。Preparation、Bulk Preparation、Review、Rework、Command Center 和 Shot Workspace continuation 不调用 `startBatch`、Task submit 或 Comfy submit；Queue 明确提示只有点击“开始”才会创建并提交生产任务。Start 成功后显示可在生产监控查看运行任务的反馈。
- 新尝试会清除旧 preparation / admission / rework continuation，避免旧成功 CTA 在失败或新结果后继续指向旧 batch；失效的 exact batch 继续由既有 Shot Queue focus failure 规则显示“目标生产批次不存在或已不可用”。
- 未修改 Rust、数据库 schema、migration、backup/version、installer、Queue/Task/Review domain authority 或 chunking/admission 规则；没有开始 DEV-112。

## Focused regression coverage

- `ShotBatchReviewBoard.test.tsx`: confirmed rework remains no-auto-start and exposes exact READY batch only after the user clicks the Queue CTA; selected-result Shot / Asset continuation; failed rework leaves no Queue CTA.
- `ProjectWorkflowFormalProductionUat.test.tsx`: Bulk Prepare creates the READY batch without Comfy submission, then passes the exact batch to the Queue continuation before an explicit test Start.
- `EpisodeProductionPanel.test.tsx` and `SeriesProductionPanel.test.tsx`: READY/not-started result language, blocked-result CTA suppression, and exact created batch callbacks.
- `ProjectCommandCenter.test.tsx`: unique READY daily-board batch is carried into the existing production navigation request.
- `ProductionQueueDrawer.test.tsx` and existing Scene / Review Inbox / Queue tests: Queue authority note, Scene exact CTA, Review exact target and no execution outside Queue.

## Validation

```text
pnpm test                  PASS — 149 files / 818 tests
pnpm exec tsc --noEmit     PASS
pnpm build                 PASS
git diff --check           PASS
Source-only CI 34723844389 PASS — Frontend source checks + Rust source checks
```

DEV-111 is complete. DEV-112 is planned but not started; stop here and wait for product-owner confirmation.
