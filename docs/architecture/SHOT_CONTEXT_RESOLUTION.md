# Shot Context Resolution

状态：DEV-046 冻结设计
适用版本：AI Studio 0.7.0 / Narrative Production V1

## 1. 目标

ShotContextResolver 是 0.7.0 的单一上下文入口。它把 Project/Series/Episode/Scene/Shot、Profile、ReferenceSet、Prompt Template、Stage Config、Workflow 和当前运行能力组合成一个可解释的 ResolvedShotContext。

Krea2 和 MiniMax H3 不得各自直接读取这些表。它们只消费：

1. Resolver 输出的 ResolvedShotContext。
2. 由 Context Adapter 生成的既有 GenerationValues。
3. 现有 workflow version、recipe 和 runtime capability。

Resolver 是动态读取服务；Preparation 才负责冻结生产输入。

## 2. 输入范围

Resolver 的最小输入：

- project_id
- shot_id
- stage：image 或 video
- mode：DRAFT、PREFLIGHT、PREPARE

内部读取：

- Project、production_series、production_episodes、production_scenes、shot_scene_assignments。
- Shot、stage config、stage prompt、legacy shot_reference_assets。
- ShotProfileBinding、ShotReferenceSetBinding。
- 当前有效 ProfileRevision。
- ReferenceSet 和 ordered ReferenceSetItem。
- Prompt Library / Prompt Template。
- Workflow version、Recipe、runtime scope。
- ComfyPreflightService 的当前或缓存 capability。

不存在的新表或 legacy 项目数据不应让 resolver 崩溃；它必须返回缺失原因和 legacy mode。

## 3. 继承算法

固定路径：

Project → Series → Episode → Scene → Shot

### 3.1 标量值

从 Project 到 Shot 依次合并。每个字段保存 value、source_scope、source_id 和 revision_id。后来的显式值覆盖前值。

示例：

~~~text
style.stylePrompt:
  project default = "2D ink"
  scene explicit = "night blue"
  shot absent
  result = "night blue"
  source = scene
~~~

### 3.2 集合值

默认策略为 add：

1. 按父到子顺序收集。
2. 按 role + stable profile/reference ID 去重。
3. 保留首次出现的 ordinal；Shot 显式 ordinal 可以重新排序。
4. replace 会清空指定 role 的祖先集合。
5. remove 会加入 tombstone，阻止被移除项再次从祖先注入。

### 3.3 冲突

以下情况直接生成 BLOCKER：

- 同一 scope 的同一 role + ordinal 指向不同 profile。
- CostumeVariant 不属于绑定的 CharacterProfile。
- SceneProfile、CharacterProfile 或 ReferenceSet 跨 project。
- ReferenceSet 有重复 asset、缺失 asset 或非 image asset。
- ProfileRevision 不存在或内容 hash 不匹配。

Resolver 不做静默猜测，不按名称模糊匹配来解决冲突。

## 4. Shot Reference Pack

Pack 是 shot-level 的统一输入契约，字段：

~~~text
ShotReferencePack {
  shotId
  characters: [
    {
      profileId
      costumeVariantId?
      ordinal
      referenceSetIds[]
    }
  ]
  scene: {
    profileId?
    referenceSetIds[]
  }
  props: [
    {
      profileId
      ordinal
      referenceSetIds[]
    }
  ]
  style: {
    profileId?
    referenceSetIds[]
  }
  referenceSets: [
    {
      id
      role
      ordinal
      required
    }
  ]
  promptContext: {
    explicitAdditions
    explicitRemovals
    negativePrompt
  }
}
~~~

Pack 保存的是 binding 和意图，不保存展开后的全部 Asset metadata。展开结果只进入 ResolvedShotContext 或 Production Snapshot。

多角色决策：

- 允许多个角色。
- 使用 role=CHARACTER 和 ordinal 区分角色实例。
- 同一个 CharacterProfile 可以在镜头中出现多次，但每个实例需要明确 ordinal。
- 不要求一个角色只能拥有一个 ReferenceSet；Resolver 按 binding 顺序合并并去重。

## 5. ResolvedShotContext

这是所有生成和 readiness 的统一输入 DTO，至少包含：

~~~text
ResolvedShotContext {
  project: { id, name, description }
  structure: {
    series?, episode?, scene?, shot: { id, ordinal, name }
  }
  stage: "image" | "video"
  profiles: {
    style?
    scene?
    characters: [
      { profileId, revisionId, ordinal, costumeVariantId?, prompt, negativePrompt }
    ]
    props: [
      { profileId, revisionId, ordinal, prompt, negativePrompt }
    ]
  }
  referenceAssets: [
    {
      assetId
      sha256
      role
      ordinal
      sourceReferenceSetId?
      sourceProfileId?
    }
  ]
  promptContext: {
    globalStyle
    scene
    characters
    costumes
    props
    shotAction
    camera
    lighting
    outputSpecification
    negativePrompt
    renderedText
    sourceTrace[]
  }
  workflow: {
    workflowVersionId
    recipeId
    workflowId
    runtime
    outputType
  }
  output: {
    width?
    height?
    durationSeconds?
    count
  }
  legacy: {
    hasReferencePack
    usesLegacyShotReferences
  }
  resolverIdentity: {
    contextHash
    profileRevisionIds[]
    referenceSetRevisionIds[]
    resolvedAt
  }
}
~~~

实际 wire DTO 使用 camelCase。contextHash 是 canonical JSON 的 hash，不包含当前时间。

## 6. Prompt Context Builder

顺序必须稳定，不能让调用方自行拼接：

1. Global Style
2. Scene
3. Character
4. Costume
5. Props
6. Shot Action
7. Camera
8. Lighting
9. Output Specification

每一段输出：

- label
- normalized text
- source scope
- source entity ID
- revision ID
- omitted reason，可空

渲染规则：

- 空段不生成多余逗号或空行。
- 同一 profile 的重复片段按 stable ID 去重。
- Character/Prop 顺序按 ordinal，再按 profile ID。
- Reference asset 顺序按 binding ordinal，再按 ReferenceSetItem ordinal。
- Shot prompt 明确写入 Shot Action 段，不覆盖 profile 的 identity。
- negative prompt 单独收集并去重。
- 超长 prompt 返回 WARNING 或 BLOCKER，不能静默截断。

现有 PromptTemplateContext 可以作为 legacy source，但不能成为第二套 resolver。Prompt Template 预览最终应调用同一套 context builder。

## 7. Stage Adapter

Resolver 先产出与模型无关的 context，再由 stage adapter 转换。

### Krea2 Image

- 输出类型必须是 image。
- 使用 promptContext.renderedText。
- 使用 image stage workflow version / recipe。
- 参考图可以是 ReferenceSet 展开的多图，只在 recipe 支持时注入。
- 不能把 video-only values 注入 image stage。

### MiniMax H3 Video

- I2V：需要一个已选 image asset，注入单个 image input。
- REF2VA：需要 plural reference_images，按 resolver 顺序注入，最小 2 张，最大值服从 recipe。
- 当前 ordered_reference_binding 的重复和边界校验继续生效。
- 视频输出 spec 至少包含 duration、width、height 或明确的 recipe default。
- 先检查当前 stage 的 workflow/runtime scope，再编译 values。

Adapter 的输出应是现有 ShotBatchService / ProductionQueueService 可接受的 GenerationValues；不得创建新的执行 API。

## 8. Readiness 与 Preflight

### 8.1 Readiness

聚合状态：

- READY：所有 required gate 通过；可以存在非阻塞 warning。
- INCOMPLETE：没有技术 blocker，但缺少用户必须补齐的信息。
- BLOCKED：存在不可执行、跨项目、冲突、workflow capability 或输出契约 blocker。

建议 score：

- 初始 100。
- 每个 INCOMPLETE gate 扣 15。
- 每个 WARNING gate 扣 5。
- 每个 BLOCKER gate 扣 35。
- 最终限制在 0–100。

score 只用于排序和显示，不能绕过 blocker。

### 8.2 七类 gate

| Gate | PASS 条件 | 常见 INCOMPLETE | 常见 BLOCKER |
| --- | --- | --- | --- |
| Character | 需要的角色和 costume variant 可解析 | 未绑定角色 | profile/revision 冲突或跨项目 |
| Scene | SceneProfile 或明确 legacy scene context | 场景语义未补齐 | scene/profile 不属于当前层级 |
| Reference | ReferenceSet 展开后 asset 存在、类型正确、顺序有效 | 可选参考未补 | 删除、跨项目、REF2VA 数量不满足 |
| Prompt | rendered prompt 非空且在限制内 | 缺 shot action | 变量缺失、超过硬限制 |
| Workflow | version/recipe 可用且输出类型匹配 | 尚未选择 workflow | recipe 解析失败或 runtime 不支持 |
| Output | image/video spec 完整且在 recipe 范围内 | 使用默认值待确认 | 宽高/时长违反硬限制 |
| Comfy Capability | 当前或缓存 capability 足够 | 尚未刷新 capability | offline、missing node、workflow blocked |

每个 gate 返回：

~~~text
ReadinessCheck {
  key
  state: PASS | WARNING | INCOMPLETE | BLOCKER
  message
  source
  entityIds[]
  fixAction?
}
~~~

### 8.3 后端权威

Frontend 可以做即时 UX 提示，但最终 readiness 必须由 Rust service 计算。Scene、Episode、Series 页面使用批量 endpoint；不得在 React 中复制 gate 逻辑后自行决定可生产。

## 9. 动态解析与 Production Snapshot

解析模式：

- DRAFT：快速返回当前字段和来源，允许不完整。
- PREFLIGHT：运行全部 gate，读取当前/缓存 Comfy capability。
- PREPARE：运行全部 gate，并输出可冻结的 context identity。

Prepare 成功后，进入现有生产边界时冻结：

- contextHash
- profile revision IDs 和 content hashes
- reference set revision IDs
- concrete asset IDs、sha256 和 order
- rendered prompt
- workflow version、recipe、runtime
- output specification

冻结位置优先复用：

- production_batch_items.values_json
- production_runs / production_stages.frozen_config_json
- generation_snapshots.resolved_inputs_json

不要为同一份事实再造一个 context_snapshots 表，除非后续性能实证证明现有 snapshot 不能承载。

## 10. 规划中的接口

~~~text
shot_reference_pack_get(project_id, shot_id)
  -> ShotReferencePack

shot_reference_pack_update({
  projectId, shotId, pack, expectedUpdatedAt?
})
  -> ShotReferencePack

shot_resolve_context({
  projectId, shotId, stage, mode: "DRAFT" | "PREFLIGHT" | "PREPARE"
})
  -> ResolvedShotContext

shot_preflight({ projectId, shotId, stage })
  -> ShotReadiness

scene_resolve_contexts({
  projectId, sceneId, stage, shotIds?
})
  -> { items: ShotContextSummary[] }

scene_preflight({
  projectId, sceneId, stage, shotIds?
})
  -> { items: ShotReadinessSummary[], counts }
~~~

这些是接口契约，不是 DEV-046 的实现。

## 11. 性能与错误处理

- 单 Shot 可以有完整 trace；批量 card 只返回摘要和 blocker codes。
- scene 页面采用一次 structure load + 批量 context/readiness，不逐卡 Tauri invoke。
- Context cache 必须以 source updated_at、revision IDs 和 stage 失效。
- asset 缺失、profile 删除、reference set 为空必须返回稳定 error code。
- 任何部分解析结果都标记 partial，不得被当成 READY。

## 12. 测试清单

- project → shot 的五级继承。
- shot override、replace、remove。
- 多角色、多道具 ordinal 稳定。
- same-role conflict 进入 BLOCKER。
- ReferenceSet 展开顺序和 checksum。
- I2V 单图、REF2VA 2–N 图约束。
- prompt builder 九段固定顺序。
- profile edit 后 draft context 变化，已冻结 production context 不变。
- legacy Shot 无 pack 时仍能读到旧 prompt/reference。
- 500 shots 批量 resolver 不出现 N+1。
