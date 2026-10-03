# Phase11 test ownership and reliability

Inventory precedes test/CI migration. The JSON matrix discovers all named frontend
test groups and Rust test-bearing files; Rust attribute counts are static
declarations, **not** expanded execution counts. The baseline CI log is authority:
993 frontend cases; 1219 Rust unit and 441 integration passes, 3 ignored.

## Layer responsibilities

Pure Unit/Domain: rules, parsing, selectors, mapping. Application/Feature:
orchestration, loading/errors, lifecycle and authority interaction. Contract:
real serialization and typed request/response semantics. Repository/Integration:
real SQL, transactions, migrations and filesystem adapters. Architecture: source
structure/import/command registration restrictions. Native: actual bootstrap,
registration, managed state, route wiring and persisted restart. Different layers
or failure modes are not duplicates. Source assertions cannot prove UI behavior.

## Four bounded improvements

1. Full architecture runner ownership: four existing tests start the identical
DEV-088 process. Move shared output assertions into one authoritative test; keep
all feature-specific source/authority assertions. Expose the existing runner as
`pnpm test:architecture`, rather than inventing another guard system. Test guard
positive/negative fixtures; grandfather lists remain exact and nonexpanding.
2. Runs lifecycle and behavior: replace the implementation-text Open Run proof
with actual accepted-result navigation, retaining the original no-auto-generation
assertions. Controlled promises exercise stale route/project results; fake time
verifies notification coalescing, subscription and timer disposal without sleeps.
3. Real project database fixture: compose an owning TempDir with a freshly
migrated real SQLite pool. Reuse only bootstrap/lifetime, not mutable data or a
global database. Keep domain-specific seeding and all assertions. Validate
isolation, persistence/reopen and transaction failure against real SQLite.
4. Native IPC observation: classify actual command requests, excluding preflight
and bodyless probes. Keep repeated request IDs distinguishable from genuine new
attempts; do not silently collapse retries or duplicate command invocations.

## Fast/full gates and resources

Local: at most ten distinct core cases (explicit filters), architecture runner,
tsc/build, cargo fmt/check, plus isolated Native acceptance. Inspect RAM/VRAM and
active owned runners first; no concurrent local builds/tests, no user process
termination. Local full suites are NOT_RUN, never mislabeled full regression.
Remote: final-head master Source-only CI retains all frontend/Rust cases and
serial Rust command. No increased timeout, retries, ignored cases, path filtering
or test deletion. CI logs provide measured suite and compilation times separately.

## CI baseline and optimization limits

Measured baseline run37101464010: frontend suite82.28s; Rust executed suites
758.40s; critical-path job1685s. Rust test command also spent10m07s compiling,
which must not be presented as test execution. Jobs already run independently.
Rust cache is intentionally restore-only after previously documented post-cache
timeouts. No safe cache/thread/sharding change is presumed. Consolidating the
four duplicate full guard executions removes proven redundant setup. Other slow
real-DB/runtime cases remain KEEP_SLOW/DEFER unless specifically investigated.

## Compatibility/freeze

No frontend/Rust production change is planned. Keep Phase8 backend, Phase9 styles,
Phase10 owners, existing route/state/client authorities, IPC contracts, migration
42 and backup20. Existing ignored cases require external/user-owned resources;
offline CI must not enable them by accident. Native acceptance must use a fresh
copy of an owned isolated fixture, real close/relaunch and close the owned app.
Historical Native evidence is not proof of this phase's acceptance.

## Verified local/native checkpoint

Ten distinct automated local cases passed: seven frontend and three real SQLite
integration cases. No additional cases or full local suites ran. Existing
FrontendConsolidation target9 exercised valid and forbidden guard fixtures;
the existing Product client case executed the full unchanged DEV-088 runner.
tsc/build, architecture command, fmt and queue-recovery target compilation passed.
No production bug was found and no production source was changed.

One implementation-text assertion became stronger accepted-result navigation
coverage. No test case was removed, skipped or ignored: four redundant full
runner launches (five to one existing owner) were removed; feature assertions
stayed intact. Four frontend and two Rust integration cases were added. One
owning DB fixture replaces five bootstrap copies; real migrations/seeding remain.
The source scanner still reports mixed/source groups and port/sleep candidates
for review; these are not proof of flaky execution. The new IPC classifier case
contains a fake localhost URL only, not network I/O. Three selected previously
unverified lifecycle scenarios now have deterministic coverage.

The CI frontend build runs `pnpm exec vite build` after the unchanged explicit
TypeScript step, removing proven duplicate tsc from `pnpm build`. Full frontend
and serial all-target Rust tests, caches, timeouts and independent jobs are intact.
Before/after timing comes from actual CI logs; one run cannot establish causality
or promise overall critical-path acceleration.

Fresh isolated Native acceptance passed normal Create, Runs, Library, Workflow
Lab, Advanced bridges, Back/keyboard/dialogs/project switch and a genuine
close/relaunch preserving route and settings. The observer measured one real
task/profile/reference-list load on project switch. Both own Native apps closed.
The copied fixture already had an environment endpoint: creating that same URL
was correctly rejected; editing the existing profile using Update Environment
saved and survived restart. This was a harness/fixture adaptation, not a product
bug or a retry masking failure. Current-phase sanitized evidence is recorded in
`phase11-native-acceptance.json`; historical screenshots are not claimed current.

Final exact-head CI remains the full-suite authority. Its final measurements are
reported in chat rather than making a new evidence commit that changes CI_HEAD.
