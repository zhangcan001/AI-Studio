# AI Studio v2 Personal Edition Data Foundation Design

```text
TASK=DEV-118
STATUS=PLANNING_ONLY
BASELINE=AI_STUDIO_v1.3.1_STABLE
CODE_CHANGE=NO
DATABASE_MIGRATION=NO
SCHEMA_IMPLEMENTATION=NO
UI_CHANGE=NO
API_CHANGE=NO
```

This document defines a conceptual data foundation for the future Asset
Library, Prompt Studio, Local Tool Hub, and Project Archive. It is a design
artifact only. It does not create SQLite tables, modify Rust models, change
React pages, add CRUD, or authorize Asset Library implementation.

## 1. Data Philosophy

AI Studio v2 should not primarily manage files. Folders and files are storage
locations; they do not explain what an asset means, how it was created, which
project used it, or which earlier version it replaced.

The data foundation should manage:

```text
Asset Identity
Creation History
Relationship
Provenance
Version
```

The key question is not only “where is this file?” but also:

```text
What is it?
Where did it come from?
How was it created?
What did it use?
Which project and production context used it?
Which version is current without deleting history?
```

### Principles

```text
LOCAL_FIRST=YES
PERSONAL_USE_FIRST=YES
DATA_OWNERSHIP=YES
LONG_TERM_STORAGE=YES
BACKWARD_COMPATIBILITY=YES
LOW_COMPLEXITY=YES
```

1. **Identity is stable.** A display name, path, or checksum is evidence about
   an entity, not the entity's durable identity.
2. **History is append-oriented.** New prompts, attempts, results, and asset
   revisions are recorded as new versions or records; old results are not
   silently overwritten.
3. **Relationships are explicit.** A reference, derivation, project
   membership, or replacement link must be inspectable rather than inferred
   from folder names.
4. **Provenance is first-class.** A result should retain enough input context
   to answer how it was produced even if the external tool is later offline.
5. **Project isolation is the default.** Shared personal assets are an
   explicit opt-in relationship, not an accidental consequence of a global
   search or a reused name.
6. **Metadata and media have different lifecycles.** SQLite should manage
   searchable metadata and relationships; large media should remain in
   user-owned local storage.
7. **The user remains the authority.** The data layer records choices and
   observations; it does not approve results or make creative decisions.

## 2. Core Entity Design

The entities below are conceptual records for a future v2 design. Field names
are descriptive planning vocabulary, not a schema proposal.

### Entity: Project

Project is the primary personal work container and the default isolation
boundary. It groups production work, assets, prompts, generations, results,
and archive snapshots.

Conceptual attributes:

```text
Project
id
name
type
description
created_at
updated_at
status
archive_reference
```

`type` may distinguish AI animation, TTS/radio drama, AI music, or a custom
personal project without requiring separate data models. `status` should be
user-visible and simple, such as active, paused, archived, or closed.

Project relationships:

- owns or explicitly includes project-scoped assets;
- includes shots, tasks, reviews, and production results from v1.x;
- references reusable prompts and generation records;
- has named archive snapshots and export history;
- never gains access to another project's private records merely because two
  records have the same name.

### Entity: Asset

Asset is the central catalog identity for a creative item or a concrete piece
of source/generated media. It describes meaning and context in addition to a
local location.

Conceptual attributes:

```text
Asset
id
type
name
description
location
checksum
created_at
updated_at
version
tags
status
project_memberships
provenance_reference
```

Field intent:

- `id`: stable identifier; never derived from a name or mutable path;
- `type`: controlled initial kind such as Character or Image;
- `name`, `description`: user-facing labels and context;
- `location`: local path, relative workspace reference, or external URI;
- `checksum`: optional content identity/evidence for duplicate detection and
  archive validation;
- `created_at`, `updated_at`: catalog lifecycle timestamps;
- `version`: current user-visible version or version label, not destructive
  replacement of earlier content;
- `tags`: optional user-controlled labels and facets;
- `status`: draft, active, archived, missing-reference, or superseded;
- `project_memberships`: explicit links to one or more projects when sharing
  is intentional;
- `provenance_reference`: link to the import, prompt, generation, or external
  source that produced or introduced the asset.

#### Asset Type

```text
Character
Scene
Prop
Image
Video
Audio
Voice
Music
Prompt
Document
```

The list is a starting vocabulary, not a rigid ontology. A Character may have
many image or voice representations; a Document may be source text, script,
brief, subtitle, or production note. The catalog should preserve the stable
asset identity while allowing concrete versions to use different files.

#### Asset ownership and cross-project sharing

The design must support the useful case where one character belongs to several
projects, while retaining project isolation:

```text
Personal Asset Identity
        │
        ├── explicit membership ──> Project A
        └── explicit membership ──> Project B
```

The default creation path should still be project-scoped. A user may promote
an asset to an explicitly shared personal identity, and each project gets a
visible membership/link with its own context. A private project asset must not
become discoverable from another project through an unqualified name search.
Project-specific labels, notes, or selected versions should be represented as
context, not by mutating another project's historical record.

### Entity: Prompt

Prompt is a reusable instruction identity with versioned content and recorded
execution context. It can be a standalone creative asset or an input to a
Generation.

Conceptual attributes:

```text
Prompt
id
template
model
parameters
reference_assets
result_assets
version
created_time
updated_time
tags
project_memberships
```

`template` can contain plain text and optional user-defined variables.
`model`, `parameters`, and `reference_assets` describe the intended or
observed inputs; they must be snapshotted on a Generation so later edits do
not rewrite history. `result_assets` is a navigational relationship, not a
copy of output media.

Prompt coverage is expected to include:

- MiniMax H3;
- ComfyUI workflows and prompts;
- IndexTTS voice prompts or synthesis instructions;
- ACE-Step music prompts.

The common data foundation should store portable provenance fields without
pretending that these tools have identical parameters or output semantics.

### Entity: Model

Model is a user-maintained description of an external model or provider used
by Prompt Studio or a Generation.

Conceptual attributes:

```text
Model
id
provider
name
version
capabilities
local_tool_reference
metadata
```

Model records describe what was used; they do not install, host, or replace
the external model. Provider and model names must remain historical facts in a
Generation snapshot even when the user's current Model record is edited.

### Entity: Reference

Reference is an explicit input relationship used by a Prompt or Generation.
It may point to an Asset version, a local file, or an external reference that
cannot yet be cataloged.

Conceptual attributes:

```text
Reference
id
source_type
asset_id
asset_version
location
role
checksum
created_time
```

Examples of `role` include character identity, scene style, pose, audio guide,
or music reference. The role is descriptive; the user can inspect what was
used without requiring automatic interpretation.

### Entity: Generation

Generation is the durable record of one AI generation behavior or one
imported external run. It is an attempt/provenance record, not a second task
queue.

Conceptual attributes:

```text
Generation
id
input
model
parameters
references
output
status
evaluation
created_time
project_id
external_tool
```

Field intent:

- `input`: prompt version, source document, text segment, or other input
  snapshot;
- `model`: model/provider snapshot and observed version;
- `parameters`: exact recorded settings, including seeds or constraints when
  the external tool exposes them;
- `references`: explicit reference assets/versions and roles;
- `output`: result/asset-version links and file evidence;
- `status`: requested, running, completed, failed, cancelled, imported, or
  unknown as appropriate to the external record;
- `evaluation`: user notes, rating, review state, or rejection reason; never
  an autonomous quality judgment;
- `created_time`: when the generation attempt was recorded/started;
- `project_id`: owning project context;
- `external_tool`: the local tool or provider entry used for the attempt.

Generation remains separate from the existing v1.x Task model. If a future
implementation connects a Generation to a production Task, it must retain the
historical Task and exact workflow reference rather than reinterpret it.

### Entity: Result

Result is the user-visible output of a Generation. A Result may be an image,
video, audio file, music track, document, or a set of files.

Conceptual attributes:

```text
Result
id
generation_id
asset_version
location
checksum
status
evaluation
created_time
```

Result should normally become or point to an Asset version so it can be
searched and reused. The record must retain the Generation relationship even
if the user later selects a different result as the preferred version.

### Entity: Version

Version is a history-preserving revision of an Asset, Prompt, or project
output. It is a conceptual cross-cutting concern rather than permission to
overwrite a record.

```text
Version
id
parent_identity
version_label
created_time
source_version
content_reference
checksum
status
notes
```

Generation attempts use a related attempt sequence such as `Attempt 01` and
`Attempt 02`. They are not treated as silent replacements for one another.

## 3. Asset Relationship Design

The central relationship is a chain of meaning and provenance, not merely a
chain of files:

```text
Project
   |
   | explicit membership / production context
   v
Asset
   |
   | selected version, derivative, or reference
   v
Prompt
   |
   | prompt version + model + parameters + references
   v
Generation
   |
   | produces
   v
Result
   |
   | cataloged as an asset version
   v
Asset / Version
```

The same flow can be expanded for a character-driven image result:

```text
Character
    ↓
Reference Image
    ↓
Prompt
    ↓
Generation
    ↓
Result
    ↓
Project
```

### Relationship rules

| Relationship | Cardinality | Meaning |
| --- | --- | --- |
| Project ↔ Asset | many-to-many through explicit membership | one reusable personal asset may be used by several projects; private assets remain isolated |
| Asset → Asset Version | one-to-many | a stable concept has concrete revisions/files |
| Asset ↔ Asset | many-to-many | references, derivatives, replacements, and related assets are explicit and typed |
| Prompt → Prompt Version | one-to-many | prompt edits preserve earlier instructions |
| Prompt ↔ Asset | many-to-many | prompts may reference many assets and assets may be reused by many prompts |
| Prompt → Generation | one-to-many | one prompt version can produce many attempts/results |
| Generation ↔ Reference | many-to-many | each run records several references and each reference can be reused |
| Generation → Result | one-to-many | a run may return multiple files or candidate outputs |
| Result → Asset Version | usually many-to-one or one-to-one | a result can be cataloged as a reusable concrete asset revision |
| Shot ↔ Asset Version | many-to-many | existing production shots can use explicit asset versions |

Relationships should carry their own context where necessary: role, order,
selected state, project membership, or creation time. A relationship must not
be inferred from a matching name. Removing a current link must not erase the
historical Generation or Result that recorded the link.

## 4. Prompt Entity Design

Prompt Studio should treat a prompt as a versioned production input rather
than a text snippet in a folder.

```text
Prompt
├── template
├── model
├── parameters
├── reference_assets
├── result_assets
├── version
└── created_time
```

### Prompt design rules

1. The editable template is separate from immutable Prompt Version snapshots.
2. Model and parameter fields remain provider-aware; a common envelope is
   allowed, but provider-specific values must not be discarded.
3. Reference assets are linked by stable identity and selected version where
   available, with a fallback local location/checksum for uncataloged input.
4. Results are navigable links to Generation/Result records, not duplicated
   prompt output blobs.
5. Prompt versions can be reused across projects only through explicit
   membership or sharing; project context remains visible.
6. MiniMax H3, ComfyUI, IndexTTS, and ACE-Step are supported as provenance
   profiles, not forced into one identical execution API.

Prompt Studio records what the user authored or imported. It does not become
an AI Agent, automatic prompt optimizer, or autonomous next-step selector.

## 5. Generation Record Design

A Generation record should answer “what happened?” in a way that remains
useful after the external tool is closed:

```text
Generation
├── input
├── model
├── parameters
├── references
├── output
├── status
├── evaluation
└── created_time
```

At minimum, a completed or imported record should preserve:

- project context;
- prompt identity and exact Prompt Version;
- model/provider name and observed model version;
- parameters as entered or imported;
- ordered reference identities, versions, roles, and checksums where
  available;
- external tool identity and tool/version metadata;
- start/created/completed timestamps when known;
- result locations, checksums, media type, and output count;
- terminal status and any user-visible error;
- user evaluation, review notes, or selected/rejected state.

The record is append-oriented. A failed attempt remains useful evidence; a
retry creates another Generation rather than erasing the failure. If the
existing Production Queue later initiates a Generation, the integration must
preserve Queue/Task history and must not create a second execution authority.

## 6. Provenance Design

Provenance connects an output to the exact context that produced it. For
example:

```text
Project A
  └── Result Image
        ├── Generated Time: 2026-09-13T...
        ├── Prompt: Prompt V3
        ├── Model: MiniMax H3
        ├── Parameters: recorded provider-specific values
        ├── Reference: Character X, selected version
        ├── External Tool: recorded tool/provider version
        └── Output: local location + checksum when available
```

### Provenance requirements

Every result should make these questions answerable:

```text
Where from?
How created?
Using what?
```

The design should distinguish:

- **live links**, which help the user navigate to the current Asset or Prompt;
- **historical snapshots**, which preserve what the Generation actually used;
- **file evidence**, including location, media type, size, and checksum when
  available;
- **unknown values**, which are recorded as unknown rather than guessed.

Editing a current Prompt, Model, or Asset must not rewrite a historical
Generation snapshot. If a referenced file is moved, the provenance record
remains and reports the missing location until the user explicitly relinks it.

## 7. Version Design

Versions prevent the primary long-term failure mode of a creative workspace:
losing the context of an earlier result because a file or prompt was replaced.

### Asset versions

```text
Character v1
Character v2
```

The Asset identity describes the character concept. Each Asset Version points
to a concrete file or representation, its provenance, checksum, creation time,
and user-selected status. A new version does not delete the old version.

### Prompt versions

```text
Prompt v1
Prompt v2
Prompt v3
```

Prompt Version contains the exact template content and relevant model,
parameter, and reference snapshot used for a Generation. The current editable
Prompt may advance while historical runs continue to point at their original
version.

### Generation attempts

```text
Attempt 01
Attempt 02
Attempt 03
```

Attempts are separate Generation records. A retry, failure, cancellation, or
alternative parameter set remains visible. The user may mark one Result as
preferred without deleting candidates or the attempt history.

### Version rules

1. Never overwrite old content as the only copy.
2. Stable parent identity and version identity are separate concepts.
3. “Current” and “preferred” are user selections, not destructive mutations.
4. Historical production references remain valid when a newer version exists.
5. A version can be archived or superseded while remaining restorable.

## 8. Search Design

Search is planned as a local, project-aware index. It is not implemented in
DEV-118.

### Basic search and filters

The first search layer should support:

```text
name
tag
type
project
```

Useful additional filters are status, version, created/updated date, model,
external tool, checksum, missing-reference state, and generation outcome.
Search results should show project context and relationship breadcrumbs so a
same-named Character in two projects is not ambiguous.

### Text search

The likely first advanced search is local full-text search over name,
description, notes, Prompt templates, model/provider labels, tool metadata,
and provenance summaries. FTS is a future implementation choice, not a table
or index added by this task.

### Future vector search

Embedding/vector search is reserved as an optional future enhancement for
local discovery. It must not be a v2 data prerequisite, a cloud dependency, or
an automatic classifier. If later approved, the vector index should be
rebuildable from canonical metadata and should respect project isolation.

## 9. Storage Design

The preferred conceptual workspace separates catalog metadata from large
media while keeping both under user-owned local storage:

```text
AI Studio Workspace
├── projects
├── assets
├── prompts
├── generations
└── archive
```

The exact directory layout remains a later implementation decision. Stable
IDs, not human folder names, should be the durable link between metadata and
files.

### Database saves

SQLite should save:

- project and membership metadata;
- Asset, Prompt, Model, Reference, Generation, Result, and Version metadata;
- relationship and provenance records;
- tags and searchable text;
- locations, checksums, media descriptors, and missing-file state;
- archive snapshot manifests and validation results.

### Files save

The local file system should save:

- source documents and production inputs;
- images, video, audio, voices, and music;
- previews and derived thumbnails where useful;
- exported results and archive packages.

Large media should not be stored as routine SQLite blobs. A file can be
relinked or archived without changing its catalog identity. The system should
report missing files instead of silently deleting metadata.

### Ownership and backup

The workspace belongs to the user. Backup and archive operations should be
explicit, inspectable, and restorable without an online service. A package
should include a metadata manifest, relationship/provenance data, and an
inventory of media with checksums or missing-file markers.

## 10. SQLite Strategy

### Recommendation

Continue with the current Tauri + Rust + SQLite persistence route for v2
metadata. It is appropriate for a single-user Windows desktop application:

- no database server or cloud account is required;
- transactions protect related metadata changes;
- indexes and FTS can support a substantial personal catalog;
- backup is local and inspectable;
- Rust repository ports preserve a clear persistence boundary.

Changing databases before real personal workload measurements would add
complexity without solving the core continuity problem.

### Planning scale

The expected personal workload is media-heavy but metadata-light compared with
an enterprise system. A reasonable initial design target is:

```text
Active projects             1–50
Cataloged assets             1,000–100,000
Asset/prompt versions        10,000–500,000
Generation records           10,000–1,000,000 over long-term use
Large media                  Stored outside SQLite
```

These are capacity planning ranges, not promises or acceptance limits. The
design should measure actual personal collections before adding infrastructure.

### Index strategy

Future implementation should prioritize indexes for:

- stable IDs and project membership;
- `project + type + status` catalog filters;
- updated/created time and current/preferred version;
- tag relationships and checksum lookup;
- Asset/Prompt/Generation/Result provenance links;
- missing-file and archive validation state;
- external tool/model filtering.

Full-text search should be an explicit local FTS index over user-facing text
and provenance summaries. It should be rebuildable from canonical records.
Vector search is not part of the initial SQLite requirement and should not
drive the core schema.

### Migration strategy

Any future schema work requires a separately approved implementation task and
must be additive/backward-compatible where practical:

1. back up the existing database before migration;
2. use versioned, deterministic migrations;
3. preserve v1 Project, Shot, Task, Review, and historical production IDs;
4. preserve unknown or provider-specific metadata rather than dropping it;
5. validate counts, relationships, and file references after migration;
6. provide a clear recovery path if validation fails.

DEV-118 adds no table, index, migration, or database code.

## 11. Backward Compatibility

The v1.3.1 stable data model remains the compatibility anchor. Future v2 data
must be additive around the existing production foundation, not a replacement
for it.

### Records that must not break

```text
Project
Shot
Task
Review
```

Their existing IDs, status history, project ownership, and historical
references must remain readable. Existing results and production records must
not be reinterpreted as new Asset or Generation facts without an explicit,
validated mapping.

### Compatibility rules

- Existing Project records remain valid project containers.
- Existing Shot and Review records continue to work when an optional Asset
  Version relationship is later added.
- Existing Task history remains the source of truth for prior production
  execution.
- Existing workflow identity remains the exact `workflowVersionId + recipeId`
  pair; names must never be used to infer identity.
- Existing queue behavior and Queue Start execution authority remain unchanged.
- Existing local files are inventoried and reported; they are not moved or
  renamed implicitly by a data migration.
- New Asset, Prompt, Generation, Result, Model, Reference, and Version records
  may link to old records through explicit compatibility relationships, while
  keeping the old records intact.
- If a v1 record lacks provenance, the future catalog records “unknown” or
  “legacy” rather than fabricating a source.

The first future implementation should prefer a read-compatible catalog and a
careful import/association flow over a destructive rewrite of the v1 database.

## 12. Non Goals

The v2 data layer explicitly does **not** include:

```text
AI_AGENT=NO
CLOUD_SYNC=NO
MULTI_USER=NO
PERMISSION=NO
SAAS=NO
AUTO_DECISION=NO
```

It also does not include an autonomous prompt optimizer, automatic result
approval, cloud identity/account management, enterprise sharing, or a second
Queue/Task execution model. The data foundation records explicit user and
external-tool activity; it does not make decisions on the user's behalf.

## 13. Architecture Diagram

The minimum conceptual flow is:

```text
Project
   |
   | owns / explicitly includes
   v
Asset
   |
   | referenced by a versioned prompt
   v
Prompt
   |
   | model + parameters + references
   v
Generation
   |
   | produces one or more
   v
Result
   |
   | cataloged as a durable revision
   v
Asset Version
```

Supporting records attach as follows:

```text
Model      ── describes ─────────────> Generation
Reference  ── supplies input to ─────> Prompt / Generation
Version    ── preserves history for ─> Asset / Prompt / Result
Shot       ── uses ──────────────────> Asset Version
Review     ── evaluates ─────────────> existing Result / Task context
```

This diagram describes relationships only. It does not prescribe table names,
Rust structs, transport commands, or UI screens.

## 14. Risk Analysis

### Data Growth

Generation history and media inventory can grow continuously even for one
person. Large metadata queries and backups may become slow.

**Mitigation:** keep media outside SQLite, index common filters, use bounded
previews, make history/archive policy explicit, and measure real collections.

### Asset Explosion

Every generated candidate can look like a new asset, producing duplicates,
near-duplicates, and an unusable catalog.

**Mitigation:** distinguish stable identity from versions/results, preserve
attempts without automatically promoting every file to a primary asset, offer
user-selected preferred versions, and begin with a small asset vocabulary.

### Duplicate Detection

Checksums identify byte-identical files but not visually or semantically
similar images, audio, or video.

**Mitigation:** use checksums for deterministic duplicate evidence first;
provide explicit user review for probable duplicates; reserve optional local
similarity/vector indexing for later and never make it a destructive merge.

### File Path Change

Drives, folders, permissions, and external media can change. A path-only
identity would break provenance.

**Mitigation:** use stable IDs, retain path plus checksum and relative/archive
references, represent missing/offline files explicitly, and make relinking a
user-confirmed operation.

### Backup Size

Video and audio archives can be much larger than the database and may make
full copies expensive.

**Mitigation:** separate metadata backup from media packaging while keeping a
complete manifest, support incremental or selected-project archives later,
show estimated size, and never claim a backup is complete when files are
missing.

### SQLite Scale

SQLite is a strong local metadata store, but giant blobs, unbounded text, too
many indexes, or long-running searches can cause locking and performance
issues.

**Mitigation:** store metadata rather than media blobs, keep indexes focused,
use FTS only for relevant text, paginate catalog queries, and validate
performance against real personal data before considering another database.

### Migration Complexity

Adding Asset, Prompt, Generation, Version, Model, and Reference relationships
around the v1 production model can accidentally break historical references or
project isolation.

**Mitigation:** design additive migrations, preserve IDs and unknown data,
back up first, validate counts and relationships, keep Queue/Task authority
unchanged, and require a separate implementation/review gate for each schema
step.

## 15. Recommended Next Step

```text
RECOMMENDED_NEXT_STEP=DEV-119_ASSET_LIBRARY_PROTOTYPE_DESIGN
START_NOW=NO
```

The next planning step should be **DEV-119 — Asset Library Prototype Design**.
It should test the smallest useful catalog experience against real personal
examples: project context, asset identity, version history, tags, provenance,
missing-file handling, and search/filter wayfinding.

DEV-119 should remain a prototype/design gate until the product owner approves
the entity vocabulary, project-sharing rule, storage boundary, and migration
strategy. It must not begin Asset Library production implementation or add
SQLite tables as an implicit follow-up to DEV-118.

Until that approval, AI Studio v1.3.1 remains the active stable baseline and
this document remains a planning-only artifact.
