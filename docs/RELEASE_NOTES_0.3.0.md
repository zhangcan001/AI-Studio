# AI Studio 0.3.0 Release Notes

Status: code ready / live validation deferred
Date: 2026-08-11

> 0.3.0 当前是本地开发候选线。没有创建 `v0.3.0` tag，没有创建 GitHub Release，也没有上传安装包。

## Product scope

0.3.0 聚焦两个独立批量产品：

- 批量图片：提示词列表 → Krea2 批次 → 图片 Assets，workflow ID 为 `wfl_kera2_t2i_local_v2`。
- 批量视频：MiniMax H3 FL2VA（文生、一张图、首尾帧）与 REF2VA（图片、音频、图片+音频、视频+图片）都通过现有统一生产队列进入视频 Assets；FAST 保留 `wfl_minimax_h3_fl2va` / `wfl_minimax_h3_reference_video`，QUALITY 使用独立不可变 workflow 包。

旧 Shot 数据和后端仍保留以支持兼容恢复，但 Shot 不再出现在普通产品导航，也不再作为图片到视频的主链路。

## Release hardening changes

- 批量图片入口收敛为 Krea2 已就绪 → 公开参数 → Prompt 列表 → 持久化图片队列；支持按空行拆分、添加、复制、删除和排序。
- Krea2 批量图片增加 Recipe-bound width/height 控件，提供 8 个官方宽高比以及按 Recipe 约束过滤的 1K/2K 预设；自定义值严格校验，不自动取整或裁剪。
- MiniMax H3 输出分辨率预设调整为图片规格中的 14 档 16:9 MP 梯度：608×352、736×416、864×480、960×544、1056×608、1152×640、1216×672、1280×736、1344×768、1376×768、1504×832、1664×928、1824×1024、1920×1088；Project Folder 自动导入默认使用 960×544。
- 资产库支持选择图片、视频、音频 Asset，并为图片保存项目级视频提示词；H3 批量视频入口按模式创建独立的严格串行 Production Queue。
- MiniMax H3 批量视频新增“从本地导入”模式：Prompt-only、同名图片/Prompt、首尾帧配对、`h3-batch.json` 与安全的 `h3-omni-batch.json`；图片、视频、音频先导入正常 Asset，再汇合到现有 Production Queue，队列和快照只保存 Asset ID，不保存本地绝对路径。
- MiniMax H3 本地导入新增 `PROJECT_FOLDER`：ProjectRoot 的每个一级子文件夹独立作为一个 Segment，自动推断 FL2VA/REF2VA 模式，读取 Prompt front matter，按自然顺序排列参考媒体，并支持在提交前编辑每段 Prompt、模式、首尾帧、媒体顺序、时长和分辨率；所有 Segment 仍汇入一个严格串行 Production Queue。
- H3 只接受先经过本机 `/object_info` 与真实 graph 审计的精确 Recipe 语义键；产品能力为最高 15 秒、最高 2K，`duration_seconds` 公开为 Recipe 驱动的 1–15 秒下拉，step 1、默认 5 秒；QUALITY 默认 20 步正式工作流，FAST 保留 4 步 Turbo，均使用单任务串行队列，历史本机验证档位单独保留。
- H3 新增四个不可变 QUALITY `2.0.0` 生产包：FL2VA T2V、I2V、First/Last 与 Omni REF2VA。T2V/REF2VA 使用 INT8 ConvRot 与 HyperStep Middle-36，I2V/First-Last 保持正式 20 步采样但不添加 HyperStep；四个 QUALITY 图均不含 Turbo LoRA。旧 FL2VA `1.0.0`、Omni REF2VA `1.3.0` FAST 包以及 `1.2.0`、`1.1.2` 和更早历史版本保持不变。普通 H3 workspace 按每个 workflow ID 假设一个活动生产 Recipe 的选择逻辑记录为技术债，本轮不扩展 Recipe Registry。
- 新增 `011_asset_video_prompt.sql`；Project Backup 升级为 v5，并保留 v1–v5 恢复兼容。
- 创建队列时冻结输入、参数和随机 Seed；两个产品都使用严格串行队列与失败继续策略。
- 普通导航收敛为“批量图片 / 批量视频 / 资产库 / 任务 / 项目 / 工作流 / 设置”。
- 资产库支持单个或批量删除图片、视频、音频素材；删除前检查活动任务与生产队列引用，安全清理项目内素材文件和缩略图，同时保留任务、快照、事件历史。
- 设置 → ComfyUI 增加“释放显存/内存”：仅在 AI Studio 与 ComfyUI 队列均空闲时调用官方 `POST /free`，只卸载模型并释放内存，不删除模型文件。

## Verification status

本轮 Code Gate：Rust 370 tests / 0 failed；frontend 39 files / 128 tests / 0 failed；frontend build 与 Tauri installer build PASS。产物 SHA 以 `RELEASE_SHA256_0.3.0.txt` 为准。GPU、Computer Use 和 Desktop Live Gate 暂不执行；真实 Gate A/B 标记为 `DEFERRED BY PRODUCT OWNER`，不是失败。

当前候选线状态：

`AI STUDIO 0.3.0 = CODE READY / LIVE VALIDATION DEFERRED`

详情见 [`docs/M3_FINAL_RELEASE_GATE_0.3.0.md`](M3_FINAL_RELEASE_GATE_0.3.0.md)。
