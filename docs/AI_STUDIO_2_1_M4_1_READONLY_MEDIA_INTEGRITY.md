# AI Studio 2.1 M4-1 — Readonly Media Integrity

## Scope and authority

Parent `7926f74ea3a77771d489fc834c93ed876d64163c`; master-direct. This checkpoint only adds explicit readonly inspection of existing project-managed Asset resources. AssetRepository identity, AssetStore read boundaries, AssetQueryService and typed Product transport remain the sole authorities. No repair, relink, replacement, import, deletion, checksum refresh, persisted health, background scanner or competing store is introduced.

## Contract

`product_library_media_verify(projectId, resourceRef)` accepts Asset only and checks exact project ownership before file facts. The independent typed facts are boundary SAFE/REJECTED/NOT_CHECKED, existence PRESENT/MISSING/NOT_CHECKED, readability READABLE/UNREADABLE/NOT_CHECKED, checksum MATCH/MISMATCH/INVALID_EXPECTED/NOT_CHECKED, and preview PASS/FAIL/CHECK_UNAVAILABLE/NOT_APPLICABLE/NOT_CHECKED. DTO includes assetId, assetType and checkedAt, but no paths, hashes, stderr or private source. Errors are sanitized; no error-message parsing is used.

The shared path resolver backs existing validation and the new inspection port. Traversal, out-of-project paths, symlinks (including aliases back into the project) and nonregular files fail closed. Missing descendants are classified only after the nearest existing ancestor is validated. Stream failures use typed reinspection to handle disappearance/boundary races, not English error strings. Files can change during a check; results are observations at checkedAt, not a filesystem-lock or persisted health guarantee.

SHA-256 uses existing `Asset.sha256` (64 hex characters); expected hashes are never updated. Existing `open_read_stream` supplies chunks no larger than 1 MiB. Image preview buffers at most 32 MiB encoded bytes and applies the same 32 MiB decoded allocation limit; oversized checks are unavailable rather than damage. Video/audio reuse typed MediaProbe outcomes; absent probes are unknown, empty optional metadata is not failure. Existing import/metadata/poster/protocol paths are unchanged.

## UX and bounded scan

Asset detail exposes **检查此媒体**. Media-category Library exposes **检查当前项目媒体**. Scan uses existing `library.list(category=media, keyword=null, favoriteOnly=false, tagId=null, limit=20)` and serial verification (concurrency 1), retaining only the last 50 result details with cumulative counts. Stop prevents new media checks after the current check; project changes discard old publications/scheduling, asset changes including A→B→A discard stale single results. No polling, automatic scan, persisted results or new global state. Ordinary detail/relations remain the usage entry. Missing/damaged records and references remain intact; explanatory reimport text is not an automatic operation.

## Local verification

- Frontend focused: 73 passed, one existing opt-in benchmark skipped. Includes 14 media UI/controller cases, Product typed command, Library reuse/search/pagination/detail and successor regressions.
- Latest Rust integrity contract: 7 passed, including independent classifications, stream-open disappearance, cross-project/traversal/nonregular rejection, immutable rows/files, 128 MiB video via 128 × 1 MiB chunks (buffered read never called), 33 MiB image preview unavailable and typed video/audio probe outcomes.
- Filesystem inspection: 2 passed; real Windows leaf and intermediate symlink fixtures exercised, including inside-project aliases. Filesystem adapter 3, AssetQuery 2, MediaProbe 2, generated import 8, source import 11, media protocol 4 passed. Library contract focused checkpoint: 20 passed and one existing opt-in benchmark ignored.
- TypeScript, frontend build, Rust fmt, all-target cargo check and architecture command passed. Existing compiler warnings were left untouched.
- Full local suites are NOT RUN, explicitly delegated to final exact-HEAD Source-only CI by this task. No old run is substituted.
- RAM/VRAM checked before runners, no user workloads stopped and no GPU generation. One cached source-import check briefly overlapped an already-started frontend target; both passed with available RAM. Subsequent runners were serialized; no strict-all-runners-sequential claim is made.

## Successor and freeze

The M4 proof validates 22 existing and 7 added source/test/guard paths with exact parent/live hashes and complete untouched aggregates before historical projection. Seven historical M1/M2/M3 manifests remain byte-identical. Phase13/14 historical file inventories exclude only validated successor backend additions, mirroring existing frontend projection; full live backend aggregate is still checked. Negative proof tests reject parent, legal-path, aggregate, count and invariant drift. No blind repinning, expanded generic allowlist, removed assertion or raised timeout.

Product 2.0.0-personal; migration 42; formal tables 71; no 043; backup 20. Queue, task state machine, workflow engine, binding OCC, telemetry and schema unchanged. No installer/tag/release. Stop after M4-1; relink/replacement/automatic repair deferred.

## Native and final CI

Owned Native evidence is recorded below. Final exact-HEAD CI remains required and is reported in the final task result; this document does not label pending CI as PASS.

### Owned Native result

PASS through the standard resource-checked `pnpm tauri dev` path, with an isolated owned copy containing 110 assets and 95 prerecorded terminal tasks. Explicit single checks showed: healthy image/video SAFE/PRESENT/READABLE/MATCH/PASS; missing file MISSING with downstream checks NOT_CHECKED; changed valid PNG MISMATCH while preview PASS; invalid image bytes with matching SHA MATCH while preview FAIL. Ordinary **使用位置**, historical generation-run relation and navigation actions remained visible. No private path was exposed by the new facts/scan UX. Native unreadable ACL denial is NOT_VERIFIED_PORTABLY; typed unreadable classification and UI presentation are covered by automated fixtures.

The first explicit project scan completed all 110 media despite the Library being filtered to `M4 05`/video. It reported 83 fixture problems (mostly deliberately invalid old thumbnail-only image fixture bodies), not 83 lost production assets; results remained limited to the latest 50 entries. A second explicit scan was stopped at 109/110, and subsequent observation retained 109 and **已停止**. The first slower automation attempt reached completion before Stop and is not counted as cancellation evidence. Automated controlled pending-response tests prove no next-item scheduling after cancellation.

Before/after exact row-content digests matched for assets 110, tasks 95, asset_versions/asset_relations/generation_snapshots/production_batches/production_batch_items/shot_reference_assets/reference-set/anchor binding tables (all 0). Every managed-media and thumbnail byte digest also matched, including expected missing files. Verification performed no writes, generation, queue/task creation or GPU work. The app and owned dev runner were closed afterwards. SQLite contains extra FTS tables; the formal-table query is the exact pool.rs whitelist, which returns 71, with migration MAX=42.

An initial owned fixture attempt violated an existing domain invariant by assigning source_task_id to a source_image. Fixture assembly was corrected to generated_image before retaking the baseline and conducting acceptance. No production code was changed to accommodate invalid fixture data.
