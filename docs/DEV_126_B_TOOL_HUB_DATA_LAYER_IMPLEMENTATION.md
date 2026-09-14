# DEV-126-B — Local Tool Hub Data Layer Foundation Implementation

## Status

- Version baseline: AI Studio 1.3.1 Stable
- Baseline commit: b1fe7b5072fde0bec5a0f9a61dce9c2fbbb1ad9c
- Scope: SQLite metadata foundation, Rust domain/repository/service/command layers, and tests
- Frontend changed: No
- Execution authority changed: No

DEV-126-B adds a generic Local Tool Hub registry while preserving the existing
ComfyUI settings/runtime authority and the Production Queue execution boundary.
The registry records tool metadata and explicit observations; it does not run,
install, probe, or control a local process.

## Migration

Migration 035_local_tool_hub_data_foundation.sql is additive only. It adds:

| Table | Responsibility |
| --- | --- |
| tools | Canonical tool identity, type, description, metadata, and creation time |
| tool_instances | A locally configured path/endpoint and an explicit health observation |
| tool_versions | Append-only observed tool versions and metadata |
| tool_capabilities | Typed capability names and metadata for a tool |

The migration adds indexes for case-insensitive tool/capability lookup and
tool-child queries. Child foreign keys use ON DELETE RESTRICT, so a tool
with instances, versions, or capabilities cannot be deleted accidentally and
historical observations are not silently removed. A tool version is unique per
tool and version string; a capability is unique per tool and capability name.

Existing projects, assets, shots, tasks, reviews, and other v1/v2 tables are
not dropped or rewritten. Migration compatibility coverage verifies that
existing project, asset, shot, task, and review rows survive an upgrade
through migration 035.

## Domain Model

The canonical Rust domain is in src-tauri/src/domain/tool.rs:

- Tool: id, name, tool_type, description, JSON metadata, and created_at.
- ToolInstance: id, tool_id, optional filesystem path, optional endpoint,
  status, and optional last_checked.
- ToolVersion: id, tool_id, observed version, observed_at, and JSON metadata.
- Capability: tool_id, capability_name, and JSON metadata.
- ToolHealthStatus: AVAILABLE, MISSING, or UNKNOWN.

All input is validated before persistence. JSON values are serialized through
the existing repository helpers. The UNKNOWN initial state allows an instance
to be registered without implying that a process was checked.

## Rust Backend

The implementation follows the existing repository-port architecture:

- src-tauri/src/application/ports/tool_repository.rs defines the
  ToolRepository persistence port.
- src-tauri/src/infrastructure/database/repositories/tool.rs implements
  SQLite CRUD and child queries using the shared SQL/time/JSON/error helpers.
- src-tauri/src/application/tool_service.rs owns validation, identifiers,
  timestamps, and view conversion.
- src-tauri/src/commands/tool.rs exposes typed Tauri commands for tools,
  instances, versions, capabilities, and explicit health observations.
- src-tauri/src/app_state.rs and src-tauri/src/lib.rs wire the repository,
  service, and commands into the existing application.

Health recording accepts one of the three persisted statuses and timestamps
the caller-supplied observation. It contains no filesystem probing, endpoint
request, process start, process stop, installation, auto-run, or workflow
execution behavior.

## Compatibility Boundaries

- AppSettings/JsonSettingsStore remain the authority for existing ComfyUI
  configuration.
- ComfyAdapter/ComfyService behavior is unchanged.
- Queue, Task, Review, and Production Flow code is unchanged.
- No second executor, workflow engine, task model, or tool integration was
  introduced.
- The frontend and typed frontend transport are unchanged in this phase.

The generic registry can describe ComfyUI and other local tools without
replacing the existing ComfyUI service. A later Tool Hub UI may consume the
typed commands, but this phase intentionally has no frontend surface.

## Tests

Coverage added or updated:

- Domain validation for tool metadata, health statuses, and instance
  locations.
- SQLite repository coverage for tool CRUD, instance health observations,
  version queries, capability queries, and restricted parent deletion.
- Service coverage for CRUD, child queries, and the no-probing health
  observation path.
- Migration compatibility coverage through migration 035, including
  preservation of existing Project, Asset, Shot, Task, and Review rows.
- Existing migration/latest-version gates updated from 034 to 035.

Validation commands:

~~~text
cargo fmt --all -- --check
cargo check
cargo test
~~~

The full Rust gate passed before the implementation commit. Frontend validation
is not applicable because no frontend files were changed.

## Completion Criteria

- [x] Additive Tool Hub schema
- [x] Canonical Rust Tool/Instance/Version/Capability domain
- [x] Repository and service layers
- [x] Typed Tauri command boundary
- [x] Explicit health status handling without process control
- [x] Migration and persistence tests
- [x] Existing ComfyUI and Production Core authorities preserved
- [x] Full validation gate passed: cargo fmt --check, cargo check, and cargo test
