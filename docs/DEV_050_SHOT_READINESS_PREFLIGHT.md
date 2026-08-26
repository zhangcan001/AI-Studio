# DEV-050 — Shot Readiness + ComfyUI Preflight

## Baseline

- `DEV050_START_SHA`: `1ae47a904fd1d8e677b833a41cc95290f581292b`
- Branch: `master`
- Baseline was clean and matched `origin/master`.
- DEV-049 targeted regression was run before DEV-050 edits.

## DEV-049 regression and selected-input patch

`dev049_context_resolver` remains the regression gate for the five-level
resolver, prompt builder, legacy fallback, bounded batch resolution, and
context hash behavior.

`ResolvedStageInput` is now part of `ResolvedShotContext`:

- Image stage always has no selected generation image input.
- Video stage validates `ShotRecord.selected_image_asset_id` through the
  existing bulk asset lookup.
- Missing, cross-project, and non-image selections emit
  `CONTEXT_SELECTED_IMAGE_NOT_FOUND`,
  `CONTEXT_SELECTED_IMAGE_PROJECT_MISMATCH`, or
  `CONTEXT_SELECTED_IMAGE_TYPE_INVALID` diagnostics with error severity.
- A valid video selection contributes both asset id and SHA-256 to
  `ContextHashInput`; changing the selected image changes the video hash.
- Image-stage hashes do not include the selected output image.

## Readiness model

The backend evaluates exactly seven gates:

1. `CHARACTER`
2. `SCENE`
3. `REFERENCE`
4. `PROMPT`
5. `WORKFLOW`
6. `OUTPUT`
7. `COMFY_CAPABILITY`

Each gate contains checks with `PASS`, `WARNING`, `INCOMPLETE`, or `BLOCKER`
state. Gate state is the worst check state. Overall status is `READY` when
there are no incomplete or blocker gates, `INCOMPLETE` when there is no blocker
but at least one incomplete gate, and `BLOCKED` when any blocker exists.
Warnings do not prevent `READY`.

The score starts at 100 and is reduced once per gate: warning `-5`, incomplete
`-15`, blocker `-35`, clamped to `0..100`. A partial resolver context always
adds the `PROMPT / CONTEXT_PARTIAL` blocker and can never be `READY`.

Character-less shots pass when there is no character intent. Scene profiles or
usable legacy scene context pass the scene gate; missing scene semantics are
incomplete. Reference errors, costume mismatches, and invalid assets are
blockers; an empty optional reference set is not a blocker. Empty prompt is
incomplete and missing profile revision is a warning.

## ComfyUI integration

Readiness reuses the existing `ComfyPreflightService` and its current endpoint
configuration. Cached readiness calls `cached_current()` and never refreshes
ComfyUI. Live preflight calls `current()` once per request or batch, only for
connection, capability, node, runtime, and workflow readiness checks.

Offline/incompatible connections, unavailable capabilities, and missing nodes
for the selected workflow are blockers. Runtime busy is a warning. A warning
from an unrelated workflow remains a warning when the selected workflow is
ready. No prompt is submitted, no task or queue item is created, and no image
or video generation occurs.

## Workflow matching

The selected `workflow_version_id` and `recipe_id` are matched exactly against
the fast `WorkflowLifecycleService::list_workspace()` result. Missing version
or recipe selection is incomplete. Missing, invalid, archived, disabled, or
blocked selected workflow state is a blocker; degraded or unchecked readiness
is a warning. Recipe names are never fuzzy-matched.

Structured workflow `mode` and `category` metadata determine stage
compatibility. Unknown mode is a warning rather than a guessed blocker.

## Video rules

- `I2V` video shots require a valid selected image keyframe; otherwise
  `VIDEO_KEYFRAME_REQUIRED` is incomplete.
- `REF2VA` video shots require at least two resolved reference images.
  Structured recipe/workflow metadata is consulted for a maximum; exceeding it
  is a blocker, and no arbitrary maximum is hard-coded.

## Batch and scene strategy

Single and batch APIs resolve contexts through `ShotContextResolver`. Batch
requests are capped at 500 shots, use one resolver batch, one workspace read,
and (for live preflight) one ComfyUI `current()` call. Evaluation after the
snapshot is pure and does not scale ComfyUI refreshes with shot count.

Scene APIs load the scene assignment once, preserve assignment order, resolve
the assigned shots as one batch, and return compact items containing shot id,
ordinal, name, status, score, warning/incomplete/blocker counts, and context
hash. Full prompt, reference asset, and source-trace payloads are not repeated
in the scene summary.

## Read-only commands

- `shot_readiness_cached`
- `shot_preflight`
- `scene_readiness_cached`
- `scene_preflight`

Requests and responses use camelCase serialization at the Tauri boundary.
Commands only resolve, inspect, and (for live preflight) refresh ComfyUI
capabilities.

## No-generation guarantee

DEV-050 does not add an engine, provider, executor, queue, task submission,
candidate selection, output download, or workflow compilation path. It does
not modify `GenerationService`, production queue/orchestrator semantics,
review behavior, or ComfyUI prompt submission. Real GPU/ComfyUI generation is
not required; live capability tests use mocks/snapshots.

## Multi-agent evidence

The implementation used Main plus four parallel agents with disjoint ownership:

- Agent A: readiness domain and pure evaluator.
- Agent B: DEV-049 selected video input and resolver regressions.
- Agent C: readiness service and ComfyUI bridge.
- Agent D: read-only commands and DEV-050 evaluator/contract tests.

Agents did not create nested agents, commit, or push. Main performed module
registration, AppState wiring, command registration, documentation, final
verification, commit, and push.

## Compatibility

- Product version remains `0.6.2`.
- Database migrations stop at `023`; migration `024` is intentionally absent.
- Backup format remains `12`.
- Manifest version remains `1`.
- No frontend `src/**` files are changed.

## Verification

The final verification wave passed Rust formatting, `cargo check`, the complete
Rust test suite, frontend tests, frontend build, and `git diff --check`. The
DEV-049 targeted suite passed 18 tests and the DEV-050 targeted suite passed 15
tests. The complete Rust run passed 621 library tests plus 76 integration tests,
with one ignored test and no failures. Frontend verification passed 80 test
files and 289 tests; `pnpm build` also passed. No standard test requires a
running ComfyUI instance.

## Deferred

- `DEV-051 — Asset Library 2.0 + Consistency Asset Management`
- `DEV-052 — Production Preparation + Generation Admission`

Readiness is a derived inspection layer. Persistent readiness history, a
preparation executor, generation admission, and UI work remain outside
DEV-050.
