# DEV-113 — Release Candidate Technical Debt Register

This is an observation-only register. Only P2/P3 debt is recorded; no debt below was fixed in DEV-113.

## FRONTEND

### FE-113-01 — First-run wayfinding (P2)

The shell has a strong global rail and project context, but the first-run experience does not identify a producer-oriented starting sequence. The implementation is safe and navigable; the debt is discoverability, not a missing capability. Likely areas for a later pass: `src/app/StartupScreen.tsx`, `src/app/App.tsx`, and the empty project state in `src/features/projects/ProjectCommandCenter.tsx`.

### FE-113-02 — Split delivery continuation (P2)

Review, monitor, Asset, Shot, and Task each expose useful exact links, but final accepted-result confirmation is distributed across those existing surfaces. A future UX pass should compose the path without introducing another execution authority. Relevant surfaces include `src/features/production/ProductionMonitor.tsx`, `src/features/production/ProductionReviewInbox.tsx`, `src/features/assets/AssetPreview.tsx`, and `src/features/assets/AssetUsagePanel.tsx`.

### FE-113-03 — Diagnostic vocabulary and tab hierarchy (P3)

Technical terms such as Production Package, Asset Usage, Parse / Validate, Preview, WorkflowVersion, and Recipe remain visible. The public production state language is correct; this is terminology polish and should not remove technical details needed for diagnosis.

## RUST

No new P2/P3 Rust defect was found in this audit. Existing repository ports, project scoping, exact workflow identity, and queue authority remain consistent with the prior closeout evidence. No Rust source was changed.

## DATABASE

No new P2/P3 database defect was found. DEV-113 introduced no schema or migration. Historical task, production, review, and asset references remain part of the existing continuity model.

## BACKUP

### BK-113-01 — Backup is separate from the production entry story (P3)

Project backup/restore is available in project management and truthfully creates a new project without auto-generation, but it is not part of the first-time production wayfinding narrative. This is acceptable for 1.3 and should remain separate from the production queue contract.

## CI

### CI-113-01 — Full source gate is manual for documentation-only changes (P2)

`.github/workflows/ci.yml` runs automatically for tags and non-documentation pull-request paths, and supports `workflow_dispatch`; a documentation-only push therefore needs an explicit exact-head dispatch for release evidence. This is operationally workable but slower and easier to omit than an automatic release-candidate check.

### CI-113-02 — Existing Rust cancellation test has historical flake evidence (P2)

DEV-112 recorded one initial Source-only CI failure in an existing cancellation E2E test, followed by a passing retry and a passing final exact-head run. No current failure was observed in the local frontend gate, and this audit does not change the workflow or test behavior. Repeated failures should be triaged in a dedicated reliability task rather than hidden by retries.

CI audit judgment:

```text
FAST_ENOUGH=PARTIAL — frontend feedback is quick; a cold Windows Rust gate is materially longer
STABLE=PARTIAL — recent evidence is green but includes one historical flaky retry
RELIABLE=YES_WITH_EXACT_HEAD_DISPATCH — the complete source gate is reproducible when explicitly dispatched and verified against HEAD
WORKFLOW_CHANGED=NO
```

## PERFORMANCE

### PERF-113-01 — Main frontend bundle warning (P3)

`pnpm build` succeeds but Vite reports a main minified chunk above the default 500 kB warning threshold. This is not a production-flow blocker in the audit; defer code splitting or bundle work until measured user impact justifies it.

### PERF-113-02 — Large collection cognitive load (P2)

The app correctly bounds 500-shot and 500-item displays, but users must understand multiple bounded surfaces, filters, and pages. This is intentional safety behavior with a remaining explanation/wayfinding cost, not evidence for an unbounded renderer or a second search engine.

## DOCUMENTATION

### DOC-113-01 — Release metadata lag (P2)

The current package/README language still identifies the product as 1.2.0 while the repository is auditing the 1.3 release candidate. This audit records the mismatch but does not alter release metadata because DEV-113 is not a release-versioning task.

### DOC-113-02 — Producer journey is distributed across historical closeouts (P3)

DEV-108 through DEV-112 document the individual hardening outcomes, while a first-time producer needs one concise journey map. This document and the companion friction register provide the audit record; a future product decision can choose whether to publish a user-facing guide.

## Release debt summary

```text
P0=0
P1=0
P2=6 technical-debt entries
P3=4 technical-debt entries
CODE_OR_SCHEMA_CHANGES=NONE
```

The complete user-friction register has the journey-level count `P2=9` and `P3=2`; this technical-debt document is a separate category register and intentionally records additional operational/documentation observations.

The register is intentionally non-blocking. Do not start DEV-114 automatically from these observations.
