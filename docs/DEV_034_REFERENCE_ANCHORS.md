# DEV-034 Reference Anchors

## 1. Baseline

- Branch: `master`
- `DEV034_START_SHA`: `4102ae53f7c99b88091f86f58534f07d647c62b4`
- Version remains `0.5.0`.

## 2. Existing Asset Library Reuse

Reference Anchors extend the existing Asset Library selection, paging, preview, tags, favorites, import, and organization flows. No second Asset Library or selection system was introduced.

## 3. Anchor Model

The only supported kinds are `CHARACTER`, `SCENE`, `PROP`, and `STYLE`. An Anchor stores project-scoped metadata and an ordered image membership list.

## 4. Migration 020

`020_reference_anchors.sql` adds only `reference_anchors`, `reference_anchor_assets`, and the project/kind and anchor/ordinal indexes. Primary reference is derived; there is no `primary_asset_id` column. Fresh migration and an existing 019 database both apply successfully, with foreign-key cascades enabled.

## 5. Ordered Assets

Create and update accept `assetIds[]`. The service deduplicates IDs and rewrites ordinals from zero through nineteen in one transaction. Assets must be images from the same project; videos and audio are rejected.

## 6. Primary Reference

The first ordered membership is the primary reference. If it is deleted, the next remaining ordinal becomes primary naturally. Empty Anchors are legal after update or asset deletion and report `usable = false`.

## 7. Asset Library UI

The existing `AssetLibrary` now exposes the Reference Anchor panel. It supports creation from selected images, kind/name/description editing, ordered add/remove/up/down actions, primary-by-front, filtering, and Anchor deletion. Deleting an Anchor does not delete Asset rows, tags, or favorites.

## 8. Shot Apply

Shot Workspace lists project Anchors and offers append/replace actions. The frontend computes the final ordered asset IDs, deduplicates them, validates the result, and calls the existing `replaceShotReferences` command. No Anchor apply command or new executor was added.

## 9. Snapshot Contract

Applying an Anchor writes only the current ordered Asset IDs to the Shot reference relation. There is no `anchor_id` live dependency, so later Anchor edits do not mutate an already-applied Shot.

## 10. REF2VA Ordering

Anchor order is preserved exactly. Existing REF2VA validation is applied to the final list, and `ensurePrimaryReference` keeps the selected key image first without sorting or silently truncating references.

## 11. Asset Delete

`reference_anchor_assets.asset_id` uses `ON DELETE CASCADE`; deleting an Asset removes only its memberships. The Anchor remains available for the remaining images or becomes unusable when empty.

## 12. Backup v11

Backup format version 11 exports Anchor kind, name, description, and ordered memberships. v1-v10 documents deserialize with empty Anchor data. Restore generates new Anchor IDs and remaps membership IDs through the existing restored Asset map. The v11 round-trip test preserves order and never writes source Asset IDs into the restored project.

## 13. E2E

The isolated no-GPU DEV-034 test covers Character, Scene, Prop, and Style creation; B/A/C ordering; update/reorder; cross-project rejection; video rejection; Asset deletion cascade; Anchor deletion; and Asset preservation.

## 14. Regression

- Rust full suite: `512 passed`, `0 failed`, `1 ignored`.
- Frontend full suite: `58` files, `189` tests passed.
- `pnpm build`: passed.
- `cargo fmt --all -- --check`, `cargo check`, and `git diff --check`: passed.
- No GPU, ComfyUI live run, installer build, or 500-shot benchmark was run.

## 15. Architecture

The implementation reuses the existing AssetRepository, Asset Library, ShotService contract, and queue boundaries. It adds no second AssetRepository, ShotService, Asset Library, production queue, executor, direct `/prompt` path, or live Shot-to-Anchor dependency.

## 16. Final Decision

Frozen release references were preserved:

- `v0.5.0^{}` = `02e67cff50f5da1d207478071636af166048820c`
- `v0.4.0^{}` = `94918f6322ce690ff7b1630961abb56b8a31ed11`

`DEV-034 REFERENCE ANCHORS PASS`

Next: `DEV-035 — Series / Episode / Scene + Project Manifest`
