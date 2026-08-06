# M1 Cancel and Recovery Validation

Date: 2026-08-06  
AI Studio commit: `f80e25d6291704d2a819ad59371dc5b47d38e17e`  
ComfyUI version: 0.30.1

## Automated validation

- `cargo fmt --all -- --check`: PASS
- `cargo check`: PASS
- `cargo test`: PASS (150 tests)
- `pnpm test`: PASS (7 tests)
- `pnpm build`: PASS
- Local-only Recovery with ComfyUI offline: PASS
- Mixed local/external offline Recovery: PASS
- Multiple external tasks share one `health_check`: PASS
- Recovery `/prompt` POST calls: 0
- Recovery `/upload/image` calls: 0
- Automatic retry/resubmit: NO

## Recovery semantics

- `CREATED`, `VALIDATING`, and `PREPARING` without a prompt are failed locally with `APP_RESTARTED_BEFORE_SUBMISSION`.
- `CANCEL_REQUESTED` without a prompt is completed locally as `CANCELLED`.
- Active states without a prompt that require external evidence remain unchanged and record `ACTIVE_TASK_MISSING_PROMPT_ID`.
- Submitted tasks are checked against ComfyUI only after local tasks are reconciled.
- Offline submitted tasks remain unchanged and record `TASK_RECOVERY_DEFERRED` with `COMFY_OFFLINE`.

## Cancel validation

- Modern prompt-specific cancel: PASS in Mock HTTP tests.
- Legacy pending cancel: PASS in Mock HTTP tests.
- Legacy running cancel: PASS in Mock HTTP tests.
- Blind interrupt protection: PASS in Mock HTTP tests.
- Cancel too late preserves successful output: PASS in worker tests.

## Live validation

- M0 T2I Regression: PASS (`QUEUED → RUNNING → COLLECTING → SUCCEEDED`, one generated Asset).
- Running Cancel: PASS. The task reached `RUNNING`, the UI Cancel action produced `CANCEL_REQUESTED → CANCELLED`, and the final state was not `FAILED`.
- Modern/Legacy route used: Modern `POST /api/jobs/{prompt_id}/cancel` on ComfyUI 0.30.1; no Legacy fallback was needed.
- Restart Recovery: PASS. AI Studio was closed while ComfyUI kept the task running, then restarted. The same Task and Prompt were recovered as `RUNNING` with `TASK_RECOVERY_STARTED → TASK_RECOVERY_SUCCEEDED` and no second prompt. After ComfyUI completed, manual Reconcile produced one Asset and `SUCCEEDED`.

The validation record intentionally excludes complete prompts, workflow JSON, private absolute paths, and database contents.
