# DEV-054 — Narrative Production V1 Integration

状态：已实现，待 DEV-055 Release Gate
产品兼容版本：0.6.2
集成范围：DEV-047 至 DEV-053 的 AI 漫剧生产闭环

## 1. 目标与边界

DEV-054 将一致性数据、镜头上下文、就绪度、生产准备、现有队列、人工审核和审计连接成一条可操作路径：

    Project → Series → Episode → Scene → Shot
            → Profile / Costume / ReferenceSet
            → Scope / Shot Binding
            → ResolvedShotContext
            → Readiness / Preparation
            → 现有 ProductionBatch / ProductionQueue
            → 用户手动 Start
            → ComfyUI
            → 候选选择 / H3 视频 / A-B Review
            → Snapshot / Production Audit

本次集成没有创建第二执行器、第二队列、第二 ProductionRun、Scheduler、自动启动或无人值守生成。正式执行边界仍为：

    Existing ProductionQueueService
      → Existing GenerationService
      → Existing WorkflowCompiler
      → Existing Comfy Adapter
      → ComfyUI

## 2. 已落地能力

### 2.1 Binding backend

- Project、Series、Episode、Scene 使用统一 ConsistencyScopeBindingService。
- Shot 使用新增 ShotConsistencyBindingService。
- Profile binding 支持 Character、Scene、Prop、Style；Character 可带 CostumeVariant。
- ReferenceSet binding 支持 Character、Scene、Prop、Style、ShotReference。
- 保存接口使用稳定 camelCase wire DTO；role、profileType、inheritanceMode、scopeType 仍使用约定的 SCREAMING_SNAKE_CASE 值。
- 普通写入提供 EXPLICIT、REPLACE、REMOVE；INHERITED 只用于既有数据的读取和展示。
- Scope 与 Shot 的 profile/refset pack 使用 SQLite combined transaction；失败时不留下半个 binding pack。
- 后端生成 binding ID 和时间戳，更新时保留 createdAt。
- 所有写入继续做 project、owner、profile type、costume、ReferenceSet purpose 和 ordinal conflict 校验。

### 2.2 Context and preparation continuity

- shot_context_draft_get 直接复用同一个 ShotContextResolver，不运行 Comfy live preflight。
- Resolver 一次读取结构树和项目范围 binding，再按 Project → Series → Episode → Scene → Shot 合并。
- SourceTrace 保留每个有效 profile、ReferenceSet 的来源层级。
- ReferenceSet 展开为稳定排序的 concrete image assets，并进入 context hash。
- Video stage 将 Shot 的 selected image asset 与 sha256 放入 ResolvedStageInput 和 context hash。
- 无有效新 ReferenceSet 时保留旧 Shot prompt/reference 的 legacy fallback。
- Preparation snapshot 保存 resolved context、readiness、prompt、ReferenceSet/Asset 顺序、workflow/recipe、output、stage input 和最小 Comfy capability evidence；Profile 后续修改不会改变已冻结快照。

### 2.3 Creation / Production / Review IA

- 不新增全局“一致性”或“绑定” rail。
- Creation workspace 的 Project、Series、Episode、Scene scope 显示 ScopeConsistencyWorkspace，呈现 ancestor direct bindings、当前层 direct bindings、继承关系和保存后的 backend truth；这些上层页面不冒充最终 Shot ResolvedShotContext，也不展示镜头级 context hash。
- Shot creation workspace 增加“一致性”子页，支持 Shot binding、Costume、ReferenceSet，并通过 `shot_context_draft_get` 展示镜头级 ResolvedShotContext、当前 stage 的 prompt 和 context hash。
- Preparation、Queue、Review 仍分别位于 Production、Review workspace；创建页不负责启动生成。
- Dirty binding 草稿离开 scope 时有导航保护。

### 2.4 Command Center / Audit

- Project Command Center 增加 consistency summary、preparation summary、quick actions 和非阻塞 legacy 状态。
- Production Audit 增加 preparation snapshot lineage、context hash、activity 和按需历史 snapshot detail。
- Audit detail 以历史 snapshot 为事实来源，不重新解析当前 Profile，也不触发 Comfy、Queue 或生成写入。

## 3. Backward compatibility

- 不新增 migration 025；最大 migration 仍为 024。
- Product version 仍为 0.6.2；Manifest 仍为 2；Backup 仍为 14。
- 旧项目、旧 Shot prompt、旧 Shot reference assets 和旧 stage config 继续走 legacy path。
- 没有 binding 的旧 Shot 不会因为缺少 Reference Pack 而无法解析。
- 没有修改 001–023 的 checksum 或历史数据语义。

## 4. 验证记录

DEV-054 targeted checks：

| 检查 | 结果 |
| --- | --- |
| dev054_consistency_bindings | PASS — 3 tests |
| dev054_command_center_audit | PASS — 3 tests |
| dev054_narrative_integration | PASS — 1 test |
| ConsistencyBindingEditor + ScopeConsistencyWorkspace | PASS — 6 tests |
| ShotCreation + ReviewCompare + ShotBatchReviewBoard | PASS — 31 tests |
| ProjectCommandCenter + ProductionAuditCenter | PASS — 23 tests |
| cargo check | PASS |
| cargo fmt --all -- --check | PASS |
| cargo test -- --test-threads=1 | PASS — all targets |
| pnpm test | PASS — 92 files / 345 tests |
| pnpm exec tsc --noEmit | PASS |
| pnpm build | PASS |
| git diff --check | PASS |

dev054_narrative_integration 使用真实 SQLite 和公开应用服务验证：层级 binding、REMOVE 后 Shot 显式重加、prompt/hash 变化、Video selected image、legacy fallback、500-shot batch limit，以及准备 snapshot 的不可变历史值。

DEV-049–053、备份/Manifest 兼容性与 Creation/Review 回归均已通过；完整 Rust 与前端门禁也已通过。

GITHUB_CI = NOT_CONFIGURED
LOCAL_VALIDATION = PASS

## 5. Deferred to DEV-055

- 0.7.0 version bump。
- Release gate、升级/备份最终验收、500-shot 目标规模最终门禁。
- CI/GitHub Actions 配置（当前仓库未配置）。

## 6. Final boundary

人工选择候选、人工选择视频、人工审核、人工加入 ProductionBatch、人工启动 Production Queue 均保留。DEV-054 的交付重点是“准备正确、输入可解释、资产可复用、执行可追踪”。
