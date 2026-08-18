# DEV-036 Prompt Template + Production Context

## 1. Baseline

- Version remains `0.5.0`.
- Migration remains `021`; `BACKUP_VERSION` remains `12`.
- No new Prompt Template table/repository was introduced.

## 2. No-schema design

Prompt Versions whose `kind` is `prompt` and whose body contains `{{...}}` are templates. Existing Prompt Library records and existing `shot_stage_prompts` snapshots are reused. Snippets retain their existing literal behavior.

## 3. Syntax

The parser accepts only `{{variable.path}}` with optional surrounding whitespace. Variable characters are ASCII letters, digits, `_`, `-`, and `.`. Unclosed, unmatched, nested, empty, malformed, and unknown variables return stable template error codes; no expression engine or evaluation is used.

## 4. Built-in variables

Project, series, episode, scene, shot, and anchor metadata are exposed through the documented names. Series/episode/scene/shot numbers are one-based DB ordinals. Missing descriptions resolve to an empty string; missing structure returns `PROMPT_TEMPLATE_CONTEXT_MISSING` with shot identity.

## 5. Custom variables

`custom.xxx` values are supplied as a `Map<String,String>`. The renderer enforces 50 variables, 64-character keys, 4096-byte values, and 32 KiB total input. Required missing values return `PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING`.

## 6. Anchor Context

The request accepts up to 20 selected anchors for the current project. Selection order is preserved. Per-kind names use `、`; context uses `name：description` or `name`, joined by LF. Anchor asset IDs and paths never enter the template context.

## 7. Scene Context

Production structure is loaded as one tree and indexed in memory. A shot receives its project/series/episode/scene/shot context without per-shot structure queries. Unassigned shots remain valid unless a template requests missing structure variables.

## 8. Preview

`prompt_template_preview` validates prompt entry/version provenance and returns the selected shot, original template, rendered text, variables, context, and warnings. The operation is read-only. Bulk preview is capped at 50 entries and defaults to 20.

## 9. Bulk Apply

`prompt_template_apply` accepts 1–500 shots, one stage, one exact Prompt Version, one shared anchor selection, and custom values. It renders all selected shots in memory and writes final rendered text plus Prompt Library provenance to existing stage prompt snapshots.

## 10. Atomicity

Every shot is rendered and validated before the repository transaction starts. Any syntax, context, custom value, cross-project, or size issue aborts the request with zero stage-prompt updates.

## 11. Freeze Contract

Stage prompts store final rendered text, `prompt_entry_id`, `prompt_version_id`, and one shared `updated_at`. Later template, structure, or anchor edits do not mutate old snapshots; applying again creates the new frozen result. Generation reads the frozen prompt and does not render templates.

## 12. 500 Shot sanity

The bulk path loads shots, structure, and anchors through set-based calls, then performs in-memory indexing/rendering. The deterministic no-GPU compatibility test covers 500-shot structure handling and guards against per-shot service/query call sites.

## 13. Backup v12

No backup schema bump was made. Existing v12 export/import paths retain Prompt Library template text and preserve rendered stage prompts as static snapshots.

## 14. Regression

Focused evidence for DEV-036:

- Rust template/bulk tests: 16 passed.
- DEV-036 compatibility integration tests: 9 passed.
- Frontend template/localization tests: 9 passed.

Final handoff gate evidence:

- `cargo fmt --all -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- Rust full suite: 544 passed / 1 failed / 1 ignored. The only failure is the existing `production_orchestrator_service::tests::production_run_lifecycle_keeps_batch_task_asset_lineage_without_gpu` Krea2 no-GPU lifecycle test; it is unrelated to DEV-036 prompt-template wiring, so no product code was changed to mask it.
- Frontend full suite: 61 test files / 198 tests passed.
- `pnpm build`: PASS.
- `git diff --check`: PASS.

## 15. Database

No migration was added and no existing table contract was changed. The implementation uses existing Prompt Library, production structure, reference anchor, shot bulk, and stage prompt repository ports.

## 16. Architecture

Template parsing/rendering is a pure text operation. Context assembly is application-layer orchestration. Commands expose analyze, preview, bulk preview, and apply. `GenerationService` has no template/context dependency and production batch `values_json` is unchanged.

## 17. Final Decision

DEV-036 PROMPT TEMPLATE + CONTEXT PASS. The existing persistence model, deterministic validation/error codes, bounded custom/anchor inputs, set-based bulk context loading, read-only preview, atomic apply, and frozen rendered prompts are closed and handed off. The unrelated pre-existing Krea2 no-GPU lifecycle failure is recorded above; the release/version baseline remains unchanged.
