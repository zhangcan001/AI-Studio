# M3 PRODUCTION UX PACK 08 = CODE PASS / LIVE PENDING

Date: 2026-08-10
Development line: `0.3.0`
Release status: development only; no `v0.3.0` tag or GitHub Release.

Pack 08 completes the first production-UX productization pass on the existing Studio / Production Queue / Task path. It does not introduce a second executor, a second Task model, direct React-side `/prompt` submission, or migration `010`.

## Delivered

- Production status is derived from real `ProductionBatchSummary` and `ProductionQueueOverview` records. The Creation Dashboard is project-scoped and shows queue/task counts, recent queues, recent successful workflows, and recent prompts without inventing DRAFT/SUCCEEDED state.
- Recent queue actions map to real operations: READY → start, RUNNING → pause, PAUSED → continue, and completed/archived → view. Unavailable workflow versions are shown as unavailable rather than silently substituted.
- Successful production results expose the existing safe actions: open task/result, reuse in Studio, and add output assets to the existing compare workspace. Reuse and prompt application never auto-submit a generation task.
- Prompt Library listing now uses `(updated_at, id)` keyset pagination with project/kind/keyword/tag filters, cursor continuation, stable ordering, and a 300 ms frontend filter debounce. Page size is bounded to 30–100.
- Asset organization has project-scoped transactional bulk favorite/tag operations, a maximum of 100 selected assets, cross-project rejection, and an independent selection mode that does not alter compare selection.
- Production queue name presets are persisted in `settings.json`, validated and deduplicated, and exposed in the queue creation panel without adding task data or a database migration.
- Pack 09 is now implemented as a project-scoped Shot production workspace on the existing GenerationService / Task / Snapshot / Asset chain.

## Schema and upgrade boundary

- Migrations `001–009` remain immutable. Pack 09 adds immutable-after-apply `010_shot_production.sql` for Shot orchestration metadata; it does not alter `tasks` or add a second Task system.
- The existing local database at `%LOCALAPPDATA%\AIStudio\AIStudioData\app.db` previously passed the migration 8 → 9 smoke gate; Pack 09 adds the 9 → 10 upgrade gate.
- Queue-name presets remain settings data. The active production runtime scope is Kera2 image keyframes + MiniMax H3 reference-image-to-video.

## Verification

- Rust: `cargo test -- --test-threads=1` → 304 passed, 0 failed.
- Frontend: `pnpm test -- --reporter=dot` → 31 test files, 96 passed, 0 failed.
- Build checks: `cargo check`, `cargo fmt --all -- --check`, `pnpm build`, and the Tauri MSI/NSIS packaging checks pass for the current source line.
- Existing-database upgrade smoke: pass, migration 8 → 9.
- Comfy health probe: reachable at `127.0.0.1:8188`, ComfyUI `0.30.2`, RTX 5060 Ti detected, approximately 1.87 GiB free VRAM at audit time.

## Live gate status

The Pack 06 Kera2 four-item gate and Pack 07 five-action desktop gate remain `LIVE PENDING`. This audit did not have an observable, controllable desktop UI state, and the available GPU memory was low; therefore no batch/item/task/asset/snapshot evidence is claimed. The source and automated regression are code-pass, while GPU generation and the full desktop interaction chain require a later observable run.

Pack 06 and Pack 07 closeout documents remain explicitly at `CODE PASS / LIVE PENDING`. Pack 09 is documented in `docs/M3_SHOT_PRODUCTION_PACK_09.md`.
