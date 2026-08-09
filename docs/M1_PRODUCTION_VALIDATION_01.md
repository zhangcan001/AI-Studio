# M1 Production Validation 01

Date: 2026-08-09

Scope: Kera2 persistent production queue live acceptance on the existing Windows desktop stack. MiniMax H3 OOM remediation is explicitly excluded from this gate.

## Final status

- `PRODUCTION VALIDATION 01 = PASS`
- `BATCH FOUNDATION 01 = PASS`
- `BATCH FOUNDATION 02 = PASS`
- `BATCH FOUNDATION 03 = PASS`
- `BATCH FOUNDATION 04 = PASS`
- `MINIMAX H3 16GB RUNTIME = ENVIRONMENT BLOCKED` (unchanged; not addressed here)

## Windows command environment

The earlier WSL command-channel blocker is no longer relevant to project validation. All required commands ran directly through native Windows PowerShell with the installed Rust and Node/pnpm toolchains.

Two pre-existing validation blockers were corrected before the gate:

- the production queue React ref now has an explicit `undefined` initial value compatible with the installed React type definitions
- newly added Rust queue files were normalized with the repository rustfmt configuration

No queue behavior or database migration was redesigned during this correction.

## Complete regression

- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test -- --test-threads=1`: PASS — 238 passed, 0 failed
- `pnpm test`: PASS — 13 files, 31 tests passed
- `pnpm build`: PASS
- `git diff --check`: PASS
- Tauri desktop startup and migrations 006/007: PASS

## Runtime environment

- Endpoint: `http://127.0.0.1:8188`
- ComfyUI: `0.30.2`
- GPU: NVIDIA GeForce RTX 5060 Ti
- VRAM: 15.9 GB total
- Capability node count: 4,485
- Workflow: Kera2 T2I Local v2
- Project: Default Project

No model, workflow, sampler, or ComfyUI installation content was modified.

## Real four-item queue

Production batch `pbt_bee50a8b112d48abbd20d3560e746dac` contained four distinct Kera2 items with fixed seeds and normal project-scoped generation values.

Result:

- Batch: `COMPLETED`
- Items: 4
- Succeeded: 4
- Failed/Cancelled/Skipped: 0
- Tasks created: exactly 4
- Dispatch order: ordinal 0, 1, 2, 3
- Strict sequential check: PASS; every next Task was created after the previous Task finished
- Duplicate Task after restart: none

## Pause and resume

The batch was paused while ordinal 0 was dispatched. The active Task completed normally, the batch remained `PAUSED`, and ordinals 1–3 remained `PENDING`. No later item was dispatched while paused.

Resume changed the persisted batch back to `RUNNING` and dispatched ordinal 1 next. Pause does not cancel an already dispatched Task, matching the documented contract.

## Desktop restart recovery

AI Studio was closed while the production batch remained persisted as `RUNNING`, with later items still pending. After the real desktop process restarted:

- startup Task/queue reconciliation completed
- the queue remained the same batch rather than creating a replacement
- remaining ordinals resumed automatically
- ordinals 2 and 3 completed in order
- no duplicate Task IDs or duplicate output mappings were created

## Asset Library

Each of the four production Tasks produced one project-scoped Asset:

- type/category: `image` / `generated_image`
- MIME: `image/png`
- dimensions: 768 × 1280
- file count: 4
- database Asset rows: 4
- task-output mappings: 4
- files present with matching persisted size: 4
- SHA-256 present: 4
- Asset Library command results: PASS
- Studio Asset Library UI and previews: PASS
- visual inspection: all four images are valid and correspond to their distinct production items

## Archive, restore, and delete

A separate unused control queue exercised lifecycle operations without deleting Task or Asset evidence from the real production batch:

- Archive set `archivedAt`: PASS
- Restore cleared `archivedAt`: PASS
- Delete while unarchived was rejected by the safety policy: PASS
- Archive followed by Delete removed only the control queue metadata/items: PASS
- The completed production Tasks and Assets remained intact

## Failure, skip, and requeue

ComfyUI was intentionally stopped to produce real transient failures through the normal generation pipeline.

Requeue gate:

- original Task: `FAILED`
- original error: `COMFY_STREAM_DISCONNECTED`
- original item remained unchanged with its Task/error evidence
- Requeue appended one new `PENDING` item with `retryOfItemId`
- after ComfyUI restarted, explicit Resume dispatched the retry
- retry Task: `SUCCEEDED`
- retry output Asset exists and is visible in the Asset Library
- batch: `COMPLETED` with one preserved failed item and one successful retry item

Skip gate:

- original Task: `FAILED` with `COMFY_STREAM_DISCONNECTED`
- Skip changed only the queue item to `SKIPPED`
- original Task ID and error evidence were preserved
- no output Asset was fabricated for the failed/skipped Task
- explicit Resume finalized the skipped-only queue as `COMPLETED`

`EXECUTION_ERROR`, including the known MiniMax H3 OOM class, remains excluded from safe requeue.

## UI bridge and observability

The live WebView/Tauri boundary reported the expected production overview and the Studio UI displayed all three completed validation queues. The Asset workspace displayed the new Kera2 outputs with dimensions, sizes, timestamps, and working previews. No raw storage path was returned to React.

## Next stage

Stop adding production queue features. The next task is only the dedicated MiniMax H3 OOM unblock and a real 16 GB-compatible video completion gate.
