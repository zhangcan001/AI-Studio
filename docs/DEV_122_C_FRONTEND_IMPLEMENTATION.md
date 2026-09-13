# DEV-122-C — Asset Library Frontend MVP

Status: **COMPLETE**
Baseline: **AI Studio 1.3.1 Stable**

## Scope

Implemented the first read-mostly Asset Library experience over the existing
asset authority. The page stays project-scoped and does not create a second
asset store, queue, task, review, or production-flow path.

## Page and components

The existing `AssetWorkspace` continues to host the default `AssetLibrary`
page. The MVP now provides:

- **Asset List** — `AssetGrid` and `AssetCard` show name, type, tags, current
  version state, file summary, and updated time. Existing thumbnail/media
  preview, favorite, pagination, comparison, and organization controls remain
  in place.
- **Filters** — the existing typed, project-scoped query supports keyword,
  category, media type, source kind, tag, favorite, and sort. The current
  project is shown explicitly as the project filter boundary; cross-project
  reads are not allowed by this page.
- **Asset Detail** — `AssetPreview` now presents preview, metadata, tags,
  project scope, provenance, usage, and typed asset relations.
- **Version History** — the detail view reads immutable version records,
  selects the highest version as current, and renders history newest first as a
  read-only list.
- **State handling** — list loading, next-page loading, empty results, media
  preview failure, list errors, and detail history errors have explicit UI
  states.

## Typed data boundary

The frontend uses `src/services/tauriClient.ts`, which delegates to the typed
transport in `src/services/ipc.ts`. DEV-122-C wires the existing DEV-122-B
`AssetDataService` into two read commands:

- `asset_versions_list`
- `asset_relations_list`

The relation command returns stable IDs, typed relation names, and endpoint
display names. Project validation is applied before and during the Rust
service calls. No component accesses SQLite or invokes Tauri directly.

The asset summary transport also exposes `updatedAt`; legacy fixtures and
older callers fall back to `createdAt`.

## Tests

Focused frontend coverage is in:

- `src/features/assets/AssetLibrary.test.tsx`
  - populated list metadata;
  - project scope;
  - typed media/tag filter queries;
  - loading and empty state copy;
  - manual import entry remains the existing native action.
- `src/features/assets/AssetPreview.test.tsx`
  - metadata and provenance sections;
  - relation display;
  - current and historical version display.

Rust compilation coverage includes the new command wiring and the existing
DEV-122-B migration/service tests. The existing asset summary security test
continues to ensure internal storage and generation metadata are not leaked by
the public summary DTO.

## Limitations and explicit non-goals

- Legacy assets without an explicit `asset_versions` row display
  **未建立版本记录**; the UI does not guess or backfill a version.
- The current AssetView transport does not expose a free-form description,
  model identity, or full prompt/generation record. The detail view shows
  honest empty/available states and uses `sourceTaskId` where present.
- Relation records are read-only in this phase; no graph editor or mutation UI
  was added.
- No AI tagging, auto classification, vector search, cloud sync, automatic
  generation/import, or media editing was added.
- Queue Start remains the only production execution gate. Queue, Task, Review,
  and production flow behavior was not changed.

```text
QUEUE_CHANGE=NO
TASK_CHANGE=NO
REVIEW_CHANGE=NO
PRODUCTION_FLOW_CHANGE=NO
AUTO_GENERATION=NO
```
