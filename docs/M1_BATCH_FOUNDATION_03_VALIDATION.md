# M1 Batch Foundation 03 Validation

Date: 2026-08-08

Scope: persistent production queues and local automation orchestration for the intentionally limited production model set: Kera2 image generation and MiniMax H3 video generation.

Status: IMPLEMENTED / REGRESSION VALIDATION BLOCKED BY LOCAL COMMAND CHANNEL.

## Durable queue model

Migration `006_production_queue.sql` adds two new tables without modifying earlier migrations:

- `production_batches`: project-scoped production job metadata and lifecycle state.
- `production_batch_items`: ordered immutable workflow/recipe/value snapshots with per-item Task linkage and outcome evidence.

Batch states:

- `READY`
- `RUNNING`
- `PAUSED`
- `COMPLETED`

Item states:

- `PENDING`
- `DISPATCHING`
- `DISPATCHED`
- `SUCCEEDED`
- `FAILED`
- `CANCELLED`

Every dispatched item still creates a normal independent AI Studio Task through the existing `GenerationService`. Existing Task history, cancellation, recovery, snapshot, ComfyUI execution, and Asset output paths remain authoritative.

## Automation orchestration

`ProductionQueueService` runs as a local background orchestrator. It is not a second generation engine.

For each RUNNING production batch:

1. At most one item in that batch is dispatched at a time.
2. The item is first persisted as `DISPATCHING`.
3. A normal `GenerationService::start_generation` call creates the Task.
4. The Task ID is persisted and the item becomes `DISPATCHED`.
5. The orchestrator observes the persisted Task status.
6. Only after that Task reaches a terminal state does the queue decide whether to continue.
7. When no pending/active items remain, the batch becomes `COMPLETED`.

Multiple batches may exist, while the existing generic ComfyUI submission semaphore continues to protect the local `/prompt` submission boundary.

## Failure policy

Default `continue_on_failure = false` pauses a production batch on failed or cancelled items.

When `continue_on_failure = true`, ordinary non-execution failures/cancellations may be skipped and the queue may continue to the next item.

`EXECUTION_ERROR` is always treated as a fatal execution failure that pauses the persistent batch regardless of the continue setting. This is intentional for the currently observed MiniMax H3 GPU OOM class, preventing a queue of H3 jobs from repeatedly hitting the same 16 GB VRAM failure.

No automatic retry loop is introduced. Foundation 02 `Retry Once` remains the only bounded retry path and does not apply to `EXECUTION_ERROR`.

## Crash/restart safety

The queue has an explicit `DISPATCHING` state to protect the dangerous window between queue selection and Task linkage.

If AI Studio restarts and finds `DISPATCHING` with no persisted Task ID, it does not assume the Task was never created. Instead it:

- marks the item `FAILED`
- records `QUEUE_DISPATCH_UNCERTAIN`
- pauses the production batch
- requires explicit user review/resume

This favors duplicate-generation prevention over aggressive auto-retry.

Startup ordering is:

1. existing Task recovery/reconciliation
2. production queue uncertain-dispatch recovery
3. resume batches that are still persisted as `RUNNING`

If Task recovery itself fails, production queue auto-resume is skipped.

## Frontend workflow

The Studio now keeps two distinct concepts:

- Local Batch Queue: editable/frozen in-memory items for immediate multi-Task submission.
- Persistent Production Queue: a saved ordered batch that survives navigation and application restart.

A local batch can be saved with:

- queue name
- current ordered items
- `continueOnFailure` policy

Persistent queues support:

- list by active project
- open/detail view
- start
- pause
- resume
- manual refresh
- persisted item counts and Task IDs/error codes

Pausing a queue does not forcibly cancel a Task that has already been dispatched. Resume re-observes that Task and continues only after its terminal state is known.

## Production scope lock

No Wan, Flux, Qwen, or third-model runtime/UI path was added. Kera2 image generation and MiniMax H3 video generation remain the only production model scope.

## Tests / validation hooks added

- Queue value serialization round-trip test preserves typed seed and Asset identity.
- Queue failure-policy test locks `EXECUTION_ERROR` as a mandatory pause even when continue-on-failure is enabled.
- SQLite recovery test verifies a restart during `DISPATCHING` becomes `QUEUE_DISPATCH_UNCERTAIN` and pauses the batch rather than duplicating generation.
- Existing SQLite migration tests were updated from 10 to 12 expected business tables and now include both production queue tables in the schema query.
- Foundation 01/02 batch/import/retry tests remain in the working tree.

Required regression commands before PASS:

- `cargo fmt --all -- --check`
- `cargo check`
- `cargo test -- --test-threads=1`
- `pnpm test`
- `pnpm build`
- `git diff --check`

The current high-performance MCP bash channel still fails before project execution because the Windows host cannot start the configured WSL runtime. Therefore Foundation 01, 02, and 03 must not be labeled final PASS yet.

## Next recommended stage

`BATCH FOUNDATION 04 — production queue operations + observability hardening`

Recommended scope after regression validation:

- delete/archive completed or unused queues
- explicit failed-item skip/requeue controls with safety classification
- queue/task event-driven UI refresh instead of manual refresh
- production summary/dashboard for long-running Kera2 + MiniMax H3 jobs
- final live Kera2 queue run and, when a 16 GB-compatible H3 workflow is available, live mixed Kera2/H3 queue validation
