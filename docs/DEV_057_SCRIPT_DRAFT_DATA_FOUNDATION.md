# DEV-057 Script/Draft Data Foundation

状态：DEV-057 SCRIPT/DRAFT DATA FOUNDATION PASS。

DEV-057 为 AI Studio 0.8.0 路线的第一层数据基础，但产品版本仍保持 **0.7.0**。本 DEV 只建立 ScriptDocument/DraftStructure 的领域 contract、最小持久化、不可变 revision、校验、备份和兼容性；不实现 parser、LLM、Entity Match、Storyboard、Review UI、Promote 或任何正式生产写入。

## 基线与执行

- `DEV057_START_SHA`：`435585b48164cccbbd6ad92a3f37a797e86652ad`
- 分支：`master`
- DEV-056 Source-only CI Run `33074103337`：开工前已完成，`success`
- 实现由 Main + A/B/C/D 四个并行、无嵌套代理完成；子代理不提交、不推送，Main 统一集成。

## 冻结的数据模型

- `ScriptDocument`/`ScriptSource` 描述项目内的原始 UTF-8 来源：`scr_<UUID>`、格式 `TXT|MARKDOWN|JSON`、原始字节 SHA-256、可选文件名、source span 和 diagnostics。
- `DraftStructureV1` 是文档型 Episode → Scene → Shot 草稿树；节点使用 `dnode_<UUID>`，诊断使用 `diag_<UUID>`，draft 生命周期使用 `drf_<UUID>`。
- revision 使用 `drev_<UUID>`；schema version 和 contract version 均为 `1`。
- 数组顺序参与 payload hash，map key 按 canonical JSON 排序；因此相同 payload 稳定得到相同 SHA-256，而数组重排会改变 hash。
- `SourceSpan` 全部为 0-based、end-exclusive，并校验字节边界、UTF-8 边界和 source 长度。

## SQLite 持久化

Migration 025 只增加两张核心表：

```text
projects
  └── script_sources
        └── script_import_drafts (immutable revision rows)
```

`script_sources.source_text` 是唯一的原始文本存储位置；BOM、CRLF/LF 和原始 UTF-8 字节先计算 checksum，再以 TEXT 保存。唯一键为 project + checksum + format，同项目改文件名复用 source identity，跨项目绝不复用。`script_import_drafts` 保存 summary、payload、parser/provider metadata 和 previous revision link；数据库 trigger 禁止原地 UPDATE。

本 DEV 没有 `draft_episodes`、`draft_scenes`、`draft_shots`、`draft_entity_matches` 或 `draft_node_index`，也没有 production/task/queue 表写入。5000-node 基准若超过 64 MiB 或 load + deserialize 超过 2000 ms，测试会以 `DEV-057 BLOCKED` 失败，而不是偷偷增加索引。

## Revision contract

- revision 从 1 开始；revision 1 的 previous link 必须为空，后续 revision 必须指向同 project、同 draft 的恰好上一 revision。
- append 由 repository 在同一 SQLite transaction 内读取 latest、校验 `expected_revision`、生成下一个 revision 和 previous link；过期写入返回 `DRAFT_REVISION_CONFLICT`。
- 相同 payload 且 revision metadata 无语义变化时返回现有 revision，不产生伪 revision。
- list/latest/history 只返回 metadata 和 summary，不读取 `payload_json`；默认 page size 50，最大 200，排序和 cursor 稳定。

## 容量与安全

- source 上限：16 MiB 原始 UTF-8 字节。
- 草稿容量：100 Episodes、1000 Scenes、5000 Shots；超限分别返回稳定容量错误。
- source text 不进入 list DTO、summary、history DTO、Debug 输出、错误消息或日志全文；错误只保留类型、bytes、checksum 短信息等必要上下文。
- Script/Draft service 不依赖 formal structure、Shot、Consistency、Readiness、Preparation、Batch、Task、Generation、Workflow 或 Comfy adapter；不接 AppState、Tauri command 或前端。

## Backup 与 Manifest

- Backup version 从 14 升至 15；Backup 15 包含 source text、source metadata 和全部 draft revision rows，恢复后保留 source checksum、draft/revision identity、revision order、previous links、payload 和 hash。
- Backup 14/13/12 继续可恢复；旧版本没有 Script/Draft sections。
- Manifest 仍为 version 2，故意不包含 `scriptSources`、`scriptDrafts`、`sourceText` 或 `payloadJson`。Script/Draft 是 backup-scoped working data，不改变正式 Project semantic manifest。
- Project 删除通过 FK cascade 清理 Script/Draft；DEV-057 不提供正式 delete source/delete draft service。

## 5000-node benchmark

本次实测：payload `3,528,684` bytes；serialize `176 ms`；validate `5 ms`；hash `233 ms`；SQLite insert `281 ms`；load `5 ms`；deserialize `64 ms`；load + deserialize `69 ms`。均低于门禁（64 MiB、2000 ms），因此 `DRAFT_NODE_INDEX=NOT_NEEDED_V1`，没有增加索引表。

## 验证与后续

验证范围包括 domain canonical/hash/span/capacity、source dedupe/isolation、revision chain/conflict/no-op、SQLite roundtrip、Backup 15 roundtrip、旧备份兼容、Manifest 2 exclusion、formal side-effect sentinel、Rust/frontend regression、build 和 Source-only CI。

最终门禁：Rust `cargo test` 为 `824 passed / 0 failed / 2 ignored`（lib `660 / 0 / 1`，integration `164 / 0 / 1`）；`cargo check --tests`、rustfmt 和 `git diff --check` 通过。前端 `pnpm test` 为 `92 files / 350 tests / 0 failed`，TypeScript 检查和 production build 通过。

DEV-058 下一步是 Script Import Parser：基于本 DEV 的 `ScriptSource`、`DraftStructureV1` 和 immutable revision 实现 TXT/Markdown/版本化 JSON/保守小说解析、source blocks、spans 和 diagnostics。仍必须保持零 formal writes、零 Profile create、零 Queue、零 Comfy、零真实 LLM。
