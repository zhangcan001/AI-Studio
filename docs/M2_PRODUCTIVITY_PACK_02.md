# M2 PRODUCTIVITY PACK 02 验收记录

状态：**PASS**

本阶段完成资产快速查找、后端分页素材选择器、Asset Quick Use、图片/视频对比工作台、任务历史筛选与历史输入复用。没有进入 Pack 03。

## 范围与架构

- 资产查询统一使用 `asset_library_page(query)`，查询对象包含项目、分类、关键字、媒体类型、来源、排序、游标和页大小。
- 关键字只匹配 `name` 与 `original_name`，使用参数绑定；查询始终按 `project_id` 隔离。
- 资产库和选择器均使用后端 keyset/cursor 分页，不加载全量素材到前端；排序使用 `created_at + id` 稳定游标。
- Quick Use 使用 Zustand 瞬时 intent，不写数据库、设置文件或 `localStorage`；进入创作页后按 Recipe 输入类型匹配，并保留项目校验。
- 对比工作台状态为瞬时 UI 状态，支持 2–4 张同类型图片或视频；音频和混合媒体被阻止，视频提供全部播放、全部暂停、回到开头。
- 任务历史查询沿用单一后端查询接口，支持状态、工作流、时间范围和任务 ID/工作流名称搜索；工作流选项来自当前项目历史。
- 历史复用只加载输入快照并显示来源提示，不自动生成；正常生成安全校验链保持不变。

数据库 migration 目录无变化；Project Backup、Snapshot Asset Remap、Production Queue、Workflow Compiler、Kera2 Runtime、MiniMax H3 Runtime 与动态 ComfyUI Endpoint 语义未改动。

## 自动化能力验收

### Asset Discovery

| 项目 | 结果 |
| --- | --- |
| Keyword Search | PASS |
| Category | PASS |
| Media Type | PASS |
| Source / Generated | PASS |
| Newest / Oldest | PASS |
| Keyset Pagination | PASS |
| Project Isolation | PASS |
| 中文素材名与空关键字语义 | PASS |
| 1000 条合成素材元数据性能查询 | PASS |

后端测试覆盖关键字、分类、媒体类型、来源组合、两种排序方向、跨页游标稳定性、项目隔离以及 1,000 条合成元数据的 30 条分页查询。

### Picker

| 项目 | 结果 |
| --- | --- |
| 搜索与后端分页查询 | PASS |
| 类型限制 | PASS |
| 单选/多选 | PASS |
| 选择顺序与 `maxItems` | PASS |
| 搜索变化后保留已选项 | PASS |
| Cancel / Esc / backdrop 不提交 | PASS |
| 项目切换清理瞬时选择 | PASS |

### Quick Use / Compare / History

| 项目 | 结果 |
| --- | --- |
| 一个兼容输入自动填充 | PASS |
| 多个兼容输入要求选择用途 | PASS |
| 无兼容输入不修改 Draft | PASS |
| 单输入替换确认、多输入追加、数量上限 | PASS |
| 项目不一致丢弃 intent | PASS |
| 图片对比 2–4 项 | PASS |
| 视频对比 2–4 项与组控制 | PASS |
| 音频/混合媒体阻止 | PASS |
| 任务状态/工作流/日期/搜索/分页 | PASS |
| 历史输入回填、seed 语义、项目资产校验 | PASS |
| 缺失工作流/缺失资产安全提示 | PASS |
| Quick Use / History 自动生成 | **NO** |

## Desktop Live Gate

Live 操作在当前发布版桌面程序、当前项目和本机 ComfyUI 上完成。实机数据包含源图片、生成图片和生成视频。

| Gate | 实际结果 |
| --- | --- |
| A 资产搜索、来源/类型/排序、清除筛选 | PASS；当前项目在本次验收时少于 30 条，暂无可加载的第二页；keyset 分页已由自动化与合成 1,000 条数据验证 |
| B MiniMax H3 图片选择器搜索、选择、取消/确认 | PASS；未生成 H3 |
| C 资产库“用于创作” | PASS；切换到 Studio 并填入兼容图片输入，任务数量未增加 |
| D 图片对比 | PASS；2 张图片、尺寸/文件大小/创建时间、关闭与清空均可用 |
| D 视频对比 | PASS；3 个视频播放器、全部播放/暂停/回到开头均可用 |
| E Kera2 历史加载 | PASS；Prompt、Seed、采样步数等输入回填，加载阶段未创建新任务 |
| E Kera2 手动再生成 | PASS；用户点击“开始生成”后任务成功，资产库从 27 条增至 28 条 |
| H3 历史回填 | PASS；参考图片恢复为当前项目资产，未点击生成 |
| Endpoint | PASS；`http://127.0.0.1:8188`，ComfyUI 0.30.2，GPU/VRAM 信息正常显示 |

## Regression

| 检查 | 结果 |
| --- | --- |
| Rust 单元测试 | 276 passed / 0 failed |
| Frontend 测试文件 | 25 passed |
| Frontend 测试 | 67 passed |
| `cargo fmt --all -- --check` | PASS |
| `cargo check` | PASS |
| `cargo test -- --test-threads=1` | PASS |
| `pnpm test` | PASS |
| `pnpm build` | PASS |
| `git diff --check` | PASS |
| `pnpm tauri build` | PASS |

发布构建同时生成 Windows release executable、MSI 和 NSIS 安装包。

## 架构与约束复核

- DB migration：MUST BE NO CHANGE；本轮无 migration 修改。
- Project Backup format：MUST BE NO CHANGE。
- Model-specific branch in Asset Library：MUST BE NO。
- React 直接访问 ComfyUI / SQL / storage path：MUST BE NO。
- 所有普通新增 UI 文案：简体中文；品牌、协议、格式和技术数据按既有 allowlist 保留。

## 下一阶段

本阶段完成后停止。后续如启动新工作，应另行授权 M2 Organization Pack 03（收藏、标签、项目模板及必要的持久化模型），本轮不执行。
