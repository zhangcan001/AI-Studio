# M3 CREATION FLOW PACK 09 = SOURCE STARTED

Pack 08 完成后已进入 Pack 09 source slice。本轮只建立 project-scoped Shot 领域模型与纯函数测试，尚未接入数据库、Task 新体系或生成调度器。

当前模型覆盖：

- Shot id、projectId、ordinal、name
- Prompt Library 引用或 inline prompt（二选一）
- workflowVersionId + recipeId（二者成对出现）
- reference asset IDs、selected result asset ID、shot status
- 项目内重排与结果选择的纯逻辑

后续在新增迁移前需要先确认模型；`001–009` 保持 immutable，暂不创建 `010_shots.sql`。Shot 生成必须复用 Studio / GenerationService / normal Task / Asset 链路，不创建第二套 Task 状态体系。
