# DEV-124-C — Prompt Studio Frontend MVP Implementation

```text
TASK=DEV-124-C
VERSION=AI_STUDIO_v1.3.1_STABLE
FRONTEND_CHANGED=YES
RUST_CHANGED=NO
PROMPT_STUDIO=READ_ONLY
QUEUE_START_UNCHANGED=YES
```

DEV-124-C adds a project-scoped, read-only Prompt Studio workspace. It exposes
the existing Prompt Library, the canonical Model Registry from DEV-124-B, and
the existing project Task History without creating a second Prompt, Generation,
Result, Task, or execution system.

## 1. Page and navigation

The new `prompts` workspace is available from the global Studio rail as
`提示词`. It is wired through the existing state-driven workspace navigation;
no router or production navigation path was introduced.

The page is rendered by:

```text
src/features/prompts/PromptStudio.tsx
```

and is loaded lazily by `src/app/App.tsx`. The page displays the active project
scope so that Prompt Library and Task History reads remain project-scoped.

## 2. Prompt list

The list uses the existing `listPromptLibrary` typed transport and supports the
existing Prompt Library filters:

- Prompt kind (`提示词` or `片段`)
- Name/tag keyword
- Tag
- Pagination through the existing page cursor

Each row displays the required MVP fields:

```text
Name
Type
Current Version
Model / Provider + ModelVersion
Updated Time
```

The current model label is resolved from the latest Prompt Version's optional
`modelVersionId`, then through the canonical Model Registry. An unbound or not
yet resolved model is shown explicitly rather than inferred from a name.

## 3. Prompt detail and version history

Selecting a Prompt loads its existing Prompt Library detail and renders:

- The selected Prompt Version's template text.
- Version metadata and a reverse chronological version history.
- A clickable version selector that never mutates or overwrites history.
- The linked Model Version when the Prompt Version has one.
- The Model Version's parameter schema.
- A clear empty state when reference associations are not present.

The MVP deliberately has no edit, delete, save, generation, or automatic
optimization action. Existing Prompt Library editing remains in its existing
surface and remains the single Prompt authority.

## 4. Model view

The model panel reads the DEV-124-B Model Registry through typed commands and
shows:

```text
Model
Provider
Type
Description
Model Versions
Capabilities
Parameter Schema
```

Model versions are displayed as the registry returns them. Provider version
strings are treated as opaque text; the UI does not infer semantic ordering or
replace the backend's current-version rule.

## 5. Provenance and generation history

The detail view presents the requested explainability sequence:

```text
Prompt Version
      ↓
Model Version
      ↓
Generation Snapshot
      ↓
Result Asset
```

The existing `taskHistoryPage` typed transport supplies the project generation
history and output counts. This is intentionally described as existing Task /
Result authority. The current data model does not store a direct
PromptVersion-to-GenerationSnapshot or PromptVersion-to-Reference relation, so
the UI does not invent one or claim that every project task was produced by the
selected Prompt Version. It shows the available Model Version provenance and
links the remaining steps to the existing task history/Result path.

References are likewise shown as an explicit empty state until an existing
ReferenceAnchor/ReferenceSet binding is actually available in the Prompt
Library contract. No parallel reference table or frontend reference authority
was added.

## 6. Typed data access

The page uses only the existing typed IPC boundary:

```text
src/services/tauriClient.ts
  listPromptLibrary
  getPromptLibraryEntry
  listModels
  listModelVersions
  taskHistoryPage
```

The DEV-124-C additions to the typed client are the Model and ModelVersion
views/reads. Components do not call raw Tauri `invoke` and do not access SQLite.

## 7. State and accessibility behavior

Separate loading, error, and empty states are provided for:

- Prompt list
- Prompt detail
- Model registry
- Generation history

The table uses semantic column/row headers. Interactive rows and version
buttons expose pressed state, and the workspace/panels expose labels and busy
status for assistive technology.

## 8. Files changed

### Frontend implementation

- `src/features/prompts/PromptStudio.tsx`
- `src/features/prompts/PromptStudio.test.tsx`
- `src/types/model.ts`
- `src/types/prompt.ts`
- `src/services/tauriClient.ts`
- `src/app/App.tsx`
- `src/app/App.css`
- `src/app/StudioShell.tsx`
- `src/app/studioNavigation.ts`
- `src/types/workspaceResume.ts`
- `src/components/studio/StudioGlobalRail.tsx`

### Tests and guards

- `src/app/studioNavigation.test.ts`
- `src/components/studio/StudioGlobalRail.test.tsx`
- `src/i18n/ui-localization.test.tsx`

No Rust, SQLite migration, Queue, Task, Review, production execution, or
existing Prompt Library authority files were changed.

## 9. Verification

```text
PromptStudio + navigation + localization tests = PASS
Frontend test suite (152 files / 832 tests) = PASS
pnpm exec tsc --noEmit = PASS
pnpm build = PASS
RUST_CHANGED = NO
```

## 10. Known MVP limitations

- Prompt detail currently shows the existing project Task History as the
  generation-history view; exact Prompt Version filtering will require an
  explicit provenance field in a future data-layer change.
- Prompt reference associations are not part of the current Prompt Library
  view contract, so the page reports their absence instead of duplicating the
  existing ReferenceAnchor/ReferenceSet system.
- The first read path resolves model labels by reading Prompt details for the
  returned list page. This is intentionally kept within the MVP's existing
  typed APIs; a future list projection can remove that additional read without
  changing authority boundaries.

These limitations do not change the Production Core execution path. Queue
Start remains the only execution gate, and DEV-124-C performs no automatic
generation.
