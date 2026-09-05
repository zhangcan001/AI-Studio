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
- User workflow purge preflights all references, atomically quarantines every
  referenced package directory before the database purge, restores the
  quarantine if the database operation fails, and reports structured
  `WORKFLOW_PURGE_COMPENSATION_FAILED` when rollback itself fails. Product
  workflows are not purgeable.
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

## Verification

Focused DEV-084 Rust integration coverage includes exact-pair cardinality and
conflict handling, migration reconciliation, atomic purge and no-resurrection,
product purge blocking, direct-generation state admission, unified workspace
missing/refresh diagnostics, capability cache persistence, restore
revalidation, and project-binding isolation.

The final validation command results and source commit are recorded below:

```text
DEV084_FOCUSED_RUST=15_PASS
FULL_RUST=PASS_0_FAILED
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_CLIPPY=BLOCKED_BY_PRE_EXISTING_LINT_DEBT
FRONTEND_TEST=PASS_118_FILES_563_TESTS
FRONTEND_BUILD=PASS
TAURI_BUILD=PASS
DIFF_CHECK=PASS
SOURCE_COMMIT=e1c5cc0b1de9cd115076cfc0c1fcb04d572d9ce5
```

Strict Clippy was run with `-D warnings` and remains blocked by the repository's
pre-existing lint debt; the DEV-084 additions do not introduce a separate
Clippy failure category. The release build produced the Windows executable and
installers under the dedicated DEV-084 Cargo target directory.
