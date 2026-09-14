# DEV-130.1-A — Comfy Execution Admission Lifecycle Hardening

## Scope

This change hardens the existing single execution-admission boundary. It does
not introduce a queue, task, generation, executor, or parallel execution
model. `Task` remains the generation fact carrier and the Production Queue
remains the only production entry point.

## Root Cause

`GenerationService` previously acquired `submission_gate` immediately before
the Comfy submission section and explicitly dropped the semaphore permit after
`submit_workflow` and submission telemetry were persisted. The WebSocket event
monitor and output collection then continued without the permit, so a second
interactive or queue generation could enter Comfy while the first generation
was still using the GPU.

## Implementation

- Renamed the shared gate to `execution_admission` to reflect its real scope.
- Added the internal `GenerationExecutionLease`, which owns an
  `OwnedSemaphorePermit` and the associated `TaskId`.
- Acquired the lease in `GenerationService::execute_prepared` after validation,
  snapshot persistence, and the last pre-execution cancellation checkpoint,
  before input preparation/upload begins.
- Kept the lease in scope through input preparation, Comfy subscription and
  submission, WebSocket monitoring, cancellation reconciliation, terminal task
  transition, output collection, and output import.
- Removed the old early `drop(submission_permit)` path. Normal Rust drop
  semantics release the permit when the generation method returns, including
  success, failure, cancellation, stream disconnect, timeout/error, and worker
  abort.

No new task status was added. A generation waiting for the shared lease remains
in the existing pre-execution lifecycle status until its lease is acquired;
this preserves the current Task state machine and queue compatibility.

## Lifecycle

```text
Acquire GenerationExecutionLease
        ↓
Prepare inputs / upload references
        ↓
Subscribe to Comfy events
        ↓
Submit Comfy job
        ↓
Monitor WebSocket and reconcile cancellation
        ↓
SUCCESS / FAILED / CANCELLED / adapter timeout or disconnect
        ↓
Collect/import outputs when applicable
        ↓
Return from execute_prepared → RAII lease release
```

The same `GenerationService` instance is used by interactive generation and
the Production Queue. Therefore both callers share this one semaphore without
changing `ProductionQueueService`, `Queue Start`, task linkage, or production
state authority.

## Failure and Cancellation Protection

| Path | Lease behavior |
| --- | --- |
| Comfy submit failure or protocol mismatch | `execute_prepared` returns through the lease scope; permit is released after failure persistence |
| WebSocket closes or reports a stream error | Existing stream-disconnect handling is preserved; lease is released on return |
| Execution error/interruption | Existing task failure/cancellation handling runs; lease is released after the terminal return |
| User cancellation | Existing cancel request and interrupt reconciliation run while the lease is held; lease is released after `CANCELLED` or the existing cancellation outcome |
| Adapter timeout/error | Existing adapter error path returns through the lease scope; no permit leak |
| Worker/application abort | Dropping the async frame drops `GenerationExecutionLease` and its owned permit |

The change does not infer or fabricate provenance, alter queue/task schemas, or
add a second execution authority.

## Tests

Added coverage for:

- `execution_lease_holds_permit_until_the_real_execution_returns`: a second
  lease cannot acquire until the first lease is dropped.
- `interactive_generations_share_the_execution_admission_lease`: two
  interactive generations cannot submit concurrently.
- `queue_generation_entry_point_waits_for_interactive_execution`: the queue's
  existing `start_generation_with_task_hook` entry point waits behind an
  interactive execution.
- `stream_disconnect_releases_execution_admission`: a successful submit
  followed by a WebSocket close does not leak the permit.
- `cancelled_execution_releases_admission_for_the_next_generation`: a
  cancelled execution releases admission for the next generation.

## Validation

The focused admission and cancellation tests pass. Full local validation is
recorded below after the release checks:

```text
FRONTEND_TEST=PASS (155 files, 849 tests)
TSC=PASS
BUILD=PASS
RUST_TEST=PASS (797 passed, 1 ignored)
CARGO_FMT_TARGETED=PASS
CARGO_FMT_FULL=BASELINE_FAIL (pre-existing formatting drift in archive files)
```

The full Rust gate also exposed stale committed compatibility assertions that
still expected the pre-v19 archive constant; those assertions were aligned to
the current v19 implementation. A pre-existing frontend test callback was
given an explicit parameter type so the required TypeScript gate could compile
the current test suite. Neither adjustment changes production execution
behavior.

## Completion Gates

```text
REAL_GPU_SERIALIZATION=PASS
PERMIT_RELEASE_ON_ALL_TERMINAL_STATES=PASS
QUEUE_AUTHORITY_UNCHANGED=PASS
NO_NEW_EXECUTION_MODEL=PASS
```
