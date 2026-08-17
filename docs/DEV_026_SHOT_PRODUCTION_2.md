# DEV-026 — Shot Production 2.0 Validation Record

Date: 2026-08-17
Baseline: `master` at `a74a303b236914abb9285eda6690e3517f955a82`
Release boundary: existing `v0.4.0` tag was not changed. Its peeled commit is `94918f6322ce690ff7b1630961abb56b8a31ed11`.

## Parallel Execution

Implementation and review were split across the existing backend, repository, and frontend areas. Agents A/B/C completed their scoped work; the architecture/backup audit was read-only. The shared worktree was integrated without unrelated changes or merge/reset operations.

## Existing Architecture Reuse

- Shot generation continues through `ShotService` / `ShotBatchService` → `ProductionQueueService` → `GenerationService` → `ComfyHttpAdapter`.
- No direct Comfy `/prompt` call, second executor, second queue, or alternate scheduler was added.
- Existing task lifecycle, queue recovery, strict serial H3 gate, asset store, runtime package, and project backup paths remain the source of truth.
- The real-run harness used an isolated data root and invoked the formal application chain; it did not drive the desktop UI or simulate key presses.

## Video Modes

- I2V keeps the singular `Image` input contract and requires the selected keyframe.
- Exact H3 REF2VA runtime IDs use plural `ImageAssets` and an ordered reference manifest; a selected keyframe is optional.
- REF2VA effective minimum is `max(recipe.min_items, 2)`, so a stale/zero package minimum cannot permit a one-image shot. Recipe maximum remains enforced.
- Invalid input shape, duplicate IDs, project/type mismatch, and min/max violations fail before dispatch.

## Ordered References

Reference assets are ordered by persisted `ordinal ASC`, with asset ID as a deterministic tie-breaker. Duplicate IDs are rejected. The formal manifest is passed into generation evidence, so the order is not merely a frontend display convention.

## Batch Freeze

Batch creation freezes the complete per-shot values: workflow/recipe identity, mode, seed, selected image where applicable, and the ordered REF2VA `ImageAssets` list. Later edits to the Shot do not mutate an existing batch.

## Retry

Retry is bound to the source batch item. The repository copies the source batch, workflow, recipe, frozen values, and Shot identity into the retry row and rejects cross-batch binding. A retry cannot replace the original frozen reference order with caller-supplied values.

## Restart

The no-GPU lifecycle test closes and reopens the SQLite repository, then verifies the original batch, edited/new batch, and retry all retain their persisted identities, modes, and reference order.

## No-GPU E2E

PASS: `no_gpu_three_shot_video_batch_freezes_modes_order_retry_and_restart`.

The scenario covers three shots: one I2V shot and two REF2VA shots. It verifies B/A/C freeze, a later C/B/A edit only affects the new batch, retry reuse of the old frozen B/A/C values, and persistence across restart.

## Real Live

RESULT: **BLOCKED / FAIL at the environment gate; no video output claimed.**

The isolated backend harness reached the formal Shot batch → production queue → generation service → Comfy HTTP path, but workflow validation stopped before model execution:

```text
WORKFLOW_VALIDATION_FAILED
Node 'NBH3HyperStepSimple' not found. The custom node may not be installed.
```

The local ComfyUI health endpoint was available (`0.31.1`, Python `3.12.10`, PyTorch `2.10.0+cu130`, RTX 5060 Ti/CUDA). However, `/object_info` exposes the H3 reference/image workflow nodes but not `NBH3HyperStepSimple`; the configured H3 model root also does not exist locally and the installed model directories contain no required H3 diffusion/text/VAE assets. The harness therefore produced no snapshot or video and correctly stopped at the validation boundary.

## Order Integrity

PASS in no-GPU and repository coverage: B/A/C remains B/A/C in the old frozen batch, in its retry, and after restart; a later Shot edit to C/B/A appears only in the newly created batch.

## Regression

- Rust formatting and compile check: PASS (`cargo fmt --all`, `cargo check`).
- Full Rust regression: PASS, `477 passed; 0 failed`.
- Focused REF2VA manifest tests: PASS, 4 tests.
- Focused backup/service coverage: PASS, 26 tests.
- Frontend typecheck: PASS.
- Frontend targeted tests: PASS, 8 tests.
- Frontend full test suite: PASS, 160 tests across 49 files.
- Production frontend build: PASS (`tsc && vite build`). Vite reports only the existing-style non-blocking large-chunk warning.
- `git diff --check`: PASS; only Windows line-ending normalization warnings were reported.

## Database

No migration 019 was added. The latest migration remains `018_production_orchestrator.sql`. Retry binding and frozen values reuse the existing production queue schema.

## Backup

Backup version remains `9`. Existing project backup round-trip coverage passes, including the persisted Shot/reference relations and their ordered representation. No backup format bump was needed.

## Frozen Boundaries

- Existing runtime package bytes were not modified.
- Existing `v0.4.0` tag was not recreated, moved, or force-updated.
- No force push, `reset --hard`, or `clean -fd` was used.
- No UI automation was used.

## Known Issues

The local ComfyUI installation is not a valid live H3 REF2VA execution environment: the required custom node `NBH3HyperStepSimple` and H3 model root are absent. This is an external runtime capability/model issue, not a no-GPU application regression. Installing or replacing that environment was intentionally left outside this code task.

## Final Decision

**DEV-026 SHOT PRODUCTION 2.0: FAIL for the required real-live gate; implementation, frozen multi-shot behavior, retry/restart persistence, backup coverage, and all available regression checks PASS.**

## Next Step

Restore a compatible ComfyUI custom-node set and H3 model root, then rerun only the isolated real Shot REF2VA harness. Do not claim DEV-026 PASS or begin DEV-027 until that live gate produces a validated snapshot/video through the formal backend path.
