# AI Studio 0.3.0 Final Release Hardening

Audit date: 2026-08-16
Repository: `zhangcan001/AI-Studio`
Scope: final hardening only; no product feature expansion, tag, release or binary upload.

## 1. Git baseline

- Branch: `master`
- Audit start HEAD: `8632b44d63971ce5dff4e5c86a71b7ee7b4d1c2f`
- Audit start commit: `docs: record DEV-016C REF2VA live validation`
- Working tree at audit start: clean
- Remote: local `master` matched `origin/master` at audit start
- No `v0.3.0` tag was found on `origin`; `gh release view v0.3.0` returned `release not found`

## 2. Live validation status

- DEV-016C exact three-reference-image FAST case: **FAIL / NOT COMPLETED**. Current evidence is a QUALITY three-image run at 3 seconds and 960×544, not the required FAST 5-second 864×480 run. No exact replacement task exists in the current database.
- DEV-016C Case 2 (two images + audio): PASS, with real output and ordered compiled slots.
- DEV-016C Case 3 (video + two images): PASS, with real output, ordered compiled slots and native playback evidence.
- DEV-016D QUALITY 20-Step: PASS; the current database contains a real successful task, ComfyUI prompt ID, 20 progress updates, output MP4 and persisted provenance. Full details are in `docs/M3_LIVE_VALIDATION_0.3.0.md`.
- Restart persistence of Task/Snapshot/Asset rows: PASS by read-only database verification.
- Restart UI Task History and `Load to Studio` order: **NOT RECORDED**. The relaunch window exposed no usable WebView accessibility tree and screenshot capture failed, so this was not represented as PASS.

## 3. Automated gate

All commands below were run against the current source baseline; Cargo commands were run from `src-tauri/` and frontend commands from the repository root.

- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test -- --test-threads=1`: **422 passed, 0 failed**
- `pnpm test`: **46 files / 152 tests passed, 0 failed**
- `pnpm build`: PASS; Vite production build completed
- `git diff --check`: PASS before this documentation-only update
- Targeted hardening regressions: Builtin packages 6, Workflow Library 6, Task Recovery 16, Backup/Restore 26, Workflow Compiler 16, Production Queue 10, Workflow Lifecycle 5, Task History 7, Benchmark 10 — all passed

No product source code was changed during this audit.

## 4. Migration gate

- Fresh DB: PASS — `migration_runs_against_temporary_sqlite` created the schema and checked 26 tables, foreign keys and WAL.
- Existing 0.3.0-era DB upgrade: PASS — legacy rows were created through the older migration boundary and preserved through migrations 011–015; Project, Task, Asset, Production Queue, Snapshot-era rows and Shot compatibility rows remained readable.
- Latest migration: **015 `runtime provenance`**
- Current live DB: migrations 001–015 are present and successful, with no gaps.
- Foreign keys: PASS in the application pool and migration regression (`PRAGMA foreign_keys = 1`).
- No migration files were modified.

## 5. Backup / restore gate

- `BACKUP_VERSION`: **7**
- Export: PASS — current backup round-trip test writes a complete project archive.
- Restore: PASS — Project, Assets, Task/Snapshot relationships, queue references and asset bytes are restored with remapped IDs.
- Asset video prompts, reviews, benchmark metadata and runtime provenance are covered by the current backup schema/tests.
- Old backup compatibility: PASS — fixed v1, v2, v3, v4, v5 and v6 fixtures restore; traversal/Zip Slip and rollback tests pass.

## 6. Runtime package integrity

- Immutable versions: PASS — same version plus same SHA reuses; same version plus different SHA returns a conflict.
- Builtin mismatch: PASS — `BUILTIN_PACKAGE_HASH_MISMATCH` is reported and existing user data is not overwritten.
- Repair: PASS — repair explicitly quarantines the mismatched directory before installing the embedded package.
- Workflow conflict: PASS — `WORKFLOW_VERSION_CONFLICT` remains enforced.
- Recipe conflict: PASS — `RECIPE_VERSION_CONFLICT` remains enforced.
- Semver selection: PASS — numeric comparison selects `1.10.0` over `1.9.0`, independent of load order.
- Workflow Library sync path: PASS — batch capability checks accept `workflow + recipe`, and current-version selection is semver-aware.

## 7. Runtime provenance

- Persisted: PASS — new Task rows contain app version, build commit, workflow/recipe IDs and versions, both SHA-256 values, package name/source and dynamic binding targets.
- Current live examples persisted build `d8dabc9a104f7b14cfd041cbc62c5cfde53678ac`, matching the audited application build.
- Restart: PASS for database readability; current successful Task/Snapshot/Asset rows remain present after relaunch.
- Task History: backend serialization and structured validation diagnostics tests PASS; **UI display smoke NOT RECORDED** because the WebView was not targetable in this run.
- `WORKFLOW_VALIDATION_FAILED` diagnostics include the actual runtime package, workflow/recipe hashes and dynamic targets through the persisted provenance payload.
- Old Task rows with null provenance are handled by the repository/history models without a required non-null migration.

## 8. Task recovery audit

- Startup ordering: PASS by source audit — active Task reconciliation runs before Production Queue recovery.
- Prompt identity: PASS — submission prompt ID is persisted in Task Events.
- Completed output recovery: PASS by automated recovery tests, including history success importing video/output mappings without duplicate assets.
- Duplicate `/prompt` on restart: PASS by recovery tests; restart-created and queued/running recovery paths do not resubmit.
- Normal relaunch database persistence: PASS.
- Forced-crash UI recovery and `Load to Studio` order: **NOT RECORDED**, not treated as a product PASS. No known backend recovery blocker was found.

## 9. Workflow compiler safety audit

- Internal placeholder invariant: PASS before the single ComfyUI submission path; both internal placeholder forms are rejected.
- Recipe-aware capability path: PASS — `check_runtime_workflows()` receives a `Recipe` per input and evaluates dynamic binding targets; static options remain strict.
- Missing node / incompatible input diagnostics: PASS by onboarding and generation tests.
- Binding/clear-target semantics: PASS by compiler tests, including optional media clearing and ordered multi-image binding.
- Single `/prompt` production path: confirmed in `GenerationService` through the Comfy adapter; no second production executor was found.
- No known blocking dangling-node, missing-required-output or clear-target defect was reproduced in the current audited graphs. A large validator refactor is not part of DEV-017.

## 10. Production queue safety

- Official path: `ProductionBatch → ProductionBatchItem → ProductionQueueService → GenerationService → Task → Snapshot → Asset` remains the only production chain.
- Duplicate queue: PASS by queue and recovery regression tests.
- Queue link failure / `FAILED_TO_QUEUE`: covered by the production queue and generation error paths.
- Cancellation: atomic repository transaction coverage is present for pending-item cancellation plus batch completion.
- Restart: recovery tests cover no duplicate submission and no state regression.
- Krea2/H3 automated regression: PASS in the full Rust/frontend gates; existing live Case 2/3 and QUALITY records remain successful.

## 11. UI / performance smoke

- Workflow page first open: **NOT RECORDED**
- Workflow fast list: **NOT RECORDED**
- Workflow diagnostics refresh: **NOT RECORDED**
- Task History: **NOT RECORDED**
- Asset Library: **NOT RECORDED**
- Production Queue: **NOT RECORDED**
- Workflow Benchmark: **NOT RECORDED**
- Project switching: **NOT RECORDED**

The desktop app was open, but its WebView exposed only generic window panes to the UI automation layer. Screenshot capture returned `SetIsBorderRequired failed (0x80004002)`. No page was claimed PASS from this incomplete observation.

## 12. Known non-blocking gaps

- UI smoke evidence is unavailable in this run because of the desktop WebView automation limitation. This is a validation/evidence gap, not a reproduced product defect; POST-0.3.0 follow-up is recommended.
- Backend crash-recovery tests pass, but an interactive forced-close plus `Load to Studio` proof remains unrecorded. POST-0.3.0 follow-up is recommended.
- DEV-016C exact FAST three-image live case is a required hardening gate and is therefore a release blocker for this audit, even though no new code defect was found.

## 13. Release blockers

1. **BLOCKER — DEV-016C exact live evidence missing:** no current Task records three reference images + FAST + 5 seconds + 864×480 + fixed seed + A→B→C ordering in one successful run.
2. **BLOCKER — restart UI acceptance evidence missing:** Task History and `Load to Studio` order could not be verified after relaunch.
3. **BLOCKER — required UI smoke evidence missing:** the requested Workflow, Diagnostics, Task History, Asset Library, Production Queue, Benchmark and Project switching checks were not observable in this run.

These are evidence blockers under the DEV-017 acceptance checklist. They are not being converted into fabricated PASS states or broad product changes.

## 14. Final decision

**0.3.0 RELEASE HARDENING FAIL — DEV-016C exact FAST three-reference-image live evidence and required post-restart/UI smoke evidence are not recorded.**

No tag, GitHub Release, installer upload or binary publication was performed.
