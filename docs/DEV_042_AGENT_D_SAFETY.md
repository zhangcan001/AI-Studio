# DEV-042 Agent D — No-GPU Safety / Runbook Contract

Status: `DONE`

This handoff adds only deterministic Agent D safety coverage. It does not
change production code, add a migration, start a queue, call ComfyUI, call
`/prompt`, or execute a GPU task.

## Files

- `src-tauri/tests/dev042_safety.rs`
- `src/features/stability/dev042Stability.test.ts`
- `docs/DEV_042_AGENT_D_SAFETY.md`

## Fixture

The Rust and frontend fixtures independently construct the same shape:

- 1 Series (`series-01`)
- 5 Episodes
- 10 Scenes per Episode, 50 Scenes total
- 10 Shots per Scene, 500 Shots total
- image-plan totals: `DONE=100`, `PREPARED=100`, `ELIGIBLE=180`, `BLOCKED=120`
- Episode classifications: `DONE`, `READY`, `PREPARED`, `PARTIAL`, `BLOCKED`

The Series plan derives Episode summaries from one in-memory tree and asserts
`treeLoads = 1`; no per-Episode tree query or 500-shot query loop is used.

## Prepare safety

The selected scope is Episode 2 + Episode 4:

- strict mode rejects the `PARTIAL` Episode 4 preflight with zero mutations;
- partial mode prepares 18 scene batches and 180 eligible items, while two
  blocked scenes are skipped;
- the repeat call is `NOOP` with zero new bindings;
- no test calls `production_queue_start`, `Start All`, a scheduler, or a
  generation service.

The race fixture represents Series, Episode, and Scene contenders for the same
scope. Three concurrent admission attempts leave exactly 180 unique active
Shot/Stage keys; the test does not create a second mutex-backed production
state machine.

## Manual review gates

An image task that succeeds without a selected image remains blocked with
`IMAGE_REVIEW_REQUIRED`. A selected image is required before a video item is
eligible. A succeeded video task does not populate `selectedVideo`; Video
Review remains manual.

## Production Batch Runbook

The no-GPU Runbook fixture verifies:

- Batch → Scene mapping and hierarchy ordering by Episode ordinal, Scene
  ordinal, then Stage;
- generic batches are excluded;
- a batch with multiple Scene bindings is retained with a `MIXED_SCOPE`
  warning and does not panic;
- a `RUNNING` batch is recommended first; after it completes, the first
  `READY` batch is recommended;
- admission is single-batch only. Other rows are not implicitly started.

## Architecture audit

The Rust source audit checks any DEV-042 Series/Runbook service files when
present for forbidden `SeriesQueue`, `SeriesExecutor`, scheduler/auto-runner,
direct `GenerationService`, direct `ComfyHttpAdapter`, direct `/prompt`, and
new Runbook start commands. It also checks that the existing
`ProductionRunPanel` remains independent and that the existing
`production_queue_start` boundary remains available.

The one `#[ignore]` Rust test is intentional and clearly labeled: it is a
post-merge integration contract for the production files owned by Agents A-C.
It is not a product failure and performs no runtime work.

## Validation

Run the focused gates from the repository root:

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --test dev042_safety -- --test-threads=1
pnpm exec vitest run src/features/stability/dev042Stability.test.ts
```

Expected Agent D result after the three files are added:

- Rust: 9 passed, 0 failed, 1 intentional ignored;
- Frontend: 8 passed, 0 failed;
- GPU / ComfyUI / live HTTP: not run.

## Scope boundary

Agent D does not implement or edit SeriesProductionService,
ProductionBatchRunbookService, Series UI, Runbook UI, commands, migrations,
backup format, queue/executor code, or the existing Production Run system.
