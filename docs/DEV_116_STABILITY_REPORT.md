# DEV-116 — AI Studio v1.3.1 Stability Report

```text
TASK=DEV-116
BASELINE_SHA=635194d56db119b5739ff56ab5b4902b0cfe6178
TARGET_VERSION=1.3.1
AI_STUDIO_VERSION=1.3.1
```

## 1. Cancellation audit

The audit followed the existing lifecycle from cancellation request through
adapter interruption, task reconciliation, terminal state persistence, and
`TaskExecutionRegistry` cleanup. The production implementation already checks
cancellation at its execution checkpoints, sends the adapter cancellation once,
reconciles queue/history state, and preserves a successful result when
cancellation loses a race with completion.

The reliability fix is test-only:

- `wait_for_status`, `wait_for_action`, and `wait_for_registry_absent` now use
  a 15-second bounded wall-clock deadline with a 1 ms scheduler-friendly sleep.
- Timeout failures remain explicit and identify the missing lifecycle
  observation; no test is disabled, deleted, or hidden behind a retry.
- The running-cancellation test requests cancellation twice and verifies the
  stable `CancelRequested` response, then verifies adapter interruption,
  `Cancelled`, and registry cleanup.
- Existing coverage continues to exercise cancellation before POST, after
  upload, while queued, while running, and the completion race.

```text
CANCEL_REQUEST=covered
ADAPTER_INTERRUPTION=covered
ASYNC_CLEANUP=covered
JOIN_OR_COMPLETION_WAIT=covered
TIMEOUT_BEHAVIOR=bounded_and_visible
REPEATED_CANCELLATION=covered
RESOURCE_RELEASE=registry_cleanup_verified
CANCELLATION_FLAKY_REDUCED=YES
```

## 2. CI audit

The workflow remains Source-only CI with the same frontend and Rust checks,
install steps, cache behavior, and coverage. The only workflow change is an
explicit timeout budget:

- Rust source checks: 25 minutes.
- Frontend source checks: 10 minutes.

This bounds hung runners without changing the production workflow or masking
failures. The final exact-head dispatch is recorded in the health document
once the pushed commit has completed remotely.

## 3. Frontend build audit

The existing workspace imports in `src/app/App.tsx` were moved to React
`lazy()` loaders at the same conditional workspace boundary and wrapped in a
small loading fallback. Navigation, props, state ownership, and error-boundary
placement are unchanged.

Measured build result:

```text
BUILD=PASS
MAIN_CHUNK=277.01 kB
VITE_OVER_500_KB_WARNING=NO
BUILD_WARNING=REDUCED
```

The change is intentionally limited to the existing workspace split. No new
bundle abstraction or dependency was added.

## 4. Error visibility audit

The audit covered the requested Production, Queue, Review, Import, and Build
surfaces:

- **Production:** existing monitor and production item errors retain their
  state-specific message, code, and retry/action behavior.
- **Queue:** existing admission and queue item errors retain explicit status
  and safe queue actions; no second executor or queue was introduced.
- **Review:** existing task detail/review diagnostics retain structured node
  errors, raw technical details, retry policy, and exact task navigation.
- **Import:** existing asset/workflow import issue lists retain per-item
  failures and technical detail disclosure.
- **Build:** the build gate now reports a clean measured chunk budget; no user
  state model was changed.
- **Studio task card:** a failed task now reuses `UiErrorNotice` for localized
  guidance plus expandable technical details and offers the existing task
  detail action, when provided by the parent.

The result is a clearer answer to “what failed, at which task, and what should I
do next” without inventing a new state or execution path.

## 5. Local verification

```text
FRONTEND_TEST=PASS (150 files, 824 tests)
TSC=PASS
BUILD=PASS (no >500 kB warning)
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_TEST=PASS
```

The Rust gate includes the focused cancellation E2E suite and all-target tests
with one test thread. One existing jsdom navigation warning is emitted by the
frontend suite; it does not fail the suite and is unrelated to DEV-116.

## 6. Release decision

```text
P0=NONE
P1=NONE
P2_REMAINING=CI-113-01; FE-113-03; PERF-113-02
DEV_116=COMPLETE_AFTER_FINAL_REMOTE_GATE
DEV_117_STARTED=NO
AUTO_NEXT_TASK=NO
```

The product version is aligned to `1.3.1` only after the maintenance changes,
local checks, and release documents are complete. No migration, schema, queue,
task, review, asset, or production execution change is included.
