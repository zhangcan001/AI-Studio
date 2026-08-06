# M1 Workflow Onboarding Validation

Date: 2026-08-06

Commit: WORKFLOW ONBOARDING PACK release commit (recorded in Git history)

Scope: ONBOARD-01 through ONBOARD-15 only. The next production workflow phase was not implemented.

## 1. API Workflow import and validation

Status: PASS (automated).

- The Workflows workspace is the only surface that exposes node IDs, `class_type`, mappings, capability issues, and output candidates.
- Native picking accepts `.json` only and returns an onboarding draft, never an absolute source path.
- Imports larger than 32 MiB are rejected before JSON parsing with `WORKFLOW_FILE_TOO_LARGE`.
- API format validation rejects visual workflow roots, empty roots, non-numeric node maps, malformed node objects, missing `class_type`, missing `inputs`, and broken numeric links.
- Unknown node fields and `_meta` are tolerated for forward compatibility.
- Literal arrays are not treated as links unless the first item is a numeric source node ID.
- Raw bytes remain immutable, are SHA-256 hashed, and are held by a bounded FIFO registry of 16 drafts.

## 2. Capability and mapping

Status: PASS (automated).

- `/object_info` is converted into safe capability summaries; raw ComfyUI JSON is not returned to React.
- Missing classes are grouped as `MISSING_NODE`.
- Unavailable combo values use `INPUT_OPTION_UNAVAILABLE`.
- Numeric range violations use `INPUT_VALUE_OUT_OF_RANGE`.
- Capability detection is generic and does not contain model-specific branches.
- Text, integer, seed, image, images, video, videos, audio, and audios semantic types are supported.
- Linked inputs are read-only in the wizard with the exact message `由其他节点提供，不能直接修改`.
- Plural and item bindings preserve order; unsupported dynamic/autogrow mappings are reported instead of guessed.
- Output candidates use safe IDs and generic `output_node` metadata.
- Offline import, inspect, and mapping remain available; publishing is disabled until capability checks pass.

## 3. Recipe and manifest

Status: PASS (automated).

- Recipe schema v1 is serialized by `RecipeYamlWriter` and round-tripped through the existing Recipe parser/validator.
- Existing manifest fields and schema are reused; no database migration was changed.
- Seed, integer, and plural bounds are preserved and cannot be widened beyond ComfyUI capability data.
- Existing workflow IDs receive a new semantic version; duplicate SHA-256 imports are rejected.

## 4. Atomic package publish

Status: PASS (automated).

- Publish stages the workflow, manifest, recipe, and SHA-256 metadata in a sibling staging directory.
- Readback, parse, schema validation, dry-run compilation, and capability validation run before publication.
- Final publication uses an atomic rename into a new package directory and refuses overwrite.
- Failure cleanup removes only the package being published and its staging directory.
- Successful publication refreshes the runtime catalog without restarting AI Studio.
- Versioning keeps the same logical workflow ID while producing new workflow/recipe database records.

## 5. UI workflow

Status: PASS (build/static checks); native interaction gate NOT RUN.

Implemented Workflows navigation and the seven-step flow: Inspect, Compatibility, Inputs, Outputs, Metadata, Validate, Publish. Publish remains disabled until all checks pass. After publication, Open in Studio/Test Generation selects the published recipe through the existing catalog bridge. Studio continues to receive safe catalog fields only.

The native desktop interaction gate was not run because the local UI automation adapter could not provide WebView geometry/value input. No native interaction result is claimed as a pass.

## 6. ComfyUI live gate

Status: PASS for connectivity and capability reads.

- Endpoint: `http://127.0.0.1:8188`
- Version: `0.30.1`
- GPU: `NVIDIA GeForce RTX 5060 Ti` (`cuda:0`)
- VRAM: 15.9 GB total, 14.8 GB free
- Node count: 4,486

## 7. Real workflow gates

- Existing T2I regression: NOT RUN. The live ComfyUI gate passed, but native WebView automation could not reliably inject and submit the existing Studio form; no stale result is reused as a new pass.
- Onboarded real T2I: NOT RUN. No separate validated API workflow was available in the approved onboarding locations, and native file-picker interaction was unavailable.
- MiniMax H3 onboarding/live: NOT RUN. The exact permitted library and reference-package checks found no validated MiniMax H3 package; no broad filesystem scan was performed.

## 8. Test results

PASS:

- `cargo fmt --all -- --check`
- `cargo check`
- `cargo test -- --test-threads=1` — 223 passed, 0 failed
- `pnpm test` — 9 files, 21 tests passed
- `pnpm build`
- `git diff --check`

The onboarding test coverage includes API shape errors, broken links, literal arrays, safe inspector summaries, grouped capability errors, mapping ranges and semantic types, recipe round-trip, staging readback, offline publish blocking, atomic publication, FIFO draft retention, duplicate protection, path-summary sanitization, and version increments.

## 9. Technical debt

- Native WebView acceptance still needs a manual or stable desktop automation pass.
- A real second API workflow package is needed for the live onboarding and generation gate.
- Capability rules currently provide generic summaries; future production packs can add explicit user-facing compatibility policies without coupling them to a model.

## 10. Next stage

Only `PRODUCTION WORKFLOW PACK` is recommended next. No later phase is included in this change.
