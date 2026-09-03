# DEV-077 Project Production Readiness

项目开工就绪检查已完成。该功能是项目工作流配置与现有 ComfyUI Runtime 预检的只读派生组合，不改变生产执行路径。

## Verification markers

```text
DEV077_START_SHA=37455f556ab712aed49f3ce434a59b168da56122
DEV077_CODE_SHA=b1551b8e774784c3393d0bd543e8bd2d0c8f17a7
DEV077_FINAL_SHA=b1551b8e774784c3393d0bd543e8bd2d0c8f17a7
DEV077_CLOSEOUT_DOC_SHA=RECORDED_IN_GIT_HISTORY
BRANCH=master
WORKTREE_START=clean
ORIGIN_MASTER_START=37455f556ab712aed49f3ce434a59b168da56122

MIGRATION_BEFORE=027
MIGRATION_AFTER=027
BACKUP_BEFORE=16
BACKUP_AFTER=16

PROJECT_PRODUCTION_READINESS=DERIVED_RUNTIME_STATE
PROJECT_WORKFLOW_PREFLIGHT_REUSED=YES
COMFY_PREFLIGHT_REUSED=YES
NEW_COMFY_HEALTH_SYSTEM=NO
NEW_RUNTIME_HEALTH_SYSTEM=NO
NEW_BACKEND_SERVICE=NO
NEW_TAURI_COMMAND=NO
NEW_REPOSITORY=NO
NEW_DATABASE_TABLE=NO
RUNTIME_MATCH_KEY=WORKFLOW_VERSION_ID
GLOBAL_RUNTIME_UNRELATED_WORKFLOW_BLOCKS_PROJECT=NO
DEGRADED_WORKFLOW=WARNING_RUNNABLE
RUNTIME_BUSY=BUSY_NOT_BLOCKED
VRAM_THRESHOLD_ADDED=NO
AUTO_RUNTIME_PREFLIGHT_ON_PROJECT_OPEN=NO
AUTO_START=NO
AUTO_RETRY=NO
SECOND_QUEUE=NO
SECOND_EXECUTOR=NO
EXECUTION_ADMISSION_CHANGED=NO

PROJECT_READINESS_READY=SUPPORTED
PROJECT_READINESS_PARTIAL=SUPPORTED
PROJECT_READINESS_BUSY=SUPPORTED
PROJECT_READINESS_BLOCKED=SUPPORTED
PROJECT_RELEVANT_ISSUE_FILTER=SUPPORTED
PROJECT_WORKFLOW_CHANGE_INVALIDATES_RUNTIME_SNAPSHOT=YES
PROJECT_RUNTIME_CHECK_READ_ONLY=YES
```

## 1. Multi-Agent

```text
MAIN=PASS
AGENT-A=PASS — readiness model / existing preflight contract read-only audit
AGENT-B=PASS — project UI mount point / existing UI read-only audit
AGENT-C=PASS — frontend contract / Rust boundary / architecture read-only audit
FILE_CONFLICTS=NONE
RESOLUTION=MAIN integrated the reports, implemented the owned change set, and ran the complete regression suite.
```

各 Agent 均提交了非空审计报告，未执行 commit 或 push；MAIN 是唯一执行 Git 写入的智能体。

## 2. Existing Runtime Reuse

```text
Existing ComfyPreflightService reused=YES
Existing comfy_preflight_current reused=YES
Existing getComfyPreflight reused=YES
New runtime health service=NO
New backend command=NO
```

项目页只在用户点击检查按钮时调用一次 `getComfyPreflight()`。设置页既有全局 Runtime Preflight 保持不变。

## 3. Project Runtime Composition

新增纯函数 `composeProjectProductionReadiness(projectReport, runtimeReport)`，以 `ProjectWorkflowPreflight` 的 8 条路径和现有 `ComfyPreflightReport` 组合项目级结论。

```text
Workflow match key=workflowVersionId exact equality
Project Workflow Preflight reused=YES
READY=all runnable paths matched and runtime idle
PARTIAL=runnable paths exist, blocked paths exist, runtime idle
BUSY=runnable paths exist and runtimeBusy=true
BLOCKED=no runnable paths, or connection/node/runtime path blocker
Unrelated global workflow isolation=PASS
DEGRADED handling=WARNING and runnable
Runtime busy handling=BUSY, path readiness preserved
VRAM threshold=none; display only
```

Runtime `BLOCKED` / `DISABLED` 会阻断对应路径；未知的非阻断运行时状态按 `WARNING` 处理。全局 `runtimeReport.status` 不会直接覆盖项目结论，相关 issue 按当前项目使用的 WorkflowVersion / Workflow 过滤。

## 4. UI

```text
Project readiness panel=ProjectWorkspace: Settings → ProjectProductionReadiness → ProjectWorkflowPreflight
Initial unchecked state=shown; no automatic Comfy preflight
Manual check=one read-only getComfyPreflight call
Recheck=explicit button; prior report retained on failure
Relevant issues=global issues plus current project workflow issues only
Workflow-change snapshot invalidation=actual workflowVersionId fingerprint clears old report and requires another check
```

面板显示 READY、PARTIAL、BUSY、BLOCKED、8 条路径的 Workflow / WorkflowVersion / 来源 / Runtime / 原因，以及 GPU、VRAM、节点数、活动任务、生产队列和检查时间。没有加入新的启动阻断 Gate、自动等待、自动重试或自动修复。

## 5. UAT

UAT 使用真实 `ProjectWorkflowSettings` 与 `ProjectProductionReadiness` React 组件，仅 mock Tauri client boundary；没有创建 Batch、Task 或真实生成。

```text
Case 1=PASS — Image Default=A，A READY + CONNECTED + idle，图片路径 READY
Case 2=PASS — Video Default=A，A BLOCKED，路径 BLOCKED 并显示缺少节点 X
Case 3=PASS — runtimeBusy=true + activeTaskCount=1，整体 BUSY，路径仍显示空闲后可用
Case 4=PASS — 项目配置 A→B 保存后旧 runtime snapshot 失效，要求重新检查
Case 5=PASS — 项目使用 A 时，Z 的全局 BLOCKED 不影响项目 readiness
Case 6=PASS — DEGRADED + 尚未完成真实生成验证映射为 WARNING 且可运行
```

## 6. Safety

```text
Generation created=NO
Production batch created=NO
Production queue started=NO
Auto-start=NO
Auto-retry=NO
Execution admission changed=NO
```

没有修改 `createGeneration`、Production Batch、Production Queue、`startProductionQueue`、显式连续批次 armed-next、人工 Gate 或 Comfy 执行边界。

## 7. Architecture

```text
Migration before=027
Migration after=027
Backup before=16
Backup after=16
New DB table=NO
New Rust service=NO
New Tauri command=NO
Second queue=NO
Second executor=NO
Second task model=NO
Formal executor=COMFYUI
AUTO_START_ON_CREATE=NO
IMPLICIT_AUTO_NEXT=NO
EXPLICIT_USER_ARMED_NEXT=YES
AUTO_RETRY=NO
MAX_CONCURRENT_BATCH=1
START_ALL=NO
SEQUENCE_RESTART_RESUME=NO
```

## 8. Tests

```text
projectProductionReadiness=11 passed
ProjectProductionReadiness=10 passed
ProjectProductionReadinessUat=6 passed
DEV-076 regression=9 files / 70 tests passed
Settings/runtime regression=PASS; settingsUx, ProjectCommandCenter and Comfy preflight coverage included
Comfy preflight Rust tests=4 passed

cargo fmt=PASS
cargo check=PASS
cargo test=709 passed / 0 failed / 1 ignored; all integration targets passed

pnpm test=109 files / 507 tests passed
Frontend files=109
Frontend tests=507
tsc=PASS
build=PASS; 220 modules transformed
diff check=PASS
tauri build=PASS; ai-studio.exe, MSI and NSIS bundles produced
```

## 9. Git

代码提交：

```text
commit=b1551b8e774784c3393d0bd543e8bd2d0c8f17a7
message=feat(projects): add project production readiness
push=origin/master PASS
```

本文件由第二个提交 `docs: record DEV-077 verification` 记录。文档按要求不写入自身 SHA；真实 closeout 文档 SHA 以最终 Git 历史为准。

## 10. Issues

```text
P0=NONE
P1=NONE
P2=NONE
P3=NONE
```

## 11. Final

```text
DEV077_PROJECT_PRODUCTION_READINESS=PASS
```

DEV-077 完成后停止，不自动进入 DEV-078。
