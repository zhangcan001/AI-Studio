# AI Studio 0.7.0 Logical ERD

状态：DEV-046 草案；不创建 migration 022 文件
基线 schema：001–021，0.6.2

本文件是逻辑数据模型，不是可执行 SQL。最终 migration 必须在 DEV-047/048 经过 repository、upgrade fixture 和 backup 设计后单独实现。

## 1. 设计原则

- 新表只增加一致性语义和 binding，不改旧表的生产语义。
- Asset 仍由现有 assets 表拥有；新表只保存 foreign key 和语义关系。
- ReferenceAnchor 保留。
- ShotReferencePack 是由 binding tables 组成的聚合 DTO，不复制全部展开后的资产。
- Readiness 和 ResolvedShotContext 默认派生，不在 V1 先存第二份事实。
- SQLite 关系和查询足以实现 Asset Usage Graph，不引入图数据库。
- profile revision 是最小 snapshot identity，不做事件溯源。

## 2. 关系图

~~~mermaid
erDiagram
  projects ||--o{ character_profiles : owns
  projects ||--o{ scene_profiles : owns
  projects ||--o{ prop_profiles : owns
  projects ||--o{ style_profiles : owns
  projects ||--o{ reference_sets : owns
  character_profiles ||--o{ costume_variants : has
  profiles ||--o{ profile_revisions : versioned
  reference_sets ||--o{ reference_set_items : contains
  assets ||--o{ reference_set_items : referenced
  shots ||--o{ shot_profile_bindings : binds
  shots ||--o{ shot_reference_set_bindings : binds
  character_profiles ||--o{ shot_profile_bindings : character
  scene_profiles ||--o{ shot_profile_bindings : scene
  prop_profiles ||--o{ shot_profile_bindings : prop
  style_profiles ||--o{ shot_profile_bindings : style
  reference_sets ||--o{ shot_reference_set_bindings : reused
  production_scenes ||--o{ scene_profile_bindings : defaults
  assets ||--o{ reference_anchor_assets : legacy
  reference_anchors ||--o{ reference_anchor_assets : legacy
  shots ||--o{ shot_reference_assets : legacy
~~~

profiles 是逻辑多态集合，不建议在 SQLite 创建一个无 FK 的 profiles 表；实际 profile tables 仍然分别拥有自己的外键。图中的 profiles 仅表示 profile_type/profile_id 逻辑关系。

## 3. 必需表

### 3.1 character_profiles — REQUIRED

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| id | TEXT PK | 建议 cp_ 前缀 |
| project_id | TEXT NOT NULL FK | 指向 projects |
| name | TEXT NOT NULL | project 内规范化唯一 |
| description | TEXT NOT NULL DEFAULT '' | 人可读说明 |
| canonical_prompt | TEXT NOT NULL DEFAULT '' | 角色稳定描述 |
| negative_prompt | TEXT NOT NULL DEFAULT '' | 角色负面约束 |
| default_style_profile_id | TEXT NULL | 可选风格 |
| default_reference_set_id | TEXT NULL | 可选角色参考集 |
| active_revision_id | TEXT NULL | 指向 profile_revisions |
| metadata_json | TEXT NOT NULL DEFAULT '{}' | 不放不可查询的核心关系 |
| created_at / updated_at | TEXT NOT NULL | UTC |

### 3.2 costume_variants — OPTIONAL / SHOULD

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| id | TEXT PK | 服装变体 |
| character_profile_id | TEXT NOT NULL FK | 必须属于角色 |
| name | TEXT NOT NULL | 角色内唯一 |
| prompt_fragment | TEXT NOT NULL DEFAULT '' | 服装/造型片段 |
| reference_set_id | TEXT NULL FK | 可选服装参考 |
| is_default | INTEGER NOT NULL DEFAULT 0 | 一个默认项 |
| ordinal | INTEGER NOT NULL | 稳定排序 |
| active_revision_id | TEXT NULL | 可选 revision |
| created_at / updated_at | TEXT NOT NULL | UTC |

若 DEV-047 只交付基础数据，可先建立表和 DTO，DEV-048 再开放 UI。

### 3.3 scene_profiles — REQUIRED

字段：id、project_id、name、description、environment_prompt、lighting_prompt、negative_prompt、default_style_profile_id、default_reference_set_id、active_revision_id、created_at、updated_at。

### 3.4 prop_profiles — REQUIRED

字段：id、project_id、name、description、canonical_prompt、material_prompt、scale_prompt、default_reference_set_id、active_revision_id、created_at、updated_at。

### 3.5 style_profiles — REQUIRED

字段：id、project_id、name、style_prompt、color_prompt、line_prompt、negative_prompt、output_notes、active_revision_id、created_at、updated_at。

StyleProfile 是最小全局上下文；它不是 workflow preset。

### 3.6 reference_sets — REQUIRED

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| id | TEXT PK | 集合 ID |
| project_id | TEXT NOT NULL FK | project scope |
| name | TEXT NOT NULL | project 内唯一 |
| purpose | TEXT NOT NULL | CHARACTER/COSTUME/SCENE/PROP/STYLE/SHOT |
| description | TEXT NOT NULL DEFAULT '' | 说明 |
| owner_profile_type | TEXT NULL | 逻辑 profile type |
| owner_profile_id | TEXT NULL | 由 service 校验 |
| active_revision_id | TEXT NULL | 可选 revision |
| created_at / updated_at | TEXT NOT NULL | UTC |

### 3.7 reference_set_items — REQUIRED

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| reference_set_id | TEXT NOT NULL FK | 集合 |
| asset_id | TEXT NOT NULL FK | 现有 assets |
| ordinal | INTEGER NOT NULL | 集合内顺序 |
| role | TEXT NULL | FACE/FULL_BODY/ENVIRONMENT/DETAIL |
| is_primary | INTEGER NOT NULL DEFAULT 0 | 主图 |
| created_at | TEXT NOT NULL | UTC |

主键建议为 reference_set_id + asset_id；唯一约束为 reference_set_id + ordinal。Asset 删除使用现有 delete inspection 加新关系查询。

### 3.8 shot_profile_bindings — REQUIRED

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| id | TEXT PK | binding ID |
| shot_id | TEXT NOT NULL FK | 现有 shots |
| role | TEXT NOT NULL | CHARACTER/SCENE/PROP/STYLE |
| profile_type | TEXT NOT NULL | 逻辑类型 |
| profile_id | TEXT NOT NULL | service 层验证同 project |
| costume_variant_id | TEXT NULL FK | 仅 CHARACTER |
| ordinal | INTEGER NOT NULL DEFAULT 0 | 多实例 |
| inheritance_mode | TEXT NOT NULL DEFAULT 'EXPLICIT' | EXPLICIT/INHERITED/REPLACE/REMOVE |
| created_at / updated_at | TEXT NOT NULL | UTC |

建议唯一约束：shot_id + role + ordinal + profile_id；同层冲突由 service 检查。

### 3.9 shot_reference_set_bindings — REQUIRED

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| id | TEXT PK | binding ID |
| shot_id | TEXT NOT NULL FK | 现有 shots |
| role | TEXT NOT NULL | CHARACTER/SCENE/PROP/STYLE/SHOT |
| reference_set_id | TEXT NOT NULL FK | reusable collection |
| ordinal | INTEGER NOT NULL DEFAULT 0 | role 内顺序 |
| required | INTEGER NOT NULL DEFAULT 0 | 是否为 readiness required |
| inheritance_mode | TEXT NOT NULL DEFAULT 'EXPLICIT' | 合并策略 |
| created_at / updated_at | TEXT NOT NULL | UTC |

## 4. 可选表

### 4.1 profile_revisions — OPTIONAL / SHOULD

字段：

- id TEXT PRIMARY KEY
- profile_type TEXT NOT NULL
- profile_id TEXT NOT NULL
- revision_number INTEGER NOT NULL
- content_json TEXT NOT NULL
- content_sha256 TEXT NOT NULL
- status TEXT NOT NULL DEFAULT ACTIVE
- created_at TEXT NOT NULL
- created_by TEXT NULL

由于 SQLite 无法对 polymorphic profile_id 建立一个真实 FK，ConsistencyProfileService 必须在 transaction 内验证 profile_type/profile_id。唯一约束建议为 profile_type + profile_id + revision_number。

ReferenceSet 如果需要独立 revision，可以把集合内容 hash 纳入 reference_sets.active_revision_id，或复用同一 revision contract；不要同时引入两种互不相容的版本模型。

### 4.2 scene_profile_bindings — OPTIONAL

如果 SceneProfile 不直接扩展 production_scenes，可以使用：

- scene_id
- profile_id
- inheritance_mode
- created_at / updated_at

第一版可以让 SceneProfile binding 走通用 shot_profile_bindings 的 scene defaults 查询；只有当 scene-level default 需要独立 CRUD 时再建立本表。

### 4.3 asset_usage_cache — OPTIONAL / DEFERRED

默认不建。Asset Usage Graph 先用 JOIN 查询。只有 10000+ assets 的实测查询仍不达标时，才考虑可重建的 cache；cache 不能成为唯一事实来源。

## 5. 延后表或明确不建的表

| 表/概念 | 分类 | 原因 |
| --- | --- | --- |
| shot_context_snapshots | DEFERRED | 复用 production_batch_items.values_json、production_runs.frozen_config_json、generation_snapshots.resolved_inputs_json |
| shot_readiness_cache | DEFERRED | readiness 是派生状态，先按需批量计算 |
| graph_nodes / graph_edges | DEFERRED | SQLite relations + query 已足够 |
| storyboard_documents | DEFERRED | Auto Storyboard 仅 P2 draft |
| script_import_documents | DEFERRED | TXT/Markdown/JSON draft 可先留在前端/临时 request |
| schedulers | FORBIDDEN | 不为 0.7.0 引入 unattended production |
| second_queue / second_executor | FORBIDDEN | 保持现有执行边界 |

## 6. 与 0.6.2 表的关系

保留并继续使用：

- projects
- assets
- shots
- shot_stage_configs
- shot_stage_prompts
- shot_reference_assets
- shot_generation_links
- reference_anchors
- reference_anchor_assets
- production_series
- production_episodes
- production_scenes
- shot_scene_assignments
- production_batches
- production_batch_items
- production_runs
- production_stages
- production_stage_items
- generation_snapshots

新表不改旧表必填字段。旧 Shot 的 pack/binding 可以不存在，Resolver 返回 legacy=true 并使用旧 prompt/reference/stage config。

## 7. Migration 022 草案

这是未来实现顺序，不是本次新增文件：

1. 创建 character_profiles、scene_profiles、prop_profiles、style_profiles。
2. 创建 costume_variants、reference_sets、reference_set_items。
3. 创建 shot_profile_bindings、shot_reference_set_bindings。
4. 可选创建 profile_revisions 和 scene_profile_bindings。
5. 添加 project-scoped、shot-scoped、reference-set ordinal indexes。
6. 不 backfill 旧 Shot，不删除或更新 reference_anchors。
7. 写入 _sqlx_migrations version 22 由实际 migration 执行，不在 DEV-046 伪造。

Forward-compatible 要求：

- 021 → 022 和 fresh 001 → 022 都能通过。
- 0.6.2 数据行数和 checksum 不因升级改变。
- 新表空时旧功能仍能读写。
- 删除/回滚前已有 backup 可恢复。
- Project Manifest version 1 继续输出旧字段。
- 未来 manifest version 2 再增加 profiles/referenceSets，不能让旧导出失效。

## 8. 索引草案

- character_profiles(project_id, updated_at, id)
- scene_profiles(project_id, updated_at, id)
- prop_profiles(project_id, updated_at, id)
- style_profiles(project_id, updated_at, id)
- reference_sets(project_id, purpose, updated_at, id)
- reference_set_items(reference_set_id, ordinal, asset_id)
- shot_profile_bindings(shot_id, role, ordinal, profile_id)
- shot_reference_set_bindings(shot_id, role, ordinal, reference_set_id)
- profile_revisions(profile_type, profile_id, revision_number)

查询必须先限制 project_id 或通过 shot → project 关系限制，避免跨项目误绑定。

## 9. 数据生命周期

- Profile 删除：若仍被 Shot、Scene default、Costume 或 ReferenceSet owner 使用，先返回 usage blocker；历史 snapshot 不受影响。
- ReferenceSet 删除：若被 Shot binding 使用，拒绝硬删除或要求先解除；其 items 不删除 Asset。
- Asset 删除：复用现有 inspection，补充新关系的 blockingReasons。
- Shot 删除：binding 级联删除；Profile/ReferenceSet/Asset 不级联删除。
- Project 删除：按现有 project cascade 规则清理新 project-scoped rows。

## 10. ERD 验收

实现前必须验证：

- 0.6.2 database 不执行人工 backfill 也能打开。
- 旧 Anchor 的 ordered assets 不变。
- 新 Shot 能绑定多角色、多道具和 ReferenceSet。
- Profile/ReferenceSet/Asset 跨项目关系被拒绝。
- 同一 ReferenceSet 的 ordinal 和 asset 不重复。
- revision content hash 可用于 contextHash。
- scene page 的主要查询是 set-based。
