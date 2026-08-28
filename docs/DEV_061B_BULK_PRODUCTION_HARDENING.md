# DEV-061B — Production Package Bulk Production Hardening

状态：**本地 Release Gate PASS，CI 证据待封存**

DEV-061B 把外部 Production Package 的批量创建、失败恢复和人工启动边界收敛到现有生产链：

```text
ProductionPackageService
  → H3LocalImportService
  → SourceAssetImportService
  → ProductionQueueService
  → GenerationService
  → WorkflowCompiler
  → ComfyUI
```

没有第二队列、第二执行器、Package Scheduler 或直接 `/prompt` 旁路；没有 LLM、脚本解析器或自动图片生成参与主路径。

## 结果语义

`ProductionPackageCreateBatchesResult` 和 Tauri wire DTO 现在同时表达：

- `COMPLETE`：请求项目全部创建。
- `PARTIAL`：已成功持久化的批次/映射保留，返回 `requestedCount`、`createdCount`、`remainingCount` 和 `remainingItemIds`。
- 第一 chunk 失败且尚无持久化批次时仍返回错误；后续 chunk 失败不会伪装成全量失败，也不会删除先前成功数据。
- inspection session 在 create 尝试消费，重复提交返回 `PACKAGE_SESSION_NOT_FOUND`；剩余项目必须重新 authoritative inspect。
- 每 100 个 logical root item 一个 batch，500 项最多 5 个 batch；5001 仍由 Package V1 上限阻断。

创建结果不会自动打开 Queue、创建 Task、提交 Comfy 或 Start；Queue 只由用户在既有 Queue UI 中手动启动。

## 恢复与媒体冻结

- 应用/Service 重建后，已创建 batch、Pending item 和 frozen `values_json` 从 SQLite 恢复。
- `COMFY_OFFLINE` 将活动 batch 留在可解释的暂停状态，partial resume plan 标记 `AUTO_RESUMABLE`。
- `partial_resume` 创建 `retry_of_item_id` 指向失败 leaf 的 child；重复调用同一 leaf 不创建第二 child。
- Retry 继续使用 frozen Prompt、workflow、recipe 和已导入 Project Asset；外部 package 图片替换后不会重新读取 package 原图。
- 已拥有 `prompt_id` 的 Task 在 stream disconnect 后由既有 `TaskRecoveryService` reconcile，不重复创建 Task 或 POST `/prompt`。

## Workspace 交互

Production Package Workspace 现在分别显示 COMPLETE/PARTIAL 的 requested/created/remaining/status 真值。PARTIAL 保留 `remainingItemIds`，提供“重新检查剩余项目”；重新检查以当前 package 为 authoritative truth，只预选仍为 READY 的剩余 ID。创建后不会自动打开 Queue，工作台没有 Start / Start All / Resume All / Generate 按钮；列表继续 50/page，批次摘要优先。

旧的 Project Bulk Import Dry-Run 保留为 **DEV-061A — PASS / OPTIONAL**，见 `docs/DEV_061_BULK_IMPORT_DRY_RUN.md`。

## 证据索引

| 范围 | 证据 |
| --- | --- |
| 当前红 CI 修复 | `production_run_lifecycle_keeps_batch_task_asset_lineage_without_gpu` 测试夹具恢复真实 task → output mapping → asset lineage；正式选择校验未放宽 |
| 500-item / partial truth | `src-tauri/tests/dev061b_production_package_hardening.rs` |
| restart / offline / retry / frozen media | `src-tauri/tests/dev061b_queue_recovery.rs` |
| workspace COMPLETE/PARTIAL/remaining selection | `src/features/production/ProductionPackageWorkspace.test.tsx` |
| Result wire contract | `src-tauri/src/commands/production_package.rs`、`src/types/productionPackage.ts` |

当前 Windows 开发机上的真实 SQLite 500-item text-package 测量（测试进程内 `Instant`，不是 H3 执行）：

```text
PACKAGE_INSPECT_500_MS = 5
PACKAGE_CREATE_500_MS = 879
QUEUE_RELOAD_5_BATCH_MS = 10
```

其中 `QUEUE_RELOAD_5_BATCH_MS` 包含重建 ProductionPackage/Queue 依赖后的一次 queue list 和 5 次 bounded batch detail read；测试断言重启后仍为 5 batches / 500 items。数值用于回归基线，不代表真实 H3/Comfy 执行耗时。

版本边界保持不变：Product `0.7.0`、Migration `025`、Backup `15`、Manifest `2`，Migration 026 不存在。DEV-062 是下一步发布门禁，不在本文内提前宣称 CI 通过。
