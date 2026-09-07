# DEV-087A Application / Data Boundary：Read-Side Extraction

## 结论

DEV-087A 本地实现与门禁通过，已提交并推送到 `master`。

- 基线：`b7a2532cdf3e0f588ee65ef51d8d5902789423f`
- 实现提交：`9619971fb0f5de8aadd8db10ed6785f64a0eb2d5` — `refactor(application): extract read-side data boundaries`
- 远端 Source-only CI：通过，见 [run 34110494619](https://github.com/zhangcan001/AI-Studio/actions/runs/34110494619)；[Rust source checks](https://github.com/zhangcan001/AI-Studio/actions/runs/34110494619/job/101705333845) 与 [Frontend source checks](https://github.com/zhangcan001/AI-Studio/actions/runs/34110494619/job/101705334179) 均为 success。

## 交付范围

本次只抽取四个读侧边界：

1. `ProductionAuditService` → `ProductionAuditRepository::load_project_graph`
2. `ProjectCommandCenterService` → `ProjectCommandCenterRepository::load_project_command_center_data`
3. `ProjectManifestService` → `ProjectManifestRepository::load_manifest_snapshot`
4. `DiagnosticsService` → `DatabaseHealthProbe`

Application 层保留业务聚合、推荐、错误语义、manifest 组装与文件发布；Infrastructure 层持有 SQLx、`SqlitePool`、`FromRow` 及数据库事务，并完成高层 set-based 读取。生产组合根在 `src-tauri/src/lib.rs` 注入 SQLite 实现。

## 边界证据

以下数量是从基线源码计算的 SQLx 相关 marker occurrence，包含原文件内 `cfg(test)` fixture；当前数量只统计四个 Service 的生产源码段：

| Application Service | 基线 marker occurrences | 当前生产源码 |
| --- | ---: | ---: |
| ProductionAuditService | 61 | 0 |
| ProjectCommandCenterService | 47 | 0 |
| ProjectManifestService | 59 | 0 |
| DiagnosticsService | 5 | 0 |

另外，`src-tauri/src/application/ports/**/*.rs` 不包含 SQLx 类型或实现细节。`dev087_application_data_boundary.rs` 会递归扫描 Application 源码，阻止四个目标 Service 的生产源码回流 SQLx，并阻止未登记的 Application SQLx 文件。

## 保持不变的契约

- Production Audit、Command Center、Manifest 与 Diagnostics 的对外 Service 方法和错误语义保持不变。
- Audit graph、Command Center、500-shot performance 路径继续使用 set-based 读取；未引入逐 shot / 逐 item 的查询回归。
- Manifest 仍输出 schema `ai-studio.project-manifest`、version `2`，并兼容 v1/v2；manifest 读取使用单个 Infrastructure transaction。
- DEV-086 backup boundary、所有写侧队列 / executor / task lifecycle、命令契约、前端、迁移文件均未修改。
- `workflow_benchmark_service.rs` 与 `production_orchestrator_service.rs` 保持在后续 DEV-087B 范围内。

## 本地验证

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`：通过
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`：通过
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1`：789 passed，1 ignored，0 failed
- DEV-087 architecture guard：2 passed
- DEV-054 Command Center / Audit：3 passed
- DEV-055 compatibility：6 passed
- DEV-055 performance：4 passed
- DEV-052 runtime integration：16 passed
- DEV-057 script/draft compatibility：5 passed
- Frontend `pnpm test`：122 files / 585 tests passed
- `pnpm exec tsc --noEmit`：通过
- `pnpm build`：通过；仅有既有 chunk size warning
- `git diff --check`：通过

## 模型路由记录

任务正文要求按 Luna xhigh → Luna max → Astra low 路由；当前 Codex 会话没有模型切换工具，因此未伪造该切换记录。本次按当前会话模型执行，并应用 `ponytail full` 的最小改动约束。

## 后续

DEV-087A 完成后，剩余 Application SQLx 例外继续由显式 allowlist 约束；写侧边界与 benchmark / orchestrator 的独立处理留给 DEV-087B，不在本次扩展范围内。
