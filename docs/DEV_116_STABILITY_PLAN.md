# DEV-116 — AI Studio v1.3.1 Stability Plan

```text
TASK=DEV-116
VERSION=1.3.0
TARGET_VERSION=1.3.1
BASELINE_SHA=635194d56db119b5739ff56ab5b4902b0cfe6178
```

## Boundary

This is a maintenance pass only. It does not add a product feature, user
workflow, domain model, schema, migration, queue, task, review, or asset
behavior. The existing Production Queue and Studio Store remain authoritative.

## Known technical debt

- **Rust cancellation E2E timing:** the cancellation lifecycle is implemented
  and the exact v1.3.0 release gate passed, but one historical Windows CI run
  failed before the test observed `RUNNING`. The test uses a fixed cooperative
  yield budget rather than a bounded wall-clock wait.
- **CI reliability:** the Rust gate is deterministic when it completes, but a
  cold Windows run takes materially longer than the frontend gate and the jobs
  have no explicit timeout budget.
- **Frontend build:** Vite reports the existing main chunk above 500 kB after
  minification.
- **Error visibility:** the main task and production surfaces expose error
  codes/messages and technical details, but generic failures should make the
  next safe action clearer without changing the state model.

## Possible fixes

1. Replace test-only fixed-yield polling with bounded, scheduler-friendly
   waits; add deterministic coverage for cancel, cleanup, timeout behavior, and
   repeated cancellation without hiding failures.
2. Add conservative GitHub Actions timeout budgets without removing jobs,
   lowering coverage, or changing the production workflow.
3. Check whether route-level lazy loading removes the main-chunk warning. If
   the measured gain is small or the regression risk is disproportionate,
   document `defer_to_v1_4` rather than redesigning the build.
4. Improve only user-facing error guidance where the existing code and state
   already identify a safe retry, refresh, inspect, or settings action.

## Version policy

Do not bump the product version while fixes, tests, and documentation are in
progress. The target is `1.3.1` only after all applicable local and remote
checks pass and the final stability report is complete.
