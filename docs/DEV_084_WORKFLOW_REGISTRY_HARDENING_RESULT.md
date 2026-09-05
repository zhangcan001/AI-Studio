# DEV-084 — Workflow Registry V2 Production Hardening & Legacy Cutover

## Result

DEV-084 hardens the Workflow Registry V2 production path around exact runtime
identity, package lifecycle, and explicit import commit. The formal workspace
now reads the unified Registry + runtime model; legacy workspace and automatic
onboarding commands remain only as compatibility/internal APIs and are not used
by the formal production path.

## Frozen production invariants

```text
WORKFLOW_PURGE_RUNTIME_PACKAGE_ATOMIC=YES
WORKFLOW_PURGE_JOURNAL_DURABLE=YES
WORKFLOW_PURGE_JOURNAL_BEFORE_FIRST_MOVE=YES
CRASH_BEFORE_PURGE_DB_COMMIT=RESTORE
CRASH_AFTER_PURGE_DB_COMMIT=CLEANUP
PURGE_CLEANUP_PENDING_NEXT_START=CLEANED
PURGE_RECOVERY_MALFORMED=FAIL_CLOSED
PURGE_LIFECYCLE_MUTATIONS=SERIALIZED
WORKFLOW_LIFECYCLE_AUTHORITY=SINGLE_COORDINATOR
REGISTRY_BACKED_HARD_DELETE_BYPASS=NO
REMOVE_RESTORE_PURGE_SERIALIZED=YES
RESTORE_REMOVE_RACE=SAFE
RESTORE_PURGE_RACE=SAFE
LEGACY_DELETE_VERSION_REGISTRY_ROUTE=REMOVE
PERMANENT_DELETE_ONLY=WORKFLOW_PURGE
PURGED_WORKFLOW_RESURRECTION=NO
LEGACY_ARTIFACT_GUESSING=NO
EXACT_RECIPE_ARTIFACT_UNIQUE=YES
WORKFLOW_REFRESH_RUNTIME_DIAGNOSTICS=YES
CAPABILITY_SURVIVES_REGISTRY_RELOAD=YES
REMOVED_WORKFLOW_DIRECT_GENERATION=BLOCKED
HISTORICAL_DEFINITION_LOOKUP=PRESERVED
PROJECT_BINDINGS_AUTO_RESTORE=NO
FORMAL_IMPORT_PATH=ANALYZE_THEN_COMMIT
PRODUCTION_BATCH_FROZEN_IDENTITY=PRESERVED
DEV078_REGRESSION=PASS
DEV080_REGRESSION=PASS
DEV081_REGRESSION=PASS
```

The production executor remains single-path and unchanged: no second queue,
executor, implicit auto-start, automatic retry, or rebinding path was added.

## Implemented closure

- Migration `029_workflow_registry_runtime_artifact_reconciliation.sql` removes
  only the exact provisional artifacts from migration 028, enforces the unique
  `(workflow_version_id, recipe_id)` pair, and leaves backup schema `017`
  unchanged.
- Startup library synchronization rebuilds runtime artifacts from actual
  package manifests, workflow bytes, recipe bytes, and SHA-256 values. A
  missing package is reported as missing runtime truth; it is not silently
  replaced by a legacy package or guessed artifact.
- User workflow purge preflights all references and durably writes schema v1
  `.purge/purge_<uuid>/operation.json` before the first package move. The
  journal records the operation ID, workflow ID, package names, and creation
  time. Every referenced package directory is then atomically quarantined
  before the database purge.
- Startup reconciles purge operations before builtin package installation and
  library synchronization. A workflow row still present restores every package
  actually moved; an absent row cleans the committed quarantine without
  restoring it. Partial moves leave never-moved packages in place. Malformed or
  ambiguous operations fail closed with
  `WORKFLOW_PURGE_RECOVERY_BLOCKED` and remain untouched.
- Pre-journal DEV-084 quarantine directories are recovered only when their
  package manifests identify one unambiguous USER workflow. A present Registry
  row restores them; an absent row cleans them. Product or ambiguous legacy
  quarantine data is never guessed.
- A committed purge whose quarantine removal fails returns committed success
  with `cleanupPending`; the retained journal is cleaned on the next startup.
  Before database commit, rollback restores moved directories and removes the
  journal. A failed rollback reports
  `WORKFLOW_PURGE_COMPENSATION_FAILED` and preserves recovery evidence.
- Remove, restore, purge, and startup purge recovery share one application-level
  lifecycle gate. Registry reads remain unlocked. Concurrent purge calls
  therefore produce one committed purge and, at most, a stable
  `WORKFLOW_NOT_FOUND` for the follower without runtime resurrection.
- One application-level lifecycle coordinator is the mutation authority for
  formal remove, restore, and purge commands. Restore holds the shared gate
  through runtime capability revalidation and the final enabled-state write,
  so remove/restore, restore/purge, and remove/purge cannot interleave into a
  split Registry/runtime state.
- Compatibility delete and restore commands decide their route by exact
  Registry existence. Registry-backed delete-version and delete-workflow calls
  map to logical `REMOVE`; Registry-backed restore-version calls restore the
  logical workflow and its current version. Only genuinely legacy-only rows use
  the old lifecycle path, and only `workflow_purge` permanently deletes a
  Registry-backed user workflow. Product packages remain protected.
- Registry remove now upserts runtime state for every version in the same
  transaction. Existing restored rows are therefore reliably reset to
  disabled and archived instead of escaping the conflict-update branch.
- A package rename that returns `NotFound` rechecks the source path and reports
  `AlreadyMissing` only when the source is actually absent. Product workflows
  remain non-purgeable.
- Direct generation checks the active Registry version, exact recipe, enabled
  and unarchived state, and exact runtime artifact before creating a definition
  or task. The stable rejection code is
  `WORKFLOW_UNAVAILABLE_FOR_NEW_GENERATION`.
- FAST workspace reads use Registry/database/cache state. REFRESH re-reads the
  exact package and recipe, validates hashes, and performs ComfyUI diagnostics.
  Recheck results are written back to the capability cache and survive Registry
  service reconstruction.
- Restore revalidates current runtime truth. A valid runtime returns ACTIVE and
  READY; missing or invalid runtime remains disabled and requires attention.
  Project bindings are not restored implicitly.
- Formal import is explicitly split into analyze then commit. The frontend
  workspace is separated into import control, Registry actions, list rendering,
  and the unified workspace adapter/client boundary.
- The frontend `restoreWorkflow` client now returns the exact Registry restore
  result, including current version, enabled/readiness/capability state, and
  project-binding count.

## Verification

Focused DEV-084 Rust integration coverage includes exact-pair cardinality and
conflict handling, migration reconciliation, atomic purge and no-resurrection,
product purge blocking, direct-generation state admission, unified workspace
missing/refresh diagnostics, capability cache persistence, restore
revalidation, project-binding isolation, crashes on both sides of the database
commit, deferred cleanup, partial quarantine recovery, malformed journal
fail-closed behavior, legacy quarantine recovery, concurrent purge safety,
all three lifecycle race pairings, Registry-backed legacy delete routing,
product delete/restore protection, and multi-version legacy restore routing.

The final validation command results and source commit are recorded below:

```text
DEV084_FOCUSED_RUST=30_PASS
FULL_RUST=PASS_0_FAILED_788_UNIT_PASS_1_IGNORED
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_CLIPPY=NOT_RERUN_PRE_EXISTING_LINT_DEBT_REMAINS
FRONTEND_TEST=PASS_119_FILES_566_TESTS
TSC=PASS
FRONTEND_BUILD=PASS
TAURI_BUILD=PASS
DIFF_CHECK=PASS
SOURCE_COMMIT=efadc33062e483de9c6fd9c11536c34d0e01ce32
```

Strict Clippy was not rerun for this closeout. The previously recorded
repository-wide `-D warnings` lint debt remains and no Clippy pass is claimed.
The release build produced the Windows executable and installers under the
dedicated `.target-codex` Cargo target directory. Migration remains `029` and
backup schema remains `017`.
