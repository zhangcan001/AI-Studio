# MiniMax Video V2 Phase 2 交接

日期：2026-10-08。状态：**PARTIAL / 尚未完成最终验收**。

用户因额度不足要求先推送最新代码并交接。本提交是可恢复的工作检查点，
**不是 Phase 2 PASS、发布或 Native 验收证明**。推送后停止，不提前执行 Phase 3–7。

## 基线与提交定位

- 项目：AI Studio，Tauri 2 / React / TypeScript / Rust / SQLite。
- 分支：`master`；远端：`https://github.com/zhangcan001/AI-Studio.git`。
- 开始 HEAD / Phase 1 最终基线：`8a0d037443dcc66f804379357da13c5e6bc92f74`。
- 开始 worktree 干净；交接前 fetch 确认 `HEAD...origin/master = 0 / 0`。
- 本检查点提交主题：`feat: checkpoint MiniMax Phase 2 inputs and audit corrections`。
  以 `git log -1 --format="%H %s" -- docs/handoffs/minimax-video-phase2-handoff.md`
  查找实际提交 SHA，避免在文件中写入无法自包含的自身 hash。
- Source-only CI 在 push 后自动触发；**结果尚未确认，不得使用 Phase 1 旧 CI 替代**。

## 不可破坏的边界

1. 单一 Production Queue 执行权威、Studio Store 草稿权威不变。
2. 新正式生成仍限四个授权 MiniMax H3 Base 2.2.0 视频包；新生图及 retired retry 拒绝。
3. Runtime Package 文件、migration `001–042`、Phase 1 及更早证明 JSON/hash 不得改写。
4. 输入身份是精确 `workflowVersionId + recipeId`，不按名字或排序猜测槽位。
5. 纯素材导入/输入保存不能创建 Task、Batch、GenerationLink 或伪造 Image Stage 成功。
   用户明确执行正常 Production Preparation 时，旧 Queue 的真实视频 planned link
   （task_id=NULL，真实 batch_item）是合法行为，不应误删。
6. 只做必要输入表单，没有三栏工作台/分镜卡片等 Phase 3 UI 重构。
7. 不修改真实用户数据库；migration、备份、删除、目录和媒体测试均使用 owned fixtures。

## 已实现的代码地图

| 范围 | 关键文件 / 行为 |
|---|---|
| 领域设计 | `docs/architecture/minimax-video-phase2-design.md`，实现前审议最小增量模型 |
| Schema | `src-tauri/migrations/043_shot_video_inputs.sql`：3 表，exact Recipe header、显式 input_key/ordinal、可信导入 receipt；当前表数74 / max43 |
| 持久权威 | `application/ports/shot_video_input_repository.rs`、`repositories/shot_video_input.rs`：instance+revision CAS、事务一致读/写、清空保留 header、ABA 防护 |
| 输入协调 | `application/shot_video_input_service.rs`：项目/镜头/Recipe 归属、来源、受管理文件、SHA/元数据复核；不写结果/任务 |
| 来源/安全导入 | `source_asset_import_service.rs` 及 `source_asset_import_service/source_paths.rs`：PNG/JPEG/WebP 全解码、纯批量/递归文件夹、video/audio probe、类型签名、限额/部分失败、补偿清理 |
| 四模式组装 | `shot_workflow_compatibility.rs`、`shot_service.rs`、`shot_service/creation.rs`、`shot_batch_service.rs`；Scene 复用现有 Batch Service |
| 最终约束 | `generation_input_preparer.rs`、`generation_service.rs`、`production_queue_service.rs`；仍调用现有 H3 `validate_resolved` / `validate_reference_durations`，没有竞争数字规则 |
| Readiness | `shot_context_resolver.rs`、`shot_readiness_evaluator.rs`：正式输入独立于旧生图结果/至少2参考图规则，exact 输入及 token 纳入上下文 |
| Backup21 | `project_backup_service.rs`、其 `video_inputs.rs`、`repositories/project_backup/video_inputs.rs`；支持旧 v1–20、映射恢复 identity、旋转 OCC instance |
| 删除 | `asset_deletion_service.rs`、`repositories/asset_deletion.rs`、新 composite FK：引用素材保护，整项目删除走显式事务 |
| 最小前端 | `usePersistentVideoInputs.ts`、`ShotVideoInputsPanel.tsx`；Recipe/Project/Shot owner+epoch 异步保护，脏输入禁止生成，导入不自动填槽 |
| Transport | typed `tauriClient.ts` / Product Client；新高级表单使用后端编码 opaque selectionRef，无新增 legacy importer allowlist |
| Successor | `scripts/minimax-video-phase2-successor-guard.mjs`、`docs/architecture/minimax-video-phase2.json`；先验证 live 字节，再向旧 reader 投影合法父字节 |

上述 Rust 路径均位于 `src-tauri/src/`（repositories 在 `infrastructure/database/`）。
前端 hook 在 `src/features/create/`，面板在 `src/features/shots/`。

### 输入/结果语义

- T2V 不需要图片；I2V 必须外部首帧；FIRST_LAST 分别保存 first_frame / last_frame。
- REF2VA 分别保存 reference_images / reference_videos / reference_audios 和稳定 ordinal。
- video-only 可准备；audio-only 在正式 compile/production 前拒绝。
- 保存允许不完整草稿；不能把“保存了纯音频草稿”误报成“正式 audio-only 生成获准”。
- 新图片必须有后端可信导入 receipt、source_image、source_task_id=None 及真实文件完整性。
- 历史生成图片/无 receipt 的旧 source_image 仍可读，但不自动作为正式新视频图片输入。
- Recipe 切换保留各自输入；不跨 Recipe 复制，不自动首帧填尾帧。
- 已准备批次和历史任务使用冻结 values/snapshot，之后镜头编辑不能静默改变它们。

### Raw bytes / successor 重要说明

现有 checkout 的 Runtime Package 实际为 CRLF，但 Git blob 为 LF，Git status 原本干净。
**未改写任何真实包字节**。新 guard 固定两套显式 tree digest：

- Git/新 CI checkout（默认）：`30c02a776b62d0723272d1f41f5d4e612b54e0e01b898189ff77b570e760eccf`
- 原本地 checkout：`bb5561badae002819a55966d9975295d030878d6a1447c378335dc8128e37b2e`

本地验证必须设置 `$env:AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='checkout'`；
新 clone/CI 默认 `git`。不能自动根据 live bytes 选择 profile，也不能“修复”/格式化包文件。
负向测试会拒绝单纯换行字节变化及非法 package 文件。

当前证明以 Phase 1 最终提交作为 parent/planningParent，
`phaseCommitsBeforeCheckpoint=[]` 对这一个首个 Phase 2 检查点是真实的。
schemaChanged / backupFormatChanged 在 Phase 2 为 true，在 Phase 1 仍 false。
后续若修复并产生新阶段提交，必须按照实际提交记录构建合法 successor，
不能沿用空提交链、改写 Phase 1 hash 或盲目 repin 历史聚合。

## EXTERNAL_AUDIT_CORRECTIONS

以下是**工作检查点状态**，不替代最终 exact-HEAD gate：

| 项目 | 状态 | 已有证据 / 剩余 |
|---|---|---|
| 1 数据库持久化 | PASS（定向） | 043 三表、74表/max43、backup21；input repo8测试、backup roundtrip/旧20、pool6测试；真实042和043升级 fixture，未全局替换旧数字 |
| 2 四模式分类+实际组装 | PASS（定向） | classifier3、核心输入/混合媒体/Scene测试；video-only通过、audio-only批次写前拒绝；共享 H3 约束 |
| 3 successor与负向探针 | PARTIAL | 新证明已生成、Phase1不变；早期 Phase2负向3通过，最新历史投影35通过；交接前架构全门禁PASS，最终全套负向仍须重跑 |
| 4 纯文件夹导入 | PASS（定向） | source import16测试、真实 Windows file+directory symlink1通过；递归/穿越/限额/部分失败、不调用 H3 Local commit |
| 5 输入结果隔离 | PASS（定向） | 来源拒绝/Recipe隔离/OCC/冻结核心测试、hook4、panel2；导入/save零生成副作用；合法 planned video link另行区分 |
| 6 完整验收 | PARTIAL | 最新扩展 single-shot用例未执行；全前端/全Rust/最终 exact-HEAD CI 未全部完成，不能标记 Phase2 PASS |

专门回归文件：

- `src-tauri/src/infrastructure/database/repositories/shot_video_input/tests.rs`：
  `explicit_slots_multimedia_order_and_restart_are_persistent`、
  `recipe_pair_isolation_and_clear_keep_occ_identity`、
  `concurrent_writes_have_one_winner_and_no_lost_update`、
  `cross_project_scope_and_invalid_slot_replacement_roll_back`、
  `asset_deletion_is_inspected_and_blocked_but_project_cascade_is_safe`、
  `migration_043_upgrades_v42_without_inventing_inputs_or_receipts`。
- `src-tauri/tests/support/minimax_video_input_regressions.rs`：independent FIRST_LAST、
  mixed/video-only/audio-only、tampered/history source、冻结、Scene四模式和 stale plan。
- `src-tauri/src/application/project_backup_service.rs`：inline `video_input_tests`，
  真实旧20读取、新21 remap/rotation/roundtrip、非法记录拒绝。未扩大生产 SQL allowlist。
- `src/app/MiniMaxVideoPhase2Boundary.test.ts`：伪造证明、未声明改动、非法增加、缺失证据、literal package 字节负向。

## 验证账本（不要扩大结果范围）

| 检查 | 实际结果 |
|---|---|
| 定向 input repo / classifier / readiness / pool | 最近22 PASS（8+3+5+6） |
| 来源导入 / backup full | 16 PASS / 63 PASS（后续仅测试组织等改变，仍需最终全套） |
| Phase1+Phase2四模式集成 | 前次11项10 PASS、1 fixture误判 planned link；修正后 Scene四模式单测 PASS |
| 最新单镜头扩展 | **NOT VERIFIED**：同一 Scene四模式测试新增实际 ShotService高级+normal Create准备、stale client字段剔除、最终preflight；尚未运行 |
| 首次全前端 | 1198 PASS、6 FAIL、1既有SKIP；6项为历史live schema/IPC/probe断言，已诊断修正 |
| 修正后定向前端 | 5文件 / 35 PASS（publication、backend、Phase7、Library、WorkflowLab） |
| 最新全前端重跑 | **NOT RUN** |
| 架构全门禁 | **交接前最新 PASS**，raw invoke0、typed RPC parity340；仅本文件验证账本文字在门禁后更新，运行源码未再改变 |
| TSC | **交接前最新 PASS**（包含最新边界测试修改） |
| Rust fmt | **交接前最新 check PASS**（包含最后test-only扩展） |
| Rust all-targets check | 最新 PASS（57.57s）；不可替代全Rust测试 |
| Windows路径安全 | **FILE_AND_DIRECTORY_SYMLINK_FIXTURE=PASS**，真实2种symlink，非跳过/fallback |
| 最新全Rust | 用户暂停时编译刚开始；owned进程已Ctrl-C停止，**INTERRUPTED / NOT VERIFIED** |
| Vite build / exact-head CI | 本阶段最终结果 **NOT VERIFIED** |
| Windows Native / 真GPU四模式 / installer | **NOT RUN**；明确留到Phase7，不要求本阶段重跑四轮GPU |

本地详细日志为 ignored `.codex-phase2-*.log`，不会提交私密环境日志。
要点：`.codex-phase2-projection-regressions.log`、`.codex-phase2-links-final.log`、
`.codex-phase2-rust-check-final.log`、`.codex-phase2-full-frontend.log`、`.codex-phase2-full-rust.log`。
交接前补充日志：`.codex-phase2-handoff-architecture.log`、
`.codex-phase2-handoff-tsc.log`、`.codex-phase2-handoff-fmt.log`。

## 继续工作顺序

先等待用户明确继续；不要因这个交接文件自动继续长门禁。

1. 查看当前 AGENTS、Git status/HEAD/remote/fetch，保留用户工作，不 reset。
   当前 checkout/代码为真相，memory仅历史；Ponytail一会话只读一次，Serena先读manual。
2. 每条编译/测试前检查 RAM/GPU，确认无另一 runner；保守 `-j1` / 单Vitest worker，
   不杀用户ComfyUI/应用、不开并发构建。暂停时已确认owned runner全部停止。
3. 先运行最新扩展四模式测试：

   ```powershell
   $env:AI_STUDIO_LIVE_COMFY='0'
   cargo test --manifest-path src-tauri/Cargo.toml --test dev061b_queue_recovery scene_preparation_supports_all_four_modes_without_image_result_selection_or_generation -j1 -- --test-threads=1
   ```

4. 逻辑检查点串行全门禁，不以旧结果或 targeted pass代替最终完整结果：

   ```powershell
   $env:AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE='checkout' # 仅现有本地CRLF checkout
   pnpm test --maxWorkers=1 --no-file-parallelism
   pnpm test:architecture
   pnpm exec tsc --noEmit
   pnpm exec vite build
   cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
   cargo check --manifest-path src-tauri/Cargo.toml --all-targets -j1
   cargo test --manifest-path src-tauri/Cargo.toml --all-targets -j1 -- --test-threads=1
   ```

5. 失败先根因分析、小修、定向验证；不skip、放宽allowlist、改RuntimePackage或盲目重试。
   修复源文件后处理真实 successor提交链；旧 proof的 false不变量保留。
6. 审查diff、fetch divergence、安全commit/push，核对最终HEAD的 Source-only CI。
   若无源码修改且当前检查点全门禁/CI均PASS，可复用同HEAD结果，不重复重型测试。
7. 用用户完整模板输出 `AI_STUDIO_MINIMAX_VIDEO_V2_PHASE_2_RESULT`，包括六项
   `EXTERNAL_AUDIT_CORRECTIONS`、准确文件/测试/提交/CI证据。全部源码门禁+最终HEAD CI
   通过才可 `STATUS=PASS`。之后停止；Phase3需另行授权。

## Key Learnings

1. 外部输入是持久媒体权威，不是生图成功；正常Queue planned link也不是伪造结果。
2. 合法 successor先验证新阶段，再投影旧阶段字节；不能让旧reader直接校验改动后的live schema。
3. Git normalized clean不代表literal package bytes相同；原包不写，profile必须显式。
