# M1 简体中文界面产品化验收

日期：2026-08-09

## 验收范围

本阶段只处理现有 M1 的用户可见中文展示，不增加业务能力，不改变 Tauri 2 + React + Rust 技术路线，不修改数据库 migration，不改变 Rust/前端协议枚举，不增加语言切换入口，也不引入重量级 i18n 框架。

已完成一层轻量展示适配：

- `src/i18n/statusLabels.ts`：状态、分类、模式、字段、工作流和时间/文件大小展示。
- `src/i18n/errorMessages.ts`：错误码中文映射；未知错误使用中文通用提示。
- `src/i18n/UiErrorNotice.tsx`：用户提示与折叠技术详情分离。
- `index.html`：`lang="zh-CN"` 和中文窗口标题。

## M1 简体中文界面

| 区域 | 结果 | 证据 |
| --- | --- | --- |
| 主导航 | PASS | 创作、资产库、任务、项目、工作流 |
| 创作工作台 | PASS | 工作流选择、预设、字段、批量任务、生成按钮、Seed 和媒体输入 |
| 资产库 | PASS | 分类筛选、空状态、预览、文件大小、时间和生成资产名称 |
| 任务 | PASS | 任务历史、状态筛选、任务详情、重试、取消、恢复提示 |
| 项目 | PASS | 项目列表、创建/编辑、当前项目、默认项目 |
| 工作流 | PASS | 导入、筛选、版本、能力、诊断、映射向导、校验和发布 |
| 生产队列 | PASS | 运行/暂停/完成、队列项、归档/恢复/删除、跳过/重新排队 |
| ComfyUI 状态 | PASS | 已连接、离线、不兼容、Endpoint、版本、GPU、VRAM、节点数量 |
| 错误提示 | PASS | 已知错误码中文化；未知原文只在“技术详情”折叠区显示 |
| 空状态 | PASS | 无工作流、无任务、无资产、无输出等状态均为中文 |
| ARIA / Tooltip | PASS | 导航、筛选、预览、媒体选择器、任务重试和工作流导入关键控件均有中文可访问名称 |

## 界面语言检查

在真实 Windows release WebView 中检查主页面和五个导航页：

- 普通英文按钮：**0**。
- 普通英文标题：**0**。
- 普通英文提示句：**0**。
- 普通英文错误句：**0**。
- 标准生成资产名 `Generated Image N` / `Generated Video N` 已转换为“生成图片 N”/“生成视频 N”。
- 标准工作流名 `Krea2 T2I Local` 已转换为“Kera2 文生图”。
- 用户创建的项目名、用户文件名、提示词和模型名不做翻译，避免改变用户数据含义。

允许保留的品牌、技术和数据 Token：

`AI Studio`、`ComfyUI`、`Kera2`、`MiniMax H3`、`GPU`、`VRAM`、`FPS`、`API`、`JSON`、`YAML`、`SHA-256`、`ID`、`URL`、`Endpoint`、`T2I`、`I2I`、`H3`、`cuda:0`、`NVIDIA GeForce RTX 5060 Ti`、`PNG`、`JPEG`、`WebP`、`MP4`、`B`、`KB`、`MB`、`GB`、`snake_case`，以及任务/资产/工作流的技术 ID。

以上 Token 属于品牌、格式、协议、设备、单位、模型名或技术标识，不作为普通英文界面文案统计。没有在本文件写入用户提示词、模型路径、工作流 JSON 或本机私有目录。

## 真实 ComfyUI 桌面检查

本次使用 Windows release executable 启动真实桌面窗口，并通过 WebView DOM 检查文本和布局：

| 项目 | 实测值 |
| --- | --- |
| Window title | `AI Studio - 本地 AI 创作工作台` |
| HTML language | `zh-CN` |
| Endpoint | `http://127.0.0.1:8188` |
| Version | `0.30.2` |
| GPU | `NVIDIA GeForce RTX 5060 Ti` |
| VRAM | `1.7 GB 空闲 / 15.9 GB 总量` |
| Node Count | `4485` |
| Desktop state | Connected / 已连接 |

已读取创作、资产库、任务、项目、工作流五个页面。测试窗口视口为 `1180 × 760`，各页 `scrollWidth` 均不超过 `clientWidth`，未发现横向溢出；纵向滚动为内容自然滚动。

ComfyUI 离线的后端状态处理沿用既有 offline adapter/mock gate，并由 `comfyStatusLabel` 映射为“离线”；本次桌面窗口保持真实 ComfyUI 运行，未为验收中断用户正在使用的本地服务。

## 修改文件列表

- `index.html`、`README.md`
- `src/i18n/statusLabels.ts`
- `src/i18n/errorMessages.ts`
- `src/i18n/UiErrorNotice.tsx`
- `src/i18n/localization.test.ts`
- `src/app/App.tsx`、`src/app/App.css`
- `src/features/comfy/ComfyStatus.tsx`
- `src/features/assets/AssetCard.tsx`、`AssetGrid.tsx`、`AssetLibrary.tsx`、`AssetPreview.tsx`
- `src/features/projects/ProjectWorkspace.tsx`
- `src/features/studio/GenerationStudio.tsx`、`DynamicFormRenderer.tsx`、`DynamicFormRenderer.test.ts`、`TaskProgressCard.tsx`、`ImageOutput.tsx`、`VideoOutput.tsx`、`ProductionQueuePanel.tsx`
- `src/features/studio/fields/` 下的文本、整数、Seed、图片、多图片、媒体、多媒体字段组件
- `src/features/tasks/TaskHistory.tsx`、`TaskHistoryDetail.tsx`、`TaskHistoryList.tsx`、`retryPolicy.ts`、`retryPolicy.test.ts`
- `src/features/workflows/WorkflowWorkspace.tsx`
- `docs/M1_ZH_CN_UI_VALIDATION.md`

`src-tauri/` 只执行回归检查，本阶段没有修改 Rust 业务代码、AppState、Commands、错误协议或数据库 migration。

## 架构调用链

既有业务调用链保持不变：

```text
React UI
  -> Tauri Commands
  -> Application Services
  -> Ports / Adapters
  -> Infrastructure
  -> ComfyUI HTTP / WebSocket API
```

本阶段只在 React 展示边界增加：

```text
Rust DTO / protocol enum
  -> React display mapper
  -> 中文标签、中文错误主提示
  -> 折叠技术详情（需要时）
```

没有把 ComfyUI 调用移到 React，也没有让 Commands 直接调用 HTTP 客户端。

## 回归测试

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check` | PASS |
| `cargo test -- --test-threads=1` | PASS，244 passed，0 failed |
| `pnpm test` | PASS，14 test files，40 passed，0 failed |
| `pnpm build` | PASS |
| `pnpm tauri build` | PASS，Windows x64 release executable、MSI、NSIS bundle 均生成 |

新增前端测试覆盖：状态映射、生产状态、资产分类、工作流别名、默认项目别名、日期安全格式化、已知错误码和未知错误原文隔离。

## 技术债

- 当前是单一简体中文展示层，没有语言切换或完整翻译资源管理；这是本阶段的明确范围。
- 新增 Rust 错误码时需要同步补充 `errorMessages.ts` 映射，否则会显示中文通用错误并把原文放入技术详情。
- 用户自定义项目名、文件名、提示词和模型名保留原样；如未来需要统一这些数据的展示，应另行定义数据所有权和翻译策略。
- 浏览器原生音视频控件的系统文案不由 AI Studio 控制。

## 最终状态

**M1 ZH-CN UI = PASS**

本阶段完成后停止，不进入新的功能阶段。
