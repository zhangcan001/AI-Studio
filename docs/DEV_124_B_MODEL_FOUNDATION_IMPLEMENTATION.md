# DEV-124-B — Prompt Studio Model Foundation Implementation

## Status

```text
TASK=DEV-124-B
VERSION=AI_STUDIO_v1.3.1_STABLE
CODE_CHANGE=YES
FRONTEND_CHANGED=NO
ADDITIVE_MIGRATION=YES
MODEL_AUTHORITY=CANONICAL
```
DEV-124-B adds the canonical metadata registry that Prompt Studio needs for
external image, video, TTS, and music models. It does not execute models or
change the Production Core execution boundary.

## Scope and non-goals

Implemented:

- `Model` and `ModelVersion` domain entities.
- SQLite persistence, validation, service methods, and Tauri commands.
- Optional `ModelVersion` provenance on prompt versions and generation
  snapshots.
- Migration and compatibility tests.

Explicitly not implemented:

- A new executor, queue, task, or generation pipeline.
- AI prompt optimization, automatic generation, or model installation.
- A second Prompt, Generation, Result, or Asset authority.
- Frontend screens. The backend command boundary is ready for a later typed
  Prompt Studio UI, but this foundation does not require a UI change.

## Migration

`src-tauri/migrations/034_prompt_studio_model_foundation.sql` is additive.

It creates:

### `models`

The personal model registry. It stores the external model name, provider,
model type, description, JSON metadata, and creation time. The provider/name
pair is unique so the same external model is not registered twice.

### `model_versions`

Versioned external model metadata. It stores the owning model, provider-facing
version string, JSON capabilities, JSON parameter schema, and creation time.
The version value remains text because providers use different version
formats (`1.0`, `2026-01`, release names, and similar values).

The migration also adds nullable `model_version_id` foreign keys to:

- `prompt_versions` — `ON DELETE SET NULL`.
- `generation_snapshots` — `ON DELETE SET NULL`.

Both columns have lookup indexes. Existing rows and existing callers retain a
null value. No existing Project, Shot, Task, Queue, Review, Asset, or Result
table is removed or rewritten.

## Domain and lifecycle rules

- `ModelId` uses the `mdl_` namespace.
- `ModelVersionId` uses the `mdv_` namespace.
- Model identity fields and version strings are trimmed, non-empty, and
  single-line at the service boundary.
- Metadata, capabilities, and parameter schemas must be non-null JSON values.
- `current_version` is the newest recorded version by `created_at` and ID;
  semantic ordering is deliberately not inferred from provider version text.
- Model versions are append-only from the service API. There is no delete
  command for a version.
- A model with versions cannot be deleted (`ON DELETE RESTRICT`), preserving
  historical provenance. Prompt and generation references are nullable so
  removing a model version in a future explicit maintenance workflow would
  not invalidate historical records.

## Rust implementation

The existing repository/service architecture is preserved:

```text
src-tauri/src/domain/model.rs
src-tauri/src/application/ports/model_repository.rs
src-tauri/src/infrastructure/database/repositories/model.rs
src-tauri/src/application/model_service.rs
src-tauri/src/commands/model.rs
```

The application composition root wires one `SqliteModelRepository` into the
`ModelService` and into `GenerationService` for production-time provenance
validation. Commands expose list/get/create/update/delete operations for
models and list/current/get/create operations for model versions. Persistence
continues to go through repository ports; no service writes directly to the
database.

## Prompt integration

The existing `prompt_entries` and `prompt_versions` remain the Prompt
authority. The existing service methods keep their original signatures and
store `NULL` model provenance. New internal/service and command request paths
can optionally supply a `ModelVersionId`. The repository persists and reads
that exact ID through the new nullable foreign key.

No duplicate Prompt model or Prompt Studio storage is introduced.

## Generation integration

The existing `Task` + `GenerationSnapshot` + Result/Asset mapping path remains
authoritative. Generation requests may carry an optional `model_version_id` as
metadata. In the production composition, the configured model repository
rejects an unknown model version before Task creation. When a generation
snapshot is written, it stores the optional `ModelVersionId`; existing queue
callers continue to pass `None` and Queue Start remains the only execution
gate.

No executor, queue scheduling, task state machine, review behavior, or result
mapping was changed.

## Tests

Coverage added or updated:

- Fresh and repeated migrations create the two model tables and both nullable
  provenance columns.
- A legacy database applying migration 034 preserves Project, Asset, Shot,
  Task, and Review rows and does not backfill model data.
- Model repository CRUD, deterministic listing, model version listing/current
  lookup, duplicate protection, and deletion restriction.
- Model service validation, CRUD, version creation, and current-version query.
- Prompt version model provenance round-trip.
- Generation snapshot model provenance round-trip.
- Existing migration compatibility fixtures now account for migration 034.

The frontend was not changed because this phase establishes the data
foundation and backend boundary only.

## Compatibility assessment

```text
DATA_LAYER=COMPLETE
MODEL_AUTHORITY=CANONICAL
PROMPT_INTEGRATION=COMPATIBLE
GENERATION_PROVENANCE=ADDED
RUST_CHANGED=YES
FRONTEND_CHANGED=NO
QUEUE_CHANGED=NO
TASK_CHANGED=NO
REVIEW_CHANGED=NO
```
