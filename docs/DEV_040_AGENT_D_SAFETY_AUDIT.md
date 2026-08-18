# DEV-040 Agent D — No-GPU Safety / E2E / Architecture Audit

This note records the safety-owned test boundary for DEV-040. It is intentionally
separate from the feature implementation: Agent D adds tests and audit evidence
only. No AppState, lib.rs, migration, backup format, queue implementation,
executor, generation path, ComfyUI endpoint, installer, or release asset is
changed by this scope.

## Covered contract

- Scene A fixture: 12 shots classified as DONE=3, PREPARED=2,
  ELIGIBLE=6, BLOCKED=1.
- Strict prepare refuses to create a batch when any blocker exists.
- Explicit partial prepare includes only eligible shots and skips done,
  prepared, and blocked rows.
- Repeated prepare is idempotent at the (shot, stage) active-binding
  boundary; the race fixture permits only one binding per key.
- Video planning keeps the image-review gate manual: no selected image means
  IMAGE_REVIEW_REQUIRED and no video eligibility.
- Scene lookup is project-scoped; a Scene from another Project is not a valid
  production scope.
- 500 shots / 50 scenes are planned as 50 groups of 10. The sanity fixture
  prepares at most three scene scopes and never fans out 50 production batches.

## Architecture audit

The Rust and frontend tests require the Scene Production orchestration boundary
to reuse ShotBatchService and ProductionStructureService, and reject a second
Scene queue/executor, direct GenerationService or Comfy adapter access, direct
/prompt, and queue-start ownership inside Scene Production. The audit also
preserves the existing single ProductionQueueService owner, existing Queue
Start entry point, Prompt Template bulk commands, migration 021, and backup
version 12. The frontend source audit checks the busy-action guard and
disabled controls that prevent double-click submission.

The tests are deliberately no-GPU: they use deterministic fixtures, local
binding admission, and source audits. They do not claim ComfyUI, Krea2, H3,
REF2VA, installer, or production-runtime validation.

## Files

- src-tauri/tests/dev040_safety.rs
- src/features/stability/dev040Stability.test.ts

These tests are expected to run after the main DEV-040 implementation is
integrated. If the implementation paths are absent, the architecture tests
fail loudly instead of silently passing a missing safety boundary.

## Handoff finding

The frontend targeted gate passed: 1 file / 6 tests. The Rust targeted gate
reached compilation but is currently blocked by a core implementation error
outside Agent D scope: the Tauri command batch_workflow_preset_update exposes
the private BatchWorkflowPresetUpdateRequest type from
src-tauri/src/commands/batch_workflow_preset.rs. The main Agent must make that
request type visible before the Rust safety test can execute. Agent D did not
modify that file.
