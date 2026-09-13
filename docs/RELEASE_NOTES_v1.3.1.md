# AI Studio v1.3.1

AI Studio 1.3.1 is a stability and maintenance release. It improves the
reliability of cancellation test synchronization, bounds Source-only CI jobs,
reduces the initial frontend bundle warning, and makes generic task failures
easier to diagnose without changing the product execution model.

## What changed

- Cancellation end-to-end tests now use bounded, scheduler-friendly waits and
  cover repeated cancellation while still asserting interruption, terminal
  state, and registry cleanup.
- Source-only CI keeps the existing gates and adds conservative Rust and
  frontend timeout budgets.
- Existing workspaces are loaded at the existing workspace boundary so the
  initial JavaScript chunk is below Vite's 500 kB warning threshold.
- Failed Studio tasks show localized next-action guidance, expandable technical
  details, and the existing task-detail route when available.

## Safety and compatibility

The Production Queue remains the only production execution authority. Studio
Store authority, project isolation, exact `workflowVersionId + recipeId`
identity, historical task/production references, database compatibility, and
backup behavior are preserved. This release adds no schema or migration and no
new product flow.

## Verification

- Frontend: 150 test files and 824 tests passed.
- TypeScript: passed.
- Production build: passed with no >500 kB warning.
- Rust format, check, focused cancellation tests, and all-target tests: passed.
- Source-only CI: exact final-head run is recorded in
  `docs/DEV_116_RELEASE_HEALTH.md`.

## Known non-blocking debt

- Documentation-only Source-only CI dispatch remains manual.
- Some diagnostic vocabulary remains technical by design.
- Bounded large-collection surfaces retain a wayfinding cost.

Detailed evidence is in `docs/DEV_116_STABILITY_REPORT.md` and the release
health record in `docs/DEV_116_RELEASE_HEALTH.md`.
