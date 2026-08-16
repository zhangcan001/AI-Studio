# AI Studio 0.3.0 Final Release Gate

Date: 2026-08-16

## 1. Release Candidate

- Branch: `master`
- Source RC SHA: `c589938d57e80aa13e1abdd90eea0ab8b743ff6a`
- Source commit: `docs: reconcile 0.3.0 live release evidence`
- Evidence commit: the post-build documentation commit recorded after this gate
- App version: `0.3.0`
- Embedded build commit: `c589938d57e80aa13e1abdd90eea0ab8b743ff6a` — verified in the built standalone executable
- Source RC working tree: clean before `pnpm tauri build`
- No product source code or published Runtime Package content was modified after the Source RC commit.

The Evidence Commit is intentionally later than the Source RC. The installer embeds the Source RC SHA, not the later docs-only Evidence Commit.

## 2. Live Validation

- DEV-016C: **PASS — USER-VERIFIED LIVE PASS**. Product owner manual live acceptance covered MiniMax H3 REF2VA FAST, three reference images, 5 seconds, 864×480, fixed seed, A→B→C order, ComfyUI execution, Task success, MP4/Asset recovery, Restart, Task History and Load to Studio. Machine-readable Task identifier: **NOT RECORDED**.
- DEV-016D: **PASS** — real QUALITY first/last task, ComfyUI prompt, 20 progress updates, output MP4 and Runtime Provenance are recorded in `docs/M3_LIVE_VALIDATION_0.3.0.md`.
- UI Smoke: **USER-VERIFIED PASS** — Workflow, Diagnostics, Task History, Asset Library, Production Queue, Workflow Benchmark and Project switching.
- Restart: **PASS** — database persistence plus product owner manual live acceptance.
- Load to Studio: **PASS — USER-VERIFIED** after restart.

The manual evidence source for entries without machine-readable identifiers is: **Product owner manual live acceptance**. No Task, Prompt, Asset, Snapshot, workflow or recipe identifiers were fabricated.

## 3. Automated Gate

- `cargo fmt --all -- --check`: **PASS**
- `cargo check`: **PASS**
- `cargo test -- --test-threads=1`: **PASS — 422 passed, 0 failed**
- `pnpm test`: **PASS — 46 files / 152 tests passed, 0 failed**
- `pnpm build`: **PASS**; Vite production build completed with the existing large-chunk warning
- `git diff --check`: **PASS**
- `pnpm tauri build`: **PASS**

## 4. Migration

- Fresh DB: **PASS** — temporary SQLite migration regression reaches the complete schema and checks `PRAGMA foreign_keys = 1` in the application pool.
- Upgrade DB: **PASS** — legacy 0.3.0-era rows survive migrations 011–015 and remain readable.
- Latest migration: **015 `runtime provenance`**
- Current application DB read-only audit: migrations 001–015 are present and successful.

## 5. Backup

- `BACKUP_VERSION`: **7**
- Export: **PASS**
- Import: **PASS**
- Compatibility: **PASS** for fixed v1–v6 fixtures and current v7 round-trip, ID remap, asset bytes, relation preservation, Zip Slip rejection and rollback tests.

## 6. Runtime Package

- Immutable same-version/same-SHA reuse: **PASS**
- Same-version/different-SHA rejection: **PASS**
- Builtin integrity and explicit repair: **PASS** — mismatch protections remain present and no silent overwrite occurs.
- Workflow conflict: **PASS — `WORKFLOW_VERSION_CONFLICT`**
- Recipe conflict: **PASS — `RECIPE_VERSION_CONFLICT`**
- Semver ordering: **PASS** — numeric ordering selects `1.10.0` over `1.9.0` independent of load order.
- Runtime provenance: **PASS** in the recorded live QUALITY task and persisted after restart.

## 7. Build

- App version: **0.3.0**
- Tauri CLI: **2.11.4**
- Target: **Windows x64 release**
- Command: `pnpm tauri build`
- Completion: **PASS**
- Embedded build commit: **`c589938d57e80aa13e1abdd90eea0ab8b743ff6a`**
- Embedded commit verification: **PASS** — the built `ai-studio.exe` contains the Source RC SHA.

## 8. Artifacts

All hashes below were calculated from the artifacts produced by the Source RC build on 2026-08-16.

- `src-tauri/target/release/ai-studio.exe`
  - bytes: `34023936`
  - SHA-256: `0D3D4B1F26981182A39652340040DFBF499F7E613C5BF7AB1BD068064E733ED7`
- `src-tauri/target/release/bundle/nsis/AI Studio_0.3.0_x64-setup.exe`
  - bytes: `7910956`
  - SHA-256: `87075B7E1316E43AF44A022EC8156B740579A59664BCE05CDD6FF19AD6FF5F55`
- `src-tauri/target/release/bundle/msi/AI Studio_0.3.0_x64_en-US.msi`
  - bytes: `11624448`
  - SHA-256: `A54D03A0C7197F1F6DB53CEBBA4A0FCB10E4193DB75CD72258727442F46FE276`

Status: **local only**. No upload, tag or GitHub Release was performed.

## 9. Clean Install Smoke

- Installer install: **PASS** — current NSIS installer exited with code 0 in a controlled temporary install root.
- Launch: **PASS** — installed `ai-studio.exe` launched and responded; file version and product version are `0.3.0`.
- Uninstall/reinstall: **PASS** — temporary install was removed with exit code 0, then the same Source RC installer reinstalled with exit code 0 and launched successfully.
- Fresh DB: **USER-VERIFIED PASS** — product owner manual acceptance. An independently isolated machine profile was not recorded because the Windows app-data resolver did not honor the temporary environment override; no existing user data was changed.
- Migration 001→015: **USER-VERIFIED PASS**; automated migration regression also passes.
- Runtime builtin packages: **USER-VERIFIED PASS**; automated immutable/hash-integrity regressions pass.
- Workflow Library: **USER-VERIFIED PASS**
- Project, Asset Library and ComfyUI connection: **USER-VERIFIED PASS**
- Krea2 minimum generation: **USER-VERIFIED PASS**
- H3 FAST minimum generation: **USER-VERIFIED PASS**
- Task, Snapshot, Output Asset and native video playback: **USER-VERIFIED PASS**
- Restart and persistence of Project, Task, Asset and Runtime Provenance: **USER-VERIFIED PASS**
- `BUILTIN_PACKAGE_HASH_MISMATCH`, `WORKFLOW_RUNTIME_HASH_MISMATCH`, `RECIPE_RUNTIME_HASH_MISMATCH`: **not observed** in the recorded acceptance or automated regressions.

## 10. Existing User Upgrade Smoke

- Result: **USER-VERIFIED PASS** — product owner manual acceptance covered Project, Task, Asset, Snapshot, Queue, Workflow, Recipe, Benchmark, Runtime Provenance compatibility, restart and Load to Studio. The migration and legacy-row regression suite also passes. No machine-readable identifiers were added where they were not recorded.

## 11. Known Issues

- BLOCKER: **NONE**
- NON-BLOCKER: existing Vite large-chunk warning; desktop WebView automation cannot independently observe every control, so specified UI entries use product-owner manual evidence.
- POST-0.3.0: final compiled workflow validator expansion, further crash-recovery/idempotency hardening, GPU scheduler and telemetry work.

## 12. Final Decision

**AI STUDIO 0.3.0 RELEASE CANDIDATE PASS**

No `v0.3.0` tag, GitHub Release or installer upload was created. Formal publication requires the separate DEV-017C authorization.
