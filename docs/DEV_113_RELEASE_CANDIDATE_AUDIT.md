# DEV-113 — AI Studio 1.3 Release Candidate Audit

## Audit result

```text
AUDIT=PRODUCT_AUDIT / RELEASE_READINESS / USER_JOURNEY_TEST
BASELINE_SHA=8c3217e9cff9fd9f745a9ad09e3bc1fb4c902c32
AUDIT_HEAD_AT_REVIEW=8c3217e9cff9fd9f745a9ad09e3bc1fb4c902c32
CODE_FIXES=NONE
P0=0
P1=0
P2=9
P3=2
AI_STUDIO_1_3_RELEASE_CANDIDATE=READY_WITH_P2
FIRST_TIME_USER_CAN_COMPLETE_FLOW=PARTIAL
OVERALL_PRODUCT_SCORE=7.9/10
DEV_109=COMPLETE
DEV_110=COMPLETE
DEV_111=COMPLETE
DEV_112=COMPLETE
DEV_113=COMPLETE
DEV_114_STARTED=NO
AUTO_NEXT_TASK=NO
```

### Decision

The 1.3 candidate is **READY_WITH_P2**. No P0 or P1 was found: the production path remains data-safe, the existing Production Queue is the only execution gate, and the journey can be completed without developer documentation. The first-time-user answer is **PARTIAL**, because several essential destinations are available in the UI but are not the obvious first choice for a new producer. Those findings affect speed and confidence rather than making production impossible.

This is an audit-only result. No product, domain, schema, migration, backup format, queue, task, review, asset, or workflow-engine change was made.

## Method and evidence

The audit started from master after `git fetch`, `git status`, `git branch`, and `git log --oneline -30`. Master was current with `origin/master`; DEV-112 was present; the only uncommitted path was the known local `.serena/` metadata directory, which was not staged. The audit read the README and DEV-108 through DEV-112 closeout/audit documents, then traced the current React entry, navigation, project, handoff, shot, queue, monitor, review, rework, and asset surfaces.

Relevant implementation evidence:

- `src/app/StartupScreen.tsx`, `src/app/App.tsx`, `src/components/studio/StudioGlobalRail.tsx`, and `src/components/studio/StudioTopBar.tsx` provide the startup, project context, breadcrumbs, and global destinations.
- `src/features/projects/ProjectWorkspace.tsx`, `src/features/projects/ProjectCommandCenter.tsx`, `src/features/projects/ProjectImportDryRunWorkspace.tsx`, and `src/features/projects/ExternalAgentHandoffPanel.tsx` provide project creation, status, dry-run import, and explicit handoff confirmation.
- `src/features/shots/ProjectStructureTree.tsx`, `src/features/shots/ProductionStructurePanel.tsx`, `src/features/shots/SceneProductionPreparation.tsx`, `src/features/shots/ShotWorkspace.tsx`, and `src/features/production/ProductionPackageWorkspace.tsx` provide hierarchy, preparation, and package entry points.
- `src/features/studio/ProductionQueuePanel.tsx` and `src/features/production/ProductionQueueDrawer.tsx` keep explicit Queue Start as the only real execution action.
- `src/features/production/ProductionMonitor.tsx`, `src/features/production/ProductionReviewInbox.tsx`, `src/features/studio/ProductionBatchReviewWorkspace.tsx`, `src/features/production/ReviewCompareWorkspace.tsx`, `src/features/assets/AssetPreview.tsx`, and `src/features/assets/AssetUsagePanel.tsx` provide monitor, review, rework, selected-result, usage, and file-location continuations.
- The current frontend suite passed **150 test files / 820 tests**. Coverage includes exact target navigation, 500-shot bounded views, handoff, preparation, queue start, monitor paging, review, rework, and production-package continuation.

## First-time producer journey

The simulated flow uses a new local project, a valid external-agent handoff, an imported hierarchy, ten eligible shots, the existing preparation and queue path, a completed result, and an optional rework cycle. `P2` means the step is usable but requires discovery or interpretation; `P3` means the step is clear and only has polish opportunities.

| Step | User Goal | Result | Friction | Severity |
| ---- | --------- | ------ | -------- | -------- |
| Open app | Understand what AI Studio is and how to begin | Startup truthfully prepares the local environment, then the shell exposes 项目、创作、资产、生产、审核、工作流、设置 | No concise first-run production path or “start here” explanation; a new user must infer the intended order from the rail | P2 |
| Create project | Create a project for the production | 项目 → 新建项目 creates the project and opens it; project list and active-project selector are available | Form explains local organization but not whether the next step is handoff, manual shots, or workflow setup | P2 |
| Import production handoff | Bring in external-agent production data safely | Project Command Center → 批量导入预检 → 导入外部生产数据; read-only preview, explicit confirmation, project-scoped/idempotent write, and no auto-start | The handoff entry is nested behind generic batch import language, and the format/schema explanation is more technical than producer-oriented | P2 |
| Review imported structure | Verify Series → Episode → Scene → Shot before production | Handoff success offers 打开项目结构; the tree shows the imported hierarchy and the handoff history/mappings remain available | The structure tree and mapping history are separate surfaces, so provenance and production readiness are not visible together in one first screen | P2 |
| Create production hierarchy | Add or repair the hierarchy needed for unassigned shots | The `+` menu exposes 新建系列 / 新建集 / 新建场景 / 新建镜头; 结构管理 handles rename, order, and archive | Creation is exposed as a compact icon/menu and a secondary management panel rather than a guided production setup step | P2 |
| Prepare 10 shots | Turn eligible shots into a production-ready batch | Scene/episode/project preparation shows readiness counts, selects eligible shots, performs backend preflight, and creates a `待启动` batch only | A new user must reach the production mode, choose the relevant project-production surface, and understand the package/project tabs | P2 |
| Open queue | Find the prepared batch and its action | Preparation and package creation expose 前往/打开生产队列; the queue is also available from the production rail/drawer | Queue is a secondary drawer/runbook surface, so the user can miss the authoritative next action after leaving preparation | P2 |
| Explicit start | Deliberately submit production | Queue clearly labels `待启动`, explains that only clicking 开始生产 submits real tasks, and reports `生产已启动` with a monitor continuation | No blocking friction; the explicit gate adds one intentional confirmation step | P3 |
| Monitor execution | Know what is running, done, or failed | Monitor exposes 运行中/成功/失败/已取消, progress, pagination, item details, retry, and failure guidance | Status is clear, but a large run still requires moving between monitor and result surfaces to form a complete delivery picture | P3 |
| Review results | Inspect candidates and understand the review state | Review Inbox and batch review expose 待审核, selected result, approval, rejection, and 待返工 filters/actions | The command-center inbox is a read-only projection and the user must take a second jump to the full review workspace | P2 |
| Select result | Choose the accepted candidate for the Shot | A/B review requires an explicit 选择结果并通过/审核通过 action; selected results can open the exact Asset and related Shot/Task | The chosen result is split across review, Shot, Asset, and Task continuations rather than presented as one delivery record | P2 |
| Optional rework | Request a targeted revision without accidentally running it | Review exposes 编辑返工准备; the dialog creates a new `待启动` batch, preserves the original record, and offers the queue CTA | The distinction between the old reviewed batch and the new ready batch is safe but requires reading the confirmation copy | P3 |
| Create rework batch | Save the revised production input | Rework creation succeeds as a new waiting batch; no auto-generation or second executor is introduced | The action is correct but the user must recognize that “创建返工批次” is preparation, not execution | P3 |
| Queue start again | Run only the rework batch | The same queue Start gate transitions the batch to `运行中`; project and exact batch focus are retained | No core blocker; the repeated navigation is intentional and could be more guided for first-time users | P3 |
| Find final result | Locate the final accepted Shot, Asset, and file | Completed results can open the exact asset/task/shot; monitor can open a file location or finished-products folder when a database local path exists | There is no single “final delivery” view combining accepted result, Shot, Asset, and file path; the fastest route depends on where the user starts | P2 |

## Product scores

| Area | Score | Audit judgment |
| ---- | ----: | -------------- |
| APP_ENTRY | 7.0/10 | Honest, stable shell and clear destinations, but weak first-run orientation |
| PROJECT | 7.2/10 | Creation is direct; purpose and next action are under-explained |
| HANDOFF | 8.0/10 | Strong preview/confirm/safety behavior once discovered |
| COMMAND_CENTER | 8.4/10 | Answers status, next action, issues, bounded collections, and exact targets well |
| PREPARATION | 8.5/10 | Readiness and no-auto-start semantics are explicit and tested |
| QUEUE | 8.2/10 | Sole execution authority and action language are clear; placement is secondary |
| MONITOR | 8.5/10 | Progress, failure handling, paging, and result actions are concrete |
| REVIEW | 8.2/10 | Explicit review decisions and exact result continuation are strong |
| REWORK | 8.4/10 | New READY batch and no-auto-start behavior are unambiguous |
| FINAL_RESULT | 7.0/10 | Result, Shot, Asset, and file actions exist but are split across surfaces |
| **OVERALL** | **7.9/10** | **Releaseable with documented P2 wayfinding debt** |

## DEV-109 through DEV-112 continuity checks

### Exact target navigation

PASS. Navigation requests carry the exact project plus the relevant `taskId`, `batchId`, `reviewId`/item ID, `assetId`, or `shotId`. `src/app/App.tsx` applies project switching before focus, and the precedence remains review → task → batch → asset → shot. The destination opens the expected workspace and focuses the entity; invalid explicit targets do not silently fall back. Cross-project targets are rejected or switched through the existing project context instead of leaking data.

### 500-shot locate and triage

PASS. The Command Center keeps a bounded 20-item preview and exposes View All only for larger collections. Existing project-scoped filters locate failed, running, ready, completed, unassigned, and review work; the structure tree and production monitor page large sets instead of rendering an unbounded list. Current tests cover 500-shot summaries, exact filtered navigation, 500-item monitor pagination, and 500-item production-package selection.

### Preparation → READY → Queue → Start

PASS. Preparation performs live preflight and creates only `待启动`/READY batches. It exposes Queue navigation, and the queue remains the only place that can submit actual production. The current tests assert that preparation never calls `startProductionQueue`.

### Review → Rework → READY → Queue → Start

PASS. Review can explicitly create a new rework batch. The new batch remains `待启动`, the original failure/review record remains historical, and the queue CTA/start is separate. The current review, rework, queue, and ShotWorkspace production tests assert no auto-start and preserve exact batch continuation.

### Language contract

PASS for the audited production surfaces:

| Internal state | Required user language | Observed |
| -------------- | ---------------------- | -------- |
| READY / PENDING | 待启动 | Queue, preparation, package, rework notices |
| RUNNING | 运行中 | Queue, monitor, command center, banners |
| FAILED | 失败，需要处理 | Monitor and item retry guidance |
| REVIEW | 待审核 | Command Center, Review Inbox, review workspace |

The surrounding application also preserves `已完成` and `待返工` where those are the correct public states. Raw IDs remain available inside technical details or workflow diagnostics, not as the primary state language.

## Safety and release gates

- No P0: no observed inability to produce, project isolation failure, data corruption, or automatic execution.
- No P1: no observed severe blockage in the complete preparation → queue → explicit start → monitor → review → rework continuation.
- Existing Production Queue remains the single production execution authority.
- Existing Studio Store and typed Tauri transport remain the frontend authority/boundary.
- No new schema, migration, backup version, executor, task model, review authority, asset model, or workflow engine was introduced.

## Validation record

```text
FRONTEND_TEST=PASS — pnpm test (150 files, 820 tests)
TSC=PASS — pnpm exec tsc --noEmit
BUILD=PASS — pnpm build
RUST_CHANGED=NO
RUST_TEST=NOT RUN — no Rust source changed; latest DEV-112 exact-head Source-only CI evidence was passing
REMOTE_CI=PENDING AT DOCUMENT AUTHORING — final exact-head Source-only CI is recorded in the DEV-113 result after push
```

The build emitted the existing non-blocking Vite chunk-size warning for the main bundle. The test run emitted the existing jsdom navigation warning in production coverage; it did not fail a test.

## Release decision

```text
P0_COUNT=0
P1_COUNT=0
P2_COUNT=9
P3_COUNT=2
AI_STUDIO_1_3_RELEASE_READY=YES (no P0/P1)
AI_STUDIO_1_3_RELEASE_CANDIDATE=READY_WITH_P2
FIRST_TIME_USER_CAN_COMPLETE_FLOW=PARTIAL
```

The candidate may proceed as a release candidate with the P2 list explicitly accepted by the product owner. Do not silently convert these findings into DEV-114 work; the next step is a product decision to release, fix selected P2s, or open the next theme.
