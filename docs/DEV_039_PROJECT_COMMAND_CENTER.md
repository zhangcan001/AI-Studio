# DEV-039 TURBO — Project Command Center + Post-0.6 Stability

## Result

`DEV-039 PROJECT COMMAND CENTER + POST-0.6 STABILITY PASS`

The project opens on a read-only Project Command Center. It aggregates existing project, production structure, shots, queue, task/asset, reference-anchor, prompt-template, audit, recent-activity, and cached ComfyUI/preflight state through one Tauri command:

`project_command_center_get(projectId)`

The first render does not call ComfyUI. The explicit `重新预检` action uses the existing preflight command, then refreshes the aggregate.

## Baseline and release freeze

- Baseline: `f9d1414b681c56037490bab12aa394db17e29368` (`docs: record AI Studio 0.6.0 publication`)
- Branch at start: `master`, clean, equal to `origin/master`
- Product version: `0.6.0`
- Migration: `021`
- Backup: `12`
- Manifest: `1`
- `v0.6` peeled SHA: `e3d7181f23a9b7285a426efb20ead4db17198757`
- `v0.5` peeled SHA: `02e67cff50f5da1d207478071636af166048820c`
- `v0.4` peeled SHA: `94918f6322ce690ff7b1630961abb56b8a31ed11`

No release tag, release asset, database migration, backup version, manifest version, or 0.6.1 publication was changed.

## Implemented surface

### Command Center aggregate

The backend response explicitly includes `project`, `readiness`, `content`, `production`, `issues`, `recentActivity`, `recommendedAction`, `quickActions`, and `checkedAt`, plus the detailed structure/shot/queue/task/asset/reference/prompt/comfy summaries used by the UI.

The aggregate uses set-based SQL reads. Shot rows and queue items are loaded in bulk and reduced in memory; scene progress is a single grouped query. No new queue owner, task-history owner, audit stream, workflow engine, database table, or migration was introduced.

### Continue Work

Recommendation is calculated in the backend with deterministic priority:

1. structural blocked
2. Comfy blocked while production is continuing
3. review required
4. auto resumable
5. active production
6. image review
7. video review
8. missing config
9. unassigned
10. no shots
11. ready
12. complete

Continue Work and all six quick actions are navigation-only. They do not submit, retry, delete, mutate queue state, or call ComfyUI.

### Workspace resume

`workspaceResume` is an additive `serde(default)` settings field. It stores the last project, workspace, and selected shot without a schema bump. Deleted projects fall back to the normal active-project resolver; deleted shots fall back to the first available shot or the Command Center. Save failures are convenience-state errors and never block navigation.

### Stability coverage

The DEV-039 no-GPU harness covers:

- 20 fresh project open/close cycles
- 30 reloads of a 500-shot data surface
- 20 project/workspace switches
- 100 cycles of 500-shot selection state
- 20 Command Center refresh cycles with deduplicated recent records
- 20 persisted workspace/project/queue restart cycles
- deleted project/shot/queue fallback

## Verification

Targeted checks passed before full regression:

- Rust Project Command Center tests: `5 passed`
- Frontend Project Command Center tests: `11 passed`
- Frontend DEV-039 stability tests: `7 passed`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: passed
- `pnpm build`: passed

Full regression passed once after this handoff document was added:

- `cargo check --manifest-path src-tauri/Cargo.toml`: passed
- `cargo test --manifest-path src-tauri/Cargo.toml`: `555 passed`, `0 failed`, `1 ignored`; integration suites `3 + 9 + 5 passed`
- `pnpm test`: `64 files`, `223 tests` passed
- `pnpm build`: passed
- `git diff --check`: passed

The final commit and push SHAs are recorded in the task result.

## Follow-up

- P0/P1: none known after the final regression.
- P2: existing Rust dead-code warnings and the existing Vite large-chunk advisory remain non-blocking; no speculative refactor was added.
- Next task text only: `DEV-040`.
