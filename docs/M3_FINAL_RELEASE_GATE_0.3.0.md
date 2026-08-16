# AI Studio 0.3.0 Final Release Gate

Date: 2026-08-16

## 1. Release Candidate

- Branch: `master`
- Source RC SHA evaluated by this Gate: **ASSIGNED BY THE BUILD-PREP EVIDENCE COMMIT**
- Source commit: `docs: reconcile 0.3.0 live release evidence` (pending commit)
- App version: `0.3.0`
- Embedded build commit: **NOT BUILT YET**. `src-tauri/build.rs` resolves the real Git HEAD when a build is run.
- Working tree at Gate start: clean and aligned with `origin/master`
- This is the build-preparation evidence state. The final evidence commit will record the actual Source RC SHA, installer identity and artifact hashes after the build.

## 2. Scope Freeze

The frozen 0.3.0 product scope remains Krea2 image production and MiniMax H3 video production. No new runtime, migration, package content, executor, queue architecture or UI feature was added in this Gate.

- Krea2: single and batch image generation with Asset recovery
- H3: FAST, T2V, I2V, First + Last, REF2VA, QUALITY 20-step, MP4 recovery and native playback
- Infrastructure: Snapshot, Task History, Production Queue, Runtime Provenance, migration, Backup/Restore and Workflow Benchmark foundations

## 3. Live Validation

- DEV-016C: **PASS — USER-VERIFIED LIVE PASS**. Product owner manual live acceptance covered three reference images, FAST, 5 seconds, 864×480, fixed seed, A→B→C order, ComfyUI execution, Task success, MP4/Asset recovery, Restart, Task History and Load to Studio. Machine-readable Task identifier: **NOT RECORDED**.
- DEV-016D: **PASS** — real QUALITY first/last task `tsk_496106ce-44f3-4d74-8395-6deb6bb3ee40`, ComfyUI prompt ID, 20 progress updates, output MP4 and Runtime Provenance are recorded in `docs/M3_LIVE_VALIDATION_0.3.0.md`.
- UI Smoke: **USER-VERIFIED PASS** — Workflow, Diagnostics, Task History, Asset Library, Production Queue, Workflow Benchmark and Project switching.
- Restart: **PASS** — database persistence and product owner manual live acceptance.
- Load to Studio: **PASS — USER-VERIFIED** after restart.

## 4. Automated Gate

- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test -- --test-threads=1`: PASS
- Rust test count: **422 passed / 0 failed**
- `pnpm test`: PASS
- Frontend test count: **46 files / 152 tests**
- `pnpm build`: PASS
- `git diff --check`: PASS before this documentation-only update

## 5. Migration Gate

- Fresh DB: PASS — temporary SQLite migration test reaches the complete schema and checks `PRAGMA foreign_keys = 1`.
- Upgrade DB: PASS — older 0.3.0-era rows survive the upgrade through migrations 011–015; Project, Task, Asset, Queue and compatibility rows remain readable.
- Latest migration: **015 `runtime provenance`**
- Current application DB read-only audit: migrations 001–015 are present and successful.

## 6. Backup Gate

- `BACKUP_VERSION`: **7**
- Export: PASS
- Import: PASS
- Compatibility: PASS for fixed v1, v2, v3, v4, v5 and v6 fixtures; current v7 round-trip, ID remap, asset bytes, relation preservation, Zip Slip rejection and rollback tests pass.

## 7. Runtime Package Integrity

- Immutable: PASS for same-version/same-SHA reuse and same-version/different-SHA rejection.
- Hash mismatch: PASS — `BUILTIN_PACKAGE_HASH_MISMATCH`, `WORKFLOW_RUNTIME_HASH_MISMATCH` and `RECIPE_RUNTIME_HASH_MISMATCH` protections remain present; no silent overwrite.
- Workflow conflict: PASS — `WORKFLOW_VERSION_CONFLICT`
- Recipe conflict: PASS — `RECIPE_VERSION_CONFLICT`
- Semver: PASS — numeric version ordering selects `1.10.0` over `1.9.0` independent of load order.
- Explicit repair quarantines mismatched builtin content before reinstalling the embedded package.

## 8. Installer Build

- Command: **NOT RUN** — DEV-017B requires all preceding Live/UI gates to PASS first.
- Target: NOT RECORDED
- Filename: NOT RECORDED
- Size: NOT RECORDED
- SHA256: NOT RECORDED
- Embedded commit: NOT RECORDED
- Previous local hashes were removed from `docs/RELEASE_SHA256_0.3.0.txt`; they are not valid for this Source RC SHA.

## 9. Clean Install Smoke

Not executed because no current installer was legally produced after the release prerequisites failed.

- install: NOT RECORDED
- first launch: NOT RECORDED
- fresh DB: NOT RECORDED
- workflow load: NOT RECORDED
- ComfyUI: NOT RECORDED
- Krea2: NOT RECORDED in a clean-install profile
- H3: NOT RECORDED in a clean-install profile
- Asset: NOT RECORDED
- playback: NOT RECORDED
- restart: NOT RECORDED

Existing live Task/Asset evidence is not relabeled as clean-install evidence.

## 10. Existing User Upgrade Smoke

Not executed against a copied old-user profile in this Gate. The temporary-database migration tests and existing read-only database audit pass, but they are not a substitute for installer-level upgrade smoke.

## 11. Known Issues

- BLOCKER: none from Live Validation, UI Smoke, Restart or Load to Studio; these are closed by product owner manual acceptance.
- PENDING: final production installer build, artifact hashes and installation smoke.
- NON-BLOCKER: Vite reports the existing large-chunk warning; build still succeeds.
- POST-0.3.0: final compiled workflow validator expansion and improved desktop WebView smoke observability.

No product code or published Runtime Package content was modified in this Gate. No tag, GitHub Release, installer upload or automatic publication was performed.

## 12. Final Decision

**PRE-BUILD EVIDENCE PASS — USER-VERIFIED LIVE GATES CLOSED; FINAL BUILD AND INSTALLATION SMOKE PENDING.**
