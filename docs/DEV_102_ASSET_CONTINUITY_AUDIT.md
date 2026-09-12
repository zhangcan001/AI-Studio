# DEV-102 Asset / Reference Continuity Audit

```text
TASK=DEV-102
BASELINE=d8a494fef30885ca877859948609de36841eb71e
AUDIT_RESULT=PASS
DATABASE_MIGRATION=NO
```

The current repository remains the source of truth. DEV-102 connects existing
read models and routes; it does not add an Asset store, Reference authority,
Shot-to-Asset table, delivery database, or file-copy pipeline.

## Relationship map

| Relationship | SOURCE_OF_TRUTH | DIRECTION | QUERY_AVAILABLE | UI_AVAILABLE | NAVIGATION_AVAILABLE | MISSING_CONTINUITY |
| --- | --- | --- | --- | --- | --- | --- |
| Asset → project ownership | `assets.project_id` and project-scoped Asset repositories | Asset → Project | Yes | Yes | Yes, through current project scope | None |
| Shot → direct reference assets | `shot_reference_assets` / `ShotView.referenceAssets` | Shot → Asset | Yes | Partial before DEV-102; exact cards now expose it | Exact Asset route added | Missing references must remain visible instead of becoming an empty preview |
| Asset → Shot reference usage | `AssetUsageService` / `asset_usage_get` | Asset → Shot | Yes, project-scoped and set-based | Existing usage panel; exact Shot actions added | Exact `shotId` route added | None after action wiring |
| Shot → selected image/video result | `shots.selected_image_asset_id` and `selected_video_asset_id` | Shot → Asset | Yes through `ShotView` and usage query | Existing candidate preview; explicit selected-result continuity added | Exact Asset route added | None after action wiring |
| Asset → selected result usage | `asset_usage_get` selected-keyframe bucket | Asset → Shot | Yes | Existing usage bucket; exact Shot action added | Exact `shotId` route added | None after action wiring |
| Shot → generated results | `ShotView.generationLinks.task.outputAssetIds` and existing Asset records | Shot → Asset / Task | Yes, bounded recent assets plus exact fallback reads | Existing candidate rail; generated/result section now distinct from inputs | Asset and Task routes exist and are wired | No new provenance state is required |
| Asset → production provenance | `assets.source_task_id`, task outputs, snapshots, production items, reviews | Asset → Task / Production | Yes through `AssetUsageService` | Asset preview usage panel and existing task action | Exact Task route for task relations | No new Generation History page |
| Asset → ReferenceSet/Profile | ReferenceSet items, profile defaults, costume/reference bindings | Asset → Reference authority | Yes through `asset_usage_get` | Existing usage buckets | Existing ReferenceSet/Profile workspaces remain authoritative | No redesign of consistency bindings |
| Project → deliverable Asset | Project Command Center derived selected output `assetId` | Project → Asset | Yes | Existing Complete action opens Asset Library exact asset | Exact `assetId` route already present and preserved | None after continuity wiring |

## Usage semantics

`AssetUsageService` is the only usage authority. The existing projection keeps
these meanings separate:

- `ACTIVE_REFERENCE`: direct Shot, ReferenceSet, Profile, scope, anchor, or
  active production relation; these can block deletion when the existing
  deletion authority says so.
- `SELECTED_OUTPUT`: `selected_image_asset_id` or
  `selected_video_asset_id`; this is also surfaced in the selected-keyframe
  bucket and remains authoritative on the Shot.
- `GENERATION_OUTPUT`: task output, production stage output, queue input/output,
  and snapshot relations; these are provenance/history unless the underlying
  active authority marks them as blocking.
- `REFERENCE_PROFILE`: Profile, ReferenceSet, costume, and scope bindings;
  these are displayed as existing semantic relationships, not new Asset
  categories.
- `HISTORICAL_REFERENCE`: legacy anchors, legacy Shot references, completed
  task history, and review references; these remain visible and are never
  silently replaced.

An Asset with no returned relations is shown as unused by the existing usage
read, but the UI does not infer that absence from Shot references alone: task,
snapshot, production, review, ReferenceSet, Profile, and legacy relations are
included before that conclusion is presented.

## Continuity invariants

```text
ASSET_CONTINUITY_EXISTING_AUTHORITY=PASS
ASSET_USAGE_DERIVED=PASS
NO_SECOND_ASSET_STORE=PASS
NO_SECOND_REFERENCE_AUTHORITY=PASS
ASSET_EXACT_ID_NAVIGATION=PASS
PROJECT_ISOLATION=PASS
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
APPLICATION_DIRECT_SQLX_NEW_USAGE=0
```

DEV-101 `assetRefs` continue to land in the formal Shot reference authority.
The Shot workspace now renders the exact reference IDs/assets (including a
non-silent unavailable state), and the Asset usage view can return to the
imported Shot. No handoff import, queue admission, task creation, or automatic
generation is performed by this continuity work.
