# AI Studio 2.1 M3-1 — Library Findability and Scale Baseline

## Scope and authority
Parent: `ca01163fc4359daae9a3aa9dc47a0f4a328dc353`. Only normal Library findability is changed. Existing AssetBrowseRepository and PromptLibraryService remain search authorities; OrganizationService remains tag/favorite mutation owner. The new `product_library_tags_list` command is a read-only projection through the typed Product transport, not another service/repository.

Media/images/videos/audio expose keyword + favorite + tag-ID AND filters. Prompts search existing name/tag semantics, **not body**. Profiles/reference sets remain service-filtered complete categories. All remains each-category <=12 recent summary, without a merged cursor; it is labeled 近期资源 and has no media filters. No full Runs pagination, thumbnails, cache, new index, migration, polling owner, router or store.

## Query safety
Normalize trimmed keyword/empty-to-null, trimmed tag ID/empty-to-null and false/null favorites. Cursor binds exact project + category + normalized keyword + favorite + tag ID; any mismatch fails closed as LIBRARY_QUERY_INVALID. Media filters on non-media categories are rejected, not silently applied. Controller project-owned state resets search/filters/pages on project switch; existing request epochs prevent old list/tag responses from publishing. Deleted selected tags clear on the next refresh and start an untagged first page. Load-more uses only the active query's cursor (existing sequential replay intentionally retained).

## Parent measurements and frozen budget
The parent runtime was measured before any product changes. Baseline and budget are immutable at commit `d47216d67ecb2cb948cebd2a0ae113e52d07f135`; the new successor verifies this artifact against its Git blob. This is a regression budget, **not a product SLA**.
- Same-runner query p95 budget: max(parent same shape p95 * 3, 50 ms).
- Same-harness UI p95 budget: max(parent same pages p95 * 3, 500 ms).
- No threshold raised after failure. Candidate comparisons pass without optimization.

Fixtures contain 1k/10k media **and** 1k/10k prompt entries, generated/source/image/video/audio distributions, favorites and multiple tag identities; no large media files or real user libraries. Repository/service measurements are actual migrated SQLite queries. Parent favorite/tag measurements use existing AssetLibraryService because parent Product facade did not expose those filters. Cold means a newly opened connection pool, **not** an OS cache eviction. Warm media queries have a warmup; each shape has 21 measured samples; prompt samples are recorded without claiming explicit warmup.

UI observations run real React/controller in jsdom with typed API fixtures, seven repetitions. These measure controlled settling and DOM growth, **not WebView painting or Native IPC**. Node RSS includes the test runner. Actual IPC transport duration/bytes, JS renderer memory and Native first-paint timing are NOT_AVAILABLE; no invented numbers. Response bytes are serialized fixtures/service responses, not actual IPC wire bytes. 50k/100k were intentionally not explored or promised.

### Parent query latency (ms)
|Size|Shape|p50|p95|Approx response bytes|
|---|---|---:|---:|---:|
|1000|cold|3.627|4.355|12212|
|1000|combined|1.100|1.360|5501|
|1000|favorite|1.221|1.366|12208|
|1000|keyword|1.399|1.947|12244|
|1000|promptKeyword|0.668|1.015|6745|
|1000|tag|1.154|1.407|12631|
|1000|warm|1.115|1.329|12212|
|10000|cold|6.325|7.141|12228|
|10000|combined|5.087|5.452|5501|
|10000|favorite|2.520|2.769|12240|
|10000|keyword|6.395|7.295|12228|
|10000|promptKeyword|2.187|4.613|6745|
|10000|tag|2.473|2.604|12647|
|10000|warm|2.415|2.752|12228|

### Existing controller/UI scale cost
1/5/20 loaded pages mean 30/150/600 DOM resources and **1/15/210** cumulative Product list calls because each load-more refresh replays preceding pages. The new media path adds one read-only tag-list request per refresh, using the same lifecycle owner. This cost is recorded, not optimized in M3-1. Exact observed UI percentiles/RSS, rapid input and project switch counts, and prompt pagination costs are in the baseline JSON; candidate results are in the companion JSON.

### Query plans
Media uses idx_assets_project_created plus existing asset_favorites/asset_tag_links primary-key indexes for correlated EXISTS filters. Prompt uses idx_prompt_entries_project_updated and covering idx_prompt_versions_prompt_version; observed temporary B-tree for the last ORDER BY term is documented, not rewritten. No catastrophic bounded-query regression or migration requirement discovered.

## Verification
- Real Product Library contracts cover later-page keyword/favorite/tag/combined search, prompt beyond page1, every cursor mismatch, normalized equivalent queries, project isolation, recent-summary coverage and existing detail/version/relation/delete behavior.
- Frontend tests cover typed tag source/empty state, media-only controls, query/page resets, current cursor, stale keyword/favorite/tag/project responses, deleted tags, recoverable errors and M2-2 exact Create return/provenance/explicit slots.
- Static: TypeScript, frontend build, Rust fmt and all-target cargo check. The Phase12 example only received optional DTO defaults; its benchmark semantics remain unchanged.
- Architecture: immutable M1/M2 manifests; M3 successor validates exact legal before/after sources, full untouched aggregates, baseline budget and the one exact read-only added command signature. Historical signatures are still checked; no widened exemptions.
- Local full suites are intentionally delegated to the final exact-HEAD Source-only CI after focused tests/benchmarks/static/architecture/Native, per this task.

## Isolated Native acceptance
PASS: owned copy with 101 media, 61 prompts and three project-A tags plus a foreign project-B tag. Initial media/prompt pages each showed 30 items, excluding Old Asset101/Old Prompt21; keyword search found each directly without load-more. Favorite returned the old asset; Tag A returned one asset, Tag B four different assets without leftovers; images + keyword + favorite + Tag A returned the one expected item. Switching to My Film cleared keyword/favorite/tag, showed no A resources and only Foreign B tag; returning to default project retained correct prompt lookup. Actual UIA controls were used, no injected JS or mock Native IPC. Generate/GPU never called. Tasks/batches/items/snapshots/shot references remained zero before/after. Owned window closed and dev runner stopped; user apps/ComfyUI untouched. M2-2 exact return/provenance/explicit slots regressions passed in focused tests; previous runtime evidence is not mislabeled as a new real generation.

## Freeze
Product version2.0.0-personal; max migration42; formal tables71; no043; backup20. Queue/task/workflow/OCC/delete/reuse authorities unchanged. No release/tag/telemetry. Stop after M3-1; M3 remains incomplete and mixed Runs pagination unapproved.
