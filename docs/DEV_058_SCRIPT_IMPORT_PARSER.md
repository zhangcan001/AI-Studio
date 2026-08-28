# DEV-058 Script Import Parser

状态：DEV-058 SCRIPT IMPORT PARSER PASS

基线：`691e941467eb989a14942acbbc27cad94852fbc9`；DEV-057 Source-only CI
`33089648966` 已通过。版本冻结不变：Product `0.7.0`、Migration `025`、
Backup `15`、Manifest `2`。

## 边界

DEV-058 将 ScriptSource 的 UTF-8 原文离线、确定性地解析为
`ScriptDocument`、`SourceBlock` 与 `DraftStructureV1`。输出永远是 Draft，
不创建正式 Series/Episode/Scene/Shot，不读写 Profile、ReferenceSet、Batch、
Task、Readiness、Preparation、Generation 或 ComfyUI，也不调用 LLM 或网络。

公共 parser 版本固定为 `script-import-v1`。`ScriptParseMode` 支持 AUTO、
SCREENPLAY、NOVEL；选项只有 mode 与 `preserveHumanEdits`。

## Source integrity

所有格式共享同一个 `SourceMap`：原始 UTF-8 bytes 映射到 0-based、
end-exclusive 的 line/character。character 是 Unicode scalar-value index，
不是 UTF-8 byte 或 JavaScript UTF-16 code unit。BOM 参与 checksum、语义解析
忽略，并产生 `SOURCE_ENCODING_OR_BOM`；CRLF 保留原始两个 bytes 的 offset。
SourceBlock ID、Draft node ID 与诊断 ID 都由稳定输入派生；preview 最多保留
160 个 Unicode 字符，完整原文只在 `script_sources.source_text`。

## Format behavior

- TXT/SCREENPLAY：段落、强 episode/scene 标题、对白、动作与显式角色 mention。
- Markdown：H1/H2/H3、段落、列表、quote、table-like、fenced code；code 不
  进入结构，link 只保留文本、不访问 URL。
- JSON：独立 schema v1，root 必须 `schemaVersion/title/episodes`；数组顺序
  生成 0-based ordinal，`sourceId` 只进入 `scriptImport.anchorMap.v1`，不会
  变成正式 ID；类型、版本、重复 anchor、未知字段、容量错误均可定位。
- NOVEL：章节、叙述、引号对白、保守的时空边界和心理描写候选；心理描写
  只标记 narration/thought，不自动变成 action。

## Reparse and safety

`ScriptImportService` 先 preview，再由既有 `ScriptDraftService` 写入
`PARSED`/`REPARSED` immutable revision。相同 source、parser、options 与语义
payload 是 no-op；换 source 即使结构相同也产生新 revision。reparse 通过
anchorMap 保留未变化节点 ID，更新 source spans，保留 `currentValue`、
`reviewState` 与 `origin`，并返回 retained/added/removed/changed summary。
取消产生 `SCRIPT_PARSE_CANCELLED`，不写 revision；invalid UTF-8、非法 JSON
和 capacity 超限 fail-closed，不静默截断。

## Verification

验收覆盖格式、BOM/CRLF/Unicode span、长行、空文档、JSON safety、小说保守
策略、reparse diff、人工编辑保留、取消和真实 SQLite formal-table zero
side-effect。5000 Draft shots 记录 bytes、parse、reconcile、validate、total；
不通过 benchmark 就不新增 index/migration。DEV-059 Entity Match、DEV-060
Storyboard Draft、DEV-061 Review Workspace 仍是后续任务。

## 5000-node benchmark

在本地 debug profile 的 DEV-058 集成夹具中，100 Episodes / 1000 Scenes /
5000 Shots 使用 `351134` source bytes、`1` 个 source block；一次记录为
parse `228ms`、reconcile `458ms`、validate `50ms`、service preview total
`291ms`。这些是机器相关 telemetry，不构成运行时 SLA；结果足以保持
全量 payload 策略，不新增 `draft_node_index`、Migration 026 或其他 schema。

## Multi-agent evidence

- Agent A — DONE：`source_map.rs`、`text_parser.rs`；UTF-8/BOM/CRLF/Unicode
  spans、TXT/SCREENPLAY/Novel 候选与单元测试。
- Agent B — DONE：`markdown_parser.rs`；heading/list/quote/table/code/link
  的离线 Draft 解析与单元测试。
- Agent C — DONE：`json_parser.rs`；独立 schema v1、sourceId anchor、严格
  类型/容量/unknown metadata/execution-field 隔离与单元测试。
- Agent D — DONE：`reconcile.rs`、`script_import_service.rs`、DEV-058
  SQLite/取消/reparse 集成测试。

`MULTI_AGENT_EXECUTION = CONFIRMED`；最终接线、回归、提交和发布由 Main 完成。
