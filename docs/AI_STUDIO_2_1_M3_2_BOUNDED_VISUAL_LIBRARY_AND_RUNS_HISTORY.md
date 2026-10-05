# M3-2 bounded visual Library and Runs task-history entry

Parent: `8842a2750e4402ca8e45a6e4efc4571c3e34a425` (M3-1 exact-head CI `37256828538` passed). Master-direct; no release, schema, SQL/index, queue, workflow, OCC or task-history authority changes.

## Measured seam and pre-implementation targets

The immutable M3-1 artifacts describe real SQLite queries and controlled React/jsdom observations, **not Native IPC/paint/renderer-memory metrics**. Existing 1/5/20-page replay made 1/15/210 Product list calls and retained 30/150/600 cards. `m3-2-library-targets.json` was written before product implementation: 20 list calls maximum through page20, 30 cards per keyset page, zero offscreen thumbnail requests, zero full-media list reads, concurrency4. No targets or M3-1 budgets were raised.

## Implementation

- Current-page-only keyset navigation retains query-scoped page-start cursors, not old page data. Next, previous, explicit refresh and fallback refresh each read only the selected page. Commit page index/history only on latest successful scope-owned response; polling does not cancel pending navigation. Project/category/normalized keyword/favorite/tag changes reset the stack. All remains recent-summary, Profiles/Reference Sets remain complete-category; neither gets artificial pagination.
- Tags use the existing readonly Organization adapter. Project, explicit refresh and relevant invalidation refresh tags; navigation and five-second list polling do not. Deleted selected tags clear the filter and return to page1.
- Visible-only image/video cards read existing managed thumbnails through `product_library_thumbnail_get` -> `AssetQueryService.read_thumbnail`. No storage path/MIME is invented or returned; missing/read-failed resources remain usable. Read failures do not return filesystem technical detail. No full image/video fallback, thumbnail generation or persistent cache.
- One mounted-Library transient IO limiter retains at most four outstanding reads across page/project transitions. IntersectionObserver uses the Library scroll root with bounded80px margin. Cleanup cancels queued old work, rejects stale publication and revokes every created object URL. Audio/Prompt do not request thumbnails.
- Runs explains recent50 tasks/production plus unarchived queues and exposes “查看完整任务历史”, navigating to existing project-settings/advanced-tasks. TaskHistory owns search/filter/cursor; mixed Run history pagination remains unapproved.

## Controlled candidate and regression protection

`m3-2-library-scale-candidate.json` uses the M3-1 harness with only bounded Next-page interaction/expectations adapted. Both1k/10k media+prompt fixture sizes and1/5/20-page observations now show list calls1/5/20 and DOM30/30/30. Approximate fixture response bytes, seven-sample settle p50/p95 and Node-process RSS are recorded; Node RSS includes runner, not WebView memory. A separate actual IntersectionObserver/component fixture mounts10 eligible cards: two visible read twice, three later visible bring total to five; offscreen0, full-media0, object URLs5 created/5 revoked.

M3-1 query evidence is reused only because list/repository/query-plan sources are unchanged. Candidate UI p95 must still stay within original `max(parent p95*3,500ms)` budget. Native IPC timing, WebView paint and renderer memory remain NOT_AVAILABLE. No50k/100k promise.

The M3-2 successor validates exact parent/live legal paths and untouched aggregates before historical projection. M1/M2/M3-1 manifests and all M3-1 scale artifacts remain immutable. The existing stylesheet guard recognizes only validated Library stylesheet bytes; its historical inventory test records the exact one-line/three-selector delta without changing debt budgets or other counts. Negative tests reject parent/path/file-set/aggregate/invariant drift, raised budgets, replay/DOM regressions and offscreen/full-media reads.

## Verification checkpoint

Targeted pagination/filter/race/thumbnail/Runs and real Rust Product Library contracts, TypeScript/build, Rust fmt/all-target check and architecture checks are run sequentially with RAM/VRAM inspection. Full local suites are delegated to the final exact-head Source-only CI by this task. Native and final CI results are recorded after completion; no previous-head CI is substituted.

## Freeze and stop

Version2.0.0-personal; migration42; formal tables71; no043; backup20; no tag/release/telemetry. Stop after M3-2; no M3-3, mixed Runs pagination or M4 starts automatically.

## Owned Native acceptance

PASS with101 owned image/video assets (managed PNG thumbnails plus missing-thumbnail cases) and95 prerecorded terminal historical tasks; no real project data. Native1→2→3→2 returned resource ranges001–030,031–060,061–090,031–060 with30 cards each and correct navigation states. Actual scrolling/focus displayed blue image032 and orange video036 managed thumbnails; video031 retained missing placeholder. No resource detail was opened to disguise list-thumbnail reads.

Runs showed recent coverage and its ordinary complete-task-history action opened existing TaskHistory. Load-more grew historical task rows30→60; exact task search returned1; failed/completed filters returned0/1; today filter returned0 for October1 historical fixtures. Existing authority and controls were not modified.

Owned WebView network observation was enabled before interaction: full video protocol requests0. Product read-only debug events (no IDs, prompts or paths) recorded5 thumbnail requests and0 full-image preview reads during the final Native session. Network command counts are not used for IPC timing or for postMessage-based thumbnail-count inference. Object-URL cleanup is proven by automated page/project/unmount and late-response tests, not claimed as a measured Native heap metric. Owned app, dev runner and network observer stopped. Task count95 and all queue/batch/item/snapshot/shot-reference counts0 were unchanged.

Fixture setup was corrected without business changes: system default project is intentionally absent from normal project selection, so fixture resources use the visible owned project; SQLite timestamps use repository-canonical RFC3339 offsets, and terminal tasks include queued/started/finished timestamps. Incomplete initial fixture attempts are not counted as Native PASS.

## Exact-head CI repair checkpoint

Initial run37311861499 reached the existing frontend10-minute job limit and exposed two default5-second static guard cases plus one stale M2 live-aggregate expectation. Business code and CI configuration were not changed to address this. M2 now compares the validated live successor aggregate instead of immutable M3-1 aggregate; all historical manifests stay unchanged. Immutable Git trees/blobs are batched and reused, while every valid invocation freshly reads live bytes, file sets and full untouched aggregates. Invalid proof headers/scope/invariants fail closed before historical projection; no validity result is cached. Negative assertions remain intact. Separate IPC/backend and historical consumer checks (and observability versus non-CSS style consumer checks) retain every assertion and their original default timeout without increasing it. Repeated full-chain validations within the same synchronous test are reused locally, not across tests/source changes. Warm new-guard measurement decreased from252–274ms to140–162ms; final M3-1/M3-2 proof tests decreased from initial CI21.8s/15.7s to local7.4s/4.1s (runner differences prevent a claimed normalized CI speedup). Final authoritative CI must match the repair commit.
