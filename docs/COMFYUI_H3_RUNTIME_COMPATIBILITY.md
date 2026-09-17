# ComfyUI MiniMax H3 Runtime Compatibility

## Verified incident

On the local ComfyUI 0.35.0 runtime, `NBH3HyperStepSimple` from
`ComfyUI-NB-H3-HyperStep` 0.5.1 failed in `FinalLayer.forward`: the custom node
called the current H3 final layer with four arguments, while ComfyUI required
`sigma`, `sample_sigmas`, and `shifts` as well. ComfyUI history reported an
execution error with no outputs. The elapsed-time and closing-WebSocket log
messages are not success signals and are not the root cause.

## Validated workaround

The embedded `MiniMax H3 兼容版（无 HyperStep）` package is a separate immutable
workflow identity. It removes only the HyperStep node and reconnects its two
model consumers to the exact upstream model. The original package, workflow,
plugin, model, sampler, and user data remain unchanged. The standard production
queue and Queue Start remain the only execution path.

This fallback was exercised against the local runtime and produced a 5.167 s
H.264/AAC video at 960×544, 24 fps. This is evidence for this machine/model
configuration, not a compatibility guarantee for other ComfyUI installations.
The video and test record are kept outside the repository at
`C:\Users\ADMIN\Desktop\AI-Studio-Comfy-Compatibility-Verification`.

## Failure handling

- Only the observed `FinalLayer.forward()` missing-three-arguments signature is
  classified as `COMFY_NODE_INCOMPATIBLE`; unrelated `EXECUTION_ERROR`s retain
  the ordinary continue-on-failure behavior.
- A confirmed incompatibility pauses the affected batch, including after
  startup recovery. Already-failed evidence remains untouched; later items stay
  pending and do not get fake task IDs or failure records.
- UI identifies zero-success batches as failures, shows the known cause and
  technical details separately, and distinguishes an unexecuted remainder.
- No plugin auto-update, model mutation, guessed arguments, or automatic retry
  occurs. Choosing the fallback requires new production items because an
  existing item retains its exact workflow-version/recipe pair.

## Runtime diagnostics and limits

The ComfyUI preflight reports a session-local fingerprint built from endpoint,
reported ComfyUI version, and the public `/object_info` schema. A changed
fingerprint is a warning to verify the first item. If a HyperStep node is
advertised, preflight warns that its internal Python compatibility is unknown.
Public node metadata cannot establish custom-node code version or prove
internal API compatibility; unknown stays a warning and is never promoted to a
false incompatibility block. A matching observed runtime exception pauses the
queue after the first affected execution.

Preflight health remains separate from image upload and is not an upload gate.
The batch failure policy continues to allow independent item errors to proceed;
only this confirmed shared-runtime/API incompatibility stops that batch.
