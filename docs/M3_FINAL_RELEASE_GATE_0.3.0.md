# AI Studio 0.3.0 Simplified Final Live Gate

Date: 2026-08-10
Scope: Product Scope Realignment；禁止回到旧 Shot 主路径或新增其他产品能力。

## Current status

`AI STUDIO 0.3.0 = CODE READY / LIVE VALIDATION DEFERRED`

当前代码目标是两个独立产品：批量图片（Krea2）和批量视频（MiniMax H3）。旧 Shot 数据与后端保持兼容，但不作为普通导航入口。本轮明确不执行 GPU、Computer Use、Desktop Live Gate、tag、GitHub Release 或二进制上传；Live validation 标记为 `DEFERRED BY PRODUCT OWNER`，不是失败。

## Code and compatibility gate

| Gate | Current boundary |
| --- | --- |
| Exact runtime scope | Krea2 与 H3 只按精确 workflow ID 进入各自产品入口。 |
| Asset video prompt | 图片 Asset 的提示词持久化、项目隔离、非空和 64 KiB 校验已接入。 |
| Backup compatibility | Backup v5 保存/恢复 Asset 视频提示词，并继续接受 v1–v5。 |
| Queue contract | 两个入口都创建持久化 Production Queue；输入、参数和随机 Seed 在创建时冻结；严格串行。 |
| Resolution contract | Krea2 提供 8 个官方宽高比及 1K/2K 预设；Krea2/H3 自定义 width/height 均按 Recipe min/max/step 校验，不自动取整。 |
| H3 Recipe contract | 只接受精确语义键；`duration_seconds` 为 1–15 秒、step 1、默认 5 秒，并要求 width/height integer 与 video output。 |
| H3 Recipe selection audit | 当前官方 H3 workflow ID 的活动 Catalog 只有不可变 `1.2.0`；历史包保留兼容。普通 workspace 假设一个活动生产 Recipe，作为已记录技术债，本轮不改选择系统。 |
| Ordinary UI | 主导航为批量图片、批量视频、资产库、任务、项目、工作流、设置；旧 Shot 入口隐藏。 |
| Migration / backup safety | Fresh DB、001–011 保留性、012 缺失、FK cascade、AssetVideoPrompt 边界和 Backup v5 remap/恶意输入回归覆盖。 |
| Regression | Rust 330 tests、frontend 35 files / 114 tests、frontend build、diff 检查和 Tauri installer build。 |

## H3 1.2.0 Local Package Audit

| Item | Result |
| --- | --- |
| Version | `1.2.0` |
| Workflow ID | `wfl_minimax_h3_reference_video` |
| Workflow SHA-256 | `0385e8c53ae005444ae8d12d72145c3c24b681e6fb93f9ba896be9c675a5020a` |
| Recipe SHA-256 | `5d31c17bea33ca1659cf30434415324d4a1af3bee313eb794e6916081b8a3699` |
| Package files | `manifest.yaml` / `recipe.yaml` / `workflow_api.json` = PASS |
| Duration contract | `1–15` / step `1` / default `5` = PASS |
| Resolution contract | width `32–2048` / step `32` / default `1344`; height `32–2048` / step `32` / default `768` = PASS |
| Bindings | prompt, reference_image, width, height, duration_seconds duration/math chain, seed = PASS |
| Compile | 1s PASS · 5s PASS · 10s PASS · 15s PASS · 768-class PASS · 2K-class PASS · Custom PASS |
| GPU | NOT RUN · `DEFERRED BY PRODUCT OWNER` |

The historical `1.1.2` package and its validated workflow bytes remain
preserved and were not modified. The local package audit used no user absolute
paths in this document.

## Deferred live validation — batch images

后续产品负责人批准真实验证后，在一个真实项目中：

1. 打开“批量图片”，输入 5 条 Krea2 提示词，按空行拆分成 5 张提示词卡片。
2. 确认批次为 5 项、严格串行、创建 5 个 Task，并为每项产生 Snapshot 和图片 Asset。
3. 在队列详情中核对提示词和参数被冻结；重启应用后核对队列、任务和结果仍可恢复。
4. 让其中一项失败，确认 `continueOnFailure` 保留失败证据并继续后续项。

## Deferred live validation — batch videos

1. 在“资产库”选择 3 张图片，进入“批量视频”。其中至少 1 张必须是手动导入的图片，以证明视频入口不依赖图片批次来源。
2. 为 3 张图片分别填写并保存视频提示词；确认资格状态、`最高 15 秒 · 最高 2K` 产品能力提示、`4 步 · 单任务串行` 当前 Runtime 提示、历史验证档位提示、Recipe 时长下拉（1–15 秒，默认 5 秒）和精确 H3 runtime READY。
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

上述 Live Gate 当前不执行，不计为失败；待产品负责人批准后再补充真实 Task、Asset、Playback 或重启证据。完成本轮 Code Gate 后，状态保持 `CODE READY / LIVE VALIDATION DEFERRED`。
