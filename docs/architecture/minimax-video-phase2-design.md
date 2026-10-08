# MiniMax Video V2 Phase 2 — persistent media input design

## Schema review before migration 043

Reviewed baseline: Phase 1 (`8a0d037443dcc66f804379357da13c5e6bc92f74`).
This is the schema decision, not a claim that Phase 2 has passed its gates.

Evidence from the current source:

- Migration 010: `shot_reference_assets` identifies `(shot, stage, asset)` and
  has stage-wide ordinals. It cannot represent explicit first/last slots, the
  same asset deliberately selected in separate slots, multimedia lists, or a
  workflow-version/recipe pair. `selected_image_asset_id` is a result selection.
- `shot_stage_configs.scalar_values_json` is parameter configuration, not
  asset membership; it has neither input revision nor deletion protection.
- Migration 042 project bindings use revision plus instance identity to
  prevent ABA. Shot input replacement must have the same concurrency property.
- Accepted Base 2.2 recipes: T2V has no media; I2V has `first_frame` (the
  product contract strengthens its optional recipe definition); first/last has
  two required singular images; Ref2VA has three optional plural fields,
  9 images/3 videos/3 audios. Existing H3 validation owns total/duration limits.
- Backup v20 exports/restores explicit shot and media collections in one
  repository transaction, remapping project/shot/asset and runtime identities.
  A new authority cannot be hidden in metadata and lost by this transaction.
- Source import creates managed assets without tasks, but category and
  `metadata.source` alone are not an attestation. A separate import receipt is
  necessary; it must be written atomically with the Asset by the import port.

Conclusion: existing tables cannot safely express the required authority.
An additive 043 is necessary. Migrations 001–042 remain byte-for-byte intact.

## Reviewed persistence model

1. `shot_video_input_sets`: header keyed by project, shot, exact workflow
   version and recipe; instance ID, positive revision and updated-at. Empty
   sets are retained so clearing inputs advances the revision rather than
   recreating revision 1. No Series/Episode/Scene prerequisite.
2. `shot_video_input_assets`: child rows keyed by header plus explicit input
   key and zero-based ordinal; asset ID. Ordinals are dense within each key.
   Allowed keys are first_frame, last_frame, reference_images,
   reference_videos, reference_audios. Singular slots accept only ordinal 0.
3. `external_asset_imports`: backend-only receipt keyed by asset, with project,
   verified media kind, content SHA, MIME, byte count, dimensions, duration and
   imported-at. Generic Asset insertion does not create receipts. No backfill
   from names, categories, generated assets or historical metadata.

Composite project/shot and project/asset foreign keys enforce ownership even
at repository boundaries. Child asset foreign keys are deferred NO ACTION:
individual referenced asset deletion fails at commit; whole-project cleanup
can explicitly delete assets and cascade the input set in one transaction without
order-dependent RESTRICT failures. The existing Asset→Project FK is deliberately
non-cascading and is not changed. Headers cascade on shot/project deletion. Runtime IDs are
soft, like project workflow bindings: removed Registry entries remain visible
as stale inputs; authorization still validates the exact live pair before writes
and preparation. There is no new queue, task or executor.

## Atomic writes and authority

Replace the whole input set under one SQLite transaction. An absent set requires
an absent token; an existing set requires the exact instance+revision token.
The conditional header write occurs before replacing child rows, serializing
competing writers. Conflict writes leave all rows untouched. Restore creates
new instance identities so tokens from the source installation cannot be reused.
Recipe switching reads a different header, never reinterprets old fields.

Backend adoption validates recipe input shape, project ownership, managed path
and file integrity. Every new image input also requires a matching import
receipt and an Asset with no source task. Managed generated videos remain legal
when the recipe supports them. Missing metadata fails closed. Import receipts
survive backup/restore with identity remapping and file checks; old backups
default to no receipts and require explicit reselection/reimport, never inferred
provenance. Receipt lookup is not a client-editable category flag.

## Preparation and compatibility contract

Single shot and batch readiness resolve the same saved exact-pair input set.
Preparation re-reads and validates current saved inputs; admitted batches freeze
typed asset IDs, hashes and context, and later edits do not mutate batches.
Queue Start remains explicit and Phase 1 admission remains in force. Direct
generation/queue paths also enforce external-image provenance, not only the UI.
No automatic frame extraction into image Assets. Video library posters are
previews only, never selectable source-image receipts.

Historical result selection, reference rows, tasks, assets and snapshots remain
readable. Missing formal slots/provenance display “需要重新选择素材”. No automatic
selected-image migration. Backup advances to v21 while continuing supported
v20-and-earlier reads. Asset deletion inspection includes saved input references.
Rollback means leaving additive tables and their data intact; old UI/runtime is
not a schema downgrade authorization.

## Validation and delivery

Owned databases only: fresh migration and v42 upgrade, full-replacement OCC,
ABA, cross-project denial, recipe isolation, stable ordering, empty-set revision,
restart, asset deletion and project cascade. Follow with pure import security,
all four preparation modes, immutable batches, backup roundtrip/legacy reads,
minimal typed UI scope guards, Phase 1 regression and successor negative tests.
Final full source gates and exact-head CI are required. Native four-mode GPU
acceptance is deferred to Phase 7 and cannot be claimed from mocks.

## Successor and literal package-byte profiles

The new Phase2 checkpoint is based on Phase1's final commit, not its planning
parent. Reviewed live Phase2 bytes are checked before projecting approved parent
bytes through the immutable historical proof readers. Phase1's schema/backup
invariants and original hashes remain unchanged; Phase2 declares 043 and backup21.

Existing checkout package text is CRLF whereas Git objects are LF despite a
clean Git status. Neither package tree is modified. The successor freezes both
literal raw-byte tree digests and defaults to the Git profile (including CI).
For this already-existing checkout only, local verification explicitly sets
`AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE=checkout`. Never auto-detect a profile from
current bytes or accept either newline representation per file. Invalid profiles,
profile drift, extra package files and newline-only byte drift fail closed.
Owned fixtures test both baseline representations and preserve the active profile.
This verification setting does not alter production package admission or storage.
