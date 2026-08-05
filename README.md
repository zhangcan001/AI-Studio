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

Workflow submission, generation, queue management, WebSocket task execution, and task orchestration remain out of scope for this M0 persistence phase.
