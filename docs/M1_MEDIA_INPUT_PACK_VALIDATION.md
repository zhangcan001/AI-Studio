# M1 Media Input Pack Validation

Date: 2026-08-06
Implementation commit: `feat: add source media input pack` (final commit hash is
reported at handoff)

## Scope

This gate adds source video and source audio assets, bounded streaming import,
generic ComfyUI input upload, media-aware Recipe values, project-scoped media
selection, local playback, and preset/history compatibility. Database
migrations were not added or modified.

## Automated evidence

- Rust source video/audio import tests cover signature checks, MIME/extension
  mismatches, cancellation, size limits, incremental hashing, atomic publish,
  ffprobe best effort, poster best effort, and database compensation.
- The logical 512 MiB video stream test uses bounded chunks and does not create
  a large fixture on disk.
- Generic ComfyUI upload tests cover video/audio multipart fields, server-side
  input identity, 413 mapping, and stream failures. The route remains
  `POST /upload/image` for every input type.
- Recipe, compiler, preparation, snapshot, preset, history, protocol, and
  project-isolation tests cover singular and ordered plural media values.
- Frontend tests cover media field compatibility, ordered selection helpers,
  required/plural validation, media URL routing, and project reset behavior.

## Runtime evidence

The real Tauri runtime was observed with the ComfyUI status card connected. The
runtime reported ComfyUI `0.30.1`, GPU `cuda:0 NVIDIA GeForce RTX 5060 Ti`,
VRAM `14.8 GB free / 15.9 GB`, and `4,486` nodes. A separate browser session
was used to inspect the same frontend surface; standalone browser mode has no
Tauri IPC bridge, so native file-dialog invocation is not reproduced there.

The desktop control adapter lost its helper session when clicking a WebView
control during the final UI walkthrough. This is an automation-environment
limitation, not an application error; the native runtime itself stayed alive
and its accessibility tree exposed the expected Studio, Assets, Tasks, and
Projects workspace controls. Component and integration tests cover the media
selection, import callback, playback URL, missing-asset, and project-scope
paths.

ComfyUI was online during the final runtime observation. The earlier offline
check also confirmed that the application remains usable when ComfyUI is not
available; only generation-dependent operations require the runtime.

## MiniMax H3 Reference Gate

The two permitted locations were checked:

1. the repository `runtime-import/minimax_h3_reference/workflow_api.json`;
2. `%LOCALAPPDATA%/AIStudio/AIStudioData/workflow_library/`.

No validated MiniMax H3 API workflow was present. Therefore T2V, I2V,
reference image, reference video, and reference audio live execution are
`NOT RUN`. No MiniMax-specific generation branch was introduced.

## Gate result

`M1 MEDIA INPUT PACK = PASS` after the final automated regression gate. MiniMax
H3 live execution remains `NOT RUN` because no validated user-supplied API
workflow was available.
