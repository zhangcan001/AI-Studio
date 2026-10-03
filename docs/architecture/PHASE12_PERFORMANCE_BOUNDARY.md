# Phase12 runtime performance measurement boundary

Baseline: master `0bb137303bbcf4baa98a109e7558d5899ef236fc`, CI37107002128.
Measure before optimizing. The matrix distinguishes static candidates from actual
hotspots. It does not interpret source SQL-call counts as runtime query counts.

## Reproducible Native navigation harness

Use a fresh SQLite backup of an **owned isolated** acceptance fixture; never copy
live WAL files or open real user projects. Start the existing matching debug
backend with `AI_STUDIO_DATA_ROOT`, an isolated WebView directory and debug port
9224. Serve the matching frontend bundle on localhost1420. Verify the actual
owned process before attaching. Then set `AI_STUDIO_PERF_ISOLATED=YES` and
`AI_STUDIO_PERF_FRONTEND_MODE=PRODUCTION_BUNDLE`, and run:

```
node scripts/testing/native-performance-baseline.mjs 9224 <absolute-output-path>
```

The harness measures actual navigation controls, one warmup and five samples,
min/median/max. Completion includes the relevant visible page, loading completion,
actual IPC completion and animation frames. Millisecond values include CDP/frame
observation overhead; they are evidence, not CI timing thresholds. No arbitrary
sleep or retry hides a failed action. A selector mistake in the initial harness
was corrected to the existing normal `.v3-overview` owner, not the Advanced page;
the failed harness attempt is not a product regression or valid timing sample.

The observer reuses Phase11 preflight classification. Different request IDs remain
distinct even when arguments match. Published output contains only commands,
counts, timing and byte sizes, not request bodies/digests/user content. Encoded
response bytes are transport size, not decoded DTO size. Repeated semantic calls
remain candidates until their lifecycle/intent is traced.

## Baseline discoveries and limitations

Development frontend StrictMode intentionally repeats mount effects. It remains
enabled. Do not claim these probes are accidental production calls or mix their
timings with production-bundle after measurements.

Production-bundle navigation baseline has stable per-action counts across five
samples: overview1, project switch4, Create preparation2, Runs load1, refresh1,
Library load1, prompts1, Lab10. Lab has four preset reads for two exact candidate
references: two identical repeated reads per opening. Trace the existing Benchmark
candidate-reset and preset effects before modifying that dependency boundary.

This is an initial measurement checkpoint, **not Phase12 completion**. Startup and
real process-resume timing, true project-open, Create submit, selection/edit/file/OCC,
SQL counters, React profiling, representative large data and final remeasurement
remain pending. No missing metric is zero. No general leak-free or global
duplicate-free claim follows from these navigation samples.

## Freeze and gates

Keep schema42, backup20, typed transport, route/product semantics and Phase8–11
ownership. No telemetry platform, second cache/store/queue, blanket memoization,
test deletion, new ignored cases or timing-based flaky CI assertion.
Local automated case budget remains10; initial two measurement-helper cases passed.
Full suite authority remains the final-head remote CI. Do not mark the checkpoint
CI as final performance acceptance. Close owned app/server after acceptance.
