# DEV-051 — Asset Library 2.0

状态：已实现并通过最终验证
产品版本：0.6.2
目标能力：0.7.0 一致性资产管理基础

基线：`master`，工作区干净，`HEAD == origin/master`，`DEV051_START_SHA = f448be652bfd6f7b4ae55d94409db95bcad70cb0`。

## 范围

DEV-051 在现有素材库之上增加三类项目级资产工作区：

- **素材**：继续复用现有 `AssetLibrary`、`AssetPreview` 和 `AssetPickerDialog`。
- **档案**：管理 Character、Scene、Prop、Style profile、costume variant、revision 和使用情况。
- **参考集**：管理有序的参考素材、primary/required 标记、角色/服装/场景/道具/风格/镜头用途、owner 关系和反向使用情况。

前端只负责编辑和展示；profile、reference set、asset usage 与删除阻塞均通过 Tauri 后端和项目范围查询完成。新工作区保持中文界面、现有 Studio shell、响应式布局和原有素材使用/视频批处理入口。

## 数据与兼容性

| 项目 | DEV-051 契约 |
| --- | --- |
| SQLite migration | 不新增 migration；最高仍为 023 |
| Project Backup | v13；v12 可恢复 |
| Project Manifest | v2；v1 导入时一致性字段默认为空数组 |
| 媒体内容 | 不进入 backup/manifest；不缓存缩略图或运行时结果 |

Backup v13 按外键顺序恢复 profile、costume、revision、reference set/item 以及 shot/scope binding，并保留 ordinal、inheritance mode、active revision、content hash、item role/primary/required 等逻辑字段。格式、版本和一致性关系损坏时拒绝恢复。

Manifest v2 只保存可移植的项目语义数据：`profiles`、`costumeVariants`、`referenceSets`、`referenceSetItems`、`shotProfileBindings`、`shotReferenceSetBindings`、`scopeProfileBindings` 和 `scopeReferenceSetBindings`。v1 文件不含这些字段时按空数组处理。

## 后端边界

`AssetUsageRepository` 提供项目范围的素材、档案和参考集反向查询，覆盖 reference item、anchor、shot reference、selected image/video、keyframe、profile/referenceset binding 以及生产历史。历史 task/review 只产生警告；仍存活的引用关系由后端作为删除阻塞的权威来源。

新增资产命令采用稳定 camelCase DTO，并将枚举编码为字符串。命令覆盖 profile CRUD、costume CRUD、reference set CRUD、anchor 转换以及 asset/profile/reference-set usage 查询。所有输入在服务层再次校验项目归属、类型、owner、primary、ordinal、角色兼容性和最多 20 个 reference item。

## 明确不在本任务

本任务不新增 migration 024，不实现镜头 binding UI、Shot Readiness、Scene Preparation、生产队列入队、ComfyUI 运行时或新的生成通道。DEV-051 只提供后续 DEV-052 使用的一致性资产管理与持久化基础，并保留现有生产、审核、队列和生成逻辑。

## 验证记录

- DEV-050 定向回归：15 passed。
- DEV-049 定向回归：18 passed。
- DEV-051 后端定向测试：24 passed。
- 前端测试：86 个测试文件、299 个测试通过。
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`：通过。
- `cargo check --manifest-path src-tauri/Cargo.toml`：通过。
- `pnpm build`：通过。
- `git diff --check`：通过。

## 多智能体证据

四个子 Agent 按不重叠文件范围并行完成：

- Agent A：Backup v13、Manifest v2、v12/v1 兼容和一致性 roundtrip。
- Agent B：Asset/Profile/ReferenceSet usage 查询与素材删除语义阻塞。
- Agent C：Consistency CRUD、ReferenceSet detail、anchor conversion 和命令测试。
- Agent D：AssetWorkspace、档案/参考集编辑器、usage 面板和前端测试。

所有 Agent 已完成并退出；最终 `ACTIVE_SUBAGENTS = 0`，`MULTI_AGENT_EXECUTION = CONFIRMED`。DEV-051 不修改 ComfyUI 执行链，也不触发生成。

下一任务：**DEV-052 — Scene Production Preparation + ComfyUI Generation Admission**。
