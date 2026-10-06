# AI Studio 2.1 Closeout — Release Candidate

Parent: `76a261607021d2ad10baef740d0afe9aebb6fc6f`; baseline exact-head CI
`37335055954` completed success. Public stable release remains
`v2.0.0-personal-r4`; candidate `2.1.0-personal` is NOT YET PUBLISHED.

M1/M2/M3/M4 = PASS; product work complete and frozen. No new feature work,
architecture rewrite, relink/replacement/automatic repair, mixed Runs pagination
or scale promises. F12 is CLOSED because the live README now states actual stable
and candidate status; historical notes/manifests retain their original facts.

## Candidate boundary

Version alignment: frontend/Rust/Tauri 2.1.0-personal; numeric WiX 2.1.0.
Cargo updates the root package lock normally. Schema stays migration42/formal
tables71/no043; backup remains20. The closeout successor validates exact reviewed
parent/live paths and complete untouched aggregates before projecting historical
M4/M3/M2/M1 source. No historical manifest is repinned.

## Acceptance status

Local full frontend: 1145 passed, 1 existing opt-in benchmark skipped (206 files).
TypeScript, frontend build, architecture gate, Rust format and all-targets check
passed. Full local Rust default-target tests: 1689 passed, 4 existing ignored,
zero failures; doc tests completed. This is the requested local `cargo test -j1`
gate, not a claim about final CI's separate all-targets count. The first run
found one stale DEV048 2.0 version assertion; only its three expected strings
were updated to 2.1. Migration/no043/backup assertions remain intact. The
successor constrains this test to that exact substitution and forwards the
validated live backend aggregate. Focused version regression and the complete
frontend/Rust suites were rerun successfully after this correction.
Fresh DB, owned r4/max42 data,
Backup v20 export/inspect/restore, NSIS build, installed Native and final exact
HEAD CI are PENDING until evidence below records their real completion.
No earlier CI or development-window smoke substitutes for these release gates.

## Execution-core classification

The r4-to-candidate diff has no changes in compiler, Comfy adapter, task domain,
Production Queue domain/service/command, generation service or input preparer.
Creation submission adds project-scoped immutable prompt provenance validation
and forwards `prompt_version_id`; it does not rewrite prompt text, select a
different generator or replace Queue Start. Thus queue authority, task state
machine, workflow compiler and Comfy submission semantics are UNCHANGED;
provenance is METADATA_ONLY. Real GPU smoke is SKIPPED_BY_POLICY for this
release-only candidate; unchanged execution evidence is reused, not rerun.

## Owned compatibility fixture

An owned r4/max42 copy contains 5 projects, 252 terminal tasks, 432 assets,
8 bindings, 1 reconstructed relation and 5 asset versions. Before candidate
startup: migration42/formal71, no foreign-key violations; every managed media
path resolves within the owned copy and every media file exists. The added
relation/version witness is reconstructed test data, not unique user data.
Preservation and backup round-trip checks remain PENDING.

## Installer and publication

Installer provenance/hash and install/uninstall/reinstall evidence will be added
after constructing the actual candidate. Binaries, DBs, media and acceptance logs
are local only, never committed. Final evidence-only changes may follow the
installer source; runtime/config changes instead require rebuilding/reacceptance.

Unsigned Windows installer; external ComfyUI/models/custom nodes; no remote
telemetry/cloud sync/automatic tool control. P0/P1 determination remains PENDING
release verification; P2/P3 nonblockers remain documented/deferred. No tag,
GitHub Release or upload is authorized. Stop before Publication Gate.
