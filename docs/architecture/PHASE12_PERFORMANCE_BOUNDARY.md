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
Local automated case budget remains10; executed cases are recorded at each checkpoint.
Full suite authority remains the final-head remote CI. Do not mark the checkpoint
CI as final performance acceptance. Close owned app/server after acceptance.

## Verified first optimization checkpoint

Benchmark preset loading now depends on the exact unique reference set and project,
not the candidate array, labels or order. No second cache is introduced. Existing
effect cancellation remains; project change, reference-set change and remount
still read fresh data. A count-based regression case proves initial two reads,
no reload for label editing, and fresh reads on project switch and remount.

Same-machine, same owned fixture, same debug backend and production frontend
mode, one warmup/five samples: Lab IPC10→8, preset reads4→2, repeated identical
calls2→0. Median23.0699→23.0207ms does **not** establish wall-clock acceleration;
the claim is reproducible work-count reduction only. Other measured navigation
paths retain their counts. The completed harness observed five repeated route
transitions per scenario, not a general heap/listener leak-free proof.

Future runs record actual checkout revision, production-source dirty state and
frontend bundle-index digest instead of a hardcoded baseline revision. Record the
matching backend revision with `AI_STUDIO_PERF_BACKEND_SOURCE_HEAD`; otherwise it
is explicitly NOT_RECORDED. The initial before dataset used the baseline revision;
seam1 after data was measured on the inventory commit plus the reviewed uncommitted
component change, pinned by its source digest. Do not retroactively fabricate a
missing initial bundle digest.

The Phase9 whole-source hash guard correctly rejected the authorized behavior
edit. Keep its historical manifest intact. Its successor handling now accepts
only an existing app/feature source path with a measured review, five before/after
samples, the exact frozen parent digest and a new pinned digest. New files,
transport/CSS changes, wrong parents and insufficient evidence remain rejected;
positive/negative self-tests protect this boundary. No debt entry was added.

This checkpoint completes **one** optimization, not the required three. Eight
navigation scenarios are not eight fully covered required path domains. Remaining
baseline/profiling/large-data, two additional measured optimizations and complete
correctness Native acceptance must finish before Phase12 can pass.

## Continuation: actual processes and deeper profiling (still PARTIAL)

The continuation retains all three existing commits and makes **no production
behavior change**. The earlier pending statements above describe that historical
checkpoint, not the new measurement inventory. The machine, debug executable
SHA256 and production bundle SHA256 are recorded in the JSON alongside every
group. Startup and resume use the original small isolated fixture. Deep profiling
uses `phase12-large-v1`: 55 tasks, 108 media records and 51 prompt entries, with
existing profiles/reference sets. These two datasets are deliberately separate
and are not a before/after performance comparison.

`native-process-performance.mjs` spawns the actual owned executable, observes its
new WebView, verifies an actual native window handle and closes only that spawned
PID gracefully after checking its executable path. It never uses reload for
startup/resume. Both process-cold and warm groups have one warmup and five samples.
The OS cache is **not** controlled. Primary timing is `visible`, measured before
the external PowerShell window-handle verification; `elapsedMs` includes that
verification overhead and must not be marketed as pure startup latency. Database
and recovery log timestamps are stage measurements, not universal backend-ready
proof. Startup IPC counts are attachment-onward only, not fabricated full counts.

Deep resume is prepared through real UI navigation/selection, followed by actual
process exit and launch. Create+shot, Runs+RunRef and Library+ResourceRef each have
one warmup/five samples, with the exact persisted locator and actual child detail
verified after restart. No route JSON injection/reload substitutes for this proof.

`phase12-expand-fixture.py` is an explicitly owned-fixture-only seed script. It
refuses to write while an AI Studio process runs, preserves original user-independent
fixture records, uses deterministic synthetic identifiers and validates foreign
keys. It shifts **all** task lifecycle timestamps together: changing only
`created_at` correctly fails existing domain integrity checks. Those rejected seed
attempts were harness errors, not performance samples or production regressions.

`native-deep-performance.mjs` measures true Project List→Open, Create load/shot
selection, large Runs list/detail/results, Library list/media/prompt detail, and
prompt-save mutation. Its acceptance-only new-document script observes production
React root commits and live EventTarget/timer/blob ownership. Enable CDP Page before
injection and wait for the new document marker; otherwise old DOM can satisfy a
reload wait and produce a false measurement. This profiler is never bundled into
the app. Root commits are equivalent bounded work evidence, **not actualDuration
or precise per-component execution counts**. Production minified names are not
used as stable owner identifiers.

Twenty Runs enter/leave cycles return connected listeners 148→148, global
listeners 3→3, active intervals 0→0, timeouts 0→0 and object URLs 0→0. Mounted Runs
owns one 5000ms interval, cleared on leaving. Weak registration records distinguish
detached DOM from connected/global owners without retaining DOM solely for counting.
Detached registrations fluctuate with uncontrolled GC and are not a leak metric.
Private `runInvalidation` subscriptions are still **NOT_MEASURED**; resource
acceptance therefore remains partial, not a blanket leak-free claim.

Prompt edit exposes a measured candidate: each write is followed by **two** copies
of list/get/relations/versions (9 IPC total; 4 repeated identical reads). The
existing explicit refresh and 150ms same-owner invalidation refresh explain it.
The measurement waits on the observed 150ms timer and IPC completion rather than
ending at the first save acknowledgement. It records five identical work-count
samples. No optimization2/3 is selected or applied yet: complete safe Create submit,
Workflow deep read/OCC mutation, actual SQL profiling and subscription measurements
before ranking and changing a seam.

Local cumulative test budget remains **8/10**; this continuation ran no new core
cases. Acceptance samples are the separately requested Native measurements. Case9
and case10 remain reserved for the selected backend and frontend optimizations.
Architecture guard and JavaScript syntax checks passed. Full local final gates,
final Native correctness and final-head CI remain pending; checkpoint CI success
does not close Phase12 or authorize Phase13.
