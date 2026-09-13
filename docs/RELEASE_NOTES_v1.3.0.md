# AI Studio v1.3.0

AI Studio 1.3.0 is a wayfinding and continuity release. It helps producers
understand where they are, what to do next, and how to return to a selected
result without changing the existing production safety boundary.

## What changed

- The first project screen now explains the path from creating a project to
  importing a Production Handoff, preparing Shots, starting production, and
  reviewing results.
- Production Handoff is easier to find and still uses the existing preview
  and explicit confirmation flow.
- Large project views keep navigation bounded while preserving exact
  project-scoped Shot, Task, Asset, Review, and batch targets.
- Queue, Monitor, Review, and Rework surfaces now provide clearer continuity
  and state language.
- The Project Command Center exposes the selected final result with direct
  Shot and Asset links, plus the existing file-location path when available.

## Production Flow

```text
Prepare
  ↓
Queue
  ↓
Start
  ↓
Monitor
  ↓
Review
  ↓
Rework
```

## Safety

```text
Prepare does not start generation.

Rework does not start generation.

Queue Start begins production.
```

Existing project isolation, historical task/production references, and the
current database and backup compatibility contracts are preserved. This
release does not add a schema or migration.

## Known Issues

- The frontend production build reports the existing non-blocking warning
  that the main minified chunk exceeds Vite's default 500 kB threshold.
- A Rust cancellation end-to-end test has occasional historical timing
  flakiness; the release Source-only CI gate passed.
- Some advanced diagnostic vocabulary remains technical by design and is
  retained for troubleshooting.

These are non-blocking P2/P3 items; no P0 or P1 release blocker remains.

## Verification

- Frontend: 150 test files and 823 tests passed; TypeScript check and
  production build passed.
- Rust: format, check, and all-target tests passed locally.
- Source-only CI: runs `34743034346` and `34743701927` passed; the tag run
  validated exact head `d69e250102be9254c1237d1db1fbe2d63de0b4db`.
- Installer: portable executable, MSI, and NSIS artifacts were generated and
  published with the accompanying `SHA256SUMS.txt` file.

The detailed release checklist and exact closeout record are in
`docs/RELEASE_CHECKLIST_v1.3.0.md` and `docs/DEV_115_RELEASE_CLOSEOUT.md`.
