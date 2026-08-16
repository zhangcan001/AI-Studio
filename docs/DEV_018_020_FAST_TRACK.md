# DEV-018-020 FAST TRACK Validation

Date: 2026-08-16

## Baseline

- Repository: `zhangcan001/AI-Studio`
- Branch: `master`
- DEV-018-020 start SHA: `7982ddc53df7f2fd46df4128147f1f6bc311dc74`
- Published `v0.3.0` remains present and untouched.
- Published runtime package files were not changed.

## DEV-018 — Final compiled workflow validation

`FinalCompiledWorkflowValidator` now runs after final media compilation and before snapshot persistence or `POST /prompt`.

The validator rejects, with structured compile error codes:

- internal placeholders in the final workflow;
- dangling node references and invalid output indexes;
- incomplete or mismatched uploaded-media bindings;
- missing recipe-declared output nodes or output graph reachability.

The final compiled workflow receives a deterministic SHA-256, and the validation event records the template workflow SHA, recipe SHA, and compiled SHA. Static compatibility validation remains active; dynamic Recipe binding targets are handled through the Recipe-aware path.

## DEV-019 — Submission recovery hardening

- Generation requests accept a client submission idempotency key.
- A serialized idempotency gate and repository lookup prevent duplicate task creation for the same submission attempt.
- The task records its deterministic execution identity before submission.
- `TaskSubmissionPrepared` is persisted before `POST /prompt`; `TaskSubmissionConfirmed` is persisted only after a matching Comfy prompt id is returned.
- Unknown submission outcomes are recorded as `SUBMISSION_STATE_UNCERTAIN`; the service does not blindly resubmit.
- Recovery records the uncertain state and defers resubmission until an explicit, safe recovery decision.
- Frontend generation actions reuse one key for the lifetime of an in-flight attempt.

## DEV-020 — Telemetry and scheduling foundation

- Added migration `016_generation_telemetry.sql` only. Migrations `001–015` remain immutable and unchanged.
- Added nullable task telemetry columns for execution identity, compiled workflow SHA, runtime profile/class, and prepare/submit/execute/collect timestamps.
- Derived phase durations are null when a phase was not observed; no unavailable phase is represented as a fabricated zero.
- Added exact workflow-id scheduling classification for Krea2, H3 Fast, H3 Quality, and H3 Ref2VA profiles. The existing executor and queue remain serial; every current profile has `max_concurrent = 1`.
- Task history Runtime Diagnostics exposes execution identity, compiled SHA, profile/class, phase timestamps, and derived durations.
- Benchmark candidates can read compiled SHA, profile, and timing telemetry.
- `BACKUP_VERSION` is now `8`; versions `1–7` remain accepted, and telemetry is included in backup/restore with legacy nullable compatibility.

## Architecture invariants

- One production submission path remains: `GenerationService` compiles and validates, then uses the existing Comfy adapter for `POST /prompt`.
- One existing `ProductionQueue` and executor remain in use.
- No second executor, multi-GPU scheduler, third runtime, or Krea2-to-H3 pipeline was added.
- No published runtime package bytes were changed.
- No `v0.3.0` tag was moved and no Release asset was replaced.

## Final regression

Executed from `src-tauri` where applicable:

- `cargo fmt --all -- --check` — PASS
- `cargo check` — PASS
- `cargo test -- --test-threads=1` — **446 passed, 0 failed, 0 ignored**
- `pnpm test -- --run` — **46 files passed, 152 tests passed**
- `pnpm build` — PASS
- `git diff --check` — PASS

The frontend build emitted only the existing Vite chunk-size warning; it did not fail. `pnpm tauri build` was intentionally not run because this sprint does not rebuild or replace the already published 0.3.0 installer artifacts.

## Known issues / deferred work

- Blockers: none found by the code gate.
- GPU Live Validation remains product-owner controlled and is not represented as a code-gate pass.
- Benchmark 2.0 comparison UX is the next scoped task, not part of DEV-018-020.

## Decision

**DEV-018-020 FAST TRACK PASS**

Next step: `DEV-021 — Workflow Benchmark 2.0 + Performance Comparison`.
