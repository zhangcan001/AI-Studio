# DEV-085 Engineering Baseline Stabilization Result

## Baseline

```text
BASELINE_HEAD=e96f1b23497c4bb627cfb3897ca79eb1b548e629
FINAL_HEAD=e96f1b23497c4bb627cfb3897ca79eb1b548e629
```

## Root Cause

The DEV-082 fixture assertion was platform-sensitive.

The disk-read path normalized CRLF to LF, while `include_str!` could preserve
checkout line endings. The failure was caused by textual representation, not
Workflow / Recipe / production behavior.

## Implemented changes

```text
GITATTRIBUTES_ADDED=YES
LINE_ENDING_TEST_NORMALIZED=YES
CI_RUST_FRONTEND_SPLIT=YES
```

- Added repository-level LF rules for source/configuration text and binary
  declarations for the repository's binary asset types.
- Normalized both sides of the DEV-082 manifest comparison, including CRLF and
  standalone CR.
- Split CI into independent `Rust source checks` and `Frontend source checks`
  jobs. The frontend gate uses `pnpm test`, TSC, and build.

## Verification

```text
FOCUSED_RUST=PASS
FULL_RUST=PASS
FULL_RUST_PASSED=1076
FULL_RUST_FAILED=0
FULL_RUST_IGNORED=2
FRONTEND_TEST=PASS
FRONTEND_TEST_FILES=119
FRONTEND_TESTS=566
TSC=PASS
FRONTEND_BUILD=PASS
```

The workflow structure was checked locally. `actionlint` was not installed in
the environment; no GitHub Actions run was triggered.

## Frozen invariants

```text
NO_NEW_FEATURE=YES
DATABASE_MIGRATION=NO
BACKUP_SCHEMA_UNCHANGED=YES

NO_NEW_TASK_SYSTEM=YES
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES

PRODUCTION_CHAIN_UNCHANGED=YES
WORKFLOW_LIFECYCLE_UNCHANGED=YES
IPC_CONTRACT_UNCHANGED=YES
UI_BEHAVIOR_UNCHANGED=YES
PRODUCTION_BEHAVIOR_UNCHANGED=YES
```

No migration, backup schema, production code, UI, IPC contract, or workflow
lifecycle files were changed.

## Architecture fitness future baseline

DEV-085 records the following future rules only; no architecture fitness system
was implemented in this task:

```text
APPLICATION_DIRECT_SQLX
DOMAIN_INFRASTRUCTURE_DEPENDENCY
DOMAIN_TAURI_DEPENDENCY
FEATURE_DIRECT_INVOKE
```

## Scope and CI status

```text
MASS_EOL_DIFF=NO
UNRELATED_DIFF=NO
DATABASE_MIGRATION=NO
BACKUP_SCHEMA_CHANGE=NO
PACKAGE_WORKFLOW_CONTENT_CHANGE=NO
PRODUCTION_CODE_BEHAVIOR_CHANGE=NO
UI_CHANGE=NO
IPC_CHANGE=NO
```

```text
LOCAL_GATE=PASS
RUST_CI_JOB=PENDING
FRONTEND_CI_JOB=PENDING
GITHUB_CI=PENDING
```

No push was performed, so GitHub CI remains pending.
