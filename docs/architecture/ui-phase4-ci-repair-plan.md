# Phase4 style-inventory CI repair

Validation-only child of published Phase4 checkpoint `70dff1874d9314ce800c58c3bd49a99b04321e16`. Preserve its JSON/hash and every earlier checkpoint; validate declared live repair bytes and untouched aggregates before projecting approved parent bytes. No business UI, backend, queue, migration, Backup21 or Runtime Package changes.

## Measured cause

Exact-head CI 37902204998: Rust succeeded; frontend had 1239 passed, one failed, one skipped. Only StyleBoundary target1 failed: synchronous cold proof plus inventory completed in 17.392s against 15s. Full frontend took 720.05s versus local 466.06s; local target1 was 11.536s. No runner resource telemetry proves a RAM/CPU fluctuation, and this is not evidence of deadlock or failed proof assertions.

Fresh Windows profile before optimization: m3 historical reader 8.745s, 37 Git subprocesses / 5.998s; warm UI parent 0.093s; warm m3 fresh-identity reader 0.103s; full projected inventory 3.056s; live CSS-only inventory 0.043s. Inventory repeats full source-text association for 5423 class references but only 1913 distinct classes. This repeated class association is the narrow optimizable seam; mandatory cold historical Git validation remains.

## Repair and regression contract

Cache literal substring class-to-source associations only inside one styleInventory invocation, with stable original source order and union semantics. Read fresh source/projection text for every invocation; no cross-call AST/inventory/mtime cache. Preserve every caller, inline classification, selector, metric and full-history validation.

Compare the entire historical inventory JSON digest before/after, not just counts. Baseline digest is `50eba7e3e0b93f2404ef0d175f5c7c8a9c8eaf7fe59879bf655b10d96610426e`. Owned fixtures cover substring semantics, ordering, inline classification, changed live text, alternate projections and restoration. Successor fixtures additionally reject script/proof/source changes, deletion, illegal additions and raw Runtime Package mutations.

After optimization: full inventory 1.264s/1.280s versus 3.056s/3.059s before (about 58% less inventory work); entire JSON digest exactly unchanged. Cold m3 8.499s with 39 Git subprocesses / 5.912s, including the new child checkpoint; warm fresh-identity reader 0.098s. Targeted StyleBoundary target1 completed in 10.372s; all 14 focused inventory/repair/historical probes passed. Cold Git remains the dominant mandatory cost; do not attribute its measured variance to an unmeasured runner resource cause.

After reducing repeated work, give only cold StyleBoundary target1 a finite 25s static-validation budget: CI measured 17.392s and immutable Git startup remains mandatory. Do not change global testTimeout, target3's existing 30s budget, runtime performance thresholds, assertions or negative probes. Record after measurements in delivery evidence; no blind CI rerun.
