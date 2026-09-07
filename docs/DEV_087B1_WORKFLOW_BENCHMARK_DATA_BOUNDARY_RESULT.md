# DEV-087B1 — Workflow Benchmark Data Boundary

## 结论

- BASELINE_HEAD=`8af78e0750f0f58b4726ec5c7fe39fefbf07bd12`
- IMPLEMENTATION_COMMIT=`31cfebb` — `refactor(benchmark): extract benchmark data boundary`
- FINAL_MASTER_HEAD=the final master commit containing this result document
- DEV_087B1_STATUS=PASS
- DEV_087_STATUS=OPEN

## Routing

- Main Agent: current primary Codex session
- Helper Agent: not dispatched; the requested read-only helper dispatch was rejected by the current Codex environment with invalid arguments
- Helper write access: NO
- Model routing: Luna/Astra switching was unavailable in the current environment; no model switch was fabricated
- Ponytail: full

## Boundary

- WORKFLOW_BENCHMARK_APPLICATION_SQLX=0
- WORKFLOW_BENCHMARK_SERVICE_POOL=0
- WORKFLOW_BENCHMARK_PORT_SQLX=0
- BENCHMARK_SQLX_BEFORE=37
- BENCHMARK_SQLX_AFTER=0
- SQLITE_WORKFLOW_BENCHMARK_REPOSITORY=YES
- TRANSACTION_AUTHORITY=INFRASTRUCTURE
- APPLICATION_DATABASE_ACCESS=PORT_ONLY
- SQLx rows and `FromRow` exist only in infrastructure

## Changed files

- `src-tauri/src/application/ports/mod.rs`
- `src-tauri/src/application/ports/workflow_benchmark_repository.rs`
- `src-tauri/src/application/workflow_benchmark_service.rs`
- `src-tauri/src/infrastructure/database/mod.rs`
- `src-tauri/src/infrastructure/database/repositories/mod.rs`
- `src-tauri/src/infrastructure/database/repositories/workflow_benchmark.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tests/dev087_application_data_boundary.rs`
- `src-tauri/tests/dev087b1_workflow_benchmark_boundary.rs`

## Behavior and invariants

- Draft atomicity: PASS
- Queue creation failure transitions to `FAILED_TO_QUEUE`: PASS
- Queue-link compensation is durable: PASS
- Start-admission behavior remains unchanged; the existing production start-admission coverage and full Rust suite pass
- AUTO_RETRY=NO
- Telemetry refresh fallback behavior is unchanged
- Experiment listing remains bounded to 1..50 with `created_at DESC, id ASC`
- Winner, recommendation, quality, clone, and delete behavior is preserved
- BENCHMARK_SCHEMA=UNCHANGED
- PRODUCTION_ORCHESTRATOR=UNCHANGED
- NO_NEW_QUEUE/EXECUTOR/TASK_MODEL=YES
- DATABASE_MIGRATION=NO
- IPC_CHANGE=NO
- FRONTEND_CHANGE=NO

## Tests

- Rust format/check/full local gate: PASS
- Full local Rust suite: 793 passed, 0 failed, 1 ignored; all integration suites passed
- Application boundary guard: 3/3 passed
- Repository transaction tests: 4/4 passed
- Queue failure regression: 1/1 passed
- Frontend `pnpm test`: 122 files / 585 tests passed
- Frontend TypeScript check: PASS
- Frontend build: PASS
- Remote Source-only CI for implementation commit `31cfebb`: PASS after rerunning the unrelated cancellation-race failure
- Remote Frontend job: PASS
- Remote Rust job: PASS

## Scope audit

- No migrations, commands, frontend files, production orchestrator changes, benchmark schema changes, or new queue/executor/task models were introduced.
- The repository is clean after commit and push.
