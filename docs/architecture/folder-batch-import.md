# Folder batch import v1 (v3 implementation plan)

## Entry and ownership

In the primary **分镜工作台**, choose **导入分镜脚本 → 选择文件夹**.
The advanced Shot workspace reuses the same panel under its structure menu.
Existing text import remains available through the secondary advanced-production link.
This imports a normal directory, not an archive. Source files are never rewritten.

The Rust service in `application/mod.rs` owns preview sessions. The frontend controller owns only this transient import UI, not Shot drafts or Studio Store state. All IPC goes through the typed transport. Persistence, input validation, entity binding and preparation reuse the existing services and repository ports. No schema migration, new Task model, executor or queue is introduced.

## Manifest

Place `manifest.json` at the selected root. Paths use `/` and are relative to that root.
Replace generator identities below with the **exact installed workflowVersionId + recipeId** pair; names are not identities.

```json
{
  "format": "ai-studio-shot-batch",
  "version": 1,
  "name": "Storyboard import",
  "generators": {
    "firstLast": {"workflowVersionId": "<exact version>", "recipeId": "<exact recipe>"},
    "reference": {"workflowVersionId": "<exact version>", "recipeId": "<exact recipe>"}
  },
  "defaults": {"durationSeconds": 6, "aspectRatio": "16:9", "resolutionPreset": "0.98"},
  "characters": [{"name": "Alice", "kind": "character", "images": ["images/alice.png"]}],
  "shots": [
    {"name": "Arrival", "mode": "firstLast", "prompt": "A slow camera move", "firstFrame": "images/first.png", "lastFrame": "images/last.png"},
    {"name": "Portrait", "mode": "reference", "prompt": "@Alice walks towards the camera", "references": ["images/background.png"]}
  ],
  "queue": {"create": true, "name": "Imported shots", "autoStart": false}
}
```

Modes:
- `image`: one explicit `firstFrame`, and a single-image Recipe whose key is `first_frame`.
- `firstLast`: independent `firstFrame` and `lastFrame`; never infer one from the other.
- `reference`: `references` plus explicit `@` entity covers; requires the formal three-field ReferenceMedia Recipe. Folder v1 accepts source images only. It does not infer compatibility from workflow names.
- This directory format does not add a T2V mode; existing T2V production elsewhere is unchanged.

`resolutionPreset` is an official MP tag, not a 480p/720p label. Alternatively specify both exact `width` and `height`. Do not combine the two resolution styles at the same level. The formal H3 compiler owns real duration, resolution and media constraints; the manifest's structural envelope does not override a Recipe's narrower limits (including the usual 4-second minimum).

## Preview and confirm

Preview scans safely and validates files, source provenance, exact installed generator pairs, entity references and product inputs. It writes **no** assets, Shots, Tasks, result links or batches. Errors are displayed per row; only valid selected rows can be committed. Unselected rows and their exclusive files/entities are not imported.

Names must be unique in the project. To deliberately import another copy, enter a suffix and choose **重新校验**. Only a successful server rescan produces a new monotonic preview version. Commit binds the session ID, raw manifest SHA256, preview version, suffix and row keys, then rechecks files and current project/generator/entity state before writing.

Default confirmation imports sources and prepares a batch without executing GPU work. **导入后立即开始生产** must be explicitly selected in the UI; a manifest flag alone never opts the user into execution. The existing ProductionStartAdmissionService and Production Queue remain the only start authority. Start rejection retains the prepared batch and reports a warning.

## Limits and ordering

- Manifest ≤1 MiB; 1–500 Shots; ≤100 declared entities.
- Safe scan ≤10,000 directory entries and depth 4; no links/reparse points, traversal, absolute or drive-relative paths.
- At commit, ≤500 distinct referenced files for selected rows and newly used entities. Unreferenced images do not consume this selected-file limit.
- **The existing formal single-batch cap remains 100 items.** Selecting more while requesting a batch is rejected before writes. Reduce selection or disable batch creation; there is no silent split or expanded queue allowance.
- Manifest entity `kind` is optional and must be `character`, `scene`, `prop` or `composition` (default `character`); any other value invalidates the whole manifest. A declared name that matches an existing project entity of a different kind is rejected at preview (INVALID_MANIFEST) before any write. A `composition` entity (e.g. `剧情01`, one uniquely named entity per layout image) marks its picture as the shot composition reference (framing, camera angle, subject placement, layout) in the compiled H3 prompt; mention it as `@剧情01`. Any declared or existing entity, including props and compositions, may be `@`-mentioned in `reference` mode. There is no `@` form for frames such as `@首帧`; frames use `firstFrame`/`lastFrame` only.
- 构图用途 (migration 048): a `reference` shot may set optional `openingComposition` and/or `endingComposition` to the name of a declared or existing `composition` entity that the shot also `@`-mentions, e.g. `{"prompt":"按 @取账前过渡 的构图……最后停在 @取账尾 的构图","openingComposition":"取账前过渡","endingComposition":"取账尾"}`. Both may be absent; the same composition cannot be both. Preview rejects unknown names (COMPOSITION_USAGE_UNKNOWN), non-composition entities (COMPOSITION_USAGE_WRONG_KIND), the same entity twice (COMPOSITION_USAGE_SAME), a name not `@`-mentioned (COMPOSITION_USAGE_UNBOUND) and use outside `reference` mode (COMPOSITION_USAGE_MODE). The usage is saved on `shot_story_entity_bindings.composition_usage` and is editable on the storyboard row (开场构图 / 结尾构图). The compiled H3 prompt marks the opening composition as governing only the opening frame and the ending composition only the final frame; among composition pictures the opening one is ordered first and the ending one last. Shots without 构图用途 compile exactly as before.
- Shot order follows the manifest after the project's existing ordinal; reference image order is explicit references first, then `@` covers in mention order, deduplicated. Matching is exact/case-sensitive, longest-name-first with the existing ASCII boundary rule.
- Existing entities are reused **read-only**; manifest images for them are ignored with a warning. New entities and source imports are never represented as generated images or completed Tasks.
- Batch status is **READY** with **PENDING** items until explicitly started. Preparation may create the existing NULL-task Shot-to-batch-item links; these are not generated results. Frozen batch values do not change after subsequent Shot edits.

## Staged writes and truthful compensation

`Previewed → Writing → Terminal(completed | compensated | partial)`.

Writes are separate existing transactions: A source assets; B new entities; C1 Shots; C2 scalar configs; C3 stage prompts; C4 entity bindings; D exact formal input sets with OCC; E optional official preparation. This is **not** a whole-folder transaction.

Normal failures compensate in reverse order. Only a new, never-started, all-PENDING batch may be archived then deleted through the existing Queue. No task cancellation or GPU interrupt is called. New Shots/entities/assets are removed through their services when safe; protected or concurrently edited data is retained and identified in `residual`. Source storage errors whose low-level file cleanup cannot be attested are reported as partial, not falsely described as complete cleanup.

The primary `{stage, code, message}` is separate from compensation errors and residual IDs. A terminal session returns its saved result on repeat without writing again. A per-project RAII lock prevents overlapping imports in this process. Panic releases that lock but does **not** promise compensation: the session remains non-repeatable Writing, and remnants require inspection.

Sessions are memory-only (30-minute preview/terminal TTL, at most 32; Writing is not expired). Process restart loses session receipts. There is **no durable import identity or crash-recovery transaction**. Same-name rejection prevents accidental duplication, but renaming or using a suffix intentionally creates another copy. Do not describe this as durable idempotency.

## Optional legacy compatibility

When `manifest.json` is absent, one of these may be read:
- `h3-batch.json`: `[{"image":"images/first.png","prompt":"Camera move"}]` → image mode.
- `h3-omni-batch.json`: `[{"images":["images/background.png"],"videos":[],"audios":[],"prompt":"Camera move"}]` → reference mode.

Choose the exact official generator, duration and official resolution in preview, then rescan. No generator is guessed from filenames or names. Nonempty legacy video/audio lists are explicitly rejected, not discarded. Two competing legacy manifests are rejected. A present modern manifest always takes precedence and cannot be overridden by legacy defaults. The original H3 import flow is unchanged.

## Verification and provenance

Existing same-repository source scanner, SourceAssetImportService, entity matcher, ShotBulkService, ShotService row-edit authority (the same authority used by CreationFacade), ShotVideoInputService, ShotBatchService, Queue and start admission are reused. GitHub search did not provide a suitable indexed implementation to copy; no external code or dependencies were added. The UI follows the existing neutral-dark/teal tokens and UI/UX Pro Max accessibility/interaction guidance; its optional local search script was unavailable.

Automated isolated SQLite tests cover independent slots, source receipts, exact reference ordering, frozen preparation, selection boundaries, stale previews, concurrent submission, read-only entity reuse, all committed substep rollbacks, retained cleanup failures, panic non-repeatability and explicit-start test seam. Frontend tests cover versioned rescan, disabled error rows, scope/late-response isolation, terminal receipts, explicit start opt-in and the actual mounted workbench entry.

Native acceptance must use an owned data root and WebView2 profile, record GUI/SQLite evidence, and confirm zero Tasks/GPU submissions. Automated adapter tests are not Native evidence. Runtime packages and pre-feature proofs remain byte-for-byte protected by the dedicated successor and historical parent projection.
