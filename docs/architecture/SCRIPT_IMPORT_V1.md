# Script Import V1

状态：DEV-057 Data Foundation 已落地；DEV-058 Parser 尚未实现。

DEV-057 已冻结并持久化 `ScriptDocument`/`DraftStructureV1` 的数据 contract：Migration 025 使用 `script_sources` 与不可变的 `script_import_drafts`，Backup 升至 15；Manifest 仍为 2 且故意排除 Script/Draft 工作数据。本文的解析管线、Match、Storyboard、Review 和 Promote 仍属于后续 DEV。

Script Import V1 把用户提供的 TXT、Markdown、JSON、小说或剧本转换为可审阅的 `ScriptDocument` 与 `DraftStructure`。它只产生草稿，不直接创建正式 Series、Episode、Scene、Shot、Profile、ReferenceSet、Batch、Task 或 Generation。

## 1. 设计目标

输入：原始故事/剧本文件或文本。

输出：

1. 可追溯的原文来源和诊断。
2. 有序的 Episode → Scene → Shot Draft 候选树。
3. 角色、场景、道具提及及其待确认的 Profile 候选。
4. 可被人工编辑的 Storyboard Draft 建议。

不承诺自动完成小说改编，不承诺识别所有镜头意图，不把低置信度文本当作事实。

## 2. 输入格式 contract

| 格式 | V1 解析范围 | 明确不做 |
| --- | --- | --- |
| TXT | UTF-8 文本、段落、空行、可识别标题、对白和叙述块 | 不依赖文件名猜测完整结构；不声称理解全部剧情 |
| Markdown | 标题层级、列表、段落、代码块、引用块、对话文本 | 不把代码块当作指令；不把 Markdown 样式当作正式生产字段 |
| JSON | 独立 Script Import schema、显式 `schemaVersion`、Episode/Scene/Shot draft 节点 | 不复用 H3 batch JSON、Workflow API JSON 或 Project Manifest JSON |
| 小说/非标准剧本 | 叙述、对白、心理描写、地点/时间变化、动作段落的候选切分 | 不承诺自动完美改编；不自动决定镜头数量或 Profile |

JSON 输入必须先通过 schema validation。未知版本、重复节点 ID、错误类型、非法父子关系和过大的字符串进入 diagnostics，并阻止生成可确认 Draft，除非用户修正或明确忽略非关键字段。

## 3. ScriptDocument

`ScriptDocument` 是导入来源的输入事实，不是正式 Project 内容。

```text
ScriptDocument
  sourceId
  projectId?                 // 选择导入目标后才关联项目
  format                     // TXT | MARKDOWN | JSON
  originalFilename?
  sourceChecksum
  sourceLength
  sourceStorageRef
  schemaVersion?
  parserVersion
  providerMetadata?
  sourceBlocks[]
  diagnostics[]
  importedAt
```

每个 `sourceBlock` 至少保存：稳定 block ID、原文起止 byte/line/character span、短预览、block kind、父级结构提示和解析警告。原始内容只保留一份；不能把整篇原文复制到每个 Draft Shot 或正式 Shot。

`sourceChecksum` 是 reparse 和 provenance 的稳定依据。文件重命名但内容不变时，不应无故产生新的 source identity；内容变化必须产生可比较的新导入 revision。

## 4. 解析管线

```text
读取与编码校验
      ↓
建立 byte/line/character source map
      ↓
按 format 解析 source blocks
      ↓
识别 Episode / Scene / entity / action / dialogue 候选
      ↓
生成 diagnostics 与 DraftPatch
      ↓
写入或返回 DraftStructure revision
```

### TXT

- 先按 UTF-8/BOM 处理，建立原文定位。
- 空行、连续段落、全角/半角标题标记只作为结构线索。
- `INT./EXT.`、地点、时间等常见格式只提高候选置信度，不成为无条件事实。
- 对白保留原文和说话者候选；无法识别说话者时标为 unresolved。

### Markdown

- Heading level 可作为 Episode/Scene 层级候选，但用户可调整。
- 列表项、引用和段落保留原始 source span。
- 代码块、表格和链接原样保存；链接内容不自动执行、不自动下载、不自动当作 Asset。
- 标题层级不完整时生成 warning，不静默补齐正式结构。

### JSON

建议的 Script Import JSON 交换形状：

```json
{
  "schemaVersion": 1,
  "title": "optional",
  "episodes": [
    {
      "sourceId": "episode-1",
      "name": "候选集名",
      "scenes": [
        {
          "sourceId": "scene-1",
          "name": "候选场景名",
          "shots": [{"sourceId": "shot-1", "name": "候选镜头名", "action": "..."}]
        }
      ]
    }
  ]
}
```

这是交换 contract，不是正式数据库模型。输入里的 `sourceId` 只作为来源锚点，不能直接当成正式 Series/Episode/Scene/Shot ID。额外字段可以进入保留 metadata，但不能绕过 schema 校验或产生执行字段。

### 小说

小说没有固定的 Scene heading，V1 采用“候选 + 诊断”策略：

- 叙述段落作为 source block 和 narrative cue。
- 对白提取说话者/文本候选，无法判断时保留原文。
- 心理描写标记为 thought/narration cue，不自动变成演员动作。
- 地点和时间变化生成 Scene boundary suggestion。
- 动作、空间和叙述转折生成 Shot candidate，默认低置信度。

任何“不确定”都必须可见、可定位、可拒绝。

## 5. DraftStructure 输出

```text
DraftStructure
  draftId
  sourceId
  revision
  status
  episodes[]

DraftEpisode / DraftScene / DraftShot
  draftNodeId
  parentDraftNodeId
  ordinal
  name
  description?
  sourceSpans[]
  diagnostics[]
  reviewState
  origin                 // imported | ai | human
  originalSuggestion?
  currentValue
  children[]
```

Draft 节点必须使用 `draftNodeId`，禁止借用正式实体 ID。`ordinal` 是 Draft 内可编辑顺序，不等于正式写入后的最终 ordinal；正式写入前服务端必须重新验证并重新分配合法顺序。

Draft Shot 可以携带 `action`、`dialogue`、`cameraSuggestion`、`lightingSuggestion`、`durationSuggestion`、`imagePromptDraft` 和 `videoPromptDraft`。这些是文本建议，不是 `GenerationValues`、最终 Prompt、workflow、recipe 或 output spec。

## 6. Diagnostics

每条诊断至少包含：

```text
diagnosticId
severity                // info | warning | error | blocker
code
message
sourceSpans[]
draftNodeId?
suggestedFix?
```

必须覆盖：编码/BOM、未知 JSON schema、空节点、重复 source ID、父子层级错误、名称缺失、无法定位对白说话者、场景边界不确定、超过容量、provider 返回非法 JSON、source span 越界。

`error`/`blocker` 只阻止对应 Draft 节点或正式 Confirm，不阻止用户查看其他合法节点。UI 必须能从诊断跳到原文和 Draft 节点，不能只显示一个总错误数字。

## 7. 与 LLM 的关系

默认路径是离线 deterministic parser。未来的 `DraftTextAnalyzer` 可以生成 `DraftPatch`，但输入和输出都要通过同一套 schema、长度、source span 和安全校验。

```text
ScriptDocument/source spans
          ↓
optional DraftTextAnalyzer
          ↓
untrusted DraftPatch
          ↓
validation + human review
```

LLM 不得返回或决定可信的 Profile ID、Asset ID、ReferenceSet、workflow、Batch、Task 或 Comfy 请求；它的文本结果只能标为 `ai`/`suggested`。DEV-056 不接入 OpenAI-compatible API、local LLM 或用户 endpoint，只保留未来 port 的输入输出边界。

## 8. 应用入口状态

DEV-057 不新增 Tauri command，也不接 AppState。当前可由 application service/repository 直接构造和测试；正式用户入口留给 DEV-058/DEV-061。

## 9. 未来应用入口

以下仍是规划名称，具体 command 和 UI 在后续 DEV 冻结：

| 入口 | 语义 | 写入边界 |
| --- | --- | --- |
| `script_import_preview` | 读取并校验 source，返回 blocks/diagnostics/统计 | 不写正式结构；可不持久化 |
| `script_import_draft_create` | 创建 Draft revision | 只写 Script/Draft 存储，不能写 production 表 |
| `script_import_draft_get` | 按 revision/页读取 Draft | 只读 |
| `script_import_draft_patch` | 应用用户 Edit/Merge/Split/Reorder/Accept/Reject | 只改 Draft revision 或产生新 revision |
| `script_import_diff` | 比较两个 source/Draft revision | 只读 |
| `script_import_confirm` | 显式进入正式结构的唯一入口 | 必须由后端单事务完成，不能启动生产 |

`script_import_confirm` 需要目标 Project 和目标结构位置；确认前应返回 promote preview。确认时不得由前端循环调用 `createShot` 代替一个后端事务。

## 10. Draft 与正式层的绝对隔离

解析、LLM、Draft 编辑和 Match 都不得调用：

- `production_structure_*` 的正式创建/更新 command。
- `shot_create`、`commit_shot_bulk_import` 或正式 Prompt 冻结路径。
- Profile/ReferenceSet create/bind。
- Production Batch、Queue、Task、Generation、ComfyUI 或 Review mutation。

只有用户点击“确认写入正式结构”，并且后端完成二次校验后，才允许创建正式 Episode/Scene/Shot。Confirm 成功后只返回正式 ID mapping 和 provenance，不自动绑定、不自动准备、不自动入队、不自动 Start。

## 11. 容量与恢复策略

目标规模为 100 Episodes、1000 Scenes、5000 Draft Shots。解析器和 Draft API 必须：

- 以明确的 source byte/node/depth 上限保护内存，超限返回诊断，不静默截断。
- 支持取消、进度和失败位置。
- Draft 读取按 revision、Episode、Scene、状态和搜索条件分页，返回真实 total/hasMore/cursor。
- 原文和 Draft Tree 使用窗口化/虚拟化；Inspector 只加载当前 node。
- Draft revision 可恢复；断电后不能半写入正式结构。
- 正式生产仍遵守 0.7.0 的现有 500-shot/batch limits；5000 是草稿容量目标，不是一次生产目标。

## 12. 测试与验收

DEV-058 至少应覆盖：

1. TXT、Markdown、JSON 正常样例以及小说混合叙述。
2. UTF-8、BOM、换行、长行、空文档和非标准标题。
3. Markdown code block/list/heading/source span 保真。
4. JSON version、未知字段、重复 ID、非法父子关系和类型错误。
5. 解析结果全部是 Draft，正式表/`shots` 零写入。
6. 诊断能定位到 source span 和 Draft node。
7. 5000 Draft Shot 的分页、筛选、取消、恢复和输入响应。
8. source checksum、revision、reparse diff 和人工编辑保留。
9. provider/LLM 输出非法或超长时 fail-closed，且无生产副作用。
10. 0.7.0 无 Script source 项目打开和生产不受影响。

## 13. DEV-057 持久化与兼容性

Script system 是 optional。Product 仍为 0.7.0；DEV-057 新增 Migration 025，并将 Backup 14 升为 Backup 15，Migration 024 的既有结构不被改写。

- `script_sources` 以 project + checksum + format 去重，原始 UTF-8 文本只保存于 `source_text`；`script_import_drafts` 保存全部 immutable revision、summary、payload、provider/parser metadata 和 previous link。
- Backup 15 restore 保留 Script/Draft；Backup 14/13/12 继续兼容且 Script/Draft 为空。Manifest 2 不包含 `scriptSources`、`scriptDrafts`、`sourceText` 或 `payloadJson`，所以正式 semantic manifest contract 不变。
- 已有 0.7.0 项目无 Script source 仍可打开和生产；旧 Shot 不会反向生成伪 Draft，也不会因没有 Draft 而变成 BLOCKED。
- 不把 Draft 状态映射成 Readiness status，不把 Storyboard prompt 映射成已冻结 Prompt Snapshot。

## Reparse Source Closure

同一 Draft 的 revision chain 允许 `source A → source B → source C`：revision 1、2、3 的 `draft_id` 和 `project_id` 不变，但每个 `DraftRevision` 分别冻结自己的 `source_id`、source checksum 和 payload checksum。每次 reparse 仍创建不可变 revision，`previous_revision_id` 依次连接前一 revision；source 变化不改变 revision number 或 `expected_revision` 的并发门禁。

同一 source、相同语义 payload 及相同 parser/provider metadata 可以 no-op；不同 source 即使 payload 语义完全相同，也必须产生新的 `REPARSED` revision，因为 source provenance 已变化。append 前必须验证新 source 存在、属于当前 project 且通过 source checksum/字节完整性校验，禁止跨项目引用。Backup 15 roundtrip 保留这条 multi-source chain、各 source text/checksum 及对应的 remapped source IDs；Manifest 2 继续排除 Script/Draft 工作数据。
