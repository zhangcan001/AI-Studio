# DEV-030 — Post-0.5.0 Stability Validation

Date: 2026-08-17
Repository: `zhangcan001/AI-Studio`

## 1. Baseline

- Branch: `master`
- `DEV030_START_SHA`: `8dac298309d2c18c6ae266a82a618a357aaaee97`
- Start commit: `docs: record AI Studio 0.5.0 publication`
- Working tree at start: clean
- `origin/master` at start: `8dac298309d2c18c6ae266a82a618a357aaaee97`
- Current released source commit: `02e67cff50f5da1d207478071636af166048820c`
- Current migration: `019`
- Current `BACKUP_VERSION`: `10`

## 2. Published Release Identity

- GitHub Release: `v0.5.0`
- Release state: published, non-draft, non-prerelease
- Published asset count: 4
- `v0.5.0` peeled SHA: `02e67cff50f5da1d207478071636af166048820c`
- `v0.4.0` peeled SHA: `94918f6322ce690ff7b1630961abb56b8a31ed11`
- No tag, release, asset, version, migration 019, or backup format was changed.

## 3. Remote Artifact Verification

The four assets were downloaded from the published GitHub Release, rather than
from the local `target/release` directory.

| Filename | Bytes | SHA256 | Result |
| --- | ---: | --- | --- |
| `ai-studio.exe` | 36,982,784 | `587201EA1C95D3BB1E9268747F36D595D121069B3C7908F747B43BBAFC7C5FC7` | PASS |
| `AI.Studio_0.5.0_x64-setup.exe` | 8,400,244 | `A598D358734FBFDFDA4814245E62F42F35C768ED7FEE47527213B89F4E895992` | PASS |
| `AI.Studio_0.5.0_x64_en-US.msi` | 12,349,440 | `037FC1D973BA11E083CAE66A706D265CA64FD3E96479316D46A1106AD584A62B` | PASS |
| `RELEASE_SHA256_0.5.0.txt` | 472 | `75D5EBB08A32F57EE436BCBE7DEA15D57D37D6D6FD513C24309A56ABF0566924` | PASS |

The downloaded files matched the remote checksum manifest. The manifest text
itself still contains the historical `PUBLISH_STATUS=RC_NOT_PUBLISHED` value;
this is a P2 documentation inconsistency in an already published asset and was
not modified in DEV-030.

## 4. Portable Smoke

- Source: official remote `ai-studio.exe`
- Data root: isolated temporary root
- Version/build: `0.5.0` / `02e67cff50f5da1d207478071636af166048820c`
- Migration max: `19`
- Minimal project creation: PASS
- Close and restart: PASS
- Project persistence after restart: PASS
- Result: **PASS**

## 5. NSIS Smoke

- Source: official remote `AI.Studio_0.5.0_x64-setup.exe`
- Install: exit code 0, PASS
- Launch: PASS
- Restart: PASS
- Version/build/migration: `0.5.0` / source commit above / `019`, PASS
- Silent uninstall: exit code 0, PASS
- Install directory removal: PASS
- Isolated data root retained after uninstall: PASS
- Result: **PASS**

## 6. MSI Smoke

- MSI package database was readable and reported ProductVersion `0.5.0`: PASS
- Fresh install: exit code `1603`
- Launch: not reached because Windows Installer did not register the product
- Uninstall: exit code `1605` (`product not installed`)
- ProductCode/UpgradeCode behavior: no registered installation was created, so
  no completed install/upgrade registration or residual shortcut behavior could
  be validated; no application installation was left behind
- Result: **BLOCKED by the local Windows installer environment**

This is recorded as an environment limitation, not as a demonstrated product
startup or data-loss defect. No installer refactor was made.

## 7. v0.4 → v0.5 Installer Upgrade

The official `v0.4.0` assets were available and identified, but a complete
real-world installer upgrade was not completed after the isolated MSI install
failed. Therefore the two dimensions are intentionally separated:

- `INSTALLER_UPGRADE`: **UNVERIFIED / BLOCKED by the installer environment**
- `DATA_UPGRADE`: **PASS at the database migration contract level**; the
  installer-backed preservation run remains unverified
- Project, prompt, and shot preservation across an actual v0.4 installer and a
  v0.5 installer: not claimed

The successful portable and NSIS restart checks do confirm persistence of data
created by the v0.5 release itself. They do not substitute for the unexecuted
v0.4 installer upgrade.

## 8. Data Upgrade

- Migration `018 → 019` migration tests: PASS
- Migration `019` stage-prompt backfill behavior: PASS
- Repeated migration safety: PASS
- Installer-backed v0.4 database preservation: UNVERIFIED because the official
  installer path could not be completed in this Windows environment
- No production AppData database was used as a test target

## 9. Migration 019

- Latest migration remains `019`
- `019_shot_stage_prompts.sql`: unchanged
- Migration max check: PASS
- Backfill and repeated-application tests: PASS
- Migration `020`: not created

## 10. Backup v10

- `BACKUP_VERSION`: `10`
- Backup format: unchanged
- Backup-related regression and source checks: PASS
- Backup v11: not created

## 11. Wrong Runtime UX

The historical incompatible runtime was identified as ComfyUI `0.31.1` with
missing H3 nodes/models. It was not started for a live destructive test. The
application path was audited for structured diagnostics and user-visible
messages.

Structured paths include:

- `MISSING_NODE`
- `INPUT_OPTION_UNAVAILABLE`
- `WORKFLOW_VALIDATION_FAILED` with preserved `node_errors`
- `COMFY_PROTOCOL_ERROR`
- `COMFY_OFFLINE`

The messages distinguish missing nodes/inputs, workflow validation context,
protocol errors, and an offline runtime instead of reducing every case to a
bare “generation failed” message.

- Static diagnostic coverage: PASS
- Live wrong-runtime reproduction: environment-blocked/unverified
- P0: none observed
- P1: no confirmed diagnostic blocker remained after source audit

## 12. Correct Runtime Recovery

The known-good runtime was restored on `127.0.0.1:8188` and reported:

- ComfyUI `0.33.0`
- Python `3.12.10`
- PyTorch `2.9.0+cu130`
- NVIDIA RTX 5060 Ti
- `/system_stats`: HTTP 200
- `/object_info`: HTTP 200
- Required H3 nodes present, including `NBH3HyperStepSimple`,
  `MiniMaxH3ReferenceToVideo`, `MiniMaxH3ImageToVideo`, SaveVideo, loaders,
  and the production-route nodes

One optional `ComfyUI-MiniMaxH3DualClockSampler` import warning remained because
the runtime lacks `time_shift_slope`; it did not block the required production
route or the representative generation.

Result: **PASS**

## 13. Representative Generation

The live run used the normal application path:

`ShotBulkImport → ShotBatchService/ProductionQueueService → GenerationService → ComfyHttpAdapter`

It performed one image stage followed by H3 I2V, without a direct `/prompt`
call or UI key simulation.

- Project: `prj_e38f7167-620e-4428-b05f-1019acebb497`
- Shot: `sht_25716d30-1af8-4402-b6a6-3f5c1644c432`
- Image batch: `pbt_acb5df3946ba42318f7f9a3dfc516aba`
- Image task: `tsk_614aef06-8883-4df9-a843-930f0f9a7456`
- Image asset: `ast_9c344a37-fdee-4b34-8b75-5c5c115fae0e`
- Video batch: `pbt_a1805a3a164c427eab64737d5cf5905b`
- Video task: `tsk_89978b8b-5c78-4558-a60f-8b8bfc7bce48`
- Video asset: `ast_36a8ed67-a148-4bb2-8f05-e51b1bf0f972`
- Image workflow/version: `wfl_kera2_t2i_local_v2` /
  `wfv_2407734d-ff20-44d9-ac7c-15ab514d7193`
- Image recipe: `rcp_0575fb13-6bfb-41cb-ba10-eba2719a793c`
- Video workflow/version: `wfl_minimax_h3_fl2va_i2v_quality` /
  `wfv_817672cf-3dcb-495e-ad9e-201429ba684d`
- Video recipe: `rcp_5fcf5c7e-38f0-4f89-bf37-d6d372c46fa7`
- Snapshot: GenerationService snapshot path exercised for both tasks; the
  harness emitted workflow/version/recipe evidence rather than standalone
  snapshot IDs
- Result selection: image and video assets selected on the shot; shot status
  `COMPLETED`
- Result: **PASS**

## 14. P0

No P0 was observed. No data loss, formal database damage, silent wrong-runtime
generation, or unrecoverable startup failure was reproduced.

## 15. P1

Confirmed P1 findings were reduced to zero by targeted changes. The audit also
identified cancellation semantics, atomic multi-step shot save behavior, and
backup crash-consistency as risks that were not reproduced as live P1 failures;
they remain explicit post-0.5 follow-up risks rather than being silently marked
fixed.

The MSI and real v0.4 installer-upgrade limitations are environment validation
limits, not confirmed product P1s.

## 16. Deferred P2

- Remote checksum manifest still says `RC_NOT_PUBLISHED` in its status field
- Artifact signing status was not established by this task
- UI polish, busy-state refinement, 500-shot search/cap behavior, and N+1
  performance work
- Large-service decomposition and backup inspection scaling
- MSI ProductCode/UpgradeCode behavior after a successful Windows Installer
  registration

## 17. Fixes Applied

### Retry admission and duplicate-child prevention

- Issue: pause/requeue admission was not consistently serialized and repeated
  retry requests could append another child.
- Root cause: the shared admission gate was not held by both paths, and the
  retry child lookup was not idempotent.
- Files: `src-tauri/src/application/production_queue_service.rs`
- Test: `repeated_retry_finds_the_existing_child_instead_of_appending_another`
  (targeted Rust test PASS, 1/1)
- Result: pause and requeue now share the admission gate; an existing retry
  child is returned instead of being duplicated.

### Task retry idempotency

- Issue: manual task retry did not provide a stable submission identity.
- Root cause: retry submissions lacked a task-scoped idempotency key.
- Files: `src/features/tasks/retryPolicy.ts`,
  `src/features/tasks/TaskHistoryDetail.tsx`,
  `src/features/tasks/TaskHistoryDetail.test.tsx`
- Test: `TaskHistoryDetail.test.tsx` PASS, 2/2
- Result: retries use `task-retry:<task-id>` consistently.

### Stale shot reload protection

- Issue: an older asynchronous reload could overwrite state after the project
  changed or a newer reload completed.
- Root cause: reload responses were not generation-guarded.
- File: `src/features/shots/ShotWorkspace.tsx`
- Test: full frontend regression PASS
- Result: stale success, error, and loading-finally paths no longer mutate the
  current project state.

### Production action refresh truthfulness

- Issue: a successful production action could be reported as failed when the
  follow-up admission refresh callback failed.
- Root cause: the callback exception escaped the success path.
- File: `src/features/production/ProductionRunPanel.tsx`
- Test: full frontend regression PASS
- Result: the action remains successful and the user receives an explicit
  manual-refresh notice.

No schema, installer, release, model, runtime, or large refactor change was
made.

## 18. Regression

- Rust formatting: `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` PASS
- Rust type/check: `cargo check --manifest-path src-tauri/Cargo.toml` PASS
- Rust tests: **486 passed / 0 failed / 1 ignored**
- Frontend tests: **52 files / 170 tests**, all passed
- Frontend build: `pnpm build` PASS
- Diff check: `git diff --check` PASS; only expected Windows line-ending warnings
- Product diff before documentation: six intended source/test files only

## 19. Technical Debt

The following are audit priorities only; none were refactored in DEV-030.

| File | Current lines | Priority | Reason |
| --- | ---: | --- | --- |
| `src-tauri/src/application/project_backup_service.rs` | 6,649 | HIGH | backup orchestration and format surface are concentrated in one service |
| `src-tauri/src/application/production_orchestrator_service.rs` | 3,056 | HIGH | production lifecycle and admission decisions are broad |
| `src-tauri/src/application/production_queue_service.rs` | 1,919 | HIGH | queue recovery, retry, cancellation, and dispatch are coupled |
| `src-tauri/src/application/generation_service.rs` | 2,262 | MEDIUM | generation preparation, snapshots, dispatch, and assets share a large boundary |
| `src-tauri/src/application/shot_batch_service.rs` | 1,487 | MEDIUM | bulk stage preparation and queue binding are concentrated |
| `src-tauri/src/application/shot_bulk_service.rs` | 1,140 | MEDIUM | import and bulk configuration paths are broad |

## 20. 0.6.0 Backlog

Scores use `PRIORITY = USER_VALUE*2 + RELIABILITY*2 - COST - RISK`.
All dimensions are 1–5; higher cost/risk/dependency means more delivery burden.

| Candidate | USER_VALUE | RELIABILITY | COST | RISK | DEPENDENCIES | PRIORITY |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Series / Episode / Scene hierarchy | 4 | 3 | 2 | 2 | 1 | 10 |
| Runtime Profiles + Preflight Center | 5 | 5 | 2 | 3 | 2 | 15 |
| Failed Batch Partial Resume | 5 | 5 | 4 | 4 | 4 | 12 |
| Shot Search / Filter | 4 | 3 | 4 | 4 | 4 | 6 |
| Reference Asset Library / Character Anchors | 4 | 3 | 2 | 3 | 2 | 9 |
| Prompt Template Variables | 4 | 3 | 3 | 3 | 3 | 8 |
| Project Export Manifest | 3 | 4 | 3 | 4 | 3 | 7 |
| 100–500 Shot Performance | 5 | 5 | 2 | 3 | 2 | 15 |
| Backup Scalability | 4 | 5 | 2 | 3 | 2 | 13 |
| Production History / Audit UI | 4 | 3 | 3 | 4 | 3 | 7 |

The score is not used mechanically: Runtime Profiles and large-shot
performance score highly, but they are broader enabling work. Partial Resume
directly addresses the observed batch-recovery pain and builds on the retry
idempotency and admission hardening completed here.

## 21. Recommended DEV-031

**DEV-031 — Failed Batch Partial Resume**

- Why first: it has the clearest direct value for the current production flow
  and closes the remaining partial-failure/recovery gap.
- Goal: resume only failed or unresolved items from a batch while preserving
  frozen inputs/reference order and preventing duplicate submissions.
- Main outputs: item-level resume contract, stable idempotency behavior, clear
  partial-result status, safe UI entry point, and regression/live evidence.

## Git / Freeze Record

- Product fix commit: `0ded7682437d5bdcb7de7ae10daca9df0699955d`
- Documentation commit: this document's commit is recorded in the final handoff
- Final `HEAD`: recorded after push
- `origin/master`: recorded after push
- `v0.5.0` peeled SHA remains `02e67cff50f5da1d207478071636af166048820c`
- `v0.4.0` peeled SHA remains `94918f6322ce690ff7b1630961abb56b8a31ed11`

## Final Decision

`DEV-030 POST-0.5.0 STABILITY COMPLETE`

Completion is with the explicit Windows Installer limitation documented in
sections 6–8. No confirmed P0/P1 remains; MSI registration and a real
installer-backed v0.4→v0.5 preservation run must be rechecked in a Windows
Installer-capable environment before treating those two validation dimensions
as independently complete.

NEXT: `DEV-031 — Failed Batch Partial Resume`

DEV-031 is frozen as the next task and was not started automatically.
