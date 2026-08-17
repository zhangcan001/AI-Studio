# DEV-027 — Project Production Pipeline

## Result

**PASS** — the project production pipeline is implemented and validated through the
backend, the no-GPU six-shot E2E, and one real ComfyUI representative run.

The existing `v0.4.0` tag was not changed. Its peeled SHA remains
`94918f6322ce690ff7b1630961abb56b8a31ed11`. No `v0.5.0` tag, release, or build
artifact was created.

## Scope and architecture

- Added project-scoped bulk shot import for JSON schema V1 and TSV (2–4 columns),
  capped at 500 rows.
- Import follows Parse → Normalize → Validate All → Preview → Confirm → Atomic
  Commit. It is create-only, blocks duplicate names, and appends ordinals.
- Added stage-owned image/video prompt snapshots and migration `019`. Prompt
  Library assignment stores entry/version provenance; clearing provenance keeps
  the text but removes the linkage. Later Prompt Library edits do not silently
  change a shot.
- Added atomic bulk stage configuration. Image runtime is Krea2; video runtime
  reuses the existing I2V/REF2VA validation and runtime scope.
- Added the Project Production Pipeline view with derived stage progress while
  retaining the existing human review and manual result-selection flow.
- Reused the existing `ShotBatchService`, `ProductionQueueService`,
  `GenerationService`, task/asset lineage, and ComfyUI adapter. No second queue,
  executor, generation service, or direct `/prompt` caller was introduced.
- Backup format is now v10 with stage prompt snapshots. The v9 restore path
  remains supported and was tested.

## No-GPU acceptance

`dev027_project_production_pipeline_six_shot_no_gpu_e2e` passed:

- six shots imported atomically;
- shots 1–3 configured as I2V and shots 4–6 as REF2VA;
- image and video batches created with frozen prompts/configuration;
- image results manually selected;
- ordered REF2VA references persisted and reused;
- all six shots reached `COMPLETED` with selected image and video assets;
- database close/reopen preserved the result.

## Real ComfyUI live gate

Command:

```powershell
$env:AI_STUDIO_LIVE_IN_PLACE='1'
$env:AI_STUDIO_LIVE_DB_SOURCE='C:\Users\ADMIN\AppData\Local\AIStudio\AIStudioData\app.db'
cargo test --manifest-path src-tauri/Cargo.toml application::dev027_e2e::tests::dev027_live_representative_bulk_import_image_select_i2v -- --ignored --exact --nocapture --test-threads=1
```

Runtime evidence:

- ComfyUI root: `D:\ComfyUI-WorkFisher-V2`
- Python: `D:\ComfyUI-WorkFisher-V2\python\python.exe`
- ComfyUI: `0.33.0`
- Python: `3.12.10`
- Device: `cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync`
- Endpoint: `http://127.0.0.1:8188`
- Flow: bulk import → Krea2 image → manual image selection → H3 FL2VA I2V →
  manual video selection → final `COMPLETED`

Durable live evidence:

```text
project=prj_18e0fe0d-2832-4def-8b26-561b27e2f949
shot=sht_f23354b9-3f23-42b4-8241-1b71d2390ff4
image_batch=pbt_5ba21c076c67423a8f8b89dac1f22316
image_task=tsk_cffbe42c-564d-4f80-9131-1949e59f74d5
image_asset=ast_a73610b8-ffeb-4e1b-bf2f-83b06134dd59
video_batch=pbt_25b8aa78367042e081760abae66a87b1
video_task=tsk_ba854983-157f-483a-aece-60dac5cffdea
video_asset=ast_7542628c-489f-454b-978d-7149a883dc92
image_workflow=wfl_kera2_t2i_local_v2
image_workflow_version=wfv_2407734d-ff20-44d9-ac7c-15ab514d7193
image_recipe=rcp_0575fb13-6bfb-41cb-ba10-eba2719a793c
video_workflow=wfl_minimax_h3_fl2va_i2v_quality
video_workflow_version=wfv_817672cf-3dcb-495e-ad9e-201429ba684d
video_recipe=rcp_5fcf5c7e-38f0-4f89-bf37-d6d372c46fa7
endpoint=http://127.0.0.1:8188
```

The live run passed in 566.97 seconds and left the project, batches, tasks, and
assets in the supplied local AI Studio database for audit.

## Regression evidence

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` — PASS
- `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` —
  **480 passed, 0 failed**
- `pnpm test` — **52 files, 169 tests passed**
- `pnpm build` — PASS (Vite 7.3.6; only the existing large-chunk warning)
- backup service tests — **26 passed**, including v10 round-trip and v1–v9
  restore fixtures
- migration safety tests — PASS
- frontend focused bulk/pipeline tests — **9 passed**
- `git diff --check` — PASS

## Next task

With DEV-027 complete and pushed, proceed to **DEV-028**.
