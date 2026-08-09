# AI Studio 0.2.0

## 发布范围

AI Studio 0.2.0 是 M2 Foundation 与 Runtime Productization 的本地 Windows 发行版本，继续面向本地 ComfyUI 提供项目、工作流、任务、资产和生产队列管理。

## 主要更新

- Project Backup v2 组织数据恢复安全校验：资产收藏、项目标签、标签链接和项目模板引用均经过项目边界与数量上限校验。
- 工作流运行包健康概览、兼容性详情、依赖诊断和运行就绪状态。
- 工作流配方默认值编辑、配方复制/变体和当前工作流默认预设。
- 工作流页快速测试复用普通生成服务与任务链，不绕过任务、队列和资产闭环。
- ComfyUI endpoint 持久设置与现有 Kera2 / MiniMax H3 运行时保持兼容。

## 安全与兼容性边界

- 未修改 `src-tauri/migrations/008_organization.sql`。
- 本版本不新增数据库 migration 009。
- 不新增第三模型，不包含 H3 显存策略扩展。
- 模型、文件等未声明依赖不会被客户端猜测；依赖诊断只使用运行包声明和 ComfyUI `/object_info` 可验证信息。
- 已创建 Git tag `v0.2.0` 和 GitHub Release；Windows 安装包与 SHA-256 清单作为 Release 资产提供。

## 构建产物

- `src-tauri/target/release/ai-studio.exe`
- `src-tauri/target/release/bundle/msi/AI Studio_0.2.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/AI Studio_0.2.0_x64-setup.exe`

SHA-256：

- `ai-studio.exe`: `CEE816A343978DB26BFFFA2A1D66D6D28391C9A8FF73DDB4762689FA1161FBAC`
- `AI Studio_0.2.0_x64_en-US.msi`: `E8DB7CA8FD9001DDC4A09C3F4221130FE0BC6D9CB782256E340C448F9064825C`
- `AI Studio_0.2.0_x64-setup.exe`: `C61C532C400D61F106F6E959249B390647E0A325542228C53BAA7EAE8531B130`

完整清单见 `docs/RELEASE_SHA256_0.2.0.txt`。
