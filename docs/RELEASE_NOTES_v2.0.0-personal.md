# AI Studio v2.0.0 Personal Edition

AI Studio v2.0.0 Personal Edition is the stable baseline for a local-first AI
creation workspace. It helps one person keep the production path, creative
assets, prompt/model context, local tools, provenance, and project archives in
one durable desktop workspace.

## Core capabilities

### Production Core

- Project, Shot, Task, Generation, Queue, and Review continuity;
- Queue Start remains the only production execution gate; and
- Comfy execution admission covers the actual generation lifecycle rather than
  only the IPC submission call.

### Asset Library

- project-scoped Asset and immutable AssetVersion history;
- explicit AssetRelation and GenerationAssetVersion lineage;
- image, video, and audio media metadata and previews; and
- filesystem media with SQLite metadata, checksums, and safe deletion guards.

### Prompt Studio

- reusable Prompt and PromptVersion records;
- canonical Model and ModelVersion registry; and
- explicit prompt/model context in supported generation provenance.

### Local Tool Hub

- Tool, ToolInstance, ToolVersion, and Capability inventory;
- read-only health state; and
- no automatic installation, process start, process stop, or execution.

### Project Archive

- Backup v19 logical project package;
- export, inspect, and restore with explicit project-owned ID remapping;
- AssetVersion, tool usage, and generation-to-asset lineage preservation; and
- visible UNKNOWN warnings for missing canonical models/tools without guessed
  relations.

## Architecture and data ownership

AI Studio remains local-first: SQLite owns structured metadata and the
filesystem owns media. Rust repositories and application services own
persistence; the React frontend uses typed Tauri transport. Existing Project,
Shot, Task, Queue, Review, Asset, Prompt, Generation, and Comfy authorities are
retained rather than duplicated.

Historical relationships are trustworthy by construction. Restore and lineage
operations use explicit IDs and stored output keys only; filenames, paths,
timestamps, and prompt text are never used to infer a relationship.

## Migration and archive compatibility

- Existing v1.3.1 databases continue through the additive migration path.
- Fresh databases apply the complete migration chain.
- Backup v19 is the stable archive baseline.
- Historical v18 packages remain inspectable/restorable where their contents
  permit; absent v2 data is reported visibly rather than hidden.
- Required media is checked before a restore is committed, preventing partial
  restores that point at missing files.

## Known issues

Non-blocking follow-ups are recorded in
`docs/AI_STUDIO_V2_KNOWN_ISSUES.md`: UI polish, explicit Tool Hub discovery,
Prompt statistics, search enhancement, large-library tuning, and local media
maintenance. `P0=NONE` and `P1=NONE`.

## Verification

The release gate runs the frontend test/type/build checks, Rust format/check/test
checks, and Source-only CI against the exact final release commit. The final
commit SHA and CI run are recorded in the DEV-131-0 completion result.

## Non-goals

This release does not add SaaS, cloud sync, multi-user permissions, an AI
Agent, auto-decision logic, a new Queue/Task/Generation system, or a replacement
workflow engine.
