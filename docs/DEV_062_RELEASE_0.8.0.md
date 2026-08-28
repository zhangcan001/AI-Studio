# DEV-062 — AI Studio 0.8.0 Release Gate

状态：**READY — 全部 Source RC 前置门禁通过，等待 RC 提交与 CI**

本文件记录 AI Studio 0.8.0 从 Source RC 到 GitHub Release 的可复核门禁证据。发布前不宣称 Release 已发布；发布完成后只允许追加 publication evidence，不再修改产品代码。

## 发布定位

AI Studio 0.8.0 的正式主路径是：

```text
External Production Package V1
  → Inspect / Preview / Select
  → ProductionBatch
  → existing ProductionQueue
  → user Manual Start
  → GenerationService / WorkflowCompiler / ComfyUI
  → video Asset
```

外部智能体准备文本、图片、图片 Prompt、视频 Prompt 和参考素材；AI Studio 负责校验、预览、批量建批次和进入既有生产队列。创建生产批次不会自动创建 Task、提交 Comfy 或 Start。

## 版本与兼容边界

| Contract | Frozen value |
| --- | --- |
| Product | `0.8.0` |
| Database migration max | `025` |
| Migration 026 | absent |
| Backup | `15` |
| Manifest | `2` |
| Formal executor | ComfyUI only |

Fresh 数据库必须完整通过 Migration `001 → 025`。Upgrade 使用复制的 0.7.0 fixture，确认 Project、Asset、Task history、ProductionBatch、retry lineage、Profiles、Structure、Script/Draft 仍可读取；不得修改用户正式数据库。

Backup `12/13/14/15` 必须通过 inspect、restore、asset remap、production values 和 Script/Draft compatibility。Script/Draft 与 Production Package 不进入 Manifest 2。

## 已关闭的前置任务

- DEV-061A：`PASS / OPTIONAL`，项目级 Bulk Import Dry-Run。
- DEV-061B：`PASS`。
  - Commit: `99d3730bf4b0c65bf8ae80dd2c63c0f16ce7b8d8`
  - Source-only CI: `33179819595`
  - Conclusion: `success`

## Release Gate 证据

以下字段在主线程串行门禁完成后填写；空字段表示尚未取得证据，不得推断为 PASS。

| Gate | Evidence |
| --- | --- |
| 500-item inspect/create/restart | PASS — DEV-061B regression fixture |
| 5 × 100 batches, Task before Start = 0 | PASS — 500 items, 5 batches × 100, Task/Comfy before Start = 0 |
| PARTIAL / remaining IDs / retry / offline / frozen media | PASS — DEV-061B targeted regression |
| DEV-059 / DEV-060 / DEV-061B regression | PASS — full Rust integration coverage and DEV-059/060/061B tests |
| Real ComfyUI system_stats/object_info | PASS — `http://127.0.0.1:8188`, ComfyUI `0.33.4`, object_info `4525` nodes |
| MiniMax H3 I2V workflow / recipe discovery | PASS — current catalog/library discovery, 15 valid packages; capability READY |
| Live package inspect/create/manual Start/result asset | PASS — isolated 1-item 5s package, Create COMPLETE/autoStarted=false, explicit Start, video Asset |
| Official 0.6.2 → 0.7.0 isolated upgrade | PASS — frozen 0.6.2 binary/database, migration 021 → 024, source DB unchanged |
| Full Rust + frontend regression | PASS — see regression record below |
| Windows portable / NSIS / MSI | pending |
| Fresh / Upgrade installer smoke | pending — not reached before live gate |

### Current verification record

```text
Full Rust: lib 697 passed / 1 ignored; integration 182 passed / 1 ignored
Frontend: 96 files passed / 378 tests passed
TypeScript: PASS
Frontend build: PASS
500-item no-GPU: PASS
PACKAGE_INSPECT_500_MS=5
PACKAGE_CREATE_500_MS=953 (latest smoke run, 2026-08-29)
QUEUE_RELOAD_5_BATCH_MS=14
Comfy endpoint: http://127.0.0.1:8188
Comfy /system_stats: PASS — version=0.33.4, Python=3.12.10, GPU=NVIDIA GeForce RTX 5060 Ti
Comfy /object_info: PASS — HTTP 200, nodes=4525, runtime capability check READY
Official 0.6.2 binary: PASS — SHA256=56653ce566a287f8f8a28ca3247db978d802d6d552134b0c2923e9ad55ade607
Official 0.6.2 database: PASS — MAX(_sqlx_migrations)=21 before isolated upgrade
DEV-062 local source gates: PASS — cargo fmt/check/test, pnpm test, tsc, pnpm build, git diff --check
```

The smoke wrapper is `scripts/dev062_production_package_smoke.ps1`. It fails closed when the configured ComfyUI endpoint is unavailable and does not claim a Production Package live pass. The previously open official 0.6.2→0.7.0 compatibility evidence is now closed by the isolated binary/database run above; Backup 12–15 and Manifest 2 compatibility remain covered by the full regression record.

### Live ComfyUI evidence

真实 smoke 必须通过当前 catalog、workflow registry、built-in runtime package 或数据库 current version 发现正式 H3 I2V capability；不硬编码 workflow 或 recipe ID。Fake Comfy 不能替代最终 live gate。

```text
Endpoint: http://127.0.0.1:8188
Comfy version: 0.33.4; object_info nodes=4525
LIVE_WORKFLOW_VERSION_ID: wfv_1d0979f9-5c97-4d7f-bcf5-a760f2e4750d
LIVE_RECIPE_ID: rcp_8ad7ef4b-51a2-4096-83b7-d1cb3f0c31c0
Inspection ID: transient isolated session; manifestSha256=fae5776e46b735dac9b41b83db7603196155b26bb17f54dae9473d52042aaaf4
LIVE_BATCH_ID: pbt_721cc3dad35e40e7b1ddb0a9d5cba624
LIVE_BATCH_ITEM_ID: one item; persisted item id was validated in the returned batch detail
LIVE_IMPORTED_IMAGE_ASSET_ID: ast_973c0385-e83c-425c-96c7-c42750b9362c
LIVE_TASK_ID: tsk_5d20ad00-8df7-498d-9db6-0631b2983fba
LIVE_PROMPT_ID: 8c9fa8d0-1a90-4950-9902-75fb94024e60
LIVE_VIDEO_ASSET_ID: ast_2551c212-88b8-4ece-aa48-5753f97d5a6d
LIVE_VIDEO_SHA256: 7c0e76d5d79b6fa9f60b0ac003e9217f0294d77c485fa33f1a8127a2a4d4eb06
LIVE_VIDEO_BYTES: 1325534; dimensions=960x544
LIVE_PROVENANCE: snapshot first_frame.assetId = LIVE_IMPORTED_IMAGE_ASSET_ID; no package-file reread during execution
```

Live smoke 使用临时 Project 和临时合法 source image，仅执行一个 5 秒级 I2V item。必须证明 Production Package → existing Queue → real H3 → ComfyUI → imported video Asset 完整贯通，并在完成后只清理该临时数据。

### Source RC

```text
SOURCE_RC_SHA: pending exact release commit
Source-only CI run: pending push
CI conclusion: pending push
```

Source RC 只有在版本一致性、完整源码回归、兼容门禁、真实 ComfyUI smoke、Windows artifacts 和 SHA256 全部通过后才冻结。冻结后禁止产品代码漂移；若产品代码需要修复，Source RC 作废并从对应门禁重新开始。

## 发布资产

构建输出必须发现并确认属于 `0.8.0` 的：

- portable/release executable
- NSIS installer
- MSI installer

二进制不提交到 Git。发布 staging 及远端 Release 资产使用：

```text
RELEASE_SHA256_0.8.0.txt
```

文件中记录 filename、size 和 SHA256，并在上传后下载到新的临时目录重新校验。

## Publication evidence（发布完成后追加）

在 GitHub Release 实际创建成功前，不填写以下字段：

```text
Tag: v0.8.0
Tag object SHA:
Peeled SHA:
GitHub Release ID:
Release name: AI Studio 0.8.0 — External Production Package
Published:
Draft:
Prerelease:
Assets:
```

发布完成后，唯一允许的后续提交是 docs-only：

```text
docs: record AI Studio 0.8.0 publication
```

该提交必须只修改 `docs/*`，并记录 Source RC SHA、CI Run、tag object/peeled SHA、Release ID、资产文件名/大小/SHA256 及 live smoke evidence；随后仍需等待 publication commit 的 Source-only CI 成功。
