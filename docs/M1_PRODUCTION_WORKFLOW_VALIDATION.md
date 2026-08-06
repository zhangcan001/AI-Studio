# M1 Production Workflow Pack Validation

Date: 2026-08-06

Commit: `feat: add production workflow pack` (release commit recorded in Git history)

Scope: PROD-WFL-01 through PROD-WFL-12. No Model Runtime Pack work is included.

## Runtime State

- Enable: PASS (backend command and persistence)
- Disable: PASS (backend command and persistence)
- Missing state means enabled: PASS
- Disabled hidden from Studio Catalog: PASS (backend `COALESCE` filtering)
- Disabled visible in Workflows: PASS (technical workspace lists all versions)
- Disabled affects running task: MUST BE NO; state changes do not touch tasks
- Disabled historical recovery: PASS by design; recovery continues from persisted snapshots
- Re-enable restores Catalog: PASS

Runtime state uses `005_workflow_runtime_state.sql`; previous migrations were not modified and no backfill is required.

## Backup

- Export: PASS (verified runtime package bytes only)
- Restore: PASS (safe in-memory archive validation, staging readback, capability check, atomic publish)
- Zip Slip protection: PASS
- Offline restore: PASS; package is restored disabled by default
- Absolute paths exposed: MUST BE NO
- Archive contents: `manifest.yaml`, `recipe.yaml`, `workflow_api.json` only
- Archive limits: 64 MiB compressed, 64 MiB uncompressed, maximum 4 entries
- Database, Task, Asset, Preset, model, custom-node, and enabled-state data are not exported

Native Save File / Open File dialog interaction was not run because the local WebView automation adapter could not provide stable file-picker input. The command boundary does not return the selected path to React.

## Recipe

- Duplicate Recipe: PASS (same workflow version and workflow SHA, new Recipe version and package)
- Same Workflow Version: YES
- New Recipe Version: YES, patch increment by default
- Old Recipe mutated: MUST BE NO
- Preset migration: MUST BE NO; a new recipe receives a new recipe ID
- Raw Recipe YAML remains read-only preview only

## Diff

- Workflow Version Diff: PASS (node add/remove, class changes, literal values, links)
- Recipe Diff: PASS (inputs, bindings, outputs, types and defaults summarized)
- Raw workflow JSON exposed: MUST BE NO
- Long values are truncated to 120 characters and path-like values are summarized safely
- Diff is read-only; no merge, apply, or migration operation exists

## Diagnostics

- Package valid / enabled / hashes / DB registration / capability / task evidence: PASS
- Hash mismatch: PASS (`WORKFLOW_RUNTIME_HASH_MISMATCH`, `RECIPE_RUNTIME_HASH_MISMATCH`)
- Missing runtime package: PASS (`RUNTIME_PACKAGE_MISSING`)
- DB registration missing: PASS (`DATABASE_REGISTRATION_MISSING`)
- Capability recheck: PASS (`READY`, `MISSING_NODES`, `INCOMPATIBLE_INPUT_VALUES`, `COMFY_OFFLINE`)
- Broken package visibility: PASS (`INVALID` package remains visible in Workflows)
- Stale staging: PASS (`STALE_STAGING` diagnosis plus safe cleanup by staging ID)
- Runtime package deletion: not implemented; Disable is the lifecycle-safe alternative

## Live

- Existing T2I: NOT RUN; ComfyUI connectivity was available, but native WebView generation submission could not be driven reliably by the local automation adapter.
- Enable/Disable UI: NOT RUN; backend and static UI paths are covered, native interaction remains pending.
- MiniMax H3 Onboarding: NOT RUN; no validated workflow was found in the permitted reference/library checks.
- MiniMax H3 Live: NOT RUN
- `pnpm tauri dev`: ATTEMPTED; the first launch hit an existing executable lock, and the retry left the desktop process running after the command timeout. Native interaction was not asserted.

## Tests

- Rust: 231 passed, 0 failed
- Frontend: 21 passed, 0 failed
- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test -- --test-threads=1`: PASS
- `pnpm test`: PASS
- `pnpm build`: PASS
- `pnpm tauri dev`: ATTEMPTED (native interaction pending)
- `git diff --check`: PASS

## Technical debt

- Native desktop acceptance still needs a stable manual or WebView automation pass.
- A real second workflow package is needed for production live dogfooding.
- Runtime diagnostics currently report preset count as optional because preset aggregation is not part of the lifecycle repository query.

## Next stage

Only `MODEL RUNTIME PACK` is recommended next, beginning with MiniMax H3. This change stops at the Production Workflow Pack.
