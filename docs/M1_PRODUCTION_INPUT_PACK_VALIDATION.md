# M1 Production Input Pack

Date: 2026-08-06

This validation record covers M1-29 through M1-37. The explicitly excluded
Mask, Video, MiniMax H3, delete flows, Workflow Import Wizard, Model Download,
Node Install, Drag & Drop, Thumbnail Backfill, and Exact Retry work was not
entered.

## PRESET

- Migration: PASS. `003_presets.sql` adds the project/workflow-version/recipe
  scoped table and index. `001_initial.sql` and `002_browse_indexes.sql` were
  not modified.
- CRUD: PASS. `PresetService`, `PresetRepository`, SQLite persistence, and the
  four project-aware commands are covered by Rust tests. The live UI saved,
  listed, applied, and updated a `Live Preset Gate` record.
- Project isolation: PASS. Presets store `pst_<UUID>` IDs and are queried by
  project plus workflow version plus recipe. Asset IDs are validated against
  the active project; cross-project values are rejected.
- Apply without auto generation: PASS. The live test changed prompt, steps,
  and fixed seed, applied the saved preset, observed prompt/steps/seed restore,
  and observed no new Task until Generate was pressed explicitly.

Preset persistence stores draft values only: random seeds remain tagged
`seed_random`, fixed seeds remain decimal strings, single-image values remain
single Asset IDs, and multi-image values remain ordered Asset ID arrays.

## MULTI IMAGE

- Recipe: PASS. Schema version 1 accepts `images`, validates min/max bounds,
  required minimums, and the 32-item ceiling while preserving legacy `image`.
- Ordered Asset IDs: PASS. Application values and historical snapshots retain
  duplicates and user order.
- Ordered uploads: PASS. Validation completes before sequential uploads;
  generated upload names carry stable positions and ComfyUI-returned names are
  used in the resolved snapshot.
- Compiler list binding: PASS. Lists bind as JSON arrays in input order.
- Compiler item binding: PASS. `item: N` binds the requested ordered string,
  and validator rules require an image source and a guaranteed index.
- Snapshot: PASS. User and resolved snapshots retain ordered IDs, SHA-256, and
  ComfyUI identities without absolute paths.
- Mock E2E: PASS. The backend mock E2E covers two ordered uploads, one submit,
  list binding, item binding, and snapshot order.
- Live: NOT RUN. Reason: No validated runtime multi-image Workflow Package
  supplied.

## THUMBNAIL

- Source image: PASS. PNG, JPEG, and WebP source import tests cover thumbnail
  generation, aspect-ratio preservation, and the 384 px long-edge limit.
- Generated image: PASS. The live AI Studio generation produced Task
  `tsk_d77c518e-2108-443a-8709-489fb1e511d6` and Asset
  `ast_7fcc733a-e202-4df7-bd2e-3478505f77c3`; the database recorded a
  thumbnail and the file measured 230 × 384 while the original measured
  768 × 1280.
- Best-effort failure handling: PASS. Thumbnail errors are warnings; the full
  Asset remains valid. Database failure compensation removes only full and
  thumbnail files created by that import.
- Asset Library: PASS. The live Asset Library card preferred the thumbnail,
  and legacy records with no thumbnail remain readable through full-image
  fallback.
- Full preview regression: PASS. The live card opened the original-image
  preview successfully. The same card and preview remained available after
  ComfyUI was stopped and the app was switched to Offline.

## REGRESSION

- Project isolation: PASS. Existing project-scoped task, asset, preset, and
  binary boundaries remain enforced.
- T2I Live: PASS. Runtime package `wfl_kera2_t2i_local_v2` was run through the
  AI Studio UI with the fixed seed constraint. Task events included QUEUED,
  RUNNING, COLLECTING, and SUCCEEDED.
- Cancel: PASS. Existing cancellation E2E and service tests remain green.
- Recovery: PASS. Existing startup recovery/reconciliation tests remain green.
- Task History: PASS. Existing history and ordered multi-image draft tests
  remain green.
- Asset Library: PASS. Existing browse, category, thumbnail, full-image, and
  offline preview paths remain green.

## LIVE COMFYUI EVIDENCE

- Endpoint: `http://127.0.0.1:8188`
- Version: `0.30.1`
- GPU: `cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync`
- VRAM: 1.8 GiB free / 15.9 GiB total at the status probe
- Node Count: 4,486
- Real T2I output: `AI Studio` reached `SUCCEEDED`; the generated Asset and
  thumbnail were read locally while ComfyUI was Offline.

## TESTS

- Rust: 175 passing
- Frontend: 13 passing across 5 files
- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test -- --test-threads=1`: PASS
- `pnpm test`: PASS
- `pnpm build`: PASS

## Technical debt

- Multi-image Live remains pending until a validated multi-image Runtime
  Workflow Package is supplied.
- Legacy thumbnail records remain nullable by design; thumbnail backfill is
  intentionally outside this stage.
- Desktop screenshot capture through the bundled Windows capture adapter was
  unavailable for this Tauri WebView, so final UI assertions used the app's
  Windows accessibility tree and real command-backed state instead.

## Final status

M1 PRODUCTION INPUT PACK = PASS

The next phase is not entered. The only future recommendation is the separately
scoped VIDEO FOUNDATION (Video Asset / Output Type, Video Workflow Runtime,
MiniMax H3).
