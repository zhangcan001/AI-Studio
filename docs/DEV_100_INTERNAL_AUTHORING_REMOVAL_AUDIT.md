# DEV-100 Internal Authoring Removal Audit

## Scope

This audit classifies narrative, storyboard, prompt-template, and structured-import code before the DEV-100 removal. The product boundary is deliberately narrower than the historical 0.8 narrative-preproduction design:

```text
External structured production input -> explicit formal production data -> existing production execution path
```

`Script/Draft` and prompt-template generation are not active product authorities after this task. Historical schema and Backup v17 compatibility remain protected.

## User-visible audit

```text
NARRATIVE_UI=ALREADY_NOT_USER_VISIBLE
SCRIPT_IMPORT_UI=ALREADY_NOT_USER_VISIBLE
STORYBOARD_DRAFT_UI=ALREADY_NOT_USER_VISIBLE
PROMPT_TEMPLATE_UI=REMOVED
```

No dedicated Narrative, Script Import, or Storyboard Draft workspace exists in
the current navigation. The prompt-template controls that were embedded in
Prompt Library, Shot Inspector, and production planners were active user-facing
surfaces and were removed.

## Classification

| PATH | CURRENT_PURPOSE | CALLERS | PERSISTENCE_DEPENDENCY | PRODUCTION_DEPENDENCY | DECISION | REASON |
| --- | --- | --- | --- | --- | --- | --- |
| `src-tauri/src/application/script_draft_service.rs` | Script source and immutable draft revision application service | DEV-057/058 tests and the unregistered parser service only | Migration 025 tables through the retired repositories | None; no AppState or command registration | `DELETE` | No active runtime caller and no formal production dependency. |
| `src-tauri/src/application/script_import_service.rs` | Deterministic script preview/create/reparse orchestration | DEV-058 tests only | Migration 025 through ScriptDraftService | None | `DELETE` | Internal narrative authoring is retired. |
| `src-tauri/src/application/script_import_parser/**` | Script/novel parsing and draft generation | ScriptImportService and parser tests only | No direct persistence | None | `DELETE` | Parser/generation is explicitly out of product scope. |
| `src-tauri/src/domain/script_draft/**` | Script, draft, storyboard suggestion, diagnostics, and source-span contracts | Retired application services/tests only | Payloads are stored as legacy JSON, but Backup validates/remaps them without these Rust types | None | `DELETE` | No active code requires the typed authoring domain; legacy JSON remains opaque compatibility data. |
| `src-tauri/src/application/ports/script_*_repository.rs` | Repository ports for Script Source/Draft | Retired services/tests only | Migration 025 tables | None | `DELETE` | No active application boundary uses them. |
| `src-tauri/src/infrastructure/database/repositories/script_source.rs` | SQLite Script Source repository | Retired services/tests only | Migration 025 | None | `DELETE` | No active runtime repository consumer. |
| `src-tauri/src/infrastructure/database/repositories/script_draft.rs` | SQLite immutable draft repository | Retired services/tests only | Migration 025 | None | `DELETE` | No active runtime repository consumer. |
| `src-tauri/migrations/025_script_draft_foundation.sql` | Published Script/Draft schema | Migration runner and legacy upgrade fixtures | Historical schema/checksum | None | `LEGACY_SCHEMA_ONLY` | Published migration is immutable; dropping tables would break upgrade/rollback compatibility. |
| `src-tauri/src/application/project_backup_service.rs` Script fields | Backup v17 document fields and validation | Backup export/inspect/restore | Backup v15+ compatibility and legacy payload remapping | None | `KEEP` | Backup must continue to read and restore old Script/Draft records. |
| `src-tauri/src/infrastructure/database/repositories/project_backup.rs` Script queries | Backup v17 SQL snapshot and restore rows | Project backup repository | Directly reads migration-025 tables | None | `KEEP` | Removing these rows would make legacy backups fail. |
| `src-tauri/tests/dev057_script_draft_data.rs` | Script/Draft persistence contract tests | Test-only | Migration 025 | None | `DELETE` | Tests the retired active authoring capability, not compatibility. |
| `src-tauri/tests/dev058_script_import_parser.rs` | Parser/reparse/draft authoring tests | Test-only | Migration 025 | None | `DELETE` | Tests the retired active authoring capability. |
| `src-tauri/src/application/prompt_template_service.rs` | Template parsing, variable interpolation, and prompt generation | Prompt-template bulk service and DEV-036 tests | Prompt library text only | Writes stage prompt snapshots through the bulk service | `DELETE` | This is internal prompt authoring/generation, not supplied production input. |
| `src-tauri/src/application/prompt_template_bulk_service.rs` | Context-aware single/scene/episode/series template rendering and application | Prompt-template commands and production UI | Prompt library and stage prompt tables | Mutates formal stage prompt inputs, but only after internal generation | `DELETE` | Remove the authoring/generation path; raw/manual/external prompt input remains supported elsewhere. |
| `src-tauri/src/commands/prompt_template.rs` | Tauri IPC for analyze/preview/apply template operations | Frontend template UI only | None | Reaches the retired template services | `DELETE` | Retired IPC must be unavailable. |
| `src-tauri/src/domain/prompt_template.rs` | Template parser/render context types | Retired template services/tests | None | None | `DELETE` | No remaining active consumer after template removal. |
| `src/features/prompts/PromptTemplateVariableHelper.tsx` and `promptTemplateState.*` | Template detection, variable authoring, and helper chips | Prompt library and production panels | None | None | `DELETE` | Frontend authoring surface. |
| `src/features/shots/PromptTemplatePanel.tsx` | Template preview and apply UI | Shot workspace | None | Applies generated text to formal prompts | `DELETE` | Internal prompt authoring surface. |
| Template sections in `SeriesProductionPanel`, `EpisodeProductionPanel`, `SceneProductionPanel`, and `ShotWorkspace` | Context-aware prompt template preview/apply | Production workspaces | Stage prompt tables | Mutates production prompt snapshots through generated text | `DELETE` | Keep the production planners and manual prompt input; remove only template generation controls. |
| `src/features/prompts/PromptLibraryPanel.tsx` | Save, version, compare, and apply plain prompt/snippet text | Production input workflows | Prompt library | Supplies user-selected text to formal inputs | `KEEP` | Manual/external prompt input and history are production data, not an AI author. |
| `src-tauri/src/application/prompt_library_service.rs` and prompt-library IPC | Persist prompt text, versions, and provenance | Prompt library UI and bulk prompt assignment | Prompt tables | Formal prompt input binding | `KEEP` | Required for supplied production prompts and history. |
| `src-tauri/src/application/asset_video_prompt_service.rs` | Persist asset video production prompt text | Asset commands/UI | Asset video prompt table | Production input data | `KEEP` | Name is not sufficient evidence of authoring; this service stores supplied production input. |
| `src-tauri/src/application/shot_bulk_service.rs` and `src/features/projects/ProjectImportDryRunWorkspace.tsx` | Preview and atomically import flat formal Shot production inputs | Existing project import UI and Shot workspace | Formal `shots` and stage prompt tables | Formal Shot creation only; no queue/task/Comfy call | `KEEP_AND_RENAME` | Existing formal import boundary is safe and reusable, but its current schema is not a full Episode/Scene hierarchy handoff. |
| `src-tauri/src/commands/shot_bulk.rs` and typed client methods | IPC for formal Shot import and raw prompt assignment | Existing import/production UI | Formal production tables | No automatic execution | `KEEP` | Preserve the only existing safe formal import authority. |
| `src-tauri/src/domain/production_structure.rs` and production services | Formal Project/Series/Episode/Scene/Shot hierarchy | Production structure and preparation workspaces | Formal production tables | Required by production execution | `KEEP` | External agents hand off into this authority; it is not storyboard draft state. |
| `src-tauri/src/application/generation_service.rs`, queue/task/Comfy paths | Formal production execution | Existing production UI | Production package, queue, task, snapshot tables | Execution authority | `KEEP` | Core product remains production management/execution. |
| `src-tauri/src/migrations/**` | Published schema history | Migration runner and compatibility tests | All historical schemas | Indirect | `KEEP` | No migration is added or edited. |
| `docs/architecture/NARRATIVE_PREPRODUCTION_V2.md`, `SCRIPT_IMPORT_V1.md`, `STORYBOARD_DRAFT_V1.md` | Historical narrative authoring design evidence | Documentation only | None | None | `KEEP_AND_RENAME` | Retain evidence, mark retired by DEV-100, and point to external-agent handoff. |

## External-agent handoff finding

The existing `ShotBulkService` has the correct preview-before-write, project-scoped, all-or-nothing formal Shot boundary and accepts supplied image/video prompt text. It does **not** accept a versioned Episode → Scene → Shot document, exact asset validation, exact workflow-version/recipe validation, or a single transaction spanning hierarchy plus shots. Existing structure commands are individual mutations rather than a server-side atomic import boundary. Adding that boundary safely would require a dedicated application contract and persistence/idempotency decision beyond this removal-only train.

Therefore DEV-100 freezes the provider-neutral handoff contract and records:

```text
EXTERNAL_HANDOFF_IMPLEMENTATION=BLOCKED_BY_SCHEMA
HANDOFF_IMPLEMENTATION_BLOCKED_BY_SCHEMA=YES
NEW_IMPORT_IMPLEMENTATION=NO
```

The current formal Shot import remains available and is not relabeled as a complete `ProductionHandoffV1` implementation.

## Compatibility decisions

- Migration 025 and every published migration remain byte-for-byte unchanged.
- `script_sources` and `script_import_drafts` remain inert legacy schema.
- Backup v17 script/draft export, inspect, validation, restore, and ID remapping remain intact.
- No new migration 032 is added.
- No queue, executor, task model, or ComfyUI path is added or duplicated.
- No active runtime reference to the retired Script/Draft/parser/template authoring code remains after cleanup; historical docs, migration SQL, and explicit backup compatibility code are the only allowed legacy references.
