# M1 MiniMax H3 Runtime Validation

Date: 2026-08-06

Scope: the user-supplied MiniMax H3 API workflow, onboarding, capability
validation, package publication, and one real reference-image runtime attempt.
No MiniMax-specific core branch, database migration, model installer, or
custom-node installer was added.

## Final gate

- `MINIMAX_H3_RUNTIME_INPUT = READY`
- `MINIMAX H3 RUNTIME = ENVIRONMENT BLOCKED`
- Blocking reason: the real ComfyUI execution reached the sampler and failed
  with `torch.OutOfMemoryError` on the available GPU.

The runtime status is not reported as PASS because no real H3 mode completed
with a video output. This is not an input-format, capability, or mapping
failure.

## Workflow and package

- API-format validation: PASS
- Raw workflow SHA-256: `a1053180b2e703fc9ae60163675a5cd29c3ed86b560cc2a580468e9ae2cf4bde`
- Graph: 21 nodes, 20 unique classes
- Detected mode: reference image to video
- Capability recheck: PASS (`READY`)
- Atomic onboarding publication: PASS
- Published package identity: `minimax_h3_reference_video_1_0_0_a1053180`
- Raw workflow bytes: preserved locally and ignored by Git; no full real
  workflow is committed

The existing onboarding flow was used end to end: API inspection, local
capability lookup, generic input/output mapping, dry-run validation, and
atomic package publication. Height and length links were preserved because
the current onboarding contract does not bind linked inputs directly.

## Mapping gate

| Contract field | Result | Evidence |
| --- | --- | --- |
| Prompt | PASS | Required H3 text input mapped generically; no prompt content is stored in this document |
| Width | PASS | Existing graph value and `/object_info` bounds accepted |
| Height | PRESERVED | Existing graph link retained; no fabricated direct input |
| Length | PRESERVED | Existing graph link retained; no fabricated direct input |
| Seed | PASS | Integer seed mapping uses the capability-defined range |
| Reference image | PASS | One imported project asset maps to the connected reference-image slot |
| First frame / last frame | NOT APPLICABLE | Not evidenced by this graph |
| Reference video / audio | NOT APPLICABLE | Not evidenced by this graph |
| Video output | PASS | Generic video output candidate maps the terminal SaveVideo result |

## Real ComfyUI gate

Observed before execution:

- Endpoint: `http://127.0.0.1:8188`
- Version: `0.30.1`
- GPU: `NVIDIA GeForce RTX 5060 Ti` (`cuda:0`)
- VRAM: approximately 15.9 GB total and 14.7 GB free at inspection time
- `/object_info` node count: 4,486

The published recipe was executed through the existing
`GenerationService`/ComfyAdapter path with a project-scoped reference asset.
ComfyUI accepted the prompt and began execution. The task then failed at the
sampler with a GPU out-of-memory error. AI Studio persisted the terminal
`FAILED` state and the raw error payload; no output asset was created.

## Output and lifecycle gates

- Task lifecycle: PASS for queue, execution, and persisted failure handling
- Video asset: NOT CREATED because the runtime failed before output
- Audio track: NOT VERIFIED
- Poster: NOT VERIFIED
- Local playback: NOT VERIFIED for this failed task
- Cancel: NOT RUN because the task reached a terminal error before a useful
  cancellation window
- Offline playback: NOT RUN in this H3 gate; existing media protocol tests
  remain passing
- Preset/project isolation: existing automated isolation coverage PASS; H3
  native UI isolation gate NOT RUN
- Workflow export: existing generic export coverage PASS; H3-specific export
  gate NOT RUN
- Diagnostics: package hash, registration, and capability checks PASS; live
  successful-run evidence is correctly absent

The native Workflows wizard interaction was not claimed as a pass because the
local desktop WebView automation adapter could not provide stable geometry for
the required interaction. The desktop app itself remained usable and showed
the connected ComfyUI status during the live check.

## Regression checks

- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test -- --test-threads=1`: PASS — 231 passed, 0 failed
- `pnpm test`: PASS — 9 files, 21 tests passed
- `pnpm build`: PASS
- `pnpm tauri dev`: ATTEMPTED; desktop process reached the connected runtime
  state, native wizard interaction remains pending
- `git diff --check`: PASS before final publication

## Technical debt

- The current graph exceeds the available GPU memory at its existing runtime
  settings; the workflow was not altered to hide this environment limitation.
- Height and length remain graph-linked inputs and are not direct recipe fields
  until the generic onboarding contract supports safe linked-input controls.
- Native desktop acceptance still needs a stable manual or WebView automation
  pass.
- Successful video asset import, playback, poster extraction, and optional
  audio verification require a runtime environment that can finish the H3
  graph.

## Next stage

Only `MODEL RUNTIME PACK 02 — Wan / Flux / Qwen` is recommended next. This
change stops here.
