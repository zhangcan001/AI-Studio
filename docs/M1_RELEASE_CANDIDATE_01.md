# M1 Release Candidate 01 Validation

Date: 2026-08-09

Implementation commit: `e9969da486d8e23b0f2f9fac00b2363426530086`

Result: **PASS**

## Scope

This gate validates global production admission, interactive submission safety,
a real mixed Kera2/MiniMax H3 persistent queue, deterministic restart recovery,
and the Windows release build. Production runtime scope remains limited to
Kera2 image generation and MiniMax H3 reference-to-video.

No migration was added or modified. Database migrations remain 001–007.

## Regression

| Check | Result |
| --- | --- |
| Rust tests | 244 PASS |
| Frontend tests | 35 PASS (13 files) |
| `cargo fmt --all -- --check` | PASS |
| `cargo check` | PASS |
| `cargo test -- --test-threads=1` | PASS |
| `pnpm test` | PASS |
| `pnpm build` | PASS |
| `git diff --check` | PASS |

The admission/recovery tests cover first-queue admission, cross-project busy
rejection, same-batch idempotency, paused batches with active work, terminal
release, deterministic legacy RUNNING selection, active-task priority, and the
multiple-active conflict policy. Existing production queue pause/resume,
archive/restore/delete, skip/requeue, failure policy, and uncertain-dispatch
coverage remains green.

## Global Admission

- Single global persistent production batch: PASS.
- Cross-project start while another production batch is active: rejected with
  `PRODUCTION_QUEUE_BUSY`.
- Starting the already-running batch preserves idempotent behavior.
- Pausing a batch does not cancel its dispatched task. Its active item continues
  to block every other batch until the task becomes terminal.
- A new batch is admitted after the active task becomes terminal.
- No duplicate dispatch or automatic resubmit was observed.

Live control batches:

- Batch A: `pbt_85f72253230f4bdba5cc3833dd7cbbd2`
- Batch A task: `tsk_f1ec92fa-1db3-4fbe-9558-bc878590ec8b`
- Batch B: `pbt_30e245193c274f00952ab59a5cc16d03`
- Cross-project batch: `pbt_bd331c6c4b714baf949310355283a66c`

Batch B and the cross-project batch created no task while blocked. Batch B was
admitted only after Batch A's active task reached a terminal state.

## Interactive Safety

While global production admission was busy:

- Studio Generate: disabled in the UI and rejected by the backend.
- Local Batch Submit: disabled in the UI and rejected by the backend.
- Retry Once: disabled in the UI and protected by the same backend command gate.
- Prompt/draft editing and Add current: available.
- Project switching and browsing Assets, Tasks, Projects, and Workflows: available.
- The Production queue active banner remained visible after switching projects.

Direct command invocation returned `PRODUCTION_QUEUE_BUSY` and created no task.

## Mixed Kera2 / MiniMax H3 Production

Batch: `pbt_8485e1f699ba4352b3a8d23a20412142`

State: `READY -> RUNNING -> COMPLETED`

| Item | Runtime | Task | Result | Created after previous finish |
| --- | --- | --- | --- | --- |
| 0 | Kera2 T2I Local v2 | `tsk_5699c400-a43a-4820-a359-fcffb960ce4f` | SUCCEEDED | n/a |
| 1 | MiniMax H3 1.1.2 | `tsk_9481b8de-6580-4285-96f5-aeb60c1f7ad9` | SUCCEEDED | +0.5705854 s |
| 2 | Kera2 T2I Local v2 | `tsk_afdda843-e7ed-4c66-aae8-dcc14a20fb98` | SUCCEEDED | +0.2843525 s |

The H3 item used the validated 16 GB profile: 0.1 MP, 5 seconds, four sampling
steps, and one active H3 task. All three tasks have independent generation
snapshots, persisted task events, output mappings, and task IDs. The required
CREATED, VALIDATING, PREPARING, QUEUED, RUNNING, COLLECTING, and SUCCEEDED
lifecycle stages are present. Database timestamps prove that no item was
dispatched before the preceding item finished.

Observed H3 peak GPU memory use was 15,524 MiB with 527 MiB free. The task
completed without OOM.

## Assets and Playback

The mixed queue produced three project-scoped assets in the same project:

| Runtime | Asset | Category | Metadata |
| --- | --- | --- | --- |
| Kera2 #1 | `ast_11499a1c-e1aa-433d-84ea-c54561c3e7ef` | `generated_image` | PNG, 768x1280, 989,656 bytes |
| MiniMax H3 | `ast_0394e435-83c5-4bda-a205-101a498452dc` | `generated_video` | MP4, 256x416, 5.167 s, 509,837 bytes |
| Kera2 #2 | `ast_ca84dd40-2f4a-46a4-951d-f23ac77b25ee` | `generated_image` | PNG, 768x1280, 906,010 bytes |

The H3 video contains H.264 video at 24 fps (124 frames) and AAC stereo audio
at 32 kHz. Playback reached ready state and advanced current time in both Task
Detail and Asset Library. Asset IDs, source task IDs, and project ownership
remained correctly isolated.

## Restart Admission Recovery

Fast-restart validation:

- Batch: `pbt_196680e552c143d99d443ac48de708b6`
- Original task: `tsk_34b6bbfd-b272-4820-a31d-632ad9431917`
- Output asset: `ast_b7b5a18e-8976-4308-8d89-a4c891087b9a`
- Result: batch COMPLETED and original task SUCCEEDED after desktop restart.
- Duplicate task: none.
- Automatic resubmit: none.

Recovery observed the already-dispatched task through the existing task
reconciliation path until it became terminal, then finalized the queue item.
Automated tests also prove deterministic recovery of legacy multiple-RUNNING
data: an active dispatched batch takes priority, otherwise the oldest batch by
`created_at ASC, id ASC` is selected, and other RUNNING batches are paused. A
multiple-active conflict pauses all new dispatch and records
`PRODUCTION_ADMISSION_RECOVERY_CONFLICT` without cancelling or resubmitting.

## Windows Release Gate

- `pnpm tauri build`: PASS.
- Release executable: generated and started successfully.
- Installer artifacts: MSI and NSIS generated successfully.
- Code signing configuration: unchanged.
- Existing database smoke: PASS; projects, workflows, Kera2, MiniMax H3,
  assets, tasks, and production queues loaded without migration error or data
  loss.
- Fresh database smoke: PASS; migrations 001–007 all succeeded, the default
  project was created, and the empty workflow/task state loaded normally.
- Existing user data was restored after the isolated fresh-database smoke; no
  database or asset data was deleted.

## Final Status

`M1 RELEASE CANDIDATE 01 = PASS`
