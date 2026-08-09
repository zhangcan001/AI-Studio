# AI Studio M2 FOUNDATION PACK 01

日期：2026-08-09  
开发版本：`0.2.0`  
正式版本：`v0.1.0`（已发布，保持不可变）  
正式版本基线：`c3cbbaa6ece05939bc93c75c906f409e3bacea24`

本阶段只完成 M2 Foundation Pack 01。没有创建 `v0.2.0` Tag 或 GitHub Release，也没有修改数据库 migration、v0.1.0 Tag、Release 或安装包。本轮完成了 Snapshot 资产引用重映射阻塞修复，并通过真实备份恢复、历史输入复用和恢复项目再生成验收。

## 1. Pack 状态

| 范围 | 状态 | 说明 |
| --- | --- | --- |
| M2-01 持久设置 | PASS | `AIStudioData/config/settings.json`，schema v1，未知字段容错，损坏回退与原子写入 |
| M2-02 自定义 ComfyUI 地址 | PASS | 仅允许 http/https，规范化并拒绝凭据、query、fragment、非根路径 |
| M2-03 连接运行时重配置 | PASS | 共享 `ComfyAdapterHandle`，空闲保护，切换后失效并刷新能力缓存 |
| M2-04 项目备份导出 | PASS | 后端建立校验 ZIP，原生保存对话框，不向前端暴露绝对路径 |
| M2-05 项目备份恢复 | PASS | inspection token、预览、staging、事务恢复、新 ID 与关系映射 |
| M2-06 备份安全校验 | PASS | manifest 首项、路径/符号链接/大小/数量限制、SHA-256 校验、失败补偿清理 |
| M2-07 设置与项目 UX | PASS | 设置页连接区、测试/保存应用；项目页导出/恢复预览与确认 |
| M2-08 无 Migration 配置 | PASS | 未修改 001–007，设置仅保存为 JSON |
| M2-09 开发发布 Gate | PASS | Rust、前端、构建和差异检查均通过 |
| M2-10 Snapshot 资产引用重映射 | PASS | 结构化 JSON 精确值重映射、归属校验和事务原子性均通过 |
| M2-11 Final Live Gate | PASS | 真实 Endpoint、备份恢复、历史复用和恢复项目再生成均通过 |

## 2. 持久设置与 Endpoint

设置文件格式：

```json
{
  "schemaVersion": 1,
  "comfy": {
    "endpoint": "http://127.0.0.1:8188"
  }
}
```

缺少设置文件时使用默认地址；JSON 损坏或 schema 不支持时使用默认设置，并显示：`设置文件无法读取，当前已使用默认配置。`。读取失败不会自动覆盖原文件。保存流程为临时文件写入、`sync_all`、原子替换目标文件，并尽力同步父目录。

Endpoint 变更必须先成功请求 `/system_stats` 和 `/object_info`，失败地址不会保存。候选地址测试在共享 admission gate 外执行；获得与生成、批量生成、生产队列启动和恢复相同的 gate 后，后端再次检查全局活动状态并执行快速 health check，再在 guard 内完成保存、runtime 替换、能力缓存失效与刷新。存在活动任务或生产队列占用时返回 `COMFY_ENDPOINT_CHANGE_BUSY`，不会只依赖前端按钮禁用。

## 3. Hardening Fixes

- Endpoint 竞态：`SettingsService` 使用 `ProductionQueueService.admission_gate` 的同一底层锁；最终 activity check 与 `runtime.replace()` 之间不允许新的 generation、batch 或 production queue submission 插入。新增确定性 active-task / production-queue 并发回归，确认拒绝时 settings 与 runtime 均保持不变。
- 原始日志隐私：settings 读取失败只记录错误类别，不记录错误字符串、文件内容或绝对路径；新增 raw tracing 输出测试，覆盖带 `PRIVATE_USER` 特征路径的损坏 JSON。
- Windows 原子保存：Windows 使用 `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`，非 Windows 使用同步后的临时文件 rename；不再删除旧 settings 文件作为替换 fallback，替换失败时旧内容保持可读，临时文件尽力清理。
- 流式项目备份：数据库元数据在一个短 SQLite read transaction 中快照，提交后才读取文件；ZIP 直接写入临时文件，资产和缩略图使用堆上的固定 1 MiB buffer 流式写入并校验大小/SHA-256，完成 `finish` 与 `sync_all` 后才原子发布。20 GiB ZIP、10 GiB entry、100,000 entries 限制保持不变，校验失败不会发布目标 ZIP。
- Backup format 保持 v1；恢复继续使用新 ID、staging 与事务，不提交 Comfy prompt、不上传输入、不自动 dispatch。

## 5.1 Snapshot Asset Reference Remap Blocker

根因是恢复流程此前只重映射 `task_output_assets`，没有重写 `generation_snapshots.user_inputs_json` 与 `resolved_inputs_json` 中的资产引用。修复后恢复阶段会解析 JSON，对数组和对象递归遍历，只替换资产映射表中完全匹配的 JSON 字符串值；Prompt、键名、未知字符串、数字、布尔值、null、顺序和重复项保持不变。该逻辑与工作流类型无关，不包含模型分支。

恢复在同一 SQLite 事务内完成快照转换、旧 ID 残留检查和恢复项目资产归属校验。任一校验失败返回 `BACKUP_SNAPSHOT_ASSET_REMAP_FAILED` 并回滚整个恢复，不产生半恢复项目或孤立行。备份格式仍为 v1，未新增 migration。

自动化覆盖：单资产、多资产与重复项、嵌套 JSON、文本/未知值/标量保持、H3-like 输入、完整 roundtrip、项目归属和失败原子性。

真实新备份恢复审计结果：恢复项目初始有 33 个快照，新增再生成任务后共 34 个快照；其中 6 个快照包含资产引用，识别到 2 个恢复资产 ID，旧原项目资产 ID 残留为 0，恢复项目外引用为 0。4 个有视频输出的 H3 历史快照全部通过；6 个 H3 输入快照的引用均属于恢复项目。原项目仍保留，旧的失败恢复测试项目未被迁移或删除。

## 4. 调用链

```text
React SettingsWorkspace / ProjectWorkspace
  → Tauri Commands
  → SettingsService / ProjectBackupService
  → ComfyRuntime + ComfyAdapterHandle
  → ComfyHttpAdapter(reqwest::Client)
  → ComfyUI HTTP API
```

生成、恢复、取消、能力探测和状态查询继续通过同一个共享 Adapter Handle；切换地址时替换底层 adapter，失效并刷新 capability cache，不让 Domain 层感知 Endpoint。

## 5. 项目备份格式与恢复安全

备份 manifest 使用：

```json
{
  "format": "ai-studio-project-backup",
  "version": 1,
  "createdBy": "0.2.0",
  "project": { "id": "...", "name": "..." }
}
```

备份包含项目元数据、资产字节与元数据、任务历史、事件、快照、预设和生产队列历史；不包含 `app.db`、`workflow_api.json`、`recipe.yaml` 或模型文件。活动任务被排除并记录排除数量。

恢复会生成新的项目、任务、资产、快照、预设、批次和项目项 ID，并重建关系。非终态任务恢复为 `FAILED` + `RESTORED_INCOMPLETE_TASK`，运行中的生产批次恢复为 `PAUSED`。恢复不会提交 Comfy prompt、上传输入或自动 dispatch。

ZIP 校验拒绝 traversal、绝对路径、Windows drive path、符号链接、缺少首项 manifest、错误格式、超限 entry、超限总大小和损坏 JSON。资产写入 staging 后按 SHA-256 与大小校验，任一必需资产失败返回 `BACKUP_ASSET_HASH_MISMATCH`，不进行 partial restore。

## 6. 自动化回归

| 检查 | 结果 |
| --- | --- |
| Rust 测试 | `273 passed; 0 failed` |
| 前端测试文件 | `23 passed` |
| 前端测试 | `58 passed; 0 failed` |
| `cargo fmt --all -- --check` | PASS |
| `cargo check` | PASS |
| `cargo test -- --test-threads=1` | PASS |
| `pnpm test` | PASS |
| `pnpm build` | PASS |
| `git diff --check` | PASS |
| `pnpm tauri build` | PASS |

开发构建产物已生成，但没有发布：

- `src-tauri/target/release/ai-studio.exe`
- `src-tauri/target/release/bundle/nsis/AI Studio_0.2.0_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/AI Studio_0.2.0_x64_en-US.msi`

## 7. 真实 ComfyUI 检查

本机真实 Endpoint：`http://127.0.0.1:8188`

| 项目 | 实际值 |
| --- | --- |
| Endpoint | `http://127.0.0.1:8188` |
| ComfyUI 版本 | `0.30.2` |
| 设备数 | `1` |
| GPU | `cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync` |
| VRAM 总量 | `17,102,864,384` bytes |
| VRAM 空闲 | `2,102,627,902` bytes |
| 节点数量 | `4,485` |

本轮重新构建并启动 0.2.0 release executable，日志确认数据库、001–007 migration、运行时工作流库、启动恢复和 ComfyUI capability 初始化成功。真实 UI 已完成 `http://localhost:8188` 测试、保存应用、重启持久化、运行中切换拒绝、切换后 Kera2 生成，并最终恢复到 `http://127.0.0.1:8188`；真实 `system_stats` 与 `object_info` 请求持续成功，节点数为 4,485。

Endpoint 的非法 scheme、凭据、query、fragment、规范化、设置损坏回退、JSON 保存回读和共享 Adapter A→B 切换仍有自动化覆盖。原生项目导出/恢复、恢复媒体预览/播放、历史任务输入复用和恢复项目再生成均通过真实 release executable 完成。

## 8. 发布与 Migration 保护

- `v0.1.0` Tag 仍指向正式 commit `c3cbbaa6ece05939bc93c75c906f409e3bacea24`。
- `src-tauri/migrations/` 无本轮改动。
- 未创建 `v0.2.0` Tag。
- 未创建 GitHub Release。
- 未修改 v0.1.0 Release notes 或已发布安装包。

## 9. Final Live Gate

| Gate | 结果 | 证据/边界 |
| --- | --- | --- |
| Endpoint localhost test | PASS | 真实 ComfyUI HTTP `/system_stats` 与 `/object_info` 成功；版本/GPU/VRAM/节点数已记录 |
| Endpoint save/apply | PASS | 设置页保存并应用 `http://localhost:8188` 成功 |
| Endpoint restart persistence | PASS | 完全退出并重启后仍保留 `http://localhost:8188` |
| Capability refresh | PASS | Endpoint 切换后 ComfyUI 保持连接，Kera2 与 MiniMax H3 能力可用 |
| Busy change blocked | PASS | 真实 RUNNING Kera2 期间拒绝切换；任务继续成功；生产队列并发保护自动化通过 |
| Kera2 after endpoint switch | PASS | 切换到 localhost 后真实 Kera2 任务完成并生成图片 |
| Return to 127.0.0.1 | PASS | 空闲后测试、保存并应用默认 Endpoint 成功 |
| Raw log privacy | PASS | 真实 persistent log 在损坏 settings 启动后扫描，四类禁止特征均为 0 |
| Project export | PASS | 真实项目导出成功；备份包含 50 个条目，导出时无活动任务排除 |
| Restore preview | PASS | 预览显示 22 图片、4 视频、0 音频、34 历史任务、0 预设、8 生产队列记录 |
| Project restore | PASS | 真实 UI 恢复成功；新项目 ID 与原项目不同，原项目仍存在 |
| Image preview | PASS | 恢复项目中的源图片和生成图片可预览 |
| H3 video playback | PASS | 恢复项目中的 4 个视频输出均可加载并播放 |
| Audio playback | NOT APPLICABLE | 原项目无音频资产 |
| Snapshot asset remap | PASS | 34 个恢复快照审计；旧原项目资产 ID 为 0，恢复项目外引用为 0 |
| Preset restore | NOT APPLICABLE | 原项目无预设资产 |
| Production history | PASS | 恢复项目可查看历史队列；暂停状态未自动 dispatch |
| Historical input reuse | PASS | Kera2 历史任务加载后提示词、种子和步骤恢复；加载本身未创建任务 |
| Generation from restored project | PASS | 恢复项目中手动生成的新 Kera2 任务成功，输出资产归属恢复项目 |

真实桌面操作通过 release executable 完成；数据库核验仅用于确认任务、快照和资产归属，没有绕过 UI 创建任务或修改业务数据。

## 10. 最终 Gate 状态

代码实现、自动化回归、开发版安装包构建、真实 ComfyUI HTTP 检查和全部 Final Live Gate 均通过。

本轮准确状态为：

```text
M2 FOUNDATION PACK 01 = PASS
```

## 11. 后续技术债

1. backup inspection token 保存在内存并有时限，应用退出后不会保留待恢复 inspection。
2. Windows 设置文件替换已使用 `MoveFileExW` 原子替换语义；若目标文件被外部程序锁定，仍会返回保存错误，并保留旧文件，不会覆盖写入半文件。
3. H3 本轮不重新生成，沿用真实恢复媒体播放与快照引用审计；后续若扩展 H3 规格仍需独立 OOM 验收。

本轮到此停止，不进入 M2 Productivity Pack 02、第三模型 Runtime、云同步或自动更新。
