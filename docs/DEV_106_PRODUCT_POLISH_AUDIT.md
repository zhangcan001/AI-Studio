# DEV-106 product polish audit

```text
TASK=DEV-106
SCOPE=POST_1_1_PRODUCT_JOURNEY
FEATURE_FREEZE=YES
DATABASE_MIGRATION=NO
```

This is a source- and regression-test walkthrough of the existing project
journey, not a claim that a human completed a live ComfyUI production run.
The separate real-runtime smoke result is recorded in the readiness gate.

| Step | Entry / empty state | Next action / return path | Language, duplicate, dead-end audit |
| --- | --- | --- | --- |
| Create Project | Project entry and Command Center expose project selection and no-Shot state. | Creation or import is offered; existing project navigation remains. | No new project state introduced. |
| External Agent Handoff | Existing bulk-import workspace opens **导入外部生产数据**. Format help, version 1, and copyable two-Shot example are now visible. | Back to bulk import; preview before confirm; open formal structure after import. | No agent connector or internal authoring. Exact placeholder workflow/recipe pair must be replaced or removed. |
| Preview / Confirm | Read-only preview exposes counts, blocking issue code **and path**, replay state, and write plan. | Confirm only when valid; failed/stale preview is actionable by editing and re-previewing. | Confirm creates formal hierarchy/input, not a queue or Task. |
| Daily Production | Empty state explains that a Shot or import is needed; five existing derived buckets remain bounded. | Existing exact Shot/Batch/Task/Asset targets route to repair or continuation. | No duplicate Issue store or per-Shot IPC loop. |
| Asset / Reference | Existing Asset Library and Shot reference surfaces expose usage and missing reference states. | Exact Asset/Shot links, including selected output and deliverable path. | No second asset authority or implicit reference copy. |
| Bulk Preparation | Existing project preflight is read-only; selection is at most 100 in a 500-Shot plan. | **Prepare** creates READY batch only; Production Queue is the explicit next step. | 101 selected rejects rather than silently chunking; no auto-start. |
| Production Queue | Existing queue displays READY/RUNNING and Start boundary. | Explicit Start, pause/cancel/recovery use existing Task path. | No second queue or executor. |
| Review Inbox | Project Command Center shows a bounded, load-more list; empty state says no pending review. | Exact review, Task, Shot, Asset targets; refresh remains available. | Main label is **待审核结果**; low-level IDs are folded into technical details. Bulk review mutation remains deferred. |
| Selected Result / Deliverable | Existing Shot selected-output authority and Asset Library preserve the chosen result. | Command Center Complete action opens exact deliverable Asset where present. | No competing selected-output state. |

## Large-project and authority checks

- The handoff fixture previews/confirms 500 formal Shots with zero Tasks and
  zero production batches; a 501st Shot is rejected.
- The 500-Shot preparation plan uses one batch resolver/read path and bounded
  Comfy capability checks. Selecting 101 rejects; selecting 100 READY Shots
  prepares exactly one batch with zero Task submissions.
- Daily Production and Project Command Center use existing bounded read
  projections; Review Inbox pages 25 at a time; Asset usage is bounded by its
  existing projection. These checks are regression-level, not load-test timings.
- Preparation, batch creation, review rework, and handoff confirmation do not
  start production. Only the existing Production Queue Start action does.

## Deferred P2

- A separate language pass over older independent generation workspaces would
  be useful, but is outside the frozen project handoff/production journey.
- Live accessibility and performance profiling with a populated 500-Shot
  desktop Project remains future hardening; no P0/P1 defect was found in the
  bounded source/test path.
