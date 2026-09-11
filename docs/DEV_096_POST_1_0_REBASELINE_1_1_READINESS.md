# DEV-096 Post-1.0 Rebaseline & AI Studio 1.1 Readiness Gate

```text
TASK=DEV-096
BASELINE=a66a233d14c03c08153b4de8e04449d303cc952a
PUBLISHED_1_0_SOURCE_RC=84f06d03e07b522cb5c6466e4595deb14035fd70
PUBLISHED_1_0_PUBLICATION_RECORD=8e4ed13f5f809fc1725f20a35b95dac6e7001750
PUBLISHED_1_0_TAG=v1.0.0
CURRENT_SCHEMA=031
READINESS_CODE_HEAD=4b8d70cf52fece182cce94bfe93d3758510b9c31

POST_1_0_CHANGE_AUDIT=PASS
SEMVER_CLASSIFICATION=MINOR
NEXT_VERSION_RECOMMENDATION=1.1.0

UPGRADE_1_0_0_TO_031=PASS
FRESH_001_TO_031=PASS
PROJECT_DATA_PRESERVED=PASS
TASK_DATA_PRESERVED=PASS
QUEUE_DATA_PRESERVED=PASS
ASSET_DATA_PRESERVED=PASS
WORKFLOW_DATA_PRESERVED=PASS
RECIPE_DATA_PRESERVED=PASS
PROJECT_WORKFLOW_BINDING_COMPATIBILITY=PASS
WORKFLOW_REGISTRY_V2_COMPATIBILITY=PASS
BACKUP_COMPATIBILITY=PASS
RUNTIME_PACKAGE_COMPATIBILITY=PASS
PRODUCTION_PACKAGE_V1_COMPATIBILITY=PASS

NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES

FULL_FRONTEND=PASS
TSC=PASS
FRONTEND_BUILD=PASS
RUST_FMT=PASS
RUST_CHECK=PASS
FULL_RUST=PASS
TAURI_DEVELOPMENT_BUILD=PASS
ARCHITECTURE_GUARD=PASS
DIFF_CHECK=PASS
RUST_FOCUSED_REPEAT_5X=PASS
FRONTEND_UAT_REPEAT_3X=PASS

REMOTE_CI_RUN=#34570210019
REMOTE_CI_HEAD=4b8d70cf52fece182cce94bfe93d3758510b9c31
REMOTE_CI_STATUS=GREEN

FORMAL_RELEASE_CREATED=NO
VERSION_BUMP=NO
DEV_096=PASS
```

## Authority and result

DEV-096 rebaselines the post-1.0 `master` line against the published 1.0.0
source RC and publication record. The repository remains at manifest version
`1.0.0`; this gate does not publish a new release, create a tag, upload an
asset, or start DEV-097.

The post-1.0 changes are additive and backward-compatible. They extend
project workflow binding and readiness, universal workflow onboarding, the
Workflow Registry V2/runtime artifact model, application and backup
boundaries, typed IPC errors, controller coordination, recipe promotion and
consumption, exact recipe archive/restore, and read-only recipe history. They
do not replace the existing Production Queue, executor, or task model, and no
external breaking IPC contract was found. The SemVer classification is
therefore `MINOR`, with `1.1.0` recommended for the next release after a
future release-candidate gate.

## Post-1.0 change audit

The audit from `PUBLISHED_1_0_SOURCE_RC` through `BASELINE` and the readiness
code head covered the following additive areas:

- Project workflow bindings, preflight, production readiness, and start
  admission preserve project isolation and exact workflow references.
- Universal workflow add and Workflow Registry V2 keep logical workflow,
  immutable version, exact recipe, and runtime artifact identity separate.
- Runtime artifact reconciliation, lifecycle purge/recovery, and package
  handling do not guess identity from names or legacy package metadata.
- Application data and project backup boundaries remain repository-backed;
  typed IPC transport and structured errors remain the frontend boundary.
- Controller extraction coordinates existing authorities rather than creating
  a competing store, queue, executor, or task model.
- Recipe promotion/consumption/clear, archive/restore, and history all use
  the exact `workflowVersionId + recipeId` pair. History is read-only and
  on-demand.
- CI and test stabilization changes remain validation/tooling changes and do
  not alter the published manifest version.

Migrations 027 through 031 are additive/non-destructive for the audited
contracts. Migration 027 adds project workflow bindings; 028 adds Registry V2
metadata and runtime artifact scaffolding; 029 reconciles provisional runtime
artifacts without inventing canonical identity from a legacy `package_name`;
030 adds exact recipe promotion state; and 031 adds exact recipe runtime
archive state. No destructive schema change or second lifecycle state source
was introduced.

## Compatibility evidence

### Database upgrade and preservation

The existing fresh migration matrix reaches schema 031 from 001. The new
`dev096_reconstructed_1_0_fixture_upgrades_from_026_to_031` test in
`src-tauri/tests/dev055_release_compatibility.rs` reconstructs a published
1.0.0-era database at schema 026 and runs the real migrator through 031. It
preserves project, shot, asset, task, task event, task snapshot, production
batch/item, production package binding, preset, workflow, workflow version,
recipe, benchmark, production run/template/stage/item, exact IDs, and exact
workflow/recipe SHA values. It also verifies that the upgrade does not invent
project bindings, promotion rows, archive rows, or a runtime artifact for a
legacy package-only row.

This covers the 1.0.0-to-031 upgrade path as well as the existing 001-to-031
fresh path. Project workflow binding, Registry V2, runtime package, and
Production Package V1 compatibility remain green.

### Backup contract

The current project backup contract is `BACKUP_VERSION=17`. Backup 17 includes
project-scoped workflow bindings and project-scoped workflow registry/runtime
artifact data, and the existing round-trip and legacy fixture tests remain
green, including real Backup 12/13 inputs, fixed versions 1 through 13,
rollback, Zip Slip rejection, and exact asset remapping.

Promotion and recipe archive state are global runtime lifecycle state, not
project-owned backup state. Consistent with DEV-093, Backup 17 intentionally
does not serialize `workflow_recipe_promotions` or
`workflow_recipe_runtime_states`; restoring a project reactivates only the
exact recipe when applicable and does not promote, rebind, change history, or
silently mutate global runtime lifecycle state. This is the preserved Backup
17 compatibility contract, recorded as:

```text
BACKUP_LIFECYCLE_SCOPE=GLOBAL_RUNTIME_STATE_NOT_PROJECT_BACKUP
```

### Production compatibility

Existing production package bindings, queue batches/items, task lineage, and
runtime admission remain on the established production path. Queue items with
`task_id = null` remain planned-only references, and the audited changes add no
new queue, executor, or task model. The exact workflow reference remains the
`workflowVersionId + recipeId` pair throughout inspection, admission, queue
creation, and execution.

## Validation evidence

- `pnpm test -- --reporter=dot` — PASS, 147 files and 790 tests.
- `pnpm exec tsc --noEmit` — PASS.
- `pnpm build` — PASS; the existing Vite chunk-size advisory remains
  non-fatal.
- `cargo fmt --all -- --check` — PASS.
- `cargo check --all-targets` — PASS.
- `cargo test --all-targets -- --test-threads=1` — PASS, 808 passed, 0 failed,
  1 ignored.
- Critical Rust compatibility/lifecycle/admission/backup tests — PASS for 5
  consecutive repetitions.
- Critical frontend workflow/project/recipe-history/production UAT — PASS for
  3 consecutive repetitions, 6 files and 44 tests per repetition.
- `pnpm tauri build` — PASS for the current master development build; the
  portable executable, MSI, and NSIS bundles were produced. No same-version
  installer upgrade smoke was claimed.
- `node scripts/dev088-architecture-guard.mjs` — PASS, including exact recipe
  identity, history read-only/on-demand, archive authority, repository
  boundaries, typed RPC parity, and no-new-queue/executor/task-model
  sentinels.
- `git diff --check` and the tracked-file secret scan — PASS.
- Remote Source-only CI `#34570210019` — GREEN for
  `READINESS_CODE_HEAD`; Rust source checks and frontend source checks passed.
  The only annotation was the existing GitHub Actions Node.js 20 deprecation
  notice.

## Release hygiene

README version truth now identifies 1.0.0 as the published release and 1.1.0
as post-1.0 readiness while keeping all manifest files at 1.0.0. The
historical release document `docs/DEV_074_RELEASE_1.0.0.md` was not modified.
No formal release, tag, version bump, or asset upload was created. `.serena/`
remains local metadata and is not committed.

```text
DEV_096=PASS
DEV_097=NOT_STARTED
STOP=YES
```
