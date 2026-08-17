# DEV-026 — Shot Production 2.0 Validation Record

Date: 2026-08-17
Baseline: `master` at `a74a303b236914abb9285eda6690e3517f955a82`
Release boundary: existing `v0.4.0` tag was not changed. Its peeled commit is `94918f6322ce690ff7b1630961abb56b8a31ed11`.

## Parallel Execution

Implementation and review were split across the existing backend, repository, and frontend areas. Agents A/B/C completed their scoped work; the architecture/backup audit was read-only. The shared worktree was integrated without unrelated changes or merge/reset operations.

## Existing Architecture Reuse

- Shot generation continues through `ShotService` / `ShotBatchService` → `ProductionQueueService` → `GenerationService` → `ComfyHttpAdapter`.
- No direct Comfy `/prompt` call, second executor, second queue, or alternate scheduler was added.
- Existing task lifecycle, queue recovery, strict serial H3 gate, asset store, runtime package, and project backup paths remain the source of truth.
- The real-run harness used an isolated data root and invoked the formal application chain; it did not drive the desktop UI or simulate key presses.

## Video Modes

- I2V keeps the singular `Image` input contract and requires the selected keyframe.
- Exact H3 REF2VA runtime IDs use plural `ImageAssets` and an ordered reference manifest; a selected keyframe is optional.
- REF2VA effective minimum is `max(recipe.min_items, 2)`, so a stale/zero package minimum cannot permit a one-image shot. Recipe maximum remains enforced.
- Invalid input shape, duplicate IDs, project/type mismatch, and min/max violations fail before dispatch.

## Ordered References

Reference assets are ordered by persisted `ordinal ASC`, with asset ID as a deterministic tie-breaker. Duplicate IDs are rejected. The formal manifest is passed into generation evidence, so the order is not merely a frontend display convention.

## Batch Freeze

Batch creation freezes the complete per-shot values: workflow/recipe identity, mode, seed, selected image where applicable, and the ordered REF2VA `ImageAssets` list. Later edits to the Shot do not mutate an existing batch.

## Retry

Retry is bound to the source batch item. The repository copies the source batch, workflow, recipe, frozen values, and Shot identity into the retry row and rejects cross-batch binding. A retry cannot replace the original frozen reference order with caller-supplied values.

## Restart

The no-GPU lifecycle test closes and reopens the SQLite repository, then verifies the original batch, edited/new batch, and retry all retain their persisted identities, modes, and reference order.

## No-GPU E2E

PASS: `no_gpu_three_shot_video_batch_freezes_modes_order_retry_and_restart`.

The scenario covers three shots: one I2V shot and two REF2VA shots. It verifies B/A/C freeze, a later C/B/A edit only affects the new batch, retry reuse of the old frozen B/A/C values, and persistence across restart.

## Real Live

RESULT: **BLOCKED / FAIL at the environment gate; no video output claimed.**

The isolated backend harness reached the formal Shot batch → production queue → generation service → Comfy HTTP path, but workflow validation stopped before model execution:

```text
WORKFLOW_VALIDATION_FAILED
Node 'NBH3HyperStepSimple' not found. The custom node may not be installed.
```

The local ComfyUI health endpoint was available (`0.31.1`, Python `3.12.10`, PyTorch `2.10.0+cu130`, RTX 5060 Ti/CUDA). However, `/object_info` exposes the H3 reference/image workflow nodes but not `NBH3HyperStepSimple`; the configured H3 model root also does not exist locally and the installed model directories contain no required H3 diffusion/text/VAE assets. The harness therefore produced no snapshot or video and correctly stopped at the validation boundary.

## Order Integrity

PASS in no-GPU and repository coverage: B/A/C remains B/A/C in the old frozen batch, in its retry, and after restart; a later Shot edit to C/B/A appears only in the newly created batch.

## Regression

- Rust formatting and compile check: PASS (`cargo fmt --all`, `cargo check`).
- Full Rust regression: PASS, `477 passed; 0 failed`.
- Focused REF2VA manifest tests: PASS, 4 tests.
- Focused backup/service coverage: PASS, 26 tests.
- Frontend typecheck: PASS.
- Frontend targeted tests: PASS, 8 tests.
- Frontend full test suite: PASS, 160 tests across 49 files.
- Production frontend build: PASS (`tsc && vite build`). Vite reports only the existing-style non-blocking large-chunk warning.
- `git diff --check`: PASS; only Windows line-ending normalization warnings were reported.

## Database

No migration 019 was added. The latest migration remains `018_production_orchestrator.sql`. Retry binding and frozen values reuse the existing production queue schema.

## Backup

Backup version remains `9`. Existing project backup round-trip coverage passes, including the persisted Shot/reference relations and their ordered representation. No backup format bump was needed.

## Frozen Boundaries

- Existing runtime package bytes were not modified.
- Existing `v0.4.0` tag was not recreated, moved, or force-updated.
- No force push, `reset --hard`, or `clean -fd` was used.
- No UI automation was used.

## Known Issues

The local ComfyUI installation is not a valid live H3 REF2VA execution environment: the required custom node `NBH3HyperStepSimple` and H3 model root are absent. This is an external runtime capability/model issue, not a no-GPU application regression. Installing or replacing that environment was intentionally left outside this code task.

## Final Decision

**DEV-026 SHOT PRODUCTION 2.0: FAIL for the required real-live gate; implementation, frozen multi-shot behavior, retry/restart persistence, backup coverage, and all available regression checks PASS.**

## Next Step

Restore a compatible ComfyUI custom-node set and H3 model root, then rerun only the isolated real Shot REF2VA harness. Do not claim DEV-026 PASS or begin DEV-027 until that live gate produces a validated snapshot/video through the formal backend path.

## DEV-026B Runtime Recovery + Live Closure

Date: 2026-08-17

### Baseline and parallel agents

- DEV026B start SHA: `6c48d8a1717e4dd777b9f9cb5e7586bde0aa8949`.
- `master` was clean and synchronized with `origin/master`.
- Agent A identified the current 8188 listener. Before recovery, no process was listening because the previously used bad instance had already been stopped; no unrelated Python/AI process was terminated.
- Agent B performed a bounded candidate scan under the known AI/ComfyUI locations.
- Agent C audited AI Studio settings/runtime binding read-only. The persisted endpoint is `http://127.0.0.1:8188`; H3 model paths are external ComfyUI model references, not an AI Studio setting.

### Wrong environment and environment discovery

The historical DEV-026 failure environment was `C:\Users\ADMIN\Desktop\Comfyui-minmaxh3`: ComfyUI `0.31.1`, Python `3.12.10`, PyTorch `2.10.0+cu130`, RTX 5060 Ti. It lacked `NBH3HyperStepSimple` and all nine H3 model filenames required by the shipped H3 packages.

The bounded discovery produced:

| Root | ComfyUI | H3 node | H3 models | Usability |
| --- | --- | --- | --- | --- |
| `C:\Users\ADMIN\Desktop\Comfyui-minmaxh3` | 0.31.1 candidate | Missing | None | Invalid |
| `C:\Users\ADMIN\Desktop\music-ComfyUI\ComfyUI-WorkFisher-V2` | candidate | Missing | None | Invalid |
| `D:\ComfyUI-music` | candidate | Missing | None | Invalid |
| `D:\ComfyUI-WorkFisher-V2` | 0.33.0 | Present in `ComfyUI-NB-H3-HyperStep\nodes.py` | Complete | Selected |

### Selected environment and hard gates

- Root: `D:\ComfyUI-WorkFisher-V2`.
- Python: `D:\ComfyUI-WorkFisher-V2\python\python.exe`.
- Main: `D:\ComfyUI-WorkFisher-V2\ComfyUI\main.py`.
- Live PID: `32004`.
- Command: `main.py --force-upcast-attention --use-pytorch-cross-attention --listen 127.0.0.1 --port 8188`.
- `/system_stats`: HTTP 200.
- `/object_info`: HTTP 200.
- ComfyUI `0.33.0`, Python `3.12.10`, PyTorch `2.9.0+cu130`, CUDA device `NVIDIA GeForce RTX 5060 Ti`.
- Required node gate: PASS, including `NBH3HyperStepSimple`, `MiniMaxH3ReferenceToVideo`, `MiniMaxH3MemoryEfficientSageAttentionPatch`, media loaders, model loaders, and `SaveVideo`.
- Required model gate: PASS. REF2VA diffusion, Qwen3VL text encoder, video VAE, and audio VAE all exist under the selected ComfyUI `models` tree.
- The pre-existing `ComfyUI-MiniMaxH3DualClockSampler` `time_shift_slope` import warning remains a non-blocker; it does not prevent the required H3 node or REF2VA route from loading.

### Shot live closure

PASS through the formal backend path:

`ShotBatchService → ProductionQueueService → GenerationService → ComfyHttpAdapter`

- Shot: `sht_dev026b_ref2va`.
- Batch: `pbt_f75e90b6eac0475db648dfb23dc64c52`.
- BatchItem: `pbi_b260fa90f7914ae8b2577be52d5c7ca5`.
- Task: `tsk_6626977a-53fa-40a2-b254-39dc70976eb3`.
- Comfy prompt: `75df5d22-827e-40bb-b6a1-e2725029b350`.
- Snapshot: `snp_b4a36680-c2ad-4cdf-8efd-6ffc04d3c285`.
- Generation execution: `gen_27f96f2508438af13d923285db7587709060eec682ce23bfe1c3a0fcb3d06217`.
- Compiled workflow SHA-256: `d57cae6c574388f08eee049a26104f2cf2c3ac9a3365662a1923bae853f3e66f`.
- Video Asset: `ast_568bd99d-8639-46fb-a36a-5e3fa23338e7`.
- Final states: Task `SUCCEEDED`, BatchItem `SUCCEEDED`, Batch `COMPLETED`.

### Reference integrity and video

The frozen Shot values, Snapshot user inputs, resolved upload identities, compiled workflow, and Comfy history all preserve B/A/C:

- Snapshot asset IDs: `ast_d9415383-f501-40b7-8b24-e927d7b6ed2a` → `ast_f9e2f40a-eaa2-44ec-9627-d03187330ad1` → `ast_ab88a868-2c43-4d84-a61f-05a7d0ee345c`.
- Compiled/Comfy nodes: node `24` uploaded B, node `28` uploaded A, node `29` uploaded C.
- Comfy history status: `success`, `completed=true`, output node `21`.
- ffprobe: H.264 video + AAC audio, `960×544`, `24 fps`, duration `1.625 s`, `179,927 bytes`.

### Code, regression, database, and backup

- Product code changes: **NO**. The live harness was temporary, then removed; no runtime package bytes, workflow, migration, or settings file was changed.
- Regression: reused the DEV-026 evidence (`477` Rust tests and `160` frontend tests passed) and ran only the DEV-026B harness compile/live test plus read-only history checks. The live harness test passed `1/1`; no full regression rerun was needed.
- Migration remains `018`; no migration `019`.
- `BACKUP_VERSION` remains `9`.
- No model, custom node, virtual environment, or local path was committed.

## Final Decision (DEV-026B)

**DEV-026 SHOT PRODUCTION 2.0: PASS — valid H3 runtime restored, real Shot REF2VA live gate passed, B/A/C integrity passed, and the validated video asset exists.**

## Next (not automatically started)

`DEV-027 — Project Production Pipeline: Bulk Shot Import + Prompt Assignment + End-to-End Episode Production`
