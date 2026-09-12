# DEV-104 — Bulk Production Preparation & Operations Consolidation Result

```text
TASK=DEV-104
BASELINE=ab16b39c09e5955d6b310e427bb95fcc601742a7
THEME=Bulk Production Preparation / Operations Consolidation
IMPLEMENTATION_HEAD=26ea2fe4772e093690889b2a87685376233cadfb
DATABASE_MIGRATION=NO

BULK_PREPARATION=PASS
PROJECT_PREFLIGHT=PASS
PLAN_SCOPE_500=PASS
PREPARATION_BATCH_LIMIT_100=PASS
PREPARE_NO_START=PASS
CREATE_BATCH_NO_AUTO_START=PASS
STRICT_MODE_DEFAULT=PASS
EXPLICIT_PARTIAL_MODE=PASS
SERVER_REVALIDATION=PASS
IDEMPOTENT_PREPARATION=PASS
NO_SILENT_CHUNKING=PASS
EXACT_WORKFLOW_VERSION_RECIPE_PAIR=PASS
TASK_CREATION_FROM_PREPARE=NO
COMFY_SUBMIT_FROM_PREPARE=NO
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
AUTO_START=NO
AUTO_RETRY=NO
AUTO_REBIND=NO

FRESH_DB_001_TO_032=PASS
UPGRADE_1_0_TO_032=PASS
UPGRADE_1_1_TO_032=PASS
BACKUP_V18=PASS
PRODUCTION_REGRESSION=PASS

FRONTEND_TEST=PASS (147 files, 797 tests)
TSC=PASS
FRONTEND_BUILD=PASS
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_TEST=PASS (1,036 passed, 0 failed, 3 ignored across all targets)
TAURI_BUILD=PASS (MSI and NSIS bundles)
ARCHITECTURE_GUARD=PASS
RPC_PARITY=PASS (279 frontend commands checked)

CI_OPT_003_REMOTE_CI_REQUIRED=NO
REMOTE_CI_STATUS=SKIPPED_BY_POLICY (normal master push CI is disabled)

DEV_104=PASS
DEV_105=NOT_STARTED
STOP=YES
```

## Evidence

Project Production now has one typed, project-level `project_production_preflight`
request for up to 500 shots and one explicit `project_production_admit` request
for up to 100 selected shots. Both use the existing
`ProductionPreparationService`; admission re-resolves and live-preflights the
submitted IDs, preserves exact `workflowVersionId` + `recipeId` identity, and
creates only the existing READY batch/snapshot records.

`ShotBulkConfigPanel` exposes stage selection, project plan summaries, READY /
prepared / blocked / done filters, exact pair and reference visibility, strict
mode by default, and a visible partial-preparation opt-in. Over-limit selection
disables preparation and is never silently chunked. The panel and the legacy
`ShotBatchPlanner` no longer call queue start. Opening or starting the existing
Production Queue remains an explicit user action, so preparation creates no
Task and submits nothing to ComfyUI.

The mandatory create/start audit is recorded in
`docs/DEV_104_BULK_PRODUCTION_AUDIT.md`. The latest migration remains
`032_external_production_handoffs.sql`; DEV-104 added no migration or second
production runtime path. Full local frontend, Rust, architecture, build, and
Windows MSI/NSIS gates passed. Under CI-OPT-003, normal master pushes do not
wait for Source-only CI because this task did not explicitly require a remote
run.

## Roadmap state

`docs/roadmap/AI_STUDIO_POST_1_1_ROADMAP.md` marks Bulk Production
Preparation / Operations as `CLOSED (DEV-104)`. The next main theme is
Production Execution / Review Operations; DEV-105 remains not started.
