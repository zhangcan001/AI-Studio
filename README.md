# AI Studio

AI Studio is a Windows desktop foundation for a local AI image/video production workbench. M0 contains the Tauri 2 + React shell, Rust layering, SQLite migration, application data directory initialization, ComfyUI capability detection, and a pure local Recipe/Workflow compiler.

Current milestone progress:

M0 = PASS.

M1 progress:

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
video output cards, and the no-package NOT RUN gate for Generic Video and
MiniMax H3 live execution.

The ComfyUI live gate passed independently: `http://127.0.0.1:8188` reported
version `0.30.1`, one NVIDIA GeForce RTX 5060 Ti device, and 4,486 nodes. The
offline/restart gate also passed: port 8188 became unavailable while AI
Studio stayed alive, and restarting ComfyUI restored the API.

M1 live I2I is not run because no validated runtime I2I Workflow Package was supplied.
The existing M0 text-to-image runtime remains the only live generation package.

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
