# DEV-047 Consistency Data Foundation

状态：完成（Rust data-contract foundation）
适用基线：AI Studio 0.6.2 / 0.7.0 architecture freeze

## 1. Baseline

- Repository：`C:\Users\ADMIN\Documents\ChatGPT\AI Studio`
- Branch：`master`
- `DEV047_START_SHA`：`9be76290da80a72b54e9c510ee6fe09ca27c2893`
- Start state：working tree clean，`HEAD == origin/master`
- Product version：`0.6.2`
- Existing migration maximum：`021`
- Existing backup version：`12`
- Existing manifest version：`1`

## 2. Scope and guardrails

DEV-047 delivers the first consistency data foundation only:

- serializable domain records;
- stable consistency IDs;
- pure validation primitives;
- application repository port contracts;
- domain and contract tests.

This DEV deliberately does not implement full Profile or ReferenceSet CRUD.
The following remain deferred to later DEV work:

- SQLite repositories and Migration 022;
- Tauri commands, application state, and frontend DTO wiring;
- Context Resolver, Readiness/Preflight, Production Preparation, and UI;
- changes to queue, orchestrator, generation, review, audit, or ComfyUI runtime.

`ReferenceAnchor` remains unchanged as the 0.6.2 compatibility relation.
Profiles describe semantic identity, ReferenceSets describe reusable ordered
asset relations, and Assets remain physical media.

## 3. Domain contracts

`src-tauri/src/domain/consistency/profile.rs` defines:

- `CharacterProfile`;
- `CostumeVariant` (a Character child, not a top-level Profile);
- `SceneProfile`;
- `PropProfile`;
- `StyleProfile`;
- `ProfileRevision`;
- `ProfileType` with only `Character`, `Scene`, `Prop`, and `Style`;
- `ProfileRevisionStatus` with only `Active` and `Archived`;
- `ConsistencyProfileRecord`, a typed enum containing only `Character`, `Scene`,
  `Prop`, and `Style` records.

`reference_set.rs` defines `ReferenceSet`, `ReferenceSetItem`, and
`ReferenceSetPurpose` (`CHARACTER`, `COSTUME`, `SCENE`, `PROP`, `STYLE`,
`SHOT`).

`binding.rs` defines `ShotProfileBinding`, `ShotReferenceSetBinding`,
`BindingRole`, and `InheritanceMode`.

All records use `chrono::DateTime<Utc>` timestamps and typed Rust fields. IDs
are represented as strings at this contract layer so DEV-048 can map them to
the existing SQLite text columns without introducing a second persistence
model.

## 4. Stable ID contract

Every generated ID is `<prefix><hyphenated UUID>` and is validated before it
crosses the domain boundary.

| Entity | Prefix |
| --- | --- |
| `CharacterProfile` | `cp_` |
| `SceneProfile` | `scp_` |
| `PropProfile` | `pp_` |
| `StyleProfile` | `stp_` |
| `CostumeVariant` | `cv_` |
| `ReferenceSet` | `rs_` |
| `ProfileRevision` | `prv_` |
| `ShotProfileBinding` | `spb_` |
| `ShotReferenceSetBinding` | `srb_` |

`generate_consistency_id` creates the canonical form.
`validate_consistency_id` rejects an empty/whitespace value, wrong prefix,
bad UUID, non-hyphenated UUID, and path-like input with a diagnostic
`INVALID_CONSISTENCY_ID` error.

## 5. Validation contract

Validation is pure and does not access a database.

- Profile names are trimmed for validation, must be non-empty, and are limited
  to 120 Unicode scalar characters. Input is never silently normalized.
- Optional descriptive text is limited to 4,000 Unicode scalar characters.
- Prompt fragments are limited to 20,000 Unicode scalar characters.
- `metadata_json` is limited to 64 KiB and must parse as a JSON object;
  arrays, scalars, and malformed JSON are rejected.
- ReferenceSet items require non-empty asset IDs, non-negative unique ordinals,
  unique assets, contiguous ordinals beginning at zero, and at most one primary
  item. A supplied role must be non-empty and bounded.
- Profile binding roles must match their `ProfileType`; `SHOT_REFERENCE` is
  reserved for ReferenceSet bindings; a CostumeVariant is valid only on a
  Character binding; binding ordinals cannot be negative.
- ReferenceSet binding ordinals cannot be negative.
- Cross-project ownership and asset existence are intentionally service/repository
  responsibilities for DEV-048, not faked inside pure validation.

## 6. Repository ports

The following `async_trait`, `Send + Sync` contracts use the existing
`RepositoryError` and strong domain records:

- `ConsistencyProfileRepository`: project/profile-scoped profile list/find/
  insert/update/delete, CostumeVariant operations, and immutable
  ProfileRevision list/find/insert operations;
- `ReferenceSetRepository`: project-scoped list/find/insert/update/delete,
  item listing, and atomic `replace_items` semantics;
- `ShotConsistencyRepository`: profile-binding and ReferenceSet-binding listing
  plus atomic replacement operations per shot.

No repository implementation is included. `replace_items` and the two shot
replacement methods define transaction boundaries for DEV-048; they do not
perform persistence in DEV-047. Revision content has no update operation.

## 7. Serialization contract

`ProfileType`, `ProfileRevisionStatus`, `ReferenceSetPurpose`, `BindingRole`,
and `InheritanceMode` serialize and parse using fixed uppercase values. The
domain records round-trip through `serde_json` without dynamic `Value` fields
or untyped repository tuples. Camel-case command DTOs remain a later command
layer concern.

## 8. Tests

`src-tauri/src/domain/consistency/tests.rs` covers:

- all frozen ID prefixes and invalid/path-like values;
- uppercase enum serialization and database parsing;
- Unicode name, description, prompt, and metadata boundaries;
- ReferenceSet ordering, uniqueness, primary, owner, and role rules;
- profile/reference binding compatibility and ordinal rules;
- JSON round trips for all required domain records;
- common `ConsistencyProfileRecord` accessors and CostumeVariant exclusion;
- repository port `Send + Sync` and object-safety compilation contracts.

No test uses SQLite, network access, GPU generation, or a Tauri command.

Final verification:

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`：PASS
- `cargo check --manifest-path src-tauri/Cargo.toml`：PASS
- `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`：
  639 passed, 0 failed, 1 ignored
- `pnpm test`：80 files, 289 tests passed
- `pnpm build`：PASS
- `git diff --check`：PASS

## 9. Compatibility and deferred work

- No `src-tauri/migrations/022*.sql` exists; migration maximum remains 021.
- 0.6.2 Project, Shot, Asset, ReferenceAnchor, Prompt, Queue, Run, Review,
  backup version 12, and manifest version 1 contracts are untouched.
- Existing legacy shots do not require a ReferenceSet or profile binding.
- DEV-048 may add Migration 022, SQLite transactions, CRUD, project-boundary
  checks, and asset/reference integrity checks on top of these contracts.
- DEV-049 may consume these records for context resolution; DEV-050 may build
  readiness and preflight on the resulting context.

## 10. Final decision

**DEV-047 CONSISTENCY DATA FOUNDATION PASS**

Next task: **DEV-048 — Profiles + Reference Sets**.
