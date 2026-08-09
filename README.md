# AI Studio

AI Studio is a Windows desktop foundation for a local AI image/video production workbench. M0 contains the Tauri 2 + React shell, Rust layering, SQLite migration, application data directory initialization, ComfyUI capability detection, and a pure local Recipe/Workflow compiler.

Current development version: `0.2.0` (M2 Foundation Pack 01).
The released `0.1.0` baseline remains available in the Git history and GitHub
Release; this development cycle does not modify its tag or release assets.

Current milestone progress:

M0 = PASS.

M1 progress:

Production scope is intentionally limited to Kera2 for image generation and MiniMax H3 for video generation. The shared runtime remains generic, but no third model runtime pack is part of the product plan.

**M1 Release Candidate 01 = PASS.** Global single-GPU production admission,
cross-project and interactive submission safety, deterministic restart
recovery, a real ordered Kera2/MiniMax H3/Kera2 persistent queue, project-scoped
assets and playback, and Windows Release fresh/existing database gates are all
validated. Evidence is recorded in `docs/M1_RELEASE_CANDIDATE_01.md`.

**M1 简体中文界面 = PASS。** 主导航、创作工作台、资产库、任务历史、项目、
工作流、生产队列、ComfyUI 状态、空状态和用户可见错误提示已完成简体中文产品化。
协议枚举、数据库 migration、用户提示词、用户文件名、模型名和技术标识保持不变；
错误原文仅在折叠的技术详情中显示。验收证据记录在
`docs/M1_ZH_CN_UI_VALIDATION.md`。

**M1 Product UX Polish = PASS.** Creation-first Studio, catalog-driven workflow cards,
project-scoped graphical asset picker, compact task status, result-first preview,
and Kera2/H3 live UX gates are validated. Evidence is recorded in
`docs/M1_PRODUCT_UX_POLISH_VALIDATION.md`.

- Desktop foundation and Rust application layering
- SQLite migration and local application data directories
- ComfyUI connection, `/system_stats`, and `/object_info` capability detection
- Recipe YAML parsing and semantic validation
- ComfyUI API Workflow validation and immutable local compilation
- Task domain and state machine
- Task event persistence with transactional state transitions
- Immutable generation snapshots
- Generation orchestration through `GenerationService`
- ComfyUI `/prompt` submission with client-generated `prompt_id`
- ComfyUI WebSocket execution tracking and normalized Task events
- Output collection from `/history/{prompt_id}` and `/view`
- Project-rooted generated image Asset Store with SHA-256 and image metadata
- Transactional Asset Repository and compensating cleanup on database failure
- `COLLECTING → SUCCEEDED` only after output validation and Asset persistence
- Idempotent `prj_default` project bootstrap and project asset directory
- Runtime Workflow Library package validation, version conflict protection, and sync
- Catalog ViewModel bridge exposing `textarea`, `integer`, `seed`, and `image` fields
- Non-blocking `generation_create`, persisted `task://updated` events, and task queries
- Asset ID-only query bridge with binary image IPC and Blob URL cleanup in React
- Dynamic Generation Studio, task progress card, and image output grid
- Source and generated image Asset categories with nullable `source_task_id`
- Native PNG/JPEG/WebP Source Asset import with SHA-256 and atomic storage
- Recipe `image` input, pure Image compiler values, and Application Input Preparer
- ComfyUI `/upload/image` multipart adapter with server-returned input identity
- Asset picker, recent image selection, and `ImageField` preview UI
- Mock I2I E2E covering validation, upload, snapshot identity, and prompt submission
- Task cancellation domain with `CANCEL_REQUESTED` and `CANCELLED` terminal handling
- Prompt-specific ComfyUI cancellation with modern API and safe legacy queue fallback
- Startup task recovery and manual history/queue reconciliation without automatic resubmit
- Cancel and recovery UI driven by persisted `task://updated` events, without frontend polling
- Workspace navigation between Studio, Assets, and Tasks
- Project-scoped task history with status filters, keyset pagination, detail views, and safe snapshot reuse
- Project-scoped Asset Library with category filters, keyset pagination, binary previews, and Blob URL cleanup
- Local-first historical input loading that never auto-generates a new task
- Project Repository CRUD foundation with stable ID-based project roots and metadata validation
- Active Project selector, persisted local context, and Projects workspace
- Project-scoped generation, task history, task events, asset listing, and binary reads
- Cross-project task cancellation and asset access rejected before side effects
- Generic `video` Recipe outputs using ComfyUI SaveVideo/PreviewVideo-compatible
  SavedResult normalization from the existing `images` history key
- Streamed generated video import with incremental SHA-256, atomic local file
  publish, optional ffprobe metadata/poster, and `generated_video` assets
- Atomic `task_output_assets` output mappings with restart-safe recovery and no
  duplicate output import
- Bounded Tauri 2 local video protocol with HEAD/single-Range seeking and
  project/asset isolation, plus inline video playback in Task Outputs and the
  Asset Library
- Source video and source audio assets with category-safe constructors,
  bounded streaming import, incremental SHA-256, atomic publish, optional
  media metadata, and best-effort poster generation
- Generic media-aware ComfyUI input upload through `POST /upload/image`, with
  server-returned input identity and no whole-file video/audio buffering
- Recipe `video`, `audio`, `videos`, and `audios` values with ordered binding,
  sequential upload preparation, media snapshots, preset/history compatibility,
  and project-scoped missing-asset validation
- Source video/audio playback through the bounded local media protocol and
  Asset Library source-media filters
- Batch foundation with independent per-item Task creation, partial-failure reporting,
  frozen frontend batch drafts, and a single-concurrency ComfyUI submission gate
- JSON task-list import and explicit transient-error `Retry Once` from reusable snapshots;
  `EXECUTION_ERROR` (including MiniMax H3 GPU OOM) is never quick-retried
- Persistent project-scoped production queues with ordered Task dispatch, pause/resume,
  restart recovery, uncertain-dispatch duplicate protection, and fatal execution-error stop
- Production queue control/observability with archive/restore/safe delete, explicit skip/requeue,
  event-driven status refresh, project production summary, and direct Task-detail navigation
- Global production admission allowing one persistent production batch across all projects,
  with active-item protection, interactive submission blocking, and deterministic legacy recovery

Batch Foundations 01–04 and `PRODUCTION VALIDATION 01` are PASS. Native Windows Cargo/pnpm regression is green, and a real four-item Kera2 persistent queue passed strict ordering, pause/resume, desktop restart recovery, Asset Library output, Archive/Restore/Delete, and transient-failure Skip/Requeue gates. Evidence is recorded in `docs/M1_PRODUCTION_VALIDATION_01.md`.

M1 fourth phase (Project Workspace + Project Isolation) code is complete. Historical task
inputs are exposed through a safe DTO boundary; raw workflow payloads, recipe
YAML, prompt IDs, storage paths, SHA-256 values, and asset metadata are not
returned to the frontend. Browse queries use the existing SQLite schema plus
`002_browse_indexes.sql`; no existing migration was changed.

M1 Project Workspace Final Gate validation is complete. It covers strict
Project ID hardening, UI project create/rename, active-project persistence,
new-project text-to-image generation, project-scoped Task/Asset isolation,
running-task switching, and the offline/restart gate. The exact evidence is
recorded in `docs/M1_PROJECT_WORKSPACE_VALIDATION.md`.

M1 Production Input Pack validation is complete. It adds project-scoped Preset
persistence and Studio controls, ordered multi-image recipe/application values,
best-effort source/generated thumbnails, Asset Library thumbnail fallback, and
historical draft compatibility. The exact evidence is recorded in
`docs/M1_PRODUCTION_INPUT_PACK_VALIDATION.md`.

M1 Video Foundation validation is recorded in
`docs/M1_VIDEO_FOUNDATION_VALIDATION.md`. It covers the generic video protocol,
streaming persistence, atomic task output mappings, bounded local playback,
video output cards, and the historical no-package gate that preceded the live
MiniMax H3 runtime package.

M1 Media Input Pack validation is recorded in
`docs/M1_MEDIA_INPUT_PACK_VALIDATION.md`. It covers source video/audio input,
generic media upload, media-aware Recipe compilation and preparation,
project-scoped playback, and the historical MiniMax H3 input-readiness gate.

The current ComfyUI live gate passed at `http://127.0.0.1:8188` with version
`0.30.2`, one NVIDIA GeForce RTX 5060 Ti device, and 4,485 nodes. The
offline/restart gate also passed: port 8188 became unavailable while AI
Studio stayed alive, and restarting ComfyUI restored the API.

The MiniMax H3 16 GB runtime gate is PASS. Immutable Workflow Package `1.1.2`
completed a real 5.167-second reference-to-video Task on the RTX 5060 Ti,
persisted the MP4 in Asset Library, and played it through the native Windows
desktop media protocol. The bounded profile uses the installed pruned NVFP4
UNet, 0.1 MP, four sampling steps, a 1–5 second Recipe range, and single-task
execution. Evidence and operating limits are recorded in
`docs/M1_MINIMAX_H3_RUNTIME_VALIDATION.md`.

Kera2 image generation and MiniMax H3 reference-to-video are now the two
live-validated production runtimes. No third model runtime pack is planned.

**AI Studio 0.1.0 Final Release Gate = PASS.** NSIS is the primary Windows
installer; the final release evidence, artifact hashes, clean install/data
preservation gate, and user-facing notes are recorded in
`docs/M1_FINAL_RELEASE_GATE.md` and `docs/RELEASE_NOTES_0.1.0.md`.

M2 Foundation Pack 01 adds migration-free persistent settings, a validated and
live-switchable ComfyUI endpoint, and validated project backup preview/export/
restore. It is development-only until the M2 gate is complete; no `0.2.0`
tag or GitHub Release has been created.

Runtime Workflow Packages are loaded only from
`%LOCALAPPDATA%/AIStudio/AIStudioData/workflow_library/`. Test fixtures are not
installed as runtime packages, and no model files are bundled or modified.

## Development

```text
pnpm install
pnpm tauri dev
```

The first launch creates `%LOCALAPPDATA%/AIStudio/AIStudioData/` with the SQLite database and runtime directories. The M0 screen verifies the Rust `ping` command, database initialization, and resolved data root.

Rust checks run from `src-tauri/`:

```text
cargo fmt --all
cargo check
cargo test
```

The default Rust test suite uses Mock ComfyUI HTTP/WebSocket services. It does
not submit compiler fixtures to a real ComfyUI instance. A live generation
smoke test is only appropriate when a validated workflow and recipe are
explicitly supplied through `AI_STUDIO_LIVE_WORKFLOW` and
`AI_STUDIO_LIVE_RECIPE`.
