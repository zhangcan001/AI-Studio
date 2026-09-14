# DEV-126-C — Local Tool Hub Frontend MVP Implementation

## Status

- Version baseline: AI Studio 1.3.1 Stable
- Data foundation: DEV-126-B complete
- Frontend changed: Yes
- Rust changed: No
- Scope: read-only Local Tool Hub overview and typed transport integration

DEV-126-C exposes the Local Tool Hub as a dedicated workspace. It reads the
metadata registry through the existing typed Tauri transport and presents tool
identity, instance locations, observed versions, capabilities, and explicit
health observations. It does not execute, install, probe, start, or stop a
process.

## Workspace and Navigation

The new Local Tool Hub workspace is available from the global studio rail as
the 工具 entry. The route is added to the workspace resume/navigation model and
uses the existing StudioShell layout. It is intentionally independent of the
active production project because Tool Hub metadata is personal/local
inventory, not Project/Shot/Task data.

Files:

- src/features/tools/LocalToolHub.tsx
- src/features/tools/LocalToolHub.css
- src/features/tools/LocalToolHub.test.tsx
- src/types/tool.ts
- src/services/tauriClient.ts
- src/app/App.tsx
- src/app/StudioShell.tsx
- src/app/studioNavigation.ts
- src/components/studio/StudioGlobalRail.tsx
- src/types/workspaceResume.ts

## UI Surface

### Tool List

The list renders:

- name
- type
- derived health status
- latest observed version
- path and/or endpoint location

Selecting a row opens its detail view. Status is derived from the persisted
instance observations: AVAILABLE takes precedence when any instance is
available, then MISSING, otherwise UNKNOWN.

### Tool Detail

The detail view renders:

- tool description and metadata
- instance locations
- per-instance health and last observation time
- capability names and metadata
- observed version history, newest first
- a read-only health summary

Missing or unregistered locations remain visible rather than being deleted.
The page explicitly explains that health is last-known metadata and that no
runtime probe or process control is performed.

### State Handling

The page handles:

- loading state while the registry or child records are read
- empty state when no tools are registered
- list errors with a user-facing retry via Refresh
- per-tool detail errors with a scoped retry
- MISSING and UNKNOWN health states
- empty instance, capability, and version collections

## Data Access Boundary

The frontend uses only typed wrappers in src/services/tauriClient.ts:

- listTools
- listToolInstances
- listToolVersions
- listToolCapabilities

Components do not call raw Tauri invoke, SQLite, filesystem APIs, ComfyUI
endpoints, or process APIs. The existing Rust ToolService and repository
remain the authority for validation and persistence.

## Compatibility

- AppSettings and ComfyService remain the existing ComfyUI authorities.
- Queue, Task, Review, and Production Flow are unchanged.
- No executor, installer, workflow engine, auto-run, or auto-generation path
  was added.
- Version history is read-only.
- No second Tool, Model, Prompt, or Generation system was introduced.

## Tests and Validation

Frontend tests cover:

- tool list rendering and required columns
- detail rendering
- capability display
- observed version display
- health and missing-tool states
- loading, empty, and list-error states
- typed transport calls

The focused Tool Hub/navigation/localization tests passed:

~~~text
4 test files, 14 tests passed
~~~

The complete frontend suite passed:

~~~text
153 test files, 840 tests passed
~~~

Required checks passed:

~~~text
pnpm test
pnpm exec tsc --noEmit
pnpm build
~~~

Rust files were not modified, so Rust validation is not part of this
frontend-only phase.

## Limitations

This MVP is a browse-and-understand surface with refresh and scoped retry.
It intentionally does not add process actions, automatic health checks,
installation, tool execution, or a second settings authority. Future
metadata registration/editing can be added only through the existing Tool
commands and without changing the execution boundary.
