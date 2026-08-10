# M3 CREATION FLOW PACK 09 = HISTORICAL SOURCE NOTE

本文件保留 Pack 09 初始 source slice 的历史记录。正式实现已进入
`docs/M3_SHOT_PRODUCTION_PACK_09.md`，并复用现有 GenerationService / Task /
Snapshot / Asset 链路；没有创建第二套 Task 状态体系。

当前模型覆盖：

- Shot id、projectId、ordinal、name
- Prompt Library 引用或 inline prompt（二选一）
- workflowVersionId + recipeId（二者成对出现）
- reference asset IDs、selected result asset ID、shot status
- 项目内重排与结果选择的纯逻辑

历史 source slice 的“尚未接入数据库”描述不再代表当前状态。`001–009` 仍保持
immutable，Pack 09 的正式迁移为 `010_shot_production.sql`；活动生产范围冻结为
Kera2 图片关键帧 + MiniMax H3 参考图生视频。
