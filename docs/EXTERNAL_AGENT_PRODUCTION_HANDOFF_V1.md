# External Agent Production Handoff V1

```text
CONTRACT=EXTERNAL_AGENT_PRODUCTION_HANDOFF
VERSION=1
STATUS=FROZEN_NOT_IMPLEMENTED
PROVIDER_NEUTRAL=YES
AI_STUDIO_AUTHORING=NO
PREVIEW_REQUIRED=YES
CONFIRM_REQUIRED=YES
AUTO_QUEUE=NO
AUTO_TASK=NO
AUTO_GENERATION=NO
```

## Purpose and scope

This document freezes the target exchange shape for an external narrative or
creative agent to hand structured production inputs to AI Studio. It is a
provider-neutral data contract, not a claim that the hierarchical importer is
available in the current binary. DEV-100 deliberately records
`HANDOFF_IMPLEMENTATION_BLOCKED_BY_SCHEMA=YES`.

The external agent owns story, screenplay, storyboard, and prompt-generation
decisions. AI Studio owns project-scoped validation, formal production records,
prompt input snapshots, workflow/recipe binding, preparation, queue admission,
Task history, and ComfyUI execution.

## Target document

The target V1 document has exactly these top-level fields:

| Field | Required | Meaning |
| --- | --- | --- |
| `schemaVersion` | yes | Integer `1`. |
| `projectId` | yes | Exact target AI Studio project ID. |
| `source` | yes | External source label and optional source revision metadata. |
| `series` | yes | Ordered series records. |
| `limits` | no | Explicit bounded-input declaration; the server remains authoritative. |

Each `series` record has `externalId`, `name`, `description`, `ordinal`, and
`episodes`. Each `episode` has the same identity/display fields and `scenes`.
Each `scene` has the same identity/display fields and `shots`. `externalId` is
an opaque source label and is not an AI Studio database ID.

Each `shot` has:

```json
{
  "externalId": "shot-001",
  "name": "The gate opens",
  "ordinal": 1,
  "description": "A gate opens in the rain.",
  "imagePrompt": "cinematic wide shot, rain, backlight",
  "videoPrompt": "the camera slowly pushes toward the opening gate",
  "assetRefs": [{ "assetId": "ast_existing_reference" }],
  "stages": {
    "image": {
      "workflowVersionId": "wv_exact_image",
      "recipeId": "recipe_exact_image"
    },
    "video": {
      "workflowVersionId": "wv_exact_video",
      "recipeId": "recipe_exact_video"
    }
  }
}
```

`imagePrompt` and `videoPrompt` are supplied production inputs. V1 does not
define `negativePrompt`, template variables, prompt rewrite instructions, or a
provider-specific prompt dialect. Recipe-bound scalar and asset input values
remain owned by the existing workflow/recipe configuration path; this handoff
must not smuggle a second parameter model into the document.

`assetRefs[].assetId` values must be exact assets already owned by the target
project. A future implementation must validate every reference before any
write. Workflow identity is always the exact pair
`workflowVersionId` + `recipeId`; a display name, latest record, or provider
name is not sufficient.

## Validation and transaction rules

The future importer must:

1. reject unknown fields rather than silently dropping them;
2. reject duplicate `externalId` values at every sibling collection and
   reject duplicate effective hierarchy paths;
3. reject an empty or wrong `schemaVersion`, a wrong `projectId`, invalid
   identifiers, invalid ordinals, and cross-project asset references;
4. validate every exact workflow/recipe pair before persistence;
5. enforce bounded input before parsing untrusted large documents; the current
   formal Shot bulk boundary is 500 leaf Shots and prompt text is bounded by
   the existing 64 KiB prompt contract;
6. preserve source revision/provenance and provide idempotent replay semantics
   without guessing from names; and
7. perform one server-side all-or-nothing transaction for hierarchy records,
   Shots, prompt input/snapshot provenance, references, and handoff identity.

Preview is read-only and returns normalized records, warnings, errors, exact
references, and the write plan. Confirm is a separate explicit action. Neither
preview nor confirm may enqueue a batch, create a Task, start ComfyUI, retry,
or silently mutate an existing project outside the confirmed transaction.

The current schema has no handoff identity/provenance table or transaction
contract covering this entire hierarchy. Consequently idempotency and the
single-transaction promise are intentionally **blocked**, not approximated by
client-side names or a partial write sequence.

## Example source metadata

```json
{
  "schemaVersion": 1,
  "projectId": "prj_example",
  "source": {
    "agent": "external-story-agent",
    "revision": "story-42"
  },
  "series": []
}
```

The example is contract-only. It is not accepted by the current hierarchical
import path because that path is not implemented.

## Existing implementation boundary

The current `ShotBulkService` accepts only its existing flat JSON/TSV contract:
it is a formal Shot import with preview and atomic commit. It does not accept
this document, create the hierarchy, establish handoff provenance, or provide
idempotent external-agent replay. It remains supported and is not a second
production executor.

## Out of scope

- Script, Draft, Narrative, Storyboard, screenplay, or prompt authoring UI.
- Prompt templates, variable expansion, generation, rewrite, or provider
  adapters.
- Automatic queue admission, Task creation, generation, retry, or ComfyUI
  execution.
- A second Shot model, queue, executor, Task model, or persistence migration.
- Name-based identity inference or cross-project asset lookup.
