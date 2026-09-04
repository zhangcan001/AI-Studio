# DEV-081 — Complex ComfyUI Workflow Graph-Aware Onboarding

## Scope

DEV-081 adds generic graph-aware onboarding for ComfyUI API workflows. It keeps
the published API workflow immutable, creates Recipe bindings against safe
production semantics, and reuses the existing Compiler and Production Package
chain.

## Privacy and baseline

```text
BASELINE_SHA=e9c8c3b2efc89f5a58c595d431a920f125e190f2
REAL_FIXTURE=local-fixtures/minmax-8步加速.json
REAL_FIXTURE_LOCAL_ONLY=YES
REAL_FIXTURE_COMMITTED=NO
```

The real desktop workflow is intentionally not copied into the repository.
Automated coverage uses the sanitized, topology-equivalent fixture at
`src-tauri/tests/fixtures/workflows/minimax_h3_8step_graph_api.json`.

## Implemented contract

```text
GRAPH_AWARE_LINKED_INPUTS=YES
GRAPH_OUTPUT_SCORING=YES
VHS_VIDEO_COMBINE_OUTPUT=SUPPORTED
UTILITY_OUTPUT_DEPRIORITIZED=YES
LINKED_PROMPT_INFERENCE=YES
LINKED_WIDTH_HEIGHT_OVERRIDE=YES
DERIVED_DURATION_SOURCE=YES
TUNING_PARAMETERS_OPTIONAL=YES
PRODUCTION_PACKAGE_COMPATIBLE_RECIPE=YES
ORIGINAL_WORKFLOW_MUTATED=NO
MIGRATION=027
BACKUP=16
```

The graph analyzer indexes both directions, traces scalar leaves, and limits
automatic parameter inference to the selected output's upstream closure.
Linked `prompt` values can bind to a unique text leaf; `width` and `height`
use an execution-time direct sink override; a proven duration × FPS expression
binds `duration_seconds` to its unique numeric source. Ambiguous paths remain
review issues instead of being guessed.

The output scorer selects media sinks such as `VHS_VideoCombine` and
`SaveVideo`, deprioritizes preview output, rejects cache/debug/utility nodes,
and reports equal best candidates as `AMBIGUOUS_OUTPUT`.

## Local real-workflow UAT

When the user has placed the real file in `local-fixtures/`, run:

```powershell
$env:AI_STUDIO_REAL_WORKFLOW_FIXTURE="$PWD\local-fixtures\minmax-8步加速.json"
cargo test dev081_real_workflow -- --ignored --nocapture
```

The test reports format, graph/output selection, each core binding, optional
defaults, mode, and capability separately without printing the real prompt.

## Final closeout truth markers

These values describe the final closeout against the `8b87b1f4` baseline.

```text
FINAL_CLOSEOUT=YES

WORKFLOW_NAME_PREFERS_FILENAME=YES
GENERIC_FILENAME_FALLBACK_TO_NODE_TITLE=YES

SANITIZED_AUTO_ONBOARD=PASS
SANITIZED_PUBLISHED_RECIPE_USED_BY_PRODUCTION_PACKAGE=PASS
PROJECT_VIDEO_DEFAULT_EXACT_PAIR=PASS
PRODUCTION_BATCH_EXACT_PAIR=PASS
DEV078_EXACT_ADMISSION=PASS
FAKE_COMFY_SUBMIT=PASS
DEV081_8STEP_WORKFLOW_REACHED_EXECUTOR=YES

REAL_FIXTURE_GRAPH_UAT=YES
REAL_COMFY_CAPABILITY_UAT=NOT_AUTOMATED
```

`REAL_FIXTURE_GRAPH_UAT=YES` covers local graph inference only. Its capability
result comes from fixture-generated `object_info`; it does not prove capability
against the user's real ComfyUI installation.

## DEV-081 P1 REAL-UAT DIAGNOSTIC CLOSEOUT — final MAIN result

This section records the sanitized test input and the final audit evidence for the
Production Package preflight, structured-error propagation, and existing-SHA
Recipe regeneration work. Agent-D did not modify production implementation
code and did not run Git commit or push.

### Sanitized Production Package fixture

The repository fixture is:

`src-tauri/tests/fixtures/production_packages/dev081_t2v_3_items/production-package.json`

It is schemaVersion 1, contains three T2V items, and uses only these prompts:

```text
DEV081 test prompt 01
DEV081 test prompt 02
DEV081 test prompt 03
```

Its defaults are `durationSeconds=5`, `width=960`, and `height=544`. It has no
media paths, workflow IDs, recipe IDs, user prompts, or local absolute paths.
The user's real desktop workflow remains local-only and was not copied into
the repository.

### MAIN validation checklist

MAIN validated the following items against the implementation and the full
test run:

```text
REAL_UAT_DIAGNOSTIC_CLOSEOUT=YES
PACKAGE_PROJECT_AWARE_INSPECTION=YES
PACKAGE_RECIPE_PREFLIGHT_BEFORE_CREATE=YES
INSPECT_CREATE_RECIPE_PARITY=YES
PACKAGE_ITEM_ALREADY_CREATED_VISIBLE_AT_INSPECT=YES
PACKAGE_PROJECT_WORKFLOW_CHANGED_REQUIRES_REINSPECT=YES
STRUCTURED_PACKAGE_ERROR=YES
PACKAGE_ERROR_CODE_PRESERVED=YES
GENERIC_INVALID_INPUT_FOR_PACKAGE_FAILURE=NO
EXISTING_SHA_CURRENT_RECIPE_CHECKED=YES
EXISTING_SHA_OUTDATED_RECIPE_DETECTED=YES
REGENERATE_RECIPE_VERSION=YES
OLD_RECIPE_IMMUTABLE=YES
WORKFLOW_VERSION_UNCHANGED_DURING_RECIPE_REGEN=YES
EXPLICIT_PROJECT_REBIND_REQUIRED=YES
DEV081_T2V_3ITEM_INSPECT=PASS
DEV081_T2V_3ITEM_CREATE=PASS
DEV081_8STEP_REACHED_FAKE_COMFY=YES
```

### MAIN validation result

The MAIN validation is complete. The final single-test evidence run reported:

```text
PACKAGE_MODE=FL2VA_TEXT_TO_VIDEO
PACKAGE_RESOLVED_WORKFLOW_VERSION=wfv_7830a675-26cb-40eb-b413-05ab7f3a653e
PACKAGE_RESOLVED_RECIPE=rcp_1ee51e43-3da1-4bec-892f-39c6ef04bab4
PACKAGE_RESOLUTION_SOURCE=VIDEO_DEFAULT
INSPECT_READY_COUNT=3
INSPECT_BLOCKED_COUNT=0
CREATED_COUNT=3
AUTO_START_ON_CREATE=NO
DEV078_EXACT_ADMISSION=PASS
FAKE_COMFY_SUBMIT_COUNT=3
EXEC_WIDTH=960
EXEC_HEIGHT=544
EXEC_DURATION=5
EXEC_STEPS=8
EXEC_DENOISE=1
EXEC_FPS=24
DEV081_8STEP_REACHED_FAKE_COMFY=YES
```

The dedicated DEV-081 run also proves same-SHA old-Recipe detection,
regeneration to `1.0.1`, unchanged WorkflowVersion identity, preserved old
Recipe, and explicit project rebinding. The repository-wide Rust and frontend
gates passed after the compatibility assertion was updated to the structured
`PACKAGE_ITEMS_ALREADY_CREATED` contract. Real ComfyUI capability polling
remains `NOT_AUTOMATED`; the final closure uses sanitized fixtures and the
deterministic fake Comfy adapter.
