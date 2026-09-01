# DEV-071 — Real Project Production Pilot V1 Closure

状态：`PASS — REAL PROJECT PRODUCTION PILOT V1`

审计日期：2026-09-01（数据库与 ComfyUI 时间戳为 UTC）

本报告固化 AI Studio 0.9.0 的真实多集 Production Pilot 证据。证据来自真实 Production Package manifest、`production_package_batch_bindings`、Production Batch / Item、Task、Task Event、Asset、输出目录和 ComfyUI history；这不是 Unit Test、Fake Comfy、Mock 或 Synthetic DB-only test。首帧是程序化测试图，但视频生成实际经过 H3 / ComfyUI 生产链并落盘为真实视频文件。

## 1. Pilot Goal

验证一个真实 Season 下的三个 Production Package 是否能够经由 durable package identity 绑定到正确 Batch，按用户的既有 Queue 手动启动，完成真实 H3 / ComfyUI I2V 生产，并在 Batch、Task、Asset、文件和 UI 收敛层面形成可审计的闭环。

本次审计只读应用数据库、manifest、输出文件和 ComfyUI HTTP 状态；没有修改数据库，也没有修改产品版本或生产代码。

## 2. Test Package

真实素材包根目录：

`C:\Users\ADMIN\Desktop\AI_Studio_DEV071_Real_Project_Pilot\AI_Studio_DEV071_Real_Project_Pilot\SeasonPilot_玄门异象`

| Package | Manifest item 数 | mode | duration | frame size | 首帧 PNG |
|---|---:|---|---:|---|---:|
| `EP01_山门异象` | 8 | `I2V` | 5 s | 960×544 | 8 |
| `EP02_古寺追踪` | 8 | `I2V` | 5 s | 960×544 | 8 |
| `EP03_夜殿决战` | 8 | `I2V` | 5 s | 960×544 | 8 |
| **合计** | **24** | **全部 I2V** | **全部 5 s** | **全部 960×544** | **24** |

manifest 重读确认：`schemaVersion=1`、`packageType=AI_STUDIO_VIDEO_PRODUCTION`，全部 24 个 item 的 `mode=I2V`，全部带 `firstFrame`。

## 3. Environment

| 项目 | 实际值 |
|---|---|
| Git branch | `master` |
| Git baseline | `HEAD = origin/master = fe41fa35574433cf7f80ca9a1eb22b401c116e2e` |
| Product version | `0.9.0` |
| Release | `v0.9.0`（release source `80448f37c640658d601f9507c33f92796cad9751`） |
| Migration | `026` — `production package batch bindings` |
| Migration 027 | `ABSENT` |
| Application DB | `C:\Users\ADMIN\AppData\Local\AIStudio\AIStudioData\app.db` |
| Latest release binary | `C:\Users\ADMIN\Documents\ChatGPT\AI Studio\src-tauri\target\release\ai-studio.exe` |
| Persisted task app version | `0.9.0`（24/24） |
| Persisted task build commit | `d0376947fa9f5920bac100e7176948f98488f11c`（24/24） |
| Formal workflow | `minimax_h3_fl2va_1_0_0` / workflow `1.0.0` / recipe `1.0.0` |
| Runtime profile | `H3_FAST` / `GPU_STANDARD_SERIAL` |
| ComfyUI | `0.33.4`，HTTP `/system_stats=200`，`/object_info=200` |
| Python | `3.12.10` |
| GPU | `NVIDIA GeForce RTX 5060 Ti` |
| Comfy node catalog | 4525 nodes |

## 4. Production Package Evidence

以下关联全部来自 `production_package_batch_bindings` 的 `package_key`，不是根据 Batch name 模糊猜测。每个 binding 均为 `source_kind=PRODUCTION_PACKAGE`、`project_id=prj_default`、`chunk_index=0`、`chunk_count=1`。

| Package ID / name | PACKAGE_KEY | MANIFEST_SHA256（DB = 实际文件） | manifest bytes | schema / type | item 数 |
|---|---|---|---:|---|---:|
| `DEV071-EP01` / `EP01 山门异象` | `ef0e6664eb2c464b8afc71c74797aecb21245338864d5e60355c16afee946a3e` | `e024cd44fa5f120d7db7f6306c0bbf22551ea8e10f9515c654c71e15ef236170` | 4865 | `1` / `AI_STUDIO_VIDEO_PRODUCTION` | 8 |
| `DEV071-EP02` / `EP02 古寺追踪` | `989037e820f1e03c4ee2dfc806caf2698931fb68f53ac7fc666af41792630028` | `9ee281496479b72a7c9fadd1e3d66ecc8090f3021feffc8a5f64ff123c971c4f` | 4787 | `1` / `AI_STUDIO_VIDEO_PRODUCTION` | 8 |
| `DEV071-EP03` / `EP03 夜殿决战` | `645f2d2c43ea67941e6dca29bc4d6559f6627ac8722613bd6ff23b9f4e6987c8` | `73b0c054f64e01edcca1e8bdd894c7b0277ba9766e7e5595f746a337625df5ad` | 4796 | `1` / `AI_STUDIO_VIDEO_PRODUCTION` | 8 |

`PACKAGE_COUNT=3`、`ITEM_COUNT=24`、manifest item IDs 与 binding item IDs 匹配 `24/24`。

## 5. Batch / Item Truth

| Package | PACKAGE_KEY | BATCH_ID | Batch status | created_at | updated_at | total | succeeded | failed | cancelled | pending | running |
|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|
| EP01 | `ef0e6664…946a3e` | `pbt_af7fc72b1a204fdd96b5c70cdb96510a` | `COMPLETED` | 13:47:21.499656300 | 14:01:52.420663700 | 8 | 8 | 0 | 0 | 0 | 0 |
| EP02 | `989037e8…30028` | `pbt_3693131dbdc94eeb8839aa55ac70a63f` | `COMPLETED` | 13:47:36.161936400 | 14:32:22.772774600 | 8 | 8 | 0 | 0 | 0 | 0 |
| EP03 | `645f2d2c…987c8` | `pbt_336dee8a655c4365bc175d5b689227fa` | `COMPLETED` | 13:47:36.290909000 | 14:18:29.180891800 | 8 | 8 | 0 | 0 | 0 | 0 |

全局数据库 truth：

```text
BATCH_COUNT       = 3
TOTAL_ITEMS       = 24
SUCCEEDED_ITEMS   = 24
FAILED_ITEMS      = 0
CANCELLED_ITEMS   = 0
PENDING_ITEMS     = 0
RUNNING_ITEMS     = 0
```

本次三个实际启动的 Package 均已进入 terminal `COMPLETED`；不存在把未启动 Package 伪造成失败的情况。

## 6. Task Truth

| Package | Task total | succeeded | failed | cancelled | unique task IDs | parent task | submission attempt |
|---|---:|---:|---:|---:|---:|---:|---:|
| EP01 | 8 | 8 | 0 | 0 | 8 | 0 | `[1]` |
| EP02 | 8 | 8 | 0 | 0 | 8 | 0 | `[1]` |
| EP03 | 8 | 8 | 0 | 0 | 8 | 0 | `[1]` |
| **合计** | **24** | **24** | **0** | **0** | **24** | **0** | **24/24 为 1** |

Task lineage 检查：

- `TASK_CREATED=24`、`TASK_SUBMISSION_CONFIRMED=24`、`TASK_QUEUED=24`、`TASK_RUNNING=24`、`TASK_COLLECTING=24`、`TASK_SUCCEEDED=24`。
- `TASK_ID` 唯一数为 `24/24`；每个 item 恰有一个关联 Task；不存在重复 Task、孤儿 Task 或无 lineage 的额外 Task。
- Task error fields 与 Item error fields 均为 0 条；24 个 `prompt_id` 唯一。

结论：`TASK_LINEAGE=PASS`。

## 7. Asset / File Truth

输出目录：

`C:\Users\ADMIN\AppData\Local\AIStudio\AIStudioData\projects\prj_default\assets\generated\video`

| Package | video Asset | file exists | non-empty | total bytes | min bytes | max bytes | avg bytes |
|---|---:|---:|---:|---:|---:|---:|---:|
| EP01 | 8 | 8 | 8 | 4,342,416 | 330,005 | 1,002,828 | 542,802.00 |
| EP02 | 8 | 8 | 8 | 8,116,403 | 330,125 | 3,136,175 | 1,014,550.38 |
| EP03 | 8 | 8 | 8 | 4,996,281 | 330,532 | 1,431,814 | 624,535.12 |
| **合计** | **24** | **24** | **24** | **17,455,100** | **330,005** | **3,136,175** | **727,295.83** |

数据库与文件核验：

```text
VIDEO_ASSET_TOTAL = 24
VIDEO_FILE_EXISTS = 24
VIDEO_FILE_MISSING = 0
FILE_NON_EMPTY     = 24
DB_SHA256_MATCH    = 24/24
SIZE_MATCH         = 24/24
ASSET_TYPE         = video (24/24)
ASSET_CATEGORY     = generated_video (24/24)
ITEM_TASK_ASSET_JOIN = 24/24
```

每个 `SUCCEEDED ProductionBatchItem` 均有一个 video Asset，`storage_path` 存在、文件大小大于 0，实际 SHA-256 与数据库记录一致。

## 8. Production Completion

以下只使用数据库最终 truth：

```text
EP01_TOTAL          = 8
EP01_SUCCEEDED      = 8
EP01_FAILED         = 0
EP01_FINAL_STATUS   = COMPLETED

EP02_TOTAL          = 8
EP02_SUCCEEDED      = 8
EP02_FAILED         = 0
EP02_FINAL_STATUS   = COMPLETED

EP03_TOTAL          = 8
EP03_SUCCEEDED      = 8
EP03_FAILED         = 0
EP03_FINAL_STATUS   = COMPLETED
```

## 9. Manual Start Safety

三个 Batch 的创建时间为 13:47:21–13:47:36；第一个 Task 创建时间为 13:47:46.339137600，第一个 Task 开始时间为 13:47:48.452994900。创建多 Package Batch 时没有同步自动创建或自动启动 Task。

正式生成来自用户已有 Production Queue 的手动启动；该操作路径和本轮通过结论由用户确认，数据库时间线同时显示 Batch 创建与 Task 创建/启动分离。

```text
AUTO_START      = NO
AUTO_NEXT       = NO
AUTO_RETRY      = NO
START_ALL       = NO
SECOND_QUEUE    = NO
SECOND_EXECUTOR = NO
DIRECT_COMFY    = NO
FORMAL_EXECUTOR = COMFYUI
```

## 10. Retry

本轮没有失败后重试：

```text
FAILED_FIRST_ATTEMPT = 0
RETRIED_ITEMS        = 0
RETRY_SUCCEEDED      = 0
RETRY_USED           = NO
```

24/24 Task 的 `submission_attempt=1`，24/24 Item 的 `retry_of_item_id` 为空；没有编造 Retry 测试或 Retry lineage。

## 11. UI Convergence

用户已确认本轮 DEV-071 测试通过，且数据库独立显示三个 Batch 为 `COMPLETED`、Item 的 `PENDING/RUNNING` 均为 0；本次文档审计没有重新截取 UI 截图，因此以下 PASS 明确属于“用户确认 + 数据库终态交叉验证”，不冒充新的截图证据：

```text
BOARD_CONVERGENCE  = PASS (user-confirmed; DB terminal truth)
QUEUE_CONVERGENCE  = PASS (user-confirmed; pending=0, running=0)
MONITOR_CONVERGENCE = PASS (user-confirmed; DB terminal truth)
```

ComfyUI 当前队列也已清空：`/queue=200`、`queue_running=0`、`queue_pending=0`。

## 12. Restart Evidence

```text
RESTART_TESTED = NO
```

本次 DEV-071 审计没有执行或捕获 App restart，因此不把 DEV-069 的 restart UAT 冒充为 DEV-071 真人 Pilot restart。当前数据库中的 Batch、Task、Asset、Package binding 均完整可关联，但这不等于本次执行过 restart 测试。

## 13. Performance Evidence

以下为 24 个真实 Task 的数据库时间戳计算，未估算：

```text
FIRST_TASK_START  = 2026-09-01T13:47:48.452994900+00:00
LAST_TASK_FINISH  = 2026-09-01T14:32:22.678212400+00:00
TOTAL_WALL_TIME   = 2674.225218 s  (~44m34.225s)
AVG_ITEM_WALL_TIME = 95.42 s       (started_at -> finished_at)
```

分集窗口：EP01 `843.366344 s`、EP02 `739.627680 s`、EP03 `762.163615 s`。这些数字是实际 Task lifecycle 的 start-to-finish 观测值，覆盖本次已记录的 ComfyUI 提交、排队/执行、模型加载与生成、输出收集和文件落盘链路作为一个整体；数据库没有提供可独立审计的 queue/model-load/generation/file-I/O 分段耗时，因此不做分段估算或画质评分。

```text
PERFORMANCE_DATA = RELIABLE_DB_TASK_LIFECYCLE
```

## 14. Observed Problems

本轮用户没有报告真实 P0/P1/P2/P3 或 UX 问题；没有从数据中自行制造问题：

```text
P0 = 0
P1 = 0
P2 = 0
P3 = 0
UX = NONE_REPORTED
```

ComfyUI 实际执行证据：24 个 `TASK_SUBMISSION_CONFIRMED`、24 个唯一 prompt；24/24 `/history/<prompt_id>` 返回 HTTP 200、`status.completed=true`、`execution_success`，`execution_error=0`、`execution_interrupted=0`，且 `/queue` 最终为 running 0 / pending 0。捕获证据范围内未观察到连接错误：

```text
COMFY_SUBMISSIONS       = 24
COMFY_FAILURES          = 0
COMFY_CONNECTION_ERRORS = 0 observed in captured pilot evidence
```

最后一项只表示本轮 24 个实际执行和任务事件范围内未观察到连接错误，不声称替代完整 ComfyUI 历史服务日志审计。

## 15. Final Decision

本轮实际启动的 3 个 Batch 全部 terminal；24 个 Item、24 个 Task、24 个 video Asset 和 24 个输出文件全部成功，Package → Batch durable binding、Item → Task → Asset join、文件存在性及 SHA-256 均通过；没有自动启动下一批、视频缺失、永久 stale UI 或真实 P0/P1 问题。用户已确认测试通过。

```text
DEV-071 = PASS — REAL PROJECT PRODUCTION PILOT V1
```

架构冻结检查：

```text
PRODUCT_VERSION  = 0.9.0
MIGRATION        = 026
MIGRATION027     = ABSENT
AUTO_START       = NO
AUTO_NEXT        = NO
AUTO_RETRY       = NO
START_ALL        = NO
SECOND_QUEUE     = NO
SECOND_EXECUTOR  = NO
DIRECT_COMFY     = NO
FORMAL_EXECUTOR  = COMFYUI
MULTI_AGENT      = CONFIRMED
MULTITHREAD_USED = NO
ACTIVE_SUBAGENTS = 0
```

本 DEV 仅新增本文件作为验收记录；提交、Source-only CI run 与最终 `master` hash 在任务最终报告中记录。
