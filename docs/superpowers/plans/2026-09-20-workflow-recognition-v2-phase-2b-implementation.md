# Workflow Recognition V2 Phase 2B Implementation Plan

> For the implementer: execute this plan only after review. Do not commit,
> push, tag, release, build an installer, or begin unrelated architecture work.
> The first implementation gate is real paired UI/API fixture collection. If
> that gate does not pass, stop before changing the normalizer or onboarding
> path.

## Baseline and hard boundaries

- Required implementation baseline:
  0e3080ebaba8f2f787fbcbab44574d13c9da6a98.
- HEAD and origin/master must remain equal to that SHA.
- Approved V2 design:
  docs/superpowers/plans/2026-09-20-workflow-recognition-v2-phase-2b-design.md.
- This plan is the only new artifact for this task. The V2 design cleanup is
  limited to provenance wording and conversion of the old Open Questions
  section into Implementation Verification Items.
- No database migration, runtime package schema change, Production Queue
  change, Task change, Artifact change, backup change, or second execution
  authority is allowed.
- API-format import remains the existing path and remains usable offline.
- UI-format import is never directly executable. It must become a validated
  normalized API workflow before recognition, capability readiness, recipe
  creation, or commit.
- Phase 2B published packages continue to contain the existing
  workflow_api.json API contract only.
- Rust local gates use --jobs 2; frontend lint remains
  N/A_NOT_CONFIGURED.

## Architecture invariants

The implementation must preserve these invariants throughout every task:

~~~text
one UI file picker per import session
same draft ID across reanalysis
one object_info fetch per reanalysis operation
API import bypasses the UI normalizer
UI_SOURCE_PENDING may have normalized_api=None
NORMALIZED_API_READY is required before API-only flows
manual mapping cannot bypass normalization safety
semantic/structural identity starts only after normalization
explicit commit is the only package publish path
original UI bytes never become workflow_api.json
~~~

The compatibility input is explicit and versioned:

~~~text
NormalizationCompatibilityContext:
  workflow_format_version
  frontend_version
  schema_fingerprint
  normalizer_policy_version
~~~

The first supported set is the exact set proven by real fixtures. It is not
"all ComfyUI".

## Source audit and expected file matrix

The current source audit found these concrete boundaries:

| Path | Planned responsibility |
| --- | --- |
| src-tauri/src/application/workflow_onboarding_service.rs | Existing import, in-memory draft registry, one-fetch reanalysis, validation, recipe construction, identity lookup, and publish gate. Extend minimally for UI source/pending state. |
| src-tauri/src/application/workflow_recognition_schema.rs | Existing pure /object_info parser for declared types, ranges, uploads, and outputs. Extend only with stable schema projection/fingerprint data needed by the adapter. |
| src-tauri/src/application/workflow_analysis_service.rs | Existing API-graph analysis and Recognition V2 evidence. Consume normalized API only; no UI parsing or scoring rewrite. |
| src-tauri/src/application/workflow_recognition_service.rs | Existing API recognition and identity classification. Keep algorithm unchanged; call it only with normalized API documents for UI imports. |
| src-tauri/src/application/workflow_semantic_identity.rs | Existing API canonicalization and SHA functions. Do not alter the identity algorithm. |
| src-tauri/src/application/mod.rs | Register the new pure modules. |
| src-tauri/src/application/workflow_ui_normalizer.rs | New pure UI parser, link/widget/node normalization, output selection, provenance, and structured failures. |
| src-tauri/src/application/workflow_ui_serialization.rs | New pure schema-to-serialization descriptor adapter and compatibility policy types. |
| src-tauri/tests/fixtures/workflow_ui/phase2b/ | Real paired UI/API fixtures, matching object_info, metadata, fingerprints, and expected normalized result. |
| src-tauri/tests/dev103_workflow_ui_normalizer.rs | Golden-pair, synthetic positive, negative, identity, and compatibility tests. |
| src-tauri/tests/dev104_workflow_ui_onboarding.rs | Pending draft, one-fetch reanalysis, offline resume, missing-node, and commit safety integration tests. |
| src/types/workflowOnboarding.ts | Additive source/normalization/diagnostic view types; do not create a second capability authority. |
| src/features/workflows/hooks/useWorkflowSmartImportController.ts | Preserve UI pending drafts; remove the current non-API discard behavior; keep reanalysis picker-free. |
| src/features/workflows/WorkflowSmartImport.tsx | Route UI pending plans to the pending/error view instead of treating every UI result as a terminal format error. |
| src/features/workflows/WorkflowImportIssues.tsx | Display pending/blocked diagnostics, compatible retry, and API export fallback. |
| src/features/workflows/workflowSmartImportModel.ts | Map normalization and compatibility error codes to user-facing messages. |
| src/features/workflows/WorkflowImportIssues.test.tsx | Frontend pending/error/fallback regression coverage. |
| src/features/workflows/WorkflowAddUat.test.tsx | Smart Import UI behavior coverage. |
| src/features/workflows/hooks/useWorkflowSmartImportController.test.tsx | Same-draft and no-second-picker coverage. |

No change is expected in src-tauri/src/commands/workflow_registry.rs or
src/services/tauriClient.ts unless additive serialized fields require a
type-only adjustment; command names and transport boundaries remain unchanged.
Do not add raw frontend invoke calls.

## Task 1 — COLLECT_AND_FREEZE_REAL_GOLDEN_PAIRS

### Goal

Collect and freeze the first supported compatibility set before writing any
normalizer, draft model, onboarding conversion path, or frontend pending UI.

### Files

- src-tauri/tests/fixtures/workflow_ui/phase2b/kera2_t2i/
- src-tauri/tests/fixtures/workflow_ui/phase2b/h3_fast_t2v/
- src-tauri/tests/fixtures/workflow_ui/phase2b/h3_reference/
- A metadata.json file in each directory.
- No product source file.

Each pair must contain:

~~~text
ui_workflow.json
api_workflow.json
object_info.json
metadata.json
expected_normalized_api.json
~~~

Fixture metadata must additionally preserve the serialization-profile identity
and provenance:

~~~text
source_frontend_version
serialization_profile_version
frontend_source_revision when available
~~~

### Preconditions

- Use a real local ComfyUI installation and one compatible environment per
  pair.
- The UI and API exports must represent the same workflow from the same
  environment.
- Record workflow format version, frontend version, backend version when
  available, schema fingerprint, and normalizer policy version.
- Do not infer object_info from recipe.yaml.
- Do not hand-write an API file or reverse-engineer a UI file from an API file.

### Exact code boundary

Only fixture artifacts and their metadata are in scope. Do not add
workflow_ui_normalizer.rs, change WorkflowOnboardingDraft, or modify frontend
code until this task passes.

### Tests first

Define fixture manifest validation before accepting the corpus:

- both UI and API files parse;
- the API file passes current API validation;
- object_info.json is an object;
- metadata has no unknown version placeholders;
- the recorded schema fingerprint matches the canonical snapshot;
- expected output and core bindings are explicit.

### Implementation

Collect:

1. Kera2 T2I with prompt, seed, width, height, steps, and image output.
2. H3 FAST T2V with prompt, seed, dimensions, duration, fps, optional steps or
   denoise, and video output.
3. H3 reference/first-last/reference-video with prompt, seed, dimensions,
   duration/fps, and the actual reference slots present in the workflow.

The expected normalized API result is the trusted API export copied into the
fixture as an expected value, with any intentional canonicalization documented.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer golden_fixture
~~~

The exact test filter may be introduced with the fixture loader.

### Acceptance

- Exactly three initial real pairs are present.
- Every pair has complete version and fingerprint metadata.
- No pair has unknown frontend or workflow format version.
- The supported compatibility set is an explicit finite set derived from these
  records.
- Synthetic fixtures are clearly separate and do not count toward the three.

### Stop condition

If any pair or required version metadata is unavailable:

~~~text
PHASE2B_IMPLEMENTATION_STATUS=BLOCKED_ON_FIXTURES
~~~

Stop. Do not write a generic positional converter.

## Task 2 — Build the pure UI document parser

### Goal

Parse only the UI fields required for deterministic conversion. Keep layout
metadata out of the conversion authority.

### Files

- New src-tauri/src/application/workflow_ui_normalizer.rs.
- src-tauri/src/application/mod.rs.
- Unit tests in the new module.
- src-tauri/tests/dev103_workflow_ui_normalizer.rs for fixture loading.

### Preconditions

- Task 1 passes.
- The parser accepts the UI shape already detected by
  detect_comfy_workflow_format.
- The parser performs no API conversion and no ComfyUI I/O.

### Exact code boundary

Define pure data types for:

- root nodes and links;
- root workflow format version, using the exact source-declared field (the
  source-equivalent root version field may be mapped to the typed
  workflow_format_version; absence remains unknown);
- root extra.frontendVersion as source-declared frontend provenance;
- node id, type, and mode;
- node inputs and outputs;
- widgets_values;
- optional widgets_values_named;
- properties;
- link tuple fields and source/target slots.

Do not promote position, size, groups, editor selection, or arbitrary layout
metadata to execution authority. The production compatibility context must
take workflow_format_version from the imported UI source and frontend_version
from source extra.frontendVersion when trusted. A live ComfyUI frontend version
may only be compared with source provenance; it must never replace missing
source provenance. Missing source frontend version is UNKNOWN and fails the
compatibility gate.

### Tests first

Add failing tests for valid UI 0.4 shape, parsing the source workflow format
version, parsing extra.frontendVersion, missing nodes/links, duplicate node
IDs, malformed link tuples, missing/non-array widget values, optional named
values, unknown node mode, and preservation of original source bytes outside
the parsed document. Name the compatibility regressions explicitly:

~~~text
parses_workflow_format_version
parses_extra_frontend_version
missing_frontend_version_fails_compatibility_gate
live_frontend_version_does_not_replace_missing_source_version
~~~

### Implementation

Return a structured UI document or typed parse failure. Preserve node ordering
only as source evidence; never use it as execution order. Keep original bytes
outside the parsed document for draft provenance.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer parse_ui
~~~

### Acceptance

The parser is deterministic, has no I/O, and represents every field needed by
later link, widget, mode, and output tests without making layout authoritative.

### Stop condition

Any parser behavior that guesses class identity, input names, widget order, or
output selection is out of scope.

## Task 3 — Build the schema serialization descriptor adapter

### Goal

Convert the existing RecognitionSchemaContext plus frozen compatibility metadata
into a pure UiSerializationDescriptorSet.

### Files

- New src-tauri/src/application/workflow_ui_serialization.rs.
- Minimal additive changes to src-tauri/src/application/workflow_recognition_schema.rs.
- src-tauri/src/application/mod.rs.
- Unit tests in both modules and dev103_workflow_ui_normalizer.rs.

### Preconditions

- Task 1 supplies exact supported compatibility contexts.
- Existing schema parsing remains the source for declared types,
  required/optional inputs, ranges, enum options, uploads, and outputs.
- No frontend JavaScript is available to Rust.

### Exact code boundary

Define:

~~~text
NormalizationCompatibilityContext
SupportedNormalizationCompatibilitySet
FrontendSerializationProfile
UiSerializationDescriptorSet
UiNodeSerializationDescriptor
UiInputSerializationDescriptor
SerializerKind
~~~

FrontendSerializationProfile is the production serializer authority. It must
bind:

~~~text
frontend_version
workflow_format_version
serialization_profile_version
source provenance
supported standard serializer semantics
frontend_source_revision when available
~~~

The production construction chain is:

~~~text
RecognitionSchemaContext
+ FrontendSerializationProfile
+ NormalizationCompatibilityContext
-> UiSerializationDescriptorSet
~~~

Golden pairs validate that this profile matches real exports; they do not
become a source for generic index-specific serializer rules. The adapter must
never infer serializer semantics from a Kera2/H3 package, node ID, workflow
SHA, recipe ID, or a widgets_values position.

Use at least:

~~~text
StandardDirect
StandardCombo
StandardUpload
StandardSeed
WorkflowOnlyControl
UnsupportedCustom
Unknown
~~~

Each descriptor must capture class identity, ordered API inputs,
linked/widget role, required/optional/hidden/default information, declared
type, options/ranges, media semantics, API inclusion, and serialization kind.

Expose a stable canonical schema fingerprint helper. Do not guess frontend
version from object_info; it must come from fixture/environment metadata or an
explicit compatible runtime source.

### Tests first

Add tests for deterministic fingerprints, exact FrontendSerializationProfile
identity/provenance, required versus optional precedence,
upload/media metadata, output slot types and output-node flags, exact
compatibility tuple matching, unknown format/frontend/schema/policy rejection,
serializer classification, seed handling, and workflow-only controls.

### Implementation

Build the adapter as a pure translation layer. It may consume the exact
FrontendSerializationProfile selected by the compatibility context, but it
must not execute frontend code. Reject an unverified combination or unavailable
profile before any widget cursor is advanced.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib workflow_recognition_schema
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer compatibility
~~~

### Acceptance

The normalizer receives a complete, versioned descriptor set without scattered
reads from RecognitionSchemaContext. Unsupported compatibility is a structured
blocker.

### Stop condition

If the adapter needs a guessed frontend version, package-name special case, or
JavaScript execution, stop and mark the combination unsupported.

## Task 4 — Implement pure link normalization

### Goal

Reconstruct API link inputs before any widget input merge.

### Files

- src-tauri/src/application/workflow_ui_normalizer.rs.
- Unit tests in the normalizer module.
- src-tauri/tests/dev103_workflow_ui_normalizer.rs.

### Preconditions

- Parsed UI document from Task 2.
- Descriptor set from Task 3.
- No widget values are consumed in this task.

### Exact code boundary

For every UI link tuple:

~~~text
[link_id, origin_node, origin_slot, target_node, target_slot, type]
~~~

cross-check:

~~~text
target_node.inputs[target_slot].link
~~~

Emit only:

~~~text
api_input_name: [origin_node_id, origin_slot]
~~~

Validate target/source node existence, target/origin slot existence, link ID
agreement, duplicate target inputs, contradictory tuples, and type evidence
when available. Link IDs and UI display types must not appear in API values.

### Tests first

Add tests for one valid link, mixed linked and widget inputs, dangling source,
missing target socket, mismatched link ID, duplicate target input, conflicting
top-level/node-level evidence, invalid origin slot, and type contradiction.

### Implementation

Return deterministic node/input-scoped diagnostics. Never silently choose one
of two conflicting link representations.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer link
~~~

### Acceptance

All valid links produce the existing API link representation and every listed
negative case fails closed.

### Stop condition

Do not proceed to widget merge until link normalization is independently green.

## Task 5 — Implement pure widget normalization

### Goal

Map workflow-persistence widget values to API input names only through verified
serialization descriptors.

### Files

- src-tauri/src/application/workflow_ui_normalizer.rs.
- src-tauri/src/application/workflow_ui_serialization.rs if descriptor helpers
  need a small pure extension.
- Unit tests and dev103_workflow_ui_normalizer.rs.

### Preconditions

- Task 3 descriptors and Task 4 link results are available.
- Linked inputs are excluded from widget cursor consumption as described by the
  descriptor.
- Rust does not execute frontend JavaScript.

### Exact code boundary

Only descriptors classified as STANDARD_SERIALIZATION_PROVEN may consume
positional values. Use this precedence:

~~~text
widgets_values_named when present and valid
-> verified positional widgets_values descriptor mapping
-> fail closed
~~~

Cross-check named values with schema descriptors, UI widget/input metadata,
and positional values when available. Distinguish:

~~~text
WORKFLOW_SERIALIZATION
API_PROMPT_SERIALIZATION
~~~

Implement a descriptor cursor with consumed count, missing value detection,
unused value detection, type/options/range validation, optional/default
semantics, upload/media handling, seed/control handling, and dynamic-combo
branch validation.

WorkflowOnlyControl, UnsupportedCustom, and Unknown never emit an API input.
A named control key is not proof of API inclusion.

### Tests first

Add tests for valid named values, named/positional conflict, standard direct and
multiline strings, integer, float, boolean, combo, upload, optional omission
versus explicit default, standard seed, client-only control, custom/unknown
serializer, count/order mismatch, missing/unused values, and dynamic branch
mismatch.

### Implementation

Return named literals plus provenance. Scope diagnostics to node ID, input name,
and widget index when relevant. Never shift after an unreconciled mismatch.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer widget
~~~

### Acceptance

Every supported widget path is standard and deterministic. Unsupported
serialization produces NORMALIZATION_BLOCKED or UnsupportedSerializer, never a
guessed API value.

### Stop condition

Any test that requires copying raw positional values without a descriptor is
invalid and must be removed.

## Task 6 — Build the pure node/API graph normalizer

### Goal

Combine verified class identity, normalized links, and widget literals into a
validated API graph with deterministic outputs.

### Files

- src-tauri/src/application/workflow_ui_normalizer.rs.
- src-tauri/src/application/workflow_ui_serialization.rs only for descriptor
  helpers.
- Unit and integration tests in dev103_workflow_ui_normalizer.rs.

### Preconditions

- Tasks 2–5 pass.
- Compatibility context is an exact supported tuple.
- API class identity comes from compatible schema, not a blind copy of UI type.

### Exact code boundary

For each ordinary executable node:

1. Resolve verified API class_type.
2. Merge normalized links and widget literals by named input.
3. Preserve explicit omission/default semantics.
4. Reconstruct all executable nodes and preserve declared output slot/type
   evidence and output provenance.
5. Produce normalized WorkflowDocument, API value/bytes, source-to-API
   provenance, diagnostics, and compatibility context. Do not implement a
   second semantic image/video output scorer here.

After NORMALIZED_API_READY, the existing WorkflowAnalysisService remains the
sole semantic output inference/scoring authority:

~~~text
UI normalizer -> complete API graph -> WorkflowAnalysisService -> outputs
~~~

The normalizer may reject an invalid slot, impossible link, or schema output
contradiction as conversion validation. It must not duplicate output scoring.

Fail closed for bypass, virtual, primitive, reroute without proven semantics,
subgraph, mute/never, unknown mode, frontend-only graph rewrite, and unknown
custom node. Do not simulate ComfyUI frontend graph rewriting.

### Tests first

Add tests for an ordinary linked/literal graph, verified custom node, unknown
custom node, declared image/video output slots, output provenance, mode/bypass/virtual/
primitive/subgraph blockers, class identity mismatch, and link/literal collision.

### Implementation

Keep the result pure. No registry, package store, Comfy adapter, frontend, or
database dependency.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer node
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer output
~~~

### Acceptance

The result is either a complete API-shaped workflow with declared output
evidence or a structured failure. Final semantic output selection is performed
only by WorkflowAnalysisService. No partial executable graph is returned.

### Stop condition

Do not integrate onboarding until positive and negative normalizer tests are
green.

## Task 7 — Validate normalized workflow and preserve identity semantics

### Goal

Send only normalized API workflows through existing validation, compiler, graph,
recognition, and identity boundaries without changing identity algorithms.

### Files

- workflow_ui_normalizer.rs tests.
- workflow_onboarding_service.rs validation integration.
- workflow_semantic_identity.rs should remain unchanged; add tests only if an
  integration gap is found.
- workflow_recognition_service.rs should remain unchanged unless a type-only
  call-site adjustment is required.
- dev103_workflow_ui_normalizer.rs.
- dev104_workflow_ui_onboarding.rs.

### Preconditions

- Task 6 returns a validated API-shaped document.
- Existing WorkflowValidator, WorkflowGraph, binding validator, dry-run
  compiler, and SHA functions are available.

### Exact code boundary

For NORMALIZED_API_READY only:

- parse with WorkflowDocument;
- run WorkflowValidator and WorkflowGraph;
- run existing Recognition V2 analysis;
- calculate semantic/structural identity;
- run existing recipe/binding/dry-run validation.

Before that state retain raw SHA only. Do not alter API canonicalization,
semantic SHA, or structural SHA algorithms.

### Tests first

Add tests for UI/API normalized graph equality, different raw SHA, equal
semantic/structural SHA for equal normalized graphs, identity changes for
different links/literals, invalid normalized graph rejection, and no
normalized identity for pending UI state.

### Implementation

Use existing recognition identity classification. Do not introduce a second
duplicate authority. Verify existing ExactSemantic behavior where appropriate.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer identity
cargo test --manifest-path src-tauri/Cargo.toml --test dev104_workflow_ui_onboarding validation
~~~

### Acceptance

Identity convergence occurs only for equal normalized API graphs and existing
API validation remains authoritative.

### Stop condition

Any need to change identity hashing or duplicate classification is a scope
escalation.

## Task 8 — Add the pending UI draft lifecycle

### Goal

Represent UI source and normalization lifecycle without creating a fake
WorkflowDocument.

### Files

- workflow_onboarding_service.rs.
- workflow_ui_normalizer.rs for result/error types.
- application/mod.rs if exports are needed.
- src/types/workflowOnboarding.ts.
- dev104_workflow_ui_onboarding.rs.

### Preconditions

- Tasks 1–7 pass.
- Existing API import behavior is captured before draft changes.
- The in-memory registry remains the only draft registry.

### Exact code boundary

Extend the internal WorkflowOnboardingDraft minimally to express:

~~~text
source_format
original_source_bytes
source_filename
raw_sha
workflow_format_version
frontend_version
compatibility_context
normalization_state
normalized_api: Option<WorkflowDocument>
normalization_diagnostics
~~~

Keep original_source_bytes backend-only. The serialized frontend onboarding
view must not return the raw UI blob over IPC. It may expose only:

~~~text
source_format
source_filename
raw_sha
workflow_format_version
frontend_version
normalization_state
diagnostics
compatibility summary
existing review information
~~~

The explicit invariant is RAW_UI_BYTES_OVER_IPC=NO; do not place large workflow
bytes in React state or make internal provenance storage a frontend authority.

Use explicit states:

~~~text
UI_SOURCE_PENDING
NORMALIZED_API_READY
~~~

The API path creates NORMALIZED_API_READY with an API document. The UI path
may create UI_SOURCE_PENDING with normalized_api=None. Do not use a placeholder
document.

Add serialized source/normalization state and diagnostics. Pending views must
not claim API nodes, API validation, recognition, or capability readiness that
has not occurred. Keep draft IDs stable.

### Tests first

Add tests for API import remaining immediately usable, UI import creating a
pending draft with raw source and no API document, deterministic blocker and
source metadata, explicit discard, and stable draft ID.

### Implementation

Refactor API-only helpers behind a require_normalized_api boundary. In
particular, do not call recipe construction, package identity lookup, binding
validation, or publish logic while normalized_api=None.

Reuse existing onboarding states where possible and add only the minimum
normalization-state view. Do not create another capability enum.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev104_workflow_ui_onboarding pending
cargo test --manifest-path src-tauri/Cargo.toml --test dev081_complex_workflow_onboarding
~~~

### Acceptance

A UI pending draft is representable, serializable without raw bytes,
discardable, and non-committable without a fake API document. Existing API
draft tests remain green.

### Stop condition

Any path requiring a placeholder WorkflowDocument must be refactored before
continuing.

## Task 9 — Integrate reanalysis and preserve one-fetch behavior

### Goal

Make pending UI drafts resumable through workflow_reanalyze_import(draftId)
without a second picker or second object_info fetch.

### Files

- workflow_onboarding_service.rs.
- dev104_workflow_ui_onboarding.rs.
- Existing one-fetch tests in workflow_onboarding_service.rs or
  dev081_complex_workflow_onboarding.rs.

### Preconditions

- Task 8 supports pending UI source.
- ComfyAdapter call-count harness is available.
- Existing API reanalysis behavior is captured.

### Exact code boundary

For one reanalysis operation:

~~~text
get_object_info once
-> build RecognitionSchemaContext
-> build UiSerializationDescriptorSet
-> check compatibility
-> normalize UI source if pending
-> validate normalized API
-> recognition
-> capability
-> update same draft
~~~

Offline/timeout leaves the UI draft pending with WAITING_FOR_COMFY_UI.
Missing schema or version mismatch leaves it pending with MISSING_NODES,
WAITING_FOR_COMPATIBLE_SCHEMA, or the equivalent structured state. API import
retains its existing static/offline behavior.

Reorder auto_confirm_internal so identity lookup and API-only validation do not
run before normalization. Preserve manual mappings and metadata.

### Tests first

Add tests for offline selection then same-draft resume, one object_info call,
no second picker, stable draft ID, schema mismatch pending, and unchanged API
offline behavior.

### Implementation

Keep command signatures unchanged. Centralize one schema snapshot and pass it
through adapter, normalizer, analysis, and capability evaluation. The
normalizer and adapter never fetch object_info.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev104_workflow_ui_onboarding reanalyze
cargo test --manifest-path src-tauri/Cargo.toml --test dev081_complex_workflow_onboarding reanalysis_fetches_object_info_once
~~~

### Acceptance

~~~text
OBJECT_INFO_CALL_COUNT=1
DRAFT_ID_STABLE=YES
SECOND_FILE_PICKER=NO
API_IMPORT_REGRESSION=NONE
~~~

### Stop condition

Any nested schema fetch or picker call is a hard failure.

## Task 10 — Integrate Recognition, capability, and manual mapping

### Goal

Ensure downstream behavior consumes only the normalized API graph and the same
schema snapshot.

### Files

- workflow_onboarding_service.rs.
- workflow_analysis_service.rs only for type-neutral call-site adjustment.
- workflow_recognition_service.rs only for type-neutral call-site adjustment.
- dev104_workflow_ui_onboarding.rs.
- Existing dev102_workflow_recognition_v2.rs as regression corpus.

### Preconditions

- Task 9 produces NORMALIZED_API_READY.
- Existing Recognition V2 and capability tests are green.
- Manual mapping precedence is established by Phase 1.

### Exact code boundary

After normalization:

- run existing Recognition V2 against WorkflowDocument;
- run capability against normalized nodes and the same schema;
- calculate existing identity and duplicate classification;
- expose recognition issues/candidates;
- permit manual mapping only for safe normalized candidates;
- preserve manual mapping precedence on reanalysis.

Never score UI nodes or widgets_values directly. Manual mapping cannot bypass
missing schema, widget normalization, link validation, or commit gates.

### Tests first

Add tests for Kera2 recognition bindings, H3 FAST core bindings, reference
media slots, normalized capability, manual mapping retention, manual mapping
rejection for missing schema/serializer, and UI/API duplicate convergence.

### Implementation

Reuse current analysis, recognition, capability, and recipe code. Do not add a
second recognition path or duplicate authority.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer recognition
cargo test --manifest-path src-tauri/Cargo.toml --test dev104_workflow_ui_onboarding capability
cargo test --manifest-path src-tauri/Cargo.toml --test dev102_workflow_recognition_v2
~~~

### Acceptance

Recognition and capability outputs describe normalized API graph, and Phase
2A.1 regressions remain green.

### Stop condition

Any recognition heuristic that reads UI layout or widget position is out of
scope.

## Task 11 — Add minimal frontend pending/error/readiness states

### Goal

Expose pending UI import state without redesigning Smart Import or creating
frontend authority.

### Files

- src/types/workflowOnboarding.ts.
- src/features/workflows/hooks/useWorkflowSmartImportController.ts.
- src/features/workflows/WorkflowSmartImport.tsx.
- src/features/workflows/WorkflowImportIssues.tsx.
- src/features/workflows/workflowSmartImportModel.ts.
- Existing frontend tests listed in the source matrix.

### Preconditions

- Backend plan/view fields from Tasks 8–10 are stable.
- Typed transport functions remain unchanged.
- Existing Smart Import/Advanced Editor tests pass before edits.

### Exact code boundary

Add only fields required to show current normalization state, blocker reason,
node/input context, recommended action, source format, and compatibility
fallback.

Controller changes:

- retain the analyzed UI draft instead of discarding non-API drafts;
- always retrieve the draft for a pending plan;
- keep reanalyzeWorkflowImport(draftId) as the resume path;
- preserve the current session and avoid a second picker;
- do not open Advanced Editor until an API document is ready unless it can
  safely represent pending state without API-only actions.

Component changes:

- stop converting every UI result into a terminal format error;
- show waiting, missing-node, normalization-blocked, needs-review, and ready;
- show node/input-scoped diagnostics;
- keep Export API Format as fallback for unsupported compatibility,
  serializer, mode, or schema;
- disable commit until backend reports importable/ready.

### Tests first

Add tests for pending rendering, same-draft resume, no second picker, waiting
retry, missing node/serializer fallback, ready review/commit UI, unchanged API
error behavior, and blocked advanced publish.

### Implementation

Reuse existing WorkflowImportIssues and controller error paths. Do not add
raw Tauri calls, a second store, or client-side normalization decisions.

### Focused test command

~~~powershell
pnpm test --run src/features/workflows/hooks/useWorkflowSmartImportController.test.tsx src/features/workflows/WorkflowImportIssues.test.tsx src/features/workflows/WorkflowAddUat.test.tsx
~~~

### Acceptance

The frontend makes pending UI state understandable and resumable while
remaining a view/controller over backend state.

### Stop condition

If frontend code starts interpreting widget serialization, class identity, or
API readiness independently, stop.

## Task 12 — Add golden, negative, regression, and commit-safety tests

### Goal

Prove the complete conversion contract, not just JSON parseability.

### Files

- src-tauri/tests/dev103_workflow_ui_normalizer.rs.
- src-tauri/tests/dev104_workflow_ui_onboarding.rs.
- src-tauri/tests/fixtures/workflow_ui/phase2b/.
- Existing dev079/dev081 tests only for focused harness additions.
- Frontend tests from Task 11.

### Preconditions

- Tasks 1–11 pass.
- Three real pairs and matching schema snapshots are available.
- Fixture loader verifies metadata and fingerprints.

### Exact code boundary

Positive cases:

~~~text
simple linked numeric input
mixed linked + widget inputs
standard STRING and multiline STRING
INT / FLOAT / BOOLEAN / COMBO
standard upload selector
optional input
known seed serialization
Kera2 prompt/seed/width/height/steps/image output
H3 FAST prompt/seed/width/height/duration/fps/video output
H3 reference or first/last media slots/video output
~~~

Negative cases, all fail closed:

~~~text
unknown custom serializer
widget count/order mismatch
widgets_values_named conflict
control-only widget
missing custom node
BYPASS
virtual node
primitive node
subgraph
unsupported frontend version
unsupported workflow format version
schema fingerprint incompatibility
dangling link
duplicate link
socket/tuple contradiction
ambiguous output
~~~

Add commit-safety tests for pending and blocked drafts. Verify workflow_api.json
is normalized API export, never source UI bytes.

### Tests first

Run golden equality before broad integration. Assert canonical API graph,
class types, exact named inputs/links, literal values, output slots/types,
semantic/structural SHA, Recognition bindings, raw SHA inequality for UI/API,
and no normalized identity before ready.

### Implementation

Keep tests deterministic and fixture-driven. Do not generate object_info from
recipes. Do not compare raw JSON byte equality.

### Focused test command

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --test dev103_workflow_ui_normalizer
cargo test --manifest-path src-tauri/Cargo.toml --test dev104_workflow_ui_onboarding
pnpm test --run src/features/workflows/hooks/useWorkflowSmartImportController.test.tsx src/features/workflows/WorkflowImportIssues.test.tsx src/features/workflows/WorkflowAddUat.test.tsx
~~~

### Acceptance

All positive pairs converge to their real API exports and all negative fixtures
remain pending/blocked with structured diagnostics.

### Stop condition

A failing golden equality test is a blocker, not a reason to relax comparison
to JSON-valid or name-only assertions.

## Task 13 — Run real desktop and offline smoke

### Goal

Validate the lifecycle with the real Tauri desktop app, native file picker, and
live ComfyUI. No generation is required.

### Files

- No product source changes.
- Manual evidence only; do not modify fixtures without recording provenance.

### Preconditions

- Tasks 1–12 pass.
- Live ComfyUI can provide the matching schema for all three pairs.
- The app is started from the implementation worktree.
- Exact app/frontend/backend/schema versions are recorded.

### Exact code boundary

Run:

1. Kera2 UI import: one picker, pending/normalize, prompt/seed/width/height/
   steps, image output, capability.
2. H3 FAST UI import: prompt/seed/dimensions/duration/fps and video output.
3. H3 reference/first-last/reference-video: only media fields present in the
   source, deterministic slot order, video output.
4. Return to Smart Import and Advanced flow with the same draft ID.
5. Select UI while ComfyUI is offline, verify pending, start compatible ComfyUI,
   resume without a second picker, and reach normalized ready.
6. Use a controlled missing-node environment or fixture-backed integration
   smoke to verify MISSING_NODES, no normalized API, and no commit.
7. Verify API-format import still works offline.

Do not generate media, modify Production Queue, or publish a package during
this smoke unless a separate acceptance step authorizes it.

### Tests first

Use the completed automated golden/negative suite as a precondition. Manual
checks cover native picker, real schema, and UI lifecycle behavior.

### Implementation

No implementation changes are made in this task. Record:

~~~text
KERA2_UI_IMPORT=
H3_FAST_UI_IMPORT=
H3_REFERENCE_UI_IMPORT=
OFFLINE_RESUME=
MISSING_NODE=
SECOND_FILE_PICKER=
DRAFT_ID_STABLE=
~~~

### Focused test command

No new command. Use the repository's existing Tauri development workflow and
retain manual evidence.

### Acceptance

All three imports normalize and recognize correctly, offline resume preserves
the draft, missing nodes fail closed, and no second picker appears.

### Stop condition

Any UI mismatch, second picker, false ready state, or provenance ambiguity
blocks completion. Add a focused test before any fix.

## Task 14 — Run full gates and stop before integration

### Goal

Verify the implementation branch without committing or releasing it.

### Files

- Only files changed by Tasks 1–13.
- No release/tag/installer files.

### Preconditions

- Tasks 1–13 pass.
- No fixture, source, or manual smoke blocker.
- Worktree contains only planned Phase 2B files and fixtures.

### Exact code boundary

Run:

~~~powershell
cargo fmt --check
cargo check
cargo test --all-targets --jobs 2

pnpm test --run
pnpm exec tsc --noEmit
pnpm build

git diff --check
git status --short
git diff --stat
~~~

Record:

~~~text
FRONTEND_LINT=N/A_NOT_CONFIGURED
RUST_TEST_JOBS=2
DATABASE_MIGRATION=NONE
RUNTIME_PACKAGE_CONTRACT_CHANGED=NO
NEW_EXECUTION_AUTHORITY=NO
~~~

Review for no second registry/executor/queue/task, no raw frontend invoke,
no fake API document, no package metadata/schema change, no identity algorithm
change, and no Production Queue or unrelated UI changes.

### Tests first

No further implementation after full gates begin. If a gate fails, return to
the responsible task, add a focused regression test, fix only that scope, and
rerun the focused command plus affected full gate.

### Implementation

Do not commit, push, tag, release, or build an installer. This plan stops at
review-ready implementation evidence.

### Focused test command

The commands above are the final local gates. Use PowerShell and Rust
--jobs 2.

### Acceptance

All local gates pass, the diff is scoped, and implementation is ready for
review. Source-only CI and release work are separate instructions.

### Stop condition

Stop immediately with:

~~~text
PHASE2B_IMPLEMENTATION_STATUS=READY_FOR_REVIEW
~~~

## Implementation failure policy

- Missing real paired fixtures blocks implementation before Task 2.
- Unknown or unverified compatibility blocks conversion, not just a warning.
- Unknown/custom serializer blocks the affected node and preserves the pending
  draft.
- Pending UI drafts never enter recipe, identity, capability-ready, or publish
  paths.
- Any second object_info fetch in one reanalysis is a regression.
- Any API import regression is a blocker.
- Any runtime package contract change is out of scope.
- Any new execution/queue/task authority is out of scope.
- Do not weaken tests to make a fixture pass.

## Final plan decision

~~~text
BASE_HEAD=0e3080ebaba8f2f787fbcbab44574d13c9da6a98
PHASE2B_DESIGN_STATUS=APPROVED
PHASE2B_DESIGN_REVISION=V2

DESIGN_DOC_CLEANUP=PASS
IMPLEMENTATION_PLAN_FILE=docs/superpowers/plans/2026-09-20-workflow-recognition-v2-phase-2b-implementation.md

GOLDEN_FIXTURE_FIRST_GATE=BLOCKING_IMPLEMENTATION_GATE_BEFORE_NORMALIZER
GOLDEN_FIXTURE_REQUIRED_COUNT=3
GOLDEN_FIXTURE_COLLECTION=NOT_EXECUTED_IN_PLAN_AUTHORING
GOLDEN_FIXTURE_COLLECTION_BLOCKER=REAL_LOCAL_PAIRED_EXPORTS_AND_EXACT_VERSION_METADATA_REQUIRED_BEFORE_IMPLEMENTATION

KERA2_PAIR_PLAN=REAL_LOCAL_UI_API_PAIR_WITH_OBJECT_INFO_AND_PROMPT_SEED_WIDTH_HEIGHT_STEPS_IMAGE_OUTPUT
H3_FAST_PAIR_PLAN=REAL_LOCAL_UI_API_PAIR_WITH_OBJECT_INFO_AND_PROMPT_SEED_DIMENSIONS_DURATION_FPS_VIDEO_OUTPUT
H3_REFERENCE_PAIR_PLAN=REAL_LOCAL_UI_API_PAIR_WITH_OBJECT_INFO_AND_ACTUAL_REFERENCE_OR_FIRST_LAST_MEDIA_SLOTS

SUPPORTED_COMPATIBILITY_SET=EXACT_GOLDEN_PROVEN_WORKFLOW_FORMAT_FRONTEND_SCHEMA_AND_POLICY_TUPLES
COMPATIBILITY_CONTEXT_PLAN=EXPLICIT_WORKFLOW_FORMAT_FRONTEND_SCHEMA_FINGERPRINT_AND_NORMALIZER_POLICY_CONTEXT
SUPPORTED_VERSION_POLICY=ONLY_EXACT_VERSIONS_RECORDED_BY_REAL_GOLDEN_PAIRS; UNKNOWN_FAILS_CLOSED
UI_DOCUMENT_MODEL_PLAN=PURE_MINIMAL_UI_ROOT_NODE_LINK_WIDGET_AND_MODE_MODEL
SCHEMA_DESCRIPTOR_PLAN=PURE_RECOGNITION_SCHEMA_CONTEXT_TO_UI_SERIALIZATION_DESCRIPTOR_ADAPTER
LINK_NORMALIZER_PLAN=SOCKET_AND_TOP_LEVEL_TUPLE_CROSS_CHECK_BEFORE_WIDGET_MERGE
WIDGET_NORMALIZER_PLAN=NAMED_VALUES_FIRST_THEN_VERIFIED_POSITIONAL_DESCRIPTOR_WITH_CURSOR_MISMATCH_FAILURE
NODE_NORMALIZER_PLAN=VERIFIED_CLASS_IDENTITY_LINK_LITERAL_MERGE_OUTPUT_SELECTION_AND_MODE_GATES
CUSTOM_SERIALIZER_POLICY=STANDARD_SERIALIZATION_PROVEN_ONLY; UNKNOWN_OR_CUSTOM_BLOCKS
BYPASS_POLICY=FAIL_CLOSED_UNLESS_GOLDEN_AND_DETERMINISTIC_RULE_PROVE_CONVERSION
VIRTUAL_NODE_POLICY=FAIL_CLOSED_UNLESS_GOLDEN_AND_DETERMINISTIC_RULE_PROVE_CONVERSION
SUBGRAPH_POLICY=FAIL_CLOSED_UNLESS_GOLDEN_AND_DETERMINISTIC_RULE_PROVE_CONVERSION

PENDING_DRAFT_PLAN=ADDITIVE_IN_MEMORY_SOURCE_AND_NORMALIZATION_STATE_EXTENSION
PENDING_WITHOUT_API_DOCUMENT=SUPPORTED
OBJECT_INFO_SINGLE_FETCH=REQUIRED
REANALYSIS_PLAN=SAME_DRAFT_ID_PICKER_FREE_NORMALIZE_VALIDATE_RECOGNIZE_CAPABILITY
OBJECT_INFO_SINGLE_FETCH_PLAN=ONE_FETCH_SHARED_BY_SCHEMA_ADAPTER_NORMALIZER_ANALYSIS_AND_CAPABILITY
IDENTITY_PLAN=RAW_SHA_PENDING; SEMANTIC_AND_STRUCTURAL_SHA_ONLY_AFTER_NORMALIZED_API_READY
DUPLICATE_CONVERGENCE_PLAN=EQUIVALENT_UI_API_NORMALIZED_GRAPHS_USE_EXISTING_EXACT_SEMANTIC_PATH
RECOGNITION_INTEGRATION_PLAN=RECOGNITION_V2_CONSUMES_NORMALIZED_API_ONLY
CAPABILITY_INTEGRATION_PLAN=CAPABILITY_USES_NORMALIZED_API_AND_SAME_SCHEMA_SNAPSHOT
FRONTEND_PLAN=MINIMAL_PENDING_BLOCKED_READY_AND_DIAGNOSTIC_STATES_WITH_EXISTING_TYPED_TRANSPORT
API_EXPORT_FALLBACK_PLAN=RETAIN_EXPORT_API_FORMAT_FOR_UNSUPPORTED_COMPATIBILITY_SERIALIZER_MODE_OR_SCHEMA
NEGATIVE_FIXTURE_PLAN=ALL_REQUIRED_SERIALIZER_MODE_VERSION_LINK_AND_OUTPUT_NEGATIVES_FAIL_CLOSED
GOLDEN_EQUALITY_PLAN=NORMALIZED_API_GRAPH_INPUTS_LINKS_OUTPUTS_HASHES_AND_RECOGNITION_BINDINGS
OFFLINE_RESUME_TEST_PLAN=SELECT_ONCE_PENDING_OFFLINE_START_COMPATIBLE_COMFY_REANALYZE_SAME_DRAFT
REAL_UI_SMOKE_PLAN=TAURI_NATIVE_PICKER_LIVE_COMFY_KERA2_H3_FAST_H3_REFERENCE_WITHOUT_GENERATION
API_IMPORT_UNCHANGED=YES
RUNTIME_PACKAGE_CONTRACT_CHANGED=NO
DATABASE_MIGRATION=NONE
NEW_EXECUTION_AUTHORITY=NO

SOURCE_CODE_CHANGED=NO
WORKTREE_STATUS=MODIFIED_ONLY_BY_PHASE2B_DOCS
COMMIT=NONE
PUSH=NONE
RELEASE=NONE
NEW_ARCHITECTURE_BLOCKER=NONE

PHASE2B_IMPLEMENTATION_PLAN_STATUS=APPROVED_WITH_MANDATORY_AMENDMENTS
READY_FOR_IMPLEMENTATION=YES_AFTER_AMENDMENTS
PHASE2B_IMPLEMENTATION_STATUS=NOT_STARTED
~~~
