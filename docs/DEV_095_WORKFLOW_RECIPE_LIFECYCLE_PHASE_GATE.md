# DEV-095 Workflow / Recipe Lifecycle Phase Gate

```text
TASK=DEV-095
BASELINE_SHA=235c9c54c53cf20f93d7f4aff2526a48a0d215f8
PHASE_GATE_HEAD=235c9c54c53cf20f93d7f4aff2526a48a0d215f8

PROMOTION=PASS
PROMOTION_CONSUMPTION=PASS
PROMOTION_CLEAR=PASS

RECIPE_ARCHIVE=PASS
RECIPE_RESTORE=PASS
RECIPE_HISTORY=PASS

EXACT_RECIPE_IDENTITY=PASS
HISTORICAL_REFERENCE_IMMUTABILITY=PASS

DIRECT_GENERATION_ADMISSION=PASS
QUEUE_CREATION_ADMISSION=PASS
QUEUE_START_ADMISSION=PASS
RUNNING_TASK_POLICY=PASS

PROJECT_BINDING_PRESERVED=PASS

NO_NEW_HISTORY_STATE=PASS
NO_AUTO_REBIND=PASS
NO_NAME_GUESSING=PASS

APPLICATION_DIRECT_SQLX_NEW_USAGE=0

NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES

FRESH_DB_001_TO_031=PASS
UPGRADE_DB_TO_031=PASS

LIFECYCLE_STRESS_10X=PASS
FRONTEND_LIFECYCLE_REPEAT_5X=PASS

FULL_FRONTEND=PASS
TSC=PASS
FRONTEND_BUILD=PASS

RUST_FMT=PASS
RUST_CHECK=PASS
FULL_RUST=PASS

TAURI_BUILD=PASS
ARCHITECTURE_GUARD=PASS
DIFF_CHECK=PASS

REMOTE_CI_RUN=#34567305896
REMOTE_CI_HEAD=235c9c54c53cf20f93d7f4aff2526a48a0d215f8
REMOTE_CI_STATUS=GREEN

WORKFLOW_RECIPE_LIFECYCLE_PHASE=PASS
FIX_COMMITS=NONE
NO_SCOPED_BUG_FIX_REQUIRED=YES
NO_SECRET_LEAK=YES
NO_UNEXPECTED_GENERATED_FILE=YES
SERENA_NOT_COMMITTED=YES
```

## Scope and frozen authority

DEV-095 is the final acceptance gate for the Workflow / Recipe lifecycle
implemented by DEV-090 through DEV-094. The audit found no scoped product bug
requiring a code fix. The exact `workflowVersionId + recipeId` pair remains the
identity for promotion, clear-promotion, archive, restore, admission, and
history queries.

- Promotion and clear-promotion remain explicit registry mutations; consuming
  generation and production paths use the authoritative promoted pair.
- Archive state is owned only by `workflow_recipe_runtime_states` from
  migration 031. Archive and restore affect the exact pair, preserve immutable
  Recipe rows and historical references, and do not auto-rebind or promote.
- Recipe History is read-only and on-demand. It derives bounded related facts
  from existing state; it adds no history table, migration, index, or competing
  state source.
- Existing Production Queue and Task paths remain the execution authority.
  Queue items with `task_id = null` remain planned-only references; no new
  queue, executor, or task model was introduced.
- Project binding and running-task behavior remain unchanged. No lifecycle
  operation guesses identity from names or latest rows.

## Database and migration audit

Existing migration regression helpers cover both a fresh `001 -> 031` database
and legacy upgrade fixtures through migration 031. The migration source and
repository tests preserve the exact composite identity, composite foreign-key
membership, and cascade behavior for promotion and runtime archive state.
No migration or history state was added after 031.

## Validation evidence

- `node scripts/dev088-architecture-guard.mjs` — PASS, including exact recipe
  identity, history read-only/on-demand, archive authority, no direct SQLx,
  and no-new-queue/executor/task-model sentinels.
- Focused lifecycle validation — Rust lifecycle/admission/migration paths
  repeated 10 times; frontend lifecycle/UAT/history paths repeated 5 times.
- Full frontend: `pnpm test` — 147 files, 790 tests passed; TypeScript and
  `pnpm build` passed.
- Full Rust: format check, `cargo check --all-targets`, and
  `cargo test --all-targets -- --test-threads=1` passed with 807 tests passed,
  0 failed, and 1 ignored.
- `pnpm tauri build` — PASS; MSI and NSIS bundles produced.
- `git diff --check` — PASS.
- Remote Source-only CI `#34567305896` was dispatched against
  `PHASE_GATE_HEAD`; Frontend source checks and Rust source checks both passed.
  The only annotation was the existing GitHub Actions Node.js 20 deprecation
  notice.

## Result

```text
DEV_095=PASS
DEV_096=NOT_STARTED
STOP=YES
```
