# AI Studio v2 Architecture Freeze Audit

```makefile
VERSION=v2.0.0-personal
ARCHITECTURE_STATUS=FROZEN
DATA_MODEL=STABLE
MIGRATION_STATUS=STABLE
RELEASE_BACKUP_VERSION=19
CURRENT_BACKUP_VERSION=20
PROVENANCE=STABLE
EXECUTION_CONTROL=STABLE
```

## Freeze decision

The v2 Personal Edition architecture is frozen at the existing additive
boundaries. The release contains no new execution engine, queue, task model,
generation authority, or alternate asset/result store.

## Boundary audit

| Area | Frozen authority | Result |
| --- | --- | --- |
| Production Core | Project, Shot, Task, Queue, Review, GenerationService, and Comfy execution admission | Stable; Queue Start remains the only production gate |
| Asset Library | Existing `assets` plus AssetVersion, AssetRelation, and explicit provenance extensions | Stable; project isolation and immutable history retained |
| Prompt Studio | Existing Prompt/PromptVersion plus Model/ModelVersion registry | Stable; no second prompt or generation system |
| Local Tool Hub | Tool, ToolInstance, ToolVersion, and Capability metadata | Stable; visibility only, no process control |
| Project Archive | v19 release baseline; v20 current additive extension | Stable; exact ID maps, media integrity checks, ArtifactReview preservation, and visible UNKNOWN state |
| Frontend boundary | Typed Tauri transport and existing feature services | Stable; components do not access SQLite directly |
| Persistence boundary | Rust repository ports backed by SQLite migrations | Stable; historical Project/Shot/Task/Queue/Review rows preserved |

## Data and migration status

The current migration chain is additive through the v2 data-layer migrations.
Existing v1.3.1 databases remain on the same upgrade path; fresh databases run
the complete chain. No migration in this freeze removes or renames historical
production tables or repairs relationships heuristically.

## Provenance and execution controls

The explicit supported lineage is:

```text
ToolInstance / ToolVersion
        ↓
Generation(Task) ← PromptVersion ← ModelVersion
        ↓
GenerationAssetVersion
        ↓
AssetVersion → Asset
```

Comfy admission is held until the real execution reaches a terminal state.
Success, failure, cancellation, timeout, submit errors, disconnect handling,
and application shutdown paths do not create a permanent permit leak.

## Production execution authority closure

```text
single executor != single execution authority
```

GenerationService remains the execution implementation used by the queue worker.
Production Queue Start is the sole product execution authority. Product
submission commands no longer invoke GenerationService directly.
`generation_create`, `generation_create_batch`, and `shot_generate` persist
items in the existing Production Queue; active one-click product surfaces then
call the official `production_queue_start` command. Remaining direct service
starts in `generation_e2e.rs` and `cancellation_e2e.rs` are test-only.

Evidence:

- `src-tauri/tests/production_execution_authority_boundary.rs::production_execution_requires_queue_start`
- `src-tauri/tests/dev052_runtime_integration.rs::direct_generation_submission_creates_no_task_or_comfy_submit_before_queue_start`
- `src-tauri/tests/dev052_runtime_integration.rs::direct_generation_batch_creates_n_tasks_and_submissions_only_after_queue_start`
- `src-tauri/tests/dev052_runtime_integration.rs::shot_generation_submission_keeps_linkage_pending_until_queue_start`
- `src-tauri/tests/dev052_runtime_integration.rs::task_history_retry_stays_queued_and_preserves_parent_attempt_and_project_scope`

## Freeze exclusions

The following remain outside the frozen architecture and require a new product
decision before implementation:

- autonomous Agents or automatic decisions;
- cloud synchronization or multi-user permissions;
- a new executor, workflow engine, or queue;
- heuristic historical lineage repair; and
- a schema rewrite or replacement of the current archive contract.

## Audit conclusion

```text
ARCHITECTURE_FREEZE=PASS
NO_NEW_DOMAIN_AUTHORITY=PASS
QUEUE_AUTHORITY=PASS
ARCHITECTURE_P0_STATUS=CLOSED
MIGRATION_COMPATIBILITY_BOUNDARY=PASS
RELEASE_ARCHIVE_BASELINE=BACKUP_V19_STABLE
CURRENT_ARCHIVE_CONTRACT=BACKUP_V20_ARTIFACT_REVIEW
```
