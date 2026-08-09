# AI Studio M1 Final Release Gate

日期：2026-08-09
版本：`0.1.0`
代码基线：`e92e831391d9c0c417e9bccc8e9f298a38392b7c`
本轮阻塞修正：`f537b29b578242559266428aadce7a1cab0dbc14`

本轮只处理 0.1.0 发布阻塞、回归、安装包和文档，不增加新模型、数据库 migration、
云端能力或下一阶段功能。`src-tauri/migrations` 的 001–007 与基线一致，未修改。

## Gate 结果

| Gate | 结果 | 证据 |
| --- | --- | --- |
| 版本冻结 | PASS | package、Cargo 和 Tauri 配置均为 0.1.0 |
| 数据库 migration | PASS | 001–007 未发生 diff，已有数据目录保留 |
| Clean Install | PASS | NSIS 静默安装后应用可启动，默认数据正常建立 |
| Clean Uninstall | PASS | 卸载后程序文件移除，干净数据仍保留 |
| Reinstall | PASS | 重新安装后应用可启动并正常关闭 |
| Existing Data / Upgrade | PASS | 原有 3 个项目、18 个工作流文件和历史任务/资产仍可读取 |
| Kera2 真实生成 | PASS | 最终 Gate 运行 2 个真实图片任务，均成功并进入资产库；此前四项持久队列验收为 PASS |
| 持久生产队列 | PASS | 顺序执行、暂停/恢复、重启恢复、Archive/Restore/Delete、Skip/Requeue 已在 Foundation 01–04 验收通过 |
| MiniMax H3 | PASS（沿用既有证据） | 16 GB 显存的 0.1 MP、5 秒、4 steps 实机证据未因本轮发布修正改变 |
| Safe Exit | PASS | 运行中任务显示原生中文警告；继续运行不取消任务，任务完成后关闭正常退出 |
| Single Instance | PASS | 第二次启动后仍只有一个 AI Studio 进程且已有窗口可响应 |
| Diagnostics Export | PASS | ZIP 仅含诊断摘要、说明和日志，内容扫描无敏感项命中 |
| NSIS Installer | PASS | 0.1.0 x64 安装包生成并完成安装/卸载/重装验收 |
| MSI Installer | PASS（构建） | MSI 已生成；未在非提升权限会话执行管理员安装，属于非阻塞限制 |
| 文档 | PASS | 用户发布说明和本 Gate 记录已补齐 |

## 真实 ComfyUI 证据

- 接口：`http://127.0.0.1:8188`
- ComfyUI：`0.30.2`
- GPU：`cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync`
- 显存总量：`17,102,864,384` bytes（约 15.9 GiB）
- 能力节点数量：`4,485`
- Kera2 最终 Gate：2 个 `SUCCEEDED` 图片任务，PNG 资产已写入资产库。

Kera2 持久队列的四项连续任务、暂停/恢复、重启恢复、失败 Skip/Requeue 和资产库闭环
证据见 `docs/M1_PRODUCTION_VALIDATION_01.md`。MiniMax H3 的实机和播放证据见
`docs/M1_MINIMAX_H3_RUNTIME_VALIDATION.md`。

## 现有数据保留

升级前的工作流库指纹为 18 个文件、48,248 bytes；最终仍为 18 个文件、48,248 bytes。
原有项目数据在最终目录中仍可读取，SQLite 中保留 3 个项目、1 个预设、9 个生产批次、
7 条 migration 记录。最终项目目录为 48 个文件、32,366,489 bytes；相对升级前的
44 个文件、29,734,885 bytes，增加的 4 个文件来自本轮新增的两张真实 Kera2 图片及其
缩略图，不是数据丢失或覆盖。

## 诊断包边界

实际导出的 ZIP 条目为：

- `diagnostics.json`
- `README.txt`
- `logs/ai-studio.2026-08-09`

内容扫描未命中 `app.db`、`workflow_api.json`、`recipe.yaml`、`manifest.yaml`、
Windows 绝对路径、`Users`、`PRIVATE_PROMPT` 或完整提示词。技术性的版本、GPU、
`cuda:0`、HTTP 接口和 `x86_64` 信息允许保留。

日志中已观察到启动、启动恢复、安全退出确认、安全退出取消、Kera2 运行和应用重启记录；
日志未写入完整提示词、工作流 JSON、Recipe YAML 或用户存储绝对路径。

## 安装包指纹

以下为最终 0.1.0 构建输出；安装包不提交到 Git：

| 文件 | 大小 | SHA-256 |
| --- | ---: | --- |
| `AI Studio_0.1.0_x64-setup.exe` | 6,152,047 bytes | `1D2BF7045CBB54491B03159CFD5358D750C8A9A1C22F08B97C789C8FC65FDA3E` |
| `AI Studio_0.1.0_x64_en-US.msi` | 8,933,376 bytes | `82D403023F828951E8494921D5628FEC6B431CCF8C2D1B227DAB85356957DACB` |

## 发布限制与停止点

- 本版本不创建 Git tag 或 GitHub Release。
- 不把安装包、诊断 ZIP、运行时数据库、用户工作流、模型或测试素材提交到仓库。
- Wan、Flux、Qwen 未提升为正式运行承诺。
- 本 Gate 完成后停止，不进入新的功能阶段。

## 结论

在最终回归命令全部通过、工作区保持干净且 `master` 推送成功后，
`AI STUDIO 0.1.0 = READY FOR RELEASE`。
