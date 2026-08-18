# DEV-041 — Episode Production Planner

## 交付结论

DEV-041 已完成并合入现有 AI Studio 0.6.0 生产架构。Episode 层只负责
规划、作用域解析和准备编排；实际 Shot 计划、Batch 创建、队列 admission 和
人工 Review 仍由已有 Scene / ShotBatch / Production Queue 边界负责。

本次没有新增 migration、backup version、queue、executor、generation service、
第二套 ProductionBatch、第二套 status engine，也没有调用 GPU、ComfyUI、live
generation、installer 或 release 流程。

## 基线与冻结边界

- Branch: `master`
- `DEV041_START_SHA`: `69fa3c94df18c9e413b96c7d2156aaa9708028a6`
- Product: `0.6.0`
- Migration: `021`
- `BACKUP_VERSION`: `12`
- Manifest version: `1`
- Frozen `v0.6.0` peeled SHA: `e3d7181f23a9b7285a426efb20ead4db17198757`
- Frozen `v0.5.0` SHA: `02e67cff50f5da1d207478071636af166048820c`
- Frozen `v0.4.0` SHA: `94918f6322ce690ff7b1630961abb56b8a31ed11`

## Backend

`EpisodeProductionService` 复用 `ProductionStructureService` 一次加载 Episode
树，再按 Scene ordinal 生成稳定作用域。它复用现有
`SceneProductionService::plan_scope/prepare_scope`，因此不绕过已有的
ShotBatch 创建、active binding 重查和 prepare gate。

Episode plan 输出每个 Scene 的 `DONE / PREPARED / READY / PARTIAL / BLOCKED /
EMPTY` 分类、DONE/PREPARED/ELIGIBLE/BLOCKED 数量、已有 Batch、阻塞原因和
Episode 汇总。准备请求支持：

- 默认 strict：选中 Scene 存在 BLOCKED 时在任何 mutation 前返回，保证 0 mutation。
- partial：只准备当前可生产内容，跳过 DONE、EMPTY、BLOCKED，并把结果标成
  `PARTIAL`；重复请求返回 NOOP。
- 最多 50 个 Scene；批量 Shot 作用域最多 500 个唯一 Shot，不自动拆分。
- 每个 Scene / stage 最多由已有 Scene service 创建一个新的 READY Batch。
- prepare 只创建并返回 Batch，不自动启动 Production Queue 或 GPU。

新增 Tauri commands：`episode_production_plan`、`episode_production_prepare`。

## Multi-Scene preset 与 Prompt

现有 `ShotBulkService` 增加 Scene scope 收集：去重、按项目全局 Shot ordinal
排序、校验 project membership，并在 500 Shot 上限前拒绝超限；只更新 stage
config，保留 references、selected image/video、anchors、assignment 和 ordinal。

现有 `PromptTemplateBulkService` 增加 Scene scope 包装，沿用既有 per-Shot
render 和 atomic transaction；每个 Shot 仍使用自己的 Series/Episode/Scene/Shot
context，任一模板变量或作用域错误都会在写入前失败。

## UI

新增 `EpisodeProductionPanel` 并挂载到现有 Shot Workspace，提供：

- Series / Episode / image-video stage selector。
- Scene 表格、全选可准备、全选有阻塞、筛选、逐 Scene blocker 和深链到场景。
- strict 默认开关，partial toggle，确认文案明确“不自动启动 GPU”。
- preset 批量应用、Prompt preview/apply、custom variables、context anchors。
- SUCCESS / NOOP / PARTIAL / BLOCKED 结果、Batch/item 数量、阻塞清单和生产队列导航。
- busy 状态锁定交互；没有自动启动队列，也不跳过人工 Image Review。

## No-GPU 安全 fixture

DEV-041 fixture 覆盖 6 Scene / 60 Shot：

- `shotTotal=50`, `DONE=15`, `PREPARED=10`, `ELIGIBLE=23`, `BLOCKED=2`。
- strict prepare：0 mutation。
- partial prepare：3 Scene-scoped batches / 23 items。
- 重复 prepare：0 新 Batch、0 新 item。
- Episode+Episode 与 Episode+Scene race：每个 Shot/stage 仅保留一个 active binding。
- 500 Shot / 50 Scene / 5 Episode 规划，以及 5 Scene prepare 不超过 5 Batch / 50 item。
- 所有 fixture 都不启动 Tauri、ComfyUI、queue worker、browser 或 GPU。

## 验证结果

定向验证：

- Episode service/command tests: `8 passed / 0 failed`。
- Shot bulk tests: `6 passed / 0 failed`。
- Prompt bulk tests: `6 passed / 0 failed`。
- `dev041_safety` Rust integration: `9 passed / 0 failed / 1 ignored`。
- Episode/Scene/Structure/stability frontend tests: `17 passed / 0 failed / 1 todo`。

最终回归：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check  PASS
cargo check --manifest-path src-tauri/Cargo.toml                  PASS
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
  576 passed / 0 failed / 1 ignored                               PASS
pnpm test
  68 files passed; 244 passed / 1 todo                            PASS
pnpm build                                                        PASS
git diff --check                                                  PASS
```

格式检查从仓库根目录执行时必须带 `--manifest-path src-tauri/Cargo.toml`，因为
仓库根目录没有 Cargo.toml。

## Final decision

DEV-041 满足 Episode planner、multi-scene preset/prompt、strict/partial prepare、
idempotency、race safety、500 Shot scope、manual review gate 和 no-GPU 回归要求。
本任务允许直接提交并推送 `master`；不创建新 tag、release 或 installer。
