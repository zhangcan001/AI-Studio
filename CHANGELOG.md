# Changelog

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
