# AI Studio 2.1 M2-2 — Create / Library precise reuse

## Scope and authorities

- Parent: `974d59e513961155a1b551b4de3da5658e1359bb` (M2-1 accepted).
- Recent shortcuts remain 20 Prompt entries (latest versions), 100 media assets plus existing Shot links. Older resources use the existing project-scoped Library name query and 30-item keyset pages.
- Studio Store remains the sole draft owner. Its existing return marker carries the exact Create Project/Shot/Stage route. Library-origin reuse preserves generator, values, dirty state, RunRef and accepted state until an explicit input edit invalidates accepted state under the existing contract.
- Standalone Library requires a visible image/video choice. Audio remains readable/playable but is not advertised as ordinary image/H3 input.
- Media intent resolves the exact project-scoped asset ID. Single and multiple compatible slots both require a click; only the selected draft field changes. Returned older draft assets resolve by exact ID too; project/epoch cancellation excludes stale responses. No generation or Shot reference write occurs.

## Prompt provenance

- Recent choices and Library intents carry `promptId` + `promptVersionId`. Explicit typed selection atomically writes text/provenance into Studio Store; manual edits/removal clear provenance even if the resulting text is identical. No identity lookup by text/name/path/time.
- Provenance travels with draft snapshots through stage and advanced/settings/Library returns and through readiness/generate requests.
- Shared Rust `prepare_submission` uses PromptLibraryService to validate both IDs present or absent, project ownership, exact entry/version and exact immutable text. Invalid provenance yields `INVALID_INPUT / EDIT_INPUT / prompt` before queue/task writes. Ordinary manual text without IDs remains valid.
- Valid provenance is forwarded through existing DirectGenerationContext into GenerationSnapshot.prompt_version_id. Prompt model metadata is not execution model authority; model_version_id remains None.

## Freeze and boundary

No new query/store/router/polling/execution authority, migration, backup version, telemetry, release or product-version change. Migration max42 / formal table71 / backup20 / product2.0.0-personal.

The new M2-2 review validates immutable parent blobs, exact legal changed/new paths, complete live aggregates and untouched aggregates before M2-1/M1 historical parent projection. Old manifests are unchanged. Historical guards intentionally inspect the validated historical projection; the M2-2 manifest separately pins the actual live Rust aggregate. Negative proofs fail closed.

## Verification

Targeted frontend and controlled Rust submission/Library tests, TypeScript/build/fmt/check, IPC parity and architecture guards are the local gates. An owned isolated Native fixture has 21 prompts, 101 images and a two-image-slot H3 generator; no real project/GPU/generation is used. Exact final-head Source-only CI owns full-suite authority. Detailed final evidence is reported in chat; unperformed gates are never represented as PASS.

### Local and Native evidence

- Focused final frontend: 42 passed in 5 targets, including explicit selection/clear, exact return, late-project response rejection, older asset re-resolution, Shell names and fail-closed successor probes.
- Controlled Rust: 4 creation submission contracts plus 1 Library typed-intent contract passed. Valid A2 identity reaches immutable GenerationSnapshot; foreign/wrong/changed/partial identity fails both readiness and generate without queue/task writes; manual text remains valid; recent choices keep exact identity and bound20.
- TypeScript, frontend build, Rust fmt/check(all-targets,j1), architecture/IPC guards passed. Complete local suites intentionally delegated to exact final-head CI under this task policy; no repeated profiling.
- Native: owned isolated DB/WebView, offline endpoint, 21 prompts / 101 images. UI recent picker contains20 Prompt buttons /100 image items and excludes Old Prompt21 / Old Asset101. Existing Library name search recovers both; return retains the same default project, Shot01, video stage, FL2V quality2.1.1 generator, width1280/height544/duration5. Image intent initially leaves both slots empty; explicit tail application changes only tail. Manual prompt append and another Library/back return preserve edited text and old tail selection. Shell/breadcrumb/Overview show 默认项目; ordinary My Film remains unchanged.
- Before/after counts of tasks, production batches/items, Shot generation links, snapshots and persistent Shot references all remain0. Owned application closed. Native DB migration max42 and formal allowlisted table count71 verified; no043.
- Remote CI result is reported with exact final commit in the final chat report; the prior M2-1 run is not reused as authority.
