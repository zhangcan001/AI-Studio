# DEV-023 Production Orchestrator Live Validation

日期：2026-08-16
仓库：`zhangcan001/AI-Studio`
范围：真实本地 ComfyUI、隔离项目 `DEV023 Live Validation`、AI Studio 0.4.0 RC

## Baseline

- DEV-023 开始 SHA：`222adbb07483c962d89a5fd07ff0fc5167584f99`
- 开始时分支：`master`
- 开始时工作区：clean，且 `master` 与 `origin/master` 同步
- 基线：Rust `458 passed / 0 failed`；前端 `46 files / 152 tests`
- 隔离项目：`prj_8ab1eb7e-9db0-4b78-bd38-4d23b2a4f2c8`（`DEV023 Live Validation`）
- v0.3.0 tag、Release、Release assets 与 Runtime Package bytes 未修改

## Environment

- AI Studio source version：`0.4.0`
- SQLite：实际运行 `app.db` 的 `_sqlx_migrations` 为 `1–18`，全部 `success=1`
- Foreign keys：应用 SQLx pool 配置与 Rust migration 测试均为 `PRAGMA foreign_keys = 1`
- Runtime Library：启动时同步 `15` 个 valid packages；Krea2 与 H3 package 均可用
- Workflow diagnostics：`/object_info` HTTP 200，应用 capability cache 正常；未发现阻塞诊断

## ComfyUI

- Endpoint：`http://127.0.0.1:8188`
- `/system_stats`：HTTP 200
- ComfyUI：`0.33.0`
- ComfyUI frontend package：`1.49.6`
- Python：`3.12.10`
- PyTorch：`2.9.0+cu130`
- GPU：`cuda:0 NVIDIA GeForce RTX 5060 Ti`
- Driver：`610.88`
- 观测显存：约 `17.1 GB` 总量；测试期间维持单 GPU、串行调度，没有启用 H3 并发或多 GPU

正式执行链保持不变：

`ProductionOrchestratorService → ProductionQueueService → GenerationService → Task → Snapshot → Asset → Comfy adapter → POST /prompt`

代码搜索确认 `/prompt` 仍只在既有 GenerationService / Comfy adapter 路径上提交；没有新增第二个 queue、executor 或直接 Comfy 调用。

## Krea2 Live

- Workflow：`wfl_kera2_t2i_local_v2`
- Workflow Version：`wfv_2407734d-ff20-44d9-ac7c-15ab514d7193`
- Recipe：`rcp_0575fb13-6bfb-41cb-ba10-eba2719a793c`，version `1.1.0`
- ProductionRun：`prun_f08dd9449a9941258472e98ca9aa74ab`
- Stage：`prst_c7564ad7ef294ea9afae30b33b04e8ce`
- Batch：`pbt_21895d00b3ce4c4b8bfefc2e91fe30e1`
- Batch items：`pbi_54c81838145d4ff28c4e65a4334b6e6e`、`pbi_d7739680d6a84675be6629d55e67b68d`
- Tasks：`tsk_632814f1-8204-468e-9559-b20755ce7e40`、`tsk_cfe9dc59-bf7e-44d1-8e31-8275f4fcd2b8`
- Comfy prompt IDs：`840f3b49-3bbf-4e31-8348-4bc369df1354`、`3c28b96c-6135-4fe1-afee-bd0b8fa3b5b9`
- Snapshots：`snp_fd2c446e-4e2e-472c-9048-4b95ee3954d8`、`snp_c7994009-177f-4ac7-8bd0-e3c62508f6c2`
- Image Assets：`ast_d9eaadd6-2d95-4155-a6d9-0b143b8156f9`、`ast_47f37452-0f39-4c74-8263-6add70a07279`
- 结果：`SUCCEEDED`；两张 PNG 均真实存在、带 thumbnail，`768×1280`，各 `1,109,996` bytes；两张内容 hash 为 `4a6a56d0726a49fbd962b041c9fe7cbc6e5f60166691c146c85ed77e3a9849bd`
- Runtime Provenance：workflow/recipe SHA、`generation_execution_id`、`compiled_workflow_sha256`、`submission_idempotency_key` 与完整 telemetry 均已落库

## Selection

- Selection Stage：`prst_33e0669dd16142199272fe237f8e86db`
- Selection item：`prsi_d0222438fc724d8e8d1ea26d44ae53ea`
- selected Asset：`ast_d9eaadd6-2d95-4155-a6d9-0b143b8156f9`
- 校验：Asset 属于同一项目、同一 Krea2 Stage；Selection item 的 `source_asset_id` 与 `asset_id` 一致
- 结果：Selection `SUCCEEDED`，H3 Stage 已进入 `READY`

## H3 FAST Live

- Workflow：`wfl_minimax_h3_fl2va`；package `minimax_h3_fl2va_1_0_0`
- mode：`FL2VA_IMAGE_TO_VIDEO` / `H3_FAST`
- duration：`5 seconds`
- resolution：`864×480`
- seed：`42023`
- Stage：`prst_11cb4c4846494f4d9728578fc4becb93`
- Batch：`pbt_9216caa3997f45ff8fa47e51f0c75935`
- Batch item：`pbi_5612214006a64af980fac5fcbf8e2603`
- Task：`tsk_e9e460ce-21fb-4be8-8c26-e3709fd101a6`
- Comfy prompt ID：`74da0b57-4f4a-4f09-b09a-ec6442bbabb0`
- Snapshot：`snp_c9ecba03-6743-430e-8871-2377c04a8415`
- Video Asset：`ast_1931c9c9-b1ce-4472-bb9a-127cee865c36`
- input mapping：`source_asset_id=ast_d9eaadd6-2d95-4155-a6d9-0b143b8156f9`，`reference_index=0`
- playback：`ffprobe` 读取到 H.264 video + AAC audio，`864×480`，`5.167s`；MP4 文件 `1,117,028` bytes，thumbnail 存在
- 结果：Task、Stage、ProductionRun 均 `SUCCEEDED`；Video Asset 已进入 Asset Library，Runtime Provenance 与 telemetry 完整

## H3 QUALITY Live

- ProductionRun：`prun_6a76c80db57c4ee0be448f21dc5f5273`
- mode：`FL2VA_IMAGE_TO_VIDEO` / `H3_QUALITY`
- Workflow Version：`wfv_817672cf-3dcb-495e-ad9e-201429ba684d`
- Recipe：`rcp_5fcf5c7e-38f0-4f89-bf37-d6d372c46fa7`
- Stage：`prst_67faf1af21d7494495fc02968d2a9ee9`
- Task：`tsk_25b34274-7d4f-43ba-8792-0e97140a65f6`
- Comfy prompt ID：`546e5611-2574-410a-833f-9560972073ba`
- Video Asset：`ast_a9d60f6f-0156-4bc8-a416-28888aaae194`
- duration / resolution / seed：`1s / 864×480 / 42027`
- quality sampling：compiled snapshot 的 `BasicScheduler` 为 `steps=20`
- playback：真实 MP4，H.264 + AAC，`864×480`，`1.625s`，`150,649` bytes
- 结果：`SUCCEEDED`；Task、Snapshot、Asset、Provenance、telemetry 均存在

## REF2VA Live

- 结果：`NOT EXECUTED — POST-0.4.0 NON-BLOCKER`
- 原因：当前 Production Run UI 没有可靠的 REF2VA mode 选择入口；本轮不强塞复杂 UI，也不将其伪装为 live pass。现有 H3 mode/runtime helper 与后端 coverage 保留，后续单独开放真实三图顺序验证。

## Restart Recovery

- 成功数据重启：AI Studio 正常关闭后重新启动，隔离项目、ProductionRun 历史、Stage、Task、Asset 与视频均仍可从 UI/数据库读取；未操作 ComfyUI 进程。
- Active-task Run：`prun_b10ff17e13d54f90891e170f0e645c22`
- H3 Task：`tsk_dcd9e35e-de46-42fa-8f1b-9fa85e4b0abb`
- 原 prompt ID：`61941478-b905-47bb-bd43-c7dbebcb53a2`
- generation execution：`gen_b38d9b267b3d4b5a788ad3788e7a075cf4915c1c07f11430d33aca83d6ecc6eb`
- submission key：`production-item:pbi_53eee26b51604dbda7e1e23aabb8f301`
- 重新启动日志：`examined=1 succeeded=1 failed=0 deferred=0 unresolved=0`
- 结果：重启前后同一 Task / prompt / execution / submission identity；该 H3 最终收集为一个 Video Asset `ast_0d80a742-033a-4a42-959f-1ace9a63e7e1`，没有重复 Task、BatchItem 或 Asset

## Duplicate Submission

- Run：`prun_a13d67478ec24dfdb0323497cc6d5976`
- 对同一 H3 Stage 连续两次调用 `run video`
- 第一调用创建唯一 H3 item `prsi_f19c551283a941a495ecf100120d8eb3`、Task `tsk_7a99fefe-94a7-49ca-92b6-8a4da47aabbd`、Asset `ast_ccc1f993-9265-40d2-9036-f7e53d19ddb1`
- 第二调用被后端拒绝：`H3 Stage 已经创建批次，重复触发不会创建第二个任务。`
- 数据库：H3 item count `1`，对应 submission key 的 Task count `1`
- 结果：duplicate `/prompt`、Task、BatchItem、output Asset 均为 `0`

## Retry

- Run：`prun_68bacbe7b6e14d74bbd1374cb1e5d38a`
- 原失败 Task：`tsk_98822304-2103-4fa5-a2df-48d171f66cdd`
- 原失败：`EXECUTION_INTERRUPTED`，`execution interrupted at node 3`；通过 ComfyUI 官方 `POST /interrupt` 安全中断，未破坏 runtime package 或全局设置
- 原 item：`prsi_75598873890c4e068827f639878d50b4`，attempt `1`，无 output Asset
- Retry item：`prsi_998d1b79df4c4a3d9926a5e4212a308e`，attempt `2`，parent 指向原 item
- Retry Task：`tsk_4b880203-844f-49a2-8786-0b2e2aa78ef9`
- Retry execution：`gen_bdad6e08638627a2bc348a5f4bc719a399387b1ae96d8986e309089e46df7d55`
- Retry Asset：`ast_3e6c7e71-0c5f-41dd-b38d-be72d84cfe79`，MP4 `864×480`、`5.167s`、`697,351` bytes
- 结果：原失败 evidence 保留，retry 成功，Krea2 未重跑；Run 为合理的 `PARTIAL_FAILED`

## Lineage

- Forward：ProductionRun → Krea2 Stage/Batch/Task → Image Asset → Selection item/source Asset → H3 Stage/Batch/Task → Video Asset
- Reverse：H3 `source_asset_id` 反向指向被选择的 Krea2 Image Asset；所有对象均为隔离项目 `prj_8ab1eb7e-9db0-4b78-bd38-4d23b2a4f2c8`
- 结果：数据库 join 与 Production Run diagnostics 均能证明正向、反向 lineage；Task snapshot、runtime provenance、telemetry、submission identity 均保留

## 0.4.0 Productization

- Production Run 现在通过独立 `Production Run` tab 直接可达；原先只在 Prompt Library 间接进入的问题已修复
- 面板使用既有 `h3RecipeForMode`，按 `FL2VA_IMAGE_TO_VIDEO` 正确选择 FAST / QUALITY I2V Recipe，不再取目录中第一个任意视频 Recipe
- 普通用户流程可见：Run 名称、Krea2 数量、Run Images、生成图片选择、H3 prompt、FAST/QUALITY、duration、width/height、Run Video、Final Video Asset
- 状态：No Runs、loading、Krea2 图片加载、H3 running、H3 failed/Retry、Runtime unavailable/error 均有非空状态或错误提示
- Button safety：busy 状态禁用重复触发/重试；后端 stage gate 与 submission idempotency 继续生效
- Diagnostics：可展开查看 Run/Stage/Workflow Version/Recipe/Batch/frozen config/Task/Asset/source/reference_index/attempt/submission/error；任务按钮仍可打开完整 Task history

## Regression

- `cargo fmt --all -- --check`：PASS（在 `src-tauri` crate 目录执行）
- `cargo check --manifest-path src-tauri/Cargo.toml`：PASS
- `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`：`458 passed / 0 failed`
- `pnpm test`：`46 files / 152 tests passed`
- `pnpm build`：PASS；仅有既有 Vite chunk-size advisory
- `git diff --check`：PASS

## Migration

- latest migration：`018`
- Fresh DB：Rust migration test PASS，`001 → 018`
- Upgrade DB：legacy/temporary DB through `001 → 018` PASS；当前用户 DB 实际已是 `018`
- schema drift：未发现；没有创建 migration `019`

## Backup

- `BACKUP_VERSION`：`9`
- v1–v8：固定 legacy fixtures inspect/restore PASS
- v9：manifest version `9` 与 round-trip PASS
- ProductionRun roundtrip：Rust backup tests 覆盖 ProductionRun、Stages、StageItems、Templates、Benchmark 数据与 asset lineage restore；PASS

## Installer

- Source RC SHA：`bed395a6b610e433df03339a2af62b6a6e61eadf`
- `pnpm tauri build`：PASS
- Embedded Build Commit：与 Source RC SHA 一致；release executable 中可检索到该 SHA
- 产物：standalone、MSI、NSIS 均成功生成，逐项 SHA-256 见 `docs/RELEASE_SHA256_0.4.0.txt`
- 约束：未创建 tag、GitHub Release 或上传 installer；未覆盖 `0.3.0` SHA 文件，也未修改 Runtime Package bytes

## Clean Install

结果：`PARTIAL CLEAN SMOKE`。

- Installer：`AI Studio_0.4.0_x64-setup.exe`
- 安装命令：NSIS silent install，退出码 `0`
- 隔离目录：`C:\Users\ADMIN\AppData\Local\Temp\AIStudio_DEV023_0.4.0_install`
- 启动验证：installed `ai-studio.exe` 正常启动；backend/database `ready`，version `0.4.0`，正式版模式，ComfyUI `CONNECTED`，workflow packages `15/15` valid
- UI 验证：Production Run tab 可见，面板显示 `Prompt → Krea2 → 选图 → H3`，空状态显示 `暂无 Production Run，请先新建一个运行。`
- 重启验证：installed app 正常关闭后再次启动，上述 status/diagnostics 保持一致；随后已正常关闭
- 边界：Windows Tauri LocalDataDir 未提供可用的临时数据根覆盖，因此没有声称 fresh DB clean install；这次是安装/启动/UI smoke，不是 full clean live pass

## Upgrade

结果：`PARTIAL UPGRADE SMOKE`，正式用户数据未被改写。

- 已从隔离的 v0.3.0/legacy snapshot 创建临时副本：`C:\Users\ADMIN\AppData\Local\Temp\AIStudio_DEV023_upgrade_20260816\AIStudio\AIStudioData\app.db`
- 副本起点：migration `015`；projects `9`、tasks `229`、assets `291`、snapshots `215`、production_batches `66`、workflows `9`、workflow_versions `15`、recipes `17`
- SQLite migration code：fresh/upgrade Rust tests 均通过 `001 → 018`；实际用户 DB 当前为 migration `018`
- 限制：Windows Tauri `local_data_dir()` 不受本次 `LOCALAPPDATA` 临时环境变量重定向影响，无法让 installer 进程安全地指向上述副本；因此没有把“安装器对复制的 v0.3 数据升级”冒充成 full pass
- 结论：数据库升级实现 PASS；installer against copied user data 未执行，保留为 post-0.4.0 可重复性工作

## Known Issues

- BLOCKER：无；基础 Krea2 → Selection → H3 FAST → Video Asset、重启恢复、重复提交保护、lineage 与代码门禁均已通过
- NON-BLOCKER：REF2VA 本轮未执行，按范围记为 POST-0.4.0；0.4.0 不强制开放复杂 REF2VA UI
- NON-BLOCKER：fresh DB clean install 与 installer against copied v0.3 data 的完整隔离 smoke 受 Windows Tauri 数据目录不可重定向限制，已完成 partial smoke，不影响已通过的核心 live/code gates
- POST-0.4.0：REF2VA 三图真实 ComfyUI 验证、更完整的 reference_index 审核工作区，以及可控数据根下的 installer upgrade smoke

## Final Decision

`AI STUDIO 0.4.0 RELEASE CANDIDATE PASS`

核心 Krea2 → Selection → H3 FAST → Video Asset live chain、重启恢复、重复提交保护、失败重试、lineage、H3 QUALITY 扩展验证、代码回归、migration、backup 与 RC build gates 均通过。REF2VA 与完整 fresh/upgrade installer isolation 按上文明确保留为 non-blocker/post-0.4.0；本任务不执行 tag、Release 或上传。

## RC Metadata

- SOURCE_RC_SHA：`bed395a6b610e433df03339a2af62b6a6e61eadf`
- Embedded Build Commit：`bed395a6b610e433df03339a2af62b6a6e61eadf`
- Artifact SHA-256：见 `docs/RELEASE_SHA256_0.4.0.txt`
