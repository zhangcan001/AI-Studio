# AI Studio

AI Studio is a Windows desktop foundation for a local AI image/video production workbench. M0 contains the Tauri 2 + React shell, Rust layering, SQLite migration, application data directory initialization, ComfyUI capability detection, and a pure local Recipe/Workflow compiler.

Current M0 progress:

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
- Catalog ViewModel bridge exposing only `textarea`, `integer`, and `seed` fields
- Non-blocking `generation_create`, persisted `task://updated` events, and task queries
- Asset ID-only query bridge with binary image IPC and Blob URL cleanup in React
- Dynamic Generation Studio, task progress card, and image output grid

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
