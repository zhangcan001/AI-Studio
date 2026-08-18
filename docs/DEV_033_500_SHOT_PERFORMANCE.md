# DEV-033 500-Shot Performance

## 1. Baseline

- `DEV033_START_SHA`: `7e5e11456881563ab66b2fe6c27ed02718d809e8`
- Branch: `master`, clean, version `0.5.0`.
- Migration remains `019`; `BACKUP_VERSION` remains `10`.
- Frozen tags were not modified.

## 2. Bottleneck

`ShotService::list()` returned `ShotView[]`, but the SQLite repository loaded stage configs, references, and generation links inside a per-shot loop. The service then queried stage prompts and tasks again while composing each view.

## 3. N+1 Before

The old repository path used `1 + 3N` related SQL calls. With the old service hydration, the stage-prompt fallback could expand to an estimated `1 + 5N + 3N²` fan-out for a 500-shot project.

## 4. Bulk Hydration

- SQLite shot list now loads shots, stage configs, references, generation links, and stage prompts with five project-level queries.
- `ShotService` keeps the `listShots() -> ShotView[]` contract and composes views in memory.
- Generation-link tasks use `TaskRepository::find_many_by_ids`; SQLite uses one `IN (...)` query instead of per-task `find_by_id` calls.
- `getShot()` keeps the single-shot path and existing CRUD semantics.

## 5. Search

`ShotWorkspace` filters in memory by case-insensitive shot name or prompt text. Filtering does not trigger another `listShots` request.

## 6. Status Filter

The toolbar reuses `deriveShotStatus` and the existing stage status derivation. It exposes all, configuration, image processing/review/selected, video processing/review, completed, and failed states.

## 7. Display Pagination

- Display-only pagination keeps the complete `shots[]` in memory.
- Page sizes are `25`, `50`, and `100`; default is `50`.
- Search, status, and page-size changes reset to page 1.
- The list renders at most 100 Shot rows. The selected Shot detail remains stable when it is outside the current filter.
- Move Up/Down is disabled while search or status filtering is active, preventing filtered-order corruption.

## 8. 500-Shot Benchmark

Isolated SQLite, no GPU, no media generation:

| Shots | Before ms | After ms | SQL calls before → after |
|---:|---:|---:|---:|
| 100 | 22.777 | 4.449 | 301 → 5 |
| 250 | 57.141 | 9.426 | 751 → 5 |
| 500 | 110.635 | 17.090 | 1501 → 5 |

The 500-shot list returned correctly, Shot 1/250/500 data was verified, 50 task-link fixtures passed, and closing/reopening the SQLite database returned 500 shots.

## 9. Pipeline Compatibility

- `ProjectProductionPipeline` still receives all project shots; a filtered list result does not replace its input.
- Pipeline summary remains O(N) and reports `total = 500` for a 500-shot project.
- Bulk Config, Review, Bulk Import (max 500), Partial Resume, and Reorder retain their existing full-project semantics.
- Filtered reorder is disabled rather than applying a partial ordered ID list.

## 10. Regression

- Targeted backend, frontend, TypeScript, benchmark, restart, and repository checks: PASS.
- `cargo fmt --all -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`: `503 passed / 0 failed / 1 ignored`.
- `pnpm test`: `55 files / 180 tests passed`.
- `pnpm build`: PASS; existing large-chunk warning only.
- `git diff --check`: PASS; only Windows line-ending warnings.

## 11. Migration

No migration 020 was needed. Existing migration `019` is unchanged; the measured bottleneck was eliminated by bulk reads.

## 12. Architecture

No second Shot system, status engine, queue, executor, progress table, direct `/prompt` path, cursor API, FTS, ShotSummary/ShotDetail refactor, backup redesign, GPU run, or installer work was added.

## 13. Final Decision

500-shot import/list, bulk hydration, search, status filtering, display pagination, restart, and compatibility gates pass. Stop at the requested scope.

`DEV-033 500-SHOT PERFORMANCE + SEARCH PASS`
