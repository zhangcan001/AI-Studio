# AI Studio v2.0 Personal Edition Architecture Plan

```text
PLAN_STATUS=PLANNING_ONLY
BASELINE=AI_STUDIO_v1.3.1_STABLE
CODE_CHANGE=NO
DATABASE_MIGRATION=NO
UI_CHANGE=NO
FEATURE_IMPLEMENTATION=NO
NEXT_MAJOR=AI_STUDIO_v2_PERSONAL_EDITION
```

This document is a product and architecture plan for review. It does not
authorize implementation, change the v1.3.1 baseline, or imply a new schema,
workflow, queue, task model, or AI capability.

## 1. Product Vision

AI Studio v2.0 is a local, personal AI creation workbench that manages the
full production lifecycle around the user's creative work. It is not an
“AI generation tool collection”. Its value is continuity: the person can keep
projects, source material, prompts, references, generated results, decisions,
versions, and production status connected in one place.

The workbench should answer four practical questions without requiring the
user to reconstruct context from folders and chat history:

1. What am I making, and which project does it belong to?
2. Which source, reference, prompt, model, and parameter produced this result?
3. What is the current version and what should be reviewed or used next?
4. Where is the durable local copy, and how can the project be restored later?

The existing Production Core remains the execution boundary. AI Studio should
organize and explain personal production work without becoming the provider of
every generation runtime or an autonomous decision-maker.

## 2. Product Position and Planning Principles

```text
PRODUCT=AI Studio Personal Edition
DEPLOYMENT=Local Windows desktop application
LOCAL_FIRST=YES
PERSONAL_USE_FIRST=YES
DATA_OWNERSHIP=YES
LOW_COMPLEXITY=YES
LONG_TERM_MAINTAINABLE=YES
```

The architecture should favor explicit records, inspectable files, reversible
operations, and small stable interfaces over automation that hides decisions.
External generation tools remain external tools. AI Studio owns the personal
project context and production record, not the provider's runtime internals.

## 3. User Scenarios

### 3.1 AI animation / AI comic production

```text
小说/原始素材
    ↓
剧本
    ↓
分镜
    ↓
角色与场景设定
    ↓
图片
    ↓
视频
    ↓
声音
    ↓
成片
```

AI Studio's role is to keep the project, shots, character and scene
references, prompts, generated assets, review decisions, and exported results
connected. The user may author narrative and prompts in external tools or
manually; v2 planning does not remove the existing external-authoring
boundary. Production submission continues through the existing Queue Start
gate.

### 3.2 TTS / radio drama

```text
文本
    ↓
角色
    ↓
音色
    ↓
配音
    ↓
混音
```

The useful continuity is the relationship between a text segment, character,
voice/model settings, rendered audio versions, review notes, and the final
mix. The source text and audio files remain user-owned local artifacts.

### 3.3 AI music

```text
需求
    ↓
Prompt
    ↓
生成
    ↓
版本管理
```

The workbench should preserve the prompt, model, parameters, references,
generation result, chosen version, and export location together. It should not
decide which track is creatively correct; the user makes that decision.

## 4. v2.0 Module Plan

### Module A — Production Core

**Purpose:** inherit the validated v1.3 production wayfinding and execution
boundary.

**Scope:**

- Project
- Shot
- Queue
- Review
- Existing Prepare → Queue → Start → Monitor → Review → Rework path

**Architecture rule:** do not refactor the Production Core as part of the v2
planning exercise. The Production Queue remains the sole production execution
authority, Queue Start remains the only execution gate, Studio Store remains
authoritative for frontend state, and the exact workflow reference remains the
`workflowVersionId + recipeId` pair.

### Module B — Asset Library

**Priority:** highest v2 priority.

**Purpose:** provide one durable local catalog for personal creative assets and
their production context.

**Asset categories:**

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
```

#### Conceptual data structure

An Asset is a project-scoped catalog record with a stable identity, human
label, type, lifecycle state, local file reference or external reference,
metadata, tags, provenance, and timestamps. The binary media is not required
to live inside SQLite. SQLite should own searchable metadata and relationships;
the local file system should own large content, subject to an explicit archive
and backup policy.

Every asset should be able to describe:

- ownership: project and optional collection/context;
- kind: character, scene, prop, image, video, audio, voice, music, or prompt;
- status: draft, active, archived, or missing-reference;
- file or URI reference, media type, size, checksum, and preview information;
- tags and human-readable notes;
- source/provenance and the generation or import that produced it;
- parent, derivative, replacement, and related-asset links;
- created, updated, and last-used timestamps.

This is a conceptual model only. No v2 table or migration is introduced by
this plan.

#### Search and tags

Search should start with a simple local index over project, asset name,
category, tags, notes, source, model, and referenced prompt. It should support
exact filters first (project, type, tag, status, version) and text search
second. Search results must retain project isolation and should never infer an
asset's identity from a display name alone.

Tags should be user-controlled, reusable, and optional. The initial design
should avoid a mandatory tag taxonomy or an automatic semantic classifier.
Useful facets may include `character:main`, `scene:night`, `status:approved`,
or `source:comfyui`, but the user remains the authority.

#### Versions and relationships

An asset identity represents the durable creative concept; versions represent
its concrete revisions or generated files. A version should preserve its
source asset, file/reference, provenance, creation time, checksum when
available, and user-selected status. Relationships should be explicit:

```text
Asset ── has versions ──> AssetVersion
Asset ── references ────> Asset
Asset ── derived from ──> Asset
Generation ── produces ─> AssetVersion
Shot ── uses ───────────> AssetVersion
Prompt ── references ───> Asset
```

The design should support “used by”, “derived from”, and “replaced by”
without copying media or silently changing historical production records.

### Module C — Prompt Studio

**Purpose:** make reusable production instructions and their outcomes
inspectable while preserving the user's control over authoring and model
choice.

**Conceptual records:**

```text
Prompt Template
Model
Parameter Set
Reference Set
Generation Request
Generation Result
Prompt Version
```

Prompt Studio should support:

- versioned prompt templates with optional variables and notes;
- model/provider name and capability metadata as user-maintained records;
- explicit parameter sets, including seed and output constraints when the
  external tool exposes them;
- references linked to Asset Library assets or local files;
- result records linked to the exact prompt version, model, parameters,
  references, project, and external tool;
- comparison of results and user-selected version status;
- coverage for MiniMax H3, ComfyUI, TTS, and music models through adapters or
  recorded external-run metadata, not through a mandatory common runtime.

The first design target is provenance and reuse, not an in-app editor that
replaces every external authoring surface. A generation record is evidence of
what the user asked an external tool to do; it is not an instruction for an AI
Agent to decide the next action.

### Module D — Local Tool Hub

**Purpose:** provide a consistent local management entry point for installed
tools and runtimes.

**Candidate tools:**

```text
ComfyUI
IndexTTS
ACE-Step
Agnes Creator
VRBoxPlayer
```

The Tool Hub should record a tool's display name, local endpoint or executable
location, capability labels, version, health/readiness information, and user
preferences. It may offer launch, inspect, open-output, and connection-test
actions where safe and explicit.

The Tool Hub is not a replacement runtime and not a second execution queue. It
must not silently submit production work, make scheduling decisions, or create
a competing task model. Any production execution remains under the existing
Production Queue authority and its existing transport boundaries.

### Module E — Project Archive

**Purpose:** preserve and recover a complete personal project without making
the user depend on a cloud service.

**Scope:**

- project snapshots and named milestones;
- metadata, asset references, prompt records, generations, and review history;
- local media inventory and missing-file detection;
- export/import package definition;
- backup preview, restore validation, and conflict reporting;
- archive browsing by project, date, version, and status.

Archive operations should be explicit, inspectable, and resumable. A backup
must not silently overwrite a newer project or discard historical task and
production references. The archive format should remain usable even if an
external generation tool is no longer installed.

## 5. Conceptual Data Architecture

The following records are candidates for a future v2 domain model. They are
not implemented by this plan.

### Core records

| Record | Responsibility | Key relationship |
| --- | --- | --- |
| Project | Ownership boundary for personal work | owns assets, prompts, generations, and archive snapshots |
| Asset | Stable creative concept or catalog item | has versions; relates to shots and prompts |
| Prompt | Reusable instruction and its versions | references assets; used by generations |
| Generation | Provenance record for one external generation attempt | uses prompt/model/parameters/references; produces results |
| Version | Concrete revision of an asset, prompt, or project output | belongs to a parent identity and preserves history |
| Model | User-maintained description of an external model/provider | referenced by generations and tool capabilities |
| Reference | Explicit input used by a prompt or generation | points to an asset or local file with provenance |

### Relationship rules

1. Project is the isolation boundary. Queries and operations must be
   project-scoped by default.
2. Stable IDs, not names, identify Project, Asset, Prompt, Generation, Model,
   Version, and Reference records.
3. A Generation is append-oriented provenance. Correcting a prompt or model
   record must not rewrite historical generation facts.
4. A Version points to its parent identity and preserves the previous version;
   “current” is a user-visible selection, not destructive replacement.
5. A Reference records what was used at generation time, including a stable
   asset/version reference where available.
6. Large media stays outside SQLite by default; metadata, checksums, links,
   and indexing information stay in SQLite.
7. The existing task and production references remain compatible. v2 records
   must not reinterpret historical workflow or recipe identity.

### Persistence direction

The likely long-term split is:

```text
SQLite       = project-scoped metadata, relationships, versions, search index
Local files  = source and generated media, previews, archive packages
Tauri/Rust   = persistence ports, file operations, validation, and boundaries
React        = views, filters, review surfaces, and user actions
```

This is a direction for design review, not a request to add tables, migrations,
file movers, or UI in the current task.

## 6. Technical Route Evaluation

### Continue with Tauri + React + Rust + SQLite

**Recommendation: YES.** The current stack remains the best fit for the
Personal Edition because it already provides:

- a Windows desktop distribution model with local filesystem access;
- React/TypeScript for dense catalog, filter, review, and archive surfaces;
- Rust for explicit persistence boundaries, file safety, validation, and
  integration adapters;
- SQLite for a dependable local metadata store with transactions, indexes,
  backup support, and no service deployment burden.

Changing stacks would add migration and operational cost without solving the
central v2 problem, which is continuity of personal production context.

### Constraints for the continued stack

- Keep typed transport in `src/services/tauriClient.ts` and
  `src/services/ipc.ts`; feature modules must not call raw Tauri `invoke`.
- Keep SQLite persistence behind Rust repository ports and abstractions.
- Preserve Studio Store authority rather than creating a parallel frontend
  state source.
- Preserve the Production Queue as the only production execution authority.
- Prefer metadata plus file references over storing large media blobs in
  SQLite.
- Keep migrations backward-compatible when a future implementation is
  separately approved; this planning task adds none.
- Make local paths portable where possible and detect missing/offline media
  rather than deleting records.
- Use capability adapters for external tools so the core remains stable when a
  provider changes its endpoint or file format.

## 7. Non Goals

AI Studio v2.0 Personal Edition explicitly does **not** target:

```text
AI_AGENT=NO
AUTO_DECISION=NO
CLOUD_SYNC=NO
MULTI_USER=NO
PERMISSION_SYSTEM=NO
SAAS=NO
COMMERCIAL_PLATFORM=NO
ENTERPRISE_SYSTEM=NO
CLOUD_PLATFORM=NO
SECOND_PRODUCTION_QUEUE=NO
SECOND_TASK_MODEL=NO
AUTONOMOUS_PRODUCTION_SUBMISSION=NO
```

In particular, the plan does not authorize an Agent that selects tools,
rewrites prompts, schedules jobs, or approves results automatically. It also
does not authorize cloud synchronization, account management, team
permissions, or a replacement for ComfyUI, TTS, music, or playback tools.

## 8. v2.0 Roadmap

This roadmap is a sequencing proposal for a future implementation program.
Each phase requires separate product-owner approval before code, schema, or
migration work begins.

### v2.0 Phase 1 — Personal continuity foundation (1–2 months)

Highest-value scope:

1. Define and validate the project-scoped Asset Library vocabulary and
   provenance rules.
2. Add a durable asset catalog with local references, tags, filters, previews,
   versions, missing-file detection, and explicit related-asset links.
3. Add Prompt Studio records for prompt versions, model metadata, parameter
   sets, references, and generation provenance.
4. Connect existing Project, Shot, Review, and Queue views to asset and prompt
   context without changing Production Core execution authority.
5. Establish the first local archive package and restore-validation workflow.

Phase 1 success means a person can locate an asset, understand how a result
was produced, select a version, and preserve the project locally without
reconstructing context manually.

### v2.0 Phase 2 — Local tool continuity

- Add the Local Tool Hub registry and explicit health/readiness checks.
- Add launch/open-output/connection-test actions for supported local tools.
- Add capability and model records for ComfyUI, IndexTTS, ACE-Step, Agnes
  Creator, and VRBoxPlayer where local interfaces are stable.
- Improve generation import/provenance capture while keeping submission and
  execution in the existing Queue path.
- Improve archive previews, restore conflict reporting, and media inventory.

### v2.0 Phase 3 — Long-term personal workspace

- Add richer cross-project personal search only where it preserves isolation
  and explicit ownership.
- Add durable project milestones, comparison surfaces, and export formats.
- Optimize indexing and media discovery for large local collections.
- Add optional local quality-of-life automation only when it remains explicit,
  reversible, and non-autonomous.
- Reassess storage and archive performance using real personal workloads before
  considering any architectural expansion.

## 9. Risk Analysis

### Data growth

Images, video, audio, previews, and generation history can grow much faster
than metadata. Storing all binaries in SQLite would increase backup time,
locking, and recovery risk.

**Mitigation:** keep large files on the local filesystem, store metadata and
checksums in SQLite, support inventory and missing-file states, and make
archive packaging explicit.

### Asset management complexity

Characters, scenes, prompts, results, derivatives, and versions overlap. An
overly rigid taxonomy will become a burden; an unstructured folder mirror will
not provide continuity.

**Mitigation:** start with a small set of asset kinds, user-controlled tags,
explicit relationships, stable IDs, and append-oriented versions. Add
taxonomy only when real usage demonstrates a repeated need.

### SQLite scale

SQLite is suitable for a personal metadata store, but unbounded event history,
large text search, thumbnails, or media blobs can degrade query and backup
performance.

**Mitigation:** index project and frequently filtered fields, keep blobs out of
the database, use bounded previews, archive old history deliberately, and
measure real collection sizes before introducing more infrastructure.

### UI complexity

Asset Library, Prompt Studio, Tool Hub, Archive, Production Core, and Review
can produce a dense application with competing navigation concepts.

**Mitigation:** preserve the existing navigation and Production Core, make
project context visible, use progressive disclosure, begin with search/filter
and detail surfaces, and avoid a dashboard that duplicates every existing
screen.

### Local storage and path stability

Users can move drives, rename folders, disconnect external media, or lose
permissions. A catalog that treats a path as the asset identity will become
fragile.

**Mitigation:** use stable catalog IDs, retain original path plus checksum and
optional relative/archive references, report missing/offline media clearly, and
make relinking an explicit user action.

### External tool drift

ComfyUI, TTS, music, creator, and playback tools may change endpoints,
capabilities, or output formats.

**Mitigation:** isolate integrations behind small capability adapters, record
the observed tool/model version, keep imported provenance usable offline, and
avoid making the core depend on one provider's API.

## 10. Feasibility and Personal Value

```text
AI_STUDIO_v2_FEASIBILITY=HIGH
PERSONAL_VALUE=HIGH
```

The feasibility rating is high because the direction extends the existing
local desktop stack and validated Production Core rather than replacing it.
The personal value is high because the main unsolved problem is continuity of
creative context across files, prompts, tools, and versions—exactly the area
where a local personal workbench can provide durable ownership without the
complexity of SaaS or autonomous agents.

## 11. Review Gate

Before any v2 implementation begins, the product owner should approve:

- the Asset Library identity/version/provenance vocabulary;
- the project isolation and local file reference policy;
- the Prompt Studio provenance boundary;
- the Tool Hub “management entry, not replacement runtime” boundary;
- the archive package and restore safety expectations;
- the Phase 1 scope and success criteria.

Until that review is complete, the v1.3.1 stable baseline remains the active
product baseline and this document remains planning-only.
