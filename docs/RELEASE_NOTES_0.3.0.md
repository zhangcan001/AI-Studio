# AI Studio 0.3.0 Release Notes

Status: code ready / live validation deferred
Date: 2026-08-10

> 0.3.0 当前是本地开发候选线。没有创建 `v0.3.0` tag，没有创建 GitHub Release，也没有上传安装包。

## Product scope

0.3.0 聚焦两个独立批量产品：

- 批量图片：提示词列表 → Krea2 批次 → 图片 Assets，workflow ID 为 `wfl_kera2_t2i_local_v2`。
- 批量视频：图片 Asset + 已保存视频提示词 → MiniMax H3 批次 → 视频 Assets，workflow ID 为 `wfl_minimax_h3_reference_video`。

旧 Shot 数据和后端仍保留以支持兼容恢复，但 Shot 不再出现在普通产品导航，也不再作为图片到视频的主链路。

## Release hardening changes

- 批量图片入口收敛为 Krea2 已就绪 → 公开参数 → Prompt 列表 → 持久化图片队列；支持按空行拆分、添加、复制、删除和排序。
- 资产库支持为图片保存项目级视频提示词；H3 批量视频入口支持 1–100 张图片、资格状态和独立视频队列。
- H3 只接受精确 Recipe 语义键；安全配置为 `0.1 MP · 4 步 · 单任务串行`，`duration_seconds` 公开为 Recipe 驱动的 1–5 秒下拉，默认 5 秒，非法范围阻断。
- H3 Recipe 审计确认正式 workflow ID 当前只有一个活动生产 Recipe（`1.1.2`）；历史版本保留兼容。普通 H3 workspace 假设一个活动生产 Recipe 的选择逻辑记录为技术债，本轮不扩展 Recipe 系统。
- 新增 `011_asset_video_prompt.sql`；Project Backup 升级为 v5，并保留 v1–v5 恢复兼容。
- 创建队列时冻结输入、参数和随机 Seed；两个产品都使用严格串行队列与失败继续策略。
- 普通导航收敛为“批量图片 / 批量视频 / 资产库 / 任务 / 项目 / 工作流 / 设置”。

## Verification status

Code Gate 结果为 Rust 325 tests、frontend 34 files / 108 tests、frontend build、diff 检查和 Windows 安装包构建。GPU、Computer Use 和 Desktop Live Gate 暂不执行；真实 Gate A/B 标记为 `DEFERRED BY PRODUCT OWNER`，不是失败。

当前候选线状态：

`AI STUDIO 0.3.0 = CODE READY / LIVE VALIDATION DEFERRED`

详情见 [`docs/M3_FINAL_RELEASE_GATE_0.3.0.md`](M3_FINAL_RELEASE_GATE_0.3.0.md)。
