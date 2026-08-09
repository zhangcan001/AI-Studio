# M1 PRODUCT UX POLISH 验证记录

日期：2026-08-09
基线：0e8a9fc fix: close zh-cn ui final sweep
验证实现提交：015f741 feat: polish m1 creation workspace ux

## 范围

本阶段只做前端体验重构，正式 Runtime 仍冻结为：

- Kera2 文生图
- MiniMax H3 参考图生视频

未新增数据库 migration、Rust 业务逻辑、模型、云端、登录、DSL 或语言切换；Production Admission、任务状态机、Comfy 协议、快照、资产存储和 Workflow Compiler 未改动。

## Studio Layout

- 双栏桌面布局：左侧创作控制，右侧任务与结果。
- StudioModeTabs 默认进入“单次创作”；“批量生产”只在显式切换后显示临时任务清单和持久生产队列。
- 工作流入口改为基于 catalog 的 WorkflowLauncher 卡片，保留未知 catalog item 的通用渲染。
- GenerationActionBar 位于动态输入之后，主操作为“开始生成”，操作区保持易发现。
- 生产队列 busy、ComfyUI 离线、任务事件通道、素材缺失、校验错误和不支持字段只呈现一条最高优先级阻塞原因。
- 预设名称在点击“保存当前”或“另存为”后才显示为紧凑编辑区。
- MiniMax H3 安全提示收敛为“16GB 安全配置 / 0.1MP · 5 秒 · 4 步 · 单任务”。

## Asset Picker

- AssetPickerDialog 支持 image、video、audio 和 multiple。
- 素材列表使用当前 projectId 的 project-scoped API；未向 React 暴露 storage path 或 absolute path。
- 图片优先读取 thumbnail；视频卡只显示缩略图/占位、时长和名称，不在网格中 autoplay；音频卡不创建播放器。
- 选择器保留本地导入入口，导入成功后自动选中；只有点击“确定”才提交选择，Esc/取消不会修改既有值。
- 多素材保持用户选择顺序，并继续支持上移、下移、移除。
- Project 切换会关闭 picker transient state。

## Task Status / Result Preview

- 任务状态卡改为紧凑信息：状态、进度、耗时、队列序号和取消操作保留。
- QUEUED 显示“等待生成”，COLLECTING 显示“正在整理生成结果...”，成功显示“生成完成”与用时，失败继续显示中文主错误和技术错误码。
- 右栏没有任务时只保留一个生成结果空状态；成功后主结果替换占位，多输出使用结果缩略图切换。
- 视频输出继续使用 controls、preload="metadata"、playsInline 和逻辑媒体 URL。

## Runtime Status

Release smoke 中 ComfyUI 真实状态：

- Endpoint：http://127.0.0.1:8188
- Version：0.30.2
- GPU：cuda:0 NVIDIA GeForce RTX 5060 Ti
- VRAM：约 1.8 GB 空闲 / 15.9 GB 总量
- Node Count：4485

Studio 默认显示紧凑运行环境条；离线时自动展开核心提示，连接后可手动展开接口、版本、GPU、显存和节点数量详情。

## Real Kera2 Gate

通过 release executable 的新创作界面执行：

1. 选择 Kera2 工作流卡片。
2. 输入 Prompt。
3. 点击“开始生成”。
4. 任务经历生成中并进入已完成。
5. 右侧生成结果显示图片主预览。

结果：PASS。实际得到 768×1280 生成图片并进入结果区域/资产数据。

## Real MiniMax H3 Gate

通过新 Asset Picker 选择当前项目的源图片后执行：

- 安全配置：0.1MP、5 秒、4 步、单任务
- 任务状态：生成中 → 正在整理生成结果 → 已完成
- 输出：生成视频，5 秒
- 播放地址：逻辑 http://aistudio-media.localhost/video?... URL

结果：PASS。未直接构造 Asset ID 绕过素材选择器。

## Batch / Production UI Gate

- “单次创作 → 加入批量清单 → 批量生产”流程可见。
- 清单显示顺序和工作流名称。
- 生产队列只在“批量生产”区域出现；单次创作不显示完整队列面板。
- 切回单次创作后 Prompt 草稿仍保留。
- 既有 Production Admission / pause / resume / restart / requeue / skip 回归测试继续通过。

结果：PASS。

## Desktop Walkthrough

Release executable 中已实际打开并检查工作区导航：

创作、资产库、任务、项目、工作流

重点创作页面确认：

- 工作流卡片明显
- 单次创作默认
- 批量工具不干扰单次主流程
- 主生成按钮可定位，滚动到输入区时保持 sticky 易发现
- 图形化素材选择器可打开、取消、选择和确认
- 结果主预览位于右栏
- 任务状态清晰

CDP viewport / layout smoke：

| 视口 | 横向溢出 | 结果栏 | 生成操作 |
| --- | --- | --- | --- |
| 1180×760 | 无 | 可用 | 输入区内 sticky |
| 1366×768 | 无 | 可用 | 输入区内 sticky |
| 1920×1080 | 无 | 可用 | 输入区内 sticky |

Windows CUA 状态抓取对 Tauri WebView2 返回 context not found，因此 release 窗口使用本机截图和 WebView2 本地 CDP 完成同等可复核的视觉/DOM smoke；不影响产品运行。

## Release Validation

执行命令：

- cargo fmt --all -- --check
- cargo check
- cargo test -- --test-threads=1
- pnpm test
- pnpm build
- git diff --check
- pnpm tauri build

实际测试数量和最终 PASS/FAIL 以最终报告为准。
