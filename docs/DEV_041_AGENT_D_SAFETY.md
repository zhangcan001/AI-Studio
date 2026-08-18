# DEV-041 Agent D — No-GPU E2E / Safety

## Scope

This gate adds only deterministic safety contracts. It does not add Episode
production functionality, start a Tauri runtime, contact ComfyUI, invoke a
GPU, run live generation, build an installer, publish a release, commit, or
push.

Allowed files for this Agent D handoff:

- `src-tauri/tests/dev041_safety.rs`
- `src/features/stability/dev041Stability.test.ts`
- `docs/DEV_041_AGENT_D_SAFETY.md`

No Agent A/B/C files were modified.

## Baseline

- Branch: `master`
- `DEV041_START_SHA`: `69fa3c94df18c9e413b96c7d2156aaa9708028a6`
- `origin/master`: `69fa3c94df18c9e413b96c7d2156aaa9708028a6`
- Product: `0.6.0`
- Migration: `021`
- Backup: `12`
- Active sub-agents created by Agent D: `0`

## Pure no-GPU fixture coverage

The Rust and frontend contracts cover the same Episode A fixture:

- 6 Scenes, with 50 actual Shots and one empty Scene
- `DONE=15`, `PREPARED=10`, `ELIGIBLE=23`, `BLOCKED=2`
- strict selection of Scene 2/4/5 produces zero mutation because Scene 4 is blocked
- partial selection produces 3 Scene-scoped batches and 23 items
- repeated prepare produces zero new items
- Episode+Episode and Episode+Scene races keep one active Shot/Stage binding
- Episode B Video plan keeps 10 eligible and 10 blocked behind manual image review
- Prompt context remains per Episode / Scene / Shot
- four-Scene, 40-Shot preset apply changes stage config only and preserves references and selected assets
- 500 Shots / 50 Scenes / 5 Episodes are planned with one tree-load contract per Episode
- five selected Scenes prepare to at most 5 batches / 50 items

All fixture operations are in-memory. `autoStarted` is explicitly false; no
`production_queue_start` or generation endpoint is called by these tests.

## Architecture safety audit

The Rust test checks the existing source boundary for the forbidden parallel
runtime concepts and verifies that the existing Scene layer still owns the
`ProductionStructureService` / `ShotBatchService` path. It also checks that
the existing application registers only one `ProductionQueueService` and does
not introduce `EpisodeQueue` or `EpisodeExecutor`.

At the Agent D writing point the implementation gate was intentionally pending.
The A-C implementation has now landed and the ignored Rust gate / Vitest todo
remain explicit no-GPU placeholders rather than hidden live-runtime checks.
The integrated results are recorded in
`docs/DEV_041_EPISODE_PRODUCTION_PLANNER.md`.

## Commands

Targeted, no-GPU commands:

```text
cargo test --manifest-path src-tauri/Cargo.toml --test dev041_safety -- --test-threads=1
pnpm test -- src/features/stability/dev041Stability.test.ts
```

The commands above do not start ComfyUI, a browser, a live provider, a queue
worker, or an installer.

Actual Agent D results at handoff:

- Rust targeted test: `9 passed / 0 failed / 1 ignored`
- Frontend targeted test: `8 passed / 0 failed / 1 todo`
- Agent D Rust format check: PASS
- The ignored/todo entries are the intentionally pending A-C integration gate;
  they are not reported as implementation passes.
- The broader repository currently also contains uncommitted Agent A/B
  changes; Agent D did not modify those files.

## Acceptance status

Agent D's pure fixture and existing-runtime safety contracts are integrated with
the Episode service, commands, panel, and wiring. The final no-GPU safety gate
passed with `9 passed / 0 failed / 1 ignored`.

STATUS: DONE
