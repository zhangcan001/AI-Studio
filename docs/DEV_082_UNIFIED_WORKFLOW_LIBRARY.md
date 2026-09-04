# DEV-082 — Unified Workflow Library

DEV-082 将工作流的导入、识别、用于当前项目、删除和恢复收口为一条普通用户流程。内部仍复用现有 WorkflowVersion、Recipe、Runtime Package、Catalog 与 Project Binding，但这些概念只出现在高级详情和技术诊断中。

```text
START_SHA=57dc1d01ab393b01fae8460d3d2bfa36d2b62b97
MULTI_AGENT_EXECUTION=YES
DEV083_STARTED=NO
```

## 产品契约

```text
NORMAL_USER_MODEL=IMPORT_USE_DELETE_RESTORE
WORKFLOW_IMPORT_BUTTON=导入工作流
WORKFLOW_DELETE_BUTTON=删除
WORKFLOW_RESTORE_BUTTON=恢复工作流

PRODUCT_WORKFLOW_DELETE=SUPPORTED
PRODUCT_DELETE_SUPPORTED=YES
PRODUCT_DELETE_ACTION=REMOVE_NOT_DESTRUCTIVE
PRODUCT_DELETE_IMPLEMENTATION=REMOVE_NOT_DESTRUCTIVE
PRODUCT_PACKAGE_PRESERVED=YES
PRODUCT_DELETE_PERSISTS_RESTART=YES

USER_UNREFERENCED_DELETE=HARD_DELETE
USER_NO_REFERENCE_DELETE=HARD_DELETE
USER_REFERENCED_DELETE=REMOVE
USER_HISTORY_DELETE=REMOVE
ACTIVE_TASK_DELETE=BLOCKED

DELETED_WORKFLOW_IN_NORMAL_LIST=NO
DELETED_WORKFLOW_IN_CATALOG=NO
DELETED_WORKFLOW_IN_RECOMMENDATION=NO
DELETED_WORKFLOW_IN_LEGACY_FALLBACK=NO

PROJECT_BINDING_COUNT_INSPECTION=YES
PROJECT_BINDINGS_CLEARED=YES
PROJECT_BINDINGS_CLEARED_ON_CONFIRMED_DELETE=YES

IMPORTABILITY_SEPARATE_FROM_CAPABILITY=YES
COMFY_OFFLINE_BLOCKS_IMPORT=NO
MISSING_NODES_BLOCKS_IMPORT=NO
IMPORTABLE_WITH_COMFY_OFFLINE=YES
IMPORTABLE_WITH_MISSING_NODES=YES

EXACT_RAW_IDENTITY=YES
EXACT_SEMANTIC_IDENTITY=YES
STRUCTURAL_VARIANT_DETECTION=YES
STRUCTURAL_VARIANT_AUTO_MERGE=NO

EXISTING_WORKFLOW_RERECOGNITION=YES
PRODUCT_WORKFLOW_RERECOGNITION=YES
AITUDOU_GENERIC_RERECOGNITION=PASS
AITUDOU_T2V_3ITEM=PASS

MIGRATION=027
BACKUP=16
NEW_DB_TABLE=NO
```

## 实现收口

- `workflow_recognition_service.rs` 独立负责 API/UI/非法/未知格式识别、raw/semantic/structural identity、用途与输入输出推断、Recipe freshness，以及运行能力摘要。结构指纹只用于提示，不会自动合并或覆盖。
- API JSON 在 ComfyUI 离线或缺节点时仍可保存到工作流库；状态分别反映为 `OFFLINE` / `MISSING_NODES`，而不是把“不可运行”误报为“不可导入”。UI JSON 只识别并明确要求导出 API Format，不做不安全的 UI→API 转换。
- 已存在的 raw/semantic 工作流不会重复创建；结构相似工作流进入人工选择，可添加为新工作流或现有工作流的新版本。现有工作流和系统自带工作流均可“重新识别”，更新只生成新的 RecipeVersion，旧 Recipe 与历史 Batch 不变。
- 系统自带工作流删除是 `REMOVE`：设置 `archived=true`、`enabled=false`，保留 Runtime Package、WorkflowVersion、Recipe 和历史数据；`ensure_installed` 只恢复缺失文件，不复活用户删除状态。用户工作流按活动任务/队列、历史引用和无引用情况选择 `BLOCKED`、`REMOVE` 或 `HARD_DELETE`。
- 删除检查返回 `deleteAction`、`projectBindingCount` 和绑定作用域。确认删除后仅清理对应 `workflow_version_id` 的 live Project Workflow Binding，不删除 Task、Batch、Shot 或历史引用。legacy fallback、Catalog、推荐和项目候选均排除已删除工作流。
- 前端工作区提供“全部 / 可用 / 需处理 / 已删除”筛选；普通行只保留用于当前项目、测试、删除或恢复等主要动作；删除使用 `WorkflowDeleteDialog`，显示来源、项目配置和历史影响，不使用 `window.confirm`。
- 生产架构未改变：仍使用 ComfyUI、单队列/单执行器/单任务模型，创建不自动启动，隐式自动下一批关闭，显式用户 armed next 保持，自动重试关闭，最大并发为 1，DEV-078 exact admission 保持。

## 测试夹具与安全边界

测试只使用脱敏 API graph 和仓库现有产品资源的元数据，不提交真实用户 JSON、真实 Prompt 或本地绝对路径。AITUDOU 仅作为通用识别回归夹具，不在生产代码中写死 workflow ID 或 node ID。

覆盖的识别矩阵：

```text
I1 API + Comfy ready       -> IMPORTABLE / EXECUTABLE
I2 API + Comfy offline     -> IMPORTABLE / NOT EXECUTABLE
I3 API + missing nodes    -> IMPORTABLE / NOT EXECUTABLE
I4 UI JSON                 -> RECOGNIZED_UI / NOT IMPORTABLE
I5 invalid JSON            -> INVALID_JSON
I6 unknown JSON            -> UNKNOWN

D1 same raw bytes          -> EXACT_RAW
D2 reordered JSON keys     -> EXACT_SEMANTIC
D3 prompt changed          -> STRUCTURAL_VARIANT
D4 megapixels changed      -> STRUCTURAL_VARIANT
D5 steps changed           -> STRUCTURAL_VARIANT
D6 node added              -> NEW or low structural similarity
```

AITUDOU 重新识别回归断言：同一 WorkflowVersion、旧 Recipe 不变、新 RecipeVersion 生成，并推断 `prompt`、`duration_seconds`、`width`、`height`、`seed`，以及可用时的 `steps=8`、`denoise=1`、`fps=24`。Production Package 回归保持 3 个 5 秒、960×544 的 T2V item，READY=3、BLOCKED=0、CREATED=3，创建不自动启动并通过 DEV-078 exact admission。

## 验证记录

最终提交前执行以下验证；数字以本次最终运行输出为准：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1
pnpm test
pnpm exec tsc --noEmit
pnpm build
pnpm tauri build
git diff --check
```

构建使用独立 `CARGO_TARGET_DIR` 和临时目录，不关闭或 kill 用户当前正在运行的 AI Studio。数据库迁移和备份保持 `027` / `16`，未新增 Migration 028 或新表。

```text
CARGO_FMT=PASS
CARGO_CHECK=PASS
CARGO_TARGETED=PASS
CARGO_FULL=PASS (771 library passed, 1 ignored; all integration binaries passed)
FRONTEND_TESTS=PASS (118 files / 559 tests)
TSC=PASS
FRONTEND_BUILD=PASS
TAURI_BUILD=PASS (exe + MSI + NSIS)
DIFF_CHECK=PASS
P0=NONE
P1=NONE
P2=NONE
P3=NONE
```

## Git 交付

```text
COMMIT=feat(workflows): unify import recognition and deletion
PUSH=origin/master
FORCE_PUSH=NO
DEV082_UNIFIED_WORKFLOW_LIBRARY=PASS
```

## DEV-082 FINAL LIFECYCLE CLOSEOUT

本收尾只验证已有统一工作流库的删除、恢复、项目绑定和重启语义，不新增工作流产品功能。测试使用临时 SQLite 数据库、临时运行包目录和脱敏内置夹具；不提交真实用户 JSON、Prompt 或绝对路径。

```text
FINAL_LIFECYCLE_CLOSEOUT=YES

RESTORE_RESULT_TYPE=WorkflowRestoreResult
RESTORE_READY_AUTO_ENABLE=YES
RESTORE_READY_READINESS=ACTIVE
RESTORE_BLOCKED_AUTO_ENABLE=NO
RESTORE_BLOCKED_READINESS=RESTORED_NEEDS_ATTENTION
RESTORE_OFFLINE_SUCCEEDS=YES
RESTORE_PROJECT_BINDING_AUTO_RESTORE=NO

DELETE_ORCHESTRATION_LAYER=WORKFLOW_LIFECYCLE_SERVICE
DELETE_PROJECT_BINDING_CLEANUP=APPLICATION_SERVICE
LIFECYCLE_SERVICE_CLEARS_BINDINGS=YES
DELETE_COMMAND_DOUBLE_CLEANUP=NO
REMOVE_BINDING_CLEANUP_COMPENSATION=YES
HARD_DELETE_LATE_BINDING=REMOVE_OR_REINSPECT_REQUIRED

PRODUCT_DELETE_RESTART_PERSISTENCE=PASS
PRODUCT_RESTORE_CATALOG_RETURN=PASS
PRODUCT_RESTORE_EXPLICIT_PROJECT_REBIND=PASS
TRUE_LIFECYCLE_E2E=PASS
TRUE_BINDING_CLEANUP_E2E=PASS
TRUE_RESTORE_READY_E2E=PASS

MIGRATION_BEFORE=027
MIGRATION_AFTER=027
BACKUP_BEFORE=16
BACKUP_AFTER=16
NEW_DB_TABLE=NO

FORMAL_EXECUTOR=COMFYUI
AUTO_START_ON_CREATE=NO
IMPLICIT_AUTO_NEXT=NO
EXPLICIT_USER_ARMED_NEXT=YES
AUTO_RETRY=NO
MAX_CONCURRENT_BATCH=1
SECOND_QUEUE=NO
SECOND_EXECUTOR=NO
SECOND_TASK_MODEL=NO
DEV078_IDENTITY_RULES=UNCHANGED
```

新增真实 application integration 测试：

- `dev082_product_delete_restart_restore_full_lifecycle_e2e`
- `dev082_restore_ready_reenables_workflow`
- `dev082_restore_missing_nodes_keeps_workflow_disabled`
- `dev082_restore_offline_succeeds_but_stays_disabled`
- `dev082_remove_clears_project_bindings_inside_lifecycle_service`
- `dev082_remove_binding_failure_rolls_back_archive_state`
- `dev082_hard_delete_late_binding_downgrades_to_remove`
- `dev082_deleted_product_is_unavailable_and_restored_product_can_be_explicitly_rebound`

当前 Agent-D 定向验证：

```text
DEV082_TARGETED_INTEGRATION=PASS (13 passed)
TRUE_SQLITE_APPLICATION_INTEGRATION=YES
REAL_REPOSITORIES=YES
SOURCE_STRING_ONLY=NO
```

主任务在所有并行修复整合后完成最终验证：

```text
FINAL_VALIDATION=PASS
CARGO_FMT=PASS
CARGO_CHECK=PASS
CARGO_TARGETED=PASS (DEV-082 13 integration + lifecycle unit tests)
CARGO_FULL=PASS (776 passed, 1 ignored)
FRONTEND_TESTS=PASS (118 files, 563 tests)
TSC=PASS
FRONTEND_BUILD=PASS
TAURI_BUILD=PASS (release exe + MSI + NSIS)
DIFF_CHECK=PASS
P0=NONE
P1=NONE
P2=NONE
P3=NONE
```
