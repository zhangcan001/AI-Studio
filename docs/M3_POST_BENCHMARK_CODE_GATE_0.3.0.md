# AI Studio 0.3.0 Post-Benchmark Final Code Gate

Date: 2026-08-14
Source baseline: `db5faa4ea47ab8efcec767bcbcec5a349f57dfa3`

## Final code status

`AI STUDIO 0.3.0 = CODE READY / LIVE VALIDATION DEFERRED`

本轮只收口 Benchmark/H3 workflow hardening 的自动化 Code Gate。没有新增
Runtime、Task 模型、Queue、Executor、Cloud/Login/Sync/Updater，也没有执行
GPU、Computer Use、Desktop automation 或真实发布。

## Migration gate — PASS

- Fresh temporary SQLite：`001_initial.sql` 到 `014_workflow_benchmark.sql` 全部应用。
- Existing upgrade：保留旧库数据后执行 `011`、`012`、`013`、`014`，确认 `012→013→014` 可升级。
- `production_item_reviews`、Workflow archive/package metadata、Benchmark 表均可用。
- SQLite foreign keys 保持开启，旧 Project/Task/Asset/Queue/Shot 数据保持可读。
- `001–014` 为冻结历史迁移；没有新增 `015`。

## Backup gate — PASS

- `BACKUP_VERSION = 7`。
- v7 round-trip 覆盖 Project、Task、Asset、Preset/Prompt、Production Queue、
  Asset Video Prompt、Review、Workflow refs、Benchmark experiment/candidates 和
  生产批次关联；恢复后验证 Asset bytes、Prompt、Review、Batch/Item、Candidate、
  winner 与 Asset/Task/Batch ID remap。
- 固定 v1、v2、v3、v4、v5、v6 fixture 均可 inspect/restore；当前 v7 round-trip 通过。
- 路径穿越、Zip Slip、非法 Prompt/Asset Video Prompt、重复或越界组织数据等恶意
  输入均被拒绝。

## Workflow Benchmark gate — PASS

- Benchmark candidate 创建后冻结 Workflow Version、Recipe、输入值和 Asset IDs。
- 只通过 `ProductionBatch → ProductionBatchItem → ProductionQueueService →
  GenerationService → Task → Snapshot → Asset` 进入生产；没有直接调用 ComfyUI
  `/prompt` 的第二路径。
- 状态覆盖 DRAFT、QUEUED、RUNNING、COMPLETED、PARTIAL、CANCELLED、
  FAILED_TO_QUEUE；绑定 ProductionBatch 后禁止重复排队。
- Queue link failure 会记录 `FAILED_TO_QUEUE` compensation；Workflow deletion
  inspection 会统计 Benchmark candidate 引用，保护历史真相。

## H3 and Krea2 gate — PASS (automated only)

- H3 FL2VA：T2V、I2V、First/Last；REF2VA：image-only、audio-only、
  image+audio、video+image 的 mode contracts 和 ordered media values 通过。
- GenerationService compatibility gate 对 MissingNodes/IncompatibleInputValues
  在提交前失败，并保留 node/class/input/received structured diagnostics；没有静默
  fallback。
- `ReferenceManifest` 严格校验缺失、错序和重复 Asset IDs；错误码为
  `REFERENCE_MAPPING_INCOMPLETE`，raw detail 包含 `inputKey`、
  `expectedAssetIds`、`actualAssetIds`。
- Krea2 仍只走同一 GenerationService/Task/Snapshot/Asset 链；现有 Krea2
  compiler、queue freeze 和 image regression 通过。
- H3 真实 GPU 生成、原生播放和重启期间的 Live Gate 本轮未执行。

## Automated regression — PASS

```text
cargo fmt --all -- --check       PASS
cargo check                      PASS
cargo test -- --test-threads=1  415 passed / 0 failed
pnpm test                        46 files / 152 tests passed / 0 failed
pnpm build                       PASS
git diff --check                 PASS
```

`pnpm tauri build` 本轮未执行。`docs/RELEASE_SHA256_0.3.0.txt` 中的三个
本地候选产物属于此前源码构建，已标记为 `STALE AFTER POST-GATE SOURCE CHANGES`；
没有上传、打 tag 或创建 GitHub Release。

## Release boundary

- `v0.3.0` tag：ABSENT
- GitHub Release `0.3.0`：ABSENT
- Artifact upload：NOT RUN
- GPU Live Validation：`DEFERRED BY PRODUCT OWNER`

## Changed files in this gate

- `src-tauri/src/infrastructure/database/pool.rs` — 001–014 upgrade regression。
- `src-tauri/src/application/project_backup_service.rs` — v5/v6 compatibility and
  v7 Benchmark remap regression。
- `src-tauri/src/application/generation_service.rs` — ReferenceManifest wrong-order/
  duplicate structured diagnostics regression。
- 0.3.0 scope, final gate, release notes, README and SHA status documentation synced。
