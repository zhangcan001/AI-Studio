# DEV-059 — External Production Package V1

状态：实现中 / 目标为 AI Studio 0.7.0 的批量视频生产入口。

## 目标

Production Package 是外部智能体交给 AI Studio 的文件格式。它描述已经准备好的图片、视频 Prompt 和生成参数；AI Studio 只负责读取、校验、预览，并在用户明确确认后创建现有 `ProductionBatch`。它不会创建 Series/Episode/Scene/Shot，也不会调用 LLM、Provider 或 ComfyUI。

正式链路保持不变：

```text
Production Package → Inspect/Preview → 用户确认 → ProductionBatch
  → ProductionQueue → 用户手动 Start → GenerationService → WorkflowCompiler → ComfyUI/H3
```

禁止第二队列、第二执行器、第二生成服务、直接请求 Comfy `/prompt` 和自动 Start。

## V1 契约

- 根文件名为 `production-package.json`，根目录由用户选择。
- `schemaVersion` 必须为 `1`，`packageType` 必须为 `AI_STUDIO_VIDEO_PRODUCTION`。
- `packageId` 和 item `id` 都是外部标签，不是数据库 ID；item ID 在包内必须唯一。
- 每项必须有 `id`、`name`、非空 `videoPrompt`。
- 图片、音频和视频路径必须是 package root 相对路径；拒绝绝对路径、UNC、`..`、URL、symlink/junction 越界。
- `videoPrompt` 完整保留，按 UTF-8 字节计不超过 64 KiB，不静默截断；预览最多 300 个字符。
- 未知字段只产生 warning；`workflowVersionId`、`recipeId`、`taskId`、`batchId`、`assetId`、`comfyPromptId`、`selectedVideoAssetId` 永远不执行，并产生 `PACKAGE_EXECUTION_FIELD_IGNORED`。
- 包最多 500 项；下游当前 batch 上限为 100 项时按输入顺序分块创建，绝不自动启动。

## 用户流程

1. 选择包含 `production-package.json` 的目录。
2. Inspector 重新解析 JSON，并检查每个媒体文件的存在性、普通文件属性、格式、尺寸、大小和 SHA-256。
3. Preview 展示 READY/WARNING/BLOCKED 统计和逐项错误。
4. 用户只勾选 READY 项，或明确选择部分可生产项。
5. Commit 只提交短期 `inspectionId` 与选中的外部 item ID；服务端重新检查路径、SHA、Prompt、模式和参数。
6. 检查通过后，通过既有 SourceAssetImport 与 ProductionQueue 创建 READY batch；用户在 Production Queue 中手动 Start。

Inspect 零正式数据库写入。检查后媒体发生变化时返回 `PACKAGE_MEDIA_CHANGED` 并拒绝该项；不会用替换后的文件生成。

## 主要诊断代码

`PACKAGE_JSON_INVALID`、`PACKAGE_SCHEMA_UNSUPPORTED`、`PACKAGE_EMPTY`、`PACKAGE_TOO_LARGE`、`PACKAGE_DUPLICATE_ITEM_ID`、`PACKAGE_PATH_INVALID`、`PACKAGE_MEDIA_MISSING`、`PACKAGE_MEDIA_INVALID`、`PACKAGE_MEDIA_CHANGED`、`PACKAGE_PROMPT_EMPTY`、`PACKAGE_PROMPT_TOO_LARGE`、`PACKAGE_MODE_UNSUPPORTED`、`PACKAGE_RESOLUTION_UNSUPPORTED`、`PACKAGE_DURATION_INVALID`。

## 明确不在 DEV-059

Workspace 文件夹管理、拖拽入口、复杂编辑器和正式 Shot 集成留给 DEV-060 及后续任务。DEV-057/058 的 Script/Draft 数据基础和 Parser 保留，但不参与本生产包主路径。
