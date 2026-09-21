# Workflow Recognition V2 Phase 2B Architecture Audit

## Status

This document is a read-only architecture design for importing ComfyUI UI-format workflow JSON into the existing AI Studio workflow-import lifecycle.

- Phase 2A.1 baseline: `0e3080ebaba8f2f787fbcbab44574d13c9da6a98`
- Phase 2A.1 finalization: complete; the exact-head Source-only CI run passed.
- Phase 2B implementation: not started.
- Audit mode: read-only.
- Product source changes in this audit: none.
- Release, tag, installer, and Phase 2B commit: none.
- Design revision: V2.
- Design status: ready for implementation plan after review of the frozen contract below.

The existing API-format import path, Phase 1 draft lifecycle, Recognition V2 schema evidence, capability checks, and runtime package contract remain the boundaries for this design.

## Problem

ComfyUI has two materially different workflow representations:

1. **API-format JSON** — a mapping from node IDs to objects with `class_type` and named `inputs`. This is the representation currently validated, recognized, published, and executed by AI Studio.
2. **UI-format JSON** — a saved editor document containing node layout, display metadata, socket metadata, top-level link tuples, and positional `widgets_values`. It is designed to restore an editor canvas, not to be executed directly.

The current importer correctly identifies UI-format input, but does not safely convert it. The missing conversion is not a JSON shape conversion: it is a schema-dependent reconstruction of named API inputs, graph links, node classes, widget serialization, and output semantics.

The design goal is therefore:

> Convert a UI workflow only when a compatible ComfyUI schema snapshot supplies enough evidence to produce a complete, validated API workflow. Otherwise preserve the original source in the current draft and fail closed with actionable diagnostics.

## Current behavior

### Format detection

The current Rust onboarding and recognition services classify input using the following shape rules:

- A root object with `nodes` and `links` arrays is detected as UI format.
- A non-empty root object whose keys are numeric or supported subgraph IDs, and whose values contain string `class_type` and object `inputs`, is detected as API format.
- Other JSON is classified as unknown; invalid JSON is classified separately.

The current recognition result for UI format is a recognized, non-importable result with an `EXPORT_API_FORMAT` suggested action and a `UI_FORMAT` issue. The current front-end path tells the user to export API Format JSON. It does not yet create a durable/pending UI conversion draft that can be resumed when ComfyUI schema data becomes available.

The formal API import path remains:

`file picker -> workflow_analyze_import -> API validation -> recognition/capability -> explicit commit`.

The Phase 1 reanalysis path remains picker-free:

`existing draft -> workflow_reanalyze_import -> one object_info snapshot -> analysis refresh`.

### Explicit audit answers

```text
CURRENT_UI_FORMAT_DETECTION=SHAPE_DETECT_ONLY
CURRENT_UI_IMPORT_BEHAVIOR=UI_RECOGNIZED_BUT_REJECTED_AS_NON_IMPORTABLE; USER IS ASKED TO EXPORT API FORMAT
UI_TO_API_WITHOUT_SCHEMA=UNSAFE
UI_TO_API_WITH_SCHEMA=FEASIBLE_WITH_FAIL_CLOSED_VALIDATION
LINK_INPUT_MAPPING=USE_UI_NODE_INPUT_LINKS_AND_TOP_LEVEL_LINK_TUPLES_TO_RECONSTRUCT_SOURCE_NODE_AND_OUTPUT_SLOT; CROSS-CHECK BEFORE EMITTING_API_INPUTS
WIDGET_INPUT_MAPPING=WIDGETS_VALUES_NAMED_FIRST_WHEN_VALID; OTHERWISE_VERIFIED_POSITIONAL_DESCRIPTOR; CROSS_VALIDATE_SERIALIZATION_DOMAINS_AND_FAIL_CLOSED_ON_CONFLICT
CUSTOM_NODE_SCHEMA_DEPENDENCY=REQUIRED_FOR_SAFE_CONVERSION
MISSING_NODE_BEHAVIOR=KEEP_ORIGINAL_UI_BYTES_IN_PENDING_DRAFT; STATE_MISSING_NODES_OR_WAITING_FOR_COMFY_UI; DO_NOT_GUESS_OR_PUBLISH
PENDING_UI_DRAFT_FEASIBILITY=FEASIBLE_WITH_DRAFT_SOURCE_EXTENSION_AND_EXPLICIT_UI_SOURCE_PENDING_STATE
NORMALIZATION_ARCHITECTURE=PURE_SCHEMA_BACKED_UI_TO_API_NORMALIZER_BEFORE_EXISTING_WORKFLOW_VALIDATOR_AND_RECOGNITION_PIPELINE
IDENTITY_STRATEGY=RAW_IDENTITY_FROM_ORIGINAL_SOURCE_BYTES; SEMANTIC_AND_STRUCTURAL_IDENTITY_ONLY_AFTER_NORMALIZED_API_READY
OFFLINE_UI_IMPORT_STRATEGY=STRATEGY_B_PENDING_DRAFT_THAT_PRESERVES_SOURCE_AND_RESUMES_WITHOUT_A_SECOND_FILE_PICKER
GOLDEN_UI_API_PAIR_TEST_STRATEGY=REAL_KERA2_H3_GOLDEN_PAIRS_PLUS_NEGATIVE_FIXTURES; ASSERT_NORMALIZED_GRAPH, DIAGNOSTICS, IDENTITY, AND_RECOGNITION
RECOMMENDED_PHASE2B_APPROACH=VERSIONED_SCHEMA_BACKED_NORMALIZATION_WITH_STRATEGY_B_PENDING_DRAFTS_AND_FAIL_CLOSED_VALIDATION
PHASE2B_DESIGN_STATUS=READY_FOR_IMPLEMENTATION_PLAN
SOURCE_CODE_CHANGED=NO
PHASE2B_AUDIT_MODE=READ_ONLY
NEW_ARCHITECTURE_BLOCKER=NONE
```

## Design Revision V2 — frozen compatibility and safety contract

This revision freezes the first compatibility boundary before an implementation
plan is written. It narrows the recommended design; it does not claim support
for every ComfyUI UI workflow.

### NormalizationCompatibilityContext

Every normalization attempt must carry an explicit compatibility context:

```text
NormalizationCompatibilityContext:
  workflow_format_version
  frontend_version
  schema_fingerprint
  normalizer_policy_version
```

The first implementation may convert only combinations proven by real golden
UI/API pairs and their matching schema snapshots. The supported combination is
the tuple of exact workflow format version, known frontend version, compatible
schema fingerprint, and normalizer policy version. A workflow format version
that is unknown, a missing or unknown frontend version, or an unverified
frontend/schema combination must not enter best-effort conversion.

Those cases enter the existing-domain equivalent of one of:

```text
NEEDS_REVIEW
WAITING_FOR_COMPATIBLE_SCHEMA
EXPORT_API_FORMAT
```

The concrete status names may reuse existing domain enums, but the fail-closed
meaning is mandatory. Compatibility evidence must be versioned and visible in
diagnostics and draft provenance.

### Widget mapping precedence

The mapping architecture is explicitly split between linked inputs and
widget-backed inputs:

```text
linked input
  -> UI socket/link reconstruction

widget-backed input
  -> widgets_values_named if present and valid
  -> otherwise verified positional widgets_values descriptor mapping
  -> otherwise fail closed
```

`widgets_values_named` is a more reliable persistence-level name/value
association, but it does not prove that a value belongs in the API prompt or
that it should be emitted as an API input. Named values must be cross-checked
against schema descriptors, UI input/widget metadata, and positional values
when available. A conflict between named and positional evidence is a
`NORMALIZATION_ERROR`; the normalizer must not choose one silently.

The design distinguishes two serialization domains:

```text
WORKFLOW_SERIALIZATION
  Values needed to restore the ComfyUI editor workflow.

API_PROMPT_SERIALIZATION
  Named values and links that belong in the executable API workflow.
```

The normalizer maps between these domains only through a verified descriptor.
Presence in workflow persistence is never, by itself, proof of API inclusion.

### Serializer safety policy

ComfyUI frontend behavior may use `widget.serialize`,
`widget.options.serialize`, or `widget.serializeValue(...)`. The Phase 2B Rust
normalizer does not execute frontend JavaScript. Therefore the first version
only converts widgets whose serialization is proven to be standard and
represented by the compatibility descriptor:

```text
STANDARD_SERIALIZATION_PROVEN
```

The following are blockers rather than opportunities for a positional guess:

```text
custom serializeValue
unknown custom frontend serializer
dynamic client-only transformation
unverified control widget semantics
```

The result is `NORMALIZATION_BLOCKED`, the original source remains in the
pending draft, and the diagnostic is scoped to the node and input. Raw or
positional widget values must never be copied directly into an API input when
the serializer is not proven.

### Client-only control widgets

Fields such as `control_after_generate` may be present in saved workflow
widgets without belonging to the API prompt. The invariant is:

```text
widgets_values_named contains key
!=
API workflow inputs must contain key
```

The serializer descriptor decides API inclusion. Seed/control companions,
hidden fields, and dynamic combo state must be consumed only when their
standard serialization and API inclusion are proven for the compatibility
context.

### Node mode and frontend graph policy

The following UI behavior is not ordinary node conversion:

```text
mode
virtual behavior
primitive nodes
reroute nodes
subgraphs
bypass
mute/never
frontend-only graph transforms
```

Phase 2B does not simulate the ComfyUI frontend graph rewriter. The first
version fails closed for `BYPASS`, virtual, primitive, subgraph, or other
non-standard nodes unless a golden fixture, compatible schema, and deterministic
conversion rule prove the exact semantics. A normal node converter must not
silently treat those nodes as executable API nodes, omit them, or rewrite their
edges.

### Pending draft state

The draft model must represent the absence of a normalized API document
explicitly:

```text
UI_SOURCE_PENDING
  original UI bytes exist
  normalized API document = None

NORMALIZED_API_READY
  original UI bytes exist
  normalized API document is present and validated
```

Creating a pending UI draft must not create a placeholder or fake
`WorkflowDocument` merely to satisfy an existing API. Every flow that requires
an API `WorkflowDocument` is gated behind `NORMALIZED_API_READY`. Reanalysis
keeps the same draft ID and does not reopen the file picker.

### Commit gate

Commit is permitted only when all of the following hold:

```text
normalized_api != None
normalization diagnostics have no blockers
WorkflowValidator PASS
WorkflowGraph PASS
Recognition PASS or all recognition review is explicitly resolved
Capability follows existing rules
```

The original UI source bytes may remain provenance in the draft, but they may
never be written as `workflow_api.json`. A UI source by itself is never an
importable or executable commit candidate.

### Provenance boundary and package contract

The pending draft may retain:

```text
original UI bytes
source raw SHA
source format
frontend version
schema fingerprint
normalizer policy version
normalization diagnostics
normalized API bytes, when available
```

The published runtime package contract remains unchanged in Phase 2B: it
continues to publish the existing API workflow representation. Phase 2B does
not add package metadata or a package schema. If an existing manifest extension
point is discovered during implementation audit, it must be proposed
separately; this design does not depend on it.

### Identity gate

The approved identity strategy remains:

```text
rawSha        = original imported source bytes
semanticSha   = normalized API WorkflowDocument
structuralSha = normalized API WorkflowDocument
```

Before `NORMALIZED_API_READY`, normalized semantic/structural identity is not
computed and is not used for duplicate resolution. If different compatibility
contexts or policies produce different normalized graphs, they must not be
silently merged. Provenance and the compatibility context must remain
available for comparison.

### Golden corpus and verification gates

Normalizer implementation is gated on collecting real, trusted paired exports
before implementation begins. The first corpus must include:

```text
Kera2 T2I
H3 FAST T2V
H3 reference/first-last or reference-video
```

Each pair must preserve the UI export, API export, matching `object_info`,
workflow format version, frontend version, and expected normalized API result.
`object_info` must come from the matching real environment; it must not be
generated from a recipe.

The core verification is:

```text
normalize(UI + schema) -> normalized API
```

The result must be compared with the real API export for canonical semantic
graph, exact named inputs, exact links, output slots/types, semantic hash,
structural hash, and Recognition bindings. JSON parseability alone is not a
golden acceptance criterion.

Negative fixtures are also a pre-implementation gate and must fail closed:

```text
unknown custom serializer
widgets_values count/order mismatch
named widget conflicts with positional value
control-only widget
missing custom node
BYPASS node
virtual/primitive node
schema fingerprint mismatch
frontend version unsupported
dangling/mismatched links
```

### Offline Strategy B

The approved offline behavior is unchanged:

```text
select UI once
-> preserve pending draft
-> Comfy offline
-> WAITING
-> start compatible Comfy
-> reanalyze same draft
-> normalize
```

No second file picker is permitted. API-format imports retain their current
offline behavior. A UI draft remains pending until a compatible context is
available and all normalization gates pass.
## UI JSON model

The repository's existing UI fixture establishes the main ComfyUI 0.4 shape:

- Root metadata such as `last_node_id`, `last_link_id`, `groups`, `config`, `extra`, and `version`.
- `nodes[]` entries with:
  - numeric `id`;
  - UI `type`;
  - layout and editor state such as `pos`, `size`, `flags`, `order`, and `mode`;
  - socket metadata in `inputs[]` and `outputs[]`;
  - `properties`;
  - positional `widgets_values[]`.
- `links[]` entries represented by tuples of the form:

```text
[link_id, origin_node_id, origin_slot, target_node_id, target_slot, type]
```

The tuple is graph evidence, not an API input value. The current API contract represents a linked input as a two-element value such as `[source_node_id, source_output_slot]`; the UI `link_id` and display type must not leak into the normalized API graph.

The target slot index alone is not an input name. The normalizer must use the target node's `inputs[]` entries, whose linked entries carry the input name, and must cross-check those entries against the top-level link tuple. UI order and canvas order are not execution order.

The repository currently contains inline UI fixtures rather than a checked-in paired UI/API corpus. Real paired exports are required before claiming broad compatibility.

## Conversion requirements

A production converter must perform all of the following before a UI workflow can become an importable API workflow:

1. Parse and preserve the original UI bytes and source format.
2. Resolve every executable node's API class identity from compatible schema evidence.
3. Reconstruct graph links from node socket links and top-level link tuples.
4. Map linked inputs to the existing API link representation.
5. Map widget values to named API inputs using serialization-aware descriptors, not positional guesses alone.
6. Preserve omission versus explicit defaults where the schema distinguishes them.
7. Resolve hidden/control inputs, seed controls, dynamic combo branches, optional inputs, and custom widget serializers.
8. Resolve output candidates from graph reachability plus declared node/output metadata.
9. Produce normalized API bytes and provenance diagnostics.
10. Run the existing `WorkflowDocument`, `WorkflowValidator`, `WorkflowGraph`, recognition, capability, and readiness gates.
11. Refuse to publish or execute if required evidence is missing or contradictory.

The conversion result must be deterministic for the same UI bytes, schema snapshot, and normalization policy version. A normalization diagnostic must identify the node, input, evidence source, and reason whenever conversion cannot be proven safe.

## Schema dependency

The existing `RecognitionSchemaContext` already parses useful `/object_info` evidence:

- class schemas;
- declared input types;
- required and optional groups;
- options, ranges, and steps;
- upload/media metadata;
- output types and output-node evidence.

That context is a strong dependency for Phase 2B, but a UI normalizer needs a richer adapter than the current recognition report exposes. The adapter must provide, at minimum:

- ordered widget-backed input descriptors;
- linked versus widget-backed input classification;
- hidden inputs;
- default and optional semantics;
- widget serialization flags;
- seed/control-after-generate behavior;
- dynamic combo branches;
- output slot/type metadata;
- custom node compatibility/version identity;
- a schema snapshot fingerprint.

The dependency is semantic, not merely transport-level. The converter must use the same compatible schema assumptions as the ComfyUI frontend that produced `widgets_values`. A class name, display title, node package name, or sentinel value is not sufficient evidence by itself.

Comfy-Org documents the UI save-format fields and frontend widget serialization separately; those sources should be treated as compatibility references rather than replaced by hardcoded AI Studio rules:

- [Comfy-Org workflow JSON 0.4 specification](https://github.com/Comfy-Org/docs/blob/main/specs/workflow_json_0.4.mdx)
- [ComfyUI frontend widget serialization](https://github.com/Comfy-Org/ComfyUI_frontend/blob/main/docs/WIDGET_SERIALIZATION.md)
- [ComfyUI discussion of positional widget values and object_info mapping](https://github.com/Comfy-Org/ComfyUI/issues/2275)
- [ComfyUI discussion of mapping widget values to input names](https://github.com/Comfy-Org/ComfyUI/issues/1681)

## Normalization architecture

### Recommended module boundary

Future implementation should introduce a pure application module such as:

`src-tauri/src/application/workflow_ui_normalizer.rs`.

Its public conceptual boundary is:

```text
UI JSON bytes
+ parsed RecognitionSchemaContext
+ schema fingerprint
+ normalizer policy version
-> NormalizedApiWorkflow
```

The result should contain:

- normalized API `WorkflowDocument`;
- normalized API bytes;
- source-to-target mapping provenance;
- diagnostics and warnings;
- selected output evidence;
- schema and policy fingerprints.

The module must not own persistence, file picking, draft registries, package publishing, queue execution, model loading, or UI state. Onboarding owns I/O and lifecycle; the normalizer only parses, maps, validates its conversion inputs, and returns a result.

### Pipeline

The intended pipeline is:

```text
selected UI bytes
-> detect UI format
-> create or resume the same pending draft
-> fetch one object_info snapshot for the operation
-> adapt schema into ordered serialization descriptors
-> normalize links, nodes, widgets, and outputs
-> validate WorkflowDocument / WorkflowValidator / WorkflowGraph
-> run Recognition V2 and capability/readiness checks
-> expose diagnostics or an importable draft
-> explicit commit only
```

The API import path should continue to bypass normalization and remain usable offline when its existing validation permits it.

### Link input mapping

For each UI node:

1. Read linked entries in the node's `inputs[]`, using the input name and its `link` ID.
2. Resolve that link ID against the root `links[]` tuple.
3. Verify the tuple's target node and target slot agree with the node input location.
4. Verify the tuple's origin node and origin slot exist and agree with the origin node's output metadata where available.
5. Emit the current API link form `[origin_node_id, origin_output_slot]`.
6. Reject duplicate, dangling, contradictory, or ambiguous links instead of selecting one silently.

Top-level links without a corresponding target socket, or target socket links without a consistent top-level tuple, are conversion errors unless the relevant schema explicitly documents a compatible edge case.

### Widget input mapping

`widgets_values[]` is positional and does not contain input names. A safe mapper must consume an ordered descriptor stream derived from the compatible schema and frontend serialization rules.

The descriptor stream must account for:

- linked inputs that do not consume a widget value;
- required and optional inputs;
- hidden inputs and defaults;
- STRING/multiline, INT, FLOAT, BOOLEAN, and COMBO/ENUM values;
- min/max/step and option validation;
- seed values and control-after-generate companion values;
- dynamic combo branches;
- custom widget serializers;
- media upload selectors and serialized upload references.

The mapper must not assume:

```text
widgets_values count == API inputs count
```

It must not use input names, node titles, array positions from one package, or special sentinel values as production-only shortcuts. When descriptor consumption cannot be reconciled with the actual values, the result is a structured failure requiring reanalysis or manual review.

### Output selection

UI `outputs[]`, output labels, and graph links are evidence, not a hardcoded output-node allowlist. Output selection should combine:

- schema-declared output slot types;
- output-node flags or equivalent schema evidence;
- terminal graph reachability;
- existing `WorkflowAnalysisService` output scoring;
- the normalized graph's media/output type.

If multiple outputs remain equally plausible, the draft must report ambiguity. It must not infer the answer from the last node, canvas order, or a class-name list such as `SaveImage`/`SaveVideo`.

## Draft lifecycle

### Recommended Strategy B

A UI import should use the existing Phase 1 single-file draft lifecycle:

```text
file selected once
-> UI detected
-> pending UI draft created with original bytes and source metadata
-> schema unavailable or conversion incomplete
   -> WAITING_FOR_COMFY_UI / MISSING_NODES / NEEDS_REVIEW
-> same draft reanalysis
-> schema-backed normalization
-> existing recognition/capability/readiness flow
-> explicit commit
```

The draft needs an additive source/lifecycle extension:

- original source bytes;
- source format;
- source filename and raw SHA;
- lifecycle state (`UI_SOURCE_PENDING` or `NORMALIZED_API_READY`);
- optional normalized API document/bytes, with the document explicitly `None` while pending;
- conversion status;
- schema fingerprint;
- normalizer policy version;
- diagnostics and provenance.

The draft ID must remain stable across reanalysis. Returning from Advanced Editor to Smart Import must use the same draft and must not open a second file picker. The UI may show a pending state rather than pretending the workflow is importable.

The current registry is in-memory and bounded, so this design does not require a persistence migration for the pending state. Final publication remains the existing API package contract. A pending UI draft must never be published by placing the original UI JSON into `workflow_api.json`.

### Manual mapping

Manual mapping remains higher precedence than auto-inference, but it cannot authorize unsafe graph conversion. It may resolve a recognized ambiguity only after the normalizer has established a schema-backed candidate set and all required graph invariants. It must not manufacture a missing class schema, invent an input serialization rule, or bypass validator/capability gates.

## Identity handling

Recommended identity semantics:

- `rawSha`: SHA-256 of the original imported bytes. An API export and a UI export intentionally have different raw identities.
- `semanticSha`: canonical identity of the normalized API `WorkflowDocument`.
- `structuralSha`: normalized API graph structure identity as currently defined by the analysis service.

This permits a UI export and an API export of the same workflow to converge on semantic/structural identity after normalization, while preserving provenance that the original source representations differed.

The implementation must record:

- source format;
- raw source SHA;
- normalized API SHA;
- schema fingerprint;
- normalizer policy version;
- transformation diagnostics.

Semantic convergence is allowed only when normalization completed without unresolved diagnostics. A schema-dependent transformation must not silently deduplicate two workflows merely because their names or display titles match. If schema versions produce different normalized graphs, identity must remain distinct and the difference must be visible in diagnostics.

Before `NORMALIZED_API_READY`, normalized semantic/structural identity is not
computed and is not used for duplicate resolution. The compatibility context
and normalizer policy are part of the provenance needed to explain any later
identity comparison.

## Capability interaction

Capability analysis must run after normalization for UI imports. It should consume the normalized API graph and the same schema snapshot used to convert it, so the recognition and readiness results describe the executable representation rather than the editor representation.

Expected behavior:

- API import: existing validation and offline behavior remain unchanged.
- UI import with complete compatible schema: normalize, then run existing Recognition V2 and capability checks.
- UI import with missing schema/custom node: pending draft with `MISSING_NODES` or `WAITING_FOR_COMFY_UI`; not READY and not publishable.
- UI import with schema but contradictory mapping: pending/needs-review with diagnostics; no partial ready state.
- UI import normalized successfully but with environment capability gaps: preserve the valid draft and report the existing capability state; do not rewrite the graph to hide missing runtime capability.

The normalized workflow must enter the same compiler/recipe validation boundaries as an API-imported workflow. Phase 2B must not add a second execution or queue path.

## Custom-node handling

Custom nodes are supported only through compatible live or fixture schema evidence.

Required policy:

- The node's schema must identify its API class and input/output contracts.
- `properties`, display titles, package labels, or node names may assist diagnostics but cannot replace schema identity.
- Unknown nodes on an output-relevant path block normalization.
- Unknown node schemas must never be bypassed by dropping the node, replacing it with a guessed class, or copying positional values into guessed inputs.
- A disconnected visual node may be retained in the original source and may be omitted from the normalized executable graph only under an explicit, tested policy with diagnostics. The initial production policy should fail closed when reachability or side effects are uncertain.
- Custom widget types without serialization descriptors block the affected node.
- Kera2/H3-specific behavior must come from live `/object_info` and paired fixtures, not from package-name, node-ID, sentinel, or recipe special cases.

This is particularly important for runtime packages that use custom nodes such as Kera2 and MiniMax H3. Existing runtime packages are API-format packages; they do not prove that a corresponding UI export can be safely normalized without the live schema.

## Offline behavior

Strategy B preserves the Phase 1 UX invariant without claiming that conversion can happen offline:

- Selecting a UI file creates a pending draft and stores the original bytes.
- If ComfyUI is offline or `/object_info` is unavailable, the draft remains pending with `WAITING_FOR_COMFY_UI`.
- If the schema snapshot is reachable but lacks a node, the draft remains pending with `MISSING_NODES`.
- Reanalysis retries using the same draft ID and does not reopen the file picker.
- API-format imports retain their current offline path.
- A UI draft is not importable, publishable, or executable until normalization and all existing validation/capability requirements pass.

This is preferable to silently accepting an incomplete conversion. A simpler fallback remains available: continue asking users to export API JSON when the pending flow is not enabled, but that fallback does not satisfy the intended single-file resume experience.

## Validation

Validation must be layered and fail closed:

1. JSON parse and UI shape validation.
2. Node ID uniqueness and supported ID/subgraph rules.
3. Schema class identity and compatibility validation.
4. Top-level link and per-node socket consistency.
5. Source/output slot existence and type compatibility where schema provides it.
6. Widget descriptor consumption, type, option, range, default, and branch validation.
7. Hidden, optional, seed/control, upload, and dynamic-combo serialization checks.
8. Normalized `WorkflowDocument` parsing.
9. Existing `WorkflowValidator` and `WorkflowGraph` validation.
10. Output selection and output-type validation.
11. Recognition V2 analysis and manual-mapping precedence.
12. Capability/readiness and any existing dry-run checks.
13. Explicit commit/publish validation.

Diagnostics should be deterministic, node/input scoped, and actionable. Validation errors must never be converted into a best-effort executable API graph.

## Golden fixtures

Phase 2B normalizer implementation is blocked until real paired fixtures are
collected; hand-written API fixtures alone are insufficient. The first batch
must include Kera2 T2I, H3 FAST T2V, and an H3 reference/first-last or
reference-video workflow:

- The same workflow saved/exported in UI format and API format.
- The exact ComfyUI version/frontend version used to produce both files.
- The matching `/object_info` snapshot, including custom nodes.
- Expected normalized graph, semantic/structural identity, output type, and recognized bindings.
- Normalization provenance and diagnostics.

The matching `object_info` must come from the trusted local ComfyUI environment
that produced the pair. It must not be generated from a recipe. Each fixture
must also record the workflow format version, frontend version, schema
fingerprint, and normalizer policy version.

The corpus should include at least:

1. A core T2I graph with prompt, seed, sampler controls, dimensions, and image output.
2. A video T2V graph with prompt, seed, duration/frames, and video output.
3. A reference-video graph with multiple reference media inputs.
4. A graph containing linked and widget-backed inputs interleaved.
5. A seed/control-after-generate node.
6. A dynamic combo or branch-dependent widget serialization case.
7. Optional and hidden inputs.
8. A custom-node graph with a valid schema.
9. A missing-node version of that graph.
10. Dangling, duplicate, mismatched, and ambiguous links.
11. Multiple output candidates.
12. UI/API pairs that differ in irrelevant layout metadata but normalize identically.

For each pair, assert:

- normalized API graph equality under the intended canonicalization;
- raw identity inequality when source bytes differ;
- semantic/structural identity equality only when normalized graphs are equal;
- exact linked-input mappings;
- exact widget-to-input mappings;
- expected output;
- expected diagnostics and pending state;
- no second file picker on reanalysis;
- manual mappings are retained and not overwritten.

The existing repository has a small inline UI fixture but no complete paired Kera2/H3 UI/API corpus. Those real exports must be collected and versioned as test fixtures before production compatibility is claimed.

Negative fixtures are a separate gate and must all fail closed: unknown custom
serializer; `widgets_values` count/order mismatch; named-versus-positional
conflict; control-only widget; missing custom node; `BYPASS`; virtual or
primitive node; schema fingerprint mismatch; unsupported frontend version; and
dangling or mismatched links.

## Failure modes

| Failure | Required behavior |
| --- | --- |
| Invalid JSON or unsupported root shape | Reject with format/parse diagnostics; do not create an executable graph. |
| UI file while ComfyUI is offline | Preserve pending draft; report `WAITING_FOR_COMFY_UI`; reanalysis remains picker-free. |
| Missing custom node schema | Preserve original bytes; report `MISSING_NODES`; do not guess or publish. |
| UI/API class mismatch | Reject normalization and identify the node/schema mismatch. |
| Link tuple and node socket disagree | Reject the affected graph; do not choose one representation silently. |
| Dangling or duplicate link | Reject with source/target diagnostics. |
| Widget count or descriptor order mismatch | Reject the affected node; do not shift later values heuristically. |
| Dynamic combo branch cannot be identified | Pending/needs-review; no positional fallback. |
| Unknown custom widget serializer | Pending/needs-review or reject; never copy raw positional values into named inputs. |
| Output candidate is ambiguous | Pending/needs-review; do not use canvas order or last-node heuristics. |
| Normalization changes under a different schema snapshot | Preserve provenance and prevent silent identity merge. |
| Manual mapping targets an unsafe/missing schema input | Reject the mapping; manual precedence does not bypass safety gates. |
| Capability is missing after valid normalization | Keep the valid normalized draft with the existing capability state; do not rewrite it as ready. |
| Commit attempted before normalization/validation | Reject commit; only normalized, validated API workflows may be published. |

## Non-goals

Phase 2B does not include:

- changing Production Queue authority, task semantics, or backend execution;
- changing Artifact, SQLite, migration, or backup behavior;
- changing runtime package contents or adding package-specific conversion rules;
- changing the Phase 2A.1 recognition scoring/evidence model;
- implementing a second executor or a second source of truth;
- making UI JSON directly executable;
- guaranteeing offline UI conversion without schema evidence;
- adding a generic heuristic converter that guesses widget order;
- redesigning the entire import UI;
- adding lint tooling, version bumps, tags, releases, installers, or CI workarounds.

## Implementation boundaries

Future implementation must stay inside these boundaries:

- **Onboarding/lifecycle** owns file selection, draft creation/resume, schema fetching, status, diagnostics, and explicit commit.
- **UI normalizer** owns only pure UI-to-API conversion and provenance.
- **Schema adapter** owns the translation from `RecognitionSchemaContext` and compatible frontend serialization metadata into ordered descriptors.
- **Recognition/capability** consumes the normalized API workflow and existing schema context; it does not parse UI layout.
- **Workflow domain/validator/graph** remains the validation boundary for the normalized executable representation.
- **Package publication** continues to emit the existing API package contract.
- **Frontend** displays pending/error/readiness states and preserves the draft; it must use typed IPC.
- **Tests/fixtures** own paired UI/API evidence and deterministic regression coverage.

No boundary may introduce a second registry, executor, task model, production queue, or persistence authority.

## Recommended approach

Adopt **schema-backed normalization with Strategy B pending drafts**:

1. Keep current API import unchanged.
2. On UI selection, create a same-session pending draft that preserves original bytes.
3. Fetch one compatible `/object_info` snapshot and derive a schema fingerprint.
4. Normalize only with verified node, link, widget, output, and serialization evidence.
5. Fail closed for missing/contradictory evidence.
6. Feed the normalized API graph through the existing validator, graph, Recognition V2, capability, and explicit commit path.
7. Persist provenance in the pending draft needed to explain identity and reanalysis.
8. Use real paired UI/API exports and object_info snapshots as the acceptance corpus.
9. Keep the export-to-API instruction as a safe fallback when the pending flow cannot proceed.

This approach preserves Phase 1's single-picker and same-draft guarantees while avoiding unsafe workflow-specific hardcoding. It also keeps the final runtime package/API contract stable and leaves execution authority untouched.

## Implementation Verification Items

Design Revision V2 freezes the architecture. The following are implementation
verification items, not open architecture questions:

1. Collect trusted paired UI/API fixtures for Kera2 T2I, H3 FAST T2V, and H3
   reference/first-last or reference-video workflows.
2. Record the exact workflow format version, frontend version, backend version
   when available, schema fingerprint, and normalizer policy version for every
   pair. The supported version set is determined by these real fixtures; it is
   not reopened as a generic compatibility discussion.
3. Verify that each supported widget uses `STANDARD_SERIALIZATION_PROVEN` and
   that no Rust path executes frontend JavaScript.
4. Verify descriptor handling for named values, positional values, control
   widgets, optional inputs, dynamic combos, and upload selectors.
5. Verify that pending drafts retain source provenance and permit
   `normalized_api=None`, while the final package remains API-only.
6. Verify deterministic link/output diagnostics and compatibility failures with
   node/input scoped evidence.
7. Verify the existing one-fetch reanalysis path, API import path, manual
   mapping precedence, and explicit commit gate without adding a new authority.
8. Verify the minimal frontend pending/error/readiness states and the
   `EXPORT_API_FORMAT` fallback using the existing typed transport.

## Final audit decision

```text
PHASE2B_DESIGN_REVISION=V2
SUPPORTED_FORMAT_POLICY=ONLY_WORKFLOW_FORMAT_VERSIONS_PROVEN_BY_REAL_GOLDEN_UI_API_PAIRS
FRONTEND_VERSION_POLICY=KNOWN_FRONTEND_VERSIONS_ONLY; UNKNOWN_OR_MISSING_VERSION_FAILS_CLOSED
NORMALIZATION_COMPATIBILITY_CONTEXT=WORKFLOW_FORMAT_VERSION_PLUS_FRONTEND_VERSION_PLUS_SCHEMA_FINGERPRINT_PLUS_NORMALIZER_POLICY_VERSION

NAMED_WIDGET_VALUES_POLICY=USE_FIRST_WHEN_PRESENT_AND_VALID; CROSS_VALIDATE_WITH_SCHEMA_UI_METADATA_AND_POSITIONAL_VALUES; CONFLICT_IS_NORMALIZATION_ERROR
POSITIONAL_WIDGET_VALUES_POLICY=USE_ONLY_WITH_VERIFIED_STANDARD_SERIALIZATION_DESCRIPTOR; OTHERWISE_FAIL_CLOSED
CUSTOM_SERIALIZER_POLICY=STANDARD_SERIALIZATION_PROVEN_ONLY; CUSTOM_OR_UNKNOWN_SERIALIZER_BLOCKS_NORMALIZATION
CONTROL_WIDGET_POLICY=CLIENT_ONLY_CONTROL_FIELDS_REQUIRE_VERIFIED_API_INCLUSION; NAMED_PRESENCE_ALONE_IS_NOT_SUFFICIENT

NODE_MODE_POLICY=NON_STANDARD_MODE_OR_FRONTEND_GRAPH_REWRITE_REQUIRES_GOLDEN_FIXTURE_AND_DETERMINISTIC_RULE
BYPASS_POLICY=FAIL_CLOSED_UNLESS_EXPLICITLY_PROVEN
VIRTUAL_NODE_POLICY=FAIL_CLOSED_UNLESS_EXPLICITLY_PROVEN
SUBGRAPH_POLICY=FAIL_CLOSED_UNLESS_EXPLICITLY_PROVEN

PENDING_DRAFT_WITHOUT_API_DOCUMENT=UI_SOURCE_PENDING_WITH_NORMALIZED_API_DOCUMENT_NONE; NO_PLACEHOLDER_WORKFLOW_DOCUMENT
COMMIT_REQUIRES_NORMALIZED_API=YES; BLOCKER_FREE_DIAGNOSTICS_WORKFLOW_VALIDATOR_WORKFLOW_GRAPH_RECOGNITION_AND_EXISTING_CAPABILITY_RULES

FINAL_PACKAGE_CONTRACT_CHANGED=NO
PROVENANCE_SCOPE=PENDING_DRAFT_SOURCE_BYTES_RAW_SHA_SOURCE_FORMAT_FRONTEND_VERSION_SCHEMA_FINGERPRINT_POLICY_VERSION_DIAGNOSTICS_AND_NORMALIZED_BYTES_WHEN_AVAILABLE

IDENTITY_STRATEGY=RAW_SHA_FROM_SOURCE; SEMANTIC_AND_STRUCTURAL_SHA_ONLY_AFTER_NORMALIZED_API_READY; NO_SILENT_CROSS_CONTEXT_MERGE

GOLDEN_PAIR_GATE=REAL_KERA2_T2I_H3_FAST_T2V_AND_H3_REFERENCE_OR_FIRST_LAST_UI_API_PAIRS_WITH_MATCHING_OBJECT_INFO_AND_VERSION_CONTEXT_REQUIRED_BEFORE_IMPLEMENTATION
NEGATIVE_FIXTURE_GATE=UNKNOWN_SERIALIZER_WIDGET_MISMATCH_NAMED_POSITIONAL_CONFLICT_CONTROL_ONLY_MISSING_NODE_BYPASS_VIRTUAL_PRIMITIVE_SCHEMA_OR_FRONTEND_MISMATCH_AND_LINK_ERRORS_ALL_FAIL_CLOSED

SOURCE_CODE_CHANGED=NO
WORKTREE=MODIFIED_ONLY_BY_PHASE2B_DESIGN_DOC

NEW_ARCHITECTURE_BLOCKER=NONE
PHASE2A1_FINALIZATION_STATUS=COMPLETE
PHASE2B_AUDIT_MODE=READ_ONLY
COMMIT=NONE
PUSH=NONE
RELEASE=NONE
TAG=NONE
INSTALLER_BUILD=NOT_REQUIRED
PHASE2B_DESIGN_STATUS=READY_FOR_IMPLEMENTATION_PLAN
```




