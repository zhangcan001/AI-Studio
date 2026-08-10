# M3 Experiment Pack 06

Date: 2026-08-10
Development line: `0.3.0`
Release status: development only; no `v0.3.0` tag or GitHub Release created.

## Scope and implementation status

Pack 06 adds experiment planning on top of the existing Studio Draft and persistent Production Queue. It does not create a parallel experiment task model or a special ComfyUI submission path.

| Pack item | Implementation | Status |
| --- | --- | --- |
| P06-01–05 Experiment mode, generic variants, prompt/seed variants, planner | Single and two-dimension plans; text max 8, cartesian max 24, integer/Seed ranges owned by the Recipe; random Seeds freeze to explicit values before queue submission | CODE PASS |
| P06-06 Persistent queue execution | Each frozen plan item becomes a normal persisted production queue item; existing global GPU admission, strict sequence, partial-failure and safe-requeue behavior remain in force | CODE PASS |
| P06-07 Result grid | Queue item results load through normal Task detail, Snapshot-backed reusable Draft, and Asset records; status, changed fields, Seed, duration and output assets are summarized without paths | CODE PASS |
| P06-08 Result compare | Existing Asset Compare is reused for 2–4 image/video assets, including the existing video playback controls | CODE PASS |
| P06-09 Promote winner | “作为下一轮起点” loads the reusable Draft into Studio with experiment batch/task provenance and never auto-submits a new task | CODE PASS |
| P06-10 Profile persistence cleanup | React → Tauri command → SettingsService → JsonSettingsStore; AppSettings remains schema-compatible through serde defaults; legacy localStorage is removed only after backend save succeeds | CODE PASS |
| P06-11 Import hardening | UI-format JSON is rejected with explicit Chinese guidance; only exact normalized credential keys block import, so `tokenizer`, `tokens`, and `token_count` remain valid | CODE PASS |
| P06-12 Third runtime | Existing onboarding path remains ready for an API workflow package; the local environment still has no such package | `THIRD_RUNTIME_INPUT_REQUIRED` |

## Experiment contract

The base Draft is copied into every queue item, including bound media assets. Only Recipe textarea, integer, and Seed fields can be experiment dimensions. No media combinations are generated. A plan supports at most two dimensions, uses field-owned integer/Seed ranges, freezes random Seeds to explicit decimal values, and refuses more than 24 cartesian items. Video plans receive a generic long-running-output warning without a model-specific branch.

The queue name is `实验 · <Workflow Name> · <local datetime>`. Queue creation is persistent before execution starts. Each item therefore follows the ordinary GenerationService lifecycle and produces the ordinary Task, Snapshot, Asset, and History evidence. Result comparison is deliberately asset-based, and promoting a winner only loads the existing reusable input Draft.

## Verification

The Pack 06 pure contracts cover dimension limits, Recipe-owned ranges, frozen random Seeds, cartesian expansion, media exclusion, item removal, redacted snapshot differences, import-key exactness, and backend profile CRUD. The current Rust suite reports 295 passing tests; the TypeScript build and focused Pack 05/06 tests pass.

The live Kera2 four-item experiment gate remains an environment operation: it requires submitting four local GPU generations and checking their assets, comparison, promotion, and one manual follow-up generation. The H3 safety path remains regression-only. The third-runtime gate remains blocked only by the missing API workflow input.

Pack 07 source work may begin after Pack 06 code verification; migration `009_prompt_library.sql` is intentionally not added in this Pack.
