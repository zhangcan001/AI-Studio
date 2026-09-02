# DEV-073 — AI Studio 1.0 Product Freeze & Readiness Review

状态：`PASS — AI_STUDIO_1_0_READINESS`

本 DEV 只做产品冻结和发布准备审计，不新增功能、不修改业务代码、不新增 migration、不创建新的 queue/executor/task model，也不重新生成 DEV-071 的 24 个真实视频。审计执行时 `ACTIVE_SUBAGENTS=0`。

## 1. Decision

`P0=NONE`、`P1=NONE`。Production Core、Failure/Recovery、Data Integrity、Windows Build、Cold Start Smoke 和 Regression Gates 均通过，因此：

```text
AI_STUDIO_1_0_READINESS = PASS
NEXT_DEV = DEV-074
MUST_FIX_BEFORE_1_0 = NONE
```

## 2. Frozen baseline and release gap

```text
DEV073_START_SHA = 727d0b73945834499a6bdb57838fb88d302bb1d1
BRANCH           = master
ORIGIN_MASTER    = 727d0b73945834499a6bdb57838fb88d302bb1d1
WORKTREE         = CLEAN
```

已发布的 AI Studio 0.9.0 来源是 tag `v0.9.0` 的 peeled commit：

```text
RELEASED_V0_9_0_SOURCE          = 80448f37c640658d601f9507c33f92796cad9751
CURRENT_MASTER                  = 727d0b73945834499a6bdb57838fb88d302bb1d1
CURRENT_MASTER_CONTAINS_UNRELEASED_FEATURES = YES
```

`v0.9.0` 之后的 master 包含 DEV-071 生产试跑证据和 DEV-072 显式连续批次启动代码/修复/证据。因此不能把 GitHub `v0.9.0` 二进制描述为包含 DEV-072；DEV-072 属于当前 master 的未发布变更。

DEV-073 开始前的 Source-only CI 已核验：`33589092172`，`headSha=727d0b7…`，`completed / success`。任务模板中出现的 `33589093365` 在 GitHub 返回 HTTP 404，属于编号记录偏差，不作为成功 CI 证据。

## 3. Version, schema and architecture freeze

```text
SOURCE_VERSION             = 0.9.0
CARGO_VERSION              = 0.9.0
PRODUCTION_PACKAGE_SCHEMA  = 1
PROJECT_MANIFEST_VERSION   = 2
BACKUP_VERSION              = 15
MIGRATION                  = 026
MIGRATION027               = ABSENT
```

冻结架构复核结果：

| Contract | Frozen value | Result |
| --- | --- | --- |
| `FORMAL_EXECUTOR` | `COMFYUI` | PASS |
| `AUTO_START_ON_CREATE` | `NO` | PASS |
| `IMPLICIT_AUTO_NEXT` | `NO` | PASS |
| `EXPLICIT_USER_ARMED_NEXT` | `YES` | PASS |
| `AUTO_RETRY` | `NO` | PASS |
| `MAX_CONCURRENT_BATCH` | `1` | PASS |
| `SECOND_QUEUE` | `NO` | PASS |
| `SECOND_EXECUTOR` | `NO` | PASS |
| `SECOND_TASK_MODEL` | `NO` | PASS |
| `START_ALL` | `NO` | PASS |
| `SEQUENCE_RESTART_RESUME` | `NO` | PASS |

代码和回归检查确认 Production Package 复用既有 `ProductionBatch → ProductionQueue → GenerationService → ComfyUI` 路径；只有一个 `ProductionQueueService` 实例，没有 board 旁路 executor、隐式 scheduler 或第二套 GPU 路径。

## 4. Evidence audit: DEV-067 through DEV-072

| Evidence | Audit result |
| --- | --- |
| DEV-067 Production Package Quick Flow | PASS；Create 不创建 Task/Prompt，只有用户明确 Manual Start 才进入生产；restart queue truth 通过。 |
| DEV-068 Production Monitor & Deliverables | PASS；Queue/Monitor、播放、打开文件位置、selected-batch manifest guard 和 Asset truth 通过。 |
| DEV-069 Multi-Package Production Board | PASS；discovery、自然排序、READY/WARNING/BLOCKED gate、durable package key、partial create/resume、restart 去重、source removal 和 completion convergence 通过。历史 DEV-069D 失败记录被正确保留，最终 closure 以修复后的 PASS 为准。 |
| DEV-070 AI Studio 0.9.0 Release Gate | PUBLISHED；tag、Source RC、portable/NSIS/MSI 和 release smoke 通过。 |
| DEV-071 Real Project Production Pilot | PASS；真实 Season、3 个 package、24 个 item、24 个 H3/ComfyUI Task、24 个 video Asset 和 24 个非空输出文件闭环。 |
| DEV-072 Explicit Sequential Batch Start | PASS；显式 A→B→C 顺序、单 batch admission、最终 `IDLE` 和 D 未被隐式启动通过。 |

DEV-071 的真实生产事实：EP01/EP02/EP03 各 8 项，合计 24/24 Task、24/24 video Asset、24/24 文件存在，数据库 SHA 与文件 SHA `24/24` 匹配；`TASK_LINEAGE`、`ITEM_TASK_ASSET_JOIN` 和最终 ComfyUI queue 均通过。没有自动下一批、自动 retry 或第二执行器。

DEV-072 的真实顺序事实：

| Batch | Task started (+08:00) | Task finished (+08:00) | Final |
| --- | --- | --- | --- |
| A | `2026-09-02T11:46:36.999272+08:00` | `2026-09-02T11:48:39.948982100+08:00` | `COMPLETED`, 1/1 |
| B | `2026-09-02T11:48:43.139913100+08:00` | `2026-09-02T11:50:36.681667600+08:00` | `COMPLETED`, 1/1 |
| C | `2026-09-02T11:50:39.897547400+08:00` | `2026-09-02T11:52:18.317051400+08:00` | `COMPLETED`, 1/1 |
| D | `2026-09-02T11:54:46.805181200+08:00` | `2026-09-02T11:56:25.432223000+08:00` | `COMPLETED`, 1/1 |

`B_STARTED_AT > A_FINISHED_AT`、`C_STARTED_AT > B_FINISHED_AT`，无 batch overlap，最终 `FINAL_SEQUENCE_STATE=IDLE`，ComfyUI 最终 `running=0 / pending=0`。UAT 中用户曾误点 D，使其短暂显示等待 #3，但没有创建 D Task 或提交 ComfyUI；该事实已在 DEV-072 透明记录为 `P2=OPERATOR_DEVIATION_ONLY`，不构成产品 P0/P1。

## 5. Failure, recovery and data integrity

Failure/recovery evidence 覆盖 ComfyUI disconnected/busy/race、Task failure、partial/failure stop、pause、retry、restart、partial resume、chunk provenance、source moved/removed、duplicate import、manifest mutation、missing/invalid/WARNING/BLOCKED package、visibility/in-flight/unmount 和 terminal completion convergence。所有自动 retry、隐式 auto-start、隐式 auto-next 均保持关闭。

当前用户数据库仅作只读审计，未通过审计动作清理或启动历史数据：

```text
ACTIVE_TASKS                 = 0
PRODUCTION_BATCHES           = 105
  COMPLETED                  = 90
  PAUSED                     = 10  (historical UAT rows)
  READY                      = 5   (historical UAT rows)
PRODUCTION_PACKAGE_BINDINGS  = 16
MIGRATION_MAX                = 26
```

只读完整性查询结果：

```text
orphan_batch_projects        = 0
orphan_batch_items           = 0
orphan_item_tasks            = 0
orphan_task_outputs          = 0
orphan_output_assets         = 0
binding_missing_batch        = 0
binding_project_mismatch     = 0
binding_item_count_mismatch  = 0
duplicate_binding_keys       = 0
duplicate_binding_item_ids   = 0
package_task_output_missing  = 0
```

16 条 binding 的 `source_kind` 均为 `PRODUCTION_PACKAGE`；binding 的 batch、project、package item 数量和 Task/Asset output join 均可回读。DEV-071 的 24/24 真实 lineage 与 DEV-072 的 4 个真实 batch 进一步交叉验证了 Package → Batch → Item → Task → Asset → file 的映射。

ComfyUI 独立复核：`/system_stats=HTTP 200`，`/queue` 读取成功，`running=0`、`pending=0`。

## 6. Windows product audit and cold start

`pnpm tauri build` 在当前 master 成功，实际产物如下：

| Artifact | Bytes | SHA-256 | ProductVersion |
| --- | ---: | --- | --- |
| `src-tauri/target/release/ai-studio.exe` | 48,567,808 | `52DD1B8EEC55FFEBF7E6BBEB72F4F5FA56727A9590932EC6971BCB9AA2585951` | `0.9.0` |
| `src-tauri/target/release/bundle/nsis/AI Studio_0.9.0_x64-setup.exe` | 10,554,038 | `7CE97C492EEDDC75C8DE122F9F8996EE8B70286749C1AFB797DE648E71D67FA2` | `0.9.0` |
| `src-tauri/target/release/bundle/msi/AI Studio_0.9.0_x64_en-US.msi` | 16,789,504 | `980E51428A80D5A2AFDFD928E879090A3351D811ED33278396AAA2A07E481A39` | `0.9.0` |

```text
PORTABLE_BUILD = PASS
NSIS_BUILD      = PASS
MSI_BUILD       = PASS
```

当前 release executable fresh launch 成功，窗口标题为 `AI Studio - 本地 AI 创作工作台`；项目页面、生产模式、Production Package、批量生产包标签、Production Queue 和 Production Monitor 均已加载于同一工作区可访问树中，无 crash、无生产动作。Tauri WebView 的 Windows helper 在尝试用 element index 点击标签时返回 `coordinate input geometry is unavailable`，因此没有通过工具代替用户执行任何创建/启动动作；已有 DEV-069/072 真人 UAT 与当前前端路由/组件测试覆盖该交互路径。

Existing project smoke 采用隔离数据库快照 `C:\Users\ADMIN\Desktop\AI_Studio_DEV073_existing_project_smoke_20260902\app.db`，只读确认 `migration=026`、12 个 Project、105 个 Batch、16 个 package binding；DEV-070 的隔离 0.8.1→0.9.0 upgrade smoke、当前 master 的 `dev055_release_compatibility`（6 tests）和当前 build 的冷启动共同确认旧项目可读、迁移可达 026、Production surface 可打开。未使用用户唯一正式数据库做 destructive upgrade。

## 7. Regression gates

严格串行执行结果：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check                    PASS
cargo check --manifest-path src-tauri/Cargo.toml --all-targets                    PASS
cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1 PASS
  Rust library: 700 passed; 0 failed; 1 ignored
  Integration targets: 200 passed; 0 failed; 1 ignored

pnpm test                                                                        PASS
  Test Files: 101 passed
  Tests: 447 passed; 0 failed
pnpm exec tsc --noEmit                                                            PASS
pnpm build                                                                        PASS
git diff --check                                                                  PASS
pnpm tauri build                                                                  PASS
```

Rust ignored tests are existing live-runtime gates (including the real ComfyUI gate); no failed test was hidden or reclassified.

## 8. Issues and deferred work

```text
P0 = NONE
P1 = NONE
P2 = NON-BLOCKING
P3 = NON-BLOCKING
```

Non-blocking notes:

1. DEV-072 operator deviation：用户误点 D，短暂产生等待意图；未产生 Task/ComfyUI submission，最终按显式 D Start 完成。已记录，不改写为“未发生”。
2. V1 不提供 `Start Selected`/`Start All`；逐个点击是当前显式安全边界，若只改善便利性归类为 P2，延后 1.1。
3. DEV-072 文档中较早的 regression block 写过 `FRONTEND_TESTS=444`，最终 UAT block 和本次实际全量结果为 447；DEV-069 release guard 仍保留历史 `PRODUCT_VERSION=0.8.1` 文案。两者都是验收文档一致性问题，不是 runtime defect。
4. 构建有 Vite 单 chunk 超过 500 kB 的 advisory warning；不影响构建或运行，延后性能迭代。

Deferred to 1.1：persistent sequence restart resume、Start Selected/All convenience、season-level reporting/scheduling 和 bundle code-splitting。1.0 保持安全默认：创建不启动、没有隐式下一批、没有自动 retry、没有重启后续跑。

## 9. Final record

```text
DEV073_DOC_COMMIT = this docs-only commit; exact hash recorded in task final report
DEV073_FINAL_MASTER = final master after this document push; recorded in task final report
DEV073_SOURCE_ONLY_CI = Source-only CI for DEV073_FINAL_MASTER; recorded in task final report
AI_STUDIO_1_0_READINESS = PASS
NEXT_DEV = DEV-074
```

