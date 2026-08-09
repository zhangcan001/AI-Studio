# AI Studio M2 FOUNDATION PACK 01

日期：2026-08-09  
开发版本：`0.2.0`  
正式版本：`v0.1.0`（已发布，保持不可变）  
正式版本基线：`c3cbbaa6ece05939bc93c75c906f409e3bacea24`

本阶段只完成 M2 Foundation Pack 01。没有创建 `v0.2.0` Tag 或 GitHub Release，也没有修改数据库 migration、v0.1.0 Tag、Release 或安装包。

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

缺少设置文件时使用默认地址；JSON 损坏或 schema 不支持时使用默认设置，并显示：`设置文件无法读取，当前已使用默认配置。`。读取失败不会自动覆盖原文件。保存流程为临时文件写入、`sync_all`、替换目标文件，并尽力同步父目录。

Endpoint 变更必须先成功请求 `/system_stats` 和 `/object_info`，失败地址不会保存。存在活动任务或生产队列占用时，后端返回 `COMFY_ENDPOINT_CHANGE_BUSY`，不会只依赖前端按钮禁用。

## 3. 调用链

```text
React SettingsWorkspace / ProjectWorkspace
  → Tauri Commands
  → SettingsService / ProjectBackupService
  → ComfyRuntime + ComfyAdapterHandle
  → ComfyHttpAdapter(reqwest::Client)
  → ComfyUI HTTP API
```

生成、恢复、取消、能力探测和状态查询继续通过同一个共享 Adapter Handle；切换地址时替换底层 adapter，失效并刷新 capability cache，不让 Domain 层感知 Endpoint。

## 4. 项目备份格式与恢复安全

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

## 5. 自动化回归

| 检查 | 结果 |
| --- | --- |
| Rust 测试 | `258 passed; 0 failed` |
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

## 6. 真实 ComfyUI 检查

本机真实 Endpoint：`http://127.0.0.1:8188`

| 项目 | 实际值 |
| --- | --- |
| Endpoint | `http://127.0.0.1:8188` |
| ComfyUI 版本 | `0.30.2` |
| 设备数 | `1` |
| GPU | `cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync` |
| VRAM 总量 | `17,102,864,384` bytes |
| VRAM 空闲 | `2,101,927,486` bytes |
| 节点数量 | `4,485` |

正式版可执行文件已启动，日志确认数据库、运行时工作流库、启动恢复和 ComfyUI capability 初始化成功，并使用默认 Endpoint。Windows Computer Use 在读取 Tauri WebView accessibility/screenshot 状态时连续返回 `node_repl exec context not found`，因此本轮没有盲操作原生文件对话框；设置页和项目页的行为由 Tauri/Rust 与前端回归覆盖，原生对话框 Smoke 仍需在可用的桌面自动化环境中手动复核。

Endpoint 的非法 scheme、凭据、query、fragment、规范化、设置损坏回退、JSON 保存回读和共享 Adapter A→B 切换已有自动化覆盖。真实 `http://localhost:8188` 的 UI 测试/保存/重启流程以及切换后的 Kera2 生成未在本轮桌面辅助接口失效后重新执行；既有 Kera2/H3 实机证据保持不变，不伪造为本轮重新验收结果。

## 7. 发布与 Migration 保护

- `v0.1.0` Tag 仍指向正式 commit `c3cbbaa6ece05939bc93c75c906f409e3bacea24`。
- `src-tauri/migrations/` 无本轮改动。
- 未创建 `v0.2.0` Tag。
- 未创建 GitHub Release。
- 未修改 v0.1.0 Release notes 或已发布安装包。

## 8. 最终 Gate 状态

代码实现、自动化回归、开发版安装包构建和真实 ComfyUI HTTP 检查均通过；但本轮 Windows Computer Use 无法读取 Tauri WebView，导致真实 `localhost` 保存/重启、切换后 Kera2、真实项目备份恢复及媒体/历史复用 Gate 没有完成。

因此本轮准确状态为：

```text
M2 FOUNDATION PACK 01 = CODE PASS / LIVE GATE PARTIAL
```

待桌面辅助接口恢复后，完成上述真实 Gate 才能将最终状态升级为 `M2 FOUNDATION PACK 01 = PASS`。

## 9. 后续技术债

1. 在桌面自动化接口可用后补做 `localhost` Endpoint 保存/重启持久化、Endpoint 切换后的 Kera2、当前真实项目备份 roundtrip、恢复媒体预览/播放和恢复历史输入复用 Gate。
2. 目前 backup inspection token 保存在内存并有时限，应用退出后不会保留待恢复 inspection。
3. Windows 设置文件替换包含兼容性 fallback；若目标文件被外部程序锁定，仍会返回保存错误，不会覆盖写入半文件。
4. H3 本轮不重新消耗 GPU 资源，沿用既有实机证据与共享 Adapter 自动化覆盖。

本轮到此停止，不进入 M2 Productivity Pack 02、第三模型 Runtime、云同步或自动更新。
