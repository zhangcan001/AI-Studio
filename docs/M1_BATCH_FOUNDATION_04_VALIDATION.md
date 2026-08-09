# M1 Batch Foundation 04 Validation

Date: 2026-08-08

Scope: production queue operations and observability hardening for the intentionally limited production model set: Kera2 image generation and MiniMax H3 video generation.

Status: PASS (validated 2026-08-09).

## Queue lifecycle operations

Migration `007_production_queue_operations.sql` extends the Foundation 03 queue schema without modifying earlier migrations:

- `production_batches.archived_at` records archive state.
- `production_batch_items.retry_of_item_id` records the source item for an explicit requeue attempt.

Supported operations:

- Archive: only when the batch is not RUNNING and has no DISPATCHING/DISPATCHED item.
- Restore: clears archive state and returns the queue to normal control.
- Delete: requires the queue to be archived first and still requires no active item.
- Deleting a production queue deletes only queue metadata/items through the queue foreign-key cascade. Existing Tasks and generated/source Assets remain governed by their existing repositories and are not deleted by this operation.

Archived queues are read-only for start, pause, skip, and requeue until restored. Archived RUNNING queues are not auto-resumed on application startup.

## Failed-item operations

`SKIPPED` is now a terminal production-item state.

Skip:

- allowed only for FAILED or CANCELLED queue items
- requires the batch to be restored, non-RUNNING, and free of an active DISPATCHING/DISPATCHED item
- preserves the original Task ID, error code, error message, and queue-item evidence

Requeue:

- never mutates the original failed/cancelled item
- appends a new PENDING item at the end of the same queue
- copies the original workflow version, recipe, and typed generation values
- records `retry_of_item_id` pointing at the original item
- leaves the queue PAUSED; the user must explicitly Resume it
- is limited by the existing 100-item production-batch ceiling

Safe automatic requeue classification is intentionally narrow:

- `COMFY_OFFLINE`
- `COMFY_TIMEOUT`
- `COMFY_STREAM_DISCONNECTED`
- `COMFY_IMAGE_UPLOAD_FAILED`
- `COMFY_INPUT_UPLOAD_FAILED`
- `EXECUTION_INTERRUPTED`
- explicit CANCELLED items

The following are intentionally not requeue-safe:

- `EXECUTION_ERROR` — including the currently observed MiniMax H3 GPU OOM class
- `QUEUE_DISPATCH_UNCERTAIN`
- compile/definition/snapshot/input-integrity failures and other deterministic errors

This prevents a production queue from repeatedly exhausting the 16 GB GPU or duplicating a task whose previous dispatch outcome is uncertain.

## Observability

The Studio persistent-queue panel now provides a project-scoped production overview:

- unarchived queues
- running queues
- pending items
- active items
- succeeded items
- failed items
- archived queues

Selected queue detail includes:

- Pending / Active / Succeeded / Failed / Cancelled / Skipped counts
- Task ID for dispatched items
- error code and safe error summary
- requeue ancestry through `retryOfItemId`
- `Open Task`, which navigates to the existing Task History detail rather than creating a second task-inspection surface

The existing Task event channel now triggers a debounced queue/detail refresh. Manual Refresh remains available as a fallback if the event listener is unavailable.

## UI correction

A prior Foundation 03 stylesheet block contained literal leading `+` characters before `.production-queue-*` selectors, which made those selectors invalid CSS. Foundation 04 removes those prefixes and replaces the block with the current production-console styles and responsive layouts.

## Tests / safety hooks added

Backend:

- transient requeue classification allows Comfy/network interruptions but rejects `EXECUTION_ERROR` and `QUEUE_DISPATCH_UNCERTAIN`
- archive/restore persistence is verified
- requeue persistence verifies the original FAILED item remains unchanged and the new PENDING item records `retry_of_item_id`
- existing uncertain-dispatch recovery and fatal execution-error tests remain in place

Frontend:

- pure `productionQueuePolicy` tests allow cancelled/transient items
- explicitly reject H3/`EXECUTION_ERROR` and uncertain-dispatch requeue
- reject deterministic and non-terminal queue items

## Production scope lock

No Wan, Flux, Qwen, or third-model runtime path was added. Product scope remains Kera2 image generation + MiniMax H3 video generation only.

## Regression status

Status: PASS. Native Windows PowerShell removed the previous WSL command-channel blocker. The complete suite passed:

- `cargo fmt --all -- --check`
- `cargo check`
- `cargo test -- --test-threads=1` — 238 passed, 0 failed
- `pnpm test` — 13 files, 31 tests passed
- `pnpm build`
- `git diff --check`

The real production gate verified Archive, Restore, protected Delete, transient failure Skip, and transient failure Requeue through the live Tauri command boundary. Requeue preserved the original failed item and produced a successful linked retry with an Asset; Skip preserved the original Task/error evidence and completed without an Asset. The Studio UI reflected all queue overview counts and Asset Library outputs.

## Next recommended stage

`PRODUCTION VALIDATION 01 = PASS`.

The next work is limited to MiniMax H3 runtime OOM analysis and a 16 GB-compatible live completion gate. No additional production-queue feature expansion is recommended before that work.
