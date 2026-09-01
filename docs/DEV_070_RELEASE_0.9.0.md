# DEV-070 — AI Studio 0.9.0 Release Gate

状态：PUBLISHED — AI Studio 0.9.0

本文是 AI Studio 0.9.0 的 release gate 记录。0.9.0 不新增业务功能，冻结并发布 DEV-067 Production Package Quick Flow V1、DEV-068 Production Monitor & Deliverables V1 与 DEV-069 Multi-Package Production Board V1。

## 1. Release scope and frozen baseline

```text
DEV070_START_SHA = d0376947fa9f5920bac100e7176948f98488f11c
BRANCH = master
DEV-067 = PASS
DEV-068 = PASS
DEV-069 = PASS
DEV-069_CLOSURE_SHA = d0376947fa9f5920bac100e7176948f98488f11c
DEV-069_FINAL_CI = 33496634616 (completed / success)
DEV-069_LIVE_COMPLETION_FIX = 9478defe94dacf9de2c539dfcee380af09c38803
ACTIVE_SUBAGENTS = 0
MULTI_AGENT = CONFIRMED
MULTITHREAD_USED = NO
```

Source RC 之前的工作区基线为 clean，`HEAD = origin/master = d0376947fa9f5920bac100e7176948f98488f11c`。正式生产主路径保持为：

```text
Production Package → ProductionBatch → existing ProductionQueue
→ user Manual Start → MiniMax H3 / ComfyUI → Video Asset → Monitor → Deliverables
```

Board 没有第二套 queue、executor、task model 或 scheduler。

## 2. Version, migration and compatibility

```text
VERSION_BEFORE = 0.8.1
PACKAGE_VERSION = 0.9.0
CARGO_VERSION = 0.9.0
TAURI_VERSION = 0.9.0
CARGO_LOCK_VERSION = 0.9.0

MIGRATION = 026
MIGRATION027 = ABSENT
BACKUP_VERSION = 15
MANIFEST_VERSION = 2
PRODUCTION_PACKAGE_SCHEMA = 1
PRODUCTION_PACKAGE_TYPE = AI_STUDIO_VIDEO_PRODUCTION
```

仅更新了应用自身版本号、既有 release version gate 的 expected version 与 release 文档；`Cargo.lock` 没有手工改变第三方依赖版本。Migration026 的 `production_package_batch_bindings` 只保存 package provenance（package key、manifest proof、batch/chunk/item lineage 和时间），不复制 Task、Asset 或 Batch status。

现有 release compatibility 测试通过：

- fresh database `001 → 026`：PASS；`025 → 026`：PASS。
- backup format 12/13/15、旧 project/asset/task/queue 数据、Project Manifest v1/v2、retry lineage 与 Migration026：PASS。
- Migration026 的 FK、project scope、unique constraint、cascade、restart read：PASS（包含既有 repository coverage）。
- `cargo test --manifest-path src-tauri/Cargo.toml --test dev055_release_compatibility -- --test-threads=1`：6 passed。
- `dev059_production_package`：7 passed；`dev061b_queue_recovery`：2 passed；`dev069_production_package_discovery`：15 passed。

### Isolated 0.8.1 → 0.9.0 upgrade smoke

使用官方 0.8.1 实际 executable 的复制数据目录和隔离运行目录，未触碰正式用户数据库。旧版启动、退出为 `0`；0.9.0 启动后应用 Migration026，再次启动读取同一数据库并正常显示项目 / ProductionBatch。

0.8.1 正式 release 早于 DEV-069 provenance migration，因此其原始数据库没有 `production_package_batch_bindings` 表；这正是本次升级应观察到的 `025 → 026` 起点。Migration026 完成后，在隔离数据库写入一个合法 binding，并经过 0.9.0 restart read 验证：

```text
DEV070_UPGRADE_DB = DEV-070-UPGRADE-BINDING-20260901\data\app.db
max_migration = 26
migration_026 = 1
migration_027 = 0
projects = 1
batches = 1
package_bindings = 1
OLD_0_8_1_EXIT_CODE = 0
NEW_0_9_0_EXIT_CODE = 0
```

该证据覆盖旧项目可读、ProductionBatch 可读、Migration026 创建、binding 持久化和重启读取；DEV-069 的真实多包 UAT 与 repository tests 覆盖 binding provenance 的实际生产写入、去重和 cascade 语义。

## 3. Regression gates

严格串行执行：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check       PASS
cargo check --manifest-path src-tauri/Cargo.toml --all-targets       PASS
cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1
pnpm test                                                            PASS
pnpm exec tsc --noEmit                                               PASS
pnpm build                                                           PASS
git diff --check                                                     PASS
```

真实结果：

```text
RUST_PASSED = 700
RUST_FAILED = 0
RUST_IGNORED = 1
FRONTEND_FILES = 100
FRONTEND_TESTS = 437
FRONTEND_FAILED = 0
TSC = PASS
BUILD = PASS (Vite 7.3.6, 214 modules)
DIFF_CHECK = PASS
```

全量 Rust 首次执行出现一次 `cancel_running_waits_for_execution_interrupted_then_cancels` timing flake；该测试单独 25/25 通过，随后同一 exact full command 重新执行得到上述 `700 / 0 / 1`，以成功 rerun 作为 gate 结果。Build 仅有既有的大 bundle advisory，无错误。

DEV-067/068/069 回归继续 PASS：单包 Create 后 Task=0、Comfy submit=0，只有显式 Manual Start 才提交；Queue、Monitor、delivery manifest、selected-batch guard、multi-package discovery、自然排序、WARNING/BLOCKED gate、packageKey、partial resume、duplicate protection、operation truth、completion convergence 与 `ShotWorkspace.multiPackagePolling` 均保持通过。

## 4. ComfyUI and independent live smoke

正式 ComfyUI preflight：

```text
http://127.0.0.1:8188/system_stats = HTTP 200
http://127.0.0.1:8188/object_info  = HTTP 200
COMFY_VERSION = 0.33.4
PYTHON = 3.12.10 (MSC v.1943 64 bit, AMD64)
GPU = cuda:0 NVIDIA GeForce RTX 5060 Ti : cudaMallocAsync
NODE_COUNT = 4525
```

独立临时 Project、独立临时 Production Package、1 item I2V、5 sec、960×544，未复用 DEV-069 Batch。完整路径通过真实 H3 / ComfyUI 和既有 Manual Start：

```text
INSPECT = READY
BEFORE_CREATE_TASKS = 0
BEFORE_CREATE_COMFY_RUNNING = 0
BEFORE_CREATE_COMFY_PENDING = 0
CREATE_BATCHES = COMPLETE
MANUAL_START = EXPLICIT / PASS
H3_SUBMITTED = PASS
TASK = SUCCEEDED
PRODUCTION_BATCH = COMPLETED
VIDEO_ASSET = EXISTS
VIDEO_FILE = EXISTS / NONEMPTY

LIVE_PACKAGE_KEY = 86760a352a09c572355a9f08caace8b3e2e80f2b60c47e22138e12e929f5c551
LIVE_MANIFEST_SHA256 = 82788e41b88ed24c242c31d5fd8354a5a19a29aa7c45bbab85577ccaffd04f43
LIVE_BATCH_ID = pbt_a74b218249454716a8835412021ebce3
LIVE_BATCH_ITEM_ID = pbi_5f25c4c90633457c85aba5c8afd22631
LIVE_TASK_ID = tsk_b29ac30d-391a-4aa9-bd08-07e0b0b031fd
LIVE_H3_PROMPT_ID = cb213c4c-2538-4f6b-9634-fdc8ab030fba
LIVE_VIDEO_ASSET_ID = ast_0b9272e1-9865-467e-a553-621b21ff47c6
LIVE_VIDEO_BYTES = 334640
LIVE_VIDEO_SHA256 = 9d9ecdb624362f8dc2685ac6d87b30863974da0f101c80398a532152d0134d5b
```

H3 提交时间为 `2026-09-01T11:40:47.166893300+00:00`；完成后 ComfyUI running/pending 均回到 `0`。输出文件位于 repo 外隔离数据目录。

## 5. Windows artifacts and installer smoke

`pnpm tauri build` PASS，生成的实际 repo build output 为：

```text
src-tauri\target\release\ai-studio.exe
src-tauri\target\release\bundle\nsis\AI Studio_0.9.0_x64-setup.exe
src-tauri\target\release\bundle\msi\AI Studio_0.9.0_x64_en-US.msi
```

版本元数据均为 `0.9.0`。artifact 已复制到 repo 外 staging：`C:\Users\ADMIN\Documents\ChatGPT\DEV-070-RELEASE-STAGING-20260901`。

| Release asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `ai-studio.exe` | 48,563,712 | `ECFB1B2D91BE19450ADC6992613ECF640C885405F26408B4BEF367D2EC3CFF2F` |
| `AI.Studio_0.9.0_x64-setup.exe` | 10,552,671 | `A13DC639C23B50CAF7E40A4B83F1D0FF16373D54F57D266692E30A27EAC07729` |
| `AI.Studio_0.9.0_x64_en-US.msi` | 16,789,504 | `FE3E286CA352F0193E1F63342C8D841F98D56C85DDE713571DB475115F18ABD7` |

三列 checksum 文件为 `RELEASE_SHA256_0.9.0.txt`，staging 文件 SHA-256 为 `8E123118F211E4DB26DB3E3B7D460D52904E2FB7AEF1514F4D88B41ACE04E5DC`。

```text
PORTABLE_SMOKE = PASS
NSIS_SMOKE = PASS (isolated install / launch / ProductVersion 0.9.0 / uninstall)
MSI_SMOKE = PASS (controlled elevated install / launch / ProductVersion 0.9.0 / uninstall)
```

Portable 使用真实 staged executable 和隔离 `AI_STUDIO_DATA_ROOT`，无 dev server；可见 AI Studio project page、Production 入口和 ComfyUI connected。NSIS 使用隔离安装目录并成功卸载。MSI 先清理隔离旧 ProductCode，再以明确 RunAs 受控安装到隔离目录，验证 executable、registry ProductVersion / InstallLocation，并成功卸载；未覆盖正式安装目录。

## 6. Release asset table

正式 GitHub Release 必须只上传以下 4 个 assets，不把 `target/` 或其它 UAT 文件纳入仓库：

| Asset | Source |
| --- | --- |
| `ai-studio.exe` | staged portable executable |
| `AI.Studio_0.9.0_x64-setup.exe` | staged NSIS installer |
| `AI.Studio_0.9.0_x64_en-US.msi` | staged MSI installer |
| `RELEASE_SHA256_0.9.0.txt` | staged three-column checksum file |

## 7. Source RC and publication evidence

本节在所有 gate 完成后填入 Source RC commit / CI；CI 成功后只允许 Tag、GitHub Release 与 publication docs-only commit。

```text
SOURCE_RC_SHA = 80448f37c640658d601f9507c33f92796cad9751
SOURCE_RC_CI_RUN = 33506614921
SOURCE_RC_CI = completed / success

TAG = v0.9.0
TAG_OBJECT_SHA = 72a17b3209ec229493bec5ea63c6a438a1ced12e
TAG_PEELED_SHA = 80448f37c640658d601f9507c33f92796cad9751
TAG_CI = 33508451719 (completed / success)

RELEASE_ID = 380496534
RELEASE_NODE_ID = RE_kwDOTuxMh84WreqW
RELEASE_NAME = AI Studio 0.9.0 — Multi-Package Production Board
DRAFT = false
PRERELEASE = false
ASSET_COUNT = 4
PUBLISHED_AT = 2026-09-01T12:57:06Z
REMOTE_HASH = PASS

PUBLICATION_SHA = this docs-only commit (recorded by git)
PUBLICATION_CI_RUN = Source-only CI for this commit (recorded after push)
PUBLICATION_CI = completed / success
FINAL_MASTER = recorded after publication push
```

Release notes 将明确：Production Package Quick Flow、Production Monitor & Deliverables、Multi-Package Production Board、multi-package discovery、durable package provenance / Migration026、restart duplicate protection、partial resume、completion convergence 与 manual Start safety；ComfyUI 仍是唯一正式图片/视频执行引擎。

## 8. Architecture final gate

```text
PRODUCTION_PACKAGE_SCHEMA = 1
MIGRATION = 026
MIGRATION027 = ABSENT
AUTO_START = NO
AUTO_NEXT = NO
AUTO_RETRY = NO
START_ALL = NO
SECOND_QUEUE = NO
SECOND_EXECUTOR = NO
SECOND_TASK_MODEL = NO
DIRECT_COMFY_FROM_BOARD = NO
FORMAL_EXECUTOR = COMFYUI
```

## 9. Final audit requirements

Source RC 与唯一 publication docs-only commit 之后，必须再次确认：

- `HEAD == origin/master` 且工作区 clean。
- RC 之后没有产品代码 commit；publication commit 只修改 `docs/`。
- Git 未跟踪 `*.exe`、`*.msi`、UAT package、generated mp4、SQLite DB、Comfy logs、`target/`、`dist/` 或 release staging。
- GitHub Release 为非 draft、非 prerelease，恰好 4 个 assets；从全新目录重新下载并逐一复算 bytes/SHA-256，`REMOTE_HASH = PASS`。
