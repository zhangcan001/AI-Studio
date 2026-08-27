# Storyboard Draft V1

状态：DEV-056 架构规划，未实现。

Storyboard Draft V1 是 Script Import 产生的“镜头建议层”。它帮助用户把 Episode/Scene/Shot 候选看清楚、改准确、批量确认；它不是生产执行计划，不是最终 Prompt，也不是正式 Shot。

## 1. 角色定位

```text
ScriptDocument
      ↓
DraftStructure
      ↓
StoryboardDraft（每镜建议）
      ↓ 人工编辑 / 匹配 / 确认
Formal Production Structure
      ↓
ResolvedShotContext → Readiness → Preparation → Queue
```

Storyboard Draft 可以提出“应该拍什么”的建议，但不能决定“现在生成什么”。任何生成、候选选择、审核、加入队列和 Queue Start 仍然是 0.7.0 的 Manual Gate。

## 2. Draft Shot contract

建议的 Draft Shot 结构如下。字段名是规划 contract，实际 DTO 在 DEV-060 冻结。

```text
StoryboardDraftShot
  draftNodeId
  parentSceneDraftId
  ordinal
  name
  purpose?
  characterMentions[]
  sceneMention?
  propMentions[]
  action?
  dialogue[]
  cameraSuggestion?
  lightingSuggestion?
  durationSuggestion?
  imagePromptDraft?
  videoPromptDraft?
  sourceSpans[]
  entityMatches[]
  reviewState
  origin
  originalSuggestion?
  currentValue
  diagnostics[]
```

字段语义：

| 字段 | 内容 | 约束 |
| --- | --- | --- |
| `name` | 镜头候选名称 | 可人工编辑；不是正式 Shot name 的静默覆盖 |
| `purpose` | 镜头在叙事中的目的 | 建议文本，允许为空/低置信度 |
| `characterMentions` | 角色名称、代词、source span | 只产生匹配候选，不产生 binding |
| `sceneMention` | 地点、空间、时间和 source span | 只产生 SceneProfile 候选 |
| `propMentions` | 道具提及、材质/作用线索 | 只产生 PropProfile 候选 |
| `action` | 动作和状态变化 | 文本建议 |
| `dialogue` | 对白、说话者候选、原文定位 | 无法确认说话者时保持 unresolved |
| `cameraSuggestion` | 景别、角度、运动、构图 | 不写 workflow 或 runtime input |
| `lightingSuggestion` | 光线、色调、气氛 | 不替换 StyleProfile |
| `durationSuggestion` | 建议时长或范围 | 只在正式输出规格校验后才可使用 |
| `imagePromptDraft` | 基础画面 Prompt 草稿 | 不是最终 Prompt，不调用生成 |
| `videoPromptDraft` | 基础视频 Prompt 草稿 | 不是 H3 最终输入，不调用生成 |
| `sourceSpans` | 原文 byte/line/character 区间 | 必须可回跳原文 |
| `entityMatches` | Profile 候选和状态 | 必须人工确认 |

## 3. 禁止进入 Draft 的字段

Storyboard Draft 不得包含或伪造以下正式生产事实：

- 正式 Shot、Scene、Episode、Series ID。
- 已确认的 Character/Scene/Prop/Style binding。
- Asset ID、ReferenceSet ID、参考图片顺序或 checksum。
- Workflow、WorkflowVersion、Recipe、OutputSpec 的正式版本。
- ProductionBatch、ProductionItem、Task、GenerationSnapshot 或审核结果。
- ComfyUI 节点、队列状态、生成结果路径或运行能力状态。

如果 UI 需要显示某个正式对象的候选，使用 `candidateId`/`selectedCandidateId` 和 `confirmed`，不要把候选 ID 当成已绑定事实。

## 4. 节点状态与操作

节点状态建议：

```text
AI_SUGGESTED → PENDING_REVIEW
                     ├─ ACCEPTED
                     ├─ EDITED
                     ├─ REJECTED
                     ├─ CONFLICT
                     └─ UNRESOLVED
```

- `AI_SUGGESTED`：解析器或未来 analyzer 给出的原始建议。
- `PENDING_REVIEW`：等待人工决定。
- `ACCEPTED`：用户接受 Draft 建议，不代表正式写入。
- `EDITED`：用户修改后的 Draft 值；保留原始建议。
- `REJECTED`：用户明确不采用；默认不进入 promote preview。
- `CONFLICT`：节点、source、目标位置或匹配存在冲突。
- `UNRESOLVED`：缺少必需的人工决定或 source 无法解释。

以下操作只能改变 Draft revision：

| 操作 | 行为 | 额外约束 |
| --- | --- | --- |
| Accept | 接受当前 AI 建议 | 不写正式层，不绑定 Profile |
| Reject | 标记不采用 | 可撤销；不删除原文来源 |
| Edit | 修改字段 | 保留 originalSuggestion 和 human provenance |
| Merge | 合并同级/相邻 Draft 节点 | 保留全部 source spans 和被合并节点记录 |
| Split | 将节点拆分为多个候选 | 新节点继承 source span 子区间并标记来源 |
| Reorder | 修改同级 ordinal | 只改 Draft ordinal，不承诺正式 ordinal |
| Batch Accept/Reject | 批量改变状态 | 全量校验，不静默跳过冲突 |

Draft 操作应可撤销，或通过新 revision 保存前后状态；禁止把操作实现成直接调用正式 `createShot` 循环。

## 5. Profile Match 交互

Draft entity match 至少显示：

```text
mention
entityType             // character | scene | prop
normalizedMention
status                 // EXACT | LIKELY | NO_MATCH | AMBIGUOUS
candidateProfiles[]
evidence[]
selectedProfileId?
confirmed
```

用户对每个 match 有三种明确选择：

1. 关联已有 Profile。
2. 建议创建新 Profile（只生成后续动作，不自动创建）。
3. 忽略/保持未绑定。

规则：

- `EXACT` 也必须允许用户改选或忽略；名称命中不等于用户授权。
- `LIKELY` 只能预选，不能自动绑定。
- `NO_MATCH` 只能显示创建建议或保持未绑定。
- `AMBIGUOUS` 必须选择、编辑或保持未绑定；不能挑一个“最像的”静默提交。
- 候选必须按 project 和 ProfileType 隔离；同名不同类型不能互相命中。
- ReferenceSet、Asset、Costume 和参考顺序只能在 Profile 确认后由现有一致性 UI 处理。

## 6. Prompt Draft 与最终 Prompt

`imagePromptDraft` 和 `videoPromptDraft` 只表达 Script/Storyboard 层的基础建议，例如动作、镜头、光线和对白上下文。它们不能取代现有 `ResolvedShotContext`。

正式生产的 Prompt 仍由：

```text
Profile + Scene + Costume + Prop + Style
      + Shot Action + Camera + Lighting + Output Spec
      → existing PromptContextBuilder
      → stage-specific ResolvedShotContext
```

当 Draft 晋级为正式 Shot 后，用户可以把 prompt draft 作为可编辑输入；系统必须重新解析正式 bindings、stage、workflow、recipe 和 output spec，再由现有 Builder 生成最终生产上下文。Draft 文本本身不得直接变成 Queue `GenerationValues`。

## 7. Script Import Workspace

入口放在现有 Creation workspace 下的“脚本导入”子模式，不新增全局生产入口。

### 左栏：原文结构

- 显示文件名、格式、source checksum、标题/段落/章节和 source blocks。
- 显示行号/字符范围、解析 warning/error、低置信度和未引用原文。
- 点击 source span 可定位中栏 Draft 节点；中栏选择也能反向高亮原文。
- 原文面板不提供正式结构编辑、Profile binding、Queue 或生成按钮。

### 中栏：Episode / Scene / Shot Draft Tree

- 顶部固定显示 `DRAFT｜尚未写入正式结构`。
- 节点显示类型、ordinal、状态、置信度、冲突、source 引用和匹配摘要。
- 支持按状态、置信度、未关联 source、冲突和层级筛选。
- 提供 Accept、Reject、Edit、Merge、Split、Reorder 和批量操作。
- 仅有一个高风险主动作：`确认写入正式结构`，并且只在全部必要校验通过后启用。

### 右栏：Draft Inspector

- 当前节点的 AI 原始建议与用户当前值并排展示。
- 显示 source spans、diagnostics、Profile candidates、match evidence 和人工确认状态。
- 显示本次 promote 将创建/引用的数量、目标 Project、未解决冲突和差异。
- 保存前后保护未保存编辑；关闭/切换时提示 Draft revision 仍未提交。

## 8. 一次性写入正式结构

```text
Draft revision
   ↓ 全量校验 + promote preview
用户点击“确认写入正式结构”
   ↓ 后端单事务
Series / Episode / Scene / Shot / assignment
   ↓ 返回 draftNodeId → formalId
不自动 Profile binding、不自动 Readiness、不自动 Queue Start
```

Confirm 的要求：

1. 用户明确选择目标 Project 和已有/新建结构位置。
2. 后端重新读取 Draft revision，不信任前端数量、ordinal 或 ID。
3. 校验重复名称、空节点、父子关系、目标冲突、跨项目 ID 和已存在正式 Shot。
4. 默认 create-only；已存在正式 Shot 不自动覆盖。
5. 一个事务完成正式层写入和 mapping；任一错误整体回滚。
6. `draftId + revision + projectId` 作为幂等语义，重复提交不重复创建。
7. 成功后只返回正式 IDs、provenance 和结果摘要；不创建 Batch、Task、Snapshot，不启动 Queue。

## 9. 大数据量交互

目标为 100 Episodes、1000 Scenes、5000 Draft Shots：

- Draft Tree 使用扁平可见节点 virtualization，或按 Episode/Scene 懒加载并分页。
- 原文视图按 source block/viewport 窗口化，不把整篇小说一次渲染到 DOM。
- Inspector 只加载当前 Draft node 的完整字段。
- 搜索/筛选由后端或一次性建立的 Draft session index 支持，返回真实 `total`/`hasMore`/cursor。
- 切页、合并、拆分、重排后保持选择、键盘焦点和 source 定位。
- 5000 是 Draft 浏览目标，不是一次正式 Production batch；正式生产继续使用 0.7.0 caps。
- 任何超限必须显示可操作的诊断，不得静默丢弃节点。

## 10. 无障碍、中文与响应式门禁

- Draft Tree 具有 `aria-level`、`aria-selected`、`aria-expanded`、键盘上下移动和可见焦点。
- Merge/Split/Reorder 不只依赖拖拽；必须提供按钮/菜单替代。
- 错误靠近字段显示，并可聚焦第一个错误；状态不能只靠颜色。
- 新术语统一使用“草稿、AI 建议、人工确认、正式结构、未绑定”。
- 1280px 保持三栏；1024px 将左栏抽屉化/Inspector 可折叠；768px 以下用标签/抽屉；375px 单栏且无横向滚动。
- 所有主要操作保持足够触控命中区域；确认区在窄屏固定但不遮挡 Draft 内容。
- 新 UI 纳入现有中文文案和 UI localization tests。

## 11. 验收门禁

### Draft 安全

- AI 结果始终显示 DRAFT；解析和 Draft 操作不会产生正式 ID。
- Import、Match、Storyboard 和 Prompt draft 不触发 Profile create/bind、Batch、Task、Queue 或 Comfy。
- Accept/Reject/Edit/Merge/Split/Reorder 全部可追溯、可撤销或版本化。

### Confirm

- Confirm 前正式结构表和 `shots` 行数不变。
- Confirm 前显示完整数量、diff、目标和未解决项。
- 冲突/必填错误时主按钮禁用。
- Confirm 后一次事务生成正式结构和 mapping；失败全回滚，重复提交幂等。
- 成功后不自动绑定、不自动预检、不自动入队、不自动生成。

### 内容与规模

- TXT、Markdown、JSON、小说混合叙述都能生成 source span 和可编辑 Draft。
- Shot Draft 字段完整，image/video prompt 明确标为草稿。
- 5000 Draft Shot 支持分页/虚拟化、搜索、筛选、焦点保持和取消。
- 0.7.0 无 Script 的项目打开、生产和旧数据读取不受影响。
