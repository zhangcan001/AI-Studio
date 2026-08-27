# AI Studio Narrative Production Context

本词汇表定义 AI Studio 从脚本草稿到正式镜头生产之间的领域语言。它只记录业务概念，不记录实现细节。

## Script 与 Draft

**ScriptDocument**：用户导入的原始故事或剧本文档，以及它的格式、来源、校验和定位信息。它是输入事实，不是 Project、Shot 或生产任务。
_Avoid_: Script、Storyboard、Production Structure

**DraftStructure**：由 ScriptDocument 产生、可被人工编辑但尚未写入正式项目结构的有序 Episode → Scene → Shot 候选树。
_Avoid_: Temporary Shot、Formal Structure、Preview Result

**StoryboardDraft**：DraftStructure 中针对镜头的叙事和画面建议集合，包括动作、对白、镜头、灯光、时长和提示词草稿。
_Avoid_: StoryboardExecutor、Generation Plan、Production Run

**Formal Production Structure**：项目中已经确认并具有正式身份的 Series、Episode、Scene、Shot 及其归属关系。
_Avoid_: Draft Tree、Imported Structure

**Draft Revision**：同一导入来源的一次不可变解析结果。重新解析产生新 Revision，并通过差异审阅决定是否继续人工编辑或确认。
_Avoid_: In-place Reparse、Overwrite

## Consistency 与生产

**Profile Match**：脚本提及与当前项目 Profile 候选之间的人工确认前匹配结果，状态为 EXACT、LIKELY、NO_MATCH 或 AMBIGUOUS。
_Avoid_: Auto Binding、Inference Result

**ResolvedShotContext**：正式 Shot 在指定 stage 下经现有 Resolver 动态得到的最终一致性、提示词、参考资产和输入上下文。
_Avoid_: Draft Prompt、Storyboard Suggestion

**Readiness**：正式 Shot 是否满足进入生产准备的七类门禁及其 blocker/warning 结果。
_Avoid_: Generation Status、Draft Confidence

**Production Snapshot**：用户准入时冻结的正式生产输入和证据，包括上下文、提示词、参考资产 checksum、workflow、recipe、output spec 和运行能力证据。
_Avoid_: Live Context、Draft Snapshot

**Manual Gate**：必须由用户明确触发的确认、候选选择、审核、加入队列或 Queue Start 动作。
_Avoid_: Auto-Approve、Auto-Start、Unattended Generation

**Source Provenance**：正式结构或 Draft 节点与原始 ScriptDocument、source span、解析版本和人工修改之间的来源关系。
_Avoid_: Full Source Copy、Prompt History
