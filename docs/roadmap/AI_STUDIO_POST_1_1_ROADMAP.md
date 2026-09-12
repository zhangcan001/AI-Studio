# AI Studio Post-1.1 Roadmap

```text
TASK=DEV-099
RELEASED_BASELINE=1.1.0
BASELINE_SHA=41c0b347ee7cd4a11c5afa6441bada492427c41c
ROADMAP_STATUS=UPDATED_BY_DEV_104
NEXT_VERSION_RECOMMENDATION=1.2.0
SELECTED_FIRST_DELIVERY_THEME=External Agent → AI Studio Production Handoff
NEXT_MAIN_THEME=Production Execution / Review Operations
```

## Product baseline

AI Studio 1.1.0 is the published, frozen baseline. DEV-099 does not bump the
manifest, create a tag, publish a release, replace assets, or rewrite release
history. The manifest remains `1.1.0` until a future release-candidate task.

The current source already has strong project, shot, preparation, queue,
review, workflow, recipe, asset, backup, and diagnostics authorities. The
first post-1.1 product gap is the continuity between those authorities from the
project entry point.

## Priority roadmap

### P0 — First delivery train: Production Continuity

Close the project-to-production continuation loop using derived existing facts:

1. carry exact `shotId`, `batchId`, `taskId`, and selected deliverable
   `assetId` targets through the Project Command Center projection when those
   records exist;
2. keep next-action priority and blocked/review/running/completed semantics
   derived from readiness, shot, queue, task, audit, and selected asset facts;
3. display the business reason and exact destination class in the Project
   Command Center; and
4. reuse existing routes and focus mechanisms without starting, retrying,
   rebinding, or creating production work.

Acceptance invariants:

```text
NEXT_ACTION_STATE_SOURCE=DERIVED_EXISTING_FACTS
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
NO_NEW_MUTATION_AUTHORITY=YES
DATABASE_MIGRATION=NO
AUTO_START_ON_CREATE=NO
AUTO_RETRY=NO
AUTO_REBIND=NO
```

### P1 — External Agent → AI Studio Production Handoff

Freeze a provider-neutral, project-scoped handoff contract for structured
production inputs supplied by an external agent. AI Studio must preview and
explicitly confirm formal production input without becoming a Script, Draft,
Storyboard, or Prompt authoring product. The full hierarchy importer is
blocked by the current schema and must not add a migration, second Shot model,
queue, executor, or Task model.

### P1 — Asset / Reference Continuity — CLOSED (DEV-102)

Improve direct visibility and navigation between existing ReferenceSet/Profile,
Shot references, generated inputs, and selected deliverables. Keep the existing
Asset Library authoritative; no duplicate store, table, or implicit file copy.

DEV-102 closes this read/navigation slice with the existing
`AssetUsageService`, exact Shot/Asset/Task targets, explicit unavailable
reference states, Shot input/result separation, and Project Complete →
selected Asset continuity. DEV-103 now closes the next explainability slice.

### P1 — Daily Production Explainability — CLOSED (DEV-103)

Expand the derived explanation surface for “why blocked”, “what is running”,
“what needs review”, and “what is complete” after the P0 continuity target
model is proven in daily use.

DEV-103 closes this slice with a read-only, bounded five-bucket board under
Project Command Center, stable reason codes, exact existing repair targets,
and no migration or second persisted issue/next-action state.

### P1 — Bulk Production Preparation / Operations — CLOSED (DEV-104)

DEV-104 closes this slice with a project-level, typed preflight/admit path
backed by the existing `ProductionPreparationService`, a strict 100-shot
preparation boundary inside a 500-shot project plan, explicit partial mode,
exact workflow-version/recipe identity, and a prepare-only UI boundary. No
new queue, executor, task model, migration, or automatic start was added.
The next main theme is Production Execution / Review Operations; DEV-105 has
not started.

### P2 — Product polish and operational clarity

Improve copy, empty states, stale-fact refresh affordances, and bounded
cross-workspace breadcrumbs where they do not change domain authority.

### Deferred

- Workflow/Recipe lifecycle backend changes after the DEV-090–095 closed phase.
- Internal narrative, Script, Storyboard, or Prompt authoring architecture.
- New scheduler, retry engine, executor, or queue semantics.
- New persisted Project Issue or Next Action state.

### Not Planned

- A second Production Queue, executor, or Task model.
- Rebinding by display name, array index, or guessed latest record.
- Auto-start, auto-retry, or auto-rebind from the project entry point.
- Version bump, `v1.2.0` tag, GitHub Release, or installer publication in
  DEV-099.

## Why Production Continuity is first

The audit ranked Production Continuity first because it is high-frequency,
high-friction, and implementable entirely from existing project-scoped read
facts. It connects capabilities users already have instead of adding a new
domain model. It also gives the product a clear new ability suitable for a
future `1.2.0` release while leaving the published `1.1.0` line immutable.

The external-agent handoff is frozen as a contract, but full hierarchy
implementation is deferred because the current schema has no handoff identity,
provenance, idempotency, or single-transaction boundary. Asset continuity is
deferred as an independent theme because the first train only needs exact
deliverable navigation. Broad explainability is deferred as a separate theme
because P0 includes only the reason text required to make the selected
continuation actionable.

## Frozen implementation boundary

DEV-099 may change only the Project Command Center read projection, its typed
frontend model, existing navigation/focus plumbing, and focused tests/docs.
Persistence remains behind the repository port; no migration is required.
The formal executor remains ComfyUI and all production actions remain explicit
user actions in the existing Production Queue/Task path.
