# M1 Batch Foundation Validation

Date: 2026-08-07

Final status: PASS (validated 2026-08-09).

Scope: first batch-production foundation for the intentionally limited production model set: Kera2 image generation and MiniMax H3 video generation. No third model runtime pack, scheduler, CSV import, or automatic retry policy is included.

## Implemented

- Added `generation_create_batch` as a separate command; existing `generation_create` remains unchanged.
- Batch request is project-scoped and limited to 1..100 items.
- Each item retains its own workflow version, recipe, values snapshot, Task ID, Task lifecycle, cancellation/recovery path, and output Asset mapping.
- Batch creation is partial-success: one item failing validation/definition lookup does not remove Tasks already created for other items.
- Batch response preserves input order through per-item indexes and returns safe `code` + `message` failures.
- React Studio can freeze the current Kera2 or MiniMax H3 form values into a local batch draft, remove items, clear the draft, and submit the batch.
- Frozen batch items are copied values; later changes to the current Studio form do not mutate items already added to the batch.
- Project switching clears the local batch draft.
- A generic `Semaphore(1)` submission gate serializes the ComfyUI subscription + `/prompt` submission boundary so a large batch does not concurrently flood the local ComfyUI endpoint. The permit is released after the Task reaches the submitted/queued boundary; execution, cancellation, history tracking, and ComfyUI queue behavior remain on the existing pipeline.
- MiniMax H3 OOM behavior is not retried or hidden by the batch layer. An H3 Task can still fail independently while Kera2 or other valid items in the same batch continue through their own lifecycle.

## Explicitly not included

- CSV / Excel / JSON task-list import
- Batch persistence as a new database entity
- Priority scheduling
- Cron / timed automation
- Automatic retry
- OOM retry
- Model downloading or custom-node installation
- Wan / Flux / Qwen / third-model production support

## Validation status

Status: PASS.

The previous WSL/tooling blocker was removed by running the project toolchain directly in native Windows PowerShell. The complete regression suite passed:

- `cargo fmt --all -- --check`
- `cargo check`
- `cargo test -- --test-threads=1` — 238 passed, 0 failed
- `pnpm test` — 13 files, 31 tests passed
- `pnpm build`
- `git diff --check`

Production Validation 01 also exercised a real four-item Kera2 production batch through normal Tasks and confirmed ordered output Asset creation. Full evidence is recorded in `M1_PRODUCTION_VALIDATION_01.md`.

## Next stage

Batch Foundations 02–04 and Production Validation 01 are complete. The next work is limited to the separate MiniMax H3 16 GB OOM unblock; OOM retry remains non-automatic.
