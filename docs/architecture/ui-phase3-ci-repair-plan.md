# UI Phase3 merge CI repair

Parent: 1702f3b4b1434fc3d02c96a63346a272740b21d4. Validation-only scope;
no Phase4, product-policy, UI, migration, backup or Runtime Package changes.
Historical JSON and hashes remain immutable. A separate child validates live
repair bytes and projects exactly the four declared parent files before the
UI Phase3 proof is replayed. Rejection grants no file exclusions or projection.

## Windows diagnosis before repair

The release-only test reproduced its 10s timeout locally. Publication and
readiness files passed locally in 22.45s and 18.59s; merged Windows CI exceeded
their existing 30s case budgets. This is not evidence of a deadlock or proof
failure. No runner resource telemetry establishes a specific CI RAM/CPU cause.

Instrumented fresh-process baseline: release cold 9.55s, 48 Git subprocesses
(6.89s); identity 0.10–0.12s, 1516 raw reads, 63.8MB; warm reader 0.11s,
no Git IO. Backend aggregate 0.011s; 100 historical projection reads plus one
fresh identity 0.096s. Owned complete fixture copy 1.04s; fixture cold 5.69s,
23 repeated Git calls (4.26s); warm 0.055s; changed-doc rejection 0.445s;
restored acceptance 0.058s. Each outer call freshly hashes full bytes; nested
readers share one identity, so nested full-tree scans are not the primary cause.

## Narrow performance seam and regression

Reuse only full-SHA ls-tree/cat-file results for the same physical Git object
store. Empty owned fixture stores borrowing one read-only alternate can share
immutable IO; unrelated repos or environment-directed Git layouts cannot.
Mutable tags and accepted readers stay root-scoped and freshly scanned.
Disable Git replace refs for immutable queries. Return copies of cached buffers.
Recognize the actual ls-tree tree argument, not an arbitrary hexadecimal path;
mutable tag queries always execute live, even when a path looks like a SHA.
Do not replace raw content identity with mtime/size. Deduplicate only identical
fixture copy paths; keep every mutation/restore probe.

The subprocess-count regression failed before the fix (two executions for two
owned fixtures with one alternate) and must pass with one. Fresh-byte regressions
cover equal-length/equal-mtime mutation, CRLF-only drift, proof deletion, illegal
addition, tag changes and restore. Existing full proof and package probes remain.

Post-optimization measurements (same profiler, complete valid fixture): cold
release 10.08s / 50 Git calls, including the new legal repair checkpoint; fixture
cold 1.73s / 3 Git calls / 0.092s Git IO, versus 5.69s / 23 / 4.26s before.
Warm 0.051s; changed-doc rejection 0.430s; restored PASS 0.051s. Immutable IO
reuse removes 20 duplicate fixture Git calls without caching mutable acceptance.
The cold release case alone gets 30s (about 3x measured cold time), not an
execution/queue budget. Global testTimeout and every other budget are unchanged.
Run the three
original files individually, then full seven serial gates and exact-head PR CI;
merge safely, then verify final master CI. No retry without diagnosed cause.
