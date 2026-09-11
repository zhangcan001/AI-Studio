# DEV-092 — Recipe Promotion Clear Result

```text
TASK=DEV-092
BASELINE_SHA=7e9b0bb0fec519e5899e9f49741b6f8855a7a641
IMPLEMENTATION_SHA=5f0f534ed05f48d3e06b6829c2cf70a70b1b677d

CLEAR_PROMOTION=PASS
CLEAR_PROMOTION_IDEMPOTENT=YES
STALE_CLEAR_SAFE=YES
ARCHIVED_VERSION_CLEAR_DOES_NOT_RESTORE=YES

EXPLICIT_RECIPE_AUTHORITY=PRESERVED
CURRENT_VERSION_AUTHORITY=PRESERVED
HISTORICAL_REFERENCE_IMMUTABILITY=PRESERVED
PROMOTION_AUTHORITY=WORKFLOW_REGISTRY
ONE_PROMOTION_CONSUMPTION_RULE=PRESERVED

DATABASE_MIGRATION=NO
RUST_SOURCE_CHANGE=YES
IPC_SURFACE_CHANGE=YES

LOCAL_FULL_GATE=PASS

REMOTE_CI_REQUIRED=YES
REMOTE_CI_RUN=#116
REMOTE_CI_HEAD=5f0f534ed05f48d3e06b6829c2cf70a70b1b677d
REMOTE_CI_INITIAL_RESULT=FAIL
REMOTE_CI_INITIAL_FAILURE_TEST=application::cancellation_e2e::cancel_after_upload_stops_before_snapshot_and_post
REMOTE_CI_RETRY=GREEN
REMOTE_CI_STATUS=GREEN

DEV_092=PASS
DEV_093=NOT_STARTED
```

## Implementation

- Added an exact `(workflow_version_id, recipe_id)` clear operation through the repository port, SQLite repository, registry service, Tauri command, and typed frontend client.
- Clear is idempotent, validates the workflow-version/recipe relationship, is stale-safe, and does not restore or re-enable archived workflow versions.
- Workflow Workspace refreshes authoritative workspace and catalog state after a successful clear; no optimistic promotion state mutation is used.
- Added the DEV-092 UAT coverage and `WORKFLOW_RECIPE_PROMOTION_CLEAR=PASS` architecture sentinel.

## Verification

- Local targeted promotion repository and frontend tests: PASS.
- Full frontend tests, TypeScript check, build, Rust format/check/tests, Tauri build, and diff check: PASS.
- Source-only CI #116 initial run had an unrelated timing failure in the existing cancellation E2E test. Only the failed Rust job was retried; the retry passed Rust format, Rust check, and Rust tests. Frontend remained successful.

## Frozen Invariants

```text
NO_DATABASE_MIGRATION=YES
NO_QUEUE_BEHAVIOR_CHANGE=YES
NO_EXECUTOR_BEHAVIOR_CHANGE=YES
NO_TASK_MODEL_CHANGE=YES
NO_GENERATIONSTUDIO_BEHAVIOR_CHANGE=YES
DEV_092=PASS
DEV_093=NOT_STARTED
```

This result commit contains documentation only.
