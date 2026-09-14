# AI Studio v2 Architecture Freeze Audit

```makefile
VERSION=v2.0.0-personal
ARCHITECTURE_STATUS=FROZEN
DATA_MODEL=STABLE
MIGRATION_STATUS=STABLE
BACKUP_VERSION=19
PROVENANCE=STABLE
EXECUTION_CONTROL=STABLE
```

## Freeze decision

The v2 Personal Edition architecture is frozen at the existing additive
boundaries. The release contains no new execution engine, queue, task model,
generation authority, or alternate asset/result store.

## Boundary audit

| Area | Frozen authority | Result |
| --- | --- | --- |
| Production Core | Project, Shot, Task, Queue, Review, GenerationService, and Comfy execution admission | Stable; Queue Start remains the only production gate |
| Asset Library | Existing `assets` plus AssetVersion, AssetRelation, and explicit provenance extensions | Stable; project isolation and immutable history retained |
| Prompt Studio | Existing Prompt/PromptVersion plus Model/ModelVersion registry | Stable; no second prompt or generation system |
| Local Tool Hub | Tool, ToolInstance, ToolVersion, and Capability metadata | Stable; visibility only, no process control |
| Project Archive | Existing v19 logical ZIP snapshot/export/restore path | Stable; exact ID maps, media integrity checks, and visible UNKNOWN state |
| Frontend boundary | Typed Tauri transport and existing feature services | Stable; components do not access SQLite directly |
| Persistence boundary | Rust repository ports backed by SQLite migrations | Stable; historical Project/Shot/Task/Queue/Review rows preserved |

## Data and migration status

The current migration chain is additive through the v2 data-layer migrations.
Existing v1.3.1 databases remain on the same upgrade path; fresh databases run
the complete chain. No migration in this freeze removes or renames historical
production tables or repairs relationships heuristically.

## Provenance and execution controls

The explicit supported lineage is:

```text
ToolInstance / ToolVersion
        ↓
Generation(Task) ← PromptVersion ← ModelVersion
        ↓
GenerationAssetVersion
        ↓
AssetVersion → Asset
```

Comfy admission is held until the real execution reaches a terminal state.
Success, failure, cancellation, timeout, submit errors, disconnect handling,
and application shutdown paths do not create a permanent permit leak.

## Freeze exclusions

The following remain outside the frozen architecture and require a new product
decision before implementation:

- autonomous Agents or automatic decisions;
- cloud synchronization or multi-user permissions;
- a new executor, workflow engine, or queue;
- heuristic historical lineage repair; and
- a schema rewrite or replacement of the current archive contract.

## Audit conclusion

```text
ARCHITECTURE_FREEZE=PASS
NO_NEW_DOMAIN_AUTHORITY=PASS
QUEUE_AUTHORITY=PASS
MIGRATION_COMPATIBILITY_BOUNDARY=PASS
ARCHIVE_CONTRACT=BACKUP_V19_STABLE
```
