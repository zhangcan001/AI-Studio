# AI Studio 0.3.0 Release Notes

Status: release candidate / simplified live gate pending
Date: 2026-08-10

> 0.3.0 当前是本地开发候选线。没有创建 `v0.3.0` tag，没有创建 GitHub Release，也没有上传安装包。

## Product scope

0.3.0 聚焦两个独立批量产品：

- 批量图片：提示词列表 → Krea2 批次 → 图片 Assets，workflow ID 为 `wfl_kera2_t2i_local_v2`。
- 批量视频：图片 Asset + 已保存视频提示词 → MiniMax H3 批次 → 视频 Assets，workflow ID 为 `wfl_minimax_h3_reference_video`。

旧 Shot 数据和后端仍保留以支持兼容恢复，但 Shot 不再出现在普通产品导航，也不再作为图片到视频的主链路。

## Release hardening changes

- 批量图片入口支持提示词卡片、按空行拆分、添加、复制、删除和排序，并直接创建持久化图片队列。
- 资产库支持为图片保存项目级视频提示词；H3 批量视频入口支持 1–100 张图片、资格状态和独立视频队列。
- 新增 `011_asset_video_prompt.sql`；Project Backup 升级为 v5，并保留 v1–v5 恢复兼容。
- 创建队列时冻结输入、参数和随机 Seed；两个产品都使用严格串行队列与失败继续策略。
- 普通导航收敛为“批量图片 / 批量视频 / 资产库 / 任务 / 项目 / 工作流 / 设置”。

## Verification status

自动化回归和 Windows 安装包门完成后，仍需在真实桌面执行 Gate A（5 条 Krea2 提示词）和 Gate B（3 个图片 Asset + H3 提示词）并记录任务、快照、资产、播放和重启恢复证据。没有这组证据就不能标记为 Ready for Release。

当前候选线状态：

`AI STUDIO 0.3.0 = RELEASE CANDIDATE / SIMPLIFIED LIVE GATE PENDING`

详情见 [`docs/M3_FINAL_RELEASE_GATE_0.3.0.md`](M3_FINAL_RELEASE_GATE_0.3.0.md)。
