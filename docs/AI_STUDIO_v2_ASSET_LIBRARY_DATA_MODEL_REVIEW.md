# AI Studio v2 Asset Library Data Model Review

```text
TASK=DEV-120
REVIEW_TYPE=DATA_MODEL_REVIEW
STATUS=PLANNING_ONLY
BASELINE=AI_STUDIO_v1.3.1_STABLE
CODE_CHANGE=NO
DATABASE_MIGRATION=NO
SCHEMA_IMPLEMENTATION=NO
UI_CHANGE=NO
API_CHANGE=NO
DATA_MODEL_READY=YES
IMPLEMENTATION_RISK=MEDIUM
RECOMMEND_START_IMPLEMENTATION=NO
NEXT_STEP=DEV-121_ASSET_LIBRARY_MVP_IMPLEMENTATION_PLAN
```

This review validates whether the Asset Library concept is complete enough to
enter implementation planning. It does not create tables, Rust structs,
transport commands, pages, CRUD, or import logic. “Ready” below means ready
for a separately approved implementation plan, not permission to start coding
in DEV-120.

## Review Verdict

```text
MODEL_REVIEW=APPROVED_WITH_GUARDS
IMPLEMENTATION_GATE=DEV-121_REQUIRED
DIRECT_CODING=NO
```

The model is coherent for a single-user local workspace. The review approves
the core entities and relationships with the following mandatory guards:

- Asset identity remains separate from Generation and Result identity;
- Asset Version is an independent history-preserving concept;
- many-to-many relationships are explicit and typed;
- project sharing is explicit and cannot bypass project isolation;
- provenance stores historical snapshots rather than only live links;
- SQLite stores metadata/relationships and the filesystem stores large media;
- v1.3.1 Project, Shot, Task, Queue, Review, and historical references remain
  intact;
- DEV-121 must define implementation order, files, migrations, and tests before
  any production Asset Library code is written.

## 1. Data Model Overview

The reviewed conceptual model is:

```text
Project
   │ owns / explicitly uses
   ▼
Asset ────────────────┐
   │                  │ has history
   │ references        ▼
   ├──────────────> Asset Version
   │
   │ supplies context to
   ▼
Prompt ── model + parameters + references ──> Generation
                                                │
                                                │ produces
                                                ▼
                                             Result
```

### Core entities and responsibilities

| Entity | Responsibility | Review result |
| --- | --- | --- |
| **Project** | Personal ownership/isolation container for production work, asset usage, and history | Complete with v1 compatibility guard |
| **Asset** | Stable long-term creative identity with type, metadata, location evidence, and relationships | Complete for MVP |
| **Asset Version** | Concrete non-destructive revision of an Asset | Must be independent |
| **Prompt** | Reusable instruction identity with versioned content and execution context | Complete with provider-specific fields |
| **Model** | User-maintained external model/provider description | Complete for provenance |
| **Reference** | Explicit input link to an Asset Version, file, or external source | Complete for MVP provenance |
| **Generation** | One external generation attempt and its immutable input/output context | Complete with snapshot guards |
| **Result** | Concrete output/candidate of a Generation, including evaluation and promotion state | Must remain independent |

This model deliberately does not make Asset Library a second Production Core.
Generation is a provenance record and Result is an output record; production
execution remains under the existing Queue and Task authority.

## 2. Entity Completeness Review

### Project

**Verdict: COMPLETE, with compatibility requirements.**

Project must continue to preserve:

- project name, type, description, status, and creation/update timestamps;
- explicit relationships to used/shared Assets and selected Asset Versions;
- Prompt, Reference, Generation, and Result context where the project uses it;
- existing production history: Shot, Task, Queue, Review, Rework, and Result;
- archive/backup context without silently moving or deleting user files.

Project remains the default isolation boundary. A Project's asset usage is a
relationship, not a copy of the underlying Asset identity.

### Asset

**Verdict: COMPLETE for MVP.**

The Asset concept needs the following common metadata:

```text
Asset ID
Type
Name
Description
Tags
Location
Checksum
Version
Status
Metadata
```

Review conclusions:

- **Asset ID:** required stable identity; never derived from name or path.
- **Type:** required initial taxonomy filter; not a complete ontology.
- **Name:** required user-facing label; duplicate names remain valid.
- **Description:** optional human context and creative intent.
- **Tags:** optional user-controlled labels; no automatic classifier in MVP.
- **Location:** local path, relative workspace reference, or external URI;
  mutable evidence rather than identity.
- **Checksum:** optional deterministic content evidence for validation and
  duplicate suggestions; not a merge decision.
- **Version:** user-visible current/preferred version context; concrete
  history belongs to Asset Version.
- **Status:** Created, Active, Versioned, or Archived; missing-file state is a
  separate availability condition.
- **Metadata:** type-specific and provider-specific values without discarding
  unknown fields.

The Asset record is complete only if it can also navigate to project usage,
relations, versions, and provenance. Metadata without these relationships would
collapse the Library back into a file browser.

### Asset Version

**Verdict: INDEPENDENT ENTITY REQUIRED.**

Two options were reviewed:

#### Option A — Asset owns a version field only

```text
Asset
└── version = v3
```

This is simple, but it cannot safely preserve multiple files, historical
checksums, source versions, per-project selection, or the exact revision used
by an old Generation. It risks turning “current version” into an overwrite.

#### Option B — independent AssetVersion

```text
Asset
├── AssetVersion v1
├── AssetVersion v2
└── AssetVersion v3
```

This preserves concrete locations, checksums, provenance, creation time,
status, notes, and historical usage independently from the stable Asset
identity.

#### Recommendation

**Choose Option B.** Asset Version is a separate conceptual entity. Asset may
retain a current/preferred version reference for navigation, but that reference
must never replace or delete older Asset Versions. A Project may select a
version for its own usage context without changing another Project's history.

### Prompt

**Verdict: COMPLETE with version snapshots.**

Prompt must support:

```text
Prompt ID
Template
Model
Parameters
Reference Assets
Result Assets
Version
Created Time
```

The design is sufficient if:

- Prompt content is versioned rather than edited in place for history;
- model/provider identity and provider-specific parameters are preserved;
- references point to explicit Asset Versions or local file evidence;
- results navigate to Generation/Result records without copied media;
- project usage/membership is explicit;
- MiniMax H3, ComfyUI, IndexTTS, and ACE-Step values can retain fields that
  are not shared by every provider.

The model must not force all external tools into one identical prompt or
parameter schema. A common provenance envelope plus provider-specific values is
the lowest-complexity boundary.

### Generation

**Verdict: COMPLETE with a historical input snapshot.**

Generation can fully record one AI production behavior when it preserves:

```text
Generation ID
Project
Input / Prompt Version
Model and provider
Parameters
References and selected versions
External Tool
Workflow context when available
Outputs
Status
Evaluation
Created / start / completion time when known
```

It must be append-oriented. A failed, cancelled, or retried attempt remains a
Generation record. A retry creates another attempt and does not erase the
first attempt. Generation is not Queue, Task, or an execution scheduler.

### Result

**Verdict: INDEPENDENT ENTITY REQUIRED.**

Result should not be only a path on Generation because one Generation can
produce multiple candidates, files, or output modalities. An independent
Result is needed to preserve:

- one-to-many Generation outputs;
- concrete location, media type, checksum, and preview evidence;
- user evaluation, selected/preferred state, and rejection notes;
- promotion to an Asset Version without deleting unselected candidates;
- result-specific archive and missing-file state.

Result remains linked to its Generation and may point to an Asset Version when
the user explicitly catalogs it as a durable reusable asset.

## 3. Relationship Review

### Reviewed relationships

| Relationship | Verdict | Required guard |
| --- | --- | --- |
| **Asset ↔ Project** | Required many-to-many usage relationship | Explicit Project Usage Reference; no implicit cross-project visibility |
| **Asset ↔ Asset** | Required many-to-many typed relationship | Reference, derivative, replacement, and related links preserve history |
| **Asset ↔ Prompt** | Required many-to-many relationship | Prompt references selected Asset Versions and retains project context |
| **Prompt ↔ Generation** | Required one-to-many relationship | Each Generation snapshots the exact Prompt Version used |
| **Generation ↔ Result** | Required one-to-many relationship | Each Result remains independently evaluable and archivable |

### Does the model need AssetRelation?

**Verdict: YES, as an independent conceptual relationship record.**

An `AssetRelation` concept is justified because an Asset-to-Asset link needs
more than two IDs. It may need:

```text
source asset/version
target asset/version
relation type
role
order
project usage context
note
created time
```

Possible relation types include `reference`, `derived_from`, `replacement`,
`variant_of`, `used_with`, and `related`. The initial MVP need not expose every
type in the UI, but the relationship must remain typed and non-destructive.

This is a conceptual approval, not permission to create an `AssetRelation`
table in DEV-120. DEV-121 must choose the smallest implementation shape that
supports this many-to-many behavior without introducing a generic graph
framework.

### Relationship complexity decision

The relationships are complex enough to require explicit join/context
records, but not complex enough to justify a general-purpose graph database or
unbounded ontology. Stable IDs, typed edges, project context, and timestamps
are sufficient for the Personal Edition MVP.

## 4. Missing Entity Analysis

The review distinguishes domain entities from lightweight user preferences.

| Candidate | Decision | Review reasoning |
| --- | --- | --- |
| **Collection / Folder** | **DEFER** | Tags, type, project, Recent, and Favorites cover MVP wayfinding. Do not turn the Library into a folder manager; revisit named collections after real usage. |
| **Favorite** | **ADD_NOW** | Favorites are already part of the proposed information architecture and are a simple explicit user preference. Model as lightweight Asset/Version preference, not a new domain hierarchy. |
| **Rating** | **DEFER** | User evaluation/notes are enough initially. A numeric scale risks false precision before the user has a stable review habit. |
| **Alias** | **ADD_NOW** | A lightweight alias list supports real personal vocabulary such as `王也 = 道士角色A` and improves retrieval without changing identity. It need not be a separate entity in MVP. |
| **Asset Group** | **DEFER** | Character packs and scene packs can be represented later by typed relationships, tags, or collections. A first-class bundle model would increase complexity before its behavior is clear. |

### Missing-entity guardrails

- `Favorite` must never mean “AI-selected best result”. It is only the user's
  explicit wayfinding preference.
- `Alias` must remain a label attached to an Asset or usage context; aliases
  must not become alternate identities or bypass project isolation.
- Deferred Collection/Folder and Asset Group concepts must not be smuggled in
  as arbitrary untyped JSON with no ownership or history rules.
- Deferred Rating can be revisited through Review/Evaluation design rather than
  adding multiple competing quality systems.

## 5. Version Strategy Review

### Verdict

The current **non-destructive version strategy is correct and approved**.

```text
Asset A
├── Asset Version v1
├── Asset Version v2
└── Asset Version v3

Prompt A
├── Prompt Version v1
├── Prompt Version v2
└── Prompt Version v3

Generation
├── Attempt 01
├── Attempt 02
└── Attempt 03
```

### Asset version

Asset Version must be independent and must retain concrete location,
checksum, creation time, source/provenance, status, and notes. “Current” is a
selection for navigation. “Preferred” is a user decision. Neither deletes an
older version.

### Prompt version

Prompt Version must capture exact template content and relevant model,
parameter, and reference context. Historical Generations point to the Prompt
Version they used even after the editable Prompt advances.

### Result history

Every Generation attempt and Result candidate remains available according to
the user's retention/archive policy. Selecting a preferred Result is not a
destructive merge. Failed or cancelled attempts remain evidence when they are
part of the creative history.

### Version implementation guard

Avoid a premature universal polymorphic `Version` abstraction if it would blur
Asset Version, Prompt Version, and Generation Attempt semantics. The concepts
share non-destructive principles but have different fields and lifecycle
meaning. DEV-121 should prefer explicit domain boundaries over an abstraction
that saves a small amount of schema vocabulary.

## 6. Provenance Review

### Can the model answer the core questions?

```text
Where from?  YES, with Project + source/reference links and file evidence.
How created? YES, with Generation + Prompt Version + tool context.
Using what?  YES, with Model + parameters + selected Reference Versions.
```

The answer is reliable only when live navigation links are paired with
historical snapshots. A current Prompt or Asset can change; the Generation
record must retain what was actually used.

### Provenance completeness matrix

| Provenance item | MVP | Future / conditional |
| --- | --- | --- |
| Project | **Required** | — |
| Prompt identity and exact Prompt Version | **Required** | — |
| Prompt/template snapshot | **Required** | — |
| Model/provider name and version | **Required when available** | — |
| Provider-specific parameters | **Required** | — |
| Reference Assets and selected versions | **Required** | — |
| Reference role/order | **Required when meaningful** | — |
| External Tool identity | **Required** | — |
| Tool version | **Required when exposed** | Better capability diagnostics later |
| Workflow version | **Required for tools that expose workflows** | Rich workflow diff later |
| Seed | **Required when the provider exposes it** | Cross-tool seed comparison later |
| Input/output locations and checksums | **Required when available** | Content fingerprinting later |
| Created/start/completion time | **Required when known** | — |
| Generation status and external error | **Required** | — |
| User evaluation | **Required as notes/selection** | Numeric rating later |
| Environment | **Record basic observed environment when available** | Detailed reproducibility bundle |

### Tool and workflow version

Tool version and workflow version are not optional if the external tool exposes
them. For ComfyUI or other workflow-based tools, record the observed workflow
identity/version and preserve the existing exact production identity rules; do
not infer identity from a workflow or recipe name.

For tools that do not expose a stable version, record `unknown` or the
user-imported value rather than inventing one.

### Seed and environment

Seed belongs in the provider-specific parameter snapshot whenever available.
Environment should begin as lightweight observed context—tool version, model
version, operating/runtime details when supplied—rather than an attempt to
capture every machine setting. A detailed reproducibility bundle is future
scope because it risks large, fragile metadata.

## 7. Search Model Review

### MVP verdict

```text
MVP_SEARCH=name + tag + type + project
MVP_SEARCH_SUFFICIENT=YES
```

These filters are sufficient for the first Asset Library because they map to
the user's immediate wayfinding questions and remain understandable on a
local desktop. The MVP may add simple recency, status, version, Favorite, and
missing-reference filters if they do not change the core boundary.

Every result should visibly include project context, Asset type, current
version, and relationship/provenance breadcrumbs. A same-named Asset in two
Projects must remain distinguishable.

### Future search reservation

```text
FTS=RESERVE
EMBEDDING=RESERVE
VECTOR_SEARCH=RESERVE
```

Future local FTS may index descriptions, notes, Prompt content, aliases, model
labels, and provenance summaries. Embedding/vector search may be considered
only after real collection measurements. Any future index must be rebuildable
from canonical metadata, local-first, project-aware, and non-essential to
basic operation.

## 8. Storage Model Review

### Verdict

```text
STORAGE_MODEL=SQLite + Filesystem
STORAGE_MODEL_APPROVED=YES
```

The split is appropriate for a single-user local workspace:

```text
SQLite
├── metadata
├── relations
└── history

Filesystem
├── image / video / audio / voice / music media
├── previews
└── archive packages
```

### Media decision

Images, videos, and audio should all be external to SQLite by default. SQLite
should store location/reference, media type, size, checksum, preview reference,
availability state, and provenance. This avoids database bloat, long locks, and
unnecessarily large metadata backups.

Small text metadata may be stored in SQLite. File-backed Documents and Prompt
source files remain local files with catalog metadata and checksums.

The exact workspace directory layout remains an implementation decision. The
durable link is a stable Asset/Version ID plus file evidence, not a human
folder name.

## 9. Migration Compatibility Review

### Verdict

```text
V1_3_1_PRESERVED=YES
DESTRUCTIVE_MIGRATION=NO
```

The v1.3.1 model remains the compatibility anchor. Future v2 additions must
not break:

```text
Project
Shot
Task
Queue
Review
```

### Continuity rules

- Existing Project IDs and project ownership remain readable.
- Existing Shot and Review history remains unchanged; Asset Version links are
  optional additions, not reinterpretations.
- Existing Task and Queue records remain the source of truth for prior and
  active production execution.
- Production Queue remains the sole execution authority; Asset Library does
  not create a second queue or task model.
- Existing exact workflow identity remains the
  `workflowVersionId + recipeId` pair. Names are never used to infer it.
- Existing Result and historical production references remain valid.
- Existing files are inventoried or explicitly associated later; they are not
  silently moved, renamed, or deleted by a migration.
- Missing v1 provenance is marked `unknown` or `legacy`, never fabricated.
- Future migration must be additive, backed up, validated, and reversible or
  recoverable if relationship/file checks fail.

### Migration readiness boundary

The conceptual model is compatible enough for DEV-121 to design an additive
migration strategy. It is not an approval to run a migration now. DEV-120
creates no database change.

## 10. MVP Boundary Review

The Asset Library first version is approved with this narrow boundary:

```text
Asset Metadata
Tags
Preview
Relations
Version
Provenance
Basic Search
```

The MVP may include explicit project usage context, Favorites, and lightweight
Aliases because they directly improve wayfinding without creating new complex
domain systems.

### Deferred from MVP

```text
AI_TAGGING=DEFER
VECTOR_SEARCH=DEFER
CLOUD_SYNC=DEFER
AUTO_CLASSIFICATION=DEFER
COLLECTION_FOLDER=DEFER
RATING=DEFER
ASSET_GROUP=DEFER
```

The following remain rejected for the Personal Edition data layer:

```text
AI_AGENT=REJECT
AUTO_DECISION=REJECT
MULTI_USER=REJECT
PERMISSION_SYSTEM=REJECT
SAAS=REJECT
SECOND_QUEUE=REJECT
SECOND_TASK_MODEL=REJECT
```

Import and file scanning remain explicit future flows. They are not part of
this review's implementation authorization.

## 11. Implementation Readiness Score

```text
DATA_MODEL_READY=YES
IMPLEMENTATION_RISK=MEDIUM
RECOMMEND_START_IMPLEMENTATION=NO
```

### Interpretation

**DATA_MODEL_READY=YES** means the conceptual model is complete enough to
enter a dedicated implementation-planning stage. It does not mean that the
schema, migration, UI, import flow, or API is already approved.

**IMPLEMENTATION_RISK=MEDIUM** reflects the remaining engineering risk in
filesystem references, many-to-many relationship context, duplicate evidence,
provenance snapshots, archive size, and backward-compatible migration. These
are manageable with the approved guards but should not be hidden by a rushed
MVP.

**RECOMMEND_START_IMPLEMENTATION=NO** because the next required step is
DEV-121, which must convert this review into an implementation plan with file
structure, migration sequencing, transport boundaries, acceptance criteria,
and tests. No coding should begin automatically from this document.

## 12. Next-Stage Recommendation

```text
RECOMMENDED_NEXT_STEP=DEV-121_ASSET_LIBRARY_MVP_IMPLEMENTATION_PLAN
DEV_121_STARTED=NO
```

DEV-121 should define:

- the smallest implementation slice for Asset metadata, Asset Version,
  project usage, and AssetRelation;
- precise repository/service/typed-transport boundaries;
- SQLite migration order and rollback/recovery checks;
- filesystem reference, preview, checksum, and missing-file behavior;
- provenance snapshot and Result promotion rules;
- MVP search/index choices;
- frontend information architecture and accessibility acceptance criteria;
- focused frontend, Rust, IPC, migration, and file-reference tests.

Until DEV-121 is explicitly approved, AI Studio v1.3.1 remains the active
stable baseline. DEV-120 ends with this review document; it does not start
Asset Library implementation.
