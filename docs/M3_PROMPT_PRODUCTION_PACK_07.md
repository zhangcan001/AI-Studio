# M3 PROMPT PRODUCTION PACK 07 = PASS

Date: 2026-08-10
Development line: `0.3.0`
Release status: development only; no `v0.3.0` tag or GitHub Release.

Pack 07 is implemented as a project-scoped Prompt/Snippet Library inside Studio. The backend is the source of truth; the frontend never creates a generation task while saving, comparing, applying, or preparing prompt variants.

## Delivered contract

- Migration `009_prompt_library.sql` adds `prompt_entries` and append-only `prompt_versions`. Migrations `001–008`, including organization migration `008`, are unchanged.
- Names are trimmed single-line values up to 120 characters and use Unicode lowercase normalization. Tags are canonical trimmed/deduplicated arrays with a 20-tag limit and 32-character tag limit. Text normalizes line endings, trims outer whitespace, and caps UTF-8 bytes at 64 KiB.
- `PromptLibraryRepository` and `PromptLibraryService` enforce project ownership, `(project_id, kind, normalized_name)` uniqueness, sequential version numbering, and cascade deletion of only the entry's own versions.
- Stable Tauri commands cover list/get/create/add-version/update-metadata/delete with kind, keyword, and tag filters. Keyword search is limited to names/tags.
- Studio contains a collapsible Prompt Library panel with Prompt/Snippet filters, tags, version counts, metadata updates, version text, compare added/removed lines, explicit textarea selection, replace confirmation, and snippet prepend/append/replace actions.
- Prompt versions apply exactly to a selected Studio textarea and never auto-generate. Two-to-eight selected Prompt versions can prefill the existing Pack 06 Experiment Planner; limits remain Planner-owned (max two dimensions/max 24 items).
- Backend and frontend project isolation reject cross-project reads, writes, applies, and experiment preparation.
- Backup manifest version 3 exports `promptEntries` and `promptVersions`, remaps entry/version IDs on restore, restores all rows under the new project ID, validates duplicates/references/ownership/kind/name/tags/text/version numbers, and rolls back the full transaction on prompt failure. v1/v2 archives remain accepted with empty prompt data and unchanged legacy rows. Diagnostics do not serialize prompt text.
- Pack 08 source work starts in `src/features/production/productionUx.ts` with pure queue-summary/recent-order/action contracts; no new executor or generation endpoint was introduced.

## Verification

Automated coverage includes Chinese create/rename, sequential append, tag/search, delete cascade, project isolation, invalid ownership, 64 KiB text limit, snippet semantics, Prompt Library → Experiment 2/8 variants and Planner bounds, v1/v2/v3 backup compatibility, duplicate/reference/ownership validation, ID remapping, and atomic restore rollback. The final source regression reports 301 Rust tests and 30 frontend test files / 93 frontend tests passing.

The five-action desktop live gate (v1, v2, compare, v2 → Studio manual generation, v1+v2 → two-item Kera2 experiment) remains environment-pending in this audit because the existing desktop window did not expose a controllable state and the local ComfyUI host had low free VRAM. No live GPU result is represented as a test pass. The third-runtime boundary remains `THIRD_RUNTIME_INPUT_REQUIRED`; no model was downloaded or modified.
