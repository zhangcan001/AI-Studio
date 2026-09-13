# DEV-112 — State & Feedback Audit

**审计日期：** 2026-09-13
**基线：** `eef86fc1935c4c07959010de109c2f477720a348`
**审计结论：** P0 = 0，P1 = 0；本主题可关闭。

## 1. 必查页面

| 页面 / 表面 | 状态与动作检查 | 反馈检查 | 结论 |
| --- | --- | --- | --- |
| Project Command Center | 日常桶使用“待启动 / 运行中 / 待审核 / 已完成”；推荐动作不再把 READY 写成继续创作 | 推荐动作说明明确要求到队列点击“开始生产” | PASS |
| Shot Workspace | Shot 流程使用“待启动”“运行中”“待审核”“已选择”；导航动作使用“打开处理” | 准备/生产/审核阶段区分，候选选择不隐式批准或发布 | PASS |
| Preparation | 可准备项先通过 preflight；创建结果是“待启动” | 明确写出没有提交生成，且现有测试确认不调用 `startProductionQueue` | PASS |
| Production Queue | 项目使用“待执行”，已提交项目使用“运行中”，按钮是“开始生产/继续生产” | 创建、启动、跳过、重试都有结果反馈；重试保留原失败记录 | PASS |
| Production Monitor | 批次 READY 是“待启动”，运行是“运行中”，失败是“失败，需要处理” | 失败批次隐藏完全成功专用 CTA，但保留已完成输出查看 | PASS |
| Review Inbox | 未审使用“待审核”，选择入口使用“已选择结果” | 选择素材与发布没有合并；业务信息优先，详情折叠 ID | PASS |
| Rework | “编辑返工准备”“创建返工批次” | 返工反馈明确“状态为待启动、尚未开始真实生产”，且不自动 Start | PASS |
| Asset Library | 素材名、原始名和分类是第一视觉 | 素材来源分类不伪装成已选择/已发布；ID 作为辅助详情 | PASS |

## 2. 关键回归矩阵

| 回归场景 | 验证位置 |
| --- | --- |
| READY != RUNNING | `src/i18n/localization.test.ts`、`src/features/production/CreationDashboard.test.tsx` |
| Prepare → READY，不执行 | `src/features/shots/SceneProductionPreparation.test.tsx` |
| Start → running feedback | `src/features/production/CreationDashboard.test.tsx`、`src/features/shots/ShotWorkspace.production.test.tsx` |
| Rework → READY，等待手动 Start | `src/features/shots/ShotBatchReviewBoard.test.tsx`、`src/features/studio/ProductionBatchReviewWorkspace.tsx` |
| Review select != publish | `src/features/shots/ShotBatchReviewBoard.test.tsx`、`src/features/production/ReviewCompareWorkspace.test.tsx` |
| Failure feedback has no success-only CTA | `src/features/production/ProductionMonitor.test.tsx` |
| Command Center / Shot / Queue / Monitor 口径一致 | `src/features/projects/ProjectCommandCenter.tsx`、`src/features/shots/shotDomain.ts`、`src/features/production/ProductionQueueDrawer.tsx`、`src/features/production/ProductionMonitor.test.tsx` |

## 3. 处理过的主要问题

- 将生产批次和队列中的 READY 统一为“待启动”，RUNNING 统一为“运行中”。
- 将准备、返工、创建批次与真实生产 Start 的动作词拆开。
- 将审核入口从“最终结果/未审”等容易误读的词改为“已选择结果/待审核”。
- 将失败项目的状态和错误反馈改为“失败，需要处理”，并阻止失败批次显示完全成功的后续动作。
- 在队列项目中先显示业务名称，再显示 ID、任务和执行细节。

## 4. 保留的边界

- 未修改数据库枚举、SQLite migration、Rust、IPC command shape 或 repository port。
- 没有新增 queue、executor、task、review 或 asset domain model。
- Production Queue 仍是唯一生产执行 authority；Studio Store 仍是前端状态 authority。
- 外部 Production Package 的协议检查值 `READY/WARNING/BLOCKED` 保留在兼容性详情中；它不是队列批次 READY。
- 现有 direct start/retry 路径保持行为，仅统一已审计入口的可见语义；没有声称新增统一执行路径。

## 5. 验证

Focused tests passed during implementation. Final local gates passed: `pnpm test --run` (150 files / 820 tests), `pnpm exec tsc --noEmit`, and `pnpm build`。Exact-head Source-only CI is recorded after push in `docs/DEV_112_CLOSEOUT.md` and the task result。
