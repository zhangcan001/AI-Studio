# Changelog

## v2.0.0-personal

AI Studio v2.0.0 Personal Edition freezes the local-first personal AI creation
workspace baseline.

### Highlights

- Production Core remains the single Queue/Task/Generation execution path with
  Comfy execution admission held for the real generation lifecycle.
- Asset Library provides project-scoped Assets, immutable AssetVersions,
  AssetRelations, previews, and explicit provenance.
- Prompt Studio provides Prompt/PromptVersion reuse with the canonical
  Model/ModelVersion registry.
- Local Tool Hub records local Tool, ToolInstance, ToolVersion, and Capability
  metadata without starting, installing, or executing tools.
- Project Archive v19 exports and restores project data with explicit ID remap,
  provenance preservation, and visible UNKNOWN handling.

### Architecture

- Local-first storage remains SQLite metadata plus filesystem media.
- Project-owned relationships use exact IDs; filenames, paths, timestamps, and
  prompt text are never used to guess historical lineage.
- No second Asset, Generation, Queue, executor, AI Agent, SaaS, cloud sync, or
  multi-user authority is introduced.

### Migration and archive

- Existing v1.3.1 data remains on the additive migration path.
- Backup format v19 is the stable archive baseline. Historical v18 packages
  remain readable with visible compatibility warnings when v2 data is absent.

### Verification

- Frontend tests, TypeScript, production build, Rust format/check/tests, and
  exact-final-head Source-only CI are required release gates.

### Known issues

Non-blocking follow-up items are recorded in
`docs/AI_STUDIO_V2_KNOWN_ISSUES.md`; this release has `P0=NONE` and `P1=NONE`.

## v1.3.1

AI Studio v1.3.1 is a stability and maintenance release.

### Highlights

- More reliable cancellation E2E synchronization, including repeated-cancel
  coverage and explicit cleanup assertions.
- Conservative Source-only CI timeout budgets with all existing gates retained.
- Route-level workspace loading that removes the frontend >500 kB warning.
- Localized failed-task guidance with technical details and task-detail action.

### Safety

- Production Queue remains the only execution gate.
- No schema, migration, task model, or production execution behavior changed.

### Testing

- Frontend regression suite, TypeScript check, production build, Rust checks,
  and Source-only CI were run for the maintenance release.

## v1.3.0

AI Studio v1.3.0 makes the production path easier to understand and follow
from first project entry through the selected final result.

### Highlights

- Production navigation improvements with exact project, Shot, Task, Asset,
  Review, and batch continuation.
- Large-project locate and bounded collection views that keep large runs
  usable.
- Queue, Review, and Rework continuity with explicit next actions.
- Clear production state language for waiting, running, failed, review, and
  completed work.
- First-time user guidance for project creation, Production Handoff,
  preparation, Queue Start, and review.
- Final-result visibility linking the selected result to its Shot, Asset, and
  available file-location flow.

### Safety

- Production Queue remains the only execution gate.
- Preparation and Rework never auto-start production.

### Testing

- Frontend regression suite, TypeScript check, production build, Rust checks,
  and Source-only CI were run for the release.
