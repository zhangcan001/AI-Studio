# AI Studio 2.0.0-personal Release Candidate Checklist

This is release evidence, not a tag, GitHub Release or universal zero-data-loss
claim. Publication remains separate. The final exact documentation HEAD must
complete Source-only CI; the chat closeout records that SHA/run/conclusion.

## Version and frozen contracts — PASS

- package/Cargo/Tauri product versions: `2.0.0-personal`.
- Numeric MSI metadata: `bundle.windows.wix.version=2.0.0`; identity and targets
  unchanged. Windows targets=all generated NSIS and MSI.
- Migrations001–042, 71 named business tables, no043; backup format20.
- No Queue authority, Task state machine, binding OCC, workflow engine or
  remote telemetry change. Historical Phase7–13 manifests remain unchanged.
- Queue Start remains the only production execution gate. Readiness/acceptance
  does not imply execution, a media artifact, approval or selected result.

## Data compatibility — PASS (bounded evidence)

- Actual installed final-source app initialized a new isolated data root,
  applied all42 migrations and created a project through the real UI.
- Legal reconstructed pre-reset fixture: schema001–039 from original migration
  SQL/checksums plus compatible facts from an owned read-only Native fixture.
  This is NOT an official historical binary acceptance. It was upgraded to42
  by the installed candidate. Source/original user data were never modified.
- Preserved original projects2, shots1, tasks63, assets108, prompt entries51,
  prompt versions76 and exact binding stage/mode/workflowVersionId/recipeId
  facts; workflow/recipe history retained.108 media content hashes unchanged.
  Foreign keys/integrity passed. Binding OCC fields initialized compatibly.
- One inherited audio fixture had stale SHA metadata. Export correctly rejected
  it; only the isolated copied fixture SHA was corrected to actual bytes.
  This is fixture correction, not weakening archive integrity validation.
- Final installed UI exported/inspected/restored v20 (221 entries) to a new
  project, navigated into the restored project and stayed responsive.108 media
  hashes, shots/selected results, tasks63, entries51/versions76, queues12,
  bindings2/exact pairs, asset versions1, reviews5, reference sets1 and
  character profiles1 matched;13 generation snapshots and5 output links retained.
- Native fixture has0 structure series and0 asset relations. Rich lineage,
  structure/provenance and historical v18/v19 compatibility are covered by
  the real full Rust service suite, NOT falsely claimed as populated Native
  facts. Reader supports v1–v20; no claim every earlier version was separately
  exercised by an installed historical binary or stores all later fields.
- No filename/path/time/prompt-text inference used to restore relationships.

## Installed Native acceptance — PASS

- Final-source NSIS installed into an owned isolated TEMP program directory.
  New data root/new WebView profile: project list/create, Overview, Create,
  Runs, Library, project/system settings and Advanced Workflow Lab usable.
- Second launch exited0; only the first backend survived; existing UI accepted
  navigation afterward. No obvious startup hang.
- Active-task safety fixture was introduced only in the isolated copied DB,
  without starting an executor/GPU workload. Close warned; Continue retained
  a responsive app; confirmed exit closed it without cancelling the task.
  Fixture status was restored before the next startup.
- Verified silent uninstall removed program executable. Full-table logical
  row/count digests, every stored-media hash and archive hash were unchanged.
  Same candidate reinstall and subsequent installed launch preserved those
  business/media facts, opened Overview and reported a healthy DB.
- A first witness script failed on SQLite BLOB JSON conversion; it is NOT
  acceptance evidence. The fixed witness was captured BEFORE a separate real
  uninstall/reinstall cycle, with checked exit codes and exact comparisons.
- Pre-existing isolated app binary remains byte-identical. No user application,
  ComfyUI or unique user database was closed, restarted or deleted.

## Product smoke — PASS

- Create: actual shot picker, image and H3 video generator selection, prompt,
  dimensions/duration parameters, selected/candidate results, visible offline
  readiness boundary. No generation submitted; dirty-draft leave guard accepted.
- Runs: list/detail, historical success/results, paused-with-failed-items
  rendering, explicit recoverable retry capability, historical-input action
  returned to Create. No retry execution performed merely to satisfy smoke.
- Library: image preview512px, real video duration5s played (readyState4,
  paused=false, time advanced), prompt/version detail, explicit use relations,
  Use in Create and delete-impact inspection. No resource deletion executed.
- Workflow Lab: list, saved exact versions/recipes, history and diagnostics;
  no new publish. Five seeded definitions exist, but presence≠readiness.
- System diagnostics: healthy DB/offline fixture environment, bounded recent
  failure summary and task technical timeline with unknown missing durations.

## Installed diagnostics/privacy — PASS

- ZIP created via actual installed UI and native Save dialog. Only safe
  diagnostics.json, README and owned sanitized logs included.
- Positive harmless log-control survived export; seeded private prompt,
  credential/token, path, workflow and recipe source sentinels did not.
- Complete archive scan found no DB bytes, private prompt bodies, absolute
  paths, media bytes, source JSON or credentials/tokens. README's word Prompt
  is a policy statement, not an actual private prompt.
- Export is local save only; no upload operation. Source/privacy architecture
  guards and full regressions pass; no remote telemetry added.

## Final-source local checks — PASS

- Frontend188 files /1016 passed, sequential maxWorkers1.
- Rust47 target summaries /1696 passed /3 pre-existing ignored, all-targets,
  single build job and test-threads1. No new ignored/removed tests.
- TypeScript, frontend build, cargo fmt/check all-targets, architecture guards,
  Phase12 performance and Phase13 observability successors passed.
- Resource inspection before builds/tests; no concurrent build/test runners.
- Two real blockers have regression tests. Historical manifests were not
  blindly repinned: scoped Phase14 source proof validates reviewed live bytes,
  exact paths and untouched/full aggregates against immutable Phase13 parent.

## Execution and known limits

- New GPU generation SKIPPED_BY_POLICY. Compared GenerationService,
  ProductionQueueService, OutputCollector, ComfyUI infrastructure and runtime
  packages against quality-fix commit c916ea2: no diff. No new H3 benchmark.
- MSI generated/numeric-config regression PASS; MSI install NOT VERIFIED.
  Primary installed acceptance is NSIS. Candidate unsigned; trust prompt may
  occur. Generic dormant audio labels and external node/model setup visible
  in Known Issues. Existing P2 quality follow-ups remain, no new feature work.
- P0/P1 candidate blockers discovered here are closed; no claim beyond tested
  fixtures/environments. Candidate is not READY until exact final-HEAD CI PASS.

## Evidence retention and Git

Installers, diagnostic ZIPs, DB/media/archives and acceptance logs are local,
not committed. Installed-source SHA/fingerprints are in RC_BASELINE. The final
closeout is documentation-only; it must prove no runtime/config diff from that
installer source. Ordinary master push only; no force, tag, Release or next phase.
