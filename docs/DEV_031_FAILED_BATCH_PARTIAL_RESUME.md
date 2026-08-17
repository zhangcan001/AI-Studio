# DEV-031 Failed Batch Partial Resume

日期：2026-08-17。范围是失败批次的局部恢复；不新增表、不新增 migration、不改变 Backup 格式、不新增 Queue/Executor。

## 1. Baseline

- 分支：`master`
- 开发前 HEAD / `origin/master`：`bd55c286a1fb9988fc004d766338dc13b8ceb8d0`
- `v0.5.0^{}`：`02e67cff50f5da1d207478071636af166048820c`
- `v0.4.0^{}`：`94918f6322ce690ff7b1630961abb56b8a31ed11`
- Migration：019；`BACKUP_VERSION`：10；版本仍为 0.5.0

## 2. Design

复用现有 `ProductionBatchItem.retry_of_item_id`、`is_safe_requeue_source`、`ProductionQueueService`、`GenerationService` 和同一个 SQLite queue/executor。重试只创建 retry child，不修改原失败 item。

## 3. Lineage

`build_retry_lineages` 从 root（`retry_of_item_id IS NULL`）遍历到当前 leaf，返回 root、leaf、`attempt_count`。缺 parent、cycle、一个 parent 多 child、不可达 item 统一 fail closed 为 `PRODUCTION_RETRY_LINEAGE_INVALID`。

## 4. Partial Resume API

- `ProductionQueueService::partial_resume_plan(project_id, batch_id)`
- `ProductionQueueService::partial_resume(project_id, batch_id, selected_leaf_item_ids)`
- Tauri：`production_queue_partial_resume_plan`、`production_queue_partial_resume`
- Plan 提供 logical/attempt totals、resolved、auto-resumable、review-required、pending、active、`can_resume` 和每条 lineage entry。

## 5. Atomic / idempotent behavior

一次 partial resume 在一个 SQLite transaction 中插入全部 retry children、复制 Shot binding、将 batch 置为 `PAUSED` 后提交；任一 source/binding 错误都会回滚。重复同一 leaf IDs 返回 `created_count = 0` 和 `already_prepared_count = N`。多轮重试从当前 leaf 创建 child，保留 workflow、recipe、原始 `values_json` 和 Shot 关系。

## 6. UI

现有 Batch Detail 增加折叠区“失败项恢复”：AUTO_RESUMABLE 默认勾选，REVIEW_REQUIRED 只读；确认时立即锁定按钮。恢复成功后复用 `startProductionQueue`；若队列忙，显示“恢复任务已准备完成，当前生产队列繁忙，可稍后启动。”

## 7. No-GPU E2E

真实 SQLite + `ProductionQueueService` 隔离数据库测试通过：6 个 logical items（2 succeeded、COMFY_TIMEOUT、EXECUTION_ERROR、cancelled、COMFY_OFFLINE），选择 2/4/5 首次创建 3 个 retry，重复请求创建 0 个且识别 3 个既有 retry；模拟 retry 成功后为 `resolved = 5`、`review_required = 1`。测试不启动 ComfyUI、不提交 prompt、不操作 UI。

## 8. 100-item boundary

focused repository regression 通过：100 个 logical roots 可以追加 physical retry（101 行、logical total 仍为 100）；101 个初始 logical items 仍被拒绝。retry attempts 不计入 100 上限。

## 9. Restart / Backup

现有 production queue restart、冻结输入/REF2VA 顺序、Shot binding、Backup v10 roundtrip regression 均在最终 Rust suite 中通过。本轮未修改 migration 或 Backup schema/格式。

## 10. Krea2 Live smoke

未形成 PASS。初始检查确认 `127.0.0.1:18188` 无 listener；本机已有 ComfyUI Desktop 启动后仍无 backend listener。直接使用已有 ComfyUI 源码启动时，当前 Python 环境先出现 `torchvision::nms` 二进制不兼容；兼容性 shim 后又缺少 `torchsde`。未安装或升级依赖，未调用 `/prompt`，未产生 Krea2 source/retry/Task/Asset，因此没有伪造 offline→resume→success 证据。

## 11. Regression

- Rust targeted service：17 passed；repository：12 passed；no-GPU E2E：1 passed
- Frontend targeted partial-resume：3 passed
- Final Rust：495 passed / 0 failed / 1 ignored
- Final frontend：53 files / 173 tests passed
- `pnpm build`：PASS
- `cargo fmt --all -- --check`、`cargo check`、`git diff --check`：PASS

## 12. Known limitations

只剩环境项：恢复已验证的 ComfyUI/Python 依赖后，需要按 DEV-031 低成本流程重新做一次 18188 offline → 8188 Krea2 success，并记录 source item、retry child、旧 error、新 Task、新 Asset 和最终 logical state。没有 H3、REF2VA 或 installer 测试。

## 13. Final decision

代码闭环、No-GPU、边界、重启/Backup regression 和前端交付通过；Krea2 Live Gate 为环境阻塞，故本轮为 **CONDITIONAL PASS**，不是完整 Live PASS。

提交：backend `7dad514`；frontend `f21189a`。四个子任务均已关闭，`ACTIVE_SUBAGENTS=0`。
