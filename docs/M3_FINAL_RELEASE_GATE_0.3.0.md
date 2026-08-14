# AI Studio 0.3.0 Simplified Final Live Gate

Date: 2026-08-14
Scope: Product Scope Realignment；禁止回到旧 Shot 主路径或新增其他产品能力。

## Current status

`AI STUDIO 0.3.0 = CODE READY / LIVE VALIDATION DEFERRED`

当前代码目标是两个独立产品：批量图片（Krea2）和批量视频（MiniMax H3）。旧 Shot 数据与后端保持兼容，但不作为普通导航入口。本轮明确不执行 GPU、Computer Use、Desktop Live Gate、tag、GitHub Release 或二进制上传；Live validation 标记为 `DEFERRED BY PRODUCT OWNER`，不是失败。

## Post-Benchmark automated code gate — 2026-08-14

本轮在 source baseline `db5faa4ea47ab8efcec767bcbcec5a349f57dfa3` 上完成最终自动化回归；没有执行 GPU 或真实桌面 Live Gate：

- Rust：`415 passed / 0 failed`。
- Frontend：`46 files / 152 tests / 0 failed`。
- `cargo fmt --all -- --check`、`cargo check`、`pnpm build`、`git diff --check`：PASS。
- Fresh migration `001→014`、现有库 `012→013→014` upgrade、Backup v7 round-trip/remap、v1–v6 fixed compatibility、Benchmark chain、H3 compatibility gate、ReferenceManifest 与 Krea2 regression：PASS。
- `pnpm tauri build`：本轮未执行；旧本地候选产物及 SHA 已标记为 `STALE AFTER POST-GATE SOURCE CHANGES`。

最终状态仍为：`AI STUDIO 0.3.0 = CODE READY / LIVE VALIDATION DEFERRED`。

## Code and compatibility gate

| Gate | Current boundary |
| --- | --- |
| Exact runtime scope | Krea2 只按精确 workflow ID 进入批量图片；MiniMax H3 只按 FAST 的 `wfl_minimax_h3_fl2va` / `wfl_minimax_h3_reference_video` 或 QUALITY 的四个 mode-specific workflow ID 进入批量视频；不创建第三 Runtime。 |
| Asset video prompt | 图片 Asset 的提示词持久化、项目隔离、非空和 64 KiB 校验已接入。 |
| Backup compatibility | Backup v7 保存/恢复 Asset 视频提示词、审片/返工版本、Workflow Benchmark 实验/候选与生产队列引用，正确 remap IDs，并继续接受 v1–v7。 |
| Queue contract | 两个入口都创建持久化 Production Queue；输入、参数和随机 Seed 在创建时冻结；严格串行。 |
| H3 input sources | MiniMax H3 支持 Asset Library 与一个仅限 `PROJECT_FOLDER` 的普通本地导入入口：每个一级子文件夹对应一个 Segment。旧配对/清单/文本/首尾帧格式保留后端兼容，但不在普通 UI 展示。所有 Source Image/Video/Audio 先进入正常 Asset，再进入同一 Production Queue；Queue 与 Snapshot 不保存外部绝对路径。 |
| Resolution contract | Krea2 提供 8 个官方宽高比及 1K/2K 预设；H3 提供图片规格中的 14 档 16:9 输出梯度（0.2–2.0 MP）；Krea2/H3 自定义 width/height 均按 Recipe min/max/step 校验，不自动取整。 |
| H3 Recipe contract | 只接受经本机 `/object_info` 与 graph 审计的精确语义键；FL2VA 支持 `prompt` / optional `first_frame` / optional `last_frame`，REF2VA 支持 plural `reference_images` / `reference_videos` / `reference_audios`；`duration_seconds` 为 1–15 秒、step 1、默认 5 秒，并要求 width/height integer 与 video output。 |
| H3 Recipe selection audit | 默认/推荐 profile 为 QUALITY（四个不可变 `2.0.0` mode-specific 包、20 步）；FAST 保留旧 FL2VA `1.0.0` / Omni REF2VA `1.3.0` 的 4 步 Turbo 图且未修改。Project Folder 按 mode + profile 选择并冻结 workflow/Recipe；普通工作区已按 `workflowVersionId + recipeId` 支持多个正式 Recipe 的显示、手动选择与推荐/兼容回退。 |
| Workflow Parameter Exposure | 从既有 Workflow Version 创建新 Recipe draft；仅开放安全 literal input。发布时原始 Workflow JSON bytes 与 Workflow SHA 保持不变，不新增 Workflow Version，只注册新 Recipe version。发布返回值解析为真实 DB Recipe ID。 |
| Preset / preferred preset scope | 继续复用既有 Preset 系统；Preset 与 Preferred Preset 都按 `project + workflowVersionId + recipeId` 作用域读取/保存，旧 Recipe 的预设不会被新 Recipe 改写。 |
| Task Truth | 生产选择、Preset、Queue 与 GenerationService 都使用真实 `workflowVersionId + recipeId`；Task/Snapshot 继续保存 Recipe YAML、原始输入与 resolved inputs，不引入第二套 Task Truth。 |
| Ordinary UI | 主导航为批量图片、批量视频、资产库、任务、项目、工作流、设置；旧 Shot 入口隐藏，H3 本地导入仅展示 Project Folder。 |
| Asset deletion safety | 资产库删除前检查活动 Task/Production Queue 引用；数据库关系、项目内主文件和缩略图按事务边界清理，任务历史保留。 |
| Comfy memory release | 设置页仅在 AI Studio 与 ComfyUI 队列空闲时调用官方 `POST /free`；只释放模型内存，不删除模型文件。 |
| Migration / backup safety | Fresh DB、001–014、现有 012→013→014 upgrade、FK integrity、Backup v7 remap、v1–v7 compatibility 和恶意输入回归覆盖。 |
| Benchmark gate | Benchmark metadata and candidate freeze are covered; queue creation stays on the existing ProductionBatch/ProductionQueueService/GenerationService chain, repeat queue is blocked after binding, and workflow deletion counts Benchmark references. |
| H3 validation gate | FL2VA/REF2VA mode contracts, compatibility gate, exact ordered ReferenceManifest, and structured `REFERENCE_MAPPING_INCOMPLETE` diagnostics are covered by automated regression. |
| Regression | Rust `415 passed / 0 failed`; frontend `46 files / 152 tests / 0 failed`; frontend build and diff check PASS. Full evidence: `docs/M3_POST_BENCHMARK_CODE_GATE_0.3.0.md`. |

## H3 Production Quality / FAST Package Audit

QUALITY 是默认/推荐档案，FAST 是保留的低成本预览档案。两者都进入同一既有 Production Queue、Task、Snapshot、Asset 链；没有新增执行器或队列。

| Profile / package | Workflow ID / version | Audited graph contract |
| --- | --- | --- |
| QUALITY FL2VA T2V | `wfl_minimax_h3_fl2va_t2v_quality` / `2.0.0` | `MiniMaxH3ImageToVideo`；FL2VA INT8 ConvRot、20 steps、`res_multistep`/`simple`、SageAttention、HyperStep Middle-36、无 Turbo LoRA |
| QUALITY FL2VA I2V | `wfl_minimax_h3_fl2va_i2v_quality` / `2.0.0` | `MiniMaxH3ImageToVideo`；FL2VA INT8 ConvRot、20 steps、`res_multistep`/`simple`、SageAttention、optional `first_frame`、无 HyperStep/ Turbo LoRA |
| QUALITY First/Last | `wfl_minimax_h3_fl2va_first_last_quality` / `2.0.0` | FL2VA INT8 ConvRot、20 steps、optional `first_frame` + `last_frame`、无 HyperStep/ Turbo LoRA |
| QUALITY Omni REF2VA | `wfl_minimax_h3_reference_video_quality` / `2.0.0` | `MiniMaxH3ReferenceToVideo`；REF2VA INT8 ConvRot、20 steps、SageAttention、HyperStep Middle-36，保留 plural reference image/video/audio slots、无 Turbo LoRA |
| FAST legacy | `wfl_minimax_h3_fl2va` / `1.0.0`; `wfl_minimax_h3_reference_video` / `1.3.0` | 原有 4-step Turbo 图，保持不变 |

QUALITY 与 FAST 都使用 duration `1–15`、step `1`、default `5`；H3 输出预设严格采用 `608×352` 至 `1920×1088` 的 14 档 16:9 MP 梯度，QUALITY 默认 `960×544`。Project Folder 每个 Segment 按 UI Override → Front Matter → Prompt 正文显式规格 → 素材比例 → Recipe 默认值解析 duration/resolution；无显式规格且无可用素材推断时才使用 `5 秒 / 960×544`，非法 Prompt 分辨率会阻塞而不是静默回退。提交时按每个 Segment 的 mode + profile 选择具体 workflow/Recipe，并冻结到 ProductionBatchItem。

The compiler removes only the audited optional graph links when a mode does not supply
that media family. It never writes placeholder file paths to persisted Queue values.
All seven UI modes use the existing `ProductionBatch → ProductionBatchItem →
ProductionQueueService → GenerationService → Task → Snapshot → Asset` chain.

## Historical H3 1.2.0 Local Package Audit

| Item | Result |
| --- | --- |
| Version | `1.2.0` |
| Workflow ID | `wfl_minimax_h3_reference_video` |
| Workflow SHA-256 | `0385e8c53ae005444ae8d12d72145c3c24b681e6fb93f9ba896be9c675a5020a` |
| Recipe SHA-256 | `5d31c17bea33ca1659cf30434415324d4a1af3bee313eb794e6916081b8a3699` |
| Package files | `manifest.yaml` / `recipe.yaml` / `workflow_api.json` = PASS |
| Duration contract | `1–15` / step `1` / default `5` = PASS |
| Resolution contract | width `32–2048` / step `32` / default `1344`; height `32–2048` / step `32` / default `768` = PASS |
| Bindings | prompt, reference_image, width, height, duration_seconds duration/math chain, seed = PASS |
| Compile | 1s PASS · 5s PASS · 10s PASS · 15s PASS · 768-class PASS · 2K-class PASS · Custom PASS |
| GPU | NOT RUN · `DEFERRED BY PRODUCT OWNER` |

The new built-in package file hashes are recorded by source package contents:

| Package | workflow_api.json SHA-256 | recipe.yaml SHA-256 |
| --- | --- | --- |
| FAST FL2VA `1.0.0` | `30d022667959e6ff99031ca2f6b1dcf61be0827b185ca01d365231a073d265de` | `cf1d25d6a27f329b4e9bf4e4a9c74e42b0899629633b3aa4fc82bf92756fab55` |
| FAST Omni REF2VA `1.3.0` | `817d0296122c275694d3adc5541e8df1b41af470c0e4e9f4a12bdb9f3962539d` | `909cdbe2e17a6bbaae3361fb7f1063979b2c2cf5856d4f6b5e1e00e070544048` |

QUALITY package source hashes:

| Package | workflow_api.json SHA-256 | recipe.yaml SHA-256 |
| --- | --- | --- |
| QUALITY FL2VA T2V `2.0.0` | `bd5ca4f305f50ec4ced9c572ac8e304bffb4486568033f0c54ab7447a30d6423` | `49581dc2737b6bfa1222c900633bacc910217098035831bd21160306f0738494` |
| QUALITY FL2VA I2V `2.0.0` | `7435f948c402fb2eb8ace8351bb99598d9bd102c150057f535f766fc5bd6260e` | `51aaecbbe464c58f7a529adccab8a5bc88323db5dadefe3b476faf00bdaa4431` |
| QUALITY First/Last `2.0.0` | `96d98f74684cc3f76d028a182829a6f2a7ac3c0d37173a6182edc68b63ced992` | `761760bb34f7ffd30e448e28ebc5ea43ecaac110c6cbf5ebf932aaf5bc3636ab` |
| QUALITY Omni REF2VA `2.0.0` | `6d4ffb57059fb3b67b70118323aac4744c0d73158a68e076802b5703cc371ac3` | `00605f2f242c93e79af7df5abfe5af26b438614988e81a3da7d87d24d4058b6e` |

The historical `1.1.2` package and its validated workflow bytes remain
preserved and were not modified. The local package audit used no user absolute
paths in this document.

## MiniMax H3 input sources

| Input | Result |
| --- | --- |
| A. Asset Library | PASS · selected image/video/audio Assets → mode-specific frozen values → normal H3 ProductionBatch |
| B. Project Folder Local Import | PASS · native folder dialog → read-only Segment inspection → per-Segment Prompt duration/resolution extraction with source labels → source Image/Video/Audio Asset import → normal H3 ProductionBatch；ordinary UI only exposes this entry |
| FL2VA modes | PASS · Prompt-only / one-image / first+last-frame contracts |
| REF2VA modes | PASS · image-only / audio-only / image+audio / video+image contracts |
| Local pairing | PASS · recursive relative-path stem pairing, natural order, PNG/JPG/JPEG/WebP with TXT/MD, UTF-8/BOM, multiline prompt preservation |
| Legacy Local Import formats | BACKEND COMPATIBILITY PRESERVED · Prompt-only `.txt`, same-name pairing, first/last pairing, `h3-batch.json` and `h3-omni-batch.json`; hidden from ordinary UI |
| Project Folder Segment Import | PASS · natural folder order；text/I2V/First-Last/REF2VA auto detection；arbitrary single-image I2V；per-Segment Prompt/front matter/spec extraction/mode/media ordering/resolution/duration drafts；read-only inspection；normal Asset Library import；independent frozen queue values；TOCTOU and source-path boundary checks；one serial Production Queue |
| JSON manifest | PASS · `h3-batch.json`, relative image paths only, maximum 100 entries, duplicate/unknown/boundary paths blocked |
| Queue boundary | PASS · session keeps the absolute root only in Rust for 20 minutes; persisted queue values contain Asset IDs, not external paths |
| Auto start | PASS · default ON; OFF creates a READY normal ProductionBatch |

Local Batch Import does not create a second executor, queue, prompt table,
asset category, migration, or folder watcher. Existing cancel-pending, asset
delete guards, and ComfyUI memory-release guards remain generic.

## Previous candidate artifacts (stale after post-gate source changes)

The hashes below belong to the previous local candidate build and are stale after
the post-benchmark test/doc source changes. This code gate does not require a new
installer build; no upload, tag, or GitHub Release was made.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `src-tauri/target/release/ai-studio.exe` | 31424512 | `6dda820faa5ca6b00c4871e8105db643f9a61f123bcfb58128fbeef763512757` |
| `src-tauri/target/release/bundle/nsis/AI Studio_0.3.0_x64-setup.exe` | 7422990 | `2c9cccbef05c07fef015b6111baebff6e9fa09f4a6821b20589617309e012682` |
| `src-tauri/target/release/bundle/msi/AI Studio_0.3.0_x64_en-US.msi` | 10870784 | `3277658787e4fe926eb2b095dee4f0ad824c7ae990a6b66bc79d0b1d837984a0` |

The complete previous-candidate list is marked stale in
`docs/RELEASE_SHA256_0.3.0.txt`.

## Deferred live validation — batch images

后续产品负责人批准真实验证后，在一个真实项目中：

1. 打开“批量图片”，输入 5 条 Krea2 提示词，按空行拆分成 5 张提示词卡片。
2. 确认批次为 5 项、严格串行、创建 5 个 Task，并为每项产生 Snapshot 和图片 Asset。
3. 在队列详情中核对提示词和参数被冻结；重启应用后核对队列、任务和结果仍可恢复。
4. 让其中一项失败，确认 `continueOnFailure` 保留失败证据并继续后续项。

## Deferred live validation — batch videos

1. 在“资产库”选择 3 张图片，或在“批量视频”切换到“从本地导入”并选择一个最小任务目录。其中至少 1 张必须是手动导入的图片，以证明视频入口不依赖图片批次来源。
2. 为 3 张图片分别填写并保存视频提示词；确认资格状态、`最高 15 秒 · 最高 2K` 产品能力提示、QUALITY 默认 `20 步正式工作流`（或显式选择 FAST 的 `4 步 Turbo`）、单任务串行、历史验证档位提示、Recipe 时长下拉（1–15 秒，默认 5 秒）和精确 H3 runtime READY。
3. 创建 H3 批次，确认 3 项、严格串行、3 个 Task、3 个 Snapshot 和 3 个视频 Asset；视频可以用原生播放器播放。
4. 编辑一条提示词后重新创建或检查批次，确认队列项保留编辑后的冻结值；Krea2 批次不应被创建或自动依赖。

## Live validation — Workflow Parameter Exposure

该 Gate 只验证生产配置闭环，不改变 Workflow graph：

1. 在“工作流”选择一个已注册、可用于普通生产页的 Workflow Version，记录其 Workflow Version ID、Recipe ID、Recipe version 与 Workflow SHA-256。
2. 打开“生产参数”，只选择一个安全的未连接 literal input（优先 `steps`、`width`、`height` 或 `duration_seconds`），保存并发布新 Recipe。
3. 发布后确认 Workflow Version ID 未变化、Workflow SHA-256 与发布前完全一致、Workflow Version 总数未增加；Recipe version 增加且新 Recipe ID 与旧 Recipe ID 不同。
4. 打开对应普通生产页，确认 WorkflowSelector 能看到并选择新 Recipe；DynamicFormRenderer 显示新增公开参数，旧 Recipe 仍可单独选择。
5. 在新 Recipe 下创建一个 Preset，并将其设为 Preferred Preset；切换回旧 Recipe，确认旧 Recipe 的 Preset/Preferred Preset 不被改写。
6. 使用新 Recipe 创建一次真实 Task；在 Task / Snapshot / Queue 证据中核对冻结的是新 Recipe 的真实 DB `recipeId`，并确认 Snapshot 仍保存对应 Recipe YAML、raw inputs 与 resolved inputs。
7. Gate PASS 条件：`Original Workflow Immutable`、`Workflow SHA Unchanged`、`New Recipe Version`、`Existing Preset Reused`、`Task Truth` 六项全部有真实 UI/数据库/Task 证据；任一项缺证据则保持 PENDING，不以自动测试代替 Live Gate。

## Regression commands

```text
cargo fmt --all -- --check
cargo check
cargo test -- --test-threads=1
pnpm test
pnpm build
git diff --check
pnpm tauri build
```

上述 Live Gate 当前不执行，不计为失败；待产品负责人批准后再补充真实 Task、Asset、Playback 或重启证据。v0.3.0 tag 与 GitHub Release 均保持 ABSENT。完成本轮 Code Gate 后，状态保持 `CODE READY / LIVE VALIDATION DEFERRED`。
