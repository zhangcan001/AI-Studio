# AI Studio 0.3.0 Product Scope Realignment

Date: 2026-08-10
Release status: `CODE READY / LIVE VALIDATION DEFERRED`

## Product direction

0.3.0 不再把 Shot→Krea2→H3 作为普通产品主路径。Shot 相关表、迁移、服务和历史任务继续保留，用于兼容既有数据；普通产品导航不再展示旧 Shot 入口。

正式产品只有两个相互独立的批量入口：

| Product | User flow | Exact workflow ID |
| --- | --- | --- |
| 批量图片 | 提示词列表 → Krea2 图片批次 → 图片 Task / Snapshot / Asset | `wfl_kera2_t2i_local_v2` |
| 批量视频 | MiniMax H3 FL2VA / REF2VA mode-specific Asset or local import inputs → H3 视频批次 → 视频 Task / Snapshot / Asset | FAST: `wfl_minimax_h3_fl2va`, `wfl_minimax_h3_reference_video`; QUALITY: `wfl_minimax_h3_fl2va_t2v_quality`, `wfl_minimax_h3_fl2va_i2v_quality`, `wfl_minimax_h3_fl2va_first_last_quality`, `wfl_minimax_h3_reference_video_quality` |

Krea2 批量图片直接提供提示词卡片、粘贴文本按空行拆分、添加、复制、删除和排序。MiniMax H3 批量视频支持 FL2VA 文生视频、一张图生视频、首尾帧，以及 REF2VA 图片、音频、图片+音频、视频+图片模式；普通 UI 的本地入口统一为 ProjectRoot（每个一级子文件夹一个 Segment），可自动识别 Prompt-only、任意单图 I2V、首尾帧和 REF2VA 混合模式；旧配对/清单格式只保留后端兼容。本地媒体先创建正常 source Asset 和 Asset Video Prompt（图片），再与资产库输入汇合到既有 `ProductionQueue` → `GenerationService` → `Task` → `Snapshot` → `Asset` 链；Production Queue 永不保存本地绝对路径。

## Frozen runtime and queue contract

- 普通批量图片目录只接受 `wfl_kera2_t2i_local_v2`。
- 普通批量视频目录只接受 `wfl_minimax_h3_reference_video`。
- Krea2 Recipe 必须提供 `prompt`、`width`、`height`、`seed`；界面显示 8 个官方宽高比预设，并按 Recipe 约束过滤 1K/2K 预设，非法自定义值不自动取整或裁剪。
- 创建队列时冻结每项的工作流版本、Recipe、输入 Asset ID、提示词、数字参数和随机 Seed；随机 Seed 在持久化前解析为固定值。
- 队列严格串行执行，`continueOnFailure` 保留失败项并允许后续项目继续；致命执行错误仍按既有队列策略暂停。
- H3 输出分辨率预设固定为图片规格中的 16:9 梯度：`608×352`、`736×416`、`864×480`、`960×544`、`1056×608`、`1152×640`、`1216×672`、`1280×736`、`1344×768`、`1376×768`、`1504×832`、`1664×928`、`1824×1024`、`1920×1088`；Project Folder 按 UI Override → Front Matter → Prompt 正文显式规格 → 素材比例 → Recipe 默认值为每个 Segment 独立解析 duration/resolution；无规格且无素材推断时才使用 `5 秒 / 960×544` 默认档，手动值仍按 Recipe 合法范围校验。
- H3 产品能力边界为最高 15 秒、最高 2K；QUALITY 为默认/推荐的 20 步正式工作流，FAST 保留历史 4 步 Turbo 工作流；两者都使用既有单任务串行队列，历史本机验证档位单独标注为 `0.1 MP · 5 秒 · RTX 5060 Ti 16GB`。
- H3 Recipe 必须具备经过本机 `/object_info` 与 graph 审计的精确语义键：FL2VA 的 `prompt` / optional `first_frame` / optional `last_frame`，或 REF2VA 的 plural `reference_images` / `reference_videos` / `reference_audios`，以及 `width`、`height`、`duration_seconds`（1–15、step 1、默认 5）、`seed` 和 video output；契约缺失时显示 `当前本地 H3 工作流未启用该模式`，不静默猜字段。
- H3 当前内置 FAST 生产 Recipe 为 FL2VA `1.0.0` 与 Omni REF2VA `1.3.0`，QUALITY 生产 Recipe 为四个不可变 `2.0.0` 包（T2V、I2V、First/Last、REF2VA）；Project Folder 按 mode + profile 选择并把 QUALITY/FAST 冻结进队列真相。历史 `1.2.0`、`1.1.2` 及更早 H3 包继续保留用于兼容。技术债：普通 H3 workspace 当前按每个正式 workflow ID 假设一个活动生产 Recipe；本次冻结不新增 Recipe Registry 或选择系统。

## Compatibility contract

- 既有 `001`–`011` 迁移、Shot 表和 Shot 后端保持可读可恢复，不删除表、不重写历史数据；`012` 不存在。
- `011_asset_video_prompt.sql` 为图片 Asset 保存项目级视频提示词；提示词会 trim、非空校验，UTF-8 最大 64 KiB。
- Project Backup 版本升级为 v5，新增 Asset 视频提示词数据；恢复继续接受 v1–v5，并执行项目边界校验与 Asset ID remap。
- 不创建第二 Task 模型、第二执行引擎或隐藏的 Shot 自动链路。
- 本地 H3 Project Folder 导入使用 Rust 短时会话（20 分钟），React 只接收 session ID、目录显示名、Segment/相对文件名、Prompt 预览和检查状态；旧导入模式仍由后端兼容但不进入普通 UI；不创建第二队列、第二 Prompt 表、外部路径表或目录监控器。

## Ordinary UI boundary

主导航使用“批量图片”“批量视频”“资产库”“任务”“项目”“工作流”“设置”。普通产品界面不使用旧 Shot 生产术语，H3 本地导入只显示 Project Folder；通用 Workflow 技术工作区和旧兼容代码不等同于正式产品入口。

## Explicit non-goals

本版本不包含第三 Runtime、第二执行引擎、Cloud/Login/Sync/Updater、时间线/NLE、Audio Mixer、Marketplace、移动端或新的外部发布渠道。GPU、Computer Use 和 Desktop Live Gate 由产品负责人暂缓；本轮只完成源码、迁移/备份安全和自动化 Code Gate，不创建 tag、GitHub Release 或上传二进制。
