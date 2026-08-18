# DEV-035 Production Structure + Project Manifest

## 1. Baseline

- Branch: `master`
- DEV-035 baseline: `14c6cc2466b2fedcc1a7f17df66bf6a96ee7c3e1`
- Version remains `0.5.0`.
- Frozen `v0.5.0^{}` remains `02e67cff50f5da1d207478071636af166048820c`.

## 2. Migration 021

`021_production_structure.sql` adds only `production_series`, `production_episodes`, `production_scenes`, and `shot_scene_assignments`. The existing `shots.project_id`, `shots.ordinal`, and all prior Shot tables remain unchanged. No migration 022 was added.

## 3. Structure Model

The persisted hierarchy is Project → Series → Episode → Scene. Series, Episode, and Scene names are trimmed, single-line, and capped at 100 characters; descriptions are capped at 1000 characters. Persisted ordinals are zero-based and the UI displays one-based labels.

## 4. Shot Assignment

`shot_scene_assignments` uses `shot_id` as its primary key and stores the scene-local ordinal. A Shot belongs to at most one Scene. Assignment accepts 1–500 Shot IDs in caller order, moves an existing assignment atomically, and rejects cross-project IDs with `PRODUCTION_STRUCTURE_PROJECT_MISMATCH`.

## 5. Tree Loading

The backend loads Series, Episodes, Scenes, assignments, and project Shot IDs with set-based queries, then assembles the tree in memory. The frontend keeps the full Shot list for Pipeline operations and applies Scene filtering only to the list view.

## 6. CRUD and Reorder

Create operations append using `MAX(ordinal) + 1`. Rename, delete, and reorder are available at all three hierarchy levels. Reorder validates complete coverage, duplicate IDs, unknown IDs, and cross-parent IDs before committing a transaction. Scene deletion cascades only its assignments; Shot rows survive as unassigned.

## 7. Scene Shot Order

Scene-local Shot reorder requires complete coverage and rewrites only assignment ordinals. Global Shot reorder remains the existing ShotService operation. Global move controls are disabled while query, status, or Scene filters are active.

## 8. Frontend Structure Panel

Shot Workspace now renders the structure tree, create/rename/delete/reorder actions, deletion confirmation, Scene-local Shot order controls, and a 1–500 Shot assignment picker. The picker supports name/Prompt search and bulk select. No UI automation or simulated key presses are used.

## 9. Scene Filter

The Shot list exposes All, Unassigned, and Scene filters. Filtering changes only the displayed list; `ProjectProductionPipeline` continues to receive the complete `shots[]` collection. Structure panel actions refresh the tree without changing global Shot ordinals.

## 10. Project Manifest Contract

`project_manifest_export` writes a separate pretty UTF-8 LF JSON file with format `ai-studio-project-manifest` and version `1`. It contains generated time, project ID/name/description, hierarchy, ordered assignment IDs, Shot prompt/status/stage configuration/reference data, and ordered Reference Anchor IDs.

## 11. Manifest Privacy and Publishing

Manifest output is a whitelist. It excludes `root_path`, absolute paths, local media bytes, ComfyUI endpoints, settings, secrets, and runtime-only fields. Sorting is deterministic, filenames are sanitized, and publication uses a temporary file followed by atomic replacement. The Shot Workspace exposes the export action and save dialog.

## 12. Backup v12

Project Backup advances from v11 to v12 and includes Series/Episode/Scene and Shot assignments. v1–v11 documents remain readable with empty structure defaults. Restore remaps Series, Episode, Scene, Shot, and Anchor IDs; unassigned Shots remain legal and no media files are copied by the structure layer.

## 13. Compatibility

Existing ShotService, Shot bulk import, Project Production Pipeline, production queue, generation, and Reference Anchor behavior remain the source of truth. No `shots.scene_id` column, Shot redesign, new generation executor, or live ComfyUI dependency was introduced.

## 14. Verification

- Rust: `522 passed`, `0 failed`, `1 ignored`; DEV-035 integration: `3 passed`, `0 failed`.
- Frontend: `60` files, `194` tests passed.
- `cargo fmt --all -- --check`, `cargo check --manifest-path src-tauri/Cargo.toml`, `pnpm build`, and final `git diff --check`: passed.
- No GPU run, live ComfyUI run, installer build, or 500-shot performance benchmark was performed. The structural sanity fixture covers 500 Shots, 50 Scenes, and 500 assignments.

## 15. Final Decision

DEV-035 is complete with migration 021 and Backup v12. The repository remains on `master`, version `0.5.0`, and the frozen release tags are unchanged.

`DEV-035 PRODUCTION STRUCTURE + PROJECT MANIFEST PASS`

Next: `DEV-036 — Prompt Template Variables + Anchor/Scene Context`
