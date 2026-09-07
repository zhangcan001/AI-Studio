# PX-04 Workflow Center & Production Profiles 验收结果

日期：2026-09-08  
实现提交：`566c13f`  
CI：Source-only CI `34151925014`（Rust 与 frontend 均 success）

## 结论

```text
PX-04=PASS
WORKFLOW_CENTER=PASS
PRODUCTION_PROFILE_PROJECTION=PASS
EXACT_IDENTITY=PASS
RUST_IPC_DATABASE_CHANGE=NO
```

已在现有 `WorkflowWorkspace` 上增加唯一的工作流中心总览。生产档案是由现有项目绑定、工作流目录、工作区健康事实和运行参数档案实时派生的前端投影，没有新增持久化 profile 表或第二状态源。

## 已交付

- 工作流中心总览：可生产工作流、需处理工作流、当前项目已使用、运行参数档案四项指标。
- 当前项目上下文：图片默认、视频默认和七类视频模式路径。
- 业务优先展示：用途、状态、工作流来源、参数档案数量；`WorkflowVersion` 与 `Recipe` 仅放在折叠的技术详情中。
- `更改工作流` 复用现有 Project Workspace / Project Workflow Settings。
- `管理参数` 复用现有 runtime profile 面板入口。
- 失效绑定、兼容回退、默认回退和部分读取失败均保持可见，不静默改写项目配置。
- 所有工作流和运行参数匹配均使用精确的 `workflowVersionId + recipeId`，没有按名称猜测。
- 未新增 Rust command、IPC、队列、executor、task model、生命周期语义或数据库迁移。

## 变更范围

```text
src/app/App.tsx
src/app/App.css
src/features/workflows/WorkflowWorkspace.tsx
src/features/workflows/WorkflowCenterOverview.tsx
src/features/workflows/WorkflowProductionProfiles.tsx
src/features/workflows/workflowCenterModel.ts
src/features/workflows/WorkflowProductionProfiles.test.tsx
src/features/workflows/workflowCenterModel.test.ts
```

## 验证结果

本地验证：

- `pnpm test`：125 test files，594 tests passed。
- `pnpm run build`：通过；仅保留既有 chunk size warning。
- `pnpm exec tsc --noEmit`：通过。
- `node scripts/dev088-architecture-guard.mjs`：通过，272 个前端 command parity 检查通过。
- `cargo test --manifest-path src-tauri/Cargo.toml`：796 passed，1 ignored，0 failed。
- `git diff --check`：通过。

远端 Source-only CI：

- Frontend source checks：success（tests、TypeScript、build）。
- Rust source checks：success（format、check、all-target tests）。
- CI 运行链接：[34151925014](https://github.com/zhangcan001/AI-Studio/actions/runs/34151925014)

## 约束确认

```text
AI Studio code changed: YES（仅 PX-04 前端实现）
AI Studio Git changed: YES（实现提交 + 本文档提交）
Rust/IPC/database changed: NO
DEV-089 implemented: NO
```

