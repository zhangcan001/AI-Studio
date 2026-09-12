# DEV-105 — Production Execution / Review Entry-Point Audit

Baseline: `d28decdd337e5beacdd8c36f0edb48519c22a550`  
Implementation head: `6a6d6cab31d5097befa4ad3a366c52f229655716`

## Audit matrix

| PATH | USER_ACTION | CREATES_BATCH | CREATES_TASK | STARTS_QUEUE | SUBMITS_COMFY | EXPLICIT_START | DECISION |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `ShotBulkConfigPanel` | Prepare selected shots | YES — existing project admission | NO | NO | NO | NO | SAFE: prepare-only |
| `ShotBatchPlanner` | Prepare a shot batch | YES — existing batch path | NO | NO | NO | NO | SAFE: prepare-only |
| `SceneProductionPanel` | Prepare scene work | YES — existing scene admission | NO | NO | NO | NO | SAFE: prepare-only |
| `SceneProductionPanel` | Start prepared batch | NO | NO | YES | YES, through the existing executor | YES — `startPreparedBatch` | SAFE: explicit execution |
| `EpisodeProductionPanel` | Prepare episode work | YES — existing episode admission | NO | NO | NO | NO | SAFE: prepare-only |
| `SeriesProductionPanel` | Prepare series work | YES — existing series admission | NO | NO | NO | NO | SAFE: prepare-only |
| `ProductionQueuePanel` | Create queue batch | YES — existing `ProductionQueueService.create` | NO | NO | NO | NO | SAFE: create is not start |
| `ProductionQueuePanel` | Click Start / Continue | NO | YES, when execution begins | YES | YES, through `production_queue_start` | YES — visible Start/Continue action | SAFE: sole start authority |
| `ProductionPartialResumePanel` | Partial resume | NO new batch; appends pending retry items through existing policy | NO | NO | NO | NO | SAFE: explicit start remains later |
| `ProductionBatchReviewWorkspace` | Regenerate one item | YES — READY rework batch | NO | NO | NO | NO | SAFE: rework creation only |
| `ProductionBatchReviewWorkspace` | Regenerate marked items | YES — one READY rework batch with selected items | NO | NO | NO | NO | SAFE: bulk rework creation only |
| `ShotBatchReviewBoard` legacy review | Confirm one regeneration | YES — READY rework batch | NO | NO | NO | NO | SAFE: opens existing queue only |
| Queue item operation | Requeue failed item | NO new batch; appends pending retry item | NO until explicit start | NO | NO | NO | SAFE: retry item creation only |
| `Production Package` | Inspect / commit a package | YES — existing package batch, `auto_started=false` | NO | NO | NO | NO | SAFE: package creation is not start |
| `External Agent Handoff` | Preview / confirm import | NO queue batch | NO | NO | NO | NO | SAFE: provenance/import boundary only |
| `AssetVideoBatchWorkspace` legacy path | Local import with “导入后立即开始生成” selected | YES — legacy local-import batch | NO | YES, only when the visible opt-in is selected | YES | YES — explicit legacy opt-in | LEGACY_EXPLICIT / OUT OF SCOPE |

`GenerationService`, ComfyUI submission, and task execution remain behind the
existing Production Queue start boundary. No prepare, review, regenerate,
package, handoff, retry, or partial-resume action silently crosses that
boundary.

## Review regeneration audit

The old review regeneration contract accepted `autoStart` and the review UI
passed `autoStart: true`. DEV-105 removes that capability end-to-end:

- review service regeneration creates the existing READY rework batch only;
- single and bulk command DTOs no longer expose `autoStart`;
- the typed frontend client no longer sends the field;
- the review UI says `已创建返工批次，等待启动。` and offers `打开生产队列`;
- task creation and ComfyUI submission occur only after the user starts the
  batch from `ProductionQueuePanel`.

Rework lineage continues to use the existing batch/item/retry lineage fields.
The original review state, note, task, and failure evidence remain historical
records; creating a rework batch does not overwrite them.

## Project Review Inbox

The project-level Review Inbox is a bounded, read-only projection over
`production_item_review`, batch/item/task lineage, the existing Shot result
links, and Asset rows. It uses one project-scoped set-based query with a
bounded page size and incremental loading. It does not add a review table,
second review state, or second queue. Exact existing IDs route users to the
review item, queue task, Shot, and Asset, and the inbox displays the Shot
selected output without creating a competing preferred-result authority.

Bulk review mutation remains deferred. Existing individual status/note
mutations remain the review mutation authority.

## Queue semantics confirmed

`READY`, `RUNNING`, `PAUSED`, and `COMPLETED` remain queue lifecycle states;
cancel-pending, archive/restore, skip, requeue, and partial resume mutate the
existing queue state or lineage only. `APPROVED`, `STARRED`, `REGENERATE`,
`REJECTED`, and `UNREVIEWED` remain independent review states. Approval or
starred status does not imply the Shot selected result.

## Guard evidence

`node scripts/dev088-architecture-guard.mjs` reports:

```text
REVIEW_REGEN_NO_AUTO_START=PASS
REWORK_BATCH_EXPLICIT_START=PASS
REVIEW_USES_EXISTING_AUTHORITY=PASS
NO_SECOND_REVIEW_QUEUE=PASS
EXECUTION_START_AUTHORITY=PASS
REVIEW_REGEN_AUTO_START_CAPABILITY=REMOVED
```

