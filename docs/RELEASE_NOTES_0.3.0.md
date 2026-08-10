# AI Studio 0.3.0 Release Notes

Status: release candidate / not released
Date: 2026-08-10
Baseline: `6ae12dc5d92fcb6735b9baf771864d71ad4753c2`

> 0.3.0 当前只是一条本地开发发布候选线。没有创建 `v0.3.0` tag，没有创建 GitHub Release，也没有上传安装包。

## Scope

0.3.0 冻结为 Kera2 图片关键帧 + MiniMax H3 参考图生视频的双 Runtime 产品范围，覆盖 Prompt Library、Experiment、Production Queue/Dashboard、Asset Organization、Project Templates、Backup v4、Shot 制作和 Shot Batch Production。通用 Workflow onboarding / technical workspace 仍可用，但不扩大普通生产 UI 的 Runtime 范围。

精确产品 Runtime ID：

- `wfl_kera2_t2i_local_v2`
- `wfl_minimax_h3_reference_video`

## Release hardening changes

- 移除 Shot Batch Planner 按名称、分类、模式拼接字符串的模糊 Runtime 判断。
- 在 Rust application 层和前端生产 UI 层加入精确 ID scope helper，并覆盖伪 Kera2 / 伪 MiniMax H3 回归测试。
- Creation Launcher、Creation Studio 和 Shot Recipe 选择仅展示冻结的两个产品 Runtime；通用 Workflow Workspace 不受影响。
- 保持 migrations `001–010` 不变，不增加 `011`；保持 Backup v4 与 v1–v4 兼容恢复。
- 保持单实例、安全退出、诊断隐私、项目隔离、冻结配置、严格顺序、失败保留和重启恢复约束。
- 补齐 0.3.0 Scope Freeze、Final Release Gate、安装包 SHA-256 清单和本地发布验证记录。

## Verification status

自动化代码门和 Windows 安装包门已通过，最终状态仍取决于真实可控桌面 Live Gate。Live Gate 必须在一个 `Release Gate 0.3.0` 项目中留下可核对的 3 Shot、Kera2 batch、人工关键帧选择、H3 batch、视频播放、失败保留和重启恢复证据；没有这组证据就不能标记为 Ready for Release。

本地候选制品及摘要记录在 [`docs/RELEASE_SHA256_0.3.0.txt`](RELEASE_SHA256_0.3.0.txt)：

- `src-tauri/target/release/bundle/nsis/AI Studio_0.3.0_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/AI Studio_0.3.0_x64_en-US.msi`
- `src-tauri/target/release/ai-studio.exe`

当前候选线的诚实状态：

`AI STUDIO 0.3.0 = RELEASE CANDIDATE / LIVE GATE PENDING`

详情见 [`docs/M3_FINAL_RELEASE_GATE_0.3.0.md`](M3_FINAL_RELEASE_GATE_0.3.0.md)。
