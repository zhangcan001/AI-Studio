# PX-02 — Shot Production Workspace 实施结果

## 结论

`PX-02` 已完成并推送到 `master`。

- 实现提交：`1af2e1ecc29291a63876378e42f6c0406758fa1e`
- 基线提交：`83af0103aa54ed541cc3f08e51cfbbea7a655582`
- 远端 CI：[Source-only CI #34100388137](https://github.com/zhangcan001/AI-Studio/actions/runs/34100388137)
- 远端结果：Rust source checks 与 Frontend source checks 均 `success`

## 实现范围

### 1. Shot Production Pipeline read model

新增 `src/features/shots/shotProductionState.ts`，使用现有 `ShotView`、`stageConfigs`、`referenceAssets`、`generationLinks`、候选选择字段和任务状态派生以下步骤：

`Prompt → References → Image → Image Review → Video → Video Review → Complete`

模型状态包含 `COMPLETE`、`ACTIVE`、`READY`、`BLOCKED`、`PENDING`、`FAILED`，并提供单一 `Next Action`。参考素材按照当前 Recipe / 视频输入模式判断，纯文本视频、单关键帧视频、多参考图视频分别复用现有兼容性规则；不要求参考素材的工作流不会被阻塞。仅图片阶段完成且未配置视频阶段时，视频与视频审核会被标记为跳过完成，沿用现有 `deriveShotStatus` 的图片工作流语义。

### 2. Production progress UI

新增 `src/features/shots/ShotProductionProgress.tsx`，接入 `ShotCreationWorkspace` 顶部。步骤和 `继续` 按钮均为导航-only：

- 提示词：打开生成页并聚焦提示词 Inspector。
- 参考：打开参考工作区并聚焦参考 Inspector。
- 图片 / 图片确认：切换到图片阶段生成入口。
- 视频 / 视频审核：切换到视频阶段生成入口。

进度条不直接调用生成、保存、候选确认、队列、重试或其他副作用动作。样式合并在现有 feature-owned `ShotCreationWorkspace.css` 中，使用横向滚动容器避免在窄窗口制造全页横向溢出。

### 3. Selection state extraction

新增 `src/features/shots/hooks/useShotWorkspaceSelection.ts`，集中管理：

- `selectedShotId` 与 `workspaceSelection`。
- 深链 `initialSelectedShotId` 的初始定位。
- 结构树 / 搜索 / 面包屑的选择导航。
- reload、删除镜头后的当前镜头恢复与 `onShotSelected` 通知。

创建 UI 状态未继续拆分，以避免扩大 `ShotWorkspace` 的已有状态边界。候选缩略图预览与显式候选确认的原有分离保持不变。

## 行为与边界验收

| 验收项 | 结果 |
| --- | --- |
| `SHOT_PRODUCTION_PIPELINE` | PASS |
| `PIPELINE_SOURCE=EXISTING_SHOT_STATE` | PASS |
| `NEW_PERSISTED_STATE=NO` | PASS |
| `PIPELINE_NAVIGATION_ONLY=YES` | PASS |
| 提示词 / 参考 / 图片 / 图片审核 / 视频 / 视频审核 / 完成步骤 | PASS |
| 可选参考工作流不被阻塞 | PASS |
| 单关键帧视频输入门禁 | PASS |
| 多参考图视频输入模式 | PASS |
| 失败与进行中任务状态 | PASS |
| `initialSelectedShotId` 深链 | PASS |
| workspace resume / reload selection | PASS |
| 候选预览与显式确认保持分离 | PASS |
| 生产模式与审核模式 | UNCHANGED |
| 顺序批量生产 | UNCHANGED |
| 多生产包 | UNCHANGED |
| 500 镜头树上限 / 分页约束 | UNCHANGED |
| IPC / Rust / 数据库 / migration | NO CHANGE |
| `src/services/tauriClient.ts` | NO CHANGE |

生产条只为当前 `selectedShot` 构建一个 read model，没有为整棵 500-shot 树增加逐镜头生产条渲染或新的队列状态。

## 验证记录

本地验证：

- `pnpm vitest run src/features/shots/shotProductionState.test.ts src/features/shots/ShotProductionProgress.test.tsx src/features/shots/hooks/useShotWorkspaceSelection.test.tsx src/features/shots/ShotCreationWorkspace.test.tsx src/features/shots/ShotWorkspace.test.ts`：22 tests passed
- `pnpm test`：122 test files / 585 tests passed
- `pnpm exec tsc --noEmit`：passed
- `pnpm build`：passed；仅有既有 chunk size warning
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`：passed；仅有既有 dead-code warnings
- `git diff --check`：passed
- 架构扫描确认新增 read model、进度组件和 selection hook 未引入 `tauriClient`、`taskEvents`、IPC 或持久化调用

远端验证：

- Frontend source checks：tests、TypeScript check、frontend build 全部 passed
- Rust source checks：format check、Rust check、Rust tests 全部 passed
- 仅记录 GitHub Actions 对 Node.js 20 action runtime 的既有 deprecation annotation，不影响结论

## 实际执行说明

- 本次会话未调用模型切换工具，因此没有虚构任务要求中的模型切换阶段；按当前会话默认模型完成实现与验证。
- 遵循 `ponytail` full：复用现有 `shotDomain`、`shotWorkflowCompatibility`、候选确认和 workspace 选择行为，不新增依赖、不新增后端状态机。
