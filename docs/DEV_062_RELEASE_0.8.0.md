# DEV-062 — AI Studio 0.8.0 Release Gate

状态：**BLOCKED — Source RC 尚未冻结，GitHub Release 尚未发布**

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
| Real ComfyUI system_stats/object_info | BLOCKED — endpoint refused connection |
| MiniMax H3 I2V workflow / recipe discovery | BLOCKED — live Comfy preflight unavailable |
| Live package inspect/create/manual Start/result asset | BLOCKED — live Comfy preflight unavailable |
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
Comfy /system_stats: BLOCKED — connection refused
Comfy /object_info: BLOCKED — not reached after fail-closed preflight
```

The smoke wrapper is `scripts/dev062_production_package_smoke.ps1`. It fails closed when the configured ComfyUI endpoint is unavailable and does not claim a Production Package live pass. Compatibility audit also remains open for a complete 0.7→0.8 upgrade fixture and non-empty Backup 12–14 evidence; existing targeted tests passed but are not promoted to full release PASS without that evidence.

### Live ComfyUI evidence

真实 smoke 必须通过当前 catalog、workflow registry、built-in runtime package 或数据库 current version 发现正式 H3 I2V capability；不硬编码 workflow 或 recipe ID。Fake Comfy 不能替代最终 live gate。

```text
Endpoint:
Comfy version:
LIVE_WORKFLOW_VERSION_ID:
LIVE_RECIPE_ID:
Inspection ID:
LIVE_BATCH_ID:
LIVE_BATCH_ITEM_ID:
LIVE_TASK_ID:
LIVE_PROMPT_ID:
LIVE_VIDEO_ASSET_ID:
```

Live smoke 使用临时 Project 和临时合法 source image，仅执行一个 5 秒级 I2V item。必须证明 Production Package → existing Queue → real H3 → ComfyUI → imported video Asset 完整贯通，并在完成后只清理该临时数据。

### Source RC

```text
SOURCE_RC_SHA:
Source-only CI run:
CI conclusion:
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
