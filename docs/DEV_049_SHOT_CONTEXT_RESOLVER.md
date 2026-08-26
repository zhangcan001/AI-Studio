# DEV-049 — Hierarchical Shot Context Resolver

状态：实现说明 / Release note

## Baseline

- Branch：`master`
- Start SHA：`f6b5fc855d663069e134152a04940c10bb37882f`
- Product：`0.6.2`
- Backup：`12`
- Manifest：`1`
- 工作区在开发前 clean，未把未知提交并入本任务。

## Architecture gap

DEV-048 的 Migration 022 已提供 Shot-level profile/reference-set bindings，但 Project、Series、Episode、Scene 没有持久化 binding 来源。若只在 resolver 中猜测上层默认值，就无法保存继承关系、来源和覆盖策略，也无法支撑后续 readiness 解释。

## 为什么新增 Migration 023

022 已进入 master，且可能已经被本地数据库执行。修改历史 migration 会带来 SQLx migration checksum mismatch 风险。因此 DEV-049 不修改 022，而新增 forward-only Migration 023，只创建：

- `consistency_scope_profile_bindings`
- `consistency_scope_reference_set_bindings`

023 只覆盖 PROJECT/SERIES/EPISODE/SCENE；Shot 继续复用 022 的两张 Shot binding 表，没有 024。

## Scope persistence

`ConsistencyScopeType` 固定为 PROJECT、SERIES、EPISODE、SCENE。上层 profile binding 使用 `hpb_` ID，reference-set binding 使用 `hrb_` ID。Repository 提供 project list 与两种 transactional replace；service 用一次 `load_tree_data(project_id)` 校验 scope membership、profile/reference-set project ownership、role/type、costume ownership 和 same role+ordinal conflict。

`scope_id` 与 polymorphic `profile_id` 不伪造 SQLite FK；写入 service 和 resolver 都对旧数据库/手工 SQL 做防御校验。

## Five-level inheritance

Resolver 路径固定为：

```text
Project → Series → Episode → Scene → Shot
```

未分配 Scene 的 Shot 仍可走 Project→Shot。Scope bindings 先按 project 批量读取，Shot bindings 通过 022 port 批量读取。每个输出值保留 `SourceTrace`，包括 scope、scope_id、binding_id、entity_id 和 inheritance mode。

## Merge rules

- EXPLICIT/INHERITED：按 role+entity add/override；
- REPLACE：清空当前 role 的祖先集合后加入当前值；
- REMOVE：删除 entity 并创建 tombstone；更深层 EXPLICIT 可重新加入；
- 同一 scope 同 role+ordinal 出现不同 entity：输出 ERROR，冲突 slot 不选任意赢家；
- Scene/Style 为 single-value；Character/Prop 按 ordinal、ID 稳定排序。

## Reference Pack

`ShotReferencePack` 聚合 characters、scene、props、style、reference_sets、prompt_context 和 source_trace。有效 profile 的默认 ReferenceSet、上层/Shot ReferenceSet binding 进入同一个 merge/expand 路径。

ReferenceSet items 与 assets 采用 bulk read，展开为 `ResolvedReferenceAsset`。Asset 必须存在、属于当前 project 且为 Image；否则产生 ERROR/partial。最终顺序为 role rank → binding ordinal → item ordinal → asset ID，重复 concrete asset 只保留首次出现。

## Prompt Builder

`PromptContextBuilder` 固定九段顺序：Global Style、Scene、Character、Costume、Props、Shot Action、Camera、Lighting、Output Specification。空文本不落 segment，正向文本以单个换行渲染，来源和 revision 保存在 PromptSegment。

negative prompt 独立收集，按 Style→Scene→Character ordinal 顺序 normalized 去重。Scene 没有 SceneProfile 时使用 ProductionScene.description 的 LEGACY 片段；Shot Action 只读取请求 stage 的 stage prompt，空缺才 fallback 到 Shot legacy prompt。

## Revision behavior

active revision 存在时批量读取并校验 profile type/id 与 content SHA-256。缺失或不匹配产生 ERROR，但保留 live profile 内容生成可解释的 partial context。没有 active revision 时使用 live profile JSON hash 并给出 WARNING。ReferenceSet 使用 ordered item + asset sha256 计算 `ReferenceSetContentHash`。

## Legacy fallback

没有有效新 ReferenceSet binding 时，旧 Shot 的当前 stage `shot_reference_assets` 按 ordinal 展开；`legacy.has_reference_pack=false`，有旧参考图时 `uses_legacy_shot_references=true`。legacy prompt 继续作为 Shot Action fallback。

只要有有效新 ReferenceSet binding，新 pack 优先，不追加 legacy references。只有 profile binding 而没有有效新 ReferenceSet 时仍允许 legacy references，避免把 profile-only 数据误判为完整新 pack。

## contextHash

`ContextHashInput` 包含 project/structure/stage、有效 profile IDs 与内容 hash、costume IDs、ReferenceSetContentHash、ordered asset IDs/sha256、ordered prompt segments、negative prompt、workflow/recipe、scalar values 和 output spec。使用现有 sha2 SHA-256。

`resolved_at`、随机 ID、diagnostic message、HashMap iteration order 排除在 hash 外；因此同一输入稳定，生成相关字段变化会改变 hash。

## Batch performance

`resolve_many_draft` 限制 500 shots，批次读取路径为：project 一次、structure tree 一次、shot list 一次、scope bindings 两次、shot bindings 两次 bulk、四类 profile list、costumes bulk、reference sets/items bulk、revisions bulk、assets bulk。SQLite bulk 实现使用分块 IN 查询，不对每个 shot 进行 repository.find。

## Diagnostics

Diagnostic severity 只有 WARNING/ERROR。解析器覆盖 project/shot not found、profile/reference project mismatch、profile/reference ordinal conflict、asset not found/project mismatch/image required、costume mismatch、revision missing/hash mismatch 和 single-role conflict 等错误码，并以 `partial` 汇总 ERROR。

## Tests

DEV-049 测试覆盖：

- fresh 001→023、022→023 升级与既有 022 行保留；
- EXPLICIT/INHERITED/REPLACE/REMOVE、deeper re-add、同 scope conflict；
- 九段 prompt order、negative dedupe、stage separation；
- ReferenceSet content hash、asset order、context hash stability/sensitivity；
- legacy fallback、新 pack precedence；
- resolver revision/asset validation；
- 500-shot batch 与 counting fake/spy repository call counts。

## Compatibility

产品兼容版本保持 0.6.2，Backup 12、Manifest 1 不变。旧生产表、旧 prompt、旧 reference assets、Production Queue/Run/Audit/Review/Comfy runtime 均未改动。前端 `src/**`、Tauri commands、command registration 和 AppState wiring 均未修改。

## Deferred DEV-050

DEV-049 不实现 Readiness、Preflight、READY/INCOMPLETE/BLOCKED、七类 gates、Comfy capability、Production Preparation、snapshot 或 queue integration。下一步为：

`DEV-050 — Shot Readiness + Preflight`

其后端权威输入是本 DEV 产出的 `ResolvedShotContext`。
