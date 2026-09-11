# DEV-094 Recipe History & Lineage Result

```text
TASK=DEV-094
BASELINE_SHA=00828159a4e95afc1f7aa74b592241af76e1837d
IMPLEMENTATION_SHA=fabab5eed020c3b296a2d9d308651c424c1700b4
REMOTE_CI_RUN=#118
REMOTE_CI_HEAD=fabab5eed020c3b296a2d9d308651c424c1700b4
REMOTE_CI_STATUS=GREEN
```

## Scope

Recipe History is a read-only, on-demand view of the existing exact
`workflowVersionId + recipeId` identity. It derives lifecycle, promotion,
archive, task, project, queue, template, preset, shot, experiment, and
benchmark references from persisted facts; no history table, migration, index,
or competing state source was added.

## Backend boundary

- `RecipeHistoryQueryService` is the application read authority.
- `RecipeHistoryQueryRepository` is a read-only exact-pair port.
- `SqliteRecipeHistoryQueryRepository` owns all SQL and uses bounded related
  queries plus keyset task pagination.
- Tasks remain the execution authority. Queue items without `task_id` are
  represented as planned-only references; generation evidence is reached via
  persisted task identity and snapshots.
- The typed command is
  `workflow_recipe_history_get(workflowVersionId, recipeId, taskCursor, taskLimit)`.

## Frontend boundary

- The existing Workflow Workspace recipe row opens `RecipeHistoryPane` on
  demand; there is no initial history fanout, new workspace, route, or store.
- Exact pair identity is retained for every request and task-page load-more.
- History display is read-only and preserves existing mutation ownership.
- `WorkflowWorkspace.tsx`: `1307 -> 1369` lines; the increase is the existing
  workspace composition and on-demand history surface, not a new lifecycle
  authority.

## Frozen invariants

```text
RECIPE_IDENTITY=workflowVersionId + recipeId
READ_ONLY_HISTORY=YES
HISTORY_TABLE=NO
HISTORY_MIGRATION=NO
HISTORY_INDEX=NO
INITIAL_HISTORY_FANOUT=0
RELATED_DETAIL_LIMIT=50
TASK_PAGE_DEFAULT=20
TASK_PAGE_MAX=100
QUEUE_ITEM_WITHOUT_TASK=PLANNED_ONLY
MUTATION_BEHAVIOR=UNCHANGED
NEW_STATE_SOURCE=NO
NEW_ZUSTAND=NO
RUST_CHANGE=YES
IPC_SURFACE_CHANGE=YES
DATABASE_SCHEMA_CHANGE=NO
CSS_CHANGE=NO
```

## Validation

Local gates passed:

- Frontend: `pnpm test` — 147 files, 790 tests passed.
- TypeScript: `pnpm exec tsc --noEmit` — PASS.
- Frontend build: `pnpm build` — PASS.
- Architecture guard — PASS, including all existing Shot/Asset/Workflow
  controller sentinels and `RECIPE_HISTORY_QUERY_AUTHORITY`,
  `RECIPE_HISTORY_READ_ONLY`, and `RECIPE_HISTORY_ON_DEMAND`.
- Rust format: `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` — PASS.
- Rust check: `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` — PASS.
- Rust tests: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1` — 807 passed, 1 ignored.
- Tauri bundle: `pnpm tauri build` — PASS; MSI and NSIS bundles produced.
- `git diff --check` — PASS.

Remote Source-only CI #118 passed both Frontend source checks and Rust source
checks for `IMPLEMENTATION_SHA`.

## Result

```text
RECIPE_HISTORY_QUERY=PASS
EXACT_PAIR_IDENTITY=PASS
READ_ONLY_BOUNDARY=PASS
ON_DEMAND_LOADING=PASS
BOUNDED_RELATED_DETAILS=PASS
KEYSET_TASK_PAGINATION=PASS
TASK_EXECUTION_AUTHORITY=PRESERVED
QUEUE_PLANNED_ONLY_SEMANTICS=PRESERVED
MUTATIONS=UNCHANGED
REMOTE_CI=GREEN
DEV_094=PASS
DEV_095=NOT_STARTED
```
