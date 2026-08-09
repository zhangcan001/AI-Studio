# M1 DAILY USE / RELEASE HARDENING

日期：2026-08-09

基线：`8d06b1d docs: close m1 product ux polish validation`

实现提交：`66bb6bf feat: harden m1 daily use release`

版本：`0.1.0`

本阶段保持 Tauri 2 + React + Rust 技术路线，不增加数据库 migration，不增加模型、云端、登录、遥测、自动更新或同步功能。

## Single Instance

结果：PASS

- 使用 `tauri-plugin-single-instance`，在数据库、AppState 和启动恢复之前注册。
- 第二次启动同一 Release executable 后自动退出，实测始终只有一个 AI Studio 进程和一个中文窗口标题。
- 第二进程的参数、工作目录和数据库访问不会进入主实例业务流程。

## Logging

结果：PASS

- 使用 `tracing-appender` 按日滚动 UTF-8 日志；Release 默认 INFO，开发版 DEBUG，第三方库限制为 WARN。
- 日志目录沿用现有 AppData 日志目录，不向界面暴露原始路径。
- 只清理 AI Studio 自有日志，保留约 7 天并限制总量约 100 MiB；清理失败只告警，不阻断启动。
- Release 实机产生持久日志，实测 1 个自有日志文件、约 4.2 KiB。
- 日志和诊断包扫描确认不包含完整 Prompt、绝对路径、数据库/项目/资产路径或测试隐私标记。

## Diagnostics

结果：PASS

- 新增安全的 `DiagnosticsService`、`diagnostics_summary` 和 `diagnostics_export`。
- 摘要包含版本、平台、架构、数据库健康度、ComfyUI 状态/版本、GPU/显存、工作流包统计、全局活动任务、生产队列忙闲、日志可用性和保留天数。
- 导出使用原生保存对话框，包名为 `AI-Studio-Diagnostics-YYYYMMDD-HHMMSS.zip`。
- 导出包只包含 `diagnostics.json`、README 和受限的近期自有日志，不包含数据库、项目、资产、工作流原文、配方原文、完整 Prompt 或绝对路径。
- Rust 隐私测试覆盖私有 Prompt、Windows 路径和导出包内容；测试通过。

## Startup

结果：PASS

- Bootstrap 未完成时显示“正在准备创作环境……”，不伪造百分比。
- ComfyUI 离线只降级运行环境，不阻断 AI Studio 启动。
- 数据库、工作流库和恢复失败使用中文原生错误对话框和安全错误代码；可重试。
- 空白数据目录 Release 启动完成迁移、工作流同步和 ComfyUI 能力刷新，日志记录恢复检查 `examined=0`。

## Exit / Recovery

结果：PASS（源码、回归测试和 Release 启动恢复通过）

- 使用真实 Tauri v2 `getCurrentWindow().onCloseRequested`。
- 退出检查调用全局 `runtime_activity_status`；任务或生产队列活动未知时采取保守确认策略。
- 退出不会取消或中断任务；确认退出时交给 Tauri close-requested 默认销毁流程，取消退出时才调用 `preventDefault()`。
- 启动恢复显示“正在同步上次未完成的任务……”和同步结果，不重复提交任务。
- Rust 任务恢复、Production Admission 和前端退出判断测试均通过。
- 当前 WebView2 CUA 在窗口 close-request/模态交互上返回 `node_repl exec context not found`，因此没有把自动化关闭消息当作人工点击结果；Release 进程、恢复日志和代码路径均已核验。

## Settings

结果：PASS（前端回归与 Release 构建通过）

- 新增“设置”工作区，包含应用信息、运行环境、ComfyUI 状态、连接测试、节点刷新和诊断包导出。
- 设置页只显示安全摘要，不显示原始日志、数据库路径、项目根目录或资产路径。
- 新增设置页渲染测试；全部用户可见普通文案保持简体中文。

## No Workflow Startup UX

结果：PASS

- 无工作流时显示中文三步引导，不暴露内部工作流库路径。
- 提供“前往工作流管理”和“测试 ComfyUI 连接”操作。
- 新增无工作流引导测试。

## Release

结果：PASS

- 窗口标题为 `AI Studio - 本地 AI 创作工作台`，最小窗口约 `1000 × 700`。
- Release executable、MSI 和 NSIS bundle 均成功生成，版本保持 `0.1.0`。
- NSIS 静默安装、安装版启动、安装版单实例和静默卸载均通过；卸载后既有数据库、工作流库和项目文件指纹保持一致。
- 当前 Windows 会话不是管理员，MSI 静默安装返回 Windows Installer `1603`；MSI 构建产物通过，NSIS 已完成实际安装验收。
- 未添加虚假发布者、签名、更新器或自动更新配置。
- Kera2/H3 Runtime 代码未改动，本阶段不重复执行 H3 OOM 生成；既有 Runtime Gate 证据保持有效。

## Real ComfyUI Smoke

| 项目 | 实测值 |
| --- | --- |
| 接口地址 | `http://127.0.0.1:8188` |
| ComfyUI 版本 | `0.30.2` |
| GPU | `NVIDIA GeForce RTX 5060 Ti` |
| 显存 | 总量约 `15.9 GB`；空闲随运行时约 `1.2–1.5 GB` |
| 节点数量 | `4485` |

## Data Safety Smoke

- 既有数据库 SHA-256：`203EADD24B78F15B164A1BB1AEA55EA253CCCA3DF7E9CB25FE1D7A78E503DB13`。
- Release/安装/卸载前后数据库指纹一致。
- 工作流库：18 个文件，48,248 字节；前后保持一致。
- 项目文件：44 个文件，29,734,885 字节；前后保持一致。
- 未修改 migration 文件。

## Tests

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check` | PASS |
| `cargo test -- --test-threads=1` | PASS，248 passed，0 failed |
| `pnpm test` | PASS，23 test files，58 passed，0 failed |
| `pnpm build` | PASS |
| `git diff --check` | PASS（仅有 Windows 换行提示） |
| `pnpm tauri build` | PASS，Windows x64 executable、MSI、NSIS 均生成 |

## 修改范围

- 单实例启动与中文发布元数据。
- 持久日志、日志隐私、保留策略和诊断包。
- DiagnosticsService、Tauri diagnostics commands、设置工作区。
- 启动引导、恢复提示、无工作流引导和安全退出 UX。
- Release 响应式状态栏与窗口尺寸约束。
- Rust 248 项回归测试和前端 58 项回归测试覆盖。

## 技术债

- 当前日志和诊断仍是本地单机能力，尚无崩溃上报、云端诊断或远程协作。
- MSI 在非管理员会话下需要后续用管理员权限补做一次安装验收；NSIS 安装路径已完成实际验收。
- WebView2 CUA 在当前桌面上下文不能可靠操作 Tauri 原生/模态窗口，后续人工 Windows Gate 仍需补充真实鼠标确认路径。
- 当前版本仍保持既有 Runtime 范围，不在本阶段处理 MiniMax H3 OOM。

## Final Status

**M1 DAILY USE / RELEASE HARDENING = PASS**

下一阶段建议：仅进入 **M1 Final Release Gate**。

本阶段完成后停止。
