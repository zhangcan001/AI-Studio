# DEV-128-C — Provenance Lineage Integration & Visibility

## Scope

DEV-128-C exposes the explicit cross-module provenance records introduced by DEV-128-B in the existing Asset, Prompt, and Task detail surfaces. This is a read-only frontend integration. It does not create, infer, repair, or execute any relationship.

## Authority and transport

- `Task` remains the generation authority; `generationId` is the existing task ID.
- `Asset`, `AssetVersion`, `Prompt`, `ModelVersion`, `Tool`, and `ToolVersion` remain their existing authorities.
- The frontend uses the typed wrappers in `src/services/tauriClient.ts`:
  - `listGenerationToolUsages(projectId, generationId)`
  - `listGenerationAssetVersionLinks(projectId, generationId)`
- Components do not access SQLite or call raw Tauri `invoke`.
- `projectId` is passed on every lineage read so the backend's project isolation remains in force.

## Visibility changes

### Asset detail — `src/features/assets/AssetPreview.tsx`

The existing source section now includes a read-only `Generation History` view with:

- `Prompt Version`
- `Model Version`
- `Tool Version`
- `Source Task`
- explicit `Tool Usage` records
- explicit `Asset Versions` records

Prompt and model values are shown as unavailable when the current Asset/Task transport does not contain a direct historical relation. Tool and asset-version values are shown only when returned by `ProvenanceLineageService`.

### Prompt detail — `src/features/prompts/PromptStudio.tsx`

The prompt detail view now includes read-only cross-module panels for:

- `Used Generations`
- `Generated Assets`
- `Model Versions`
- `Tools`

The existing project task history remains separate from direct prompt usage. Because the current data layer has no `PromptVersion → Generation` relation, the UI labels project task history as contextual and does not claim that those tasks used the selected prompt. Missing PromptVersion links and missing Generation → AssetVersion/Tool Usage links are shown explicitly.

### Generation detail — `src/features/tasks/TaskHistoryDetail.tsx`

Task detail now renders:

- `Tool Usage`
- `Asset Versions`
- `Provenance Timeline`

The timeline contains only the source task event, returned tool-usage events, returned asset-version events, and the existing task completion timestamp. It does not infer a Prompt, Model, Tool, or Asset relationship from names, paths, or prompt text.

## State handling

All three surfaces handle:

- loading while lineage reads are pending;
- empty provenance for older tasks/assets without explicit links;
- missing historical links without converting them into false relations;
- read failures through the existing user-facing error conversion.

The lineage effects use an active-request guard so late responses cannot overwrite a different asset, prompt project, or task detail.

## Tests

- `src/features/assets/AssetPreview.test.tsx`
  - explicit tool usage and AssetVersion rendering;
  - missing PromptVersion and ModelVersion state.
- `src/features/prompts/PromptStudio.test.tsx`
  - cross-module provenance rendering;
  - missing-link/empty state;
  - typed project-scoped lineage calls.
- `src/features/tasks/TaskHistoryDetail.provenance.test.tsx`
  - tool usage, AssetVersion, and timeline rendering;
  - empty provenance;
  - lineage error while preserving task detail.

## Compatibility boundary

- `QUEUE`, `TASK` creation, review, and production execution paths are unchanged.
- No automatic linking or backfill was added.
- No new Generation, Result, Asset, Prompt, Model, or Tool domain was created.
- Rust code and the database schema are unchanged in DEV-128-C.

## Validation record

| Check | Result |
| --- | --- |
| `pnpm test` | PASS |
| `pnpm exec tsc --noEmit` | PASS |
| `pnpm build` | PASS |
| Rust changed | NO |
| Rust test | NOT RUN — no Rust changes |
| Remote Source-only CI | NOT REQUESTED for this frontend-only task |
