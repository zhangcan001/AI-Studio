# AI Studio v2 Local Tool Hub Compatibility Audit

```text
TASK=DEV-126-A
AI_STUDIO_VERSION=1.3.1_STABLE
AUDIT_BASELINE_SHA=7c3b6f1825ea210c436be14d3b19bac1f43a0e95
AUDIT_MODE=DOCUMENTATION_ONLY
CODE_CHANGED=NO
```

## Scope and decision summary

This audit determines where a future AI Studio v2 Local Tool Hub can extend the
existing application without creating a second configuration system, executor,
queue, or production authority. It does not implement a registry, migration,
adapter, process launcher, installer, UI, or health scheduler.

The repository currently has one real external-tool integration: the ComfyUI
HTTP adapter and its settings/service layer. That integration is intentionally
bounded to ComfyUI and remains the runtime and production execution authority.
There is no general Tool, Tool Version, Tool Instance, executable-path registry,
or external-process launcher in the product code.

The recommended future boundary is an additive, metadata-only registry for
personal tool identities and instances. It may record paths or endpoints,
observed versions, capability metadata, and health observations. It must not
execute, install, supervise, or orchestrate external tools.

## 1. Repository structure

| Area | Existing locations | Audit finding |
| --- | --- | --- |
| Frontend application | `src/app`, `src/components`, `src/features`, `src/services`, `src/stores`, `src/types` | React/TypeScript application with typed Tauri transport; no generic Tools feature or route. |
| Settings and ComfyUI UI | `src/features/settings/SettingsWorkspace.tsx`, `src/features/comfy/ComfyStatus.tsx`, `src/types/settings.ts`, `src/types/comfy.ts` | Existing settings workspace owns ComfyUI endpoint, environment profiles, status, and preflight display. |
| Rust domain/application | `src-tauri/src/domain`, `src-tauri/src/application`, `src-tauri/src/application/ports` | Existing repository/service ports are the extension pattern; no generic Tool domain module exists. |
| Rust commands | `src-tauri/src/commands/comfy.rs`, `src-tauri/src/commands/settings.rs`, `src-tauri/src/commands/mod.rs` | Commands are domain-specific. No Tool Hub command surface exists. |
| ComfyUI infrastructure | `src-tauri/src/infrastructure/comfy`, `src-tauri/src/application/ports/comfy_adapter.rs` | HTTP adapter, connection validation, health, capability inspection, queue, and runtime operations already exist for ComfyUI. |
| Settings persistence | `src-tauri/src/infrastructure/settings/mod.rs`, `src-tauri/src/application/ports/settings_store.rs` | `AppSettings` is persisted through the JSON settings store; current ComfyUI configuration authority is not SQLite. |
| SQLite persistence | `src-tauri/migrations`, `src-tauri/src/infrastructure/database/repositories` | SQLx migrations and repository abstractions own durable application data. No Tool Hub tables exist. |
| Runtime packages | `src-tauri/runtime_packages`, `src-tauri/src/application/builtin_runtime_packages.rs` | Workflow package/artifact provisioning, not a general external-tool registry or installer. |
| Documentation and validation | `docs`, `tests`, `.github` | Existing release, v2 planning, and hardening records provide the compatibility baseline. |

The release baseline document records `81b2045...` as the v1.3.1 source
baseline. The audit baseline above is the current `master` head, which also
contains the completed v2 Asset Library and Prompt Studio work and is the
actual starting point for DEV-126-A.

## 2. Existing Tool Authority audit

| Authority or capability | Status | Evidence and boundary |
| --- | --- | --- |
| Generic `Tool` entity/registry | **ABSENT** | No Tool domain model, registry repository, migration, command, type, or feature was found. |
| Generic `ToolVersion` / `ToolInstance` | **ABSENT** | No persisted installation/version identity or executable-path registry was found. |
| Generic external-app configuration | **ABSENT** | Configuration is owned by bounded domains rather than a shared tool configuration service. |
| ComfyUI endpoint and environment settings | **PRESENT, BOUNDED** | `AppSettings` and `JsonSettingsStore` own the ComfyUI endpoint and Comfy environment profiles. Profiles contain an id, name, endpoint, and timestamps; they are not generic Tool records. |
| ComfyUI runtime adapter | **PRESENT, BOUNDED** | `ComfyAdapter`, `ComfyHttpAdapter`, and `ComfyService` own connection, health, capability, queue, memory, upload, history, and workflow runtime operations. |
| ComfyUI health/capability observation | **PRESENT, BOUNDED** | `ComfyService` caches status/capability data and `ComfyPreflightService` exposes a read-only preflight report. |
| Runtime parameter profiles | **PRESENT, BOUNDED** | `RuntimeParameterProfile` and batch presets are workflow/recipe production settings in `AppSettings`, not Tool configuration. |
| Workflow runtime package library | **PRESENT, BOUNDED** | Built-in packages, manifests, artifacts, hashes, quarantine, and restore are workflow runtime concerns, not generic tool installation. |
| External production handoff | **PRESENT, BOUNDED** | Handoff manifests and artifacts describe production delivery; they do not register or launch external applications. |
| Product process launcher | **ABSENT** | No product-level `std::process::Command` launcher or generic external process authority was found. The build script's Git command is not a product integration. |
| Generic health service/scheduler | **ABSENT** | Database health and ComfyUI health are separate bounded diagnostics; no tool polling scheduler exists. |

### Existing authority rule

The current ComfyUI settings and runtime services must not be reimplemented as
a second Tool Hub configuration or executor. A future Tool Hub may expose a
reference or summary of ComfyUI, but ComfyUI's existing settings and adapter
remain authoritative for ComfyUI behavior.

The Prompt Studio `Model`/`ModelVersion` registry is authoritative for AI model
metadata and generation provenance. It is not an external application registry
and must not be merged with `Tool`.

## 3. Current AI tool coverage

The names below were checked against the current source and configuration. An
unlisted executable, installation, or level of support must not be inferred
from a planned record.

| Tool | Current coverage | Existing authority | Future audit disposition |
| --- | --- | --- | --- |
| **ComfyUI** | **INTEGRATED** through a typed HTTP adapter and settings/preflight UI. The endpoint accepts HTTP/HTTPS, so the current integration can point to a local or reachable service. | `AppSettings`/`JsonSettingsStore` for settings; `ComfyService`/`ComfyAdapter` for runtime behavior and health. | Register or display metadata only through a future Tool Hub; do not duplicate endpoint settings or move Queue execution. |
| **IndexTTS** | **NOT FOUND** in product source, migrations, commands, or frontend. | None. | Candidate future external TTS tool record; no integration or installed-state claim. |
| **ACE-Step** | **NOT FOUND** in product source, migrations, commands, or frontend. | None. | Candidate future external music tool record; metadata-only until separately approved. |
| **Agnes Creator** | **NOT FOUND** in product source, migrations, commands, or frontend. | None. | Candidate future external creator-tool record; no executable or process authority. |
| **VRBoxPlayer** | **NOT FOUND** in product source, migrations, commands, or frontend. | None. | Candidate future local media/player record; no player launch or playback integration. |

The four absent tools should not receive special-case adapters during
DEV-126-A. Their first compatible representation, if approved later, is a
user-managed metadata record rather than an assumption about their APIs,
installation paths, or versions.

## 4. Tool entity design

### 4.1 Recommended ownership model

```text
Tool (logical product/tool identity)
  └── ToolInstance (one local installation or reachable endpoint)
        ├── observed version metadata
        ├── capability observations
        └── last health observation
```

The registry is personal/local application metadata. It does not become a
project-owned replacement for Project, Task, Generation, Result, or Queue. A
future project reference may point to a Tool or ToolInstance, but production
execution remains governed by the existing workflow and Queue path.

### 4.2 Candidate entities

These are design candidates only; no schema is created by this audit.

| Entity | Candidate responsibility | Minimal fields/notes |
| --- | --- | --- |
| `Tool` | Canonical logical identity shared by instances | `id`, `name`, `kind`, `provider`, `description`, `metadata`, `created_at`, `updated_at`. `kind` distinguishes categories such as image, TTS, music, creator, or player. |
| `ToolInstance` | A local installation or endpoint for a Tool | `id`, `tool_id`, display name, normalized `path` or canonical `endpoint`, `observed_version`, status, last health-check time, non-secret metadata, created/updated timestamps. An endpoint-only tool does not require an executable path. |
| `Capability` | Capability metadata observed for an instance or version | Stable capability key, supported/status value, details metadata, capture time, and the owning instance. MVP may store details as JSON while retaining a queryable key. |
| `ToolVersion` | Historical/version-specific compatibility identity | Defer a separate entity until the product needs multiple installed versions, compatibility history, or version-level capabilities. MVP may keep `observed_version` on `ToolInstance`. |
| `Configuration` | Tool-specific operational settings | Keep each domain's existing configuration owner. Do not introduce a generic configuration table merely to mirror ComfyUI's JSON settings. Revisit only with an explicit one-owner migration. |

`Model` and `ModelVersion` remain separate canonical entities for Prompt Studio
AI model metadata. A model used by a tool is provenance, not proof that the tool
and model are the same entity.

### 4.3 Path, endpoint, and health semantics

- Store a normalized path or endpoint reference, never executable bytes, caches,
  or media assets in SQLite.
- Treat an endpoint as a first-class instance location because ComfyUI may be
  reached over HTTP/HTTPS and may not have a local executable path.
- Preserve the user-entered path/endpoint separately from normalized display
  metadata if later UX requires it; do not silently rewrite a user path.
- Represent missing, moved, or unreachable locations as observable status, not
  destructive deletion. A health check should update an observation and leave
  the identity/history intact.
- Do not persist credentials, tokens, or other secrets in plain JSON metadata or
  SQLite. Secret handling requires a separate approved security design.
- MVP health is an explicit observation or read of an existing adapter, not a
  background process, polling scheduler, or automatic repair system.

## 5. Database and storage audit

### 5.1 Current persistence authorities

The Rust application owns SQLite through SQLx migrations and repository
abstractions. Current durable domain data includes production, project, asset,
prompt, model, generation, and provenance records. The Prompt Studio model
foundation has `models` and `model_versions`; these are AI model authorities,
not Tool Hub tables.

The settings layer separately persists `AppSettings` through the JSON settings
store. ComfyUI endpoint settings, Comfy environment profiles, runtime
parameter profiles, queue presets, and related settings currently belong there.
The audit does not migrate or copy them.

No `tools`, `tool_instances`, `tool_versions`, `tool_capabilities`, or generic
tool configuration tables currently exist.

### 5.2 Future additive storage shape

If implementation is approved, the minimal SQLite addition should be additive
and separate from existing production tables:

```text
tools
tool_instances
tool_capabilities
```

Possible later additions are `tool_versions`, a domain-specific configuration
table after an explicit ownership decision, and durable health-observation
history if personal diagnostics require it. They are not part of this audit's
implementation.

SQLite is a good fit for the expected scale of personal metadata: dozens or
small hundreds of tools/instances, searchable names/tags/categories, and
bounded capability observations. It is not a suitable store for binaries,
model weights, media caches, process supervision, or high-frequency telemetry.

### 5.3 Configuration authority and migration preparation

The migration rule is:

```text
ADDITIVE_ONLY=YES
EXISTING_PROJECT_SHOT_TASK_QUEUE_REVIEW=UNCHANGED
COMFYUI_SETTINGS_AUTHORITY=APPSETTINGS_JSON_UNTIL_EXPLICIT_MIGRATION
DUAL_WRITE=NO
```

A future Tool Hub migration must choose one of two explicit paths before any
implementation:

1. Keep ComfyUI endpoint/environment profiles in `AppSettings` and store only a
   Tool Hub reference/summary; or
2. Perform a deliberate, tested, one-owner migration from the JSON settings
   authority to a new domain-owned configuration store.

The second path must include compatibility reads, rollback behavior, and a
clear cutover. It must not silently create two writable sources. DEV-126-A
selects the first path for the MVP.

## 6. Rust backend audit

| Existing area | Current responsibility | Future extension boundary |
| --- | --- | --- |
| `src-tauri/src/domain` | Domain types and invariants for existing production and v2 entities | Add a small Tool/ToolInstance/Capability metadata domain only in a later implementation task. |
| `src-tauri/src/application/ports` | Repository and adapter ports | Add a Tool repository port; preserve `ComfyAdapter` as the ComfyUI runtime port. |
| `src-tauri/src/infrastructure/database/repositories` | SQLx-backed persistence through repository abstractions | Add a Tool repository for additive tables; do not bypass repository ports with command-level SQL. |
| `src-tauri/src/application` | Services coordinate domain behavior and existing lifecycle authorities | Add metadata CRUD/health-observation service only; no process launch or executor service. |
| `src-tauri/src/commands/comfy.rs` | Typed ComfyUI status/settings/runtime commands | Remains the ComfyUI command authority. Do not duplicate these commands under a Tool namespace. |
| `src-tauri/src/commands/settings.rs` | Settings and Comfy environment/runtime profile commands | Remains the existing settings authority until an explicit migration. |
| `src-tauri/src/application/builtin_runtime_packages.rs` | Workflow package provisioning and artifact lifecycle | Remains separate from Tool registration and installation. |

The likely future implementation shape is:

```text
src-tauri/src/domain/tool.rs
src-tauri/src/application/ports/tool_repository.rs
src-tauri/src/infrastructure/database/repositories/tool.rs
src-tauri/src/application/tool_service.rs
src-tauri/src/commands/tool.rs
```

That shape is a planning boundary, not a request to create the files now. Any
future command surface should be limited to list/get/register/update metadata
and record an explicit health observation. It must not spawn a process, install
software, execute a workflow, or change Queue/Task/Review behavior.

## 7. Frontend audit

### Existing UI and transport

- `SettingsWorkspace` loads ComfyUI settings, Comfy status, environment
  profiles, and preflight data through typed service calls.
- `ComfyStatus` presents connection, version, device, node, and capability
  information in the existing visual language.
- `src/services/tauriClient.ts` and the IPC/typed transport layer provide the
  frontend access path; feature components do not directly access SQLite or
  issue raw Tauri commands.
- Settings navigation currently points to the Settings/Diagnostics workspace.
- No generic Tools page, tool store, tool types, tool route, or executable path
  management UI exists.

### Future frontend boundary

A later implementation may add a Local Tool Hub workspace that reuses the
existing Settings/Comfy visual language and typed transport. The smallest
useful read-only/basic-management surface would be:

```text
Tool List
  └ Tool / Tool Instance Detail
       ├ path or endpoint metadata
       ├ observed version
       ├ capability metadata
       └ health status / last observation
```

It must not add a second ComfyUI endpoint editor. ComfyUI settings should be
linked to or displayed from the existing settings authority. Loading, empty,
and error states should be part of a future UI implementation, but no frontend
files change in DEV-126-A.

## 8. Authority decision

The following ownership decisions prevent duplicate systems:

```text
Tool=NEW_CANONICAL_TOOL_REGISTRY (future; not implemented in DEV-126-A)
Configuration=EXISTING_DOMAIN_AUTHORITY (ComfyUI AppSettings JSON now; no dual-write)
Capability=COMFY_SERVICE_ADAPTER for ComfyUI; future persisted tool_capabilities for registered external tools
Model=EXISTING_PROMPT_STUDIO_MODEL_MODEL_VERSION_REGISTRY (separate from Tool)
Execution=EXISTING_PRODUCTION_QUEUE_AND_DOMAIN_ADAPTERS (not Tool Hub)
```

In particular:

- The future Tool registry owns logical Tool/ToolInstance metadata, not
  ComfyUI's operational settings.
- `AppSettings` remains the single writable authority for current ComfyUI
  endpoint/environment profiles.
- ComfyUI capability and health evidence continues to come from
  `ComfyService`/`ComfyAdapter`; other tools can later have metadata records
  without claiming live support.
- There is no dual-write, mirrored ComfyUI configuration, second executor, or
  second Queue.

## 9. Local Tool Hub MVP boundary

### In scope for a later implementation

- Canonical personal Tool registry.
- Tool Instance records for local paths or reachable endpoints.
- Basic metadata management: name, kind/provider, description, and location
  reference.
- Observed version information.
- Capability metadata and its capture time.
- Explicit health status/last observation, including unavailable or missing
  location states.
- Searchable/listable metadata through Rust repositories and typed frontend
  transport.
- A read-only or basic-management UI with loading, empty, and error states.

### Explicitly out of scope

```text
AUTO_EXECUTION=NO
PROCESS_LAUNCH=NO
AUTO_INSTALLATION=NO
WORKFLOW_ORCHESTRATION=NO
QUEUE_CHANGE=NO
TASK_CHANGE=NO
REVIEW_CHANGE=NO
BACKGROUND_HEALTH_SCHEDULER=NO
AI_AGENT=NO
CLOUD_SYNC=NO
MULTI_USER=NO
```

The Tool Hub is an inventory and explainability surface, not a replacement for
ComfyUI, IndexTTS, ACE-Step, Agnes Creator, VRBoxPlayer, the Production Queue,
or the existing workflow runtime.

## 10. Implementation readiness and risks

```text
IMPLEMENTATION_READY=YES
IMPLEMENTATION_RISK=MEDIUM
```

The work is ready for a separately approved implementation task because the
extension points and authority boundaries are clear. Risk remains medium for
these reasons:

1. **Configuration authority migration:** duplicating or prematurely moving
   ComfyUI JSON settings would create drift and break compatibility.
2. **Path and security handling:** local paths can become stale or moved, and
   secrets must not be persisted as ordinary metadata.
3. **Health semantics:** a last-known status must be distinguished from live
   availability without introducing an unbounded polling service.
4. **ComfyUI boundary:** the existing adapter already has runtime operations;
   the Tool Hub must reference it without becoming a second executor.
5. **Entity scope:** introducing ToolVersion, generic Configuration, or
   per-project tool ownership too early would increase schema and UI complexity
   without MVP value.

## Audit result

```text
TOOL_AUDIT=PASS
AI_TOOL_COVERAGE=ComfyUI existing; IndexTTS/ACE-Step/Agnes Creator/VRBoxPlayer not integrated
ENTITY_DESIGN=READY
DATABASE_AUDIT=PASS (SQLite suitable for additive metadata; current ComfyUI settings remain JSON)
BACKEND_AUDIT=PASS
FRONTEND_AUDIT=PASS
AUTHORITY_DECISION=PASS (single owner per domain; no dual-write)
MVP_BOUNDARY=Tool registry + path/endpoint + observed version + capability metadata + health status
CODE_CHANGED=NO
DEV_126_A=COMPLETE
DEV_126_B_STARTED=NO
AUTO_NEXT_TASK=NO
```

DEV-126-B may define the additive data-layer implementation after product
owner review. No implementation is started by this audit.
