# DEV-038 — AI Studio 0.6.0 Installed RC Validation + Release Publication

日期：2026-08-18

## Final decision

`DEV-038 PASS — AI Studio 0.6.0 published`

The release is published at [GitHub Release v0.6.0](https://github.com/zhangcan001/AI-Studio/releases/tag/v0.6.0), with exactly four assets, non-draft and non-prerelease.

## Source and release identity

- Source RC: `e3d7181f23a9b7285a426efb20ead4db17198757`
- RC message: `chore: prepare AI Studio 0.6.0 release candidate`
- Master evidence commit before publication: `42978880d2a555e947c675aa3795de2bfeb5142a`
- `v0.6.0` annotated tag object: `71324989d99c4089d977c1cba1c00c275ce8d5f0`
- `v0.6.0^{}` peeled SHA: `e3d7181f23a9b7285a426efb20ead4db17198757`
- Frozen `v0.5.0^{}`: `02e67cff50f5da1d207478071636af166048820c`
- Frozen `v0.4.0^{}`: `94918f6322ce690ff7b1630961abb56b8a31ed11`
- Product version: `0.6.0`
- Migration: `021`
- `BACKUP_VERSION`: `12`
- Project manifest: `v1`

## Frozen artifacts

The local frozen set and the downloaded published set have identical size and SHA-256 values.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `AI Studio_0.6.0_x64_en-US.msi` | 13,213,696 | `418AD0DB7F4BA375D899E45A270F14860EA1A7D892B92A1EC17CAC58568B9028` |
| `AI Studio_0.6.0_x64-setup.exe` | 8,937,793 | `2FDBBD8DE317B6A21EBE66E408BBE6C7146E357473A86B00939E33D730F62406` |
| `ai-studio.exe` | 40,024,576 | `2EA3BBD080A524E782765001823E1030E124F88A3C234FBB3C689428DAFC7F70` |
| `RELEASE_SHA256_0.6.0.txt` | 464 | `A04A01B0167C285915627E2AFDBBFDAEC3519B6A590F30CE5F195475E5ECD9E8` |

The EXE contains the exact RC SHA and has `FileVersion=0.6.0` / `ProductVersion=0.6.0`. The published GitHub asset API digests and a second post-publication download matched all four rows above.

## Installed and data validation

- Portable fresh start: PASS. Isolated data root created the database and runtime directories, applied migrations through `021`, and restarted cleanly.
- Portable `0.5.0 → 0.6.0`: PASS. A clean official 0.5.0 fixture preserved project and shot sentinels; migration chain was `019 → 020 → 021`; restart passed.
- NSIS fresh install/restart/uninstall: PASS. Silent install exit `0`, version `0.6.0`, isolated data preserved after uninstall.
- NSIS `0.5.0 → 0.6.0` upgrade/restart/uninstall: PASS. Project and shot sentinels remained present; migration reached `021`.
- MSI static gate: PASS for x64 metadata and the v0.5/v0.6 shared UpgradeCode. MSI install was `ENV_BLOCKED` by Windows Installer Error 1925 / exit `1603`; the official v0.5 MSI control produced the same environment error, so this is not classified as a product regression.

## Runtime and live Product Flow

Runtime: `D:\ComfyUI-WorkFisher-V2`, ComfyUI `0.33.0`, Python `3.12.10`, CUDA `13.0`, NVIDIA `GeForce RTX 5060 Ti`, endpoint `http://127.0.0.1:8188`. `/system_stats` and `/object_info` passed; Krea2, H3, and REF2VA capabilities were present. The known optional MiniMax dual-clock import warning and aria2c permission warning did not block the required nodes or runtime.

The isolated offline profile gate for `http://127.0.0.1:18188` returned `COMFY_OFFLINE`; apply failed without changing the active `8188` endpoint. Focused Rust preflight and safe-apply tests passed.

The exact RC identity was used for the minimum-cost real Krea2 Product Flow. It passed through bulk shot import → Krea2 image stage → ProductionQueueService → GenerationService → ComfyHttpAdapter → asset persistence, without UI key simulation or a direct `/prompt` call:

- Project / shot: `prj_1294efa3-ec69-4a43-964c-96ea67a6b4e2` / `sht_db2d129e-0e50-4813-a771-820b14b4113c`
- Batch / task / snapshot / asset: `pbt_3281e48e3c2a42e3ac7d7b89ee74d67b` / `tsk_97a475d5-5c11-4176-9ffa-f29528a65107` / `snp_e8b3f7b1-e5bd-4553-a219-ac2e449379ef` / `ast_617d937f-2c7c-4c6a-9bf5-5be0b8b11735`
- Workflow/version: `wfl_kera2_t2i_local_v2` / `wfv_2407734d-ff20-44d9-ac7c-15ab514d7193`
- Recipe: `rcp_0575fb13-6bfb-41cb-ba10-eba2719a793c`
- Prompt: `A cinematic still of a red kite flying over a quiet winter lake, soft morning light, detailed composition`
- Seed: `757004854220816`; final task status: `SUCCEEDED`; persisted asset: PNG, `768×1280`, `1,178,067` bytes
- Task identity: `app_version=0.6.0`, `build_commit=e3d7181f23a9b7285a426efb20ead4db17198757`

## Regression and findings

No full regression was rerun because the RC validation introduced no product-code changes. DEV-037 remains the authoritative full baseline: Rust `548 passed / 0 failed / 1 ignored`, DEV-035 integration `3 passed`, DEV-036 integration `9 passed`, frontend `62 test files / 205 tests passed`, and build PASS. DEV-038 added the targeted offline gates, installer checks, portable/upgrade checks, and live RC Product Flow above.

- P0: `0`
- P1: `0`
- P2: MSI environment Error 1925, unsigned installer artifacts, existing Rust dead-code/Vite chunk warnings, and known optional Comfy warnings; none blocked publication.

## Publication verification

- Annotated tag pushed: `v0.6.0`
- Draft release verified with exactly four assets before publication.
- Published release verified as `isDraft=false`, `isPrerelease=false`, with exactly four assets.
- Published asset names, sizes, API digests, and post-publication downloaded hashes match the frozen manifest.
- Existing `v0.5.0` and `v0.4.0` tags were preserved unchanged.
- Final documentation commit: `docs: record AI Studio 0.6.0 publication`
