# M1 MiniMax H3 16 GB Runtime Validation

Date: 2026-08-09

Scope: unblock the user-supplied MiniMax H3 reference-to-video workflow on an
NVIDIA GeForce RTX 5060 Ti 16 GB, publish a bounded runtime package through the
existing onboarding path, complete a real AI Studio generation, import the
video into Asset Library, and verify native desktop playback. No database
migration, model installer, custom-node installer, or MiniMax-specific core
execution branch was added.

## Final gate

- `MINIMAX_H3_RUNTIME_INPUT = READY`
- `MINIMAX_H3_16GB_RUNTIME = PASS`
- `MINIMAX_H3_VIDEO_ASSET = PASS`
- `MINIMAX_H3_DESKTOP_PLAYBACK = PASS`

The previous `ENVIRONMENT BLOCKED` result is superseded. A real five-second H3
video completed through AI Studio on the target 16 GB GPU and became a playable
project asset.

## Root cause

The original graph had three independent problems that amplified or obscured
the OOM:

1. H3 `width` remained a 1344 literal, `ResolutionSelector.width` was linked to
   H3 `height`, and `ResolutionSelector.height` was linked to H3 `length`.
2. The intended duration expression was not connected to H3 `length` at all.
   The nominal ten-second control therefore did not bound the actual frame
   count.
3. The original pruned INT8 UNet filename was no longer available in the live
   ComfyUI model list, while the installed pruned NVFP4 variant was available.

The initial reduced direct probe preserved the bad links and still completed,
but `ffprobe` proved that it produced 430 frames at 1344 x 256 and 17.917
seconds instead of the intended short probe. That result confirmed the NVFP4
memory reduction while also exposing the incorrect graph semantics.

The final graph uses these links:

- `ResolutionSelector.width -> MiniMaxH3ReferenceToVideo.width`
- `ResolutionSelector.height -> MiniMaxH3ReferenceToVideo.height`
- `ComfyMathExpression.INT -> MiniMaxH3ReferenceToVideo.length`
- `duration_seconds -> PrimitiveFloat -> ComfyMathExpression`

The expression's INT output is index 1. An intermediate package linked index 0
(FLOAT); ComfyUI correctly rejected it before execution, and AI Studio
persisted the failed validation task. The immutable package was superseded
rather than edited in place.

## Final 16 GB package

- Workflow: `wfl_minimax_h3_reference_video`
- Version: `1.1.2`
- Package: `minimax_h3_reference_video_1_1_2_0385e8c5`
- Workflow SHA-256:
  `0385e8c53ae005444ae8d12d72145c3c24b681e6fb93f9ba896be9c675a5020a`
- Graph: 22 nodes, 21 unique classes
- Capability: `READY`, no issues
- Onboarding validation: API, Recipe, bindings, output, manifest, capability,
  and dry run all PASS
- UNet: installed pruned NVFP4 variant
- Attention: MiniMax H3 memory-efficient SageAttention patch
- Sampling: installed H3 Turbo 4-Step LoRA, four scheduler steps
- Resolution: 0.1 MP, 9:16, multiple of 32
- Duration Recipe range: 1–5 seconds; five seconds is the live-validated upper
  bound for this profile
- Output prefix: `video/MiniMax_H3_16GB`

The runtime package remains in the local workflow library. Raw user workflow
bytes and model files remain ignored by Git and are not bundled in the app.

## Real AI Studio gate

- Endpoint: `http://127.0.0.1:8188`
- ComfyUI: `0.30.2`
- GPU: NVIDIA GeForce RTX 5060 Ti
- Reported VRAM: 17,102,864,384 bytes (approximately 15.9 GiB)
- Final task: `tsk_e815637d-04f5-4155-a827-9a038e04b117`
- Prompt queue number: 4
- Task result: `SUCCEEDED`
- Warm execution time: approximately 56 seconds
- Peak observed by `nvidia-smi` during a five-second run: approximately
  14,933 MiB used and 1,118 MiB free
- Output asset: `ast_bff9b28c-1879-4699-8904-0ef2c4d0bb46`

The 1.1.1 candidate also completed before the duration cap was tightened. The
final 1.1.2 package was run again so the active workflow version itself has
successful-run evidence.

## Video and Asset Library gate

- File: MP4, H.264 video plus AAC stereo audio
- Dimensions: 256 x 416
- Frame rate: 24 fps
- Frames: 124
- Duration: 5.167 seconds
- File size: 237,867 bytes
- Persisted SHA-256 matched the stored file
- Project-scoped Task-to-Asset mapping: PASS
- Generated-video Asset Library filter: PASS
- Thumbnail creation and visual inspection: PASS
- Decoded frame count: 124
- Unique decoded frame hashes: 124, confirming a non-static video

## Windows playback correction

The valid MP4 initially failed in WebView2 because the frontend built a
macOS/Linux-style `aistudio-media://localhost/...` URL. Wry maps registered
custom protocols to `http://<scheme>.localhost/...` on Windows. The URL builder
now selects the Windows form from the user agent and retains the custom-scheme
form on other platforms.

Native Asset Preview verification after the fix:

- URL: Windows Wry custom-protocol form
- `readyState`: 4
- Browser duration: 5.167 seconds
- Browser dimensions: 256 x 416
- Media error: none
- Playback clock advanced from 0 to 0.860 seconds during the automated check

## Regression checks

- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test -- --test-threads=1`: PASS — 238 passed, 0 failed
- `pnpm test`: PASS — 13 files, 32 tests passed
- `pnpm build`: PASS
- Windows custom-protocol URL unit test: PASS
- `git diff --check`: PASS

## Operating constraints

- Treat 0.1 MP, at most five seconds, and one active H3 task as hard limits on
  this 16 GB profile.
- Do not increase H3 resolution, duration, or concurrency automatically. The
  observed five-second run had roughly 1.1 GB of remaining physical VRAM at
  peak.
- Only the five-second upper bound was run in this gate. Shorter Recipe values
  are bounded but were not individually quality-validated.
- Higher-resolution or longer H3 production requires a separate measured
  profile, more VRAM, or additional model/runtime optimization.

This gate stops after the 16 GB H3 completion and playback result. No third
model runtime pack is entered here.
