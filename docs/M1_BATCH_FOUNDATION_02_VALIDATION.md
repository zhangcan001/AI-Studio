# M1 Batch Foundation 02 Validation

Date: 2026-08-07

Scope: task-list import and bounded retry policy layered on the existing Kera2 image + MiniMax H3 video batch foundation. No third model production support and no second execution engine are introduced.

## Task-list import

Status: IMPLEMENTED / regression validation pending.

The Studio batch panel accepts a local JSON task list. The file is parsed in React and converted into the same frozen batch items used by manual `Add current`; submission still uses `generation_create_batch` and the existing `GenerationService` pipeline.

Schema v1:

```json
{
  "schemaVersion": 1,
  "items": [
    {
      "workflowVersionId": "<current runtime workflow version id>",
      "recipeId": "<current runtime recipe id>",
      "values": {
        "prompt": { "type": "string", "value": "example" },
        "seed": { "type": "seed_random" }
      }
    }
  ]
}
```

Import gates:

- valid JSON only
- `schemaVersion = 1`
- 1..100 items per imported file
- every item requires `workflowVersionId`, `recipeId`, and typed `GenerationValues`
- recipe must exist in the current enabled generation catalog
- existing local batch + imported items may not exceed 100
- source media/project ownership is still validated by the existing backend generation/input-preparation boundary; the import parser does not bypass it

Typed values accepted by the importer match the current Studio contract: string, integer, random/fixed seed, single image/video/audio Asset, and ordered image/video/audio Asset lists.

CSV/Excel import is not included in this stage. JSON is the canonical automation-friendly v1 format.

## Bounded retry policy

Status: IMPLEMENTED / regression validation pending.

Retry is explicit and creates a new independent Task from the existing safe reusable draft. The failed original Task and its evidence are never modified or resubmitted in place.

Quick retry is allowed only when all of the following are true:

- original Task status is `FAILED`
- failure code is classified as transient
- runtime recipe is still available
- reusable saved inputs are available
- referenced media Assets are still present in the same project
- ComfyUI is currently connected
- the current detail view has not already created a retry Task

Transient quick-retry codes:

- `COMFY_OFFLINE`
- `COMFY_TIMEOUT`
- `COMFY_STREAM_DISCONNECTED`
- `COMFY_IMAGE_UPLOAD_FAILED`
- `COMFY_INPUT_UPLOAD_FAILED`
- `EXECUTION_INTERRUPTED`

The UI action is `Retry Once`. It creates exactly one new Task for that action and never starts an automatic retry loop.

## Explicit non-retry failures

`EXECUTION_ERROR` is never quick-retried. MiniMax H3 GPU out-of-memory currently lands in this class, so H3 OOM remains a visible failed Task requiring workflow/environment review rather than repeated GPU execution.

Other deterministic/unsafe-to-duplicate classes are also excluded from quick retry, including workflow validation/protocol failures, oversized inputs/outputs, snapshot failures, and result/history states where a duplicate generation may be unsafe.

Users may still inspect/load saved inputs for manual correction where the existing reusable-draft boundary permits it.

## Tests added

Frontend pure tests were added for:

- valid JSON task-list parsing
- unavailable recipe rejection
- malformed value rejection
- batch value deep snapshot independence
- failed-item retention after partial batch submission
- transient retry allowance
- MiniMax H3/`EXECUTION_ERROR` retry denial
- offline and missing-media retry denial

Rust batch size tests from Batch Foundation 01 remain present.

## Regression status

Full project regression commands remain BLOCKED by the current local MCP command channel before project execution: the Windows host reports an unavailable/misconfigured WSL runtime. A secondary MCP endpoint also returned HTTP 401. These are tooling/environment blockers and are not reported as application test failures.

Required before final PASS:

- `cargo fmt --all -- --check`
- `cargo check`
- `cargo test -- --test-threads=1`
- `pnpm test`
- `pnpm build`
- `git diff --check`

This stage must not be labeled PASS until those commands run successfully.

## Next stage

Recommended next stage after regression validation: `BATCH FOUNDATION 03 — persistent production queue + automation orchestration`. It should add durable batch/job grouping and controlled continuation without expanding beyond Kera2 and MiniMax H3.
