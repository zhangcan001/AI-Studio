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

These values describe the final closeout against the `ef718f3e` baseline.

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
