# DEV-032 Runtime Profiles + Preflight Center

日期：2026-08-17。范围是 ComfyUI 环境档案、安全切换和当前环境只读预检；不新增 SQLite 表、migration、Backup 字段或第二 Runtime。

## 1. Baseline

- `DEV032_START_SHA`: `8a2b0729feaf36928f33b32806f774fc7f41a6d8`
- 分支：`master`；开发前工作区干净且与 `origin/master` 一致
- Version：`0.5.0`
- Migration：`019`
- `BACKUP_VERSION`：`10`

## 2. Existing Runtime Architecture

复用既有 `SettingsService`、`SettingsStore`、单一 `ComfyRuntime`、`ComfyService`、`ComfyAdapterFactory`、`DiagnosticsService`、Production Queue admission gate 和 workflow capability/readiness infrastructure。Profile apply 唯一通过 `SettingsService::save_and_apply(endpoint)`。

## 3. Naming distinction

新增 `ComfyEnvironmentProfile`，只表示保存的环境快捷方式；`RuntimeParameterProfile` 仍表示 workflow/recipe 生成参数，语义和数据结构未改变。

## 4. Environment Profile storage

`AppSettings.comfy_environment_profiles` 使用 `#[serde(default)]`，schema 仍为 `1`，最多 20 条。只持久化 id、name、endpoint、created/updated 时间，不持久化 GPU、VRAM、节点数或 Comfy/Python 版本。名称大小写不敏感去重，endpoint 复用 `ComfyConnectionConfig::from_endpoint` 并做规范化去重。

## 5. Safe Apply

环境档案支持 list/save/update/delete/apply。Apply 继续执行 candidate test → admission gate → activity check → held-gate health check → settings persistence → shared runtime replacement → capability invalidation。错误 endpoint 不会先写入 settings；删除当前 profile 也不会改变当前 endpoint。

## 6. Preflight

新增只读 `ComfyPreflightService` 和 `comfy_preflight_current` command，输出 READY/WARNING/BLOCKED、endpoint、连接状态、ComfyUI/Python、GPU、VRAM、节点数、活动任务、生产忙碌状态、workflow ready/blocked 汇总及结构化 issues/missing nodes。不会切换 endpoint、启动 generation、安装节点或下载模型。

## 7. Workflow capability reuse

预检复用 `ComfyService` 的 `/system_stats`、`/object_info` 和既有 `WorkflowLifecycleService` workspace capability/readiness 数据；禁用 workflow 只输出 INFO，不计入全局 blocked。无连接、object_info 不可用、无可生产 workflow 或全部生产 workflow blocked 时为 BLOCKED；部分 workflow 缺节点且仍有可用 workflow 时为 WARNING。

## 8. UI

`SettingsWorkspace` 的“ComfyUI 运行环境”区域新增已保存环境卡片、当前环境 badge、测试/应用/编辑/删除和简单保存表单；新增“运行预检”卡片，展示状态、Comfy/Python/GPU/VRAM/nodes、生产状态、workflow 汇总和问题建议。未新增第二 Settings 页面。

## 9. Backward compatibility

旧 settings JSON 没有 `comfyEnvironmentProfiles` 时加载为 `[]`；restart 后 profiles 保留。`preferred_presets`、`runtime_profiles`、`production_queue_name_presets` 在 endpoint apply 后均保持不变。Settings 不进入 Project Backup。

## 10. No-GPU E2E

- Settings profile focused：CRUD、验证、重复、apply preservation、busy gate、RuntimeParameterProfile 保持通过。
- Preflight focused：READY、WARNING/missing node、BLOCKED/offline、disabled workflow INFO 通过。
- Live harness 不提交 `/prompt`、不生成图片/视频，仅执行 profile、connection、preflight 基础 capability、safe apply、restart persistence。

## 11. Live 8188 / 18188

复用验证 runtime `D:\ComfyUI-WorkFisher-V2`（仅验证报告使用，未进入产品代码）。

- `http://127.0.0.1:8188`：Test PASS；ComfyUI `0.33.0`；Python `3.12.10`；GPU `cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync`；节点 `4516`；基础 preflight capability PASS。
- `http://127.0.0.1:18188`：无 listener；Test 返回 `COMFY_OFFLINE`；Apply 返回 `COMFY_OFFLINE`；失败后当前 endpoint 仍为 `8188`。
- Known Good Apply：PASS；settings restart 后两个 profile 仍存在；generation：`NOT_RUN`。

## 12. Regression

- Rust：`502 passed / 0 failed / 1 ignored`
- Frontend：`53 files / 173 tests passed`
- `pnpm build`：PASS
- `cargo fmt --all -- --check`：PASS（在 `src-tauri` crate 目录执行）
- `cargo check --manifest-path src-tauri/Cargo.toml`：PASS
- `git diff --check`：PASS

## 13. Architecture Gate

通过：无第二 `ComfyRuntime`、无第二 `SettingsService`、无重复 capability engine、无重复 workflow validator、无新 Queue/Executor、无 direct `/prompt`。Migration 仍为 019，Backup 仍为 10。

## 14. Final Decision

DEV-032 环境档案 CRUD、旧 settings 兼容、安全切换、busy gate、只读预检、workflow readiness、前端 SettingsWorkspace、8188 good gate 和 18188 offline gate 全部通过。

**FULL PASS**

下一任务：`DEV-033 — 100–500 Shot Production Performance`
