# AI Studio v2 Personal Edition Stable Baseline

```text
VERSION=v2.0.0-personal
BASELINE_STATUS=READY
BASELINE_REF=v2.0.0-personal
```

## Product positioning

AI Studio v2 Personal Edition is a **local-first AI creation workspace** for
one person's end-to-end creative production. It organizes projects, prompts,
models, tools, generations, assets, provenance, and local archives without
trying to become a general AI generation marketplace.

It is explicitly not:

- a SaaS product;
- a cloud platform;
- a multi-user or enterprise system; or
- an AI Agent platform.

## Completed modules

### Production Core

- Project, Shot, Task, and Generation history;
- persistent Production Queue and Review/Rework continuity; and
- Comfy Execution Admission held through the real generation lifecycle.

### Asset Library

- project-scoped Asset and AssetVersion;
- explicit AssetRelation;
- previews, metadata, and filesystem-backed media; and
- GenerationAssetVersion provenance.

### Prompt Studio

- canonical Prompt and PromptVersion;
- Model and immutable ModelVersion registry; and
- explicit prompt/model provenance on supported production records.

### Local Tool Hub

- Tool;
- ToolInstance;
- observed ToolVersion; and
- Capability metadata with read-only health state.

The Tool Hub is an inventory and visibility boundary. It does not install,
start, stop, or execute tools.

### Project Archive

- Backup v19 at the original v2.0.0-personal release baseline; current trunk extends it additively as Backup v20 for ArtifactReview;
- logical project export and restore;
- explicit project-owned ID remapping;
- AssetVersion and provenance restoration; and
- visible UNKNOWN handling for unavailable canonical models or tools.

## Data architecture

The stable conceptual entities are:

```text
Project
Asset
AssetVersion
Prompt
PromptVersion
Model
ModelVersion
Tool
ToolInstance
ToolVersion
Generation
GenerationToolUsage
GenerationAssetVersion
Archive
```

`Generation` is the existing Task/Generation production authority; it is not a
new parallel table or executor. `Archive` denotes the existing logical backup
package/service boundary, not a new database authority.

Metadata is stored in SQLite through Rust repositories and application
services. Media and other large files remain filesystem-backed. The frontend
uses typed Tauri transport and never accesses SQLite directly.

## Core design principles

### Single authority

Each domain has one source of truth. The baseline does not introduce a second
Asset, Generation, or Queue system. Production Queue remains the only
production execution gate, and Task remains the Generation fact carrier.

### Trustworthy history

Relationships are created or restored only from explicit stable IDs and stored
output keys. The system never guesses historical relations from filenames,
paths, timestamps, or prompt text.

### Local first

The personal workspace owns its SQLite metadata and filesystem media locally.
Cloud sync, multi-user permissions, SaaS concerns, and autonomous decisions are
outside this baseline.

## Freeze boundary

This document records the v2 Personal Edition baseline only. Future work must
be separately approved and must not silently change the frozen architecture,
archive contract, production authority, or historical meaning of existing
records.
