# CI-OPT-003 — Reduced Remote CI Policy

```text
TASK=CI-OPT-003

Baseline SHA=9e74538fb87d58188e251a6de20b5844fde3cc12
Implementation SHA=5779476676a6b21494f920b8c7e98e09918ae5c0

NORMAL_MASTER_PUSH_CI=OFF
PULL_REQUEST_CI=ON
WORKFLOW_DISPATCH=ON
RELEASE_TAG_CI=ON

REMOTE_CI_FREQUENCY_REDUCED=YES
REMOTE_CI_COVERAGE_REDUCED=NO

RUST_JOB=UNCHANGED
FRONTEND_JOB=UNCHANGED
RUST_CACHE=PRESERVED
```

## Verification

The implementation commit changed only `.github/workflows/ci.yml`. No Source-only CI #115 was automatically created by the normal master push; the latest automatic source run remained #114. Ordinary master pushes therefore no longer automatically trigger Source-only CI.

```text
Implementation commit scope:
PRODUCT_CHANGE=NO
WORKFLOW_CHANGE=YES (implementation only)
CODE_CHANGE=NO
DOC_ONLY=NO (implementation commit)

Result commit scope:
PRODUCT_CHANGE=NO
WORKFLOW_CHANGE=NO
CODE_CHANGE=NO
DOC_ONLY=YES
```

## Default development policy

```text
LOCAL_FULL_GATE=REQUIRED
AUTO_COMMIT=YES
AUTO_PUSH=YES

REMOTE_CI_REQUIRED=NO
AUTO_WAIT_REMOTE_CI=NO
```

High-risk, release, and phase-final tasks may explicitly set:

```text
REMOTE_CI_REQUIRED=YES
```

The result documentation commit itself is Markdown-only and is not expected to trigger Source-only CI.

```text
CI_OPT_003=PASS
```
