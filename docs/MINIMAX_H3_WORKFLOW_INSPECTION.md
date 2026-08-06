# MiniMax H3 Workflow Inspection

Date: 2026-08-06

## Input integrity

- `MINIMAX_H3_RUNTIME_INPUT = READY`
- API workflow format: PASS
- SHA-256: `a1053180b2e703fc9ae60163675a5cd29c3ed86b560cc2a580468e9ae2cf4bde`
- Top-level node count: 21
- Unique class count: 20
- The raw bytes were copied unchanged into the local runtime-import area and are ignored by Git. The full workflow is not committed.

The root is a numeric node map. Every node has an `inputs` object and a
`class_type`; no visual-workflow conversion was performed.

## Graph classes

The inspected graph contains these 20 unique classes:

`BasicGuider`, `BasicScheduler`, `CLIPLoader`, `ComfyMathExpression`,
`CreateVideo`, `KSamplerSelect`, `LoadImage`,
`MiniMaxH3MemoryEfficientSageAttentionPatch`, `MiniMaxH3ReferenceToVideo`,
`NBH3HyperStepSimple`, `PrimitiveFloat`, `PrimitiveStringMultiline`,
`RandomNoise`, `ResolutionSelector`, `SamplerCustomAdvanced`, `SaveVideo`,
`UNETLoader`, `VAEDecode`, `VAEDecodeAudio`, `VAELoader`.

## Capability

Local ComfyUI was online during inspection:

- Endpoint: `http://127.0.0.1:8188`
- Version: `0.30.1`
- `/object_info` classes reported: 4,486
- Required workflow classes present: 21 of 21
- Capability result: `READY`
- Critical diagnostics: none

No raw `/object_info` response or full prompt was exposed to the frontend or
recorded in this report.

## Detected mode

The graph is a MiniMax H3 reference-image-to-video workflow. The reference
image is connected to the H3 reference input, and the terminal output is a
video output. The graph does not prove a text-to-video, first-frame-only,
first-plus-last-frame, reference-video, or reference-audio mode, so those
modes were not invented or enabled.

## Input candidates

| Candidate | Graph evidence | Result |
| --- | --- | --- |
| `prompt` | H3 reference node text input | Supported as a required recipe input; no prompt value is recorded here |
| `width` | H3 reference node, existing literal 1344 | Supported; capability range is preserved |
| `height` | H3 reference node linked from another node | Link preserved; not surfaced as a direct mapping because linked inputs are not bindable in the existing onboarding contract |
| `length` | H3 reference node linked from another node | Link preserved; not guessed or rewritten |
| `seed` | Random noise node integer input | Supported with ComfyUI's bounded integer range; no widened or model-specific rule was added |
| `reference_image` | One `LoadImage` output connected to the H3 reference-image list | Supported as one static reference-image slot |
| `ref_image_size` | H3 reference node literal `match` | Preserved as workflow configuration, not promoted to a new UI field |

The graph has one static reference-image slot. No dynamic list/autogrow
binding was required, and no prompt parser or token substitution was added.

## Output candidates

- The terminal `SaveVideo` node is recognized through the generic output
  candidate path.
- The recipe output is `generated_video` / `video`.
- No image, audio, or poster output was claimed from this inspection alone.
