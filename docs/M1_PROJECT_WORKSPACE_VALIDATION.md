# M1 Project Workspace Validation

Date: 2026-08-06
Commit: `b0ace8a`

## Scope

M1-21 through M1-28:

- Project Repository CRUD foundation and metadata validation
- Project Service with ID-based local project roots and database-insert compensation
- Active Project persistence and fallback to `prj_default`
- Header project selector and Projects workspace
- Project-scoped generation, task history, task events, asset browsing, binary reads, and cancellation
- Cross-project side-effect protection

## Automated checks

- `cargo fmt --all -- --check`
- `cargo check`
- `cargo test -- --test-threads=1`
- `pnpm test`
- `pnpm build`

Result: PASS. Rust has 166 passing tests and the frontend has 12 passing tests
across 5 test files. TypeScript/Vite production build and `git diff --check`
also pass.

## Live ComfyUI Gate

- Endpoint: `http://127.0.0.1:8188`
- Health: PASS (`/system_stats` and `/object_info`)
- ComfyUI version: `0.30.1`
- Devices: 1
- GPU: `cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync`
- VRAM total: `17,102,864,384` bytes
- VRAM free: `15,875,309,568` bytes
- Node count: `4,486`

The exact ComfyUI Python listener was stopped for the offline check. Port 8188
became unavailable while the AI Studio process remained alive. ComfyUI was
then restarted from `D:\ComfyUI-WorkFisher-V2\ComfyUI` with its original
arguments and the API recovered with the same status values. Result: PASS.

## Desktop Gate Status

The desktop control session was stopped by the physical Escape safety signal
before the Projects workspace could be driven. Therefore this run does not
claim a new-project UI creation, project-scoped live T2I generation, or a
visual project-switch check. The database/filesystem snapshot after the run
contains only `prj_default` with 11 historical tasks and 7 generated assets;
no test project was created by the aborted session.

## Manual validation checklist

### Remaining desktop validation: Project Create / Rename

- Open Projects and create `Project Live Test` with a description.
- Confirm the project appears in the list and the header selector.
- Rename the project and confirm the header updates without restarting.
- Confirm there is no Delete Project or custom root-path control.

### Project Switch / State Isolation

- Switch between Default Project and Project Live Test.
- Confirm Studio input values reset to the selected workflow defaults.
- Confirm Tasks and Assets reload for the selected project.
- Confirm a task from the previous project is not shown in the active project.
- Confirm switching away from a running task does not cancel it.

### New Project T2I Live

- In Project Live Test, use the existing `wfl_kera2_t2i_local_v2` package.
- Run one text-to-image generation and observe `QUEUED → RUNNING → COLLECTING → SUCCEEDED`.
- Confirm the Task and Asset are visible only in Project Live Test.
- Confirm the new asset is under the new project root and not under `prj_default`.
- Switch to Default Project and confirm its existing history/assets remain isolated.

### Offline Project Use

- Stop ComfyUI and refresh the app.
- Confirm ComfyUI is Offline and Generate is disabled.
- Confirm project create, switch, browse, and rename still work.
- Restart ComfyUI after the check.

## Security boundary

Project DTOs expose only `id`, `name`, `description`, `createdAt`, and
`updatedAt`; root paths never cross the Tauri/React boundary. Task events add
only `projectId`. Cross-project task lookup/cancel and asset binary reads fail
as not found before ComfyUI or filesystem side effects.

## Not included

Preset, multi-image, mask, video, MiniMax H3, Project Delete, Asset Delete,
Task Delete, and Workflow Delete remain outside this phase.
