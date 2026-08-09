# M2 Runtime Productization Pack 04

状态：`完成（代码、自动化回归与发行构建通过）`

本阶段继续使用既有 Tauri / React / Rust 架构，没有创建 migration 009，也没有修改 `src-tauri/migrations/008_organization.sql`。

## 完成项

- P04-01 Runtime Health Dashboard：工作流页展示运行包总数、生产就绪、待验证和阻塞诊断。
- P04-02 Workflow Compatibility Detail：保留节点能力检查、输入不兼容和 ComfyUI 离线状态，并在运行包详情中展示。
- P04-03 Missing Dependency Diagnostics：能力问题明确标记来源为 ComfyUI `/object_info`；未声明的模型或文件依赖不做猜测。
- P04-04 Recipe Default Editing：API 工作流输入映射支持编辑默认值、数值边界和多媒体数量边界。
- P04-05 Workflow Preferred Preset：默认预设写入持久 `settings.json`，按项目、工作流版本和配方隔离；用户已编辑草稿时不会被后台加载覆盖。
- P04-06 Runtime Duplicate / Variant：复用既有配方复制草稿和 semver 递增发布流程。
- P04-07 Workflow Quick Test：运行包页面用最低标量默认值创建快速测试任务，复用正常 `GenerationService`，需要素材的工作流自动转入创作页补充输入。
- P04-08 Runtime Readiness Gate：运行包状态区分 `生产就绪`、`待验证` 和 `已阻塞`，并显示最近一次真实成功验证时间。
- P04-09 Existing Kera2/H3 Regression：未修改 Kera2/H3 workflow runtime、Production Queue 或 seed 约束；既有回归测试保持通过。
- P04-10 0.2.0 Scope Freeze：版本维持 `0.2.0`，本阶段不增加第三模型，不创建 tag 或 GitHub Release。

## Backup v2 阻塞修复

`validate_organization_document()` 现在使用统一标签规范化规则验证：

- 标签名必须非空、无换行、去首尾空白后保持规范形式，长度不超过 32 个字符。
- `normalized_name` 必须等于共享规范化函数的结果。
- 同一项目最多 100 个标签；同一素材最多 20 个标签。
- 拒绝重复规范名称、未知引用、重复链接和跨项目引用。

新增恶意备份测试覆盖空名、换行、超长、非规范空白、规范化不匹配、重复规范名称、101 个项目标签和 21 个素材标签；边界值 100 / 20 通过。

## 调用链

工作流页快速测试 → Tauri `generation_create` → `ProductionQueueService` admission → `GenerationService::start_generation` → recipe/compiler → ComfyUI → Task / Asset。

默认预设调用链为：React → Tauri preset command → `PresetService` 校验归属 → `SettingsService` → 原子 `settings.json`。

## 验证记录

- Rust：294 passed / 0 failed。
- Frontend：26 个测试文件、71 个测试通过。
- `cargo fmt --all -- --check`、`cargo check`、`cargo test -- --test-threads=1`、`pnpm test`、`pnpm build`、`git diff --check`、`pnpm tauri build`：PASS。
- Windows 发行版启动：进程 `ai-studio.exe` 响应正常，窗口标题为 `AI Studio - 本地 AI 创作工作台`。
- 实机 ComfyUI：`http://127.0.0.1:8188` / `0.30.2` / `cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync` / 约 `1.9 GB / 15.9 GB` / `/object_info` 4485 个节点。
- `008_organization.sql` SHA-256：`DB952B13F6D788E23701A29CB229BEAE4C36950AD69A367C4B108A5F2F819B20`。
