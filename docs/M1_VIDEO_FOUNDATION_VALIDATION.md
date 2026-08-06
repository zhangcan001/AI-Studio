# M1 Video Foundation Validation

Date: 2026-08-06
Baseline commit: `2befd4f69d409ace4baae21eeed83792628f7038`

## Scope

This gate adds generic video output support to the existing image generation
pipeline. It does not add source video/audio inputs, video/audio uploads,
reference video/audio conditioning, or a MiniMax-specific workflow branch.

## Protocol

- ComfyUI history normalization reads `outputs[node].images` as the generic
  SavedResult collection used by SaveVideo and PreviewVideo nodes.
- Unknown fields are ignored; the complete `object_info` response is never
  returned to React.
- A `video` Recipe output selects video handling. History JSON does not decide
  whether an output is an image or video.

## Persistence and storage

- `AssetType::Video`, category `generated_video`, optional width/height and
  `duration_ms` are persisted in the new `004_video_outputs.sql` migration.
- `task_output_assets` is inserted in the same SQLite transaction as the
  generated `assets` rows.
- New task queries and history details read mapped outputs in Recipe output
  order and ordinal order. Legacy tasks continue to use the
  `source_task_id` fallback without backfill.
- New video files are stored under
  `<project root>/assets/generated/video/ast_<uuid>.<ext>`.
- Network chunks are hashed and written to an `.tmp` file before flush, sync,
  and rename. The full video is never represented as one IPC `Vec<u8>`.
- ffprobe and ffmpeg are optional PATH tools. Missing tools keep generation
  successful and leave optional metadata/poster fields empty.

## Playback

React receives only
`aistudio-media://localhost/video?projectId=...&assetId=...`. The Tauri 2
custom protocol checks project ownership and the asset type, supports HEAD and
single byte ranges, rejects multi-range/invalid requests, and caps one body at
8 MiB. It uses bounded file range reads, so seeking remains local-first and
works while ComfyUI is offline.

## Automated evidence

- Rust covers SavedResult normalization, video output collection, bounded
  128 MiB chunked import, SHA-256, interrupted-stream cleanup, HTML/MIME
  rejection, atomic mapping commit/rollback, mapped-output recovery,
  Range parsing, and protocol stream download.
- Frontend has 14 passing unit tests, including logical media URL construction,
  while the production build covers the video card, poster fallback, safe
  player, Task Output, Task History, and project-scoped Asset Library paths.
- Test fixtures under `src-tauri/tests/fixtures/comfy_history/` are synthetic
  protocol fixtures only and are not installed into the runtime workflow
  library.

## Live gate status

The repository currently contains no validated user-supplied generic video or
MiniMax H3 runtime package. Therefore Generic Video Live and MiniMax H3 Live
are reported as NOT RUN rather than simulated. The T2I rerun was attempted,
but ComfyUI port 8188 was offline and the local process launch was denied by
Windows with Win32 error 5, so no new live task ID or output is claimed here.

## Gate result

`M1 VIDEO FOUNDATION = PASS`
Generic Video Live = NOT RUN
MiniMax H3 Live = NOT RUN

The next recommendation is MEDIA INPUT PACK only; it is not included in this
change.
