# AI Studio 0.3.0 Scope Freeze

Date: 2026-08-10
Development baseline: `6ae12dc5d92fcb6735b9baf771864d71ad4753c2`
Release status: development-only release candidate; no `v0.3.0` tag, GitHub Release, or binary upload.

## Frozen product scope

0.3.0 的产品范围冻结为以下既有能力：

- Kera2 图片创作与 Shot 关键帧生产。
- MiniMax H3 参考图生视频与 Shot 视频生产。
- Prompt Library / Prompt Versions。
- Experiment 变体准备、比较和显式提交。
- Production Queue / Dashboard。
- Asset Library 与项目级收藏、标签组织。
- Project Templates。
- Project Backup v4。
- Shot 制作、人工候选确认与 Shot Batch Production。

本轮不增加 Pack11 或新的产品能力；只允许修复缺陷、完成 release hardening、自动化回归、安装包构建、文档和可复核 Live Gate。

## Frozen production runtime contract

普通 Studio、Shot 和 Batch UI 只接受以下两个精确 `workflow_id`：

| Runtime | Exact workflow ID | Production stage |
| --- | --- | --- |
| Kera2 image | `wfl_kera2_t2i_local_v2` | `image` / keyframe |
| MiniMax H3 video | `wfl_minimax_h3_reference_video` | `video` / reference-image-to-video |

运行时范围策略位于：

- Rust Planner / application gate：`src-tauri/src/application/product_runtime_scope.rs`
- Creation Launcher / Studio / Shot UI gate：`src/features/runtime/productRuntimeScope.ts`

两个边界 helper 都按精确 ID allowlist 判断，不能从显示名称、分类、模式或模糊字符串推断产品运行时。`WorkflowWorkspace` 和 Workflow onboarding 仍保留通用技术工作区能力，但不把新导入的通用工作流带入普通生产 UI。

本轮新增的回归明确覆盖：

- `wfl_other` + `Kera2 Test Fake` 不得进入生产范围。
- `wfl_fake` + `MiniMax H3 Reference Video Clone` 不得进入生产范围。
- 两个冻结 ID 在错误 Shot 阶段也不得通过。

## Frozen data and compatibility contract

- `src-tauri/migrations/001_initial.sql` 至 `010_shot_production.sql` immutable。
- 不创建 migration `011`。
- Backup format 保持 v4；恢复入口继续接受 v1、v2、v3、v4，并执行项目边界、素材校验和 ID remap。
- Shot Batch 继续复用既有 `ProductionBatch` / `ProductionBatchItem`、`GenerationService`、Task、Snapshot 和 Asset 链，不创建第二执行器或第二 Task 模型。

## Explicit non-goals

本冻结线不包含第三 Runtime、第二执行引擎、Cloud/Login/Sync/Updater、时间线/NLE、Audio Mixer、Marketplace、移动端或新的外部发布渠道。

任何超出上述范围的需求都必须进入下一版本评审，不在 0.3.0 release hardening 中顺手实现。
