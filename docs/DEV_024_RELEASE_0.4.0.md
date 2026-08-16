# DEV-024B AI Studio 0.4.0 Publication Evidence

日期：2026-08-16 23:18 Asia/Shanghai  
仓库：`zhangcan001/AI-Studio`

## Publication identity

- Source RC SHA：`94918f6322ce690ff7b1630961abb56b8a31ed11`
- Evidence commit before publication：`722b3b84935cbd8873654e373216b8e8172886f5`
- Tag object：`d60ab787e1f00c918f1f3ea4538131a80981ba6c`
- Tag peeled commit：`94918f6322ce690ff7b1630961abb56b8a31ed11`
- Source RC / tag match：`PASS`
- GitHub Release：`https://github.com/zhangcan001/AI-Studio/releases/tag/v0.4.0`
- Title：`AI Studio 0.4.0`
- Draft：`false`
- Prerelease：`false`

## H3 clean restart validation

- ComfyUI was closed and restarted cleanly; `/system_stats` and `/object_info` returned HTTP 200.
- Environment: ComfyUI `0.33.0`, Python `3.12.10`, PyTorch `2.9.0+cu130`, RTX 5060 Ti, single GPU and serial execution.
- Existing Krea2 asset reused: `ast_f93c35c1-fe39-4147-953d-c4164ab41a8d`; Krea2 was not rerun.
- Workflow: `wfl_minimax_h3_fl2va`; version `wfv_e5d5098a-5c7e-40ce-a15e-7b10c53b135a`; recipe `rcp_84e7adbd-c80c-40fc-9746-b5da500ce2f4`.
- Frozen parameters: `H3_FAST` / `FL2VA` / `1s` / `864×480` / seed `42030`.
- Task：`tsk_0d0819a6-8182-452c-b99f-88d64d48eb78`；prompt：`59e2ffe6-abfa-4429-ba8c-76f276b27211`；Snapshot：`snp_76037110-8273-441e-9457-4ed2e9b395e8`。
- Generation execution：`gen_58464bcee3ec0bdd4b7c7861b9968fab907df67b22ce19cdc97a65e192683b7b`。
- Compiled workflow SHA：`07a095b17ef984c02c07242d73843cbe7b9ce8a2d73ddd2feaeed5129e3af221`。
- Video Asset：`ast_017c5e5b-127e-4e7b-9b43-bcc699539a47`；`146262` bytes；SHA-256 `3f2bda5246d4a9df3df5a480d6c664f36624f448e127c52bb5b642341c53f0c5`。
- Playback：`ffprobe` confirmed MP4, H.264 video + AAC audio, `864×480`, `1.625s`.
- Result：clean attempt `Task SUCCEEDED` / Video Asset exists / playback valid；`H3 clean smoke PASS`。
- The earlier two OOM attempts remain in the same Run as historical evidence; the Run remains `PARTIAL_FAILED` because those failures were intentionally preserved. OOM conclusion: `TRANSIENT ENVIRONMENT ISSUE`.

## Final regression and frozen state

- Rust fmt：PASS
- Rust check：PASS
- Rust tests：`464 passed / 0 failed / 0 ignored`
- Frontend：`46 files / 152 tests passed`
- Frontend build：PASS
- `git diff --check`：PASS
- Migration：`018`; migration `019` absent
- Backup：`BACKUP_VERSION = 9`
- Runtime Package bytes：unchanged from v0.3.0
- v0.3.0 tag/release/assets：untouched

## Published assets

GitHub's standard `gh release upload` naming normalizes spaces to periods; this matches the existing v0.3.0 release convention. Every remote digest and byte count was independently compared with the local file.

| local file | GitHub asset name | bytes | SHA-256 / GitHub digest | match |
| --- | --- | ---: | --- | --- |
| `src-tauri/target/release/ai-studio.exe` | `ai-studio.exe` | 36,426,752 | `2A2642C657B96396852FED03375F7F174F78680A5D6F8AB96521D3EA13373E0B` | PASS |
| `src-tauri/target/release/bundle/nsis/AI Studio_0.4.0_x64-setup.exe` | `AI.Studio_0.4.0_x64-setup.exe` | 8,283,719 | `5587B79AAB0F71D8C8121ABDAC16CF063D256CB175249485A5B0C95A42702067` | PASS |
| `src-tauri/target/release/bundle/msi/AI Studio_0.4.0_x64_en-US.msi` | `AI.Studio_0.4.0_x64_en-US.msi` | 12,185,600 | `AB0B7EE760C310EA6E507B8A8460F44F62E103F5EAD23BF981EE4736A5CCADCA` | PASS |
| `docs/RELEASE_SHA256_0.4.0.txt` | `RELEASE_SHA256_0.4.0.txt` | 1,365 | `21AA3FFF9688F5463E29F87D0020301A7B67F7D7537968B9C99DD0F281BCE618` | PASS |

## Final result

`AI STUDIO v0.4.0 PUBLISHED PASS`

The post-publication documentation commit must not move `v0.4.0`; the tag remains peeled to the Source RC above.
