# DEV-025 — REF2VA Production Run Validation

Date: 2026-08-17

## Baseline

- Branch: `master`
- Pre-change HEAD: `f495d34e66f7bd5943f23635792ac6238bfcb7b0`
- `v0.4.0` remains unchanged; no new tag, release, installer, or Runtime Package byte was created.
- The DEV-025 worktree changes were the intended continuation from the previous handoff.

## Parallel Execution

The implementation was split into disjoint ownership areas: backend orchestration and queue semantics, the Production Run UI, and tests/data-root architecture audit. Integration, regression, live validation, documentation, commit, and push remained with the main task.

## Architecture

The production path is now explicit:

`run_images → ordered select_assets → frozen REF2VA recipe → run_video → same-batch retry → latest-attempt stage statistics`

`reference_index` is persisted on every selected reference StageItem. Selection order is preserved and validated as contiguous, unique, and recipe-bounded. REF2VA accepts 2–9 references; non-REF2VA I2V remains single-reference.

## Backend

- REF2VA workflow and recipe matching is strict; unsupported mode/recipe combinations fail closed.
- Ordered reference bindings are injected into the shipped workflow in selection order.
- Duplicate asset IDs are rejected instead of silently deduplicated.
- Retry creates a new attempt in the original ProductionBatch and preserves `retry_of_item_id`, `parent_stage_item_id`, frozen values, source assets, and `reference_index`.
- Stage statistics select the latest attempt for each ordered reference, so an old failed attempt does not poison a successful retry.
- Idempotent queue submission and existing task/asset lineage behavior remain intact.

## UI

- Production Run freezes mode, recipe, values, and selected references once selection/video work has started.
- Reordering/removal/card controls are disabled while frozen.
- REF2VA minimum and recipe maximum are enforced in the UI and backend.
- The frontend suite includes duplicate rejection and ordered-selection coverage.

## No-GPU E2E

PASS. The deterministic lifecycle test executes the real orchestrator path:

`run_images → three candidate outputs → manual B/A/C selection → run_video(REF2VA) → forced failure → retry_video in the same batch → three successful latest attempts`

The test verifies `reference_index` `0/1/2`, B/A/C source order, attempt `2`, retry lineage, and final batch status.

## Live

ComfyUI live smoke: PASS.

- Endpoint: `http://127.0.0.1:8188`
- ComfyUI: `0.33.0`
- PyTorch: `2.9.0+cu130`
- Device: `cuda:0 NVIDIA GeForce RTX 5060 Ti` (16 GB class)
- Health: `/system_stats` 200; `/object_info` 200
- Package: `minimax_h3_reference_video_quality_2_0_0`
- Reference order: B → A → C
- Asset IDs: `ast_d9415383-f501-40b7-8b24-e927d7b6ed2a`, `ast_f9e2f40a-eaa2-44ec-9627-d03187330ad1`, `ast_ab88a868-2c43-4d84-a61f-05a7d0ee345c`
- Comfy prompt ID: `e1d7e0ef-0b5f-45da-8297-486461ed0b00`
- Comfy node errors: none
- Sampling: 20 steps, approximately 152 seconds
- Output: `MiniMax_H3_Quality_00048_.mp4`
- Output metadata: H.264/AAC, 960×544, 24 fps, 39 video frames, 1.625 seconds, 406,501 bytes

This smoke submitted the frozen Runtime Package directly to the local ComfyUI API with existing DEV023 Krea2 assets. It proves the real package, model set, GPU, ordered three-reference input, and output path work. It was not imported into the AI Studio database and therefore does not claim an AI Studio `run_id`, task ID, or asset ID for the generated video.

## Order Integrity

PASS in the deterministic orchestrator E2E and in the live Comfy smoke request. The live request payload bound reference indices `0/1/2` to B/A/C respectively; the generated output completed without node validation errors.

## Recovery

PASS in the no-GPU E2E. The failed H3 item is retried in the same ProductionBatch with a new attempt and preserved frozen values and lineage. Latest-attempt aggregation returns a successful final batch.

## Regression

- Rust format check: PASS
- `cargo check`: PASS (existing dead-code warnings only)
- Rust tests: `468 passed, 0 failed`
- Frontend tests: `47` files, `156 passed`
- Frontend build: PASS
- `git diff --check`: PASS

## Database

- No migration 019 was added.
- Existing schema remains compatible with the new nullable `reference_index` field.
- Backup v9 behavior remains covered by the existing round-trip test.

## Backup

PASS. The backup round-trip test creates a new project and preserves asset bytes.

## Data Root

Implemented as a small, backward-compatible seam: `AI_STUDIO_DATA_ROOT` is accepted only when it is a non-empty absolute path. The default path is unchanged, and the path override is covered by unit tests. No migration or published package content depends on the override.

## Architecture Boundaries

- Published v0.4.0 artifacts and Runtime Package bytes were not modified.
- The new data-root seam is intentionally limited to path resolution; it does not introduce a second storage abstraction.
- No automatic DEV-026 work was started.

## Known Issues

- The installed Tauri webview exposed the Production Run tab in its accessibility tree, but the local computer-control bridge could not resolve click geometry; screenshot capture also failed with the OS `SetIsBorderRequired` unsupported-interface error. The app remained on the Batch Images page, so the full UI-driven AI Studio Production Run gate is **BLOCKED**, not PASS.
- ComfyUI startup logs contain a pre-existing DualClock custom-node import warning (`time_shift_slope` mismatch). The MiniMax H3 Director routes and the tested REF2VA package still loaded and executed successfully.
- The live Comfy smoke output was not inserted into the AI Studio database because doing so outside the orchestrator would invalidate lineage evidence.

## Final Decision

DEV-025 implementation and offline orchestration gates: **PASS**.

Real local ComfyUI/RTX 5060 Ti REF2VA smoke: **PASS**.

Full AI Studio UI/GenerationService Production Run live gate: **BLOCKED by local UI automation geometry**, with the blocker recorded explicitly above. No false end-to-end PASS is claimed.

## Next Step

Use a follow-up UI-capable validation environment to drive the Production Run tab through AI Studio’s own GenerationService and record the resulting run/task/asset lineage. This is a validation follow-up, not an automatic DEV-026 implementation.
