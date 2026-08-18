# DEV-042 Series Production Overview + Batch Runbook

## Baseline and frozen contracts

- Branch: `master`
- Baseline HEAD and `origin/master`: `02cb92634fe94cf76c86a50c05eca74213ff8f1a`
- DEV-041 start SHA: `69fa3c94df18c9e413b96c7d2156aaa9708028a6`
- Product version remains `0.6.0`.
- Migration remains `021`; `BACKUP_VERSION` remains `12`; Manifest remains `1`.
- Frozen tags were not changed: `v0.6.0 = e3d7181f23a9b7285a426efb20ead4db17198757`, `v0.5.0 = 02e67cff50f5da1d207478071636af166048820c`, and `v0.4.0 = 94918f6322ce690ff7b1630961abb56b8a31ed11`.

No migration, table, backup format, release asset, GPU run, ComfyUI live run, installer run, or `0.6.1` work was added.

## Reused architecture

Series is a read/orchestration projection over the existing `ProductionStructureService`, `EpisodeProductionService`, `SceneProductionService`, `ShotBulkService`, `PromptTemplateBulkService`, `BatchWorkflowPresetService`, and `ProductionQueueService`.

`SeriesProductionService` owns one structure-tree load per operation and calls Episode scope helpers. It has no dependency on Generation, Comfy, or `ProductionOrchestratorService`. A Scene remains one existing Production Batch. The existing queue admission and start path remain the only execution path.

`ProductionBatchRunbookService` is read-only and derived. Its SQLite adapter hydrates Shot-bound batch rows with one set-based JOIN across the existing batch, item, Shot link, Shot, and Scene assignment tables. Generic batches are not copied into the Series projection.

## Plan and prepare scope

`series_production_plan(projectId, seriesId, stage)` returns Series, Episode, Scene, and Shot totals plus Episode classifications `EMPTY`, `DONE`, `PREPARED`, `READY`, `PARTIAL`, and `BLOCKED`. Existing Episode/Scene classifications remain the source of truth.

`series_production_prepare` accepts optional Episode IDs and `allowPartial`. Empty selection means the whole Series. The selected scope is bounded at 20 Episodes, 100 Scenes, and 500 unique Shots. Duplicate or foreign Episode IDs fail with `SERIES_EPISODE_SELECTION_INVALID`; oversized scopes use the Series limit codes.

Strict mode plans every selected Episode before the first mutation. Any `BLOCKED` or `PARTIAL` Episode returns `SERIES_PRODUCTION_BLOCKED` with zero mutation. Partial mode skips `DONE`, `EMPTY`, `PREPARED`, and fully blocked Episodes, and delegates eligible Scene preparation to the existing Episode/Scene services. Results are `SUCCESS`, `NOOP`, `PARTIAL`, or `BLOCKED`; an error after earlier batches preserves those batches and returns the existing partial boundary.

The existing Scene prepare gate plus active-binding recheck keep Series+Episode, Series+Scene, and repeated prepare races idempotent. A repeat prepare creates no duplicate binding or Batch.

## Presets and prompts

The Series panel derives the selected Episode/Scene/Shot scope and reuses the existing bulk stage-config and prompt-template commands. Shots are deduplicated and ordered by global Shot ordinal. Applying a preset does not alter references, selected image/video assets, anchors, Scene assignment, or Shot ordinal. Prompt preview is capped at 20; the overall Series bulk scope remains capped at 500. Prompt context is resolved per Shot with Series → Episode → Scene → Shot context layers.

## Batch Runbook

The Runbook is a derived view over active batches and recent completed batches. It maps Batch → item → Shot binding → Scene assignment → Episode → Series and never parses a Batch name. A Batch spanning multiple Scenes or stages is surfaced with `RUNBOOK_MIXED_SCOPE`, warning state, and no guessed Scene. Generic unbound Batches remain visible only in the normal Production Queue.

Stable order is Series ordinal, Episode ordinal, Scene ordinal, IMAGE before VIDEO, Batch creation time, then Batch ID. Recommendation is UI-only: current RUNNING first, then the existing admission blocker or PAUSED Batch, then the first READY batch that can start. There is no polling, scheduler, Start All, Runbook executor, or Series queue.

The Runbook start button directly invokes the existing `production_queue_start(projectId, batchId)`. Queue admission, running-state, and error semantics remain in `ProductionQueueService`. ProductionRunPanel remains independent.

## Safety fixture and verification

The DEV-042 fixture models 1 Series, 5 Episodes, 50 Scenes, and 500 Shots (10 Scenes per Episode and 10 Shots per Scene). It covers strict zero-mutation blocking, partial preparation, repeat idempotency, Series/Episode/Scene races, preset reference/asset/anchor/assignment/ordinal preservation, four-layer prompt context, Runbook ordering/recommendation/mixed scope/generic exclusion, manual image/video review, and forbidden architecture names.

Targeted gates:

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- Series and Runbook Rust focused tests
- `cargo test --manifest-path src-tauri/Cargo.toml --test dev042_safety -- --test-threads=1`
- Series, Runbook, and stability Vitest files
- `pnpm build`

Final regression is the single required Rust/frontend suite from the DEV-042 handoff. No live or GPU validation is part of this task.

Final recorded results: Rust `628 passed / 0 failed / 2 ignored` across the library and integration suites; the two ignored cases are the pre-existing live ComfyUI gate and the pending DEV-041 integration gate. Frontend `71` test files passed with `259 passed / 1 todo`; `pnpm build` passed.

## Final decision

DEV-042 is complete when the targeted and final regression gates are green, the worktree is clean, and `master` is pushed to `origin/master`. The next task is `DEV-043` — Post-0.6.0 Series Execution Observability + Operator Controls; it is only recorded as the next task and is not started here.
