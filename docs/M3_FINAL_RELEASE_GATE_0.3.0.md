# AI Studio 0.3.0 Simplified Final Live Gate

Date: 2026-08-10
Scope: Product Scope Realignment；禁止回到旧 Shot 主路径或新增其他产品能力。

## Current status

`AI STUDIO 0.3.0 = RELEASE CANDIDATE / SIMPLIFIED LIVE GATE PENDING`

当前代码目标是两个独立产品：批量图片（Krea2）和批量视频（MiniMax H3）。旧 Shot 数据与后端保持兼容，但不作为普通导航入口。未获得真实桌面操作证据前，不把 GPU 生成、队列重启或媒体播放写成 PASS；不创建 tag、GitHub Release 或上传二进制。

## Code and compatibility gate

| Gate | Current boundary |
| --- | --- |
| Exact runtime scope | Krea2 与 H3 只按精确 workflow ID 进入各自产品入口。 |
| Asset video prompt | 图片 Asset 的提示词持久化、项目隔离、非空和 64 KiB 校验已接入。 |
| Backup compatibility | Backup v5 保存/恢复 Asset 视频提示词，并继续接受 v1–v5。 |
| Queue contract | 两个入口都创建持久化 Production Queue；输入、参数和随机 Seed 在创建时冻结；严格串行。 |
| Ordinary UI | 主导航为批量图片、批量视频、资产库、任务、项目、工作流、设置；旧 Shot 入口隐藏。 |
| Regression | 需完成 Rust、frontend、build、diff 检查和 Tauri installer build。 |

## Required live Gate A — batch images

在一个真实项目中：

1. 打开“批量图片”，输入 5 条 Krea2 提示词，按空行拆分成 5 张提示词卡片。
2. 确认批次为 5 项、严格串行、创建 5 个 Task，并为每项产生 Snapshot 和图片 Asset。
3. 在队列详情中核对提示词和参数被冻结；重启应用后核对队列、任务和结果仍可恢复。
4. 让其中一项失败，确认 `continueOnFailure` 保留失败证据并继续后续项。

## Required live Gate B — batch videos

1. 在“资产库”选择 3 张图片，进入“批量视频”。其中至少 1 张必须是手动导入的图片，以证明视频入口不依赖图片批次来源。
2. 为 3 张图片分别填写并保存视频提示词；确认资格状态、`0.1 MP · 1 秒 · 4 步` 安全配置和精确 H3 runtime READY。
3. 创建 H3 批次，确认 3 项、严格串行、3 个 Task、3 个 Snapshot 和 3 个视频 Asset；视频可以用原生播放器播放。
4. 编辑一条提示词后重新创建或检查批次，确认队列项保留编辑后的冻结值；Krea2 批次不应被创建或自动依赖。

## Regression commands

```text
cargo fmt --all -- --check
cargo check
cargo test -- --test-threads=1
pnpm test
pnpm build
git diff --check
pnpm tauri build
```

只有代码门、上述两个真实 Live Gate 和安装包门全部有可核对证据时，才可把状态改为 Ready for Release。当前文档不记录尚未执行的 Task、Asset、Playback 或重启数量。
