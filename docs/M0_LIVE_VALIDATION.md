# M0 Live Validation

Date: 2026-08-06

AI Studio baseline commit: c9f486e380554339d6aec3ee39fa4e056c9bc6ed
AI Studio implementation commit: recorded in the Git history for this validation run

MILESTONE 0 = PASS

## Runtime Workflow

- Catalog selection: PASS (latest Recipe version is selected for the current Workflow Version)
- Workflow package ID: wfl_kera2_t2i_local_v2
- Workflow version: 1.0.0
- Recipe version: 1.0.1
- API Workflow: VALID
- API Workflow SHA256: 1d99a10d27c3dd6bd1d385cffd12b2082c1731444d3b2ab77a25f7d1bc3d740b
- Seed binding: node 2 / input `seed` / class `Seed (rgthree)`
- Runtime Seed constraint: `0 .. 1125899906842624`
- Runtime package sync: VALID

The live `/object_info` response reports the `Seed (rgthree)` INT range as
`-1125899906842624 .. 1125899906842624`. AI Studio keeps the domain Seed value
unsigned (`u64`), so the Runtime Recipe uses the effective non-negative range
`0 .. 1125899906842624`. The API Workflow and node definition were not modified.

## Capability

- Endpoint: http://127.0.0.1:8188
- ComfyUI: 0.30.1
- GPU: cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync
- VRAM: 17102864384 total bytes; UI observed approximately 14.7 GB free / 15.9 GB total
- Node Count: 4486
- Missing Nodes: NONE
- Missing Models: NONE

## Live M0-FIX-002

- Catalog: PASS
- Dynamic Form: PASS (Seed range displayed as `0 – 1125899906842624`)
- Non-blocking: PASS
- Prompt Validation: PASS
- Created: PASS
- Validating: PASS
- Preparing: PASS
- Queued: PASS (queue #4)
- Running: PASS (real GPU execution; progress reached 3/10 and 30%)
- Progress: PASS
- Collecting: PASS
- Succeeded: PASS
- Asset: PASS (1 PNG, 768 x 1280, 1,057,236 bytes)
- Image UI: PASS
- Restart Persistence: PASS (task and image remained visible after restarting AI Studio; ComfyUI showed Offline)

The successful task resolved its random Seed to `1106502232922927`, which is
within the Runtime Recipe range. The persisted generation snapshot contains
the Recipe range and the resolved Seed; no Seed validation error was returned
by ComfyUI.

## Previous Failure Addressed

The previous live failure was `WORKFLOW_VALIDATION_FAILED` for node 2,
`Seed (rgthree)`, input `seed`, because a random value exceeded the node's
maximum. M0-FIX-002 moves the constraint into the Recipe, removes the global
rgthree-specific compiler constant, validates Fixed and Random values with
unsigned-safe range logic, and preserves `node_errors` in `TaskError.raw`.

## M1

NOT ENTERED
