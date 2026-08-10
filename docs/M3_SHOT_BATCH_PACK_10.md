# M3 SHOT BATCH PRODUCTION PACK 10 = CODE PASS / LIVE PENDING

Date: 2026-08-10
Development line: `0.3.0`
Baseline: `9fa9183` (`feat: add m3 shot production pipeline`)
Active production scope: Kera2 image keyframes + MiniMax H3 reference-image-to-video.

## Product contract

Pack10 closes the durable multi-Shot production flow:

`多个 Shot → 批量 Kera2 关键帧 → 人工逐 Shot 选图 → 明确点击批量视频 → MiniMax H3 严格顺序 → 人工逐 Shot 选最终视频`

Kera2 成功不会自动创建 H3 任务。图片批次完成后，系统停在人工审图；只有用户明确点击“批量生成视频”，且 Shot 已选定当前项目图片时，才会创建视频 Production Batch。

生产运行时范围继续冻结为：

- Kera2 image keyframes（当前 Kera2 workflow scope）
- MiniMax H3 reference-image-to-video（当前 H3 workflow scope）
- 不新增第三 Runtime、第二执行引擎、Cloud/Login/Sync/Updater、时间线/NLE、Audio Mixer 或 Marketplace。

## P10 delivery

| Slice | Result |
| --- | --- |
| P10-01 Shot Batch Planner | Shot 制作页增加图片/视频两阶段规划、资格检查、全选符合条件、单独取消、1–100 项限制和阻塞原因。 |
| P10-02 Batch Kera2 Keyframes | 图片批次使用普通 `ProductionBatch` / `ProductionBatchItem`，每个 Shot 一个普通 Task，继续沿用全局严格顺序。 |
| P10-03 Persistent Binding | `ShotBatchService` 在一个 SQLite transaction 中创建 batch、items 和 `shot_generation_links(task_id=NULL, production_batch_item_id=...)`。每个 item 最多一个 Shot-stage link。 |
| P10-04 Restart-safe Recovery | Runner 通过持久 item binding 恢复；`DISPATCHING` 仍按现有 `QUEUE_DISPATCH_UNCERTAIN` 规则暂停，不自动重复提交。 |
| P10-05 Keyframe Review Board | 按 Shot 展示任务候选、图片预览、人工“设为关键帧”、最近失败、任务详情和重新加入队列。不会自动选择第一张成功图。 |
| P10-06 Batch H3 Planning | 只展示已配置 H3 阶段且拥有合法当前项目关键帧的 Shot；跨项目、错误类型、缺失素材和不可用 Recipe 均阻塞。 |
| P10-07 Sequential H3 Production | H3 batch `continueOnFailure=true`，但仍复用现有 ProductionQueueService 的全局单活跃 item 和 fatal/dispatch-uncertain 暂停规则。 |
| P10-08 Failure / Retry UX | 失败作为辅助历史展示；显式重试创建新的普通 queue item/Task，旧 Task 和 link 保留。危险的 `EXECUTION_ERROR` / uncertain dispatch 不走自动 quick retry。 |
| P10-09 Final Review | 视频候选支持原生播放和人工“设为最终视频”；选择结果后不会自动提交下一阶段。 |
| P10-10 Progress Dashboard | 派生展示总镜头、待关键帧、关键帧已选、视频生成中、待视频确认、已完成、失败/需处理及 `已完成 N / Total`。 |
| P10-11 Pack09 Live Closeout | 本轮没有可控 GPU/UI 实机证据，因此不伪造 Pack09 Live PASS；保持 `CODE PASS / LIVE PENDING`。 |
| P10-12 Pack10 Live Gate | 代码、持久化、备份和构建门通过；真实三 Shot Kera2→人工选图→H3→播放→重启链仍等待可控实机环境。 |

## Data and execution invariants

- 没有创建 `ShotBatchTask`、`ShotQueue`、`ShotExecutor` 或 `ShotBatchExecutor`。
- Runner 准备 dispatch 时调用现有 `GenerationService.start_generation_with_task_hook`；Task 创建后先写入 Shot link，再允许 Comfy 执行。
- Hook 失败会将 Task 置为 `FAILED`，不发生 Comfy submission；queue item 同步记录失败。
- Queue item 冻结 Prompt、scalar、Reference assets、selected keyframe、workflow version 和 Recipe。之后编辑 Shot 只影响未来批次，界面会显示冻结配置提示。
- Shot 状态仍从配置、选中素材和最新关联普通 Task 派生；active task 优先，selected result 优先于后续失败，失败记录作为辅助信息保留。
- 删除 Shot 时，PENDING / DISPATCHING / DISPATCHED 的绑定会阻止删除；只有 terminal 历史允许删除，Task、Snapshot、Asset 不随 Shot 删除。
- 删除已归档 Production Batch 时，现有 `ON DELETE SET NULL` 保留 Shot 历史 link 和 Task 证据。

## Migration and backup gate

- `src-tauri/migrations/001_initial.sql` 至 `010_shot_production.sql` 未修改。
- `011` 不存在。Pack10 使用 `010` 已预留的 `shot_generation_links.production_batch_item_id`，没有新增 migration。
- Backup version 保持 v4；roundtrip 覆盖 Shot、Task、Asset、Production Batch、Batch Item 和 Shot link 的 ID remap，并验证 binding 指向恢复后的 Batch Item、项目隔离和重复 binding 拒绝。

## UI safety and scope

- 新增的批量规划、冻结配置、人工复核、失败处理和进度面板均使用简体中文，普通 UI 不暴露 node id、`class_type`、storage path、workflow JSON 或 raw Recipe YAML。
- Shot workspace 保持最低 `1000×700` 布局；批量表格和复核区域在容器内滚动，不把关键操作藏在不可见溢出中。
- 规划器的运行时资格检查只允许 Kera2 图片和 MiniMax H3 参考图生视频范围，不为第三 Runtime 添加旁路。

## Verification

The final automated gate is:

- `cargo fmt --all -- --check` — PASS
- `cargo check` — PASS
- `cargo test -- --test-threads=1` — PASS, 314 tests
- `pnpm test --run` — PASS, 32 test files / 98 tests
- `pnpm build` — PASS
- `git diff --check` — PASS
- `pnpm tauri build` — PASS, MSI + NSIS bundles

The honest release gate remains:

`M3 SHOT BATCH PRODUCTION PACK 10 = CODE PASS / LIVE PENDING`

No live GPU/UI result is claimed without an observable ComfyUI endpoint and controllable desktop session. After Pack10, the 0.3.0 scope is frozen for release hardening; no new production runtime or broad product subsystem is opened in this line.
