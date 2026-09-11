# Project Identity

- `project_id`: `ai-studio`
- Project: AI Studio

## Product and Architecture

- AI Studio is a Tauri 2 desktop application with a React/TypeScript frontend and Rust backend.
- SQLite persistence is owned by the Rust application layer through the existing repository abstractions and SQLx migrations.
- GitHub (`origin`) and the checked-out repository are the source of truth for the current implementation; memory is historical context only.

## Development Boundaries

- Keep the existing Production Queue path as the single production execution authority. Do not add a second queue, executor, or task model.
- Preserve Studio Store authority; controllers coordinate lifecycle but must not create a competing source of truth.
- Treat a workflow reference as the exact `workflowVersionId` + `recipeId` pair. Never infer identity from workflow or recipe names.
- Preserve project isolation for project-owned data and operations.
- Frontend features must use the typed transport in `src/services/tauriClient.ts` and `src/services/ipc.ts`; do not call raw Tauri `invoke` from components or feature modules.
- Rust application services must use repository ports and abstractions for persistence rather than bypassing them with direct database writes.
- Keep migrations backward-compatible and preserve historical task/production references when lifecycle behavior changes.
- Prefer repository and source inspection over stale memory when they disagree.

## Verification

- Match validation to the change: focused frontend tests plus TypeScript/build for frontend changes; focused Rust tests plus `cargo check` for Rust changes; IPC command parity and the raw-invoke guard for command/transport changes.
- Run the full frontend, Rust, and Tauri gates for cross-layer changes.
- Follow CI-OPT-003: local full gates are the default; run remote Source-only CI only when the task policy explicitly requires it.
- Use Serena for symbol-level code retrieval and existing architecture guards where applicable.

## Context Routing

- Store AI Studio-specific architecture decisions, bug roots, CI lessons, and compatibility findings under `PROJECT:ai-studio:*` memory.
- Keep cross-project rules in Global AGENTS or GLOBAL memory; do not copy project history into GLOBAL.

## Local Metadata

- `.serena/` is local Serena project metadata and must not be committed.
