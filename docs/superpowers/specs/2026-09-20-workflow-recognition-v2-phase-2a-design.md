# Workflow Recognition V2 — Phase 2A Design

## Status

- Baseline: `75793c27d96668ea72229299984c2ce08899c0db`
- Branch: `master`
- Scope: recognition evidence and deterministic scoring only
- Commit/push/release: explicitly out of scope for this phase

## Intent

Upgrade workflow recognition from primarily name-oriented guesses to a
deterministic, explainable, multi-evidence evaluator while preserving the
Phase 1 import-draft lifecycle.

The evaluator must work with no ComfyUI connection, use `/object_info` when a
reanalysis session has it, remain pure, and fail safe when evidence is weak or
conflicting. Recognition is an import-preview decision; it must not persist a
workflow or commit a recipe.

## Non-goals

- No Phase 2B workflow-format converter or new workflow format.
- No database, migration, identity-hash, recipe-schema, or runtime-package changes.
- No new executor, queue, task model, or production execution path.
- No change to runtime capability state semantics; only the source of parsed
  capability metadata is shared.
- No hardcoded Kera2/H3 names, node IDs, hashes, recipe IDs, or package rules.
- No UI redesign. Existing Smart/Advanced draft behavior and explicit commit
  remain unchanged.

## Current boundary

`WorkflowAnalysisService` is the pure graph analyzer. It currently recognizes
inputs and outputs from the workflow document and raw bytes only.
`WorkflowOnboardingService` owns the reanalysis lifecycle and currently fetches
`/object_info` for capability checks. Its `WorkflowInputView.kind` describes
the current JSON value (`string`, `number`, `link`, etc.); it is not a declared
Comfy schema type and must not be reused as one.

The existing `WorkflowGraph` traces, output-path checks, capability enrichment,
and recipe-independent package fixtures are the reusable foundations.

## Architecture

### 1. Pure schema context

Add a small pure schema module that parses an already fetched `/object_info`
JSON value into a `RecognitionSchemaContext`. It contains normalized metadata
for each class:

- declared input types and current-value constraints;
- enum/options, numeric min/max/step, and optional/required membership;
- declared output media/data types and `output_node` status;
- class-level metadata needed for scoring.

The parser performs no I/O, registry access, persistence, or recipe lookup.
It is the single parser authority for recognition and capability consumers.
The existing raw-JSON capability wrappers may remain at test or compatibility
boundaries, but the production reanalysis path passes the parsed context so it
does not parse or fetch the same data a second time.

### 2. One-fetch reanalysis flow

The reanalysis path becomes:

```text
get_object_info() once
  -> parse RecognitionSchemaContext once
  -> enrich capability/node views from the context
  -> evaluate capability from the context
  -> analyze_workflow_with_schema(..., Some(&context))
  -> store the schema-aware analysis in the current draft
  -> derive automatic mappings from that analysis
```

If `/object_info` is offline or times out, the context is absent. Capability
continues to report `ComfyOffline`, while the analyzer runs the static graph,
class, title, input-name, and literal evidence path. This makes
`OFFLINE_RECOGNITION_SUPPORTED=YES` explicit.

The existing `analyze_workflow` entry point remains the offline-compatible
entry point and delegates to the schema-aware implementation with no context.
The new entry point is additive and does not alter workflow identity hashing.

### 3. Evidence model

Each input/output candidate carries an ordered, deterministic list of:

```text
kind, reason, weight
```

The evidence vocabulary includes at least:

```text
EXACT_INPUT_NAME
GRAPH_DIRECT_SINK
GRAPH_OUTPUT_PATH
SCHEMA_TYPE_MATCH
CLASS_TYPE_HINT
NODE_TITLE_HINT
LITERAL_TYPE_MATCH
NUMERIC_RANGE_MATCH
MEDIA_TYPE_MATCH
OFF_OUTPUT_PATH
SCHEMA_TYPE_CONFLICT
UTILITY_NODE
```

The score is the sum of fixed named weights. Evidence ordering is stable so
the same workflow produces the same report. `source` remains the compact UI
summary, for example `GRAPH+SCHEMA+NAME`; detailed evidence is retained in the
analysis report and issue candidates so tests and advanced details can explain
the decision without requiring score rendering in the normal UI.

The report additions are additive and optional for frontend consumers:

- selected input and output entries expose their evidence;
- ambiguous candidates expose a concise reason and their evidence;
- existing fields, workflow JSON, identity fields, and mapping semantics stay
  compatible.

### 4. Input scoring

Candidate generation keeps the existing semantic vocabulary and supported
fields: prompt, negative_prompt, seed, width, height, duration_seconds, fps,
steps, cfg, denoise, first_frame, last_frame, reference_image(s),
reference_video(s), and reference_audio(s). The vocabulary is not expanded
with product-specific fields.

For each candidate, the evaluator combines:

1. graph role and direct-sink/output-path evidence;
2. declared schema type versus semantic field type;
3. primitive or media type compatibility;
4. class type and node title hints;
5. exact/alias input-name evidence;
6. literal kind, numeric range, and option evidence;
7. off-path and utility-node penalties.

Declared schema type and current JSON value kind are compared separately. A
name such as `image_strength` cannot win merely because its name contains
`image` when the schema declares `FLOAT` and graph/type evidence points to a
numeric control. Conversely, an unfamiliar custom class can be recognized
when schema, graph, and primitive/media evidence agree.

Manual mappings remain authoritative outside this evaluator:

```text
USER_CONFIRMED > AUTO_INFERENCE
```

### 5. Output scoring

Output candidates are scored from schema output types, `output_node`, graph
terminality, selected-output dependency paths, class type, title, media type,
downstream relevance, and utility penalties. Real final image/video outputs
are preferred over preview, cache, debug, and auxiliary-save branches.

If two final outputs have materially equal evidence, the evaluator returns all
real candidates and emits `AMBIGUOUS_OUTPUT`; stable ordering is used only for
display and never as an implicit winner. A low-confidence or conflicted result
is similarly left for review rather than silently selected.

### 6. Confidence and ambiguity

All thresholds and penalties live together as named scoring constants. A
confidence decision uses:

- top candidate score;
- second candidate score;
- score margin;
- explicit conflict penalties;
- required evidence for the candidate kind.

The evaluator may auto-select only when the score and margin thresholds pass
and no disqualifying conflict exists. Otherwise it emits the existing issue
codes with real candidate references. Deterministic node/input/output sorting
is a tie-break for report order only, not a confidence substitute.

## Lifecycle invariants

Phase 2A must preserve all Phase 1 invariants:

- one file picker for a new import session;
- reanalysis of the existing draft without a second picker;
- resume reuses the current draft;
- manual mapping takes precedence over inference;
- Smart Import and Advanced Editor operate on the same draft;
- explicit commit is the only persistence path;
- `autoOnboardWorkflow()` and `autoConfirmOnboarding()` are not restored as
  ordinary import entry points.

The schema-aware analysis produced during reanalysis replaces the draft's
stale static analysis for that session, so automatic mappings and recognition
reporting use the same evidence snapshot as capability.

## Verification design

### V1 baseline

Before changing scoring behavior, parameterized tests load the nine existing
runtime packages from `src-tauri/runtime_packages`. Their `workflow_api.json`
and `recipe.yaml` files remain the source fixtures; no duplicate JSON fixtures
are added. The benchmark records total expected core bindings, correct,
missing, wrong, and ambiguous counts, plus output correctness using the
current V1 analyzer.

Advanced recipe fields that are not generic or not inferable are classified as
optional advanced expectations rather than forced into the core benchmark.

### V2 acceptance

The same corpus is evaluated with V2 and must satisfy:

- wrong high-confidence input bindings: `0`;
- wrong high-confidence outputs: `0`;
- V2 correct input bindings >= V1 correct input bindings;
- V2 correct outputs >= V1 correct outputs;
- ambiguous and missing counts do not regress without a recorded reason.

Independent synthetic fixtures cover:

1. nonstandard prompt name with STRING schema and graph-path evidence;
2. misleading image-like name whose schema is FLOAT;
3. duplicate seed controls with only one on the selected output path;
4. preview SaveImage versus terminal final output;
5. equal real double outputs producing `AMBIGUOUS_OUTPUT`;
6. unknown custom class recognized from schema/graph/type evidence.

Schema fixtures are independent of recipe expectations. Tests assert evidence
kind/reason, score ordering, margins, ambiguity behavior, output ranking, and
`OBJECT_INFO_CALL_COUNT == 1` for a reanalysis session.

### Regression gates

Run the existing Kera2, H3 fast T2V, H3 first/last, H3 reference/multireference,
offline, capability, and Phase 1 draft-lifecycle tests. Then run:

```text
cd src-tauri
cargo fmt --check
cargo check
cargo test --all-targets --jobs 2

pnpm test -- --reporter=dot
pnpm exec tsc --noEmit
pnpm run build
git diff --check
```

Frontend lint remains `N/A_NOT_CONFIGURED`; no lint tooling is added.

Manual smoke uses trusted local Kera2 T2I, H3 T2V, and H3 reference/first-last
workflows and verifies that capability refresh, inference, draft stability,
and manual mapping precedence remain intact.

## Expected implementation files

- `src-tauri/src/application/workflow_recognition_schema.rs` — pure normalized
  `/object_info` parser and schema context.
- `src-tauri/src/application/mod.rs` — module export.
- `src-tauri/src/application/workflow_analysis_service.rs` — schema-aware
  entry point, evidence collection, scoring, confidence, and output ranking.
- `src-tauri/src/application/workflow_onboarding_service.rs` — one-fetch
  reanalysis flow and capability consumers using the shared context.
- `src-tauri/src/application/*workflow*` tests or focused Rust test modules —
  V1 benchmark, synthetic fixtures, call-count, and regressions.
- `src/types/workflowOnboarding.ts` — additive evidence view types only if the
  serialized report exposes the detail to the frontend.

No runtime package, migration, schema, release, or production execution
authority files are expected to change.
