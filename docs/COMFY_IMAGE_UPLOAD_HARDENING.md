# ComfyUI Image Upload Hardening

## Root cause

The upload path had two independent failure amplifiers:

1. `GenerationInputPreparer` called `health_check()` before the first media
   upload. A slow or temporarily unresponsive health endpoint could therefore
   reject an otherwise usable `/upload/image` endpoint before any upload was
   attempted.
2. Small images used replayable multipart bytes, but images over 16 MiB were
   converted to a one-shot stream. A connection reset or response timeout
   could not safely rebuild the multipart request for those images.

Increasing the upload timeout alone does not solve either failure mode. It
only makes a stalled upload wait longer.

## Implemented boundary

- `GenerationInputPreparer` no longer uses health/system-stat endpoints as an
  upload admission gate. The upload endpoint is authoritative for the upload
  operation; health remains available to the UI/status paths.
- Every `ComfyImageUpload` is a replayable byte source, including images over
  16 MiB. Each retry constructs a new `Part` and `Form`, preserves the exact
  upload filename, and keeps `overwrite=false`.
- The upload client keeps `no_proxy()` and
  `pool_max_idle_per_host(0)`, uses a dedicated 180-second request timeout,
  and now has an explicit 10-second connect timeout.
- Image upload copies first enforce a 2048px longest edge, then repeatedly
  downscale proportionally until the encoded bytes fit within 16 MiB. PNG is
  encoded as PNG, so alpha is not discarded; the Asset Library original is
  never changed.
- Retries cover timeout/offline/connection failures and HTTP 408, 429, 502,
  503, and 504. Numeric or RFC 2822 `Retry-After` values are honored for
  HTTP 429. HTTP 413 and other 4xx responses are terminal.

## Observability

Upload events are structured with task/asset identity when the caller has an
exact relationship, filename, source bytes, upload bytes, dimensions, attempt,
attempt/total elapsed time, HTTP status, and error class. Image preprocessing
also records its own decode/resize/encode duration before the HTTP request
timer starts. Phases include:

```text
preflight -> request_start -> response_received -> parse_response -> retry
```

The `preflight` event records local request preparation; it does not perform a
health probe or block the upload. Reqwest does not expose reliable separate
connect-versus-body-upload timings through this adapter, so the logs do not
claim to measure either phase independently.

## Files changed

- `src-tauri/src/application/ports/comfy_adapter.rs` — exact upload context
  metadata at the typed adapter boundary.
- `src-tauri/src/application/ports/mod.rs` — context re-export.
- `src-tauri/src/application/generation_input_preparer.rs` — remove the hard
  health gate, attach upload context, and enforce encoded-size-aware image
  preprocessing.
- `src-tauri/src/infrastructure/comfy/client.rs` — replayable multipart
  uploads, timeout/connect configuration, retry classification, and logs.

## Regression coverage

- replayable small image upload
- replayable image over 16 MiB after a disconnect
- first timeout followed by success
- first connection failure followed by success
- HTTP 408/429/502/503/504 followed by success
- HTTP 413 and ordinary 4xx are not retried
- delayed response after the request body is received
- upload succeeds when the health endpoint is unavailable
- 2048px image whose encoded bytes exceed 16 MiB is reduced
- PNG transparency is retained in the upload copy
- server-returned upload identity remains the value consumed by the existing
  generation snapshot/workflow path
