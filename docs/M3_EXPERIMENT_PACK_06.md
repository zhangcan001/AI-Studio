# M3 EXPERIMENT PACK 06 = PASS

Date: 2026-08-10
Development line: `0.3.0`
Release status: development only; no `v0.3.0` tag or GitHub Release.

Pack 06 is implemented on the normal production path:

`Experiment Planner → createProductionQueue → persisted production batch/items → startProductionQueue → ProductionQueueService → GenerationService → Task/Snapshot/Asset`

No ExperimentTask, special executor, direct `/prompt` submission, or React-side concurrent generation loop was added.

| Pack item | Result |
| --- | --- |
| Planner and variant limits | PASS — one/two dimensions, text max 8, cartesian max 24, Recipe-owned integer/Seed ranges, frozen random Seeds |
| Persistent queue execution | PASS — ordinary queue admission, strict ordinal dispatch, pause/recovery and retry policy |
| Result grid and compare | PASS — Task/Snapshot/Asset summaries, Seed, changed fields, duration, output assets, and existing 2–4 asset compare |
| Winner promotion | PASS — `getReusableDraft → Studio Draft`, provenance retained, no automatic Task or `/prompt` |
| Runtime profile persistence | PASS — Tauri settings backend, `settings.json`, legacy localStorage migration only after backend save |
| Import hardening | PASS — exact normalized credential-key detection; `tokenizer`, `tokens`, and `token_count` remain valid |
| Third runtime | `THIRD_RUNTIME_INPUT_REQUIRED` — no API workflow package was supplied |

The result grid now distinguishes the original session base (`与实验基础参数比较`) from restart-safe comparison (`与首个实验结果比较`). When the original base is unavailable after restart, the first reusable result is explicitly labeled `比较基准`; no queue name or field-change inference is used. The experiment name prefix remains UI-only.

## Verification

Pure Pack 06 tests cover dimension limits, field-owned ranges, frozen Seeds, cartesian expansion, media exclusion, snapshot diff redaction, import-key exactness, and queue policy. The final source regression reports 301 Rust tests and 30 frontend test files / 93 frontend tests passing.

The Kera2 four-item GPU Live Gate is an environment operation and must be recorded with its actual batch/item/task/asset/snapshot evidence. During this audit the existing desktop window could not expose a controllable UI state (`SetIsBorderRequired failed: 不支持此接口`) and the ComfyUI host reported only about 1.8 GiB free VRAM, so no unobservable GPU run is claimed as evidence. The implementation and automated regression are PASS; the local Live Gate remains explicitly environment-pending rather than fabricated.

Pack 07 is now implemented in the same development line. Migrations `001–008` remain unchanged; `009_prompt_library.sql` belongs to Pack 07.
