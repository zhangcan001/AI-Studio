# AI Studio Post-1.1 Product Audit

```text
TASK=DEV-099
BASELINE=41c0b347ee7cd4a11c5afa6441bada492427c41c
PUBLISHED_BASELINE=1.1.0
AUDIT_DATE=2026-09-11
AUDIT_RESULT=PASS
```

DEV-100 rebaselines the product boundary: internal Script, Draft, Storyboard,
and Prompt authoring are out of scope and removed from the active runtime.
Structured production input may be supplied by an external agent or entered
manually, then remains subject to the existing project-scoped validation,
snapshot, Queue, Task, and ComfyUI authorities.

## Audit method and authorities

The audit used the current `master` source, current schema/migrations, current
typed transport, and current tests as the authority. The historical product
records were cross-checked but were not treated as implementation truth:

- `docs/DEV_073_AI_STUDIO_1_0_READINESS_REVIEW.md` confirms the released
  production and regression baseline.
- `docs/DEV_095_WORKFLOW_RECIPE_LIFECYCLE_PHASE_GATE.md` closes the
  Workflow/Recipe lifecycle phase and preserves exact
  `workflowVersionId + recipeId` identity.
- `docs/DEV_096_POST_1_0_REBASELINE_1_1_READINESS.md`,
  `docs/DEV_097_RELEASE_1.1.0_RC.md`, and
  `docs/DEV_098_RELEASE_1.1.0_PUBLICATION.md` establish the frozen 1.1.0
  publication baseline.
- PX-01, PX-02, and PX-04 describe the Project Cockpit, Shot Production
  Workspace, and Workflow Center surfaces that were verified against source.
- The historical narrative, Script Import, and Storyboard architecture
  documents are retired evidence under DEV-100. External-agent handoff is a
  frozen contract only; the current schema blocks a safe hierarchy-wide
  implementation. Context/Preparation/Queue/Review remain owned by the
  existing production services.

## Capability matrix

| Domain | Existing capability | User-visible completeness | Missing continuity | Risk |
| --- | --- | --- | --- | --- |
| Project | Project Workspace and Project Command Center aggregate project, progress, issues, runtime, preparation, and recommendations | PARTIAL | The cockpit has a derived recommendation but not every recommendation has a deep target; blocked reasons are not consistently tied to the exact repair surface | Medium |
| Narrative | External-agent/manual structured production input and formal production structure remain; internal Script/Draft authoring is out of scope | OUT OF SCOPE / RETIRED | A hierarchy-wide external handoff needs schema identity, provenance, idempotency, and one transaction | High / explicitly blocked |
| Storyboard | Formal Shot/Scene/Production surfaces remain; internal Storyboard Draft authoring is retired | OUT OF SCOPE / RETIRED | No internal storyboard editor or automatic conversion is part of the product boundary | Low |
| Shot | Shot Workspace supports exact shot selection, references, workflow/recipe configuration, readiness, preparation, queue admission, generation, review, and resume selection | PARTIAL | A project recommendation can reach a shot or batch, but Queue/Task/Deliverable return paths are not uniformly exposed from the same continuity model | Medium |
| Asset | Asset Library, generated/source asset preview, usage, tags, references, profiles, and exact project scope exist | PARTIAL | Completed production can be counted by the cockpit but does not directly open the exact existing deliverable asset | Medium |
| Workflow | Workflow Center, registry, runtime package diagnostics, project bindings, profiles, and exact-pair presentation exist | COMPLETE for 1.1 scope | Mostly presentation-level handoff from production surfaces; no lifecycle backend gap found | Low |
| Recipe | Promotion, archive/restore, history, exact-pair admission, and immutable historical references are covered by DEV-090–095 | COMPLETE / FROZEN | No reason to reopen lifecycle semantics; use only existing state for navigation and explanation | Low |
| Production | Preparation, queue admission, formal ComfyUI executor, batch runbook, production monitor, audit, review, and deliverable assets exist | PARTIAL | These authorities are individually strong but the project entry point does not expose a single exact continuation target for every state | Medium |
| Queue | Existing Production Queue is the sole manual execution authority with runbook, start/pause/cancel/retry/resume policies | PARTIAL | Project-level queue counts carry batch identity, but shot/task identity is not always carried through the command-center projection | Medium |
| Review | Shot review boards, batch review productivity, compare, explicit result selection, and audit links exist | PARTIAL | `Needs Review` can reach a shot or queue, but failure/review continuity is not uniformly coupled to the exact task/item target | Medium |
| Deliverables | Generated assets, task output assets, selected shot results, preview, usage, and local delivery manifests exist | PARTIAL | The final project-level `Complete` action returns to creation instead of the exact existing deliverable authority | Medium |
| Backup | Project backup/restore and schema compatibility gates are released and project-scoped | COMPLETE for current contract | Backup is not the missing daily-production continuity surface | Low |
| Diagnostics | ComfyUI status/preflight, Workflow Center diagnostics, production audit, issue codes, and technical detail surfaces exist | PARTIAL | Diagnostic facts are visible, but the next action does not always state the business reason and repair destination together | Medium |

## Real user journey audit

The current source journey is:

```text
Project Workspace / Command Center
  → Shot Workspace (Creation / Production / Review)
  → existing Preparation and Production Queue
  → existing Task History / Production Audit
  → existing Review / selected output asset
```

The path is not a dead product. The main continuity gaps are:

1. `ProjectCommandCenterService` derives the priority and reason of the next
   action, but its payload only carried a shot or batch target. The repository
   projection already reads shot links, queue items, tasks, and selected output
   assets, so the missing task/asset targets were a projection gap rather than
   a missing domain model.
2. The project hero showed a useful label and detail, but it did not make the
   blocked reason and the exact target class explicit. Users could still need
   to rediscover whether to open a queue, task, shot, or asset.
3. A running queue could be opened by batch, and review could be opened by
   shot, but the command-center projection did not consistently carry the
   corresponding shot/task identity for queue-derived actions.
4. A completed project was sent to a new creative round. That is a reasonable
   fallback but not the requested continuity endpoint when an existing
   selected output asset is already the deliverable authority.
5. A provider-neutral external-agent handoff is now the documented boundary,
   but current source does not expose a complete hierarchy-wide importer.
   Implementing it now would require a new persistence contract and migration,
   so DEV-100 records the schema blocker rather than approximating it.

No duplicate queue, executor, task model, issue table, or persisted Next Action
state was found or is warranted. Project isolation is enforced by the existing
repository queries and exact project-scoped IPC calls.

## Gap ranking

Scores use a 1–5 scale; higher is better except `REGRESSION_RISK`, where a
higher score means lower risk.

| Rank | Gap | USER_VALUE | DAILY_FREQUENCY | FRICTION_REDUCTION | ARCHITECTURE_FIT | IMPLEMENTATION_CONFIDENCE | REGRESSION_RISK | Total |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Project → exact Shot/Queue/Task/Review/Deliverable continuation | 5 | 5 | 5 | 5 | 5 | 4 | 29 |
| 2 | Daily production explainability as a broad project surface | 4 | 5 | 4 | 4 | 3 | 3 | 23 |
| 3 | Asset/Reference → Shot and deliverable usage continuity | 4 | 4 | 4 | 4 | 3 | 3 | 22 |
| 4 | External Agent → formal production handoff | 5 | 3 | 4 | 3 | 1 | 2 | 18 |

The first row is the highest-value implementable slice. Asset and explainability
work is included only where it is necessary to make the selected continuity
target understandable; a separate asset architecture or second state model is
not part of this train.

## Theme decision

```text
SELECTED_THEME=External Agent → AI Studio Production Handoff
SELECTED_THEME_CODE=A
NEXT_VERSION_RECOMMENDATION=1.2.0
```

The selected theme is a clear new product ability: an existing project fact can
lead the user to the exact existing work item rather than merely naming a
workspace. It is therefore a minor product release recommendation (`1.2.0`),
while the manifest remains `1.1.0` during DEV-099.

The implementation is intentionally limited to the existing Project Command
Center read projection and navigation surfaces:

- derive task, shot, batch, and selected deliverable targets from existing
  records;
- show the business reason and target surface in the project continuation
  card;
- reuse existing App routing, Shot Workspace selection, Task History focus,
  and Asset Library preview; and
- add focused tests for priority, exact IDs, blocked explanation, running and
  review targets, completed deliverables, project isolation, and refreshed
  facts.

## Deferred and excluded candidates

- Full External Agent → Production Handoff implementation is `P1` for a later
  train because its missing hierarchy identity/provenance boundary needs a
  schema-backed contract. Internal authoring remains out of scope.
- Asset / Reference Continuity is `P1` when it is a direct read/navigation
  improvement, but no second asset store or implicit copy is planned.
- Broad Daily Production Explainability is `P1`; the selected train only adds
  the explanation needed by the exact Project Continuity action.
- Workflow/Recipe backend lifecycle changes are `Deferred` and remain closed by
  DEV-095. This train does not add compare, new lifecycle states, Registry V3,
  archive semantics, or mutation authority.
- A second queue, executor, scheduler, retry engine, task model, persisted
  Next Action table, UI-only production state machine, and version/release
  publication are `Not Planned` for DEV-099.
