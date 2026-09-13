# Changelog

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
