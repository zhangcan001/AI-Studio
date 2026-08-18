# DEV-040 — Scene Production Automation

## Scope

DEV-040 adds reusable batch workflow presets and scene-scoped production planning to AI Studio 0.6.0. It does not add a second generation runtime, queue, executor, ComfyUI path, database migration, backup format, manifest field, installer, release, or tag.

Baseline: `master` at `1a1fbcb2c3443b93f2e2c6ff6779fdc3ad3f013f`, equal to `origin/master`, clean before implementation.

Frozen release references were checked before work and remain unchanged:

- `v0.6.0^{}` = `e3d7181f23a9b7285a426efb20ead4db17198757`
- `v0.5.0^{}` = `02e67cff50f5da1d207478071636af166048820c`
- `v0.4.0^{}` = `94918f6322ce690ff7b1630961abb56b8a31ed11`

## Batch workflow presets

Presets live only in `AppSettings` as `batchWorkflowPresets` with serde defaults. Schema version stays `1`; no SQLite, migration, backup, or manifest changes are required.

Each preset stores only reusable workflow version, recipe, and sanitized scalar values for Image and/or Video. Save validation reuses the existing recipe sanitizer and rejects unavailable workflows. Project-owned IDs and media/reference/selection fields are removed. CRUD enforces a maximum of 30 presets, trimmed 1–80 character names, case-insensitive unique names, descriptions up to 500 characters, and at least one stage.

Existing presets whose workflows later disappear remain stored and are listed with `available: false` and `reason: WORKFLOW_UNAVAILABLE`. They cannot be applied until the workflow is available again.

The Shot workspace can save the selected Shot's Image/Video config, rename/delete presets, and apply one stage to the current Scene through the existing bulk stage-config service. Applying requires confirmation and preserves Ordered References, Selected Image, and Selected Video.

## Scene plan and prepare

`SceneProductionService` verifies the Scene through the existing project-owned Production Structure tree, asks `ShotBatchService` for the existing eligibility rules, and orders rows by project-global Shot ordinal. Rows are classified as `DONE`, `PREPARED`, `ELIGIBLE`, or `BLOCKED`, with existing ShotBatch blockers preserved for display.

Preparation is bounded to 100 eligible Shots and returns `SCENE_PRODUCTION_TOO_LARGE` above that limit. Strict preparation defaults to `allowPartial=false`; any blocked row returns `SCENE_PRODUCTION_BLOCKED` and creates no batch. Explicit partial preparation skips DONE/PREPARED/BLOCKED rows and creates only eligible rows. Empty eligible sets do not create an empty batch.

Preparation is guarded by a short service mutex and a set-based active-binding recheck immediately before delegating to `ShotBatchService::create`. Repeated calls are idempotent and concurrent calls cannot create duplicate Shot/Stage bindings in the same application instance. The resulting batch remains READY; the UI starts it only through the existing `production_queue_start` command.

No Image → Video automatic transition or asset selection is performed. Video eligibility continues to require the existing manually selected image/reference state, and both image and video results remain subject to manual Review.

## UI and prompt integration

`SceneProductionPanel` is mounted in the existing Shot workspace beside the existing Production Structure and project pipeline panels. It provides Scene/Stage selection, preset management/application, prompt preview/application, plan counts and blockers, strict/partial prepare, busy locking, error codes, and explicit queue start. It uses the existing `PromptTemplateBulkService` path; the stage prompt is frozen by the existing prompt application flow.

The panel does not create a second structure tree, generation path, executor, queue, or direct ComfyUI request.

## No-GPU review gate

Deterministic tests cover the requested Scene A shape (DONE 3, PREPARED 2, ELIGIBLE 6, BLOCKED 1), strict block, partial six-item preparation, repeat idempotency, concurrent admission, Scene B video image-review blocking, cross-project scope, and a 500-Shot/50-Scene planning sanity check. The architecture audit checks that Scene production reuses Structure, ShotBatch, Prompt bulk, and Queue Start boundaries and does not reference GenerationService, Comfy, `/prompt`, or a second queue/executor.

No GPU, ComfyUI live validation, UI key simulation, installer validation, release publication, or tag mutation is part of DEV-040.

## Validation gate

Targeted validation is run before the single final full regression. Final release safety requires:

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
pnpm test
pnpm build
git diff --check
```

Product remains `0.6.0`, migration remains `021`, backup version remains `12`, and Manifest remains `1`.

## Final validation

DEV-040B final gate was run on `master` at `70216025b7818c07539597a0e1374aa71875b6d4`, with a clean worktree and `HEAD == origin/master`.

Targeted Rust:

- `dev040`: 6 passed, 0 failed, 0 ignored.
- `BatchWorkflowPresetService`: 7 passed, 0 failed, 0 ignored.
- `SceneProductionService`: 2 passed, 0 failed, 0 ignored.
- The targeted suites cover preset CRUD and safety, DONE/PREPARED/ELIGIBLE/BLOCKED classification, strict and partial prepare, repeat and concurrent prepare admission, manual Image Review gating, and the 500 Shot / 50 Scene fixture.

Targeted frontend:

- 2 files / 8 tests passed: `SceneProductionPanel.test.tsx` (3) and `dev040Stability.test.ts` (5).

Final regression:

- `cargo fmt --all -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- Rust full: 564 passed, 0 failed, 1 ignored.
- Integration suites: DEV-035 3 passed, DEV-036 9 passed, DEV-039 5 passed, DEV-040 6 passed; 23 passed total.
- Rust doc-tests: 0 passed, 0 failed.
- Frontend: 66 files / 231 tests passed.
- `pnpm build`: PASS; 155 modules transformed.
- `git diff --check`: PASS.

Architecture and safety confirmation:

- `BatchWorkflowPresetUpdateRequest` is public and `batch_workflow_preset_update` compiles.
- Preset and Scene commands are formally registered; `AppState` contains the Preset and Scene services.
- Scene production reuses `ProductionStructureService`, `ShotBatchService`, the existing `ShotBatchRepository`, `PromptTemplateBulkService`, and `production_queue_start`.
- No DEV-040 direct `/prompt`, `ComfyHttpAdapter`, `GenerationService`, `SceneQueue`, `SceneExecutor`, or duplicate `ProductionBatch` path exists.
- `batch_workflow_presets` uses `serde(default)` and settings preservation remains intact.

Version and release freeze:

- Product `0.6.0`; migration `021`; backup version `12`; Manifest `1`.
- `v0.6.0^{}` = `e3d7181f23a9b7285a426efb20ead4db17198757`.
- `v0.5.0^{}` = `02e67cff50f5da1d207478071636af166048820c`.
- `v0.4.0^{}` = `94918f6322ce690ff7b1630961abb56b8a31ed11`.

## Final decision

DEV-040 BATCH WORKFLOW PRESETS + SCENE PRODUCTION AUTOMATION PASS
