# AI Studio 2.1.0 Personal Edition

Git tag: `v2.1.0-personal`.
GitHub Release: [published](https://github.com/zhangcan001/AI-Studio/releases/tag/v2.1.0-personal).
Public stable release: **v2.1.0-personal**. Previous validated release:
**v2.0.0-personal-r4**. M1–M4 product work is complete and frozen.
Release tag head: `5af3f20273e722466c82b91ede4970cd83e0bcb8`.
Final branch CI: `37416738731` PASS; tag CI: `37434078940` completed / success.

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

- Product version is 2.1.0-personal; numeric WiX version is 2.1.0.
- Database migration remains 42 (71 formal tables); no migration 043.
- Backup export remains v20; historical reader compatibility is regression
  tested. Restore creates a new project; do not test on a unique user database.
- Windows installer is unsigned. Windows may display a trust warning.
- ComfyUI, models and custom nodes remain external prerequisites.
- No automatic repair/relink, media replacement, general tool auto-discovery,
  automatic tool installation/start/stop, Prompt auto-scoring or usage statistics.
- No mixed Runs full-history pagination, remote telemetry or cloud sync.
- Installed backup/Native acceptance and independent installer provenance are
  recorded in `AI_STUDIO_2_1_CLOSEOUT.md`; installation build alone is not
  installation acceptance. Published NSIS bytes match the accepted installer.


## Backup compatibility

Backup v20 restored asset-version identities remain readable, including
legacy restored `asv_` identities. Existing history is retained without rewriting
user data. This compatibility fix does not change the archive format.
