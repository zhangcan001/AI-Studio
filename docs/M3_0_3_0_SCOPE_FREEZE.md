# AI Studio 0.3.0 Product Scope Realignment

Date: 2026-08-10
Release status: `CODE READY / LIVE VALIDATION DEFERRED`

## Product direction

0.3.0 不再把 Shot→Krea2→H3 作为普通产品主路径。Shot 相关表、迁移、服务和历史任务继续保留，用于兼容既有数据；普通产品导航不再展示旧 Shot 入口。

正式产品只有两个相互独立的批量入口：

| Product | User flow | Exact workflow ID |
| --- | --- | --- |
| 批量图片 | 提示词列表 → Krea2 图片批次 → 图片 Task / Snapshot / Asset | `wfl_kera2_t2i_local_v2` |
| 批量视频 | 项目图片 Asset + 已保存视频提示词 → H3 视频批次 → 视频 Task / Snapshot / Asset | `wfl_minimax_h3_reference_video` |

Krea2 批量图片直接提供提示词卡片、粘贴文本按空行拆分、添加、复制、删除和排序。MiniMax H3 批量视频支持两种输入来源：从 Asset Library 选择 1–100 张图片并保存视频提示词，或从本地任务目录按同名图片/Prompt 配对或 `h3-batch.json` 批量导入。后者先创建正常 source image Asset 和 Asset Video Prompt，再与前者汇合到既有 `ProductionQueue` → `GenerationService` → `Task` → `Snapshot` → `Asset` 链；Production Queue 永不保存本地绝对路径。

## Frozen runtime and queue contract

- 普通批量图片目录只接受 `wfl_kera2_t2i_local_v2`。
- 普通批量视频目录只接受 `wfl_minimax_h3_reference_video`。
- Krea2 Recipe 必须提供 `prompt`、`width`、`height`、`seed`；界面显示 8 个官方宽高比预设，并按 Recipe 约束过滤 1K/2K 预设，非法自定义值不自动取整或裁剪。
- 创建队列时冻结每项的工作流版本、Recipe、输入 Asset ID、提示词、数字参数和随机 Seed；随机 Seed 在持久化前解析为固定值。
- 队列严格串行执行，`continueOnFailure` 保留失败项并允许后续项目继续；致命执行错误仍按既有队列策略暂停。
- H3 产品能力边界为最高 15 秒、最高 2K；当前 Runtime 仍显示 4 步、单任务串行，历史本机验证档位单独标注为 `0.1 MP · 5 秒 · RTX 5060 Ti 16GB`。
- H3 Recipe 必须具备精确语义键：`prompt` textarea、`reference_image` image/images、`width` integer、`height` integer、`duration_seconds` integer（1–15、step 1、默认 5）、`seed` seed 和 video output；契约缺失时显示 `H3 runtime unavailable`，不静默猜字段。
- H3 当前活动生产 Recipe 为不可变 `1.2.0`；`1.1.2` 及历史 H3 包继续保留用于兼容。技术债：普通 H3 workspace 当前假设正式 workflow ID 只有一个活动生产 Recipe；本次冻结不新增 Recipe Registry 或选择系统。

## Compatibility contract

- 既有 `001`–`011` 迁移、Shot 表和 Shot 后端保持可读可恢复，不删除表、不重写历史数据；`012` 不存在。
- `011_asset_video_prompt.sql` 为图片 Asset 保存项目级视频提示词；提示词会 trim、非空校验，UTF-8 最大 64 KiB。
- Project Backup 版本升级为 v5，新增 Asset 视频提示词数据；恢复继续接受 v1–v5，并执行项目边界校验与 Asset ID remap。
- 不创建第二 Task 模型、第二执行引擎或隐藏的 Shot 自动链路。
- 本地 H3 导入使用 Rust 短时会话（20 分钟），React 只接收 session ID、目录显示名、相对文件名、Prompt 预览和检查状态；不创建第二队列、第二 Prompt 表、外部路径表或目录监控器。

## Ordinary UI boundary

主导航使用“批量图片”“批量视频”“资产库”“任务”“项目”“工作流”“设置”。普通产品界面不使用旧 Shot 生产术语；通用 Workflow 技术工作区和旧兼容代码不等同于正式产品入口。

## Explicit non-goals

本版本不包含第三 Runtime、第二执行引擎、Cloud/Login/Sync/Updater、时间线/NLE、Audio Mixer、Marketplace、移动端或新的外部发布渠道。GPU、Computer Use 和 Desktop Live Gate 由产品负责人暂缓；本轮只完成源码、迁移/备份安全和自动化 Code Gate，不创建 tag、GitHub Release 或上传二进制。
