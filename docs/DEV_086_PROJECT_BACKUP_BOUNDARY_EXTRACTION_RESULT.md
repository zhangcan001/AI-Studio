# DEV-086 — Project Backup Boundary Extraction Result

## 1. 交付状态

- 基线提交：`7958cc00f16d0f73d858e2740dfe487de440d048`
- 实现提交：`5a1fd49` (`refactor(backup): extract project backup database boundary`)
- 范围：仅调整 Project Backup 的应用层 / 端口 / 基础设施数据库仓储边界；未修改 migrations、commands、AppState、frontend 或 Cargo manifests。

## 2. 最终架构指标

| 指标 | 结果 |
| --- | --- |
| `PROJECT_BACKUP_APPLICATION_SQLX` | `0` |
| `PROJECT_BACKUP_SERVICE_SQLITE_POOL` | `0` |
| `PROJECT_BACKUP_SERVICE_TRANSACTION_TYPE` | `0` |
| `DATABASE_IMPLEMENTATION` | `INFRASTRUCTURE` |
| `APPLICATION_DATABASE_ACCESS` | `PORT_ONLY` |

统计口径为 `project_backup_service.rs` 的生产代码区（排除原有 `#[cfg(test)] mod tests`）；端口文件同样不包含 SQLx、`SqlitePool`、`Transaction` 或 `FromRow`。

基线生产代码对应计数为：`sqlx=134`、`SqlitePool=3`、`Transaction=236`、`FromRow=58`。本次提取后四项均为 `0`。

## 3. 边界落点

新增 `ProjectBackupRepository` 端口，提供三个高层能力：

- `load_export_snapshot`
- `find_missing_workflows`
- `restore_atomic`

应用层继续负责备份编排、ZIP、文件系统资产读取 / 写入、校验、ID remapping 与 `AppError` 映射；基础设施仓储负责 SQL select / insert / update、数据库行映射、事务与提交 / 回滚。恢复的数据库写入仍由一次 `restore_atomic` 事务统一控制，保持原子性。

组合根显式构造 `SqliteProjectBackupRepository` 并注入 `ProjectBackupService`。现有以 `SqlitePool` 构造服务的兼容调用通过基础设施适配，不把 SQLx 重新带回应用生产代码。

## 4. 兼容性与不变量

- 保留 backup v17。
- 保留版本字段、序列化结构、entry names 与现有 restore / export / inspect 行为。
- 保留恢复前校验、ID remapping、文件安全检查与失败清理行为。
- 保留缺失 workflow 检测与备份资产哈希 / 文件流处理。
- 覆盖原有 v1–v15 fixture、v17 round-trip、ZIP slip、快照 remap rollback 与 256 MB streaming 行为测试。

## 5. 验证结果

- DEV-086 architecture boundary test：通过。
- Rust focused backup tests：`32 passed, 0 failed`。
- Rust full `--all-targets`：主库 `789 passed, 0 failed, 1 ignored`；其余集成测试目标全部通过。
- `cargo fmt --all -- --check`：通过。
- `git diff --check`：通过。
- Frontend tests：`119` 个测试文件、`577` 个测试通过。
- Frontend TypeScript check：通过。
- Frontend production build：通过；仅保留既有 Vite chunk size warning。
