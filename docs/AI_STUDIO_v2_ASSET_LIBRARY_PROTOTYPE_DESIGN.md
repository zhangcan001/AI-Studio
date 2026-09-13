# AI Studio v2 Asset Library Prototype Design

```text
TASK=DEV-119
STATUS=PLANNING_ONLY
BASELINE=AI_STUDIO_v1.3.1_STABLE
CODE_CHANGE=NO
DATABASE_CHANGE=NO
SCHEMA_IMPLEMENTATION=NO
UI_IMPLEMENTATION=NO
FILE_SCANNER=NO
IMPORT_IMPLEMENTATION=NO
NEXT_REVIEW=DEV-120_ASSET_LIBRARY_DATA_MODEL_REVIEW
```

This document defines a future Asset Library MVP prototype for review. It is
not an implementation plan that authorizes code, a SQLite table, a scanner, an
importer, CRUD, a page, or a search feature.

## 1. Asset Library Vision

The Asset Library is the memory of a personal AI creative workspace. It is not
a file browser, folder manager, or cloud drive. A file path can tell the user
where a file is; it cannot reliably tell the user what the asset means, which
version it is, what created it, or which production work used it.

The Asset Library should solve four recurring personal problems:

```text
素材越来越多       → 需要可理解的身份与项目上下文
版本混乱           → 需要不可破坏的版本历史
找不到来源         → 需要可读的 provenance
不知道哪个效果最好 → 需要用户选择的 preferred version / result
```

Its core positioning is:

```text
ASSET_LIBRARY=Personal AI Asset Memory System
```

It manages asset identity, creation history, relationships, provenance, and
version evolution. It does not decide creative quality, replace external
generation tools, or become a second production queue.

### Design principles

```text
LOCAL_FIRST=YES
PERSONAL_USE_FIRST=YES
DATA_OWNERSHIP=YES
LONG_TERM_STORAGE=YES
BACKWARD_COMPATIBILITY=YES
LOW_COMPLEXITY=YES
```

The first prototype should be useful with explicit user input and existing
local files. It should prefer a small understandable catalog over automatic
classification or a large abstract asset ontology.

## 2. Asset Definition

### Asset

An **Asset** is a long-term saved creative object with a stable identity,
human meaning, project context, versions, relationships, and provenance.

Examples:

```text
Character
Scene
Voice
Music
Prompt
Reference
```

An Asset can be represented by one or more local files, but its identity is not
the file path. A Character may have reference images, a voice representation,
and several revisions while remaining one recognizable creative concept.

### Generation Result

A **Generation Result** is a concrete output from one generation attempt. It
is evidence of what an external tool produced, not automatically a durable
primary asset.

Examples:

```text
Image attempt 01
Video attempt 03
Audio take 05
```

The user may promote a useful result into an Asset Version. Unselected and
failed results remain discoverable as history when they are important to
explain the creative process.

### Relationship

```text
Asset
  ↓
Generation
  ↓
Result
```

The Asset is the durable concept, Generation is the creation event, and Result
is the concrete output. This separation prevents every candidate file from
becoming an indistinguishable catalog identity while preserving provenance.

## 3. Asset Type Taxonomy

The initial taxonomy is intentionally small and extensible:

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
Reference
```

### Core identity assets

These usually represent reusable creative intent or a long-lived production
identity:

```text
Character
Scene
Prop
Voice
Music
Prompt
```

They may have many concrete media versions and may be reused with explicit
project context.

### Concrete media assets

These usually represent files or deliverables that can be previewed and
versioned:

```text
Image
Video
Audio
```

A concrete media item may be a source, a Generation Result, a selected Asset
Version, or an exported deliverable. The catalog should preserve that role in
metadata and provenance rather than relying on the extension alone.

### Supporting assets

These carry context or inputs for other assets:

```text
Document
Reference
```

Documents can be scripts, briefs, notes, subtitle sources, or other text
inputs. References can point to visual, audio, textual, or external material
used to guide a Prompt or Generation. Supporting does not mean disposable;
they can be long-term owned assets with their own versions.

### Taxonomy rule

Type is a useful filter, not a full ontology. A Character can be related to an
Image, Voice, Prompt, and Document. The design must not force a character's
meaning into a single media type or silently convert a Generation Result into
a primary identity.

## 4. Asset Lifecycle

The Asset Library lifecycle is deliberately independent from the Production
Queue and Task state machine:

```text
Created
   ↓
Active
   ↓
Versioned
   ↓
Archived
```

### State meaning

- **Created:** the identity or candidate has been explicitly recorded.
- **Active:** the asset is available for normal personal use.
- **Versioned:** the asset has meaningful revisions and history that should be
  retained; this is a catalog lifecycle marker, not a production run state.
- **Archived:** the asset is retained for history but is not part of the
  default active working set.

An archived asset can remain searchable and restorable. A missing file is a
reference/availability condition, not a replacement lifecycle state; it should
be shown clearly without deleting the identity.

### Explicit non-states

The Asset Library must not copy the Production state machine or introduce:

```text
RUNNING=NO
FAILED=NO
QUEUE=NO
RETRY=NO
EXECUTION_STATE_MACHINE=NO
```

Generation status belongs to Generation/Task history where applicable. Asset
lifecycle answers whether a catalog identity is available or archived, not
whether a job is executing.

## 5. Asset Metadata Design

The metadata model is conceptual vocabulary for the prototype; it is not a
schema implementation.

### Common metadata

```text
Asset ID
Name
Type
Description
Tags
Location
Checksum
Version
Created Time
Updated Time
Status
Project Usage
Provenance
```

### Field guidance

- **Asset ID:** stable identity independent of name and path.
- **Name:** user-facing label; duplicate names are allowed across projects.
- **Type:** one initial taxonomy category, with future extension only when
  real usage requires it.
- **Description:** short human context and intent.
- **Tags:** optional user-controlled labels, not an automatic classification.
- **Location:** local path, relative workspace reference, or external URI;
  mutable and never the identity.
- **Checksum:** optional deterministic file/content evidence for validation and
  duplicate detection.
- **Version:** current displayed version or selected revision; old versions
  remain preserved.
- **Created Time / Updated Time:** catalog lifecycle timestamps.
- **Status:** Created, Active, Versioned, or Archived, with missing location
  represented separately.
- **Project Usage:** explicit project membership/link context.
- **Provenance:** how the asset was imported, authored, derived, or generated.

### Type-specific extensions

The prototype should show only useful fields for the selected type. Examples:

| Type | Possible extensions |
| --- | --- |
| Character | appearance, role, aliases, reference style |
| Scene | location, time of day, lighting, style |
| Prop | function, material, associated scene |
| Image | dimensions, format, visual role |
| Video | duration, dimensions, frame rate, shot role |
| Audio | duration, format, language, segment role |
| Voice | speaker, language, style, voice source |
| Music | genre, mood, tempo/BPM when known, track role |
| Prompt | template, model, parameters, reference set |
| Document | format, language, source type, text role |
| Reference | source, role, linked asset/version, usage context |

Provider-specific values should remain extensible and should not be discarded
because another asset type does not use them. Unknown values should remain
unknown rather than being guessed.

## 6. Asset Relationship Model

The Library should model explicit, typed relationships instead of relying on
folder structure or matching names.

### Character relationships

```text
Character
├── Reference Images
├── Voice
├── Prompts
├── Projects
└── Generations
```

### Scene relationships

```text
Scene
├── Images
├── Videos
└── Projects
```

### Prompt relationships

```text
Prompt
├── Model
├── Parameters
├── Reference Assets
├── Generations
└── Results
```

### Relationship characteristics

The design must support many-to-many relationships:

- one Character can be used by multiple Projects;
- one Project can use many Characters, Scenes, Props, Prompts, and Results;
- one Prompt can produce multiple Generations and Results;
- one Generation can use many References and produce many Results;
- one Reference Asset can be used by many Prompts or Generations;
- one Scene can relate to many Images and Videos;
- one Asset can have many versions and derivatives.

Relationship records should be able to carry a small amount of context such
as role, order, selected version, usage note, project, and created time. The
relationship itself must not rewrite the source Asset or historical
Generation.

## 7. Project Isolation

### Recommended model

Support reusable assets through a **Global Asset + Project Usage Reference**
pattern rather than copying the asset for every project:

```text
Global Personal Asset
        │
        ├── Project Usage Reference ──> Project A
        └── Project Usage Reference ──> Project B
```

The global label and underlying identity remain shared, while each Project
gets explicit usage context and selected version. This supports the real case
where one Character is used in several projects without creating duplicate
identities.

### Isolation rules

1. Normal creation and browsing are project-scoped by default.
2. Sharing is an explicit user action that creates a visible usage reference.
3. A private project asset must not appear in another project because its name
   or checksum happens to match.
4. Project-specific notes, roles, and preferred versions belong to usage
   context where needed; they must not mutate another project's history.
5. Global search, if introduced later, must show ownership and membership
   context and respect the user's explicit sharing boundary.
6. Deleting a project usage reference must not delete a shared Asset or erase
   historical Generation/Result records.

There is no multi-user permission model here. “Global” means shared within the
single user's local Personal Edition workspace, not publicly or across
accounts.

## 8. Version Strategy

The Asset Library separates a stable identity from concrete versions:

```text
Character_A
├── v1
├── v2
└── v3
```

### Rules

1. Never overwrite the only copy of an old version.
2. The Asset identity remains stable while each Asset Version points to a
   concrete file, representation, or metadata revision.
3. The current version is a user-visible selection, not destructive mutation.
4. Historical Project Usage and Generation records keep their original
   selected version.
5. A new version may be marked preferred, active, archived, or superseded
   while all prior versions remain inspectable.
6. A Generation Result is an attempt output first; promotion to a durable Asset
   Version is explicit or governed by a separately approved workflow.

### Current version selection

The detail view should show the current/preferred version and provide a clear
history list. A Project may select a version for its own usage context without
changing the global current selection for other Projects. If no version is
selected, the UI should say so rather than infer one from recency or filename.

## 9. Provenance Display

When the user opens an Asset, the detail view should make its origin
understandable without requiring them to search logs or external tool folders.

At minimum, show:

```text
Created From
Prompt
Model
Generation
Reference
Project
Date
```

### Provenance presentation

```text
Character X / v2
├── Created From: Generation 2026-09-13-001
├── Prompt: Character consistency prompt / v3
├── Model: MiniMax H3
├── Generation: Attempt 02
├── Reference: Character X reference image / v1
├── Project: Project A
└── Date: recorded creation time
```

The design should distinguish:

- **live navigation links** to the current Asset, Prompt, Model, or Project;
- **historical snapshots** of what the Generation actually used;
- **file evidence** such as location, media type, and checksum;
- **unknown values** where the source did not provide information.

The provenance display should answer:

```text
这个东西怎么来的？
Where from?
How created?
Using what?
```

Changing a current Prompt, moving a file, or selecting a newer Asset Version
must not rewrite historical provenance.

## 10. Import Concept

Import is a future explicit user flow. It is not implemented by DEV-119 and
must not become an automatic file scanner.

```text
Select Files
     ↓
Detect Type
     ↓
Create Asset Candidate
     ↓
Calculate Metadata
     ↓
Confirm
     ↓
Add Asset
```

### Import rules

1. **Select Files:** the user chooses the files or folders to consider; the
   system does not crawl the workspace without consent.
2. **Detect Type:** infer a proposed type from file metadata/extension, but
   always allow user correction.
3. **Create Asset Candidate:** candidates are not assets until confirmed.
4. **Calculate Metadata:** read safe basic metadata and optional checksum;
   failure is reported, not hidden.
5. **Confirm:** the user reviews type, name, project usage, tags, and location.
6. **Add Asset:** only explicit confirmation creates the catalog identity.

There should be no automatic import, silent file movement, silent rename,
automatic project sharing, or destructive duplicate merge. A future importer
must preserve the original location and report missing/unsupported files.

## 11. Duplicate Detection

Duplicate detection is advisory and must never silently merge or delete assets.

### File-level duplicates

```text
Checksum
```

An exact checksum can identify byte-identical files. It is strong evidence for
the same content, but it does not decide whether two catalog identities should
be merged.

### Image-level similarity

```text
Perceptual Hash
```

A perceptual hash may identify visually similar images after resizing or
encoding changes. It should produce a “possible duplicate” review candidate,
not an automatic merge.

### Text duplicates

```text
Content Hash
```

Normalized content hashes may identify identical Documents or Prompt content.
Formatting, language, variables, and provider-specific context can still make
two text records meaningfully different.

### Future media similarity

Audio/video fingerprints or local vector similarity may be considered later,
but they are outside the MVP and must remain rebuildable, local, and
non-destructive. Duplicate evidence should always retain both source records
and the user's decision.

## 12. Storage Relationship

The preferred local boundary is:

```text
SQLite
├── metadata
├── relations
└── history

Filesystem
├── media
├── preview
└── archive
```

### SQLite stores

- Asset identity and common metadata;
- project usage references and typed relationships;
- version and lifecycle history;
- Prompt, Model, Reference, Generation, and Result provenance metadata;
- tags, checksums, locations, and missing-file state;
- archive manifests and validation results;
- future searchable text indexes.

### Filesystem stores

- source documents and prompts when file-backed;
- image, video, audio, voice, and music media;
- previews and thumbnails;
- exported results and archive packages.

A conceptual workspace may be organized as:

```text
AI Studio Workspace
├── projects
├── assets
├── prompts
├── generations
└── archive
```

The exact directory layout is deferred. Stable Asset/Version IDs, not human
folder names, should bind metadata to files. The Asset Library should detect
and report path changes rather than assuming a path is permanent.

## 13. Search Roadmap

Search is planned in layers and is not implemented by DEV-119.

### MVP search

```text
name
tag
type
project
```

The MVP should also make basic recency, lifecycle status, current/preferred
version, and missing-reference filters possible if they remain simple. Every
result should show project context and type so same-named assets are not
ambiguous.

### Future search

```text
FTS
semantic search
vector
```

Full-text search may cover descriptions, notes, Prompt content, provenance,
and model/tool labels. Semantic/vector search is optional future local
discovery and must not become a cloud requirement, automatic classifier, or
prerequisite for the Asset Library.

## 14. Asset Library MVP Scope

The first Asset Library prototype/MVP should do only the smallest useful set:

```text
Asset metadata
Tags
Preview
Relations
Version
Basic search
Provenance display
Project usage context
```

### MVP success criteria

A personal user should be able to:

1. understand what an Asset is without opening its folder;
2. find it by name, tag, type, or project;
3. see its preview and current version;
4. inspect related Characters, Scenes, Prompts, References, Generations, and
   Results;
5. answer how a selected result was created;
6. retain older versions without overwriting them;
7. distinguish a missing file from a deleted Asset identity.

### Explicitly out of MVP

```text
AI_TAGGING=NO
CLOUD=NO
VECTOR_SEARCH=NO
AUTO_CLASSIFICATION=NO
AUTOMATIC_IMPORT=NO
AUTOMATIC_DUPLICATE_MERGE=NO
```

The MVP must not add a production state machine, a second queue, cloud
storage, or autonomous decisions.

## 15. UI Information Architecture

This is a navigation proposal only; no UI is implemented in DEV-119.

```text
Asset Library
├── All Assets
├── Categories
├── Recent
├── Favorites
└── Asset Detail
```

### All Assets

The default catalog view should provide project context, type, name, preview,
current version, updated time, and missing-reference indication. Filters
should be visible and reversible rather than hidden in a complex dashboard.

### Categories

Categories are the initial Asset Type taxonomy: Character, Scene, Prop, Image,
Video, Audio, Voice, Music, Prompt, Document, and Reference. A category view
should not imply that the type is immutable if the user corrects an import
candidate.

### Recent

Recent should be a simple recency view for recently created or updated catalog
records. It must not silently equate recency with quality or preferred status.

### Favorites

Favorites are an explicit user selection for wayfinding. They are not an
automatic “best result” classifier.

### Asset Detail

The detail surface should show:

- Preview;
- Metadata;
- Relations;
- History;
- Provenance;
- project usage and selected version;
- missing-file or archive status when relevant.

The detail view should make the stable Asset identity and concrete Version
identity visually distinct, and should provide clear navigation to related
Generation Results without replacing Production Core screens.

## 16. Technical Risks

### Asset Explosion

Large numbers of generated candidates can overwhelm the catalog and blur the
distinction between durable Assets and Generation Results.

**Mitigation:** keep Generation/Result separate, require explicit promotion to
primary Asset Version, provide preferred selection, and retain a small initial
taxonomy.

### Storage Growth

Video, audio, image, previews, and long generation history can grow faster than
metadata.

**Mitigation:** keep large media on the filesystem, keep SQLite metadata
focused, show archive/inventory state, and make backup scope explicit.

### Duplicate Files

The same file may be copied, re-encoded, renamed, or imported into multiple
contexts.

**Mitigation:** use checksums as deterministic evidence, use perceptual/content
hashes only as advisory candidates, and require user confirmation before any
merge or relink.

### Path Migration

Drives and folders can move or become unavailable, breaking path-only links.

**Mitigation:** stable IDs, relative/archive references, checksum evidence,
missing-file state, and explicit user-confirmed relinking.

### Large Media

Large blobs can make database queries, locking, and backup operations fragile.

**Mitigation:** store media and previews in local files; store descriptors,
relationships, and validation evidence in SQLite.

### Backup Size

Complete media archives may be expensive to copy and restore.

**Mitigation:** separate metadata manifest from media packaging while clearly
reporting completeness, support selected-project archives later, and validate
checksums during restore.

### Search Performance

Catalog size, provenance text, tags, and future full-text indexes may make
unbounded queries slow.

**Mitigation:** project-aware pagination, focused indexes, rebuildable FTS,
simple MVP filters, and measurement against real personal collections before
adding semantic/vector search.

## 17. Recommended Next Step

```text
RECOMMENDED_NEXT_STEP=DEV-120_ASSET_LIBRARY_DATA_MODEL_REVIEW
START_NOW=NO
```

The next step should be **DEV-120 — Asset Library Data Model Review**. Review
should verify that the conceptual model is safe to implement before any table,
Rust model, transport command, file scanner, importer, or UI work begins.

The review should specifically approve:

- Asset versus Generation Result identity;
- the initial type taxonomy and extensibility rule;
- Asset lifecycle boundaries separate from Production states;
- Global Asset + Project Usage Reference isolation behavior;
- version/current/preferred semantics;
- provenance snapshot requirements;
- checksum and advisory duplicate-detection policy;
- SQLite metadata versus filesystem media boundary;
- v1.3.1 compatibility and migration safety.

Until DEV-120 is explicitly approved, AI Studio v1.3.1 remains the active
stable baseline and this document remains a planning-only artifact.
