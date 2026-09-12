# DEV-104 — Bulk Production Create / Start Audit

```text
TASK=DEV-104
BASELINE=ab16b39c09e5955d6b310e427bb95fcc601742a7
FORMAL_EXECUTOR=COMFYUI
PREPARE_NEVER_START=PASS
MAX_PROJECT_PLAN_SHOTS=500
MAX_PREPARATION_BATCH_ITEMS=100
```

The audit follows the real call boundaries rather than button text. Formal
preparation ends at a persisted `READY` batch/snapshot. Queue start remains an
existing explicit action in the Production Queue surface.

| PATH | USER_ACTION | CREATES_BATCH | STARTS_QUEUE | EXPLICIT_START | SAFE/UNSAFE | DECISION |
| --- | --- | ---: | ---: | ---: | --- | --- |
| `src/features/shots/ShotBulkConfigPanel.tsx` | Prepare selected project shots | Yes, through `project_production_admit` → `ProductionPreparationService.admit` | No | No | SAFE after DEV-104 fix | Uses one project-level preflight, strict default, explicit partial mode, 100-item bound, and opens the existing queue only after preparation. |
| `src/features/shots/ShotBatchPlanner.tsx` | Prepare selected eligible shots | Yes, existing Shot Batch authority | No | No | SAFE after DEV-104 fix | Removed the create-then-start fallback; the result is explicitly described as待启动. |
| `src/features/shots/SceneProductionPreparation.tsx` | Select READY shots and加入生产 | Yes, `scene_production_admit` → `ProductionPreparationService.admit` | No | No | SAFE | Existing live preflight and strict `allowPartial=false` boundary retained. |
| `src/features/shots/SceneProductionPanel.tsx` | Prepare scene, then click start | Yes | No on prepare; yes only in `startPreparedBatch` | Yes | SAFE | Preparation and queue start are separate user actions. |
| `src/features/shots/EpisodeProductionPanel.tsx` | Prepare selected scenes | Yes, existing episode preparation service | No | No | SAFE | Reuses existing episode authority; no duplicate project tree or queue. |
| `src/features/shots/SeriesProductionPanel.tsx` | Prepare selected episodes | Yes, existing series preparation service | No | No | SAFE | Default strict mode is off for partial preparation and the result provides existing queue navigation. |
| `src-tauri/src/application/production_package_service.rs` | Inspect / create package batches | Yes | No (`auto_started=false`) | No | SAFE | Existing package service keeps package creation separate from queue start. |
| `src-tauri/src/application/external_production_handoff_service.rs` | Preview / confirm handoff | No queue or task | No | No | SAFE | Handoff remains import/provenance only; production preparation is a later explicit operation. |
| `src/features/studio/ProductionQueuePanel.tsx` | Create an ad-hoc queue and later click start | Yes | No on create; yes in `startQueue` | Yes | SAFE | Existing queue surface remains the only start authority. |
| `src/features/production/CreationDashboard.tsx` | Start/pause an existing queue | No | Yes | Yes | SAFE | This is an explicit queue operation, not preparation. |
| `src/features/assets/AssetVideoBatchWorkspace.tsx` | Local asset batch import | Yes | Existing optional local-import flow may start | User-controlled legacy import option | OUT OF SCOPE | Not a Shot/Scene/Episode/Series preparation path; retain its existing explicit local-import option and do not route it through project preparation. |

## Boundary decisions

- `PREPARE != START` and `CREATE_BATCH != START` are enforced at the UI call
  boundary and by the existing backend preparation service.
- Project planning uses one typed `project_production_preflight` request and
  the existing `ProductionPreparationService.plan_many` authority for up to
  500 shots. Project admission is capped at 100 shots and revalidates live.
- Strict mode defaults to `allowPartial=false`. Partial mode is a visible,
  user-controlled checkbox and never turns on automatically because a blocker
  is detected.
- Selection over 100 is not silently chunked or auto-created. The UI disables
  preparation and tells the user to reduce the selection.
- No new queue, executor, Task model, migration, or persisted preparation
  state was introduced. The result remains an existing READY batch with frozen
  bindings/snapshots, followed by explicit Production Queue start.

## Verification markers

The architecture guard checks the source-level boundaries and emits:

```text
BULK_PREPARE_NO_AUTO_START=PASS
BATCH_CREATE_NO_AUTO_START=PASS
EXPLICIT_QUEUE_START_ONLY=PASS
BULK_PREPARATION_AUTHORITY=PASS
```
