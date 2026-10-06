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
Fresh DB, owned r4/max42 opening, NSIS build and installed navigation have now
been exercised. Overall closeout is **BLOCKED**, not READY: the actual backup
restore creates asset-version identities rejected by the domain reader.
Reinstall acceptance and final exact-head CI remain NOT VERIFIED.
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
Original projects/tasks/assets/relations/versions/bindings and all 432 original
media hashes were preserved across candidate startup and backup operations.
Fresh installed startup creates migration42/formal71 with no043.

## Installed acceptance and blocker

The NSIS was silently installed to an owned temporary program directory with
shortcuts disabled. Installed payload differs from the standalone build only
at Tauri's three-byte bundle marker `UNK` to `NSS`; exact normalized full-byte
comparison passed. Fresh installed launch, single-instance second launch (exit0,
one owned process), safe exit and navigation passed: Project List, Overview,
Create, Runs, Library, Project Settings, System Settings and Workflow Lab.
Overview explicitly showed offline/uninspected, rather than READY. Existing
owned r4 data opens; Runs recent and complete task-history entry, Library next
page and managed thumbnails are reachable. Explicit safe-image inspection
reports safe/existing/readable/SHA match/preview pass, without exposing paths.
Local diagnostic ZIP export succeeded; it contains diagnostics/README/sanitized
logs, not the owned absolute data root. No remote telemetry is introduced.

Actual Native archive export (221 files), inspection and new-project restore
completed at format20. Restored counts match: assets108, tasks63, prompts51,
batches12, bindings2, asset versions2, relation1, reference set1 and character1.
Media hashes and exact workflow/recipe pairs match; foreign-key check is clean;
original business rows/media remain unchanged. **These checks are not enough to
declare Backup PASS**: both restored version IDs start with `asv_`, while
`AssetVersionId::parse` accepts only `av_`. The restore allocator in
`project_backup_service.rs` generates `asv_` (line488 at installer source), and
`AssetVersionRow` calls that strict domain parser. Opening historical restored
`Generated Image 1` fails in Library detail; safe-image details still work.
The defect also exists in the r4 test copy and is not a 2.1 execution change.

P0: none observed. P1: backup restore produces domain-unreadable asset versions.
No business repair was made or hidden by fixture rewriting. Cheapest safe next
step: use the domain constructor for newly restored IDs, define readonly legacy
`asv_` compatibility without rewriting unique user data, add a real restore-to-
typed-reader regression and cover lineage references; review exact successor
scope, rerun affected/full gates, rebuild and repeat installed round-trip.
No migration043 or backup-format bump is justified by this identity defect.

Candidate was exited and uninstalled after the blocker. Application removal and
byte-for-byte preservation of both owned data trees passed. Previous owned r4
uninstall registration and program hash were restored. User applications,
ComfyUI and unique user projects were not terminated or modified. Reinstall was
NOT RUN after discovering the blocker; do not mark installed acceptance PASS.

## Installer and publication

Installer source: `88e3baef8d6982d2dbf0ef8b4685702fe0ad2252` (clean and synced
before build). NSIS build passed in9m20s; actual installer filename
`AI Studio_2.1.0-personal_x64-setup.exe`, size13066983 bytes, SHA-256
`402FB863D16993E3E9780EBA855BE512C8C84B1AC4568E6CDAB512EAD697E5AB`.
MSI NOT REQUIRED; signature NotSigned. This is a **blocked acceptance artifact**,
not a recommended user installation or publication candidate.
Binaries, DBs, media and acceptance logs
are local only, never committed. Final evidence-only changes may follow the
installer source; runtime/config changes instead require rebuilding/reacceptance.

Unsigned Windows installer; external ComfyUI/models/custom nodes; no remote
telemetry/cloud sync/automatic tool control. P2/P3 nonblockers remain
documented/deferred. No tag,
GitHub Release or upload is authorized. Stop before Publication Gate.

The source commit triggered CI37398628552; frontend success was observed while
Rust was still running. It is not final-doc-head authority. Final documentation
commit changes evidence only: RUNTIME_CHANGED=NO, INSTALLER_CONFIG_CHANGED=NO.
Final exact-head CI must be reported separately and may never clear this Native
backup blocker merely by being green. RC READY=NO; publication gate READY=NO;
2.1 RELEASED=NO; next checkpoint STARTED=NO.
