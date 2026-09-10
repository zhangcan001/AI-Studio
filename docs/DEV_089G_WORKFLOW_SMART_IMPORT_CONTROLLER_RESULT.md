# DEV-089G — Workflow Smart Import Controller Result

## Summary

DEV-089G extracted the WorkflowWorkspace Smart Import session into
`useWorkflowSmartImportController` while preserving the existing workflow
resolution and import lifecycle semantics.

```text
DEV-089G=PASS
DEV-089=OPEN
```

## Commits

```text
Baseline: b07486d202933c1e1b6816cd4f4bbef6bc1f73f5
Implementation: 82c1fc3f3a21571f7cf118736f8809e6cac9bb56
Implementation message: refactor(workflows): extract smart import controller
```

The implementation commit was pushed to `origin/master` and passed Source-only
CI run `34431993762`.

## Workspace decomposition

`WorkflowWorkspace.tsx` decreased from 1845 to 1704 lines.

`useWorkflowSmartImportController` owns the Smart Import session API and state:

- analyze remains read-only; commit is explicit;
- draft replacement is cleaned up before replacement analysis;
- resume, regenerate, and issue resolution retain their existing behavior;
- exact workflow identity continues to use `workflowVersionId + recipeId`;
- advanced import, existing workflow, and existing version routing remain in
  the composition layer;
- the existing onboarding store remains the only onboarding state source.

`workflowSmartImportModel.ts` contains the extracted pure error and version
resolution helpers. No new store, persistence layer, IPC command, or runtime
profile parameter was introduced.

## Validation

```text
Focused controller and WorkflowWorkspace regressions: PASS (20 tests)
Full frontend tests: PASS (136 files, 652 tests)
TypeScript: PASS
Frontend build: PASS
Architecture guard: PASS
Rust check: PASS
Rust tests: PASS (796 passed, 1 ignored)
Tauri build: PASS
Remote Source-only CI: GREEN (34431993762)
```

Architecture guard confirmed:

```text
WORKFLOW_SMART_IMPORT_CONTROLLER=PASS
FRONTEND_NO_RAW_INVOKE=PASS
RAW_INVOKE_OUTSIDE_TRANSPORT=0
RPC_PARITY=PASS
SHOT_WORKSPACE_* sentinels=PASS
ASSET_VIDEO_* sentinels=PASS
```

## Frozen invariants

```text
ANALYZE_READ_ONLY=YES
EXPLICIT_COMMIT=YES
EXACT_WORKFLOW_IDENTITY=PRESERVED
RESUME_REGENERATE_ISSUE_RESOLUTION=PRESERVED
NO_LIFECYCLE_BEHAVIOR_CHANGE=YES
NO_PARAMETER_EXPOSURE_CHANGE=YES
NO_RUNTIME_PROFILE_CHANGE=YES
RUST_CHANGE=NO
IPC_CHANGE=NO
DATABASE_CHANGE=NO
CSS_CHANGE=NO
```

`DEV-089G=PASS` and `DEV-089=OPEN`.
