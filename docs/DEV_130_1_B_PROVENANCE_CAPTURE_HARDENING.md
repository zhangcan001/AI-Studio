# DEV-130.1-B — Provenance Automatic Capture Hardening

## Scope and release boundary

```text
TASK=DEV-130.1-B
VERSION=1.3.1_STABLE
SCOPE=AUTOMATIC_GENERATION_PROVENANCE_CAPTURE
NEW_GENERATION_MODEL=NO
NEW_RESULT_MODEL=NO
NEW_QUEUE_MODEL=NO
HISTORY_GUESSING=NO
```

This change makes explicit provenance from the real generation success path
durable. It extends the existing `Task`/`GenerationSnapshot`, output mapping,
Asset, AssetVersion, Tool, ToolInstance, and ToolVersion authorities. It does
not create a second generation, result, queue, or executor system.

## Root cause

The Comfy execution path imported successful outputs into `assets` and
`task_output_assets`, then completed the Task. Generated outputs did not
automatically receive an initial `AssetVersion`, and completion did not carry
the caller's explicit Tool or Prompt context into the provenance service. As a
result, a successful generation could have no rows in
`generation_tool_usages` or `generation_asset_versions`.

## Implementation

### Explicit generation context

`GenerationService` now carries an optional
`GenerationProvenanceContext` through the prepared execution:

```text
generation_id       = existing Task.id
project_id          = existing Task.project_id
prompt_version_id   = caller-supplied exact identity, when present
model_version_id    = caller-supplied exact identity and existing snapshot FK
tool_instance_id    = caller-supplied exact ToolInstance identity, when present
tool_version_id     = caller-supplied exact ToolVersion identity, when present
```

Blank IDs are rejected. A ToolVersion cannot be supplied without an explicit
ToolInstance. Comfy endpoint, path, display name, runtime prompt ID, prompt
text, and timestamps are never used to infer any of these identities.

The existing `GenerationSnapshot.model_version_id` remains the canonical
ModelVersion provenance. The optional PromptVersion identity is retained as
explicit context for the capture boundary; the previously deferred standalone
PromptVersion-to-Task reverse relation is not introduced here.

### Generated AssetVersion

`insert_generated_outputs` now creates AssetVersion v1 in the same database
transaction as each generated Asset and its exact `task_output_assets` mapping.
The initial version copies the generated Asset's metadata, location, checksum,
and creation time. Source/manual asset insertion remains unchanged.

### Automatic success capture

After the existing output collector and importer succeed, the service reads the
exact output keys persisted for the current Task. This also safely covers an
output imported by a prior interrupted completion attempt; the existing
`task_output_assets` row is already an explicit fact, not a historical guess.
For each `(output_id, ordinal)` it:

1. resolves the existing output mapping for the Task;
2. resolves the current AssetVersion for that exact mapped Asset;
3. creates an `OUTPUT` `GenerationAssetVersion` relation idempotently; and
4. records `GenerationToolUsage` only when the caller supplied an explicit
   ToolInstance (and optional ToolVersion) that passes existing ownership
   validation.

Tool usage metadata records the capture boundary and the explicitly supplied
PromptVersion and ModelVersion IDs when present. No relation is batch-bound by
filename, filesystem path, prompt text, or time proximity.

## Lifecycle change

```text
Acquire GenerationExecutionLease
        ↓
Submit and monitor the existing Comfy execution
        ↓
Terminal SUCCESS
        ↓
Collect exact output keys
        ↓
Import outputs + create AssetVersion v1 atomically
        ↓
Capture exact GenerationAssetVersion edges
        ↓
Capture GenerationToolUsage when explicit Tool identity exists
        ↓
Persist telemetry + Task SUCCEEDED
        ↓
Return from GenerationService and release the existing lease
```

The DEV-130.1-A `GenerationExecutionLease` remains in scope through this
capture work. Queue Start remains the only production entry gate and the
Production Queue continues to call the existing GenerationService.

## Failure and cancellation handling

- Comfy submit, WebSocket, output collection, and output import failures return
  through the existing failure path before successful provenance capture.
- Cancelled or interrupted executions do not call successful capture, so they
  do not receive completed AssetVersion lineage.
- If explicit Tool identity is absent, the result is intentionally
  `tool_usage_recorded=false`; no ToolUsage is guessed from Comfy settings.
- If exact mapping or AssetVersion validation fails, Task success is blocked and
  the Task is failed with `PROVENANCE_CAPTURE_FAILED` rather than silently
  reporting a successful generation with incomplete capture.
- Repeated capture of the same exact output is idempotent; a conflicting
  existing AssetVersion relation is rejected.

## Tests

Added or strengthened coverage for:

- `backend_e2e_captures_explicit_tool_usage_and_prompt_context` — successful
  generation records explicit ToolInstance/ToolVersion usage, PromptVersion
  context, ModelVersion snapshot provenance, and one output lineage edge.
- `backend_e2e_imports_output_and_reaches_succeeded` — generated output gets
  AssetVersion v1 and an exact `generated_image:0` lineage edge.
- `capture_successful_generation_records_explicit_tool_and_asset_edges` —
  service-level explicit Tool and AssetVersion capture.
- `capture_without_tool_identity_does_not_guess_tool_usage` — no ToolUsage is
  created without an explicit Tool identity.
- `backend_e2e_missing_required_output_fails_without_assets` and the output
  import/download failure cases — failed generations retain zero successful
  lineage.
- `cancel_running_waits_for_execution_interrupted_then_cancels` — cancelled
  execution retains zero completed lineage.
- `cancellation_racing_success_preserves_result_and_records_not_effective` — a
  real successful terminal execution still records its output lineage even when
  cancellation races with completion.
- `generated_video_and_output_mapping_round_trip_atomically` — generated video
  output and its AssetVersion are persisted together.

## Compatibility and boundaries

```text
QUEUE_AUTHORITY_UNCHANGED=PASS
TASK_GENERATION_AUTHORITY_UNCHANGED=PASS
NO_NEW_EXECUTION_MODEL=PASS
NO_HISTORY_GUESSING=PASS
PERMIT_LIFETIME_PRESERVED=PASS
```

The change uses the existing provenance repositories and service. It does not
alter Queue, Task states, Review behavior, ComfyService behavior, or the
frontend execution flow. The typed frontend transport only accepts the
optional explicit context fields so existing callers remain compatible.

## Formatting drift

`cargo fmt --all` normalized the pre-existing archive formatting drift in
`src-tauri/src/application/project_backup_service.rs` and
`src-tauri/src/infrastructure/database/repositories/project_backup.rs` (plus
the formatting required by the new Rust changes). This is formatting-only in
the archive code; no archive behavior or schema was changed. The final
`cargo fmt --check` gate passes.

## Validation record

```text
FRONTEND_TEST=PASS (155 files, 849 tests)
TSC=PASS
BUILD=PASS
CARGO_FMT_CHECK=PASS
CARGO_CHECK=PASS (existing warnings only)
RUST_TEST=PASS (800 passed, 0 failed, 1 ignored)
REMOTE_CI_RUN=NOT_REQUESTED (master push does not trigger the tag-only Source-only workflow)
REMOTE_CI_STATUS=NOT_RUN
```

## Completion state

```text
PROVENANCE_CAPTURE=PASS
TOOL_USAGE_CAPTURE=PASS_WHEN_EXPLICIT
ASSET_LINEAGE_CAPTURE=PASS_EXACT_OUTPUT_KEY
MODEL_VERSION_CAPTURE=PASS_EXISTING_SNAPSHOT
NO_GUESSING_RULE=PASS
FAILURE_HANDLING=PASS_FAIL_CLOSED
DEV_130_1_B=COMPLETE_AFTER_COMMIT
DEV_130_1_C_STARTED=NO
AUTO_NEXT_TASK=NO
```
