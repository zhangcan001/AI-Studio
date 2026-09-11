# DEV-100 External Agent → AI Studio Production Handoff

```text
STATUS=FROZEN
AI_STUDIO_SCRIPT_AUTHORING=NO
AI_STUDIO_STORYBOARD_AUTHORING=NO
AI_STUDIO_PROMPT_AUTHORING=NO
EXTERNAL_AGENT_HANDOFF=YES
HANDOFF_TARGET=FORMAL_PRODUCTION_STRUCTURE
IMPORT_PREVIEW=YES
IMPORT_CONFIRM=EXPLICIT
IMPORT_AUTO_QUEUE=NO
IMPORT_AUTO_TASK=NO
IMPORT_AUTO_GENERATION=NO
PROMPT_AS_PRODUCTION_INPUT=YES
FORMAL_EXECUTOR=COMFYUI
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
HANDOFF_IMPLEMENTATION_BLOCKED_BY_SCHEMA=YES
```

## Product boundary

AI Studio manages and executes AI production from structured production
inputs. Narrative, screenplay, storyboard, prompt generation, prompt rewrite,
and prompt-template authoring are external-agent responsibilities. AI Studio
accepts the resulting production inputs; it does not turn them into a second
creative authoring product.

The retained application path is:

```text
external agent or manual operator
  → explicit project-scoped production input
  → preview and validation
  → explicit confirmation
  → formal Series / Episode / Scene / Shot records
  → existing preparation and Production Queue
  → existing Task / ComfyUI executor
```

Prompt text remains a production input. AI Studio may display, edit manually,
snapshot, version, bind, and persist prompt text supplied for a formal Shot;
that is not prompt authoring or prompt generation.

## Existing authorities that remain

- `ShotBulkService` remains the existing flat JSON/TSV formal Shot import
  boundary. It validates a preview, then atomically inserts supplied Shot
  descriptions and stage prompt inputs after explicit confirmation.
- Project, Episode, Scene, Shot, Asset, Prompt Library, Production Package,
  Snapshot, Workflow/Recipe, Queue, Task, and ComfyUI authorities remain
  unchanged and project-scoped.
- The exact workflow identity remains the pair
  `workflowVersionId + recipeId`; names are never used as identity.
- Backup v17 keeps its historical Script/Draft restore data. The legacy
  `script_sources` and `script_import_drafts` tables remain inert compatibility
  schema and published migrations are not edited.

## Why the full handoff is not implemented in DEV-100

The current formal import service accepts a flat list of Shots. The codebase
does not yet have one import contract that can atomically create or reconcile a
complete Episode → Scene → Shot hierarchy while validating project-owned asset
references, exact workflow/recipe pairs, idempotency, and provenance in one
server transaction. Adding that safely would require a new persistence
contract and migration, which DEV-100 explicitly forbids.

Therefore DEV-100 freezes the provider-neutral contract in
`docs/EXTERNAL_AGENT_PRODUCTION_HANDOFF_V1.md`, records the implementation
state as schema-blocked, and does not invent an importer, migration, queue, or
executor. Existing Shot bulk import remains supported as a smaller formal
boundary and is not renamed into the frozen hierarchical contract.

## Safety invariants for a future implementation

Any later implementation must retain read-only preview, explicit confirmation,
all-or-nothing persistence, project isolation, exact ID references, duplicate
and unknown-field rejection, bounded input size, durable source provenance,
and no implicit queue admission, task creation, or generation start.
