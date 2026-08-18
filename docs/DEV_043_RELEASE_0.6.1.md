# DEV-043 — AI Studio 0.6.1 Release Evidence

## Release identity

- Baseline: `04af117d25ea998912ca59aa78a895e75580b931`
- Source RC: `62e406865e2998584f8cd3265b706b4e003aab0e`
- Source RC commit: `chore: prepare AI Studio 0.6.1 release candidate`
- Annotated tag object: `06166208d064200b761f20dd96584d5f51e29aeb`
- `v0.6.1` peeled SHA: `62e406865e2998584f8cd3265b706b4e003aab0e`
- Release: [AI Studio 0.6.1](https://github.com/zhangcan001/AI-Studio/releases/tag/v0.6.1)
- Published: `2026-08-18T13:01:45Z`
- Final state: published, non-draft, non-prerelease

The existing release tag SHAs were preserved:

- `v0.4.0` → `94918f6322ce690ff7b1630961abb56b8a31ed11`
- `v0.5.0` → `02e67cff50f5da1d207478071636af166048820c`
- `v0.6.0` → `e3d7181f23a9b7285a426efb20ead4db17198757`

## Frozen artifacts

The release contains exactly four assets: the three frozen artifacts and the checksum file. Local frozen values and the GitHub remote digests match byte-for-byte.

| Artifact | Bytes | SHA256 |
| --- | ---: | --- |
| `ai-studio.exe` | 41,347,072 | `E9677E52ADCF093F9A37BC3DBFF88324D482814B44E5F1F8336029F2163023C5` |
| `AI Studio_0.6.1_x64-setup.exe` | 9,206,731 | `CDF1D0591AE832ECE0C75120E386C97EEE719F05B201CF1E766DDF832E0FD035` |
| `AI Studio_0.6.1_x64_en-US.msi` | 13,565,952 | `A9BFFD7D381A378740138C3EF032EFB19C31A1F21108B4854A04EE7D8FF2B7BC` |
| `RELEASE_SHA256_0.6.1.txt` | 465 | `F746B00E05D53AB74041E3D767464B5C65CA2F5183AA19DBF0868EED6F556800` |

GitHub's asset API exposes the two installer names with `AI.Studio_...` canonicalization, but their sizes and SHA256 values match the frozen local artifacts and checksum rows.

## Regression and scope gate

- Rust ignored tests: `2 → 1`; the sole remaining ignored test is the external ComfyUI/real-database DEV-027 live test.
- Frontend todo/skipped tests: `1 → 0`; product skip/only inventory is `0`.
- Targeted DEV-040 / DEV-041 / DEV-042 Rust contracts: `26 passed`.
- Targeted frontend DEV-040 / DEV-041 / DEV-042 coverage: `21 passed`.
- Deterministic 500-shot fixture/search coverage: passed; manual review remains intact.
- Full Rust regression: `629 passed / 0 failed / 1 ignored` with one test thread.
- Full frontend regression: `71 files, 259 passed, 0 todo`.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `pnpm build`: PASS.
- `git diff --check`: PASS.

No new queue, executor, scheduler, Start All, migration, backup, or manifest architecture was added. The ponytail pass kept the implementation to the existing services and the minimum test-debt cleanup needed for this release.

## Version and install validation

- Version sources are `0.6.1` only in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Migration remains `021`.
- `BACKUP_VERSION` remains `12`.
- Production manifest version remains `1`.
- Portable fresh launch and restart: PASS; isolated database and data directories created.
- Portable official `0.6.0 → 0.6.1` upgrade: PASS; Project, Series, Episode, Scene, Shot, Reference Anchor, Prompt version, and frozen stage prompt sentinels all preserved; migration max remains `21`; restart PASS.
- NSIS 0.6.1 fresh install, fresh launch/restart, and uninstall: PASS.
- NSIS official `0.6.0 → 0.6.1` upgrade, sentinel preservation, restart, and uninstall: PASS.
- MSI static inspection: ProductName `AI Studio`, ProductVersion `0.6.1`, x64, compatible UpgradeCode, changed ProductCode.
- MSI normal install: both official 0.6.0 control and 0.6.1 returned exit `1603` with the same Windows Installer Error `1925` privilege restriction. Classification: `MSI=ENV_BLOCKED`, P2; release allowed because the control and candidate fail identically in this environment.

## ComfyUI and Krea2 live gate

- Formal runtime: `http://127.0.0.1:8188`.
- `/system_stats` and `/object_info`: HTTP 200.
- GPU: `NVIDIA GeForce RTX 5060 Ti`, CUDA runtime, approximately 17.1 GB total VRAM.
- Krea2, H3, and REF2VA capability preflight: present. An unrelated non-core H3 custom-node import warning reported missing `time_shift_slope`; it did not affect the Krea2 product gate.
- Offline endpoint `127.0.0.1:18188`: remained blocked/not used.
- Exactly one lowest-cost Krea2 GPU Product-chain image flow was executed through the application queue; no direct `/prompt`, H3, REF2VA, video, or multi-candidate flow was used.

Live Krea2 evidence:

- Project: `prj_ff54f4cd-7767-42bc-aae0-ba5f13bf11de`
- Shot: `sht_85072377-afbf-4472-adb4-9f17c0629fc4`
- Batch: `pbt_d07ec0f903194e3ba1da0225f5e97e83`
- `TASK_ID`: `tsk_c9f94749-20b5-4375-999e-938873be7546`
- `SNAPSHOT_ID`: `snp_47646252-5da8-421f-8ebc-821cafad650a`
- `ASSET_ID`: `ast_240786c1-960e-4e32-92e6-77f0f0e3c715`
- `workflowVersionId`: `wfv_2407734d-ff20-44d9-ac7c-15ab514d7193`
- `recipeId`: `rcp_0575fb13-6bfb-41cb-ba10-eba2719a793c`
- Final task status: `SUCCEEDED`.
- Batch status: `COMPLETED`.
- Asset: `image/png`, `768×1280`.

## Final gate

- P0: `0`
- P1: `0`
- P2: `1` — MSI environment block documented above.
- Parallel agents: 4 requested, all completed and closed; `ACTIVE_SUBAGENTS=0`.
- Master was pushed at the Source RC and again after publication evidence was committed.

Decision: **AI Studio 0.6.1 is published and the DEV-043 development phase is complete.**
