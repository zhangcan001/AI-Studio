# AI Studio 2.1.0 Personal Edition — Release Candidate

This document does not announce a Git tag or GitHub Release.

Public stable release: **v2.0.0-personal-r4**. Development candidate:
**2.1.0-personal**. M1–M4 product work is complete and frozen; publication is
NOT YET PUBLISHED and requires a separate Publication Gate.

## M1 — Readiness and setup

Create readiness issues have actionable routing to the correct input, generator
or runtime destination. Overview presents connection/runtime readiness without
conflating unknown and ready. Settings explains runtime setup and applied versus
draft endpoints; Tool Hub distinguishes metadata registration from ComfyUI
connection. No automatic installation, discovery or tool start/stop is added.

## M2 — Recovery and precise reuse

Runs distinguishes retrying the original snapshot from editing inputs for a new
run; old results are preserved. Create ↔ Library reuses exact scoped resources
and explicitly selected historical versions. Prompt provenance is typed and
validated against the selected immutable text; manual prompt editing clears
provenance instead of rematching text. Queue Start remains the sole production
execution authority.

## M3 — Findability and bounded browsing

Server-backed Library search, favorite/tag filters, 30-item keyset pages and
visible-only managed thumbnails replace cumulative page replay. Across 20
pages, measured list calls decreased from 210 to 20. Existing complete task
history is reachable from Runs; **complete mixed Runs history pagination is not
included**. No 50k/100k scale guarantee is made.

## M4 — Readonly managed-media integrity

Explicit single-media and project-scoped checks use streaming SHA-256 and
independent boundary, existence, readability, checksum and preview facts.
Cancellation occurs between assets; verification performs zero database/media
writes. Unknown or unavailable preview does not mean corruption. Relink,
replacement and automatic repair remain DEFERRED.

## Compatibility and limitations

- Internal candidate version is 2.1.0-personal; numeric WiX version is 2.1.0.
- Database migration remains 42 (71 formal tables); no migration 043.
- Backup export remains v20; historical reader compatibility is regression
  tested. Restore creates a new project; do not test on a unique user database.
- Windows installer is unsigned. Windows may display a trust warning.
- ComfyUI, models and custom nodes remain external prerequisites.
- No automatic repair/relink, media replacement, general tool auto-discovery,
  automatic tool installation/start/stop, Prompt auto-scoring or usage statistics.
- No mixed Runs full-history pagination, remote telemetry or cloud sync.
- Candidate installation/backup/Native evidence and installer provenance are
  recorded in `AI_STUDIO_2_1_CLOSEOUT.md`; installation build alone is not
  installation acceptance. No installer upload, tag or Release is authorized here.
