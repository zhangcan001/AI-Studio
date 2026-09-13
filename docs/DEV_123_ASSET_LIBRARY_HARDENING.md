# DEV-123 — Asset Library Integration & Hardening

```text
TASK=DEV-123
BASELINE=AI_STUDIO_v1.3.1_STABLE
SCOPE=INTEGRATION_AND_HARDENING
NEW_FEATURE=NO
PROMPT_STUDIO=NO
TOOL_HUB=NO
AI_FEATURE=NO
SEARCH_ENGINE_REWRITE=NO
```

## 1. Project isolation

The hardening fixtures create two projects, each with two assets, one version,
and one typed relation. The results are:

- Asset list queries are scoped by `project_id`; a project only receives its
  own assets.
- Asset detail rejects an asset requested through another project boundary.
- Version and relation reads first verify asset membership in the requested
  project and reject cross-project access.
- Relation creation rejects endpoints from different projects.
- Relation endpoint names are resolved within the same project boundary.

The existing list and detail query tests cover the browse/detail side, while
the AssetDataService test covers version/relation access and cross-project
relation creation.

## 2. Data consistency behavior

| Operation | Defined behavior | Verification |
| --- | --- | --- |
| Delete asset | The existing transactional asset delete remains the authority. SQLite foreign-key cascades remove that asset's `asset_versions` and `asset_relations`; project, task history, unrelated assets, and existing production history remain. | `deleting_asset_cascades_v2_rows_and_preserves_unrelated_records` plus existing repository deletion tests |
| Delete version | No version-delete API exists. Version rows are immutable history and are not overwritten by a new version. | Existing duplicate-version rejection and current-version tests |
| Delete relation | Explicit project-scoped relation removal deletes only the selected relation. It does not affect either endpoint or version history. | `create_query_and_remove_asset_relations` |
| Orphan prevention | Asset relations and versions have foreign keys with `ON DELETE CASCADE`; normal service creation also requires same-project assets. | Migration and cascade tests |

Queue, Task, Review, and Production Flow authorities are unchanged.

## 3. Performance fixture

The Rust repository test simulates:

- 1,000 assets;
- 10,000 immutable versions (10 versions per asset);
- 10,000 typed relations;
- filtered, project-scoped browse queries;
- detail lookup, version list, current-version lookup, and relation list.

The test asserts bounded page results and exact per-asset version/relation
counts. Existing keyset pagination and the migration indexes remain in use;
no search-engine rewrite or speculative timing threshold was added, avoiding a
machine-dependent flaky gate.

## 4. Storage handling

Asset reads continue through `AssetQueryService` and `FileSystemAssetStore`.
Project membership is checked before filesystem access. If a tracked file is
missing or has been moved away from its recorded path, the read returns the
existing explicit storage-read error instead of silently returning a preview.
The frontend preview renders a user-facing unavailable-preview state.

## 5. Frontend robustness

The Asset Library tests cover:

- loading state while the project-scoped list is pending;
- empty project state and filtered-empty state;
- visible list error state while retaining the project boundary;
- detail version/relation loading and successful rendering;
- explicit version-history empty/error rendering already provided by the MVP.

## 6. Validation

```text
FRONTEND_TEST=PASS
TSC=PASS
BUILD=PASS
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_TEST=PASS
REMOTE_CI=REQUIRED_AFTER_PUSH
```

Only hardening tests and this record are added in DEV-123. There is no new
module, queue, task, review, generation, cloud, AI, or automatic-execution
path.
