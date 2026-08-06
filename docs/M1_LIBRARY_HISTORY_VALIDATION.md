# M1 Task History + Asset Library Validation

Date: 2026-08-06
Commit: `623be3d`

## Scope

Validated M1-14 through M1-20 only:

- Studio / Assets / Tasks navigation
- Project-scoped task history, status filters, keyset pagination, and task detail
- Historical snapshot parsing and safe input reuse
- Project-scoped Asset Library, category filters, pagination, and binary preview
- Frontend DTO boundary without raw workflow, recipe, prompt, path, hash, or metadata leakage

## Automated checks

- `cargo fmt --all -- --check`
- `cargo check`
- `cargo test -- --test-threads=1`
- `pnpm test`
- `pnpm build`

## Live checks

- Existing M0 text-to-image output remains visible in Task History and Asset Library.
- Historical detail loads prompt, steps, and random seed into Studio without creating a new task.
- Offline mode shows ComfyUI Offline and disables Generate while Task History, task detail, Asset Library, and image preview remain available.
- ComfyUI was restored after the controlled offline check; endpoint `http://127.0.0.1:8188` and `/system_stats` responded with version `0.30.1` and one GPU device.

## Security boundary

The browse DTOs intentionally omit raw `workflow_json`, `recipe_yaml`,
`prompt_id`, queue/progress internals, storage paths, SHA-256 values, and asset
metadata. Historical input reuse accepts only values that match the current
workflow version and recipe definition; it never resubmits the historical task.

## Not included

Preset, multi-image, mask, video, MiniMax H3, Asset Delete, Task Delete,
Workflow Delete, and Project Manager remain outside this phase.
