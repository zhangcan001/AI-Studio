# DEV-044 UI C 功能迁移矩阵

> 本表保留 DEV-044 附件列出的全部旧功能，并记录主线程接入后的落点。所有 STATUS 只使用 `MOVED`、`PRESERVED`、`MERGED`。

| OLD FUNCTION | OLD LOCATION | NEW LOCATION | BACKEND COMMAND / DATA CONTRACT | STATUS |
| --- | --- | --- | --- | --- |
| 新建 Shot | `src/features/shots/ShotWorkspace.tsx` | Project Structure Header → 新建 Shot | `createShot(projectId)` | MOVED |
| 批量导入 | `src/features/shots/ShotBulkImportPanel.tsx` | Project Structure Header → 批量导入 | `createShotBatch(request)` | MOVED |
| 导出 Manifest | `src/features/shots/ShotWorkspace.tsx` | Project / More Menu | `exportProjectManifest(projectId, destination?)` | MOVED |
| 刷新 | `ShotWorkspace` /各生产面板 | 各 context controller 与 Queue Drawer host 的刷新入口 | 现有 list/get/reload callbacks | PRESERVED |
| Structure CRUD | `src/features/shots/ProductionStructurePanel.tsx` | Project Structure Tree → 管理入口（原 CRUD 面板默认收起） | 现有 structure CRUD invoke commands | MOVED |
| Series Planner | `src/features/shots/SeriesProductionPanel.tsx` | Series context workspace | 现有 Series planner callbacks 与 batch preset/prompt contracts | MOVED |
| Episode Planner | `src/features/shots/EpisodeProductionPanel.tsx` | Episode context workspace | 现有 Episode planner callbacks 与 production commands | MOVED |
| Scene Planner | `src/features/shots/SceneProductionPanel.tsx` | Scene context workspace | 现有 Scene planner callbacks、`applyPromptTemplate` 与 batch commands | MOVED |
| Runbook | `src/features/production/ProductionBatchRunbookPanel.tsx` | Global Production workspace；Shot 页面只保留 Queue Drawer 摘要 | `getProductionBatchRunbook(request)` 与现有 single-batch start | PRESERVED |
| Project Pipeline | `src/features/shots/ProjectProductionPipeline.tsx` | Project / Analysis / Production workspace | 现有 pipeline read models 与 progress derivation | MOVED |
| Shot list / search / filter | `src/features/shots/ShotListToolbar.tsx`、`shotListQuery.ts` | Project Structure pane 的镜头定位区与搜索结果 | 现有 shot list controls、query/filter/page-size props | MOVED |
| Shot metadata | `src/features/shots/ShotWorkspace.tsx` | Shot Inspector → Metadata | `updateShot(request)` | MOVED |
| Prompt | `src/features/shots/PromptTemplatePanel.tsx`、`ShotWorkspace.tsx` | Shot Inspector → Prompt；Planner 保留批量 Prompt | `applyPromptTemplate(request)` 与现有 prompt library callbacks | MERGED |
| Stage config | `src/features/shots/ShotWorkspace.tsx` | Shot Inspector → Parameters / Advanced | `updateShot`、现有 stage-config save callbacks | MOVED |
| Reference | `src/features/shots/ShotWorkspace.tsx` | Shot Inspector → References | 现有 `shot_references_*` contract 与 ordered reference state | MOVED |
| Reference Anchor | `src/features/shots/referenceAnchorApply.ts`、`ShotWorkspace.tsx` | Inspector References / Planner Context Anchors | `reference_anchor_*` 与现有 ordered apply helpers | MERGED |
| Generate | `src/features/shots/ShotWorkspace.tsx`、`src/features/studio/GenerationStudio.tsx` | Shot Creation Workspace → Generate | 现有 generation / production callbacks | MOVED |
| Select candidate | `src/features/shots/ShotBatchReviewBoard.tsx`、`ShotWorkspace.tsx` | Shot Creation Workspace → Candidates / Manual Review | 现有 candidate selection callbacks与 asset ids | MOVED |
| Retry | `src/features/shots/ShotWorkspace.tsx`、`src/features/studio/ProductionQueuePanel.tsx` | Queue Drawer 单项 Retry；Global Review 保留批量审计入口 | `requeueProductionQueueItemByItem(projectId, itemId)` 与现有 queue requeue policy | MERGED |
| Review | `src/features/studio/ProductionBatchReviewWorkspace.tsx`、`src/features/production/ProductionAuditCenter.tsx` | Shot Workspace Manual Review + Global Review / Audit | 现有 review callbacks、task/asset references | MERGED |
| Queue navigation | `src/features/studio/ProductionQueuePanel.tsx`、`src/features/production/ProductionBatchRunbookPanel.tsx` | `src/features/production/ProductionQueueDrawer.tsx`；Open 仍由 host 导航 | `overview/details/items` props 与 `onOpen(batchId)` | MERGED |

## Queue Drawer ownership

Queue Drawer 的稳定 Props：

```ts
overview?: ProductionQueueOverview | null
details?: readonly ProductionBatchDetail[]
items?: readonly ProductionQueueDrawerItem[]
onToggle?: (expanded: boolean) => void
onStart?: (batchId: string) => void | Promise<void>
onPause?: (batchId: string) => void | Promise<void>
onRetry?: (itemId: string) => void | Promise<void>
onOpen?: (batchId: string) => void | Promise<void>
```

`runbook` 与旧 `queues` summary 是兼容性输入；它们只作为现有数据快照，不在 Drawer 内 fetch、缓存或创建第二套队列。当前后端模型没有统一的 thumbnail / resolution / duration 字段，组件只读取传入对象上的可选展示槽位，缺失时显示“—”。

## Audit result

- Old functions listed: 21
- `MOVED`: 14
- `PRESERVED`: 2
- `MERGED`: 5
- Unmapped functions: 0
- Queue Drawer scheduler / global start / automatic next admission: not part of this matrix or component contract

新壳层与镜头工作区已接入 `App.tsx` / `ShotWorkspace.tsx`；旧生产面板默认收起但仍可打开，未删除任何 backend action 或人工审核 gate。
