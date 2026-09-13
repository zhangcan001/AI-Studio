# DEV-122-B — Asset Library Data Layer Implementation

Status: **COMPLETE**
Version baseline: **AI Studio 1.3.1 Stable**
Scope: SQLite and Rust data layer only; no frontend or production-flow changes.

## 1. Implementation Summary

DEV-122-B adds the minimum v2 Asset Library foundation without creating a second
asset authority. The existing `assets` table, `Asset` domain model, asset
repository, reference systems, generation records, and usage services remain
authoritative. Two additive entities provide the missing capabilities:

- `AssetVersion` — immutable file and metadata snapshots for one existing asset.
- `AssetRelation` — typed, project-local, directed links between two existing assets.

No legacy rows are backfilled. Existing assets remain valid and can receive an
explicit version through the new service when a caller has enough provenance to
record it safely.

## 2. Migration

File: `src-tauri/migrations/033_asset_library_data_layer.sql`

The migration is additive only. It creates:

### `asset_versions`

| Column | Purpose |
| --- | --- |
| `id` | Stable `av_...` version identity. |
| `project_id` | Project isolation and cascade ownership. |
| `asset_id` | Existing authoritative asset identity. |
| `version_number` | Positive, immutable sequence number per asset. |
| `metadata_snapshot` | JSON snapshot captured at version creation. |
| `location` | Project-relative or external file location recorded for the version. |
| `checksum` | Content identity recorded for the version. |
| `created_at` | Immutable creation timestamp. |

`UNIQUE(project_id, asset_id, version_number)` prevents overwriting historical
versions. An ordering index supports version history and current-version lookup.

### `asset_relations`

| Column | Purpose |
| --- | --- |
| `id` | Stable `rel_...` relation identity. |
| `project_id` | Project isolation and cascade ownership. |
| `source_asset_id` | Directed relation source. |
| `target_asset_id` | Directed relation target. |
| `relation_type` | `SOURCE_OF`, `DERIVED_FROM`, `VARIANT_OF`, `REFERENCE`, `REPLACEMENT`, or `RELATED`. |
| `created_at` | Relation creation timestamp. |

Self-links are rejected by a table constraint, duplicate typed links are
rejected by a uniqueness constraint, and source/target indexes support lookup
from either side. Existing project, asset, shot, task, queue, and review tables
are not altered or removed.

## 3. Rust Domain and Repository

### Domain models

`src-tauri/src/domain/asset.rs` now contains:

- `AssetVersionId` and `AssetVersion` with validation for project ownership
  input, positive version numbers, non-null JSON snapshots, locations, and
  checksums.
- `AssetRelationId`, `AssetRelationType`, and `AssetRelation` with typed
  database serialization and self-link validation.

They are re-exported from `src-tauri/src/domain/mod.rs`.

### Repository port

`src-tauri/src/application/ports/asset_repository.rs` extends the existing
`AssetRepository` port with version insert/list/current and relation
insert/list/remove operations. Default unsupported implementations preserve
source compatibility for existing test fakes and non-SQLite adapters; the
SQLite adapter provides the real implementation.

### SQLite repository

`src-tauri/src/infrastructure/database/repositories/asset.rs` implements:

- project-and-asset-scoped version persistence;
- ascending version history and descending current-version selection;
- typed relation persistence and bidirectional asset relation queries;
- project-scoped relation removal;
- SQL row-to-domain validation and JSON/date decoding.

The insert queries require both referenced assets to belong to the supplied
project, preventing cross-project relation or version writes at the repository
boundary.

### Application service

`src-tauri/src/application/asset_data_service.rs` provides the application
boundary for the new operations. It parses IDs, verifies asset project
ownership, creates typed domain values, and maps repository failures without
touching commands, Queue Start, task state, or review state.

## 4. Compatibility and Migration Guarantees

- Migration 033 is additive and runs after the existing 001–032 chain.
- Existing `Project`, `Asset`, `Shot`, `Task`, and `ProductionItemReview` rows
  were verified unchanged when migration 033 was applied to a pre-033 database.
- Existing asset deletion continues to work through foreign-key cascade for the
  new version and relation rows; no alternate asset deletion path was added.
- Existing asset, prompt, generation, reference, usage, queue, task, and review
  authorities remain unchanged.
- No baseline version is guessed for a legacy asset; callers must explicitly
  provide its snapshot, location, checksum, and version number.
- `AUTO_EXECUTION=NO`; these operations only persist caller-supplied data and do
  not submit or start production work.

## 5. Tests and Verification

Added coverage includes:

- migration 033 preservation of existing project, asset, shot, task, and review
  rows plus new-table and foreign-key checks;
- asset version creation, ordered history, current-version selection, and
  duplicate version rejection;
- typed relation creation, lookup from source and target, self-link rejection,
  and removal.

Validation completed:

```text
cargo fmt --check = PASS
cargo check       = PASS
cargo test        = PASS
FRONTEND_CHANGED  = NO
```

The existing repository warnings remain warnings only; no warning suppression,
test skip, sleep, or failure removal was introduced.

## 6. Explicit Boundary for DEV-122-C

DEV-122-C may add the Asset Library frontend surface over this data layer:
typed transport, list/detail/history views, and basic management flows. It
must not create another asset store, bypass the Rust repository/service
 boundary, or change Queue Start, task execution, review, or production flow
 authority.
