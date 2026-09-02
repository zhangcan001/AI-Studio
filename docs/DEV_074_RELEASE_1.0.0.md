# DEV-074 — AI Studio 1.0.0 Release Gate

状态：`SOURCE RC PREPARATION`

本记录是 AI Studio 1.0.0 的正式 release gate。DEV-074 不新增业务功能、不做 UI 优化、不引入 P2/P3 修复、不新增数据库 schema；只验证当前 master 是否满足 1.0.0 发布条件。若发现真实 P0/P1，立即停止 Release。

## 1. Scope and release decision

```text
TARGET_RELEASE = AI Studio 1.0.0
P0 = PENDING
P1 = PENDING
MUST_FIX_BEFORE_1_0 = PENDING
AI_STUDIO_1_0_0 = PENDING
```

核心链路：`External Agents → Production Package V1 → AI Studio → ProductionBatch → ProductionQueue → Explicit User Start → Sequential User-Armed Start → ComfyUI → MiniMax H3 → Video Asset → Monitor → Deliverables`。

## 2. Frozen baseline and readiness pre-gate

```text
DEV074_START_SHA = 9698389efaa4073dbb509713ab9d03f578596f75
BRANCH           = master
ORIGIN_MASTER    = 9698389efaa4073dbb509713ab9d03f578596f75
WORKTREE         = CLEAN at start
ACTIVE_SUBAGENTS = 0
```

DEV-073 readiness pre-gate：

```text
DEV073_READINESS = PASS — AI_STUDIO_1_0_READINESS
DEV073_P0 = NONE
DEV073_P1 = NONE
DEV073_MUST_FIX_BEFORE_1_0 = NONE
DEV073_SOURCE_CI = 33594854270 (completed / success)
```

既有前置验收：

| Deliverable | Result | Evidence |
| --- | --- | --- |
| DEV-067 Production Package Quick Flow | PASS | `docs/DEV_067_PRODUCTION_PACKAGE_QUICK_FLOW.md` |
| DEV-068 Production Monitor & Deliverables | PASS | `docs/DEV_068_PRODUCTION_MONITOR_DELIVERABLES.md` |
| DEV-069 Multi-Package Production Board | PASS | `docs/DEV_069_MULTI_PACKAGE_PRODUCTION_BOARD.md` |
| DEV-070 AI Studio 0.9.0 Release | PUBLISHED | `docs/DEV_070_RELEASE_0.9.0.md` |
| DEV-071 Real Project Production Pilot | PASS | `docs/DEV_071_REAL_PROJECT_PRODUCTION_PILOT.md` |
| DEV-072 Explicit Sequential Batch Start | PASS | `docs/DEV_072_EXPLICIT_SEQUENTIAL_BATCH_START.md` |
| DEV-073 Product Freeze & Readiness | PASS | `docs/DEV_073_AI_STUDIO_1_0_READINESS_REVIEW.md` |

## 3. Version, schema and architecture freeze

```text
VERSION_BEFORE       = 0.9.0
PACKAGE_VERSION      = 1.0.0
CARGO_VERSION        = 1.0.0
TAURI_VERSION        = 1.0.0
CARGO_LOCK_APP       = 1.0.0
THIRD_PARTY_UPDATES  = NONE

PRODUCTION_PACKAGE_SCHEMA = 1
PROJECT_MANIFEST_VERSION   = 2
BACKUP_VERSION             = 15
MIGRATION                  = 026
MIGRATION027               = ABSENT
SCHEMA_CHANGE              = NONE
```

版本变更只更新应用自身版本、`Cargo.lock` 的 `ai-studio` package version 和既有 version consistency test 的 expected value；没有增加第二套版本检查器。

冻结架构：

| Contract | Frozen value | Result |
| --- | --- | --- |
| `FORMAL_EXECUTOR` | `COMFYUI` | PASS |
| `AUTO_START_ON_CREATE` | `NO` | PASS |
| `IMPLICIT_AUTO_NEXT` | `NO` | PASS |
| `EXPLICIT_USER_ARMED_NEXT` | `YES` | PASS |
| `AUTO_RETRY` | `NO` | PASS |
| `MAX_CONCURRENT_BATCH` | `1` | PASS |
| `START_ALL` | `NO` | PASS |
| `SECOND_QUEUE` | `NO` | PASS |
| `SECOND_EXECUTOR` | `NO` | PASS |
| `SECOND_TASK_MODEL` | `NO` | PASS |
| `DIRECT_COMFY_FROM_BOARD` | `NO` | PASS |
| `SEQUENCE_RESTART_RESUME` | `NO` | PASS |

## 4. Data compatibility and provenance

```text
FRESH_001_TO_026 = PASS — dev055_migration_matrix_reaches_026
UPGRADE_025_TO_026 = PASS — dev055_migration_matrix_reaches_026
UPGRADE_0_9_0_TO_1_0_0 = PASS — official v0.9.0 NSIS + isolated data root
BACKUP_V15 = PASS — dev055 compatibility round-trip
PROJECT_MANIFEST_V2 = PASS — dev055 compatibility matrix
PRODUCTION_PACKAGE_V1 = PASS — dev059/dev061b/dev069 coverage plus independent live package
PROVENANCE_BINDINGS_ONLY = PASS — binding rows preserve package identity/sourceKind without copying runtime status
```

Compatibility must preserve readable Project, Shot, Asset, Task, ProductionBatch, ProductionBatchItem and `production_package_batch_bindings`. The binding table remains provenance only: it does not copy Batch/Task/Asset status. FK, unique key, project scope, item IDs and `sourceKind=PRODUCTION_PACKAGE` are required. Formal user database is never used for upgrade or destructive testing.

## 5. Regression gates

严格串行执行：

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

pnpm test -- ShotWorkspace.sequentialBatchStart                                  PASS (10 tests)
pnpm test -- ProductionQueueDrawer                                               PASS (7 tests)
pnpm test -- ShotWorkspace.multiPackagePolling                                   PASS (6 tests)
```

回归覆盖 DEV-067/068/069/072 的 create safety、explicit start、single admission、failure pause、manual continue、partial resume、package identity、selected-batch truth、monitor/deliverables、completion convergence、A→B→C order、unarmed D 和 final sequence `IDLE`。没有 `Start Selected/All`、season scheduler、persistent sequence resume、bundle splitting 或第二执行器。

## 6. ComfyUI preflight

```text
COMFY_ENDPOINT       = http://127.0.0.1:8188
SYSTEM_STATS_HTTP    = 200
OBJECT_INFO_HTTP    = 200
COMFY_VERSION        = 0.33.4
PYTHON_VERSION       = 3.12.10 (64-bit)
GPU                  = NVIDIA GeForce RTX 5060 Ti
VRAM_TOTAL           = 17074421760
NODE_COUNT           = 4526
COMFY_PREFLIGHT      = PASS
```

## 7. Independent AI Studio 1.0.0 live H3 smoke

本次必须使用全新的 repo 外 Project、全新的 Production Package 和全新的 Batch，不复用 DEV-071/072 数据。Package fixture：

```text
LIVE_ROOT       = C:\Users\ADMIN\Desktop\AI_Studio_DEV074_1_0_LIVE_20260902
LIVE_PACKAGE    = C:\Users\ADMIN\Desktop\AI_Studio_DEV074_1_0_LIVE_20260902\ProductionPackage_DEV074_1_0_Live_EP01
LIVE_DATA_ROOT  = C:\Users\ADMIN\Desktop\AI_Studio_DEV074_1_0_LIVE_20260902\AIStudioData
MANIFEST_SHA256 = EC17E7A87CE65772DA760796B992C8F04C4A5F30028A59538D331C8BA86FDF42
ITEMS           = 1
MODE            = I2V
DURATION        = 5 sec
RESOLUTION      = 960x544
```

```text
LIVE_PACKAGE_KEY       = 6ff3cc6b94d2a6340aa44b6a3e0ed5695b8973f9f83e8cb92ba593fc83b11b38
LIVE_MANIFEST_SHA256   = EC17E7A87CE65772DA760796B992C8F04C4A5F30028A59538D331C8BA86FDF42
LIVE_BATCH_ID          = pbt_8d60f48e00b7456ebed7a9de76aa7aeb
LIVE_BATCH_ITEM_ID     = pbi_cb3a00139cc34b02af4683f034bdc27e
LIVE_TASK_ID           = tsk_5ba9f9a3-2918-4e43-bac6-253249dfcb89
LIVE_VIDEO_ASSET_ID    = ast_243051e1-f077-4794-9a60-62a616bc42a8
LIVE_VIDEO_FILE        = C:\Users\ADMIN\Desktop\AI_Studio_DEV074_1_0_LIVE_20260902\AIStudioData\projects\prj_default\assets\generated\video\ast_243051e1-f077-4794-9a60-62a616bc42a8.mp4
LIVE_VIDEO_BYTES       = 651833
LIVE_VIDEO_SHA256      = 24CE0827B4E587F7F52C33818E4FA9C85057BE8D95BEFCF26E6C1E20986E7FE6
INSPECT_READY          = PASS — 1 READY item; I2V / 5 sec / 960x544
CREATE_RESULT          = PASS — 1 batch / 1 item; user confirmed Task=0 and Comfy submit=0 at creation
CREATE_TASK_DELTA      = 0 (before first explicit Batch Start)
CREATE_COMFY_DELTA     = 0 (before first explicit Batch Start)
EXPLICIT_START         = PASS — Batch Start was the only start action
TASK_SUCCEEDED         = PASS — ComfyUI history prompt `263442ba-b507-4d5a-82fc-e42ee9ea0659`, success, no execution error/interruption
BATCH_COMPLETED        = PASS — database status `COMPLETED`; UI status `已完成`
ITEM_SUCCEEDED          = PASS — database status `SUCCEEDED`
VIDEO_ASSET_EXISTS     = PASS — `ast_243051e1-f077-4794-9a60-62a616bc42a8`
VIDEO_FILE_NON_EMPTY   = PASS — 651833 bytes; duration 5167 ms; 960x544; `video/mp4`
LIVE_H3                = PASS — independent fresh 1-item real H3 I2V smoke

Live database and runtime evidence:

```text
LIVE_PROJECT_ID        = prj_default
LIVE_PACKAGE_ID        = DEV074-1.0-LIVE-EP01
LIVE_PACKAGE_NAME      = DEV074 1.0 Live EP01
LIVE_SOURCE_KIND       = PRODUCTION_PACKAGE
LIVE_BINDING_ROOT      = \\?\C:\Users\ADMIN\Desktop\AI_Studio_DEV074_1_0_LIVE_20260902\ProductionPackage_DEV074_1_0_Live_EP01
LIVE_BINDING_ITEM_IDS  = ["DEV074_LIVE_EP01_SH001"]
LIVE_BATCH_CREATED_UTC = 2026-09-02T07:35:56.852655400+00:00
LIVE_TASK_STARTED_UTC  = 2026-09-02T07:36:17.431405200+00:00
LIVE_TASK_FINISHED_UTC = 2026-09-02T07:38:36.723730200+00:00
LIVE_BATCH_UPDATED_UTC = 2026-09-02T07:38:36.890845100+00:00
LIVE_APP_VERSION       = 1.0.0
LIVE_COMFY_QUEUE       = running=0, pending=0 after completion
LIVE_COMFY_HISTORY     = HTTP 200; completed=true; status=success
LIVE_SOURCE_IMAGE      = 13794 bytes; SHA256 FB7CB39E5B9DE25D072EF2B89781DECBA94AB635125864728ACD9C0A1A9557C2
LIVE_MANIFEST_BYTES    = 1027; SHA256 EC17E7A87CE65772DA760796B992C8F04C4A5F30028A59538D331C8BA86FDF42
LIVE_DB_MIGRATION      = 026
```

The 1.0.0 UI showed the selected package, Production Queue item and Production Monitor as `已完成`, with the completed item’s playback and file-location actions available. The generated video’s database SHA-256 matches the bytes on disk.
```

## 8. Sequential evidence

本次不重新生成 DEV-072 A/B/C；引用既有真实顺序证据，同时确认 1.0.0 independent live smoke 使用相同唯一 Queue/ComfyUI 边界：

```text
SEQUENTIAL_IMPLEMENTATION_SHA = 34fe62bd62953be9c2d24cc0bd19ba8a158e4519
SEQUENTIAL_TERMINAL_FIX_SHA   = 3923e77923578da2c73b093ca35e9bb2f633782c
SEQUENTIAL_CLOSURE_SHA        = 727d0b73945834499a6bdb57838fb88d302bb1d1
SEQUENTIAL_UAT                 = PASS (DEV-072 evidence)
A_BEFORE_B                     = PASS
B_BEFORE_C                     = PASS
UNARMED_D_NO_AUTO_START        = PASS
FAILURE_PAUSE_NO_AUTO_NEXT     = PASS
FINAL_SEQUENCE_STATE            = IDLE
```

## 9. Windows artifacts and installer/upgrade smoke

```text
PORTABLE_BUILD = PASS
NSIS_BUILD     = PASS
MSI_BUILD      = PASS
PORTABLE_SMOKE = PASS — current 1.0.0 portable executable, fresh independent live data root
NSIS_SMOKE     = PASS — isolated install / launch / ProductVersion 1.0.0 / silent uninstall
MSI_SMOKE      = PASS — elevated isolated install / launch / ProductVersion 1.0.0 / silent uninstall
UPGRADE_SMOKE  = PASS — official v0.9.0 NSIS data fixture upgraded in place to 1.0.0
```

| Artifact | Bytes | SHA-256 | ProductVersion |
| --- | ---: | --- | --- |
| `ai-studio.exe` | 48,568,832 | `108231B266332C2E3D11B0E9404F23FA36516C6537648EF91C67D983B432BCB8` | `1.0.0` |
| `AI Studio_1.0.0_x64-setup.exe` | 10,533,658 | `5BD3663921987EB1F3E864AF61B23D31C77A0D04499DC4B44DB7E496EA66ABED` | `1.0.0` |
| `AI Studio_1.0.0_x64_en-US.msi` | 16,789,504 | `4EFC095C2187137C5B71391FB407CF1F60704BE15DFC6F4C5F6C964F7288F084` | `1.0.0` |

Staging and checksum file remain outside the repository until release publication:

```text
RELEASE_STAGING = PENDING
RELEASE_SHA256_1_0_0 = PENDING
```

Installer smoke must use isolated install directories and verify start, ProductVersion `1.0.0`, and uninstall. Upgrade smoke must use the official GitHub `v0.9.0` installer, an isolated 0.9-era data root, readable old Project/ProductionBatch/bindings, Migration026, and the installed 1.0.0 binary; formal user data is excluded.

Installer evidence:

```text
NSIS_SMOKE_ROOT       = C:\Users\ADMIN\Desktop\AI_Studio_DEV074_1_0_NSIS_SMOKE_20260902
NSIS_INSTALL_EXIT     = 0
NSIS_START             = PASS — process/window started from isolated install
NSIS_VERSION           = ProductVersion=1.0.0; FileVersion=1.0.0
NSIS_UNINSTALL_EXIT    = 0; executable and install directory absent afterward

MSI_SMOKE_ROOT        = C:\Users\ADMIN\Desktop\AI_Studio_DEV074_1_0_MSI_SMOKE_20260902
MSI_PRODUCT            = AI Studio; ProductVersion=1.0.0; x64; UpgradeCode={D254323C-FE50-56FC-BE8E-86830497F401}
MSI_INSTALL_EXIT       = 0 (elevated; target install directory verified)
MSI_START              = PASS — process/window started from isolated install
MSI_VERSION            = ProductVersion=1.0.0; FileVersion=1.0.0
MSI_UNINSTALL_EXIT     = 0; executable, install directory and product registration absent afterward

UPGRADE_ROOT           = C:\Users\ADMIN\Desktop\AI_Studio_DEV074_0_9_TO_1_0_UPGRADE_20260902
UPGRADE_OLD_VERSION     = 0.9.0 (official GitHub v0.9.0 NSIS)
UPGRADE_NEW_VERSION    = 1.0.0; installer exit=0; installed executable ProductVersion/FileVersion=1.0.0
UPGRADE_PROJECT         = prj_default / Default Project readable in both versions
UPGRADE_BATCH           = pbt_dev070_upgrade_binding / DEV-070 upgrade binding fixture / READY
UPGRADE_BINDING         = dev070-upgrade-package / sourceKind=PRODUCTION_PACKAGE / item pbi_dev070_upgrade_binding
UPGRADE_MIGRATION       = 026; no migration 027
UPGRADE_POST_DB         = Project, ProductionBatch, ProductionBatchItem and binding rows readable after 1.0.0 start
UPGRADE_CLEANUP         = PASS — isolated 1.0.0 executable/uninstaller removed after verification
```

## 10. Source RC, tag and GitHub Release

```text
SOURCE_RC_COMMIT       = release: prepare AI Studio 1.0.0
SOURCE_RC_SHA           = PENDING
SOURCE_RC_CI_RUN        = PENDING
SOURCE_RC_CI            = PENDING

TAG                     = v1.0.0
TAG_OBJECT              = PENDING
TAG_PEELED              = PENDING
TAG_CI                  = PENDING

RELEASE_NAME            = AI Studio 1.0.0 — AI Production Workbench
RELEASE_ID              = PENDING
RELEASE_NODE_ID         = PENDING
RELEASE_PUBLISHED_AT    = PENDING
RELEASE_DRAFT            = false
RELEASE_PRERELEASE      = false
RELEASE_ASSET_COUNT     = PENDING (required: 4)
REMOTE_HASH_VERIFY      = PENDING
```

Release assets are exactly: portable `ai-studio.exe`, NSIS installer, MSI installer and `RELEASE_SHA256_1.0.0.txt`. Release notes must state that ComfyUI remains the sole formal image/video execution engine.

## 11. Publication record

```text
PUBLICATION_STATUS = PENDING
PUBLICATION_SHA    = PENDING
PUBLICATION_CI_RUN = PENDING
PUBLICATION_CI     = PENDING
FINAL_WORKTREE     = PENDING
```

After Source RC CI succeeds, no product/source/version changes are allowed. Only the annotated tag, GitHub Release and documentation-only publication record may follow. The publication diff must contain `docs/*` only relative to `SOURCE_RC_SHA`; the tag must peel to `SOURCE_RC_SHA`, not the publication commit.

## 12. Issues and deferred work

```text
P0 = NONE (required)
P1 = NONE (required)
P2 = NON-BLOCKING
P3 = NON-BLOCKING
```

Deferred to 1.1: `Start Selected/All` convenience, persistent sequence restart resume, season scheduler/reporting and bundle code-splitting. These are outside the 1.0.0 release gate.

## 13. Final gate

```text
DEV074_START_SHA = 9698389efaa4073dbb509713ab9d03f578596f75
SOURCE_RC_SHA    = PENDING
PUBLICATION_SHA  = PENDING
TAG_PEELED       = PENDING (must equal SOURCE_RC_SHA)
ASSETS           = PENDING (must equal 4)
REMOTE_HASH      = PENDING
SOURCE_RC_CI     = PENDING
PUBLICATION_CI   = PENDING

FINAL = AI STUDIO 1.0.0 = PENDING
```
