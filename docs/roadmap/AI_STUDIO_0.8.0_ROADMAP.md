# AI Studio 0.8.0 Roadmap：External Production Package → Batch Video Production

状态：DEV-057 Data Foundation PASS（可选能力）；DEV-058 Script Import Parser PASS（可选能力）；DEV-059 PASS；DEV-060 PASS；DEV-061 NEXT；DEV-062 PLANNED

内部代号：Narrative Preproduction V2
稳定基线：AI Studio 0.7.0 — Narrative Production V1
`DEV056_START_SHA`：`d75e62da62f2987b84037fb53725f79d35f8cea6`
0.7.0 `SOURCE_RC_SHA`：`e4a643d4b31329e291c2fb40002f1554e8a1ab34`
`v0.7.0` peeled SHA：`e4a643d4b31329e291c2fb40002f1554e8a1ab34`

本路线图来自 DEV-056 对实际仓库的四路只读审计。0.7.0 的正式生产能力已经存在；0.8.0 的首要缺口是把原始脚本变成可人工审阅的 Draft，而不是再造一个生成系统。DEV-057 已冻结并实现数据基础；解析器从 DEV-058 开始。

## 2026-08 Product Direction Pivot

AI Studio 0.8.0 的核心定位冻结为 **AI 视频批量生产工作台**：外部智能体负责文本、图片和视频 Prompt，AI Studio 负责读取、校验、预览、导入现有 ProductionQueue，并由用户手动 Start 进入既有 H3/ComfyUI 链路。DEV-058 Script Import Parser 保留为 `OPTIONAL SCRIPT IMPORT CAPABILITY`，不再是生产主路径；不删除、不回滚、不重构该能力。

原 Narrative Preproduction 路线保留为历史记录，但不再是 0.8.0 核心路线：

| 旧路线 | 状态 |
| --- | --- |
| DEV-059 Entity Match | SUPERSEDED BY PRODUCTION PACKAGE ROUTE |
| DEV-060 Storyboard Draft | SUPERSEDED BY PRODUCTION PACKAGE ROUTE |
| DEV-061 Draft Review | SUPERSEDED BY PRODUCTION PACKAGE ROUTE |
| DEV-062 Promote | SUPERSEDED BY PRODUCTION PACKAGE ROUTE |
| DEV-063 Consistency | SUPERSEDED BY PRODUCTION PACKAGE ROUTE |
| DEV-064 Integration | SUPERSEDED BY PRODUCTION PACKAGE ROUTE |
| DEV-065 Release | SUPERSEDED BY PRODUCTION PACKAGE ROUTE |

新的核心路线为：

1. **DEV-059 External Production Package V1** — 外部生产包校验、预览与批量导入（PASS）。
2. **DEV-060 Production Package Workspace** — 文件夹入口、编辑和生产工作区（PASS）。
3. **DEV-061 Bulk Production Hardening** — 批量性能、恢复与审计硬化（NEXT）。
4. **DEV-062 AI Studio 0.8.0 Release Gate** — 发布门禁与兼容性验收（PLANNED）。

## 1. 目标与不变的执行边界

### 目标

让用户可以把 TXT、Markdown、JSON、小说或剧本送入：

```text
ScriptDocument
  → DraftStructure
  → StoryboardDraft + Profile Match
  → 人工编辑/确认
  → 正式 Project / Series / Episode / Scene / Shot
  → ResolvedShotContext
  → Readiness / Preparation
  → 现有 ProductionQueue
  → ComfyUI
```

AI/解析器输出永远是 Draft。只有用户显式点击“确认写入正式结构”，并且后端完成一次事务，才允许产生正式结构身份。

### 不变边界

- ComfyUI 是唯一正式图片/视频生成执行引擎。
- 唯一正式链路仍是 `ProductionQueue → GenerationService → WorkflowCompiler → Comfy Adapter → ComfyUI`。
- 候选选择、人工审核、加入队列和 Queue Start 继续是 Manual Gate。
- 不增加第二执行器、第二队列、Scheduler、自动选片、自动批准、自动 Start 或 unattended generation。
- DEV-057 保持 Product 0.7.0；新增 Migration 025、Backup 15，但 Manifest 仍为 2。Script/Draft 只进入 Backup 15，不进入 Manifest 2。

## 2. 当前仓库审计结论

| 领域 | 0.7.0 已有 | 0.8.0 缺口 |
| --- | --- | --- |
| Prompt | Prompt Library、版本、模板解析、批量应用 | 没有“脚本 → 结构草稿”的输入 contract |
| Import | Workflow JSON、H3 TXT/Markdown/JSON、Shot TSV/JSON、任务列表 JSON | 没有通用 Script Import；现有 Shot import 确认后直接写正式 `shots` |
| Structure | Project → Series → Episode → Scene → Shot、CRUD、排序、assignment | 没有 Draft 层、批量结构晋级和原子 Promote |
| Consistency | Profile、ReferenceSet、Context Resolver、Prompt Context | 缺少 Draft mention → Profile candidate 的人工确认层 |
| Readiness | 七类 gate、Comfy preflight、批量 readiness | Draft 不应提前进入 readiness |
| Preparation | 服务端 re-resolve、Snapshot、现有 Batch/Queue admission | 只接受正式 Shot，不接受 Draft |
| UI | StudioShell、结构树、批量预览、Review compare | 没有 Script Import Workspace；现有 tree 不是 5000 Draft 的虚拟化方案 |
| Scale | 现有 Shot bulk/resolver 500 等正式生产限制 | 100 Episode / 1000 Scene / 5000 Draft Shot 的分页/虚拟化专项能力 |

实际证据索引：

- Prompt：`src-tauri/src/application/prompt_library_service.rs`、`prompt_template_service.rs`、`prompt_template_bulk_service.rs`。
- Import：`src-tauri/src/application/shot_bulk_service.rs`、`h3_local_import_service.rs`、`workflow_onboarding_service.rs`、`src/features/studio/batchImport.ts`。
- Structure：`src-tauri/migrations/021_production_structure.sql`、`production_structure_service.rs`、`src/features/shots/ProductionStructurePanel.tsx`。
- V1 Context/Production：`src-tauri/src/application/shot_context_resolver.rs`、`shot_readiness_service.rs`、`production_preparation_service.rs`、`production_queue_service.rs`。
- UI shell：`src/app/StudioShell.tsx`、`src/features/shots/ProjectStructureTree.tsx`、`src/features/production/ReviewCompareWorkspace.tsx`。

## 3. 架构决策

### 3.1 推荐 `ScriptDocument + DraftStructure`

不复制正式 `Series/Episode/Scene/Shot` 模型。推荐：

- `ScriptDocument` 保存原文来源、格式、checksum、source spans 和诊断。
- `DraftStructure` 保存可编辑的 Episode → Scene → Shot 候选树。
- `StoryboardDraft` 是 Draft Shot 的建议字段，不是执行模型。
- Match、diagnostics、source provenance 和人工操作记录作为 Draft payload 的组成部分。
- 只有 Promote 后才获得正式结构 IDs；Promote 后再单独确认 Profile binding。

### 3.2 最小持久化

候选表 `script_sources`、`script_import_drafts` 可以在未来 Data Foundation 任务中评审；V1 不建立 `draft_episodes`、`draft_scenes`、`draft_shots`、`draft_entity_matches` 四套镜像表。

建议 `script_import_drafts.payload_json` 承载一个不可变 revision 的 DraftStructure/StoryboardDraft。若 5000 Draft Shot 基准证明 JSON 全量读取不可接受，才在后续任务增加可重建 `draft_node_index`，并重新走 migration/backup/manifest/upgrade 评审。

### 3.3 唯一跨层写入边界

未来由一个后端 `script_import_confirm`（名称待冻结）完成：

```text
Draft revision
  → server validation + promote preview
  → user confirmation
  → one transaction: formal structure + mapping + provenance
  → no binding / readiness / preparation / queue side effect
```

确认前正式结构和 `shots` 必须零变化；失败整体回滚；重复确认幂等或返回可解释的 already-confirmed。

## 4. 核心 DEV 路线

以下 9 个 DEV 是根据当前仓库缺口调整后的核心路线。每个条目明确目标、依赖、输入、输出、禁止事项和验收门禁。

### DEV-057 — Script/Draft Data Foundation — PASS

| 项目 | 规划 |
| --- | --- |
| 目标 | 冻结并落地 ScriptDocument、DraftStructure、DraftRevision、source provenance 和最小存储策略 |
| 依赖 | DEV-056、0.7.0 domain vocabulary |
| 输入 | 本路线图、四路审计、TXT/Markdown/JSON 样例、5000 Draft Shot 容量目标 |
| 输出 | domain/DTO/schema contract、Migration 025、`script_sources` + `script_import_drafts`、Backup 15、revision/identity/hash/span/capacity contract |
| 禁止事项 | 不写 production 表；不改写 migration 024；不升级 Manifest 2；不建立正式结构镜像表、parser、LLM 或 command |
| 验收门禁 | schema version、project isolation、checksum、immutable revision、5000-node benchmark、Backup 15 roundtrip、旧项目无 Script 仍可打开 |

### DEV-058 — Script Import Parser — PASS

| 项目 | 规划 |
| --- | --- |
| 目标 | 将 TXT、Markdown、版本化 JSON 和非标准小说解析为 Draft preview |
| 依赖 | DEV-057 |
| 输入 | ScriptDocument/source bytes、format、解析选项 |
| 输出 | source blocks、Episode/Scene/Shot 候选、source spans、diagnostics、DraftRevision |
| 禁止事项 | 不自动创建正式 Series/Episode/Scene/Shot/Profile；不接真实 LLM；不调用 Queue/Comfy |
| 验收门禁 | UTF-8/BOM、标题/段落、Markdown code block/list、JSON schema、小说对白/叙述/心理/时空变化、零 formal writes |

### DEV-059 — Entity Match + Profile Suggestions

| 项目 | 规划 |
| --- | --- |
| 目标 | 将角色/场景/道具 mention 映射为项目隔离的 Profile 候选 |
| 依赖 | DEV-057、0.7.0 Profile/ReferenceSet |
| 输入 | Draft mentions、entity type、normalized text、现有 project Profiles |
| 输出 | EXACT/LIKELY/NO_MATCH/AMBIGUOUS、候选、证据、用户选择字段 |
| 禁止事项 | 不自动 binding；不自动创建 Profile；不从文本猜 Asset/ReferenceSet/顺序 |
| 验收门禁 | project/type scope、稳定排序、同分冲突 fail-closed、人工选择/改选/忽略完整可用 |

### DEV-060 — Storyboard Draft Builder

| 项目 | 规划 |
| --- | --- |
| 目标 | 形成每镜可编辑的 StoryboardDraft 建议 |
| 依赖 | DEV-058、DEV-059 |
| 输入 | Draft Scene/Shot、source spans、match candidates、可选文本分析 Patch |
| 输出 | name、purpose、characters、scene、props、action、dialogue、camera、lighting、duration、image/video prompt draft |
| 禁止事项 | 不生成图片/视频；不产生 GenerationValues、workflow、asset、batch 或正式 context |
| 验收门禁 | 字段完整、DRAFT 标记、source provenance、Prompt draft 与 ResolvedShotContext 分离 |

### DEV-061 — Draft Review Workspace

| 项目 | 规划 |
| --- | --- |
| 目标 | 在既有 Creation shell 下提供 Script Import 三栏审阅体验 |
| 依赖 | DEV-060、StudioShell、ProjectStructureTree、Review compare 交互 |
| 输入 | DraftRevision、diagnostics、matches、diff |
| 输出 | 左原文结构、中 Draft Tree、右 Inspector；Accept/Reject/Edit/Merge/Split/Reorder、筛选和批量动作 |
| 禁止事项 | Draft 操作不得调用 formal create、binding、readiness、queue 或 Comfy |
| 验收门禁 | DRAFT 水印、键盘/ARIA、中文、未保存保护、差异预览、5000 Draft virtualization/pagination、无横向溢出 |

### DEV-062 — Confirm to Production Structure

| 项目 | 规划 |
| --- | --- |
| 目标 | 让用户一次显式确认把已审阅 Draft 晋级为正式结构 |
| 依赖 | DEV-061、ProductionStructureService、ShotBulkService |
| 输入 | projectId、draftId、revision、target、已确认 Draft 节点 |
| 输出 | formal Series/Episode/Scene/Shot、assignment、draftNodeId → formalId mapping、provenance |
| 禁止事项 | 不覆盖已有正式 Shot；不前端循环 createShot；不自动 binding、prepare、queue 或 start |
| 验收门禁 | Confirm 前零写入、单事务、失败回滚、重复幂等、跨项目校验、目标冲突可解释 |

### DEV-063 — Prompt / Consistency Integration

| 项目 | 规划 |
| --- | --- |
| 目标 | 将正式结构和人工确认的 Profile 选择接入已有 Context/Prompt/Readiness contract |
| 依赖 | DEV-062、DEV-049、DEV-050、DEV-054 |
| 输入 | formal IDs、confirmed matches、可选 storyboard prompt draft |
| 输出 | existing bindings、ResolvedShotContext、stage prompt、readiness input |
| 禁止事项 | 不复制 resolver/prompt builder；不把 Draft prompt 当 final；不自动选择 ReferenceSet/Asset |
| 验收门禁 | image/video stage、hash、legacy fallback、Profile/ReferenceSet 人工确认、selected image 和现有七 gate 兼容 |

### DEV-064 — Narrative Preproduction V2 Integration

| 项目 | 规划 |
| --- | --- |
| 目标 | 串起 source、Draft revision、diff、provenance、formal promote 和现有准备/审计体验 |
| 依赖 | DEV-062、DEV-063、DEV-054 |
| 输入 | promoted structure、revision lineage、optional provider seam、5000 Draft fixture |
| 输出 | Command Center/Audit 摘要、恢复策略、分页/虚拟化硬化、offline/provider 配置边界 |
| 禁止事项 | 不新增 executor/queue/scheduler；不强制旧项目引入 Script；不自动运行生产 |
| 验收门禁 | snapshot lineage、large-draft performance、0.7.0 open/legacy/backup/manifest compatibility、no side effects |

### DEV-065 — AI Studio 0.8.0 Release Gate

| 项目 | 规划 |
| --- | --- |
| 目标 | 对 Script → Draft → Confirm → V1 Production 闭环做最终发布验收 |
| 依赖 | DEV-064 |
| 输入 | 完整 0.8.0 source RC、旧 0.7.0 fixtures、规模 fixture、isolated provider/Comfy fixtures |
| 输出 | 0.8.0 source RC、回归/升级/备份/Manifest/性能/安全/发布证据 |
| 禁止事项 | 不放宽 manual gate；不发布未通过 Draft/Formal 隔离、hash 或升级验证的版本 |
| 验收门禁 | P0/P1=0、full regression、旧项目打开、确认原子性、5000 Draft、Provider offline boundary、正式 Comfy 链路不变 |

## 5. 依赖与并行策略

```text
DEV-057
   ├─ DEV-058 ─┐
   └─ DEV-059 ─┴─ DEV-060 → DEV-061 → DEV-062 → DEV-063 → DEV-064 → DEV-065
```

- DEV-058 与 DEV-059 在 057 contract 冻结后可以并行。
- DEV-061 只能复用现有 shell/tree/review 交互，不把正式 `ShotView` 当 Draft model。
- DEV-062 是唯一跨层写入点；之后的 063/064 才能调用现有 Context/Readiness/Preparation。
- Provider abstraction 可以在 057/058 先定义，但真实 Provider 不应早于明确的安全、隐私和离线策略。

## 6. 0.8.0 验收总表

| 门禁 | 必须满足 |
| --- | --- |
| Input | TXT、Markdown、JSON 可导入；小说以低置信度候选处理；source span 和 checksum 可追溯 |
| Draft | 所有 AI/解析结果标记 DRAFT；可编辑、合并、拆分、重排、拒绝；不创建正式 ID |
| Match | EXACT/LIKELY/NO_MATCH/AMBIGUOUS 全覆盖；项目/类型隔离；高风险不自动确认 |
| Formal promote | 只有用户显式 Confirm；后端重新校验；单事务、回滚、幂等、mapping/provenance |
| Context | 正式 Shot 只走现有 Resolver/PromptContextBuilder；Draft prompt 不取代 final context |
| Production | 仍是 Readiness → Preparation → ProductionQueue → GenerationService → ComfyUI；手动 start |
| Safety | 不自动创建 Profile/binding/Batch/Task；不自动选片/审核/生成；无 second executor/queue |
| Scale | 100 Episodes、1000 Scenes、5000 Draft Shots 有 pagination/virtualization/内存和输入响应证据 |
| Compatibility | 0.7.0 项目无 ScriptDocument 仍可打开和生产；Migration 025/Backup 15 可升级恢复，Manifest 2 contract 保持不变 |
| Audit | provenance 可回答 manual/Script Import/Storyboard Draft 来源及 revision，不复制全文到每个 Shot |

## 7. 明确延期到 0.9+

- 全自动导演、自动生成整集、自动选择图片、自动批准视频、自动运行 Queue。
- 无人值守 pipeline、无限 Agent 自主循环、Scheduler、Auto-next。
- 自动覆盖已经人工修改或确认的正式 Shot。
- DOCX/PDF/复杂自然语言的“完美改编”承诺。
- 云端协作、账号、同步、模型训练、LoRA 和插件市场。
- `StoryboardExecutor`、`ScriptProductionQueue`、`AIProductionScheduler`、`AutoGenerationPipeline`、第二 ProductionRun。

这些不是 0.8.0 的隐含交付项，未来若重新提出必须另立 0.9+ 范围和安全评审。

## 8. 下一步

当前任务：**DEV-061 — Bulk Production Hardening**。

DEV-057 已决定 Draft 需要跨应用重启恢复，原始 UTF-8 文本只存于 `script_sources.source_text`，并以 5000-node benchmark 决定索引策略；DEV-058 已完成 TXT/Markdown/JSON v1/小说解析、source map、诊断、reparse 与 zero formal side effects，均为可选能力。DEV-059 与 DEV-060 已完成外部生产包 V1 和批量视频生产工作区，并保持不增加 `draft_node_index`、Migration 026、Backup 16 或 Manifest 3。下一步是 **DEV-061 — Bulk Production Hardening**，随后进入 **DEV-062 — AI Studio 0.8.0 Release Gate**。
