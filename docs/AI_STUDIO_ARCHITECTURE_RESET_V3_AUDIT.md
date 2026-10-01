# AI Studio Architecture Reset V3 — 独立现状审计

## Phase0B final closure — 2026-10-01（当前权威结论）

本节覆盖下方历史 Phase0 PARTIAL gate，保留历史记录，不把旧未执行项改签为旧轮 PASS。

- START_HEAD / ACTUAL_BASELINE：`397118ffb91c7dd623ec4ff70f1e563811c09e43`；master fetch/pull ff-only，无外部更新。
- 指定 Source-only CI [36846060015](https://github.com/zhangcan001/AI-Studio/actions/runs/36846060015)：同一 exact head，**completed / success**；Frontend tests、TypeScript、build、Rust fmt/check/tests 全部 success。Rust job 09:57:36Z–10:15:39Z，18m03s。新文档提交的自动 CI 不在此伪签 PASS。
- 只补 D/E、B 状态/C 配置；F 按用户收窄 gate 冻结。仅修改三份文档，没有实现 Facade/Route/Run、业务修复、迁移或运行包修改。
- 隔离项目 `prj_14526f0d-e3f5-4562-bb30-bc41e97c7bb0`；Native 用标准 `pnpm tauri dev`，根仍为 Temp `ai-studio-phase0-ffda76db`。只读 DB 对照，无手写 SQL 创建/修改 Task、Asset、状态、绑定。独立绑定 writer 使用实际 application service + repository ports，不是 SQL 注入或替代 executor。
- 临时 ControlledComfy 边界在 18788；本轮唯一成功 Native 图片提交重放历史字节，标记 **FIXTURE**，没有新 GPU 生成。历史真实 H3 播放证据沿用上轮，不重复播放验收。真实 Comfy 只读 schema/stats，不重启/关闭。

### D1 — Native 输入错误与新运行

在同一 `镜头03 / sht_80d8520d-d243-4e95-9f47-564696ac6483`，IMAGE 配方 acceptance_image 的 width=0，经普通 Generate → 正式 Queue 后产生确定性 validation failure：

```ini
INITIAL_RUN_ID=pbt_b6014f56aa5f428a84932115b93ceb4b
INITIAL_TASK_ID=tsk_56d50ee8-b565-48e8-baeb-222c104201b5
ERROR_CODE=INPUT_OUT_OF_RANGE
ERROR_USER_MESSAGE=输入值超出允许范围。
INPUT_ERROR_ACTION=RETURN_TO_CREATE_AND_USE_SHOT_LOCATOR_TO_EDIT_INPUT
NEW_RUN_ID=pbt_4b44267a1e74447896dcd862f9ada9db
NEW_TASK_ID=tsk_518f0919-4384-4e13-9f39-501a64fc0581
NEW_TASK_STATUS=SUCCEEDED
INPUT_RECOVERY=PASS
OLD_FAILED_TASK_PRESERVED=YES
```

Production 技术错误明确 `input "width" value 0 is outside range [1, 16384]`；Task 普通提示是中文，上述详细英文/code 在 Production 泄漏属于 legacy。Task 的“重试一次”禁用，并说明“该失败未被归类为临时错误”，没有盲目 retry 同一 invalid snapshot。Task 当前没有直接“编辑该镜头”CTA；实际通过创作侧栏、镜头定位搜索“镜头03”、选择原 Shot 回到编辑，width 改64，再 Generate。该路径可用但绕行必须 REPLACE，不能描述成现有一键恢复。

新 Task 两个 fixture Asset：`ast_f1aae2a3-23f4-4c72-a0e9-856737fdcf1d`、`ast_3a133d88-465f-43cc-8aa6-dfe9619ae922`。旧 Task 完整行与修正前只读快照相等，FAILED/error/finished_at 没被改成功。旧 batch/item 和旧 Shot generation history 仍在，新 Task 与 batch 使用不同 idempotency key；这是输入修正的新运行，不伪造 transient retry lineage。

### D2/D3 — 正式 recovery fixture（无 GPU）

从仓库已有 `src-tauri/tests/dev061b_queue_recovery.rs` 在 Temp 构建 test harness，链接本轮正常 dev 构建的当前 application library；未修改仓库 fixture 或正式断言。保留原 fixture 的 workflow/recipe definition seed；Task/Asset/状态全部由实际 Queue/Generation/repository 服务产生。`start_for_test` 是原正式 fixture 的 doc-hidden 测试入口，**不是 Native production admission 完整 UAT**。

1. 原用例 `restart_keeps_package_batch_idle_then_explicit_start_recovers_offline_and_retries_frozen_asset`：1 passed /4 filtered /1.83s。COMFY_OFFLINE 可正式恢复；child/new Task 使用 frozen input/exact pair；旧失败保留，重复 partial resume 幂等，不重读原 package 素材。Input validation 不能据此当 transient。
2. Temp 扩展用例 `phase0b_three_leaf_partial_recovery_preserves_success_and_failed_history`：复用上述真实服务/ControlledComfy，adapter 第2次明确 offline，构成 A-success/B-failed/C-success。初次用例误以为 continue_on_failure 会越过 offline，C 实际 Pending，断言失败；**没有隐藏失败或改业务语义**。正确 fixture 先显式继续离线暂停后的剩余 C，再开始 partial recovery。修正后 1 passed /4 filtered /3.68s。

```ini
TRANSIENT_ERROR_CODE=COMFY_OFFLINE
TRANSIENT_RETRY_SUPPORTED=YES
THREE_LEAF_BATCH=pbt_bc01860644724ac58253f7f3d98357f9
A_TASK=tsk_3823adec-37ea-49bf-85cc-cdeb78e0e739
OLD_B_TASK=tsk_fe089675-567f-4b55-b29c-44ef978ac65c
C_TASK=tsk_54a239cb-5631-490a-8ad5-904a44596a93
NEW_B_TASK=tsk_dbdb50f2-184b-4aaa-950f-6c1a6d212d59
NEW_B_RETRY_OF=pbi_b42e5477211c4c50a08934909a3531d6
A_REEXECUTED=NO
C_REEXECUTED=NO
B_RECOVERED=YES
OLD_B_FAILURE_PRESERVED=YES
A_ASSET_PRESERVED=YES
C_ASSET_PRESERVED=YES
SUBMISSIONS_TOTAL=4
TASKS_TOTAL=4
```

A/C 完整 batch item（含 Task id/时间）前后相等；两个 Asset id/source Task/SHA 不变；资产2→3。旧 B item 与 Task 保留 FAILED，child B SUCCEEDED，values/exact pair/frozen snapshot 保持。Offline 暂停是既有安全行为，不能为三叶场景重做整个批次。

### D4 — 同一失败 Task 五 surface（不修 legacy）

以下都关联 D1 的同一 failed Task/Shot/batch，不混算其他旧失败。以实测 navigation/重开为界，不声称采集毫秒级传播 SLA。

|Surface|实际 status|是否需手工刷新/重开|正确错误可见性|
|---|---|---|---|
|Create|失败后起初仍待启动/图片待执行；queue strip 0失败；重新定位 Shot 后为失败需要处理|需要重开/重新定位才能取得新投影|没有 width 错误；“下一步”仍填提示词，与 acceptance_image 原因不符|
|Production / Queue|对应 batch 已结束，有失败项目；item FAILED，Task id一致|导航入口读最新，无额外 refresh 按钮|明确 INPUT_OUT_OF_RANGE + width=0/range；普通页泄漏英文/code|
|Task History|同 Task 失败；普通中文“输入值超出允许范围”|从 Overview 对应 Shot 定位运行任务打开，无额外 refresh|正确中文；重试一次禁用；技术信息/多项 nullms 仍露出|
|Shot History|18:32:04 对应唯一输入失败运行仍“已排队”，0候选|仅点 History 未得到正确终态；后来重开 Shot 可更新 Shot stage，不据此断言 History 已同步|未显示输入错误|
|Project Overview|失败计数2→3；对应镜头03位于需要处理并可定位上述 Task|重新进入 Overview 读最新，无 refresh click|聚合失败可见但不呈现 width 原因，部分英文/ID露出|

FAILURE_PROJECTION_DIVERGENCE=YES。普通用户缺直接编辑CTA、accepted 绿色条仍保留、Create/History滞后、Overview聚合不等于Task错误，是 V3 Run projection 需要替换的缺陷，**不阻断已验证 domain recovery gate**。

### E — Native OCC、旧绑定与显式升级

因单实例 Native，采用独立 context A（实际 ProjectWorkflowBindingService/upsert，SQLite repositories）+ context B（Native stale form）构成 OCC race，不使用 DevTools invoke 或修改 IPC。临时 helper 使用 service 的 legacy 无 registry 构造器；验证 slot OCC + exact runtime pair，不额外声称跨 context registry lock/完整 admission UAT。

- A/B 同读 VIDEO/DEFAULT revision1、instance `bnd_c14989c8db1d4775a1ddeb7f7de38d96`。
- A 保存 I2V2.1，revision2；B仍持 revision1、在普通 picker 选择全能参考版本并保存 → **CONFLICT**。
- 实际 Native 文案：**“项目工作流绑定已被其他操作修改，请确认最新绑定后重试。”** 普通信息没有仅用 revision mismatch。
- B本地全能参考 draft 保留；server仍为A的I2V2.1/revision2，SILENT_OVERWRITE=NO。
- Native “检查开工条件”使用最新 server config，显示I2V2.1；上方picker仍Bdraft。SERVER_LATEST_VISIBLE=YES **经 readiness 面板**，不是已实现内联server/draft对比。额外点击与分裂UX归入legacy。

旧 binding 通过同一 actual service 合法保存为 I2V2.0/revision3：

```ini
OLD_WFV=wfv_081229d2-3c41-4b02-b57b-b207b8f3fea3
OLD_RECIPE=rcp_dacfb43b-e6d8-4573-9369-8a0c3ccac7ea
AFTER_CATALOG_AND_LIBRARY_REFRESH_PAIR=UNCHANGED
AFTER_APP_RESTART_PAIR=UNCHANGED
AUTO_UPGRADE=NO
NEW_WFV=wfv_cf180eea-ff88-4938-bb7b-6bee4a65633a
NEW_RECIPE=rcp_201e838e-1880-40ab-8a3c-fc6bc1fccd93
NEW_REVISION=4
BINDING_INSTANCE_ID=bnd_c14989c8db1d4775a1ddeb7f7de38d96
USER_EXPLICIT_UPGRADE=YES
NEW_PAIR_PERSISTED=YES
SAVE_RELOAD=PASS
```

Workflow Library 原“刷新”刷新 catalog/library，随后只关闭本任务隔离 Native/dev runner，再以相同标准dev/root重启。Old pair、revision3/instance/updatedAt不变；UI显示不可用并说明“不静默改写”。最后在普通视频默认选择器明确选择“MiniMax H3 高质量图生视频”（registry版本2.1），Save后离开/重开，服务只读确认上述新pair/revision4；IMAGE slot未变。普通picker露WFV/Recipe而未给友好版本号：VISIBLE_WORKFLOW_VERSION_ID/RECIPE_ID=YES，DISPLAY_VERSION=NOT_SHOWN（版本来自真实registry，不虚称UI显示）。隔离历史包存在hash/readiness警告，本轮不改包、不把可保存绑定当GPU准入PASS。

### B/C/F 与最终 gate

- B：沿用已完成正常fixture成功输出、候选选择、selectedImage持久化、旧Task/Asset保留证据；本轮输入修正新增成功Task不删除失败。REQUEST_ACCEPTED与Task终态明确不同；正常Create未完整独立呈现QUEUED/RUNNING，STATUS_STATES_DISTINCT=NO / B_STATUS_SEMANTICS_INCOMPLETE=YES。B_GATE=PASS_WITH_KNOWN_LEGACY_DEFECTS，不能为缺label无限补测。
- C：同一镜头03 Create，Video → 当前正式I2V2.1 exact pair；参考选 `ast_67bc78db-81d1-4c6a-ac37-d39729b022c0` 并保存；提示词编辑/应用“紫砂茶壶静置于木桌，镜头缓慢推近，窗边柔和自然光，保持参考图主体与构图。”；duration_seconds=1，544×960并普通保存配置。只读DB确认reference、prompt_text、scalar config同Shot。生成按钮可见，**未点击视频Generate**。未保存时长曾因参考保存重载恢复默认5，随后重新编辑并明确Save；final为1，草稿保存顺序UX需改进。C_CONFIGURATION_COMPLETE_IN_ONE_CREATE_CONTEXT=YES；历史真实quality2.1.1视频播放沿用上轮，不伪称为本轮I2V新输出。RAW_TECH_LEAKS=YES（recipe UUID/内部fl2va_image_to_video/asset ID）。C_GATE=PASS。
- F：按用户本轮明确收窄的角色判断冻结PASS：Runs=执行状态与恢复；Library=持久媒体发现/复用；Shot=当前创作/选用结果。不追加输入reuse/Library完整UAT，未执行者仍NOT VERIFIED，不再作为本轮gate。
- D_GATE=PASS；E_GATE=PASS；Product Contract四类分类及domain invariants已冻结。READY_FOR_IMPLEMENTATION_PHASE_1=YES只是入口资格，**本轮停止，不实施Facade**。

### 测试预算与证据位置

保守计数10项：①Native OCC race；②原正式transient fixture；③扩展三叶初次失败；④修正fixture重跑；⑤旧binding refresh/restart；⑥显式升级/reload；⑦Native输入错误；⑧同Shot修正提交；⑨H3配置（不提交）；⑩同失败五surface投影。准备编译失败非用例执行；实际测试/编译前约17–18GiB可用RAM、VRAM约2.0–2.1GiB/16GiB，串行且没有其它build/test并发；未使用内存清理器/GPU reset/用户进程终止。未做本地全量Rust/Vitest或额外GPU验收；指定已有远程完整CI是独立权威。

仓库外本地证据：Temp `ai-studio-phase0b-occ-writer.log`、`ai-studio-phase0b-old-binding-before.log` / `after-refresh.log` / `after-restart.log`、`ai-studio-phase0b-explicit-upgrade.log`、`ai-studio-phase0b-transient-result.log`、`ai-studio-phase0b-partial-result.log`（初次失败）/`partial-result-final.log`、`ai-studio-phase0b-input-recovery.json`、`ai-studio-phase0b-h3-config.json`、`ai-studio-phase0b-boundary-trace.json` 与工具Native截图。日志/JSON/本地DB/运行缓存不提交；本节摘录身份、动作、断言、失败及适用范围作为版本化审计摘要，不虚称已提交永久媒体证据包。

```ini
PHASE0_DOCS_CI=PASS
B_GATE=PASS_WITH_KNOWN_LEGACY_DEFECTS
C_GATE=PASS
D_GATE=PASS
E_GATE=PASS
F_GATE=PASS
PRODUCT_CONTRACT_COMPLETE=YES
PHASE0_EVIDENCE_COMPLETE=YES
ARCHITECTURE_CONTRACT_FROZEN=YES
READY_FOR_IMPLEMENTATION_PHASE_1=YES
ARCHITECTURE_IMPLEMENTATION_STARTED=NO
```


## Phase0 补证更新（2026-10-01；PARTIAL）

本轮 baseline `9f23e22bba73426534eabb080247db9228add91e`，fetch/pull 后 local/origin 相同、工作区干净；与原源码基线间只有两份docs。CI `36841629302` exact head 已查询 **completed/success**。用户已批准 KEEP_ENGINE / REBUILD_CONTROL_PLANE 与 project-first/single-route/Run projection/Advanced Workflow Lab；不等于实施授权。

隔离根 `C:\Users\ADMIN\AppData\Local\Temp\ai-studio-phase0-ffda76db` 用只读 SQLite backup 克隆旧隔离项目，历史文件仅读取，无 SQL INSERT/UPDATE/DELETE。标准 dev build 37.04s；资源检查约17GiB空闲RAM、2.3GiB/16GiB VRAM，不并行runner，不关闭用户应用/ComfyUI。临时18788边界借鉴ControlledComfy seam，重放历史字节；新任务全部 **FIXTURE**，不是新GPU输出，不修改仓库正式fixture。真实Comfy仅GET schema/stats。

初版临时夹具未回显请求prompt_id，backend拒绝为 SUBMISSION_STATE_UNCERTAIN；只修仓库外夹具。之后两个正常提交的 Task `tsk_78f34799-4d03-44c6-9b9c-0ffa0f09bd10` / `tsk_5a848b2a-59ec-4331-929c-664f45b403f7` 均 SUCCEEDED，各有两个fixture Asset。初版失败不是input/transient恢复闭环证据。

|Journey|本轮实际证据|Gate / 缺口|
|---|---|---|
|B|Shot候选显示新fixture输出；合法确认历史真实图片 `ast_74f7051a-c289-4e3a-a198-ce6238d91eff`，selected持久化，其他Asset/Task保留|PARTIAL；完整queued/running文案、Review及全surface同时性未闭环|
|C|历史真实H3首尾帧quality2.1.1一秒视频 `ast_dd7f218a-facc-4be5-a9d1-9287d2cfdc75` 在Shot播放、确认，selected_video持久化；Library再预览，来源Task真实运行包/版本可见|PARTIAL；当前Shot表单默认ref2va，不能当历史首尾帧任务输入；同mode配置闭环待补|
|D|真实失败Task保留，之后正常新任务成功；Create统计0failed与Production非零有入口差异，但混合旧失败，未测单失败延迟|NOT VERIFIED；input修正/transient retry/三项partial resume/五surface传播未闭环|
|E|native选择首尾帧generator保存，离开后重进；DB只读确认VIDEO/DEFAULT exact `wfv_ed7bd1b1-52ab-41b8-93de-f6138c1122bd` + `rcp_18d5fe30-a428-4be8-a62a-f19c7812322b`，revision1，IMAGE原slot不变|PARTIAL；native OCC冲突/draft和本轮旧binding refresh/restart/显式升级待补|
|F|Shot历史候选可播放选用；Library图片/视频预览；“查看生成任务”打开真实H3 Task `tsk_bddec1cc-3306-43b2-bf46-b9e3a061a5ac`，quality2.1.1；“用于创作”存在|PARTIAL；Shot History完整路径、inputs reuse动作未执行，按钮存在不是PASS|

接受文案原文：“已加入普通生产队列 pbt_… 并开始处理，镜头候选仍需手动选择。” **并没有字面“生成成功”**。绿色颜色/UUID/缺少完整终态反馈不应保留。Task成功、Asset存在、Review、Shot选用是不同事实，不能由Shot stage“已完成”推断Artifact review通过。

新Shot未生成之前已展示同项目历史候选，未来应明确“本次输出”与“历史复用”，不造来源。当前最佳媒体入口是 **Asset Library**（检索/预览/来源任务）；Run统一执行状态/恢复仍有依据，但不能吞并Library或复制Task/Asset/Review/Selection authority。

OCC既有单项 `slot_occ_requires_instance_and_revision_and_preserves_creation_time`：1 passed /1210 filtered /0.28s。直接运行9:08既有binary，源码与当前baseline间仅docs；**未重新编译test target，且repository test不是service/native UAT**。保守核心执行计数7（初版图片失败、两次fixture成功、历史视频、历史检索、binding保存/reload、OCC）；没有全量本地测试。

本地截图 `windows-computer-use-*`、Temp `ai-studio-phase0-boundary-trace.json` / `ai-studio-phase0-session.json` / `ai-studio-phase0-media.json` 是仓库外证据，不是版本化永久证据包。完整PRESERVE/DO_NOT_PRESERVE及待补oracle见新增PRODUCT_CONTRACT。

**PHASE0_EVIDENCE_COMPLETE=NO；PRODUCT_CONTRACT_COMPLETE=NO；READY_FOR_PHASE_2=NO。** 不修缺陷，不实施Facade/Route/Run。未验证项不能用代码阅读代签。

日期：2026-10-01。代码证据基线：`master@fe00472950c12593892e6d80f97fcf6bf60776e6`。

## 1. 结论与证据等级

**KEEP ENGINE / REBUILD CONTROL PLANE。** 不建议全部重写或改数据库；建议渐进替换前端壳、路由、普通产品用例边界。大文件不是删除理由，多结果页不是多个后端 authority 的证据。

```ini
FULL_REWRITE_RECOMMENDED=NO
FRONTEND_SHELL_REWRITE=YES
APPLICATION_FACADE_REWRITE=YES
CORE_ENGINE_REWRITE=NO
DATABASE_REWRITE=NO
RECOMMENDED_OPTION=C
STATUS=PARTIAL
READY_FOR_PHASE_2=NO
```

这里的 facade rewrite 是重建普通 UI 调用边界，不是重写应用服务。PARTIAL 是因为成功图片/视频、成功结果选用、失败重试保留成功项等端到端路径本轮未执行；不表示已观察到的结构问题没有证据。不能用历史 PASS/FROZEN、CI 或旧原生验收替代本轮 UX 证据。进入实施前须补齐第 4 节的安全正向路径。

证据标签：**NATIVE** 本轮 Windows 原生界面实见；**CODE** 当前源文件读码；**MEASURED** 本轮 AST/文件统计；**INFERENCE** 有代码依据、未复现的风险；**NOT VERIFIED** 未执行，不得写 PASS。下文引用均相对上述基线；不是旧方案里的估计数字。

## 2. 边界、方法、资源与证据位置

- 开始 checkout 与 origin/master 相同、工作树干净，已 fetch。只新增本审计及对应 PLAN；无源码、包、迁移改动。不修改历史文档。
- 标准 `pnpm tauri dev`，不是浏览器 mock。首次成功编译 2m48s 后受已有实例限制；用户自行关闭旧应用后，缓存编译 0.55s，隔离原生窗口启动。没有终止用户应用或 ComfyUI。
- `AI_STUDIO_DATA_ROOT=C:\Users\ADMIN\AppData\Local\Temp\ai-studio-architecture-audit-70541db4`。应用正常初始化隔离数据库；通过普通 UI 新建一个 Shot，发出一次空提示词请求并得到失败记录。**真实用户数据库未操作；没有手工 SQL 更正或 schema 变更。**“只读审计”指不修业务实现、不更改真实数据，不把隔离 UI 正常写入谎报成没有任何写入。
- 编译前约 18 GB 可用 RAM，GPU 使用约 1575/16311 MiB，没有并行 cargo/rustc。CARGO_BUILD_JOBS=1。无全量 Rust/Vitest、无实际 GPU 生成、无模型卸载。本轮 6 个核心 UX 场景，不把截图/滚动当成额外测试。
- 输出统计为 UTF-8、行数 split LF（含末尾空行）；KB 门槛为十进制 50,000/100,000 bytes。hook 数为具名 `useState/useEffect` AST 调用，不包含 custom hook 内部。API 调用数是该文件直接调用具名 tauriClient 导入的静态调用点，不是运行次数，也不是完整依赖计数。
- 截图保存在 **仓库外** `C:\Users\ADMIN\AppData\Local\Temp\ai-studio-architecture-audit-evidence`；统计脚本和 JSON 也在 Temp。桌面截图可能包含背景窗口，不入库。Temp 非长期归档，后续若需长期证据须用户指定私有归档位置。
- Windows DPI 125%；检查客户区逻辑/CSS 尺寸 1920×1080、1440×900、1180×760。分别约为物理 2400×1351、1800×1126、1475×951。不能把桌面 2560×1440 截图直接当应用 viewport。首次无效放大截图、自动化坐标失误均不计入 Journey。

证据索引：A-first-use-{1920,1440,1180}.png；A-continue-enters-consistency.png；B-image-shot-1440.png；C-h3-modes-1180.png；C-h3-duration-1440.png；D-misleading-submission-notice.png；D-empty-creation-queue.png；D-review-no-failure.png；D-failure-detail-raw-identities.png；D-null-ms.png；D-nonretryable-task.png；E-workflow-readiness-1440.png；E-project-generator-settings.png；E-generator-technical-identities.png；F-shot-history.png；F-empty-library.png。

## 3. 用户任务与产品问题排序

核心任务是生成图片和 MiniMax H3 视频、选用候选、查找结果、从失败恢复。当前用户已排除快速预览、音频输入、单独音频生成及图片+独立音频混合输出。新产品普通 UI 不重新引入这些能力；视频内音轨不等于独立音频工作流，历史 Audio 类型/资产/备份仍必须可读。

|优先级|现象与证据|根因判断|架构处置|
|---|---|---|---|
|P0|空 prompt 失败后仍见绿色已加入队列提示，创作 drawer 0 失败；生产页才见真实失败 [NATIVE]|submit/start acknowledgement 与终态结果混用；各页面读模型刷新条件不同 [CODE]|统一运行 projection/事件失效，不重写 Queue|
|P0|普通项目生成器选择框显示 WFV/Recipe UUID [NATIVE]|把精确身份正确性直接交给用户承担|保留底层 exact pair，普通 UI 只显示生成器名/版本/模式|
|P1|空项目“继续工作”到项目一致性配置，不直接落到镜头创作 [NATIVE]|route/上下文选择与“下一步”产品意图错位|Project-first，下一步对应唯一可执行目标|
|P1|失败 Task 不在 Artifact Review 空页；恢复入口散落 [NATIVE]|Task、Artifact、Shot、Batch 是不同 projection，缺主运行入口|Runs 主入口，Review 作为结果动作|
|P1|同一 shots workspace 有 creation/production/review，resume 仅存 workspace [CODE]|路由分拆成两个主状态|一个判别式 Route；resume 只是其兼容 projection|
|P1|1180 镜头 pipeline 横向滚动，prompt/参数/引用在纵向深处 [NATIVE]|固定结构树+候选+inspector+pipeline 同屏|响应式渐进披露；关键动作稳定可见|
|P2|任务运行统计出现 nullms，技术 provenance 默认展开 [NATIVE]|展示模型未归一化 nullable；诊断模型充当普通视图|共享 status/nullable formatting；诊断抽屉|
|P2|Workflow FAST 视图 0 可生产/5 待处理，显示 exact runtime package has not been refreshed；页首 Comfy 已连接 [NATIVE]|连接、刷新、可运行本是不同事实，表达未给下一步|清楚展示未检查/检查中/可用/阻断，不把“已连接”当 ready|

最后一行不证明五个生成器永久不可用：没有点击 refresh/实际 preflight，不将读模型的未检查状态判作 engine 失败。

## 4. 六个 Native Journey 与量化边界

点击为语义动作，滚动、resize、截图、自动化纠正不计。以下只量化记录到的有限片段；不声称整条任务最短路径，不虚构人口学认知指标。概念数是该片段明确列出的不同概念下界，决策数是观察到的分岔，不是完成后的统计。

|Journey|实际片段|点击|页/上下文切换|概念下界|决策下界|重复目的地|死路/回退|结论|
|---|---|---:|---:|---:|---:|---:|---|---|
|A 首次使用|home→继续工作→项目一致性→+→新建镜头|3|2|8|2|2 个首用 CTA|误向一致性 1；回退 0|观察完成；体验不通过|
|B 图片|Shot 图片→提示词 tab→生成（空 prompt）|2|0|6|1|两处生成 CTA|终态未就地呈现 1；回退 0|PARTIAL，未成功出图|
|C H3 视频|同 Shot 视频→生成器 dropdown→选 H3 文生视频→滚动查看时长|3|1（同页 stage）|7|2|1 个镜头入口，另有 Run/Studio 等|默认 ref2va、无素材而 disabled；未执行|PARTIAL，未生成视频|
|D 失败恢复|失败后 drawer→审核→生产→batch 详情|4|2|8|3|Queue/Review/Task/Shot 多投影|空 drawer、无 artifact 的 Review 两个死路；回退 0|PARTIAL，失败定位完成，重试未执行|
|E 更换生成器|创作→工作流→更改工作流→项目设置→打开图片默认 dropdown|3|2|6|2|Shot 与项目两种作用域选择|到配置页；未点击保存|PARTIAL，选择路径完成，OCC 写入未验收|
|F 历史结果|Shot→历史 tab→资产库|2|1|5|2|Shot History / Asset Library / Review / Task / Run|只有失败记录，无成功 asset|PARTIAL，正向历史结果缺样本|

A 的 8 个概念：项目、镜头、Production Handoff、批次、队列、准备、审核、一致性绑定。B：stage、workflow、recipe、prompt、candidate、selected result。C：图片/视频 stage、ref2va、fl2va_text_to_video、recipe、参考素材、秒数、质量。D：queue、batch、item、task、artifact、review、snapshot、provenance。E：项目默认、视频模式覆盖、工作流版本、recipe、运行时刷新、绑定来源。F：history、task、asset、候选、选用结果。概念不是全部有问题：镜头/提示词/素材应保留。

### A 首次使用

默认项目自动存在。home 同时有“继续工作”和五步 Production Handoff 教程，空项目开始创作按钮 disabled。继续工作进入 PROJECT 一致性 scope，显示绑定档案、参考集、保存配置、backend truth；需要结构树 + 才能新建 Shot。对只想一张图的用户，这不是必要前置。三个 viewport 首页均检查；1180 镜头有横滚。

### B 图片与 D 失败

本轮唯一普通提交没有输入 prompt。隔离 task `tsk_54021be5-381c-4b94-84d9-73f98eec8011`、batch `pbt_4ba8cf8000b646f7b2971db6c866b559`：FAILED/INPUT_REQUIRED，`prompt_id/queued_at/started_at/submitted_at` 均空；Comfy `/queue` 为空，故没有 GPU 生成。提交后绿色 notice 只证明已创建/启动请求，不证明成功；没有就地填错定位。

生产模式能看到 1 失败；进入 batch 详情才见 raw English INPUT_REQUIRED、GenerationTask ID、WFV/Recipe UUID。项目日常看板失败卡片跳 Task detail，detail 有中文“请先填写必填输入项”（优点），但 provenance 表仍直接露 build hash、package path、workflow hash；时长六字段出现 nullms。一次重试因非临时错误 disabled。Shot History remount 后显示失败记录与重试；**没有点击重试，没有验证修正 prompt 的恢复闭环**，也没有有成功项的批次，不声称保留成功项已通过。

CODE：`ShotWorkspace.generate` 892–960 保存配置→submitShotGeneration→startProductionQueue→setNotice→立即 getShot；638 附近监控、queue query enabled 只在 production mode，1560 附近 drawer creation 未传相同 overview。App 更新 taskStore 不自动等于 Shot local projection 已更新。这是观察与代码吻合的刷新边界问题，不是丢 Task。

### C H3

Shot 视频默认选择“MiniMax H3 高质量全能参考 · ref2va”，dropdown 另有高质量图生视频和文生视频，使用 fl2va 内部英文 mode；底层 recipe ID 常显。选文生视频看到时长秒数默认 5、544×960，素材列表空。本轮没保存/提交，没有再次验证 1/5/15 秒成片、首尾帧或参考素材选择；历史工程验收不能顶替。

### E 生成器

工作流一级导航先进入管理与运行时读模型；“更改工作流”跳旧 Projects workspace，与首页 Project Command Center 共用“项目”导航语义。项目默认 dropdown 实见 `Krea2 文生图 · wfv_... · rcp_...`。未保存，故没有破坏绑定，也没有复测并发 OCC。普通 picker 应返回展示名和 opaque selection token，内部仍保存 exact identity。

### F 结果

Shot History 显示失败图片 0 候选、查看任务/重试；Asset Library 无资产，Review 无 artifact。三个空投影的数据各自正确，但主结果入口不明确。实际成功历史资产打开、跳回 Shot/Run、来源复用、删除引用保护 **NOT VERIFIED**。

**UX_GATE=PARTIAL。** 补齐门槛：使用用户授权的已完成样本/隔离副本或明确标示的安全 dev fixture，走 B 成功候选选用、C 视频参数/素材→运行→成片、D 可修复错误与已有成功项重试、E 保存 exact pair 并发冲突、F 已完成历史媒体查找。不要以造假运行记录、写真实数据库或再跑高成本 GPU 来填表。Phase 0 先补证据，Phase 2 实施资格尚未授予。

## 5. IA：9 个入口不是 9 个独立业务模型

CODE：StudioGlobalRail.tsx:17 的 9 个主项：项目、创作、资产、提示词、工具、生产、审核、工作流、设置。类型里的 analysis 不是实际第十项。

|workspace（11）|可达语义|判断|
|---|---|---|
|command-center|项目首页/下一步/生产健康|KEEP 目的，重做聚合展示|
|shots|creation/production/review 三 mode|MERGE 路由，拆 page/controller，不拆事实|
|studio|直接生成/实验/批次旧入口|MERGE 到 Create；高级实验保留|
|video|素材到视频批量入口|MERGE Create 批量模式|
|assets|资产、档案、参考集|KEEP/MERGE 到 Library|
|prompts|提示词库/版本/溯源|MERGE Library + Create picker|
|tools|本地工具配置|HIDE 系统高级工具|
|tasks|任务历史与详情|MERGE Runs；诊断详情保留|
|projects|管理/备份/项目生成器绑定|MERGE 项目设置/项目选择|
|workflows|导入/识别/映射/运行包/版本|HIDE 设置→生成器→Workflow Lab|
|settings|连接、运行环境、诊断|KEEP 系统设置|

这 11 项不全是重复：结构生产包、工具配置与单 Shot 创作有真实不同需求。重复的是普通生成入口、状态展示和导航目标；不应删除其领域服务。

Route 当前 workspace + activeStudioSection 双状态，另有 focus task/batch/asset/filter、shotContextTarget、resumeShotId；workspaceResume 只存 project/workspace/shot，shots 默认恢复 creation，因此 production/review 的 section 无法精确恢复 [CODE]。这是路由表达冗余和兼容缺口，不代表实际用户数据被删除。

## 6. App 与五个复合组件

|文件|行|useState|useEffect|直接 API 点/不同函数|职责类别（≥4即 GOD）|处置|
|---|---:|---:|---:|---|---|---|
|App.tsx|1154|29|6|26/19|PAGE,CONTROLLER,DATA FETCHING,MUTATION,ROUTING,VIEW|REWRITE shell/route boundary|
|ShotWorkspace.tsx|1811|33|14|31/24|PAGE,CONTROLLER,DOMAIN LOGIC,DATA FETCHING,MUTATION,VIEW,ROUTING|REFACTOR 成 Create/Run/Review controllers|
|WorkflowWorkspace.tsx|1564|34|5|6/6|PAGE,CONTROLLER,DATA FETCHING,MUTATION,VIEW,DOMAIN LOGIC|高级 Lab 分离；不是删引擎|
|ProjectCommandCenter.tsx|1191|12|1|2/2|PAGE,CONTROLLER,DATA FETCHING,VIEW,ROUTING,DOMAIN LOGIC|已有 aggregate seam，去重复镜像|
|GenerationStudio.tsx|744|5|1|2/2|PAGE,CONTROLLER,MUTATION,VIEW,DOMAIN LOGIC|复用 draft/hooks，迁移入口|
|ProductionRunPanel.tsx|742|19|7|14/14|PAGE,CONTROLLER,DATA FETCHING,MUTATION,VIEW,DOMAIN LOGIC|编排专项 controller + Run projection|

**GOD_COMPONENTS=5/5**（App 另计）。计数是文件级，含 helper/子组件（如 ParameterExposurePane、FinalVideoPreview）；不能说每个主函数恰有全部 hook。WorkflowClient namespace 封装、自定义 hooks 的 API 未计入直接列，不能误报 Workflow 只有 6 个后端依赖、GenerationStudio 只有 2 个用例。

App 九组职责：bootstrap、routing、project、task events/recovery、runtime/capability、consistency、workflow/binding、production admission、UI-local。危险不是 1154 行本身，而是 `openWorkflowForProject` 既负责导航又 upsert bindings，`loadHistoricalInputs` 跨 provenance、store draft、路由；route transition 和数据 mutation 缺小而完整的接口。Shell 应只装配和路由，不机械增加 5 个 Provider。

Shot 既结构树 CRUD/批量配置又 stage 草稿/参考素材/提交/queue/retry/审核，reload 拉 7 类数据。Workflow 兼导入 wizard 7 步、版本生命周期、映射、diff、history、quick-run、runtime profiles；分离依据是用例边界而非每 500 行切开。CommandCenter 已有 server aggregate，仍 setSummary/activity/integrity/preflight 为镜像；shots/structure legacy state 只有 reset 而非同样 load，需要经过调用/兼容验证才删除。GenerationStudio 已分 hooks，必须复用 StudioStore 的 authority，不另建新 draft store。

## 7. 生产术语、入口与结果 authority

|概念|领域事实/执行实现|普通用户必须看？|能藏到 Run？|
|---|---|---|---|
|ProductionRun|图片→选图→视频编排事实|可展示为组合运行|可，但不可等同所有 batch|
|ProductionBatch|队列批次和策略持久事实|普通只看运行组|YES|
|ProductionQueue|执行准入/worker authority|看排队/进度，不看类名|YES|
|ProductionPackage|文件导入与映射来源事实|高级批量导入需要|默认藏，导入向导显示人话|
|ProductionPreparation|准备/预检、计划事实及 projection|看阻断项，不看模型名|YES|
|ShotBatch|批量镜头配置与计划|看选中镜头范围|YES|
|Task|独立状态机/历史事实|看运行明细状态|YES|
|Generation|编译/适配执行用例与 provenance|看生成动作/参数|YES|
|Artifact/Asset|执行产物与资产/文件事实|必须看结果，不看 artifact ID|不能隐藏媒体本身|
|Review|评审事实|需要明确选用/通过/拒绝|可做 Run/Shot 的动作|

**Run 可以成为普通运行唯一对象，但不能新建一个取代 Batch/Task 的数据库事实。** ProductionRun 与 Batch/Task 不一一对应。采用有判别式 source 的 RunRef projection，细节见 PLAN；审阅状态与运行终态分列，PAUSED 不是完成，Task SUCCEEDED/FAILED/CANCELLED 与 Batch COMPLETED 不混淆。

全部生成入口清单：GenerationStudio（single/batch/experiment hooks）、ShotWorkspace.generate/retryShot、AssetVideoBatchWorkspace、ProductionRunPanel、DirectGenerationEntry、WorkflowExecutionConfiguration（single/batch）、WorkflowWorkspace.quickTest、ProductionPackage/Scene/Episode/Series/Project admission、TaskHistoryDetail 重试。前四类重复普通路径，结构导入/多镜头/专用图片→视频编排是合理任务；Lab quick run 是高级验证。所有普通入口仍走 create/submit→Production Queue Start；本轮未发现第二执行 authority，不能因多个按钮指控多个 executor。

目标 **NORMAL_GENERATION_ENTRY_COUNT=1**：Create（单镜头或多选模式）；Runs 的重试是恢复动作，Library“用于创作”是 intent，Lab 验证不计普通入口。不要借此强行要求旧独立任务凭空生成 Shot；历史无 Shot 任务仍可打开/复用，未来新独立创作可显式创建 unassigned Shot。

|结果页面|真实 authority|目标|
|---|---|---|
|Task History|TaskRepository 状态/GenerationSnapshot 输入|Runs details 次级诊断，不主结果入口|
|Asset Library|AssetRepository 元数据 + AssetStore bytes|跨运行搜索/复用|
|Artifact Review|ArtifactRepository artifact+review|Run/Shot 审阅动作，不藏失败任务|
|Shot result|ShotRepository selected result + task links|当前镜头创作主结果入口|
|ProductionRun result|OrchestratorRepository 编排与选图|Runs 组合运行 projection|

主运行结果入口 Runs→Run results；Create 内候选是镜头上下文投影，Library 是资产搜索投影。Asset selection 与 review 不自动等同，要保留原状态约束。

## 8. Workflow 产品边界与术语矩阵

|字段/概念|普通 UI|高级 UI|仅诊断|决定|
|---|---|---|---|---|
|名称、模式、版本、可用性、适用输入、质量|YES|YES|YES|KEEP 人话|
|WorkflowVersion/Recipe exact pair|不直接展示 ID|YES|YES|HIDE identity 但保留提交精度|
|Node ID / input name|否|映射界面 YES|YES|ADVANCED|
|evidence tier / mapping source|否|识别解释 YES|YES|ADVANCED，不删溯源|
|raw blocker code|中文阻断+处理动作|可展开|YES|DIAGNOSTIC_ONLY code|
|promotion|“项目推荐版本”且须明确作用域|完整生命周期|YES|改人话，不能自动改历史 binding|
|package hashes / path / snapshot hash|否|按需导出|YES|DIAGNOSTIC_ONLY|
|binding/revision/instance ID|“项目默认/镜头覆盖”|必要冲突说明|YES|OCC token 内部保留|
|batch/queue/task/artifact|运行/排队/明细/结果|底层详情|YES|普通不展示内部 ID|
|provenance|“来源/使用参数”|完整 lineage|YES|折叠技术细节|

引擎 Workflow/WorkflowVersion/Recipe/RuntimePackage/Recognition/Compiler 全 KEEP。一级工作流入口降级设置→生成器→高级 Workflow Lab；普通选择有准确版本和当前不可用解释，不依赖名称猜 exact identity。旧引用在库 current version 改动后不得偷偷换版本。

## 9. 唯一 Authority Map（20 对象）

下表唯一 owner 指事实主来源。服务协调 ports；SQLite/文件的互补内容不被误算双 authority。UI cache、route、display label 都不是事实。

|对象|SOURCE_OF_TRUTH / port|协调者/派生读模型|风险与目标|
|---|---|---|---|
|Project|ProjectRepository|ProjectService/CommandCenter|projectId 所有查询必带|
|Structure|ProductionStructureRepository|结构服务|树选中仅 route state|
|Shot|ShotRepository|ShotService|候选 selection 保存领域，不保存在 UI 第二事实|
|Consistency profile|ConsistencyProfileRepository|consistency 服务|表单编辑草稿不是事实|
|Consistency scope binding|ConsistencyScopeRepository/ShotConsistencyRepository 各自所属 scope|上下文 preview|继承结果派生，不落成第二绑定|
|ReferenceSet|ReferenceSetRepository|Library picker|顺序和引用保留|
|Workflow registry|WorkflowRegistryRepository|WorkflowRegistryService|当前指针≠旧版本 identity|
|WorkflowVersion/Recipe definition|GenerationDefinitionRepository / WorkflowLibraryRepository 对应版本定义|registry/onboarding|不可变与来源校验；不能靠名称 join|
|Runtime artifact/state|WorkflowRuntimeArtifactRepository/RuntimeStateRepository 对应各事实|workspace query|manifest bytes 与 runtime availability 互补|
|Project binding|ProjectWorkflowBindingRepository|BindingService|instance+revision OCC，禁止全量覆盖|
|Preparation|既有准备服务和 ShotBatch/Structure 读模型|preflight/admission|健康是派生，执行时重新准入|
|Queue batch/item|ProductionQueueRepository|ProductionQueueService|唯一调度/准入/settlement|
|Task|TaskRepository|TaskHistoryRepository 读模型|事件/cache 不能 override 后端状态机|
|Generation snapshot|GenerationSnapshotRepository|GenerationService/Compiler|不可变输入/编译 provenance|
|Asset metadata|AssetRepository/BrowseRepository|AssetService|browse port 非第二元数据事实|
|Asset bytes|AssetStore|collector/ingestion|bytes 和 metadata 互补，删除需 usage guard|
|Artifact/Review|ArtifactRepository|ArtifactReviewService|审核≠选用；不可自动合并|
|ProductionRun|ProductionOrchestratorRepository|OrchestratorService→Queue|组合编排非第二 GPU worker|
|Backup|ProjectBackupRepository +目录/归档服务|ProjectBackupService|恢复事务/文件补偿保留，v20|
|Runtime settings|SettingsStore/ComfyAdapter live status|runtime服务|缓存 capability 带 generation，连接≠可用|

没有证明 persisted duplicate authority，**DUPLICATE_AUTHORITIES=NOT FOUND IN INSPECTED PATHS**，不是全库不存在的形式化证明。实际发现：route 双状态、aggregate 镜像、taskStore/Shot local/queue local stale risk。没有证据把所有 derived readiness 写成数据库事实，故该项是禁止未来行为，不造不存在的现状 bug。

## 10. State 与 Effect 分析

|类别|目标主 owner|当前观察|
|---|---|---|
|DOMAIN_STATE|现有后端 repositories|Task/Shot/Binding 等不交 UI 所有|
|SERVER_STATE|按 project+entity key 的 feature query cache/projection|TaskStore、Shot local、queue、CommandCenter 镜像分别刷新|
|ROUTE_STATE|一个 AppRoute reducer|workspace/section/focus/contextTarget 分散|
|SESSION_STATE|StudioStore draft、project selection、resume adapter|复用现有 StudioStore，不另建 competing draft|
|LOCAL_VIEW_STATE|组件：展开/hover/dialog/临时输入|应留本地，不全部塞 Zustand|

不能为了减少 useState 把所有状态搬 provider。事件以 entity IDs invalidation，缓存只读；mutation success 携带引用和需要失效的 keys，权限/准入/OCC仍后端判定。dirty stage 在 project/selection/catalog 更新时由 effect rehydrate，可能覆盖编辑是 INFERENCE，本轮未复现，迁移前须专项守住。

重点 effects 完整逐项列于附录。DERIVED_STATE_SYNC / CROSS_COMPONENT_SYNC 优先消解，不把所有 INITIAL_LOAD 当坏代码；保留已有 active/requestId guards。App bootstrap effect混初始化、事件订阅、恢复，拆生命周期边界；Shot breadcrumbs 两向同步用 canonical route 代替；Run h3Recipe/h3Prompt→h3Values 与 h3Values→prompt 双镜像只保留一个 draft owner。

## 11. Backend application 大文件：按 cohesion 拆内部，不按行数换内核

顶层 .rs **104**，递归（含 ports/repair jobs 等）**164**；合计 **5,170,686 bytes**。**23 >50KB，11 >100KB**。与请求旧数字不同是统计范围/当前内容差异，不是测量失败。下列 use 数仅顶层 `use` 行，非依赖对象个数。

|文件|lines/bytes/use行|cohesion/责任/状态与事务|判断|
|---|---|---|---|
|project_backup_service|10465/432214/10|导出、检查、兼容恢复、文件 staging/补偿；ports restore 拥有 DB事务；主 tests 从5014|KEEP authority；内部 archive/inspection/restore/compat/test modules|
|workflow_onboarding_service|10731/421041/8|import/recognition/mapping/validate/publication；draft store与commit gate；主入口1283|REFACTOR 内部阶段，发布一次 authority 不拆跨事务外露|
|workflow_ui_normalizer|7832/300257/5|无持久状态，schema/widget/subgraph normalization算法；tests7001|KEEP pure algorithm；按转换阶段拆，不新service|
|h3_local_import_service|5896/223242/15|目录扫描、segment草稿、素材导入、queue bridge；tests4270|内部 scan/inspect/commit，FS补偿保持；不另执行|
|workflow_semantic_graph|5243/199511/4|纯图索引、闭包/evidence查询memo|KEEP，局部图算法模块；不改变V3语义|
|workflow_analysis_service|4740/171615/5|recognition/scoring/schema推断；无持久owner|KEEP engine；rule/DTO/test分组|
|workflow_lifecycle_service|4068/151584/10|enable/refresh/delete/restore、shared lifecycle gate；tests2854|KEEP service authority；拆内部query/delete/restore|
|production_queue_service|3889/151221/13|4repoports+generation/recovery、running gates；DB原子item settlement；tests3132|KEEP 唯一执行authority；内部admission/worker/recovery|
|generation_service|2918/113522/18|定义+snapshot+task+adapter+collector；tests2419|KEEP executor usecase；拆compile/collect内部阶段|
|workflow_registry_service|2892/111918/8|library元数据/current/promotion/deletion共享gate，补偿|KEEP lifecycle协调；不新registry事实|
|production_orchestrator_service|2666/100452/14|图片→选图→H3编排，调用QueueService/admission；tests1650|KEEP组合workflow，不是第二queue|

大型文件包含许多测试/helper；首次 cfg(test) 常是文件中间的单个 test helper，**不能用首次 cfg 行直接扣出生产 LOC**。本表模块边界来自责任/状态/事务读码，不给每个文件拍“重写”。依赖计数不是重构目标。建议在 UI/facade 边界稳定之后再做内部拆分，保持调用接口、锁顺序、补偿、语义 fixture 不变。

## 12. 保留内核逐项决定

SQLite KEEP；migration chain KEEP（MAX42、pool断言71、无043，未重跑迁移测试）；Project/Shot identity KEEP；Task状态机 KEEP；ProductionQueue authority KEEP；AssetStore KEEP；Provenance KEEP；ComfyAdapter KEEP；WorkflowCompiler KEEP；WorkflowVersion/Recipe immutability KEEP；RuntimePackage KEEP；Binding OCC KEEP；Backup v20 KEEP；RepairJobs item checkpoint/run_gate KEEP。没有当前证据足以推翻这些。**KEEP 不等于本轮已重新通过其全量回归。**

## 13. CSS / design system

App.css 6847行/208649bytes；18 !important、34 @media；uiPolish417/10084、studioQuality607/16807、studioTokens85/3420。全局 button/input/select/textarea、workspace类和后续 quality 覆盖跨代共存。重复 selector group `.app-shell`8、`.header-context-group`8、`.app-header`6、`.project-selector`6、`.studio-layout`5；包含合法 media 重定义，不能全部判重复 bug。

NATIVE：1180 pipeline 横滚、树列压缩、prompt低于折叠；1440 form仍多纵向滚动；1920首页可容纳但术语/CTA问题不消失。没有做像素回归或所有页面断点覆盖。保留 tokens；新 Status/Action/Field/Media primitives 和 page scoped styles；旧CSS随旧调用方退休，检查 selectors usage/三个 viewport后逐页删除，不能一次删 App.css。

## 14. IPC 分类、可删除性与边界

tauriClient.ts **2163行/300运行时 exports**；generate_handler **323 commands**。export type 不计。下面附录每一个 export 分到 Project/Creation/Run/Library/Workflow/Internal/System/Legacy/Diagnostics；handler 每一个按 namespace 列出并标注产品/高级/内部/诊断/CRUD。分类是**设计用途**，不是证明正在被普通 UI 调用，更不是删除授权。

正常细粒度 CRUD 数量多，主问题是组件编排底层多次写入：Shot 保存配置→submit→start，App选择workflow→upsert→catalog/route，CommandCenter既有aggregate seam却复制多份state。product facade 应提供完整 atomic/coordinated usecase，内部委派现有服务；不是将300个函数移动到5文件后宣称解决。

具名 import usage 只辅助查找；WorkflowClient `import * as transport` 及间接调用不会出现在该简单索引，故“零具名调用”不能判 dead code。没有一个 command 在本轮被授权删除。目标普通52个用例是 proposed budget，不要求323→52硬删除，也不把诊断/admin排除出总登记导致工具不可用。

## 15. 决策与下一步

Option C 与 A/B 的路径预算、具体 facade、功能迁移/兼容矩阵、分期门槛/回滚及删除候选见 PLAN。选择 C 是因为 project-owned数据/Shotcontext 要成为路由唯一上下文；当前跨页找失败、default picker UUID、resume双状态不会仅靠CSS解决。有限正向样本未完成，所以不宣布架构实施开工。先补安全证据并评审方案，不开始 Phase2，不增043。

## 附录 A：完整前端 facade / IPC surface 盘点

300 exports：NORMAL_UI_REQUIRED（普通用途细粒度候选）=201；ADVANCED_REQUIRED=72；LEGACY_OR_INTERNAL（含diagnostic）=27。这些是用途分类，不是已完成产品facade个数或当前可达性证明。已确认应删除的Legacy=0；无法仅凭unused具名import分类为遗留。

|Use-case domain|exports|
|---|---:|
|Project|36|
|Creation|27|
|Run|58|
|Library|73|
|Workflow/Internal|77|
|System|20|
|Legacy|0|
|Diagnostics|9|

分类语义：NORMAL_PRODUCT_USE_CASE是普通功能所需operation候选，可能是DIRECT_DOMAIN_CRUD；ADVANCED_ADMIN为Lab/工具/模型等高级用途；INTERNAL_SUPPORT为引导/缓存/预设支撑；DIAGNOSTIC为健康/溯源/修复状态。CRUD是正交标签，不等于可删。52个目标聚合用例仍须契约设计，不是从201个operation硬压缩。

### Project

|Export|command / wrapper|用途|
|---|---|---|
|listProductionStructure|production_structure_tree|NORMAL_PRODUCT_USE_CASE|
|createProductionSeries|production_series_create|NORMAL_PRODUCT_USE_CASE|
|updateProductionSeries|production_series_update|NORMAL_PRODUCT_USE_CASE|
|deleteProductionSeries|production_series_delete|NORMAL_PRODUCT_USE_CASE|
|reorderProductionSeries|production_series_reorder|NORMAL_PRODUCT_USE_CASE|
|createProductionEpisode|production_episode_create|NORMAL_PRODUCT_USE_CASE|
|updateProductionEpisode|production_episode_update|NORMAL_PRODUCT_USE_CASE|
|deleteProductionEpisode|production_episode_delete|NORMAL_PRODUCT_USE_CASE|
|reorderProductionEpisodes|production_episode_reorder|NORMAL_PRODUCT_USE_CASE|
|createProductionScene|production_scene_create|NORMAL_PRODUCT_USE_CASE|
|updateProductionScene|production_scene_update|NORMAL_PRODUCT_USE_CASE|
|deleteProductionScene|production_scene_delete|NORMAL_PRODUCT_USE_CASE|
|reorderProductionScenes|production_scene_reorder|NORMAL_PRODUCT_USE_CASE|
|assignProductionSceneShots|production_scene_assign_shots|NORMAL_PRODUCT_USE_CASE|
|unassignProductionSceneShots|production_scene_unassign_shots|NORMAL_PRODUCT_USE_CASE|
|reorderProductionSceneShots|production_scene_reorder_shots|NORMAL_PRODUCT_USE_CASE|
|exportProjectManifest|project_manifest_export|NORMAL_PRODUCT_USE_CASE|
|getConsistencyScopeBinding|consistency_scope_binding_get|NORMAL_PRODUCT_USE_CASE|
|replaceConsistencyScopeBinding|consistency_scope_binding_replace|NORMAL_PRODUCT_USE_CASE|
|getShotConsistencyBinding|shot_consistency_binding_get|NORMAL_PRODUCT_USE_CASE|
|replaceShotConsistencyBinding|shot_consistency_binding_replace|NORMAL_PRODUCT_USE_CASE|
|getShotContextDraft|shot_context_draft_get|NORMAL_PRODUCT_USE_CASE|
|previewExternalProductionHandoff|external_production_handoff_preview|NORMAL_PRODUCT_USE_CASE|
|confirmExternalProductionHandoff|external_production_handoff_confirm|NORMAL_PRODUCT_USE_CASE|
|listExternalProductionHandoffs|external_production_handoff_list|NORMAL_PRODUCT_USE_CASE|
|getExternalProductionHandoffMappings|external_production_handoff_mappings|NORMAL_PRODUCT_USE_CASE|
|listProjects|project_list|NORMAL_PRODUCT_USE_CASE|
|createProject|project_create|NORMAL_PRODUCT_USE_CASE|
|updateProject|project_update|NORMAL_PRODUCT_USE_CASE|
|getProjectWorkflowConfig|project_workflow_config_get|NORMAL_PRODUCT_USE_CASE|
|upsertProjectWorkflowBinding|project_workflow_binding_upsert|NORMAL_PRODUCT_USE_CASE|
|removeProjectWorkflowBinding|project_workflow_binding_remove|NORMAL_PRODUCT_USE_CASE|
|exportProjectBackup|project_backup_export|NORMAL_PRODUCT_USE_CASE|
|inspectProjectBackup|project_backup_inspect|NORMAL_PRODUCT_USE_CASE|
|restoreProjectBackup|project_backup_restore|NORMAL_PRODUCT_USE_CASE|
|getProjectCommandCenter|project_command_center_get|NORMAL_PRODUCT_USE_CASE|

### Creation

|Export|command / wrapper|用途|
|---|---|---|
|listShots|shot_list|NORMAL_PRODUCT_USE_CASE|
|listReferenceAnchors|reference_anchors_list|NORMAL_PRODUCT_USE_CASE|
|getShot|shot_get|NORMAL_PRODUCT_USE_CASE|
|createShot|shot_create|NORMAL_PRODUCT_USE_CASE|
|updateShot|shot_update|NORMAL_PRODUCT_USE_CASE|
|deleteShot|shot_delete|NORMAL_PRODUCT_USE_CASE|
|reorderShots|shot_reorder|NORMAL_PRODUCT_USE_CASE|
|setShotStageConfig|shot_stage_config_set|NORMAL_PRODUCT_USE_CASE|
|replaceShotReferences|shot_references_replace|NORMAL_PRODUCT_USE_CASE|
|selectShotResult|shot_result_select|NORMAL_PRODUCT_USE_CASE|
|submitShotGeneration|shot_generate|NORMAL_PRODUCT_USE_CASE|
|planShotBatch|shot_batch_plan|NORMAL_PRODUCT_USE_CASE|
|createShotBatch|shot_batch_create|NORMAL_PRODUCT_USE_CASE|
|previewShotBulkImport|preview_shot_bulk_import|NORMAL_PRODUCT_USE_CASE|
|commitShotBulkImport|commit_shot_bulk_import|NORMAL_PRODUCT_USE_CASE|
|bulkAssignShotPrompt|bulk_assign_shot_prompt|NORMAL_PRODUCT_USE_CASE|
|bulkSetShotStageConfig|bulk_set_shot_stage_config|NORMAL_PRODUCT_USE_CASE|
|getReferenceAnchor|reference_anchor_get|NORMAL_PRODUCT_USE_CASE|
|createReferenceAnchor|reference_anchor_create|NORMAL_PRODUCT_USE_CASE|
|updateReferenceAnchor|reference_anchor_update|NORMAL_PRODUCT_USE_CASE|
|deleteReferenceAnchor|reference_anchor_delete|NORMAL_PRODUCT_USE_CASE|
|listPresets|preset_list|NORMAL_PRODUCT_USE_CASE|
|getPreferredPreset|preset_get_preferred|NORMAL_PRODUCT_USE_CASE|
|setPreferredPreset|preset_set_preferred|NORMAL_PRODUCT_USE_CASE|
|createPreset|preset_create|NORMAL_PRODUCT_USE_CASE|
|updatePreset|preset_update|NORMAL_PRODUCT_USE_CASE|
|deletePreset|preset_delete|NORMAL_PRODUCT_USE_CASE|

### Run

|Export|command / wrapper|用途|
|---|---|---|
|getSceneProductionPlan|scene_production_plan|NORMAL_PRODUCT_USE_CASE|
|prepareSceneProduction|scene_production_prepare|NORMAL_PRODUCT_USE_CASE|
|getSceneProductionPreflight|scene_production_preflight|NORMAL_PRODUCT_USE_CASE|
|sceneProductionPreflight|(wrapper)|NORMAL_PRODUCT_USE_CASE|
|getShotProductionPlanDetail|shot_production_plan_detail|NORMAL_PRODUCT_USE_CASE|
|admitSceneProduction|scene_production_admit|NORMAL_PRODUCT_USE_CASE|
|getProjectProductionPreflight|project_production_preflight|NORMAL_PRODUCT_USE_CASE|
|admitProjectProduction|project_production_admit|NORMAL_PRODUCT_USE_CASE|
|getEpisodeProductionPlan|episode_production_plan|NORMAL_PRODUCT_USE_CASE|
|prepareEpisodeProduction|episode_production_prepare|NORMAL_PRODUCT_USE_CASE|
|getSeriesProductionPlan|series_production_plan|NORMAL_PRODUCT_USE_CASE|
|prepareSeriesProduction|series_production_prepare|NORMAL_PRODUCT_USE_CASE|
|getProductionBatchRunbook|production_batch_runbook|NORMAL_PRODUCT_USE_CASE|
|getComfyPreflight|comfy_preflight_current|NORMAL_PRODUCT_USE_CASE|
|submitGeneration|generation_create|NORMAL_PRODUCT_USE_CASE|
|preflightWorkflowExecution|workflow_execution_preflight|NORMAL_PRODUCT_USE_CASE|
|createWorkflowExecution|workflow_execution_create|NORMAL_PRODUCT_USE_CASE|
|createWorkflowExecutionBatch|workflow_execution_create_batch|NORMAL_PRODUCT_USE_CASE|
|createProductionRun|production_run_create|NORMAL_PRODUCT_USE_CASE|
|listProductionRuns|production_run_list|NORMAL_PRODUCT_USE_CASE|
|getProductionRun|production_run_get|NORMAL_PRODUCT_USE_CASE|
|runProductionImages|production_run_run_images|NORMAL_PRODUCT_USE_CASE|
|selectProductionRunAssets|production_run_select_assets|NORMAL_PRODUCT_USE_CASE|
|runProductionVideo|production_run_run_video|NORMAL_PRODUCT_USE_CASE|
|retryProductionVideo|production_run_retry_video|NORMAL_PRODUCT_USE_CASE|
|refreshProductionRun|production_run_refresh|NORMAL_PRODUCT_USE_CASE|
|cancelProductionRun|production_run_cancel|NORMAL_PRODUCT_USE_CASE|
|saveProductionRunTemplate|production_run_template_save|NORMAL_PRODUCT_USE_CASE|
|listProductionRunTemplates|production_run_template_list|NORMAL_PRODUCT_USE_CASE|
|submitGenerationBatch|generation_create_batch|NORMAL_PRODUCT_USE_CASE|
|createProductionQueue|production_queue_create|NORMAL_PRODUCT_USE_CASE|
|listProductionQueues|production_queue_list|NORMAL_PRODUCT_USE_CASE|
|getProductionQueueOverview|production_queue_overview|NORMAL_PRODUCT_USE_CASE|
|getProductionAdmissionStatus|production_queue_admission_status|NORMAL_PRODUCT_USE_CASE|
|getProductionQueue|production_queue_get|NORMAL_PRODUCT_USE_CASE|
|getWorkflowExecutionSummary|production_queue_execution_summary_for_task|NORMAL_PRODUCT_USE_CASE|
|getProductionBatchArtifacts|production_batch_artifacts_get|NORMAL_PRODUCT_USE_CASE|
|getArtifactReviewQueue|artifact_review_queue_get|NORMAL_PRODUCT_USE_CASE|
|openArtifact|artifact_open|NORMAL_PRODUCT_USE_CASE|
|revealArtifact|artifact_reveal|NORMAL_PRODUCT_USE_CASE|
|submitArtifactReview|artifact_review_submit|NORMAL_PRODUCT_USE_CASE|
|resetArtifactReview|artifact_review_reset|NORMAL_PRODUCT_USE_CASE|
|startProductionQueue|production_queue_start|NORMAL_PRODUCT_USE_CASE|
|pauseProductionQueue|production_queue_pause|NORMAL_PRODUCT_USE_CASE|
|cancelPendingProductionQueue|production_queue_cancel_pending|NORMAL_PRODUCT_USE_CASE|
|archiveProductionQueue|production_queue_archive|NORMAL_PRODUCT_USE_CASE|
|restoreProductionQueue|production_queue_restore|NORMAL_PRODUCT_USE_CASE|
|deleteProductionQueue|production_queue_delete|NORMAL_PRODUCT_USE_CASE|
|skipProductionQueueItem|production_queue_skip_item|NORMAL_PRODUCT_USE_CASE|
|requeueProductionQueueItem|production_queue_requeue_item|NORMAL_PRODUCT_USE_CASE|
|requeueProductionQueueItemByItem|production_queue_requeue_item_by_item|NORMAL_PRODUCT_USE_CASE|
|getProductionPartialResumePlan|production_queue_partial_resume_plan|NORMAL_PRODUCT_USE_CASE|
|partialResumeProductionQueue|production_queue_partial_resume|NORMAL_PRODUCT_USE_CASE|
|pickProductionPackageRoot|production_package_pick_root|NORMAL_PRODUCT_USE_CASE|
|discoverProductionPackages|production_package_discover|NORMAL_PRODUCT_USE_CASE|
|listProductionPackageBindings|production_package_bindings_list|NORMAL_PRODUCT_USE_CASE|
|inspectProductionPackage|production_package_inspect|NORMAL_PRODUCT_USE_CASE|
|createProductionPackageBatches|production_package_create_batches|NORMAL_PRODUCT_USE_CASE|

### Library

|Export|command / wrapper|用途|
|---|---|---|
|listPromptLibrary|prompt_library_list|NORMAL_PRODUCT_USE_CASE|
|getPromptLibraryEntry|prompt_library_get|NORMAL_PRODUCT_USE_CASE|
|createPromptLibraryEntry|prompt_library_create|NORMAL_PRODUCT_USE_CASE|
|addPromptLibraryVersion|prompt_library_add_version|NORMAL_PRODUCT_USE_CASE|
|updatePromptLibraryMetadata|prompt_library_update_metadata|NORMAL_PRODUCT_USE_CASE|
|deletePromptLibraryEntry|prompt_library_delete|NORMAL_PRODUCT_USE_CASE|
|listConsistencyProfiles|consistency_profile_list|NORMAL_PRODUCT_USE_CASE|
|getConsistencyProfile|consistency_profile_get|NORMAL_PRODUCT_USE_CASE|
|createCharacterProfile|character_profile_create|NORMAL_PRODUCT_USE_CASE|
|updateCharacterProfile|character_profile_update|NORMAL_PRODUCT_USE_CASE|
|createSceneProfile|scene_profile_create|NORMAL_PRODUCT_USE_CASE|
|updateSceneProfile|scene_profile_update|NORMAL_PRODUCT_USE_CASE|
|createPropProfile|prop_profile_create|NORMAL_PRODUCT_USE_CASE|
|updatePropProfile|prop_profile_update|NORMAL_PRODUCT_USE_CASE|
|createStyleProfile|style_profile_create|NORMAL_PRODUCT_USE_CASE|
|updateStyleProfile|style_profile_update|NORMAL_PRODUCT_USE_CASE|
|deleteConsistencyProfile|consistency_profile_delete|NORMAL_PRODUCT_USE_CASE|
|listCostumeVariants|costume_variant_list|NORMAL_PRODUCT_USE_CASE|
|getCostumeVariant|costume_variant_get|NORMAL_PRODUCT_USE_CASE|
|createCostumeVariant|costume_variant_create|NORMAL_PRODUCT_USE_CASE|
|updateCostumeVariant|costume_variant_update|NORMAL_PRODUCT_USE_CASE|
|deleteCostumeVariant|costume_variant_delete|NORMAL_PRODUCT_USE_CASE|
|listReferenceSets|reference_set_list|NORMAL_PRODUCT_USE_CASE|
|getReferenceSetDetail|reference_set_detail_get|NORMAL_PRODUCT_USE_CASE|
|createReferenceSet|reference_set_create|NORMAL_PRODUCT_USE_CASE|
|updateReferenceSet|reference_set_update|NORMAL_PRODUCT_USE_CASE|
|deleteReferenceSet|reference_set_delete|NORMAL_PRODUCT_USE_CASE|
|createReferenceSetFromAnchor|reference_set_create_from_anchor|NORMAL_PRODUCT_USE_CASE|
|getAssetUsage|asset_usage_get|NORMAL_PRODUCT_USE_CASE|
|getProfileUsage|profile_usage_get|NORMAL_PRODUCT_USE_CASE|
|getReferenceSetUsage|reference_set_usage_get|NORMAL_PRODUCT_USE_CASE|
|getTask|task_get|NORMAL_PRODUCT_USE_CASE|
|listRecentTasks|task_list_recent|NORMAL_PRODUCT_USE_CASE|
|cancelTask|task_cancel|NORMAL_PRODUCT_USE_CASE|
|reconcileActiveTasks|task_reconcile_active|NORMAL_PRODUCT_USE_CASE|
|listAssetsByTask|asset_list_by_task|NORMAL_PRODUCT_USE_CASE|
|listRecentAssets|asset_list_recent|NORMAL_PRODUCT_USE_CASE|
|pickAndImportImage|asset_pick_and_import_image|NORMAL_PRODUCT_USE_CASE|
|importSourceAssets|asset_pick_and_import_source_assets|NORMAL_PRODUCT_USE_CASE|
|pickAndImportVideo|asset_pick_and_import_video|NORMAL_PRODUCT_USE_CASE|
|pickAndImportAudio|asset_pick_and_import_audio|NORMAL_PRODUCT_USE_CASE|
|readAssetImage|asset_read_image|NORMAL_PRODUCT_USE_CASE|
|readAssetThumbnail|asset_read_thumbnail|NORMAL_PRODUCT_USE_CASE|
|getAssetMediaUrl|(wrapper)|NORMAL_PRODUCT_USE_CASE|
|getAsset|asset_get|NORMAL_PRODUCT_USE_CASE|
|inspectAssetDeletion|inspect_asset_deletion|NORMAL_PRODUCT_USE_CASE|
|deleteAssets|delete_assets|NORMAL_PRODUCT_USE_CASE|
|getAssetVideoPrompt|asset_video_prompt_get|NORMAL_PRODUCT_USE_CASE|
|listAssetVideoPrompts|asset_video_prompt_list|NORMAL_PRODUCT_USE_CASE|
|setAssetVideoPrompt|asset_video_prompt_set|NORMAL_PRODUCT_USE_CASE|
|taskHistoryPage|task_history_page|NORMAL_PRODUCT_USE_CASE|
|getTaskDetail|task_get_detail|NORMAL_PRODUCT_USE_CASE|
|getReusableDraft|task_get_reusable_draft|NORMAL_PRODUCT_USE_CASE|
|assetLibraryPage|asset_library_page|NORMAL_PRODUCT_USE_CASE|
|listAssetVersions|asset_versions_list|NORMAL_PRODUCT_USE_CASE|
|listAssetRelations|asset_relations_list|NORMAL_PRODUCT_USE_CASE|
|listGenerationToolUsages|generation_tool_usage_list|INTERNAL_SUPPORT|
|listGenerationAssetVersionLinks|generation_asset_version_link_list|INTERNAL_SUPPORT|
|listAssetTags|asset_tag_list|NORMAL_PRODUCT_USE_CASE|
|createAssetTag|asset_tag_create|NORMAL_PRODUCT_USE_CASE|
|renameAssetTag|asset_tag_rename|NORMAL_PRODUCT_USE_CASE|
|deleteAssetTag|asset_tag_delete|NORMAL_PRODUCT_USE_CASE|
|assignAssetTag|asset_tag_assign|NORMAL_PRODUCT_USE_CASE|
|removeAssetTag|asset_tag_remove|NORMAL_PRODUCT_USE_CASE|
|setAssetFavorite|asset_set_favorite|NORMAL_PRODUCT_USE_CASE|
|bulkSetAssetFavorite|asset_bulk_set_favorite|NORMAL_PRODUCT_USE_CASE|
|bulkAddAssetTag|asset_bulk_add_tag|NORMAL_PRODUCT_USE_CASE|
|bulkRemoveAssetTag|asset_bulk_remove_tag|NORMAL_PRODUCT_USE_CASE|
|listProjectTemplates|project_template_list|NORMAL_PRODUCT_USE_CASE|
|createProjectTemplate|project_template_create|NORMAL_PRODUCT_USE_CASE|
|updateProjectTemplate|project_template_update|NORMAL_PRODUCT_USE_CASE|
|deleteProjectTemplate|project_template_delete|NORMAL_PRODUCT_USE_CASE|
|createProjectFromTemplate|project_template_create_project|NORMAL_PRODUCT_USE_CASE|

### Workflow/Internal

|Export|command / wrapper|用途|
|---|---|---|
|listBatchWorkflowPresets|batch_workflow_presets_list|INTERNAL_SUPPORT|
|createBatchWorkflowPreset|batch_workflow_preset_create|INTERNAL_SUPPORT|
|updateBatchWorkflowPreset|batch_workflow_preset_update|INTERNAL_SUPPORT|
|deleteBatchWorkflowPreset|batch_workflow_preset_delete|INTERNAL_SUPPORT|
|listModels|model_list|ADVANCED_ADMIN|
|getModel|model_get|ADVANCED_ADMIN|
|listModelVersions|model_version_list|ADVANCED_ADMIN|
|getCurrentModelVersion|model_version_current|ADVANCED_ADMIN|
|getModelVersion|model_version_get|ADVANCED_ADMIN|
|listTools|tool_list|ADVANCED_ADMIN|
|listToolInstances|tool_instance_list|ADVANCED_ADMIN|
|listToolVersions|tool_version_list|ADVANCED_ADMIN|
|listToolCapabilities|tool_capability_list|ADVANCED_ADMIN|
|refreshWorkflowLibrary|workflow_library_refresh|ADVANCED_ADMIN|
|pickApiWorkflow|workflow_onboarding_pick_api_workflow|ADVANCED_ADMIN|
|autoOnboardWorkflow|workflow_onboarding_auto_import_api_workflow|ADVANCED_ADMIN|
|analyzeWorkflowImport|workflow_analyze_import|ADVANCED_ADMIN|
|reanalyzeWorkflowImport|workflow_reanalyze_import|ADVANCED_ADMIN|
|commitWorkflowImport|workflow_commit_import|ADVANCED_ADMIN|
|autoConfirmOnboarding|workflow_onboarding_auto_confirm|ADVANCED_ADMIN|
|regenerateWorkflowRecipe|workflow_onboarding_regenerate_recipe|ADVANCED_ADMIN|
|rerecognizeWorkflow|workflow_rerecognize|ADVANCED_ADMIN|
|getOnboardingDraft|workflow_onboarding_get|ADVANCED_ADMIN|
|checkOnboardingCapability|workflow_onboarding_check_capability|ADVANCED_ADMIN|
|setOnboardingMetadata|workflow_onboarding_set_metadata|ADVANCED_ADMIN|
|setOnboardingInputMapping|workflow_onboarding_set_input_mapping|ADVANCED_ADMIN|
|removeOnboardingInputMapping|workflow_onboarding_remove_input_mapping|ADVANCED_ADMIN|
|setOnboardingOutputMapping|workflow_onboarding_set_output_mapping|ADVANCED_ADMIN|
|validateOnboarding|workflow_onboarding_validate|ADVANCED_ADMIN|
|publishOnboarding|workflow_onboarding_publish|ADVANCED_ADMIN|
|discardOnboarding|workflow_onboarding_discard|ADVANCED_ADMIN|
|listWorkflowWorkspace|workflow_workspace_list|ADVANCED_ADMIN|
|listWorkflowProductionWorkspace|workflow_runtime_workspace_list|ADVANCED_ADMIN|
|listWorkflowRegistry|workflow_list_registry|ADVANCED_ADMIN|
|getWorkflowRegistry|workflow_get_registry|ADVANCED_ADMIN|
|getSavedWorkflowVersionDetails|workflow_get_saved_version_details|ADVANCED_ADMIN|
|renameWorkflow|workflow_rename|ADVANCED_ADMIN|
|setWorkflowCurrentVersion|workflow_set_current_version|ADVANCED_ADMIN|
|promoteWorkflowRecipe|workflow_promote_recipe|ADVANCED_ADMIN|
|clearWorkflowRecipePromotion|workflow_clear_recipe_promotion|ADVANCED_ADMIN|
|archiveWorkflowRecipe|workflow_archive_recipe|ADVANCED_ADMIN|
|restoreWorkflowRecipe|workflow_restore_recipe|ADVANCED_ADMIN|
|removeWorkflow|workflow_remove|ADVANCED_ADMIN|
|restoreWorkflow|workflow_restore|ADVANCED_ADMIN|
|purgeWorkflow|workflow_purge|ADVANCED_ADMIN|
|refreshWorkflowProductionWorkspace|workflow_runtime_workspace_refresh|ADVANCED_ADMIN|
|repairBuiltinWorkflowPackage|workflow_repair_builtin_package|ADVANCED_ADMIN|
|setWorkflowEnabled|workflow_set_enabled|ADVANCED_ADMIN|
|recheckWorkflowCapability|workflow_recheck_capability|ADVANCED_ADMIN|
|recheckAllWorkflowCapabilities|workflow_recheck_all_capabilities|ADVANCED_ADMIN|
|duplicateWorkflowRecipe|workflow_duplicate_recipe|ADVANCED_ADMIN|
|compareWorkflowVersions|workflow_compare_versions|ADVANCED_ADMIN|
|exportWorkflowPackage|workflow_export_package|ADVANCED_ADMIN|
|importWorkflowPackageBackup|workflow_import_package_backup|ADVANCED_ADMIN|
|cleanWorkflowStaging|workflow_clean_staging|ADVANCED_ADMIN|
|inspectWorkflowDeletion|workflow_inspect_deletion|ADVANCED_ADMIN|
|inspectWorkflowPurge|workflow_inspect_purge|ADVANCED_ADMIN|
|deleteWorkflowVersion|workflow_delete_version|ADVANCED_ADMIN|
|deleteWorkflowVersionOf|workflow_version_delete|ADVANCED_ADMIN|
|deleteWorkflow|workflow_delete_workflow|ADVANCED_ADMIN|
|restoreWorkflowVersion|workflow_restore_version|ADVANCED_ADMIN|
|listGenerationCatalog|generation_catalog_list|INTERNAL_SUPPORT|
|previewWorkflowBenchmark|workflow_benchmark_preview|ADVANCED_ADMIN|
|createWorkflowBenchmark|workflow_benchmark_create|ADVANCED_ADMIN|
|setWorkflowBenchmarkRecommendation|workflow_benchmark_set_recommendation|ADVANCED_ADMIN|
|saveWorkflowBenchmarkQuality|workflow_benchmark_save_quality|ADVANCED_ADMIN|
|listWorkflowBenchmarks|workflow_benchmark_list|ADVANCED_ADMIN|
|getWorkflowBenchmark|workflow_benchmark_get|ADVANCED_ADMIN|
|setWorkflowBenchmarkWinner|workflow_benchmark_set_winner|ADVANCED_ADMIN|
|cloneWorkflowBenchmark|workflow_benchmark_clone|ADVANCED_ADMIN|
|queueWorkflowBenchmark|workflow_benchmark_queue_existing|ADVANCED_ADMIN|
|deleteWorkflowBenchmark|workflow_benchmark_delete|ADVANCED_ADMIN|
|pickH3LocalImportDirectory|h3_local_import_pick_directory|ADVANCED_ADMIN|
|rescanH3LocalImport|h3_local_import_rescan|ADVANCED_ADMIN|
|commitH3LocalImport|h3_local_import_commit|ADVANCED_ADMIN|
|updateH3ProjectSegmentDraft|h3_local_import_update_project_segment_draft|ADVANCED_ADMIN|
|getWorkflowRecipeHistory|workflow_recipe_history_get|ADVANCED_ADMIN|

### System

|Export|command / wrapper|用途|
|---|---|---|
|ping|ping|INTERNAL_SUPPORT|
|getAppStatus|get_app_status|INTERNAL_SUPPORT|
|getComfyStatus|comfy_get_status|NORMAL_PRODUCT_USE_CASE|
|refreshComfyCapabilities|comfy_refresh_capabilities|NORMAL_PRODUCT_USE_CASE|
|getComfySettings|comfy_get_settings|NORMAL_PRODUCT_USE_CASE|
|testComfyConnection|comfy_test_connection|NORMAL_PRODUCT_USE_CASE|
|saveComfyEndpoint|comfy_save_endpoint|NORMAL_PRODUCT_USE_CASE|
|listComfyEnvironmentProfiles|comfy_environment_profiles_list|NORMAL_PRODUCT_USE_CASE|
|saveComfyEnvironmentProfile|comfy_environment_profile_save|NORMAL_PRODUCT_USE_CASE|
|deleteComfyEnvironmentProfile|comfy_environment_profile_delete|NORMAL_PRODUCT_USE_CASE|
|applyComfyEnvironmentProfile|comfy_environment_profile_apply|NORMAL_PRODUCT_USE_CASE|
|getWorkspaceResume|workspace_resume_get|INTERNAL_SUPPORT|
|saveWorkspaceResume|workspace_resume_save|INTERNAL_SUPPORT|
|freeComfyMemory|comfy_free_memory|INTERNAL_SUPPORT|
|listRuntimeProfiles|runtime_profiles_list|INTERNAL_SUPPORT|
|saveRuntimeProfile|runtime_profiles_save|INTERNAL_SUPPORT|
|deleteRuntimeProfile|runtime_profiles_delete|INTERNAL_SUPPORT|
|listProductionQueueNamePresets|production_queue_name_presets_list|INTERNAL_SUPPORT|
|saveProductionQueueNamePreset|production_queue_name_preset_save|INTERNAL_SUPPORT|
|deleteProductionQueueNamePreset|production_queue_name_preset_delete|INTERNAL_SUPPORT|

### Legacy

|Export|command / wrapper|用途|
|---|---|---|

### Diagnostics

|Export|command / wrapper|用途|
|---|---|---|
|getRuntimeActivityStatus|runtime_activity_status|DIAGNOSTIC|
|getDiagnosticsSummary|diagnostics_summary|DIAGNOSTIC|
|exportDiagnostics|diagnostics_export|DIAGNOSTIC|
|repairJobsStatus|repair_jobs_status|DIAGNOSTIC|
|getProductionAuditSummary|production_audit_summary|DIAGNOSTIC|
|getProductionAuditRecentActivity|production_audit_recent_activity|DIAGNOSTIC|
|getProductionAuditLineage|production_audit_lineage|DIAGNOSTIC|
|getProductionAuditSnapshotDetail|production_audit_snapshot_detail|DIAGNOSTIC|
|getProductionAuditIntegrity|production_audit_integrity|DIAGNOSTIC|

### 全部323 Rust handlers（登记位置 src-tauri/src/lib.rs:1172）

每个名称在以下namespace组中恰列一次。DIRECT_DOMAIN_CRUD取显式create/update/delete/list/get/replace/upsert/reorder等操作名，仅是静态接口形态；archive/publish/prepare/execute等需要更深语义审核，不能自动当CRUD。现有registered handler未证明Legacy，故不捏造LEGACY清单。

#### commands::app — 2

|Command|用途|接口形态|
|---|---|---|
|ping|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|get_app_status|INTERNAL_SUPPORT|DIRECT_DOMAIN_CRUD|
#### commands::diagnostics — 3

|Command|用途|接口形态|
|---|---|---|
|runtime_activity_status|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
|diagnostics_summary|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
|diagnostics_export|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
#### commands::repair_jobs — 1

|Command|用途|接口形态|
|---|---|---|
|repair_jobs_status|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
#### commands::comfy — 6

|Command|用途|接口形态|
|---|---|---|
|comfy_get_status|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_refresh_capabilities|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_get_settings|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_test_connection|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_save_endpoint|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_free_memory|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
#### commands::preflight — 1

|Command|用途|接口形态|
|---|---|---|
|comfy_preflight_current|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::shot_readiness — 4

|Command|用途|接口形态|
|---|---|---|
|shot_readiness_cached|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_preflight|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|scene_readiness_cached|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|scene_preflight|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::settings — 12

|Command|用途|接口形态|
|---|---|---|
|comfy_environment_profiles_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_environment_profile_save|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_environment_profile_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|comfy_environment_profile_apply|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|runtime_profiles_list|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|runtime_profiles_save|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|runtime_profiles_delete|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|production_queue_name_presets_list|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|production_queue_name_preset_save|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|production_queue_name_preset_delete|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|workspace_resume_get|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|workspace_resume_save|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
#### commands::batch_workflow_preset — 4

|Command|用途|接口形态|
|---|---|---|
|batch_workflow_presets_list|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|batch_workflow_preset_create|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|batch_workflow_preset_update|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
|batch_workflow_preset_delete|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
#### commands::workflow_library — 1

|Command|用途|接口形态|
|---|---|---|
|workflow_library_refresh|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::workflow_onboarding — 14

|Command|用途|接口形态|
|---|---|---|
|workflow_onboarding_pick_api_workflow|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_auto_import_api_workflow|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_auto_confirm|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_regenerate_recipe|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_check_capability|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_set_metadata|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_set_input_mapping|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_remove_input_mapping|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_set_output_mapping|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_validate|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_publish|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_onboarding_discard|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_workspace_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::workflow_registry — 17

|Command|用途|接口形态|
|---|---|---|
|workflow_analyze_import|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_reanalyze_import|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_commit_import|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_list_registry|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_get_registry|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_get_saved_version_details|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_rename|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_set_current_version|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_promote_recipe|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_clear_recipe_promotion|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_archive_recipe|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_restore_recipe|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_remove|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_restore|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_inspect_purge|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_purge|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_rerecognize|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::workflow_workspace — 1

|Command|用途|接口形态|
|---|---|---|
|workflow_workspace_query|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::recipe_history — 1

|Command|用途|接口形态|
|---|---|---|
|workflow_recipe_history_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::workflow_lifecycle — 17

|Command|用途|接口形态|
|---|---|---|
|workflow_runtime_workspace_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_runtime_workspace_refresh|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_runtime_diagnostics|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_repair_builtin_package|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_set_enabled|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_recheck_capability|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_recheck_all_capabilities|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_duplicate_recipe|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_compare_versions|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_export_package|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_import_package_backup|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_clean_staging|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_inspect_deletion|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_delete_version|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_version_delete|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_delete_workflow|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_restore_version|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::workflow_benchmark — 10

|Command|用途|接口形态|
|---|---|---|
|workflow_benchmark_preview|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_create|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_set_winner|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_set_recommendation|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_save_quality|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_clone|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_queue_existing|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|workflow_benchmark_delete|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::catalog — 1

|Command|用途|接口形态|
|---|---|---|
|generation_catalog_list|INTERNAL_SUPPORT|COORDINATED_USE_CASE / SUPPORT|
#### commands::generation — 5

|Command|用途|接口形态|
|---|---|---|
|generation_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|workflow_execution_preflight|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|workflow_execution_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|workflow_execution_create_batch|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|generation_create_batch|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::h3_local_import — 4

|Command|用途|接口形态|
|---|---|---|
|h3_local_import_pick_directory|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|h3_local_import_rescan|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|h3_local_import_commit|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|h3_local_import_update_project_segment_draft|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_package — 3

|Command|用途|接口形态|
|---|---|---|
|production_package_pick_root|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_package_inspect|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_package_create_batches|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_package_discovery — 1

|Command|用途|接口形态|
|---|---|---|
|production_package_discover|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_package_provenance — 1

|Command|用途|接口形态|
|---|---|---|
|production_package_bindings_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_queue — 17

|Command|用途|接口形态|
|---|---|---|
|production_queue_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_overview|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_admission_status|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_execution_summary_for_task|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_start|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_pause|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_cancel_pending|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_archive|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_restore|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_skip_item|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_requeue_item|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_requeue_item_by_item|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_partial_resume_plan|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_queue_partial_resume|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::artifact — 6

|Command|用途|接口形态|
|---|---|---|
|production_batch_artifacts_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|artifact_review_queue_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|artifact_open|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|artifact_reveal|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|artifact_review_submit|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|artifact_review_reset|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_audit — 5

|Command|用途|接口形态|
|---|---|---|
|production_audit_summary|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
|production_audit_recent_activity|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
|production_audit_lineage|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
|production_audit_integrity|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
|production_audit_snapshot_detail|DIAGNOSTIC|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_batch_runbook — 1

|Command|用途|接口形态|
|---|---|---|
|production_batch_runbook|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_orchestrator — 11

|Command|用途|接口形态|
|---|---|---|
|production_run_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_run_images|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_select_assets|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_run_video|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_retry_video|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_refresh|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_cancel|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_template_save|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_run_template_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::project — 10

|Command|用途|接口形态|
|---|---|---|
|project_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_workflow_config_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_workflow_binding_upsert|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_workflow_binding_remove|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_backup_export|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_backup_inspect|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_backup_restore|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_manifest_export|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::project_command_center — 1

|Command|用途|接口形态|
|---|---|---|
|project_command_center_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::prompt_library — 6

|Command|用途|接口形态|
|---|---|---|
|prompt_library_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|prompt_library_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|prompt_library_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|prompt_library_add_version|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|prompt_library_update_metadata|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|prompt_library_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::model — 9

|Command|用途|接口形态|
|---|---|---|
|model_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_create|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_update|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_delete|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_version_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_version_current|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_version_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|model_version_create|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::tool — 14

|Command|用途|接口形态|
|---|---|---|
|tool_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_create|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_update|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_delete|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_instance_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_instance_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_instance_create|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_instance_record_health|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_version_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_version_get|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_version_create|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_capability_list|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
|tool_capability_create|ADVANCED_ADMIN|COORDINATED_USE_CASE / SUPPORT|
#### commands::provenance_lineage — 4

|Command|用途|接口形态|
|---|---|---|
|generation_tool_usage_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|generation_tool_usage_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|generation_asset_version_link_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|generation_asset_version_link_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::reference_anchor — 5

|Command|用途|接口形态|
|---|---|---|
|reference_anchors_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_anchor_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_anchor_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_anchor_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_anchor_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::consistency_assets — 25

|Command|用途|接口形态|
|---|---|---|
|consistency_profile_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|consistency_profile_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|character_profile_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|character_profile_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|scene_profile_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|scene_profile_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|prop_profile_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|prop_profile_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|style_profile_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|style_profile_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|consistency_profile_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|costume_variant_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|costume_variant_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|costume_variant_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|costume_variant_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|costume_variant_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_set_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_set_detail_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_set_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_set_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_set_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_set_create_from_anchor|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_usage_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|profile_usage_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|reference_set_usage_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::consistency_bindings — 5

|Command|用途|接口形态|
|---|---|---|
|consistency_scope_binding_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|consistency_scope_binding_replace|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_consistency_binding_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_consistency_binding_replace|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_context_draft_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_structure — 16

|Command|用途|接口形态|
|---|---|---|
|production_structure_tree|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_series_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_series_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_series_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_series_reorder|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_episode_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_episode_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_episode_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_episode_reorder|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_scene_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_scene_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_scene_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_scene_reorder|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_scene_assign_shots|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_scene_unassign_shots|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|production_scene_reorder_shots|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::external_production_handoff — 4

|Command|用途|接口形态|
|---|---|---|
|external_production_handoff_preview|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|external_production_handoff_confirm|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|external_production_handoff_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|external_production_handoff_mappings|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::shot — 10

|Command|用途|接口形态|
|---|---|---|
|shot_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_reorder|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_stage_config_set|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_references_replace|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_result_select|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_generate|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::shot_batch — 2

|Command|用途|接口形态|
|---|---|---|
|shot_batch_plan|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_batch_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::scene_production — 3

|Command|用途|接口形态|
|---|---|---|
|scene_production_plan|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|scene_production_prepare|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|scene_production_readiness_summary|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::episode_production — 3

|Command|用途|接口形态|
|---|---|---|
|episode_production_plan|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|episode_production_prepare|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|episode_production_readiness_summary|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::series_production — 3

|Command|用途|接口形态|
|---|---|---|
|series_production_plan|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|series_production_prepare|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|series_production_readiness_summary|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::production_preparation — 5

|Command|用途|接口形态|
|---|---|---|
|scene_production_preflight|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|scene_production_admit|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|shot_production_plan_detail|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_production_preflight|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_production_admit|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::shot_bulk — 4

|Command|用途|接口形态|
|---|---|---|
|preview_shot_bulk_import|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|commit_shot_bulk_import|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|bulk_assign_shot_prompt|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|bulk_set_shot_stage_config|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::organization — 15

|Command|用途|接口形态|
|---|---|---|
|project_template_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_template_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_template_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_template_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|project_template_create_project|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_tag_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_tag_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_tag_rename|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_tag_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_tag_assign|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_tag_remove|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_set_favorite|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_bulk_set_favorite|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_bulk_add_tag|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_bulk_remove_tag|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::task — 7

|Command|用途|接口形态|
|---|---|---|
|task_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|task_list_recent|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|task_cancel|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|task_reconcile_active|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|task_history_page|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|task_get_detail|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|task_get_reusable_draft|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::preset — 6

|Command|用途|接口形态|
|---|---|---|
|preset_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|preset_create|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|preset_update|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|preset_delete|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|preset_get_preferred|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|preset_set_preferred|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
#### commands::asset — 17

|Command|用途|接口形态|
|---|---|---|
|asset_list_by_task|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_list_recent|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_pick_and_import_image|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_pick_and_import_source_assets|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_pick_and_import_video|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_pick_and_import_audio|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_read_image|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_read_thumbnail|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_library_page|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_versions_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_relations_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|inspect_asset_deletion|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|delete_assets|NORMAL_PRODUCT_USE_CASE|DIRECT_DOMAIN_CRUD|
|asset_video_prompt_get|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_video_prompt_list|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|
|asset_video_prompt_set|NORMAL_PRODUCT_USE_CASE|COORDINATED_USE_CASE / SUPPORT|

## 附录 B：App 全部具名状态与函数责任

|State|line|主要职责/建议owner|
|---|---:|---|
|workspace|215|ROUTING; Route/session intent（breadcrumb派生）|
|activeStudioSection|216|ROUTING; Route/session intent（breadcrumb派生）|
|shotContextPath|217|ROUTING; Route/session intent（breadcrumb派生）|
|shotContextTarget|218|ROUTING; Route/session intent（breadcrumb派生）|
|resumeShotId|219|ROUTING; Route/session intent（breadcrumb派生）|
|videoBatchAssets|220|ROUTING; Route/session intent（breadcrumb派生）|
|focusedTaskId|221|ROUTING; Route/session intent（breadcrumb派生）|
|focusedProductionBatchId|222|ROUTING; Route/session intent（breadcrumb派生）|
|focusedAssetId|223|ROUTING; Route/session intent（breadcrumb派生）|
|focusedCollectionFilter|224|ROUTING; Route/session intent（breadcrumb派生）|
|bootstrapState|225|BOOTSTRAP|
|startupError|226|BOOTSTRAP|
|startupAttempt|227|BOOTSTRAP|
|catalog|228|WORKFLOW; SERVER_STATE readmodel，不是领域事实|
|error|229|UI_LOCAL|
|taskEventsReady|230|TASK_EVENTS|
|taskEventError|231|TASK_EVENTS|
|connectionLoading|232|RUNTIME|
|capabilityLoading|233|RUNTIME|
|reconciling|234|TASK_EVENTS|
|recoveryNotice|235|TASK_EVENTS|
|workflowNotice|236|WORKFLOW; SERVER_STATE readmodel，不是领域事实|
|projectContextLoading|237|PROJECT|
|consistencyProfiles|238|CONSISTENCY; SERVER_STATE readmodel，不是领域事实|
|consistencyReferenceSets|239|CONSISTENCY; SERVER_STATE readmodel，不是领域事实|
|consistencyCostumes|240|CONSISTENCY; SERVER_STATE readmodel，不是领域事实|
|consistencyLoading|241|CONSISTENCY; SERVER_STATE readmodel，不是领域事实|
|consistencyError|242|CONSISTENCY; SERVER_STATE readmodel，不是领域事实|
|productionAdmission|243|PRODUCTION; SERVER_STATE readmodel，不是领域事实|

|具名函数（包括纯helpers/页面函数）|line|职责|
|---|---:|---|
|workflowUseProjectDestination|97|ROUTING|
|resolveProjectCommandCenterNavigation|121|ROUTING|
|workflowDefaultSelection|170|WORKFLOW|
|bindingInput|179|WORKFLOW|
|projectWorkflowBindingsForRecipe|188|WORKFLOW|
|keepsNativeContextMenu|206|UI_LOCAL|
|App|214|ROOT_ASSEMBLY|
|clearNavigationFocus|481|ROUTING|
|applyNavigationFocus|489|ROUTING|
|navigateToRoute|498|ROUTING|
|navigateToWorkspace|514|ROUTING|
|navigateToStudioSection|518|ROUTING|
|openTask|523|ROUTING|
|openShot|528|ROUTING|
|handleShotSelected|533|ROUTING|
|openProject|538|PROJECT|
|openProductionQueue|565|PRODUCTION|
|openProductionQueueFromShot|581|PRODUCTION|
|reconnectComfy|590|RUNTIME|
|retryStartup|603|BOOTSTRAP|
|refreshCapabilities|610|RUNTIME|
|refreshRuntimeAfterEndpoint|625|RUNTIME|
|reloadCatalog|635|WORKFLOW|
|openPublishedWorkflow|639|WORKFLOW|
|openWorkflowForProject|657|WORKFLOW; MUTATION+ROUTING跨域|
|reconcileTasks|715|TASK_EVENTS|
|loadHistoricalInputs|732|SESSION_DRAFT; snapshot→StudioStore→route跨域|
|useAssetInStudio|759|SESSION_DRAFT|
|handleProjectUpdated|775|PROJECT|
|handleProjectRestored|780|PROJECT|
|handleTemplateProjectCreated|788|PROJECT|
|openVideoBatch|804|SESSION_DRAFT|
|openAssetFromShot|810|ROUTING|
|openShotFromAsset|815|ROUTING|
|navigateFromCommandCenter|819|ROUTING|

Arrow/useCallback具名callbacks补充：refreshProductionAdmission@259(PRODUCTION/RUNTIME query)；loadConsistencyBindingPack@459(CONSISTENCY query)；saveConsistencyBindingPack@466(CONSISTENCY mutation)；loadConsistencyContext@474(CONSISTENCY projection)；handleShotContextPathChange@849、handleShotContextPathSelect@852(ROUTING)。匿名JSXeventhandler归属其具名组件/路由职责，不把每个onclick当独立产品usecase。

## 附录 C：全部34个具名useEffect分类

不是34个bug；POLLING或SIDE_EFFECT_MUTATION不强行给没有的effect贴标签。customhooks的event/poll另有生命周期，未计入这些文件ASThook数。

|文件:line-range|类别|职责/异味|
|---|---|---|
|src/app/App.tsx:267–323|INITIAL_LOAD + EVENT_SUBSCRIPTION|bootstrap/subscribetask/recovery，同一effect多生命周期|
|src/app/App.tsx:325–327|INITIAL_LOAD|admissionquery|
|src/app/App.tsx:329–365|INITIAL_LOAD + CROSS_COMPONENT_SYNC|项目列表+resume+Shot校验→多个route字段|
|src/app/App.tsx:367–369|CROSS_COMPONENT_SYNC|projectchange清videoassetintent|
|src/app/App.tsx:371–393|INITIAL_LOAD|projectscopedrecentTasks|
|src/app/App.tsx:395–457|INITIAL_LOAD|consistencyprofiles/referenceSets/costumes汇聚|
|src/features/shots/ShotWorkspace.tsx:344–363|INITIAL_LOAD|projectbindingconfig|
|src/features/shots/ShotWorkspace.tsx:480–482|DERIVED_STATE_SYNC|stage/Shot变化resetpreview|
|src/features/shots/ShotWorkspace.tsx:525–528|CROSS_COMPONENT_SYNC|resumeShot不存在错误|
|src/features/shots/ShotWorkspace.tsx:597–613|CROSS_COMPONENT_SYNC + INITIAL_LOAD|focusbatch→queuefetch|
|src/features/shots/ShotWorkspace.tsx:633–636|CROSS_COMPONENT_SYNC|notifyparentmonitorbatch|
|src/features/shots/ShotWorkspace.tsx:648–648|INITIAL_LOAD|reload七类数据|
|src/features/shots/ShotWorkspace.tsx:650–653|INITIAL_LOAD|multi-packageboard|
|src/features/shots/ShotWorkspace.tsx:655–657|CROSS_COMPONENT_SYNC|breadcrumbs→parent|
|src/features/shots/ShotWorkspace.tsx:659–661|CROSS_COMPONENT_SYNC|cleanup清parentbreadcrumbs|
|src/features/shots/ShotWorkspace.tsx:663–666|CROSS_COMPONENT_SYNC|parentcontexttarget→selection|
|src/features/shots/ShotWorkspace.tsx:668–672|CROSS_COMPONENT_SYNC|initialfilter→localcontrols/stage|
|src/features/shots/ShotWorkspace.tsx:674–677|DERIVED_STATE_SYNC|derivedpage→controls|
|src/features/shots/ShotWorkspace.tsx:679–728|DERIVED_STATE_SYNC + CROSS_COMPONENT_SYNC|selectedShot/config/catalog→editingdraft/name/prompt/reference,dirtyreset风险未复现|
|src/features/shots/ShotWorkspace.tsx:730–743|INITIAL_LOAD|linked/referenceassetfetch|
|src/features/workflows/WorkflowWorkspace.tsx:260–262|INITIAL_LOAD|FASTworkspace|
|src/features/workflows/WorkflowWorkspace.tsx:264–285|INITIAL_LOAD|projectconfig|
|src/features/workflows/WorkflowWorkspace.tsx:287–303|INITIAL_LOAD|runtimeprofiles|
|src/features/workflows/WorkflowWorkspace.tsx:305–340|INITIAL_LOAD|quicktestmodelversions|
|src/features/workflows/WorkflowWorkspace.tsx:1141–1146|DERIVED_STATE_SYNC|ParameterExposurePane mappings→edits|
|src/features/projects/ProjectCommandCenter.tsx:219–228|INITIAL_LOAD + DERIVED_STATE_SYNC|resetmirrorstates/loadaggregate|
|src/features/studio/GenerationStudio.tsx:155–163|CROSS_COMPONENT_SYNC|维护project/workflowdraft兼容，不另resetStudioStore|
|src/features/production/ProductionRunPanel.tsx:259–261|DERIVED_STATE_SYNC|FinalVideoPreview failedidsreset|
|src/features/production/ProductionRunPanel.tsx:323–326|DERIVED_STATE_SYNC|recipe/prompt→H3values|
|src/features/production/ProductionRunPanel.tsx:328–333|CROSS_COMPONENT_SYNC|values→H3promptmirror|
|src/features/production/ProductionRunPanel.tsx:345–358|INITIAL_LOAD|runs10+top5details|
|src/features/production/ProductionRunPanel.tsx:360–366|INITIAL_LOAD|templates|
|src/features/production/ProductionRunPanel.tsx:402–405|INITIAL_LOAD|projectassets|
|src/features/production/ProductionRunPanel.tsx:409–412|INITIAL_LOAD|generatedidsassetrefresh|

## 附录 D：五组件直接 API 调用与函数职责索引

下面完整列出该文件具名tauriClient直接静态调用位置和具名函数，帮助后续从usecase而非行数拆分；customhooks/workflowClient间接调用不含在统计中。函数族职责见第6/7节，purehelper不单独建service。

### src/features/shots/ShotWorkspace.tsx

直接calls：getProjectWorkflowConfig@348；listShots@500；listRecentAssets@501；listPromptLibrary@502；listReferenceAnchors@503；listProductionStructure@504；getProductionBatchRunbook@505；listBatchWorkflowPresets@506；getAsset@737；updateShot@789；setShotStageConfig@800；createShot@815；deleteShot@825；replaceShotReferences@841；replaceShotReferences@872；selectShotResult@887；replaceShotReferences@914；setShotStageConfig@923；submitShotGeneration@944；startProductionQueue@953；getShot@956；requeueProductionQueueItemByItem@968；startProductionQueue@969；submitShotGeneration@974；startProductionQueue@981；exportProjectManifest@1005；bulkSetShotStageConfig@1109；bulkAssignShotPrompt@1120；getProductionBatchArtifacts@1182；pickProductionPackageRoot@1208；bulkSetShotStageConfig@1499。

具名函数：shotContextSurface@117；ProductionModeTabs@134；ShotWorkspace@292；markStageDirty@745；changeStageRecipe@749；changeScalar@775；save@785；addShot@812；removeShot@821；replaceReferences@833；applyReferenceAnchor@848；selectResult@884；generate@892；retryShot@961；loadPrompt@992；exportManifest@1001；configureBulkStage@1094；assignBulkPrompt@1119；openStructureManagement@1128；handleStructureCreate@1134；buildShotContextPath@1595；selectionForContextPathItem@1633；structurePath@1646；seriesLabel@1658；episodeLabel@1662；sceneLabel@1666；isRef2vaRecipe@1670；referenceImagesField@1674；validateRef2vaReferences@1678；ensurePrimaryReference@1690；addOrderedReference@1695；toggleOrderedReference@1700；removeOrderedReference@1706；moveOrderedReference@1710；orderedShotReferences@1718；sameReferenceOrder@1725；isImageAsset@1729；isVideoAsset@1733；isScalarField@1737；defaultScalarValues@1741；projectDefaultForStage@1749；preferredStageRecipe@1756；emptyRunbook@1766；consistencyScopeForSelection@1770；selectionForConsistencyScope@1802。

### src/features/workflows/WorkflowWorkspace.tsx

直接calls：getProjectWorkflowConfig@274；listRuntimeProfiles@292；listModels@315；listModelVersions@318；submitGeneration@802；startProductionQueue@813。

具名函数：WorkflowWorkspace@131；returnToSmartImport@381；resetImportViewForNewWorkflow@386；returnToWorkflowList@392；importWorkflow@409；importBackup@443；toggleVersion@459；registryDeletionInspection@471；inspectForDeletion@501；inspectForPurge@514；confirmWorkflowDeletion@529；restoreArchivedWorkflow@583；restoreExistingArchivedWorkflow@608；openRename@622；saveRename@627；setCurrentVersion@641；deleteVersion@653；promoteRecipe@667；clearRecipePromotion@680；archiveRecipe@693；restoreRecipe@706；recheckVersion@719；repairBuiltinPackage@730；recheckAllVersions@741；duplicateRecipe@755；quickTest@768；compareSelected@824；toggleSelected@833；ParameterExposurePane@1126；patchMapping@1172；InspectPane@1260；NodeCard@1273；CompatibilityPane@1285；InputsPane@1300；OutputsPane@1355；MetadataPane@1373；ValidatePane@1389；PublishPane@1410；SavedVersionDetailsPane@1422；storedWorkflowNodeCount@1456；VersionDiffPane@1462；IssueList@1487；quickTestValues@1491；formatCapability@1526；fieldTypeLabel@1539；SeedModeSelect@1555。

### src/features/projects/ProjectCommandCenter.tsx

直接calls：getProjectCommandCenter@203；getComfyPreflight@242。

具名函数：ProjectCommandCenter@167；refresh@230；repreflight@237；ProjectCommandCenterView@287；deriveProjectCommandCenterSummary@512；deriveProjectCommandCenterAggregateSummary@557；recommendedActionFromAggregate@602；consistencyRecommendedAction@658；ProjectCommandCenterIntegrationSummary@672；ProjectCommandCenterCollectionActions@742；DailyProductionBoard@815；DailyProductionBucketView@863；dailyProductionCollectionNavigation@912；dailyProductionNavigation@944；dailyProductionTargetLabel@970；buildSceneProgress@978；buildProjectCommandCenterIssues@1006；recommendedAction@1024；navigationRequestFromAction@1036；continuityTargetLabel@1048；nextActionReason@1057；FirstTimeUserGuide@1063；ProjectFinalResultSection@1105；auditIssue@1150；SummaryCard@1154；CardHeading@1158；ProgressBar@1162；Stat@1166；LoadingState@1170；preflightStatusLabel@1174；connectionLabel@1181；formatVram@1187。

### src/features/studio/GenerationStudio.tsx

直接calls：refreshWorkflowLibrary@296；getPromptLibraryEntry@329。

具名函数：fieldTypeLabel@45；GenerationStudio@84；refreshWorkflows@292；updateKrea2Resolution@305；applyRuntimeProfile@312；useRecentPrompt@318。

### src/features/production/ProductionRunPanel.tsx

直接calls：getAssetMediaUrl@272；listProductionRuns@347；getProductionRun@349；listProductionRunTemplates@362；assetLibraryPage@374；getAsset@384；saveProductionRunTemplate@470；refreshProductionRun@491；createProductionRun@506；selectProductionRunAssets@697；runProductionImages@700；runProductionVideo@701；retryProductionVideo@702；cancelProductionRun@703。

具名函数：firstField@73；h3PromptKey@77；h3ProfileValue@85；runtimeProfileForH3Profile@89；modeConfig@93；h3ReferenceImageMax@97；h3ReferenceImageMin@105；productionRunSelectionBounds@113；productionRunSelectionError@122；moveProductionRunAsset@138；productionRunSelectionIds@147；productionRunGeneratedAssetIds@157；productionRunModeForRun@165；draftValueMatchesField@175；preserveH3Values@191；normalizedImageCount@200；recipeValueError@206；h3ResolutionSelectionError@211；h3ParameterError@230；statusLabel@234；stageLabel@247；FinalVideoPreview@256；ProductionRunPanel@286；adoptRun@335；refreshAssets@368；updatePrompt@414；applyTemplate@419；changeVideoMode@442；saveTemplate@461；reload@490；createRun@494；execute@525；toggleAsset@581；moveSelectedAsset@598；removeSelectedAsset@602；h3NumericFields@722；numericValue@729；fieldNumericValue@734。

## 附录 E：大模块依赖与责任复核

补充第11节的依赖定义：下列计数是 service struct 直接持有的注入 collaborator（包括 optional，排除 mutex/cache/path/pure内置compiler），不是 transitive依赖或 use行数。三个pure算法模块没有持久注入collaborator，不意味着没有算法调用依赖。

|模块|注入collaborators|Repository ports|内存state / transaction owner|责任簇与render无关的拆分理由|
|---|---:|---:|---|---|
|Backup|1|1|inspectionmap，repository.restore事务+FS补偿|导出/检查/恢复/兼容4簇；协调保持一个authority|
|Onboarding|9|3（含optionalruntime/state）|draftregistry+commitgate；publishing协调ports|导入/识别/映射/验证/发布5簇；内部阶段合适|
|UI normalizer|0|0|局部graph/index，无DB事务|解析/widgetlayout/subgraph转换；pure模块边界|
|H3 local import|5|0直接（通过3services）|sessions map；asset/queue事务委派|scan/inspect/segmentdraft/import/commit5簇|
|Semantic graph|0|0|局部memo/index，无DB事务|graphclosure/evidencequery，cohesion高，不重写|
|Analysis|0|0|局部推断结果，无DB事务|recognition/scoring/schema3簇，pure规则分组|
|Lifecycle|10|5（含optionalrecipe/binding/artifact）|capability/workspacecaches；shared服务locks|读/刷新/enable/delete/restore5簇，保留共享gate|
|Queue|8|4|running/recoverysets+admission/idempotencygates；repo原子settlement|admission/worker/recovery/controls4簇，唯一queue不分裂|
|Generation|14|5（含optionalmodel）|executionregistry/gates/semaphore；task/snapshot/assetrepo写|preflight/compile/inputprepare/execute/collect/provenance6簇|
|Registry|9|7（含optionalregistry/promotion/recipestate/artifact）|lifecyclegate；ports+补偿|registry/current/promotion/lifecycle协调，高关联|
|Orchestrator|6|2|stage_trigger_gate；repository保存编排，queue执行|image/selection/video/retry/template5簇，非第二executor|

三个纯模块0仅表示无service注入持有。Helper/tests混合规模已在第11节列示，不用文件size作为cohesion测量。Backend decomposition 优先让调用方看更小完整接口，不把8个collaborator换成一个ServiceLocator隐藏耦合。
