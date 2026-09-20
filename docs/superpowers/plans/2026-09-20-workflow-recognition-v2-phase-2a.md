# Workflow Recognition V2 Phase 2A Implementation Plan

> **For the implementer:** Use the `superpowers:executing-plans` skill when
> this plan is approved for native execution. Do not commit, push, tag, build
> an installer, or create a release; those actions are explicitly excluded by
> the task.

## Baseline and constraints

- Required baseline: `75793c27d96668ea72229299984c2ce08899c0db`.
- `HEAD` and `origin/master` are currently equal to that SHA.
- The worktree contains the approved uncommitted design document under
  `docs/superpowers/specs/`; do not reset or delete it. This is the only
  pre-existing worktree change and must remain outside product source scope.
- V1 counts must be captured before changing `Candidate`, `Guess`,
  `infer_outputs`, confidence resolution, or schema-aware analysis.
- Recognition remains pure; onboarding owns I/O and passes one parsed schema
  snapshot into capability and analysis.
- Phase 1 import-draft behavior, manual mapping precedence, identity hashes,
  recipe persistence, runtime capability states, and production queue authority
  are frozen.

## Files and responsibilities

- `src-tauri/tests/dev102_workflow_recognition_v2.rs` — parameterized nine-
  package corpus benchmark and frozen V1/V2 metric assertions. It reads the
  existing package files; it does not contain duplicate workflow JSON.
- `src-tauri/src/application/workflow_recognition_schema.rs` — pure parser and
  normalized `/object_info` context consumed by capability and analysis.
- `src-tauri/src/application/mod.rs` — exports the schema module.
- `src-tauri/src/application/workflow_analysis_service.rs` — optional schema
  analysis entry point, evidence model, deterministic input/output scoring,
  confidence margins, and pure synthetic tests.
- `src-tauri/src/application/workflow_onboarding_service.rs` — shared one-fetch
  reanalysis flow and context-aware capability helpers; no broad capability
  subsystem rewrite.
- `src-tauri/src/application/workflow_onboarding_service.rs` tests and
  `src-tauri/tests/dev081_complex_workflow_onboarding.rs` — call-count,
  offline, Phase 1 lifecycle, and manual precedence regressions.
- `src/types/workflowOnboarding.ts` — additive evidence types only if the
  serialized report exposes evidence; no UI redesign.
- `docs/superpowers/specs/2026-09-20-workflow-recognition-v2-phase-2a-design.md`
  — approved design; do not alter scope during implementation.

## Task 1 — Freeze the V1 corpus benchmark before algorithm changes

### Step 1: Add the corpus test harness

Create `src-tauri/tests/dev102_workflow_recognition_v2.rs`. Enumerate and sort
the nine package directories from `CARGO_MANIFEST_DIR/runtime_packages`, then
read each package's existing `workflow_api.json` and `recipe.yaml`. Use the
existing `RecipeParser` and `WorkflowDocument::parse`; do not generate schema
fixtures from a recipe.

Use one explicit generic core vocabulary set:

```rust
const CORE_FIELDS: &[&str] = &[
    "prompt", "negative_prompt", "seed", "width", "height",
    "duration_seconds", "fps", "steps", "cfg", "denoise",
    "first_frame", "last_frame", "reference_image", "reference_images",
    "reference_video", "reference_videos", "reference_audio", "reference_audios",
];
```

Bindings whose recipe source is in `CORE_FIELDS` are core expectations. Any
future recipe source outside this set is reported as `OPTIONAL_ADVANCED` and
does not affect the core pass/fail counts. The current nine packages must be
asserted as the complete corpus and must not be silently reclassified.

For each core binding, compare the V1 analysis result by semantic key, target
node, target input, and item index. Categorize it as correct, missing, wrong,
or ambiguous by inspecting the selected inputs and matching issue candidates.
Compare each recipe output by output node and image/video type, with separate
wrong and ambiguous counts.

### Step 2: Run the benchmark against the unchanged analyzer

Run only the new benchmark before touching production recognition code:

```powershell
cd C:\Users\ADMIN\Documents\ChatGPT\AI Studio\src-tauri
cargo test --test dev102_workflow_recognition_v2 v1_corpus_benchmark -- --nocapture
```

Record the actual V1 totals in named constants or a checked-in benchmark
fixture in the test file. The test must print the exact report fields required
by the final acceptance report:

```text
V1_TOTAL_EXPECTED_CORE_BINDINGS
V1_CORRECT_BINDINGS
V1_MISSING_BINDINGS
V1_WRONG_BINDINGS
V1_AMBIGUOUS_BINDINGS
V1_EXPECTED_OUTPUTS
V1_CORRECT_OUTPUTS
V1_WRONG_OUTPUTS
V1_AMBIGUOUS_OUTPUTS
```

Do not compute V1 counts from the later V2 entry point. If the first run
reveals a benchmark classification error, document it in
`BENCHMARK_DEFINITION_CHANGE` and fix the benchmark before continuing.

## Task 2 — Define and test the pure schema context

### Step 1: Write failing parser tests

Add `workflow_recognition_schema.rs` with tests for an independent literal
`/object_info` fixture containing:

- `STRING`, `INT`, `FLOAT`, and `BOOLEAN` inputs;
- `IMAGE`, `MASK`, `LATENT`, `VIDEO`, and `AUDIO` declared types;
- enum options represented as the first member of the Comfy input tuple;
- numeric min/max/step constraints;
- `output_node` and declared output arrays;
- an unknown declared type that becomes `UNKNOWN` without panic.

Assert that enum options are not treated as a string literal list and that the
context preserves required/optional membership. Use the failing test to define
the public methods used by later tasks:

```rust
RecognitionSchemaContext::parse(&Value) -> RecognitionSchemaContext
context.node(class_type) -> Option<&RecognitionNodeSchema>
node.input(name) -> Option<&RecognitionInputSchema>
```

### Step 2: Implement the smallest parser

Implement normalized types and metadata in the new pure module. The parser
accepts malformed or unknown metadata conservatively, uses `UNKNOWN`, and
never performs I/O, recipe lookup, registry access, or adapter calls. Keep the
current JSON value kind out of this module; that value is supplied separately
by workflow analysis.

Run the focused parser test and `cargo fmt --check` before proceeding.

## Task 3 — Migrate capability consumers without a subsystem rewrite

### Step 1: Add context-aware capability helpers

Introduce internal helpers in `workflow_onboarding_service.rs`:

```rust
fn enrich_nodes_with_schema(
    nodes: &mut [WorkflowNodeView],
    schema: &RecognitionSchemaContext,
)

fn evaluate_capability_with_schema(
    workflow: &WorkflowDocument,
    nodes: &[WorkflowNodeView],
    schema: &RecognitionSchemaContext,
    dynamic_binding_targets: &BTreeSet<(String, String)>,
) -> CapabilityCheckView
```

Move only the existing input option/range/output-node/missing-class lookups to
these helpers. Preserve all current capability issue codes and states.

Keep compatibility helpers that receive raw `Value` for existing tests or
non-reanalysis callers; each wrapper parses once and delegates to the context
helper. Do not create a second parser or alter runtime capability semantics.

### Step 2: Run the existing capability tests

Run the focused onboarding/capability tests before adding recognition scoring.
Verify option, range, missing-node, and output-node behavior is unchanged.

## Task 4 — Add explainable evidence and the schema-aware analysis API

### Step 1: Write failing API and determinism tests

In `workflow_analysis_service.rs`, add tests that require:

```rust
WorkflowAnalysisService::analyze_workflow_with_schema(
    &workflow,
    raw_bytes,
    Some(&schema),
)
```

and assert that repeated analysis of the same workflow/schema returns equal
candidate order, scores, evidence order, confidence, and issues. Add checks
that a schema conflict is represented as evidence and blocks `High` confidence.

### Step 2: Add the additive report types

Define serializable `EvidenceKind` and `RecognitionEvidence` with `kind`,
`reason`, and `weight`. Extend selected input/output report entries and
ambiguous issue candidates only as additive fields; retain existing fields and
camelCase serialization. Update frontend view types if required by the
serialized report, but do not render score details or change the workflow JSON
format.

Upgrade internal `Candidate` to retain score, evidence, and
`has_schema_conflict`. Keep value, required, node/input, item index, and source
information needed by existing mapping logic.

Centralize every weight, penalty, score threshold, and margin in one named
constants block. Include the required evidence kinds:

```text
EXACT_INPUT_NAME
INPUT_NAME_ALIAS
GRAPH_DIRECT_SINK
GRAPH_OUTPUT_PATH
SCHEMA_TYPE_MATCH
SCHEMA_TYPE_CONFLICT
MEDIA_TYPE_MATCH
CLASS_TYPE_HINT
NODE_TITLE_HINT
LITERAL_TYPE_MATCH
NUMERIC_RANGE_MATCH
OFF_OUTPUT_PATH
UTILITY_NODE
```

Derive `source` from the stable evidence kinds, such as
`GRAPH+SCHEMA+NAME`, without dumping weights into normal UI output.

### Step 3: Preserve the old entry point

Make:

```rust
pub fn analyze_workflow(workflow, raw_bytes) -> WorkflowAnalysisReport
```

delegate to the schema-aware implementation with `None`. Keep raw, semantic,
and structural SHA calculations exactly unchanged.

## Task 5 — Implement input scoring test-first

### Step 1: Add independent synthetic fixtures and failing tests

Add pure analysis tests for:

1. an unknown custom class with a nonstandard prompt input whose STRING schema
   and main graph path identify `prompt`;
2. `image_strength` declared FLOAT with value `0.75`, which must not become an
   image binding and must expose `SCHEMA_TYPE_CONFLICT` for that candidate;
3. duplicate seed controls where only the main output-path seed can win;
4. an unknown class whose schema, graph, and media types identify a core field;
5. existing duration arithmetic, first/last frame, and plural reference paths.

The tests must use hand-written independent schema JSON, not recipe-derived
schema. Assert selected node/input/item index, confidence, evidence kinds, and
real ambiguity candidates where appropriate.

### Step 2: Refactor candidate generation minimally

Keep the existing semantic vocabulary, `literal_guess`, duration proof,
`trace_crosses_unrelated_media_input`, `media_family`, and indexed media logic.
Add schema/graph evidence to those candidates rather than replacing stable
duration or media behavior.

Score candidates using graph role, output-path membership, schema declared
type, primitive/media compatibility, class/title/name/literal hints, numeric
range, and off-path/utility penalties. A schema conflict prevents `High`
confidence in the first implementation.

Resolve by top score plus second score, margin, conflicts, and required
evidence. Stable sorting is display order only. If the margin is below the
ambiguity threshold, emit `AMBIGUOUS_INPUT` with actual node/input/field-type
candidates instead of selecting the first candidate.

Run the synthetic tests and all existing `workflow_analysis_service` tests.

## Task 6 — Implement output scoring test-first

### Step 1: Add failing output tests

Create independent synthetic output fixtures for:

- preview/auxiliary image branch versus terminal final output, where final wins
  through schema, terminality, media, and downstream evidence;
- two equal final outputs, where both real candidates are returned and
  `AMBIGUOUS_OUTPUT` is emitted;
- a workflow with no schema context, where static output recognition still
  works.

### Step 2: Replace only output ranking internals

Make `infer_outputs` accept the optional schema context and score output-node
metadata, declared media types, terminality, selected-output dependency paths,
class/title hints, downstream structure, and utility/preview penalties. Keep
preview as a penalty rather than an absolute class exclusion. Do not hardcode
package names or node IDs. Preserve deterministic candidate order and
existing output IDs/types.

Run the focused output tests and existing H3/Kera2 analysis regressions.

## Task 7 — Wire one-fetch reanalysis and offline fallback

### Step 1: Add a shared reanalysis helper

Refactor `run_current_inference` so the production reanalysis path performs:

```text
get_object_info exactly once
parse RecognitionSchemaContext exactly once
evaluate capability from context
enrich nodes from context
analyze_workflow_with_schema from same context
store schema-aware analysis on current draft
derive auto inference from that stored analysis
```

Offline and timeout results use `None` schema, keep `CapabilityState::ComfyOffline`,
and still store static analysis. Do not make `recognize_draft` fetch schema or
alter identity/package matching.

### Step 2: Add the call-count and lifecycle tests

Extend the existing onboarding harness with `AtomicUsize` assertion:

```text
workflow_reanalyze_import -> OBJECT_INFO_CALL_COUNT == 1
```

Assert capability, analysis, and inference use the same schema snapshot. Add
offline/timeout coverage proving `OFFLINE_RECOGNITION_SUPPORTED=YES` and that
the analysis report remains available.

Run the Phase 1 tests for no second picker, draft reuse, in-place input/output
resolution, Advanced/Smart same draft, and manual mapping precedence. Existing
user mappings must survive new inference results.

## Task 8 — Add V2 corpus metrics and acceptance gates

Extend `dev102_workflow_recognition_v2.rs` to run the same nine packages through
the schema-aware analyzer using independent generic schema fixtures or package
metadata supplied by the test harness, never schema generated from the recipe.
Report:

```text
V2_TOTAL_EXPECTED_CORE_BINDINGS
V2_CORRECT_BINDINGS
V2_MISSING_BINDINGS
V2_WRONG_BINDINGS
V2_AMBIGUOUS_BINDINGS
V2_WRONG_HIGH_CONFIDENCE_BINDINGS
V2_EXPECTED_OUTPUTS
V2_CORRECT_OUTPUTS
V2_WRONG_OUTPUTS
V2_AMBIGUOUS_OUTPUTS
V2_WRONG_HIGH_CONFIDENCE_OUTPUTS
```

Assert:

```text
V2_WRONG_HIGH_CONFIDENCE_BINDINGS == 0
V2_WRONG_HIGH_CONFIDENCE_OUTPUTS == 0
V2_CORRECT_BINDINGS >= V1_CORRECT_BINDINGS
V2_CORRECT_OUTPUTS >= V1_CORRECT_OUTPUTS
```

Keep the V1 constants frozen from Task 1. If the test schema cannot be
independently represented for a package, classify that case as unavailable
schema and test static fallback rather than deriving schema from its recipe.

## Task 9 — Frontend contract and regression verification

If backend evidence is serialized, add the matching optional evidence fields to
`src/types/workflowOnboarding.ts` and update only type-level consumers that
require them. Do not add score UI, alter Smart/Advanced behavior, or change
workflow import format.

Run the complete existing frontend suite, typecheck, and production build. The
frontend lint status remains:

```text
FRONTEND_LINT=N/A_NOT_CONFIGURED
```

## Task 10 — Full gates and manual smoke

Before reporting completion, inspect the complete diff and verify no unrelated
files or semantics changed. Run:

```powershell
cd C:\Users\ADMIN\Documents\ChatGPT\AI Studio\src-tauri
cargo fmt --check
cargo check
cargo test --all-targets --jobs 2

cd C:\Users\ADMIN\Documents\ChatGPT\AI Studio
pnpm test -- --reporter=dot
pnpm exec tsc --noEmit
pnpm run build
git diff --check
git status --short
git diff --stat
```

Record `RUST_TEST_JOBS=2` and report page-file/resource failures separately
from product failures. Perform trusted local manual smoke for Kera2 T2I, H3
fast T2V, and H3 first/last/reference recognition/import without real H3
generation. Confirm prompt, dimensions, duration, seed, media references,
mode, and output mapping are visible and correct.

The final report must use the exact requested Phase 2A template. It must state
the approved design, nine-package corpus, frozen V1 metrics, V2 metrics,
synthetic gates, Phase 1 regressions, full gate results, changed files, and:

```text
COMMIT=NONE
PUSH=NONE
RELEASE=NONE
PHASE2B=NOT_STARTED
```

`PHASE2A_STATUS=PASS` is allowed only when every required acceptance gate is
passing. Otherwise report `BLOCKED` or `FAILED` with the first failed gate and
stop without commit/push/release.

## Review focus

The final implementation review must specifically check:

1. V1 metrics were captured before candidate/scoring changes and were not
   recomputed through V2.
2. `/object_info` is fetched and parsed once in production reanalysis, with
   capability and analysis sharing the same context object.
3. Declared schema type is never confused with current JSON value kind.
4. Schema conflicts cannot become high-confidence false positives, especially
   misleading media-like names.
5. Equal final outputs remain ambiguous and off-path duplicate semantics do
   not win solely because of lexical order.
6. Phase 1 user mappings and same-draft lifecycle remain authoritative.
