# Phase10 frontend consolidation

Inventory precedes production migration. Baseline: Phase9, with backend, route
contracts, persistence formats, IPC, CSS and migrations unchanged.

## Selected seams (4)

1. Application normal-page composition: normal Create/Runs/Library rendering and
   lazy imports move to one route container. The existing AppRoute remains sole
   route authority. No DOM wrapper is added. Advanced composition stays explicit
   in App, deferred rather than hidden in a God Controller.
2. Shot consistency controller: project-scoped option requests, binding callbacks
   and cleanup move out of App into the existing shots feature. Existing services
   remain authority. It stays mounted for the application lifetime to preserve
   preloading and Advanced state; no route-dependent extra subscriptions.
3. Project task recovery: recent-task loading and manual reconciliation move to
   tasks. TaskStore remains the only task projection. Application task-event
   bootstrap stays global (it serves more than one feature).
4. Generator presentation vocabulary shared by Create and generator settings.
   Move the already-identical labels to product presentation, preserving Create's
   compatibility reexports and removing settings' cross-feature model dependency.

## State/effect ownership

| Domain | Authority / local ownership | Classification |
| --- | --- | --- |
| Application | bootstrap readiness, endpoint capability and global task event subscription | FEATURE_STATE / APPLICATION_LIFECYCLE |
| Project | ProjectStore; AppRoute project location; resume adapters only persist outputs | PROJECT_STATE / PERSISTED_STATE |
| Studio/Create | StudioStore draft and pending intents; existing CreateController request epochs | FEATURE_STATE / SERVER_DERIVED_STATE |
| Shots | ShotWorkspace editing; consistency controller options; no second persisted shot | LOCAL_EPHEMERAL / SERVER_DERIVED_STATE |
| Runs/Tasks | TaskStore; RunsController scoped server projection; task recovery controller request ownership | SERVER_DERIVED_STATE |
| Library/Assets | LibraryController scoped projection; existing asset controllers | SERVER_DERIVED_STATE |
| Prompts | LibraryController projection and explicit Advanced PromptStudio editor | SERVER_DERIVED_STATE / LOCAL_EPHEMERAL |
| Workflow Lab | existing useWorkflowLabController; registry on backend | SERVER_DERIVED_STATE |
| Settings | existing SettingsWorkspace operation state; existing backend settings | LOCAL_EPHEMERAL / PERSISTED_STATE |
| Advanced | retain existing controllers and explicit route guards | FEATURE_STATE |
| Dialogs/Overlays | existing draft-confirmation hook; feature-local dialogs stay in features | LOCAL_EPHEMERAL |

No duplicate authority was proven: do not delete state just to reduce metrics.
Contexts found: zero createContext sites; do not introduce a new global provider.
Prop handoffs for the new page container are one level, four explicit props.
Inventory property-chain depth is an AST proxy, not actual prop-drilling depth.
Large/God candidate counts identify review candidates, not automatic violations.

## Guard successors

Phase9 CSS pins and debt budgets remain exact. Only explicitly listed Phase10
seam production paths may receive reviewed frontend byte successors. Phase8
App compatibility pin follows that scoped successor; all backend, DTO, route,
persistence and remaining frontend pins remain unchanged. Import debt is
explicit and non-expanding; removal does not authorize replacement debt.
No max-lines rule, new transport, state library or modal framework.

## Verification budget

Ten focused local cases maximum: task recovery project switch/stale reply,
reconciliation, unmount cleanup; consistency project switch/stale reply, unmount,
binding callbacks; normal routes/modal handoff; architecture negative fixtures;
existing backend compatibility and Phase7 route composition regression.
No full local suite. TypeScript/build, DEV-088 and fmt/check separately.
Native isolated route/project/real-restart acceptance and responsive three widths
are required before PASS; exact final-head master Source-only CI is full-suite
authority. Until those finish, this phase is not PASS.

## Verified local checkpoint

The four seams are production-migrated. App changed from 973 to 827 lines;
local root state sites 21 to 13, root effect sites 7 to 5. Two selected feature
effects moved; the Advanced video-batch reset remains explicitly retained.
Global effect sites increased 242 to 243 due to the task request epoch cleanup.
Cross-feature imports fell 118 to 117; shared generator labels preserve their
existing Create reexports. Six existing stores remain authoritative.

App retains its established compatibility-facade importer and supplies stable
typed service ports to the two hooks. This is composition wiring, not another
IPC layer or an expansion of the legacy transport allowlist.

Ten distinct focused local cases passed (nine new, one existing backend boundary);
only existing cases were rerun. No repository-wide test suite ran locally.
TypeScript/build, DEV-088, cargo fmt/check passed. Rust warnings were not changed.
The Native evidence artifact records 40 page/width pairs, three dialog widths,
three picker/reference widths, exact task/queue/resource details, Advanced paths,
legacy Lab, keyboard/Back, one actual IPC load per project-scoped seam, and a real
close/relaunch preserving route and an isolated settings profile. Both owned
Native processes are closed. Historical Phase9 sampled computed styles and
rectangles matched at all40 pairs; this is not an exhaustive pixel comparison.

The backend's migration test counts 71 explicitly named domain tables, not all
SQLite tables: the isolated database has 75 non-sqlite tables including ancillary
tables. MAX migration remains42, no043, backup20.

The exact final-head remote CI result is the remaining authority gate; recording
its outcome in chat avoids an infinite documentation-commit/CI-head loop.
