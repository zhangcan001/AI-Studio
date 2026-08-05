# AI Studio

AI Studio is a Windows desktop foundation for a local AI image/video production workbench. M0 contains only the Tauri 2 + React shell, Rust layering, SQLite migration, and application data directory initialization.

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

ComfyUI integration, generation, workflow compilation, recipe parsing, and task orchestration are intentionally out of scope for M0.
