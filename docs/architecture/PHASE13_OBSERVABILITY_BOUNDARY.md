# Phase 13 — Observability & Diagnostics Hardening

## Status: LOCAL_GATES_VERIFIED — final-head CI pending

Baseline: `master@2795ffddf964e88f3c99af2ed3f284120da4b368`.
This checkpoint records completed local acceptance. It does not claim a release or final CI success; only the exact pushed HEAD's full Source-only CI can authorize final PASS.

## Reviewed implementation

- Read-only, project-scoped persisted diagnostic facts on the existing TaskRepository (window 50); no new repository, task/execution authority or persistence.
- Executable Task hydration/validation, producers and writes remain unchanged. Diagnostic facts can explain undated failed records and invalid telemetry that normal Task hydration intentionally rejects. The real SQLite regression checks both boundaries and persisted-state invariance.
- Missing timing remains null; invalid chronology is reported without mutation. Failure groups use stable persisted codes; missing finished_at gives latestAt=null, never created_at.
- Typed diagnostics transport and System Settings health/execution/failure/timeline/repair/export; exact existing Runs Task RunRef, Audit TASK root, and Workflow Lab diagnostics view. No duplicate run/lineage/workflow inspector.
- Failed probes are unknown, not healthy zero/idle. Failed summary refresh clears stale facts; request ownership rejects stale/unmounted/project-mismatched results. No new polling.
- Offline package diagnostics reads are bounded to five seconds and project unavailable counts as null. This is only a diagnostics read deadline, not an execution/readiness change.
- Bounded sanitized logs omit sensitive keys, credentials, paths, raw source and escaped JSON keys; safe machine/status codes remain. ZIP is local and limited to 25 MiB.
- Exact Phase8 → Phase12 backendOptimization2.sourceFreeze → Phase13 chain, explicitly reviewed paths and before/after hashes, baseline AND live untouched aggregates, full counts/aggregates and historical migration bytes. Immutable Git blob batching does not cache live source. Historical freezes are unchanged.
- CI frontend checkout fetches immutable baseline history for the same guards. Existing CI timeouts, test commands, test threads and cache strategy are unchanged.

## Verification ledger

Testing policy: NO_HARD_LIMIT; risk-driven coverage, no numeric execution ceilings.

| Coverage / reason | Result |
| --- | --- |
| Rust telemetry completeness/chronology, null aggregation, stable scoped failures, undated latestAt | PASS |
| Real SQLite diagnostic read vs strict executable Task boundary and no mutation | PASS |
| Summary unavailable/Comfy unknown/package deadline, ZIP/runtime-string privacy, log sanitization | PASS — 9 focused Phase13 Rust tests total |
| Diagnostics UI null values/exact callbacks, wrong/stale project/lifetime ownership, stale-summary refresh | PASS |
| Backend successor negative proofs and frozen retirement/IPC consumers | PASS |
| Library refresh/lifetime, Workflow preset, Runs and Audit/Settings affected behavior | PASS — 42 focused frontend tests total |
| Phase12 actual SQL parent-query and unchanged projection guard | PASS — 1 focused integration test |
| Architecture, raw invoke, IPC parity, exact Phase12/13 successor and frozen migration guards | PASS |
| TypeScript, frontend build, Rust format, cargo check all-targets -j1 | PASS |
| Tauri development compile and real isolated Native IPC | PASS |

Full local frontend suite: PASS (186 files, 1,008 tests, one worker). Full local Rust suite: NOT RUN; focused Rust/SQL and all-target check passed, and final remote full Rust suite remains mandatory. Final GitHub CI: PENDING, not substituted with any earlier run.

## Native acceptance

Real Tauri process, existing typed IPC, owned isolated data only, safe offline endpoint 127.0.0.1:1. No GPU generation, user project changes or ComfyUI intervention.

- Complete, partial, legacy, failed, undated-failure and invalid-chronology fixtures: PASS. Malformed fixtures are in a separate isolated project so the normal executable Task validation remains intact while valid Runs flows are exercised in the other project. Cross-project diagnostic facts remain isolated.
- Database health independent of offline Comfy; failed package probe bounded/unknown: PASS.
- Exact task Runs detail, Audit TASK root and existing open Workflow Lab diagnostics; Runs advanced details back to exact task Diagnostics: PASS.
- Manual refresh: PASS. Ordinary Overview/Create/Runs/Library/Project Settings hide technical identifiers: PASS.
- Native WebView responsive viewports 1920×1080, 1440×900, 1180×760: PASS, screenshots inspected; no horizontal overflow, diagnostics actions inside viewport. These are real Native renderer viewport checks, not claims of physically resizing the OS window.
- Real keyboard Enter/Ctrl+K, settings, advanced tools and back preserving original context: PASS.
- Actual ZIP export and inspection: PASS, 2,848 bytes; diagnostics.json, README.txt and sanitized dated logs only. Synthetic private prompt/token/path/source/snapshot probes absent; safe PHASE13_SAFE_CODE/status_code503 retained.
- Actual native save dialog cancellation: PASS, no partial/new file and no UI error. The first native export used the dialog's remembered fixture directory; the exact task-created ZIP was moved into the owned TEMP export directory after inspection, leaving no fixture modification.
- Real process close/restart (not reload): PASS, exact System Settings Diagnostics route and nested task RunRef/timeline restored.
- Logical hashes of all 71 named business tables before/after UI/export/cancel and after restart: identical. No business state mutation.

Resource checks were made before runners; compilation/testing was sequential with conservative parallelism. Only owned stale acceptance runners/Vite and owned Native windows were stopped. Existing compiler warnings were not changed.
Acceptance helpers, screenshots, fixture databases and ZIP remain outside commits. UI harness corrections fixed navigation synchronization/expected original return context and fixture metadata; they did not change production contracts or weaken source guards.

## Frozen boundaries and release gate

MAX_VERSION=42; named business TABLE_COUNT=71; 043_PRESENT=NO; BACKUP_FORMAT_VERSION=20.
Ancillary existing SQLite tables are not mistaken for new business schema.
No Queue/Task state machine/Run/GenerationSnapshot/Binding/OCC/Workflow/Repair/Audit authority changes, remote upload/telemetry, metrics database, telemetry repository or production profiler.

Before push: diff/scope review and fetch/divergence check. Normal master push only; no force/history rewrite.
After push: the exact HEAD full Frontend/Rust/architecture/performance/successor CI must succeed before Phase13 is final PASS. Next phase has not started.

## CI feedback correction

The initial Phase13 CI frontend gate failed on older test integrations: a historical non-CSS guard did not follow the validated Phase13 successor, the repair-status test mocked the old transport instead of the actual Diagnostics seam, an old assertion still suppressed the newly authorized read-only repair panel, and two cold structural Git/source checks exceeded Vitest's default five-second timeout under CI contention.

The follow-up changes only test adapters, exact reviewed test paths/hashes, guard scope metadata and this ledger. Historical manifests and all assertions remain; normal pages' Advanced transport ban now also covers diagnosticsClient. Two structural IO test deadlines are explicitly bounded at 30 seconds; product performance thresholds, CI job timeouts, thread counts, production code and test inclusion are unchanged.

Focused CI-failure regression targets: PASS (31 tests). Full local frontend: PASS (1,008 tests). Static/architecture/successor gates are rerun for this correction. Real Native/privacy acceptance remains applicable because production source did not change. A new final HEAD must run the full authoritative CI; the failed earlier run is not relabeled PASS.
