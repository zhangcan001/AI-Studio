# DEV-048 — Profiles + Reference Sets

## Baseline

- `DEV048_START_SHA`: `c417d08ea648d4376acb61bcbeed40852887161f`
- branch: `master`
- baseline workspace: clean
- baseline `HEAD == origin/master`: yes
- product: `0.6.2`
- migration: `021`
- backup: `12`
- manifest: `1`

## Migration 022

`src-tauri/migrations/022_consistency_profiles_and_reference_sets.sql` adds exactly the frozen DEV-048 tables:

- `profile_revisions`
- `reference_sets`
- `style_profiles`
- `character_profiles`
- `scene_profiles`
- `prop_profiles`
- `costume_variants`
- `reference_set_items`
- `shot_profile_bindings`
- `shot_reference_set_bindings`

The migration includes project/shot/asset/character foreign keys, fixed uppercase CHECK values, immutable revision uniqueness, ordered item uniqueness, boolean/ordinal constraints, and project-scoped case-insensitive unique-name indexes. Required query indexes cover profile project/update ordering, reference-set purpose/update ordering, item ordering, revision lookup, and both shot-binding orderings.

No scene binding table, cache, snapshot, storyboard, scheduler, or production table was added. There is exactly one `022_*.sql` migration and no `023` migration.

## Persistence

- `SqliteConsistencyProfileRepository`: four profile tables, costume variants, and immutable profile revisions. Reads use fixed SQL per `ProfileType`; list order is stable; deletes reject shot bindings, reference-set ownership, default-style references, costume children, and revision history.
- `SqliteReferenceSetRepository`: project-scoped reference-set CRUD, ordered item listing, and transactional `replace_items`. Missing assets, duplicate constraints, and other insert failures roll back the prior item set. Deletes reject active reference-set relations.
- `SqliteShotConsistencyRepository`: stable list/transactional replacement for profile and reference-set shot bindings. FK failures roll back the complete replacement.

The repositories are registered only through `repositories/mod.rs` and `database/mod.rs`; no `AppState`, command, or UI wiring was added.

## Application services

`ConsistencyProfileService` provides list/get/create/update/delete for Character, Scene, Prop, and Style profiles plus costume CRUD. It verifies projects, generates the frozen IDs, trims names, preserves prompt text, validates text/metadata/default relations, and preserves identity/creation timestamps on update.

`ReferenceSetService` provides list/get/create/update/delete, transactional item replacement, and explicit `create_from_anchor` conversion. It validates owner pairs, project boundaries, image-only assets, missing assets, ordered item rules, and the 20-image limit. Asset validation uses one `find_many_by_ids` call. Anchor conversion preserves asset order, sets only the first item primary, leaves roles empty, and does not copy or mutate media. V1 conversion is explicit and name uniqueness is enforced by the existing project-scoped index.

## Revisions and compatibility

`profile_revisions` supports insert/list/find only; duplicate `(profile_type, profile_id, revision_number)` and empty hashes are rejected. Profile updates do not fabricate revisions.

Fresh `001→022` and isolated `021→022` upgrade tests verify the ten new tables and that they start empty. Legacy Project, Asset, Shot, ReferenceAnchor, Prompt, ProductionSeries, Episode, Scene, ProductionBatch, and ProductionRun sentinels remain intact. No old anchor is backfilled into a Profile or ReferenceSet.

Existing migration assertions in `pool.rs` and the DEV-035/036/040 compatibility fixtures were updated only for the required new maximum migration/table count; no product or backup compatibility version changed.

## Scope and deferred work

Implemented only domain-persistence/application CRUD and compatibility coverage. Deferred to DEV-049: inherited context resolution, conflict resolution, readiness, prompt assembly, production integration, commands, UI, queue, scheduler, and generation runtime wiring.

`PORT_CHANGED=NO`: DEV-047 repository contracts were sufficient and unchanged.

## Verification

- Agent A: migration 022 and profile/revision SQLite repository; no commit/push.
- Agent B: reference-set and shot-consistency SQLite repositories with rollback tests; no commit/push.
- Agent C: profile and reference-set application services; no commit/push.
- Agent D: fresh/upgrade, CRUD, rollback, boundary, binding, and anchor-adapter integration tests; no commit/push.
- Parallel Wave 3: `cargo fmt --check` PASS; `cargo check` PASS with 109 expected warnings; frontend `pnpm test` PASS (80 files, 289 tests).
- Final Rust: `cargo test -- --test-threads=1` PASS — lib 608 passed/0 failed/1 ignored; DEV-035 3, DEV-036 9, DEV-039 5, DEV-040 6, DEV-041 10, DEV-042 10 passed.
- Final frontend: `pnpm build` PASS; TypeScript/Vite built 181 modules. `git diff --check` PASS.
