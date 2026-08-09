# M2 ORGANIZATION PACK 03 验收记录

状态：**PASS**

本阶段完成资产收藏、项目级标签、项目模板与 Project Backup v2。没有进入 Pack 04。

## Migration 008

- 新增且仅新增 `008_organization.sql`。
- 新增 `asset_tags`、`asset_tag_links`、`asset_favorites`、`project_templates` 四张表及必要索引。
- Migration 001–007 未修改。
- 真实既有数据库从 007 升级到 008 前后核心数据计数保持不变：Projects 5、Tasks 108、Assets 84、Presets 1、Production Batches 25。
- 真实升级后 migration 8 `organization` 标记成功，四张新表初始为空。
- 临时 fresh database 从 001–008 建库通过，共 16 张业务表；重复执行 migration 通过。

## 资产收藏与标签

| Gate | 结果 |
| --- | --- |
| 收藏 / 取消收藏幂等 | PASS |
| 标签创建、重名归一化、删除事务 | PASS |
| 标签项目隔离与跨项目保护 | PASS |
| 每项目最多 100 个标签 | PASS |
| 每素材最多 20 个标签 | PASS |
| 资产库收藏、标签与组合筛选 | PASS |
| 素材选择器收藏、标签与组合筛选 | PASS |
| 卡片最多展示 3 个标签并支持 `+N` | PASS |
| keyset 分页与项目隔离保持不变 | PASS |
| 1,000 个素材的合成查询 | PASS |

真实桌面验收在项目 `Default Project（恢复）` 中创建 `人物`、`参考图`、`成片` 三个标签；源图片同时收藏并添加 `人物`、`参考图`，生成视频添加 `成片`。收藏筛选、标签筛选、收藏与标签组合筛选均返回正确结果。MiniMax H3 素材选择器用“仅收藏 + 参考图”准确返回并选中该源图片，未执行 H3 推理。

## 项目模板

- 模板是全局数据，保存工作流版本、Recipe 版本和无素材 Draft。
- 模板保存与恢复保留 string、textarea、integer 和 seed 等标量。
- image/images/video/videos/audio/audios 字段全部排除；没有存储素材 ID、绝对路径或 `storage_path`。
- 从模板创建项目后自动切换到 Studio，只载入 Draft，不创建任务、不自动生成。
- 工作流不可用时模板明确标记并阻止创建。

真实桌面结果：

| 模板 | 实际保存 | 素材排除 | 从模板新建项目 | 自动任务 |
| --- | --- | --- | --- | --- |
| Pack 03 Kera2 | Prompt、固定 Seed `20260809`、Steps `12` | PASS | PASS，字段恢复 | 0 |
| Pack 03 H3 | Duration `3`、Prompt、固定 Seed `16004` | 参考图片未写入 | PASS，参考图片为空 | 0 |

当前 Kera2 Recipe 的正式表单只暴露 Prompt、Seed、Steps，因此桌面 Gate 按实际 Recipe 验证；通用标量序列化测试覆盖 integer 字段，不对不存在的 Width/Height 字段制造默认值。

## Project Backup v2

- 导出 manifest 版本为 2。
- 项目级 `assetTags`、`assetTagLinks`、`assetFavorites` 写入 `project.json`。
- 恢复时资产 ID 和标签 ID 全部生成新 ID，关联关系同步重映射。
- 组织数据与项目、任务、资产、预设和生产队列在同一恢复事务中写入；校验或写入失败时整体回滚。
- 固定 v1 fixture 可检查并恢复，组织数据安全默认为空。
- 全局 `project_templates` 明确不进入项目备份。

真实 UI 导出文件：`C:\Users\ADMIN\Desktop\AI-Studio-Pack03-Backup.zip`。

| 实机检查 | 结果 |
| --- | --- |
| Manifest Version | 2 |
| Zip entries | 54 |
| Asset files | 49 |
| Tags | 3 |
| Tag links | 3 |
| Favorites | 1 |
| Global templates in zip | 0 |
| 恢复后的 Tasks / Assets / Production Batches | 36 / 28 / 8 |
| 恢复后的 Tags / Links / Favorites | 3 / 3 / 1 |
| 资产 ID 与原项目重叠 | 0 |
| 标签 ID 与原项目重叠 | 0 |
| 悬空标签关联 / 收藏 | 0 / 0 |
| 全局模板恢复前后 | 2 / 2 |

恢复后自动切换到 `Default Project（恢复）（恢复）`。关闭并重新启动 Release executable 后，活动项目、三个标签、源图片收藏/标签和两个全局项目模板全部保持。

## Kera2 Live Gate

- Endpoint：`http://127.0.0.1:8188`
- ComfyUI：0.30.2
- GPU：`cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync`
- VRAM：约 1.8 GB 空闲 / 15.9 GB 总量（验收时）
- 新项目：`Pack 03 Kera2 Live Project`
- Task：`tsk_08911541-a8c7-4034-b78a-01b5d850face`
- 结果：`SUCCEEDED`，约 22 秒，新建 1 个图片资产

## Regression

| 检查 | 结果 |
| --- | --- |
| Rust 单元测试 | 288 passed / 0 failed |
| Frontend 测试文件 | 26 passed |
| Frontend 测试 | 70 passed |
| `cargo fmt --all -- --check` | PASS |
| `cargo check` | PASS |
| `cargo test -- --test-threads=1` | PASS |
| `pnpm test` | PASS |
| `pnpm build` | PASS |
| `git diff --check` | PASS |
| `pnpm tauri build` | PASS |

## 约束复核

- Migration 001–007 modified：**NO**。
- React 直接访问 SQL、ComfyUI 或本地 `storage_path`：**NO**。
- 收藏、标签和项目模板均沿用 React → Tauri Command → Application Service → Repository 调用链。
- Project Template 保存素材引用：**NO**。
- Project Template 创建任务或自动生成：**NO**。
- 全局 Project Template 进入项目备份：**NO**。

## 下一阶段

本阶段完成后停止。Pack 04 未开始。
