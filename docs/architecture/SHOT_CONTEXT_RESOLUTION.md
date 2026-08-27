# Shot Context Resolution

状态：DEV-049 已实现的 0.7.0 context contract

本文件冻结 Resolver 的输入、合并、排序和 hash 语义。它描述模型无关的 Draft context；Readiness、Preflight、Preparation、Stage Adapter 和执行链继续由后续 DEV 负责。

## 1. 单一入口

`ShotContextResolver` 提供：

```text
resolve_draft(project_id, shot_id, stage)
resolve_many_draft(project_id, shot_ids, stage)
```

`resolve_draft` 复用同一 batch 实现，不维护第二套 merge 算法。`resolve_many_draft` 单批最多 500 个 shot，超限返回 `CONTEXT_BATCH_LIMIT`。

所有持久化读取先批量完成，shot 级阶段只做内存 map traversal。SQLite bulk port 使用 set-based `IN (...)` 查询并按参数上限分块；已有 fake 可继续使用 port 的 default bulk implementation。

## 2. 五级路径与来源

```text
PROJECT → SERIES → EPISODE → SCENE → SHOT
```

- Project/Series/Episode/Scene binding 读取 `consistency_scope_*`（Migration 023）。
- Shot binding 读取 022 已有 `shot_profile_bindings` 与 `shot_reference_set_bindings`。
- 路径来自一次 `ProductionStructureRepository.load_tree_data(project_id)`。
- Shot 没有 Scene assignment 时仍解析 Project→Shot。
- 每个有效 binding 进入 `SourceTrace`：scope、scope_id、binding_id、entity_id、inheritance_mode。
- Scope 只能是 PROJECT/SERIES/EPISODE/SCENE；输出 source 另包含 SHOT/LEGACY。

## 3. Merge 语义

输入先按父到子 scope rank、scope id、binding id 稳定排序。

- `EXPLICIT` / `INHERITED`：按 `role + entity_id` add/override。
- `REPLACE`：清空当前 role 的祖先集合，再加入当前 entity。
- `REMOVE`：删除 entity 并记录 tombstone。
- 更深层 `EXPLICIT` 可以重新加入被祖先 REMOVE 的 entity；非显式继承不会穿透 tombstone。
- 同一 scope 的 `role + ordinal` 指向多个 profile/reference set 时，不选任意赢家，产生 `CONTEXT_PROFILE_ORDINAL_CONFLICT` 或 `CONTEXT_REFERENCE_ORDINAL_CONFLICT`，该 slot 不输出，`partial=true`。
- Scene/Style 是 single-value role；有效 merge 后出现多个值产生 `CONTEXT_SINGLE_ROLE_CONFLICT`。
- Character/Prop 是 multi-value role，最终按 ordinal ASC、entity id ASC 排序。

## 4. ShotReferencePack

`ShotReferencePack` 是有效 Profile/ReferenceSet intent 的强类型聚合，包含：

- shot_id、characters、scene、props、style；
- reference_sets（role、ordinal、required、content hash）；
- deterministic `PromptContext`；
- 全部有效 binding 的 `SourceTrace`。

Pack 不保存模型专属 GenerationValues，也不运行 workflow/recipe。Profile 默认 ReferenceSet 与 binding ReferenceSet 都进入统一 merge/expand 路径。

## 5. ReferenceSet expansion

有效 ReferenceSet 的 items 批量读取后展开成 `ResolvedReferenceAsset`：

```text
asset_id, sha256, role, ordinal,
source_reference_set_id, source_profile_id?, source_scope
```

每个 asset 必须存在、属于当前 project、且 `AssetType::Image`；否则产生 ERROR diagnostic 并使 context partial。排序固定为：

```text
role rank CHARACTER, SCENE, PROP, STYLE, SHOT_REFERENCE
→ binding ordinal
→ ReferenceSetItem.ordinal
→ asset_id
```

重复 concrete asset 只保留首次稳定出现的条目。

## 6. Prompt Context Builder

调用方不能自行拼接，Builder 固定九段：

```text
1 Global Style
2 Scene
3 Character
4 Costume
5 Props
6 Shot Action
7 Camera
8 Lighting
9 Output Specification
```

空文本不生成 segment；同来源同文本去重；正向 `rendered_text` 用单个 `\n` 连接。negative prompt 独立收集，顺序为 Style→Scene→Character ordinal，并做 normalized exact dedupe。

内容来源规则：

- Style：style_prompt、color_prompt、line_prompt、output_notes；
- Scene：SceneProfile.environment_prompt；无 SceneProfile 时使用所属 ProductionScene.description，来源为 LEGACY；lighting 只读取 SceneProfile.lighting_prompt；
- Character：canonical_prompt 与可选 CostumeVariant.prompt_fragment；
- Prop：canonical_prompt、material_prompt、scale_prompt；
- Shot Action：当前 stage prompt，缺失/空白时才 fallback 到 legacy Shot prompt；Image 不读取 Video prompt，反之亦然；
- Camera：仅读取现有 stage scalar 中明确的 `camera` 字段，不猜测；
- Output：只读取 width、height、count、durationSeconds/duration_seconds。

## 7. Revision 与 live fallback

Profile 有 `active_revision_id` 时，Resolver 批量读取 revision，验证 profile type/id 与 `content_sha256`。缺失或 hash 不匹配产生 ERROR；仍保留 live profile 内容用于可解释的 partial context。

没有 active revision 时使用 live profile JSON hash，并产生 `CONTEXT_PROFILE_REVISION_MISSING` WARNING。Revision ID、content hash 和 source trace 都进入输出；ReferenceSet 使用 ordered item + asset sha256 的 `ReferenceSetContentHash`。

## 8. Legacy 与 new-pack precedence

当没有有效新的 ReferenceSet binding 时，Resolver 兼容旧项目：

- `legacy.has_reference_pack=false`；
- 当前 stage 的 `shot_reference_assets` 按 legacy ordinal 展开；
- 有 legacy reference 时 `legacy.uses_legacy_shot_references=true`；
- legacy Shot prompt 仍可作为 Shot Action fallback。

只要存在有效的新 ReferenceSet binding，新 pack 优先，不无条件追加 legacy references。仅 profile binding 而没有有效 ReferenceSet 时，不会错误地抑制 legacy references。

## 9. Context hash

`ContextHashInput` 只包含影响生成内容的稳定字段：project/structure IDs、stage、effective profile IDs/content hashes、costume IDs、ReferenceSetContentHash、ordered concrete asset IDs/sha256、ordered prompt segments、negative prompt、workflow version、recipe、scalar values、output spec。

使用现有 `sha2` SHA-256，JSON 序列化前已固定集合顺序。`resolved_at`、随机 ID、diagnostic message、HashMap iteration order 不进入 hash。因此相同输入连续解析的 `context_hash` 相同，而 profile prompt、stage prompt、asset sha256、item order、workflow/recipe/scalar/output 改变会使 hash 改变。

## 10. Diagnostics 与范围

Context diagnostics 只有 WARNING/ERROR，不使用 READY、INCOMPLETE、BLOCKED 或 score。至少覆盖 project/shot/profile/reference/asset/costume/revision/single-role/conflict 错误码。

DEV-049 明确不包含：

- Shot Readiness / Preflight / Comfy capability gates；
- Production Preparation、snapshot、queue、batch/run 写入；
- Krea2/H3 stage adapter；
- Tauri command、UI、GenerationService、ProductionQueue/Orchestrator 改动。

上述能力以本文件定义的 `ResolvedShotContext` 为输入，进入 DEV-050 及后续任务。

## 11. DEV-054 integration boundary

Creation 一致性页通过 `shot_context_draft_get` 读取当前 Shot 的 resolved preview；该 command 只调用现有 `ShotContextResolver`，不运行 Comfy live preflight、不创建 queue/task，也不把继承 merge 逻辑复制到前端。Scope/Shot binding 的 GET 与 atomic replace 由 binding service 负责，解析结果仍以本文件定义的 source trace、legacy、partial、diagnostics 和 context hash 为准。

Production Preparation 在用户明确准入时冻结 resolved context、prompt、reference order、stage input、readiness、workflow/recipe 和最小 Comfy capability evidence。后续 Profile 或 ReferenceSet 修改不改变已冻结 snapshot；没有有效新 ReferenceSet binding 的旧 Shot 继续使用 legacy reference path。
