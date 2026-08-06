# M1 Project Workspace Final Gate

Date: 2026-08-06

This record covers FIX-PROJECT-001 and LIVE-PROJECT-001 through
LIVE-PROJECT-006. The validation stopped at this gate; Preset, Multi Image,
Mask, Video, MiniMax H3, and delete flows were not entered.

## Source Hardening

- PASS: Project IDs now use one domain validator. `prj_default` is accepted;
  created projects must use `prj_<canonical UUID>`. Empty, arbitrary,
  path-like, separator-containing, and malformed IDs are rejected.
- PASS: Project update, generation creation, task queries/history/cancel, and
  asset queries/binary reads validate the project boundary before application
  work or filesystem/ComfyUI side effects.
- PASS: No database migration was changed. The frontend retains only the
  intentional `prj_default` bootstrap constant; no root/storage path is
  exposed through the normal React bridge.

## Automated

- PASS: `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- PASS: `cargo check --manifest-path src-tauri/Cargo.toml`
- PASS: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
  — 168 Rust tests passed.
- PASS: `pnpm test` — 12 frontend tests passed across 5 files.
- PASS: `pnpm build`
- PASS: `git diff --check`
- PASS: Frontend scan found no forbidden root/storage path fields or
  unapproved project-ID literals.

## Project UI

- PASS: Created `Project Live Test`, received a UUID-backed project ID, and
  confirmed its project root was created by the application.
- PASS: Renamed it to `Project Live Test Renamed`; the description and stable
  project ID were preserved.
- PASS: Header selector, Projects workspace Open/Edit actions, and active
  project state were visible and consistent.
- PASS: After closing and reopening the development instance, the selector
  still showed `Project Live Test Renamed`.
- PASS: The UI exposes no Project Delete, custom root-path, Move Project, or
  Browse Folder action.

## New Project T2I

- PASS: The existing M0 text-to-image package was started from the new
  project through the UI.
- PASS: The live task was observed in `RUNNING`, then reached `COLLECTING`
  and `SUCCEEDED`; output collection produced an image Asset.
- PASS: Three generated tasks in the test project completed successfully,
  with three distinct prompt identities and one generated Asset per task.

## Isolation

- PASS: Read-only database verification ended with Default Project at 11
  tasks / 7 assets, Project Live Test Renamed at 3 tasks / 3 assets, and the
  offline test project at 0 tasks / 0 assets.
- PASS: Switching to Default Project showed only its existing history and
  assets; switching back showed the new project records and previews.
- PASS: Generated assets were linked to the creating project and source task;
  no cross-project record or binary read was observed.

## Running Switch

- PASS: A running task was observed while opening Projects and switching away
  from its project. The source project's task continued in ComfyUI and ended
  `SUCCEEDED`; the destination project did not display it, and no cancel or
  duplicate prompt was created.
- PASS: A separate clean running-task audit had zero `CANCEL` and zero
  `RECOVERY` events, one prompt identity, and one final Asset.
- Note: One earlier observation included a `TASK_RECOVERY_STARTED` /
  `TASK_RECOVERY_SUCCEEDED` pair because the evaluator manually pressed the
  visible `Reconcile tasks` button after the route switch. That was not caused
  by project switching, did not cancel or resubmit the task, and the clean
  audit above was performed without pressing that control.

## Offline

- PASS: ComfyUI was stopped while no task was active. AI Studio remained
  alive; after Test Connection, the UI showed Offline and Generate was
  disabled.
- PASS: While offline, `Offline Project Test` was created and renamed to
  `Offline Project Test Renamed` without a command failure or database error.
- PASS: The existing project remained readable offline: Tasks, Task Detail,
  Load Inputs, Asset Library, and Asset Preview all loaded successfully.
- PASS: ComfyUI restart restored `/system_stats` with HTTP 200. AI Studio then
  showed Connected, version `0.30.1`, one NVIDIA GeForce RTX 5060 Ti device,
  VRAM `14.8 GB / 15.9 GB`, and node count `4,486`; node refresh succeeded.

## Security

- PASS: Project DTOs expose only project metadata; project roots, storage
  paths, workflow payloads, recipe YAML, prompt IDs, and asset internals stay
  outside the React bridge.
- PASS: The validation document contains no user profile path, ComfyUI
  installation path, or project-root absolute path.
- PASS: Test projects were intentionally retained for continued local review;
  no delete flow was exercised.

## Final status

M1 Project Workspace Final Gate: **PASS**.

FIX-PROJECT-001 and LIVE-PROJECT-001 through LIVE-PROJECT-006 are complete.
Stop here as requested; the next phase was not entered.
