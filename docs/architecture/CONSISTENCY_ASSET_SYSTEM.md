# Consistency Asset System

状态：DEV-046 冻结设计
适用版本：AI Studio 0.7.0 / Narrative Production V1

## 1. 目标和边界

Consistency Asset System 解决“同一个角色、场景、道具和风格在多个镜头中可复用且可解释”的问题。它是语义层，不是第二套媒体库，也不是生产执行器。

目标：

- 用 Profile 表达稳定的语义身份和 prompt 规则。
- 用 Asset 表达可校验的物理媒体。
- 用 ReferenceSet 表达可复用、可排序的参考素材集合。
- 用 shot binding 表达某个镜头实际使用哪些语义实体。
- 让一个镜头可以有多个角色实例、多个道具和一个有效场景/风格上下文。
- 在生产准备时冻结 revision、asset ID、checksum 和顺序。

不在本设计中做：

- 删除或替换 0.6.2 ReferenceAnchor。
- 将图片文件复制进 Profile。
- 图数据库、事件溯源、云端同步。
- 自动为旧 Shot 猜测角色或场景。

## 2. 术语决定

| 术语 | V1 定义 |
| --- | --- |
| Asset | assets 表中的物理文件，拥有 storage path、sha256、mime、尺寸和来源任务 |
| Profile | 一个可复用语义实体，保存名称、描述、prompt fragments、默认关系和 revision |
| CostumeVariant | CharacterProfile 下的服装/造型变体，不是独立角色 |
| ReferenceSet | 有名称和用途的有序 Asset 集合，可被多个 Shot 绑定 |
| ReferenceAnchor | 0.6.2 的 kind → ordered image assets 低层兼容关系 |
| Binding | Shot 与 Profile 或 ReferenceSet 的显式关系，包含 role、ordinal 和覆盖策略 |

关键决定：Asset 是 physical media，Profile 是 semantic entity，ReferenceSet 是 reusable collection。三者不合并。

## 3. Profile 实体

### 3.1 CharacterProfile

最小字段：

- id
- project_id
- name
- description
- canonical_prompt
- negative_prompt
- default_style_profile_id，可空
- default_reference_set_id，可空
- active_revision_id，可空
- metadata_json
- created_at、updated_at

CharacterProfile 代表角色身份，不强行固定某一个服装。角色的多个服装用 CostumeVariant。

### 3.2 CostumeVariant

最小字段：

- id
- character_profile_id
- name
- prompt_fragment
- reference_set_id，可空
- is_default
- ordinal
- active_revision_id，可空
- created_at、updated_at

一个 Shot 可以绑定 character_profile_id + costume_variant_id；costume_variant 必须属于该 CharacterProfile。

### 3.3 SceneProfile

最小字段：

- id
- project_id
- name
- description
- environment_prompt
- lighting_prompt，可空
- negative_prompt，可空
- default_style_profile_id，可空
- default_reference_set_id，可空
- active_revision_id，可空
- created_at、updated_at

SceneProfile 描述空间身份和稳定视觉约束，不保存某一张生成结果。

### 3.4 PropProfile

最小字段：

- id
- project_id
- name
- description
- canonical_prompt
- material_prompt，可空
- scale_prompt，可空
- default_reference_set_id，可空
- active_revision_id，可空
- created_at、updated_at

### 3.5 StyleProfile

最小字段：

- id
- project_id
- name
- style_prompt
- color_prompt，可空
- line_prompt，可空
- negative_prompt，可空
- output_notes，可空
- active_revision_id，可空
- created_at、updated_at

StyleProfile 可以作为 Project 默认，也可以被 Scene 或 Shot 显式覆盖。

## 4. ReferenceSet

ReferenceSet 是“可重复使用的具体素材集合”，不是 Profile 的别名。

字段：

- id
- project_id
- name
- purpose：CHARACTER、COSTUME、SCENE、PROP、STYLE、SHOT
- description
- owner_profile_type，可空
- owner_profile_id，可空
- active_revision_id，可空
- created_at、updated_at

ReferenceSetItem 字段：

- reference_set_id
- asset_id
- ordinal
- role，可空，用于 FACE、FULL_BODY、ENVIRONMENT、DETAIL 等展示提示
- is_primary
- created_at

约束：

- 一个 ReferenceSet 内 asset_id 不重复。
- ordinal 稳定且从 0 开始。
- Profile/ReferenceSet/Asset 必须属于同一个 project。
- V1 只接受 image Asset 作为一致性参考；视频/音频继续使用现有工作流输入。
- 删除 Asset 前，AssetDeletionService 必须能报告 ReferenceSet、Anchor、Shot 和历史输出的使用关系。

## 5. ReferenceAnchor 的兼容策略

0.6.2 已有：

- reference_anchors(id, project_id, kind, name, normalized_name, description)
- reference_anchor_assets(anchor_id, asset_id, ordinal)
- reference_anchor_* commands 和 ReferenceAnchorPanel。

0.7.0 的策略：

1. 保留旧表、旧 command、旧 manifest 输出和旧 UI 入口。
2. 新 Profile/ReferenceSet 可以提供“从 Anchor 创建”适配操作。
3. 适配只复制关系定义，不复制媒体文件。
4. Anchor 的 kind 映射到 profile kind；Anchor 的 ordered assets 映射到 ReferenceSetItem。
5. 旧 Shot 继续读取 shot_reference_assets，不要求转换成 ReferenceSet。
6. 新 Shot 可以同时存在 legacy anchor reference 和 ReferenceSet binding；解析器必须按明确优先级合并并在冲突时报告。

优先级：

Shot explicit ReferenceSet / Profile binding > Scene binding > Episode binding > Series binding > Project default > legacy fallback。

legacy fallback 只用于兼容，不作为新功能的首选入口。

## 6. Shot 绑定模型

ShotProfileBinding：

- id
- shot_id
- role：CHARACTER、SCENE、PROP、STYLE
- profile_type
- profile_id
- costume_variant_id，可空
- ordinal
- inheritance_mode：EXPLICIT、INHERITED、REPLACE、REMOVE
- created_at、updated_at

ShotReferenceSetBinding：

- id
- shot_id
- role
- reference_set_id
- ordinal
- required
- inheritance_mode
- created_at、updated_at

角色和道具允许多条记录；场景和风格默认一个有效项。重复的 role + ordinal 或同层互斥绑定是冲突。

ShotReferencePack 是上述关系的统一 DTO，不再让 UI 拼接多张表：

~~~text
ShotReferencePack {
  characters: [
    { profileId, costumeVariantId?, ordinal, referenceSetIds[] }
  ],
  scene: { profileId?, referenceSetIds[] },
  props: [
    { profileId, ordinal, referenceSetIds[] }
  ],
  style: { profileId?, referenceSetIds[] },
  reference_sets: [{ id, role, ordinal }],
  prompt_context: {
    explicit_additions,
    explicit_removals,
    negative_prompt
  }
}
~~~

字段名在实际 DTO 中使用 camelCase；文档中的 snake_case 仅表示概念字段。

## 7. Profile Revision 决策

采用最小 Profile Snapshot / Revision，不做复杂版本控制系统。

ProfileRevision：

- id
- profile_type
- profile_id
- revision_number
- content_json
- content_sha256
- created_at
- created_by，可空
- status：ACTIVE、ARCHIVED

规则：

- 修改 Profile 生成新 revision，旧 revision 不覆盖。
- 未准备的 Shot 动态看到 active revision。
- Prepare/Start 将 profile revision ID 和 content hash 写入 Production Snapshot。
- 已准备或已启动的 ProductionBatch item、ProductionRun、Generation Snapshot 不重算。
- 旧 Shot 没有 revision/binding 时不强制升级。

选择这个方案是为了同时满足可复现和低复杂度；不引入 event sourcing、审计事件流或每字段 diff。

## 8. Asset Usage Graph

Asset Usage Graph 是查询概念，不是图数据库。

使用关系来自：

1. reference_set_items。
2. shot_reference_set_bindings。
3. shot_profile_bindings 关联的 profile default reference set。
4. shot_reference_assets。
5. reference_anchor_assets。
6. selected_image_asset_id、selected_video_asset_id。
7. task_output_assets 和 production history。

最小查询输出：

- asset_id
- usage_kind：PROFILE_REFERENCE、REFERENCE_SET、SHOT_REFERENCE、ANCHOR、SELECTED_OUTPUT、HISTORY
- owner_id
- owner_name
- shot_id / scene_id / episode_id，可空
- blocking：删除是否会破坏当前可生产关系

查询规则：

- 先按 project 过滤。
- 关系表一次 JOIN 或分段批量查询。
- UI 只加载摘要，详情按 owner 类型再请求。
- 不能为 1000 个 Asset 逐个调用 getAsset。

## 9. 继承与冲突

层级是 Project → Series → Episode → Scene → Shot。

- 标量 prompt：最近显式值胜出。
- Profile binding：默认 add + stable-ID dedupe。
- 明确 replace 会停止祖先同 role 继承。
- 明确 remove 会屏蔽指定 profile/reference set。
- 同层同 role 的两个不同 SceneProfile 或 StyleProfile 是 BLOCKER。
- Shot 显式角色可以覆盖祖先角色，但不删除其他 ordinal 角色。
- CostumeVariant 只能覆盖同一 CharacterProfile 的服装，不得把角色换成不相关 variant。

Resolver 必须返回来源链，例如：

~~~text
style.profileId = sty_1
source = project.default → scene explicit override
character[0].profileId = chr_2
source = episode binding → shot costume override
~~~

来源链是诊断数据，也是准备时写入 snapshot 的依据。

## 10. UI IA

当前 C UI 内新增三个子入口：

- Assets / Profiles：按 Character、Scene、Prop、Style 筛选。
- Assets / Reference Sets：查看集合、顺序、主图和使用者。
- Shot Inspector / Reference Pack：按角色、场景、道具、风格编辑 binding。

Profile detail 页面最少显示：

- 语义描述和 prompt fragments。
- 当前 revision。
- 默认 ReferenceSet。
- 被哪些 Shot/Scene 使用。
- 修改影响范围。

删除或修改前显示“被 X 个 Shot 使用”；历史 Production Snapshot 不允许被修改影响。

## 11. 服务和测试边界

ConsistencyProfileService 负责 CRUD、命名规范、project 边界、revision。
ReferenceSetService 负责有序 asset 关系、image 类型校验、usage summary 和 Anchor adapter。
ShotContextResolver 负责实际继承和 pack 合并，不由 React 计算。

最低测试：

- profile create/update/list/get。
- profile 与 asset project mismatch。
- ReferenceSet 顺序、去重、主图。
- Anchor adapter 保留 ordered asset IDs。
- multiple character ordinal。
- same-role conflict。
- replace/remove inheritance。
- revision edit 不改变 old snapshot。
- Asset 删除前列出 ReferenceSet/Shot/Anchor 使用。

## 12. 交付顺序

DEV-047 先交付数据契约和 repository contract。
DEV-048 再交付 Profile 与 ReferenceSet CRUD。
DEV-049 才接入 ShotReferencePack 和 Context Resolver。
正式 UI 和生产准备不得在 DEV-047 提前实现。

## 13. DEV-054 binding integration

DEV-054 通过 `ConsistencyScopeBindingService` 和 `ShotConsistencyBindingService` 暴露 scope/shot binding pack 的 GET 与 atomic replace。Scope 路径固定为 Project → Series → Episode → Scene；Shot binding 额外支持 SHOT_REFERENCE。Profile、CostumeVariant、ReferenceSet 的 project/role/ordinal/conflict 校验仍由后端负责，保存时间由 backend Clock 生成。

前端一致性编辑器只提交稳定的 camelCase binding DTO，新增关系提供 EXPLICIT、REPLACE、REMOVE；INHERITED 只读展示。`shot_context_draft_get` 复用同一个 `ShotContextResolver`，不在 React 中重写继承或触发 ComfyUI。已有 Profile/ReferenceSet CRUD 仍归 Assets 页面，binding UI 只负责选择和绑定。
