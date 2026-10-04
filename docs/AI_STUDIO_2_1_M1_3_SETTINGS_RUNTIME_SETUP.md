# AI Studio 2.1 M1-3 — Settings runtime setup and Tool Hub explanation

## Scope and authority

Parent: `1a66234867692f8b0ec9d6cbb587f9d5f71ddef7` (completed M1-2).
This checkpoint changes presentation and canonical navigation, not runtime authority.
M1-1 Create and M1-2 Overview production modules remain unchanged.

Settings now orders runtime preparation, ComfyUI configuration and readonly preflight
before ordinary status, execution diagnostics, tool metadata and advanced runtime/repair
operations. The first card includes in-page links to configuration and preflight plus
the current-project generator action. Existing diagnostic refresh/export and advanced
workflow diagnostics remain accessible. No second settings store or readiness service.

- Draft endpoint connection tests do not save/apply; explicit **保存并应用** retains
  the existing safe-switch implementation. Unapplied input is labelled separately.
- Saved-profile tests do not switch the active environment; explicit **应用** uses
  the existing profile service and its safety/busy checks.
- Preflight checks the applied environment's known dependencies, not generation
  success. It neither starts generation nor changes workflows/models.
- The project generator action targets the exact current project through
  `project-settings/generators`; without a project, explanatory text replaces it.
- The secondary Tool Hub action targets `system-settings/advanced-tools` with the
  current Settings route as `returnTo`. Existing canonical history and Studio Store
  remain authoritative, including nested Create return/draft preservation.

Tool Hub still reads only registered tools, instances, versions and capabilities.
Connected ComfyUI does **not** imply a registered ComfyUI tool. An empty registry
is valid, not evidence of a broken runtime connection. Health is the last explicit
observation, not live state; refresh only rereads registered metadata. No automatic
discovery, probing, installation, process launch/stop or health polling was added.

## Frozen boundary successor

`docs/architecture/m1-3-settings-runtime.json` records exactly five changed and four
added source/guard/test paths. The new bounded guard validates immutable parent
blobs, reviewed live hashes, complete file sets and untouched aggregates before
M1-2/M1-1 historical projections. Invalid proof fails closed. Twelve negative
mutations cover parent/path/hash/aggregate/count and operational-authority flags.
The M1-2 guard receives only the validated parent projection; its historical manifest,
M1-1 manifest and Phase8/10/12/13 historical manifests are not repinned.

No Rust, transport, CSS, migration, workflow fixture, queue/task authority, binding
OCC, telemetry, version, dependency or CI configuration changes. Migration maximum
remains 42, formal pool-test table count 71, no 043; backup stays v20 and application
version stays `2.0.0-personal`.

## Local evidence

Checks were sequential after RAM/VRAM inspection; no unrelated workload was stopped.

- Settings: 9 focused tests; Tool Hub: 8 (five existing plus three new).
- Canonical navigation: 3 new tests; existing route tests: 6; Settings UX: 1.
- Boundary/Create regression checkpoint: 46 tests across eight files, including
  unchanged M1-1/M1-2, historical successor/style guards and 21 Create tests.
- Final Settings/boundary check after Native layout adjustment: 10 passed.
- Final `pnpm exec tsc --noEmit`, `pnpm build`, `pnpm test:architecture`: passed.
  Architecture includes 330-command IPC parity, zero raw invokes outside transport,
  backend performance and Phase13 observability successor validation.

The first focused run found two test-selector ambiguities because both the existing
status card and endpoint form expose “测试连接”. The tests now scope to the endpoint
form; handlers and assertions were not weakened. No full local Rust/frontend rerun:
the task delegates the complete suite to the exact final-HEAD Source-only CI.
This document's local evidence is not a claim that pending remote CI has passed.

## Isolated Native evidence

One task-owned standard Tauri development session used a fresh data root and separate
WebView profile. A task-owned loopback helper served readonly object-info/system-stats
fixtures and an explicit offline mode; POST was rejected. User ComfyUI and user
settings/data were not modified; no GPU generation was performed.

Observed through real UI Automation:

1. First Settings view showed preparation/configuration ahead of deep telemetry;
   its configuration anchor reached the endpoint form and nearby preflight.
   The no-project state showed text rather than a dead generator button.
2. Editing/testing the helper endpoint showed unapplied explanation and a connected
   result while the isolated config directory still had zero files. Only clicking
   **保存并应用** created settings with that endpoint. Explicit preflight rendered
   a real warning report without producing tasks/batches.
3. Connected Settings opened Tool Hub with zero tools; metadata-only, empty-state,
   last-observation and refresh explanations were visible. Back returned to Settings.
4. With the default owned project selected, the generator action opened that
   project's image/video/reference generator settings without changing bindings.
5. In owned Create, a typed prompt and the current generator/shot survived
   **检查运行环境 → Settings → Tool Hub → Settings → Create** without discard or
   generation. Route/store tests additionally cover video/batch/shot identity,
   media input, dirty state and the existing saved return/run reference.
6. Reordered diagnostic entries remained available; export/privacy Native checks
   were reused from frozen Phase13 rather than repeated.

Readonly inspection after normal owned-session closure: tasks=0, production
batches=0, batch items=0, tools=0, tool instances=0; helper requests were GET only
(POST=0). One shot was created solely in the owned fixture. Native DB maximum
migration=42 and formal table count=71. The raw SQLite table total is 75 because
it also includes `_sqlx_migrations`, `workflow_runtime_states` and the two existing
external handoff tables; do not confuse that with the frozen pool-test allowlist.
Saved-profile switching is covered by focused contract tests, not claimed as an
additional Native profile-switch scenario. Owned app/helper sessions were stopped.

## Final gate and stop

Ordinary master push follows scoped diff review and remote race checking. The new
commit's exact-head Source-only CI must complete successfully (frontend tests,
architecture, TypeScript/build, Rust fmt/check/tests) before M1-3 and M1 can be
declared complete. Older M1-2 CI is not substituted. Final CI run/head/results belong
in the checkpoint chat report; no self-referential commit hash is embedded here.
Do not start M1-4 or M2/M3/M4 from this checkpoint.
