# AI Studio 2.0.0 Personal Edition — release candidate

AI Studio is a local-first Windows desktop workspace for project-owned image
and video production. This document describes the Architecture Reset candidate;
it does not announce a new tag or GitHub Release.

## Current product

- **Project-first workspace:** Overview, Create, Runs and Library share the
  selected project context.
- **Unified Create:** shot selection, image/video stage, prompt, generator,
  parameters and readiness in one product entry. Creating work does not bypass
  the existing Production Queue; Queue Start is the only execution gate.
- **Unified Runs:** execution history, results, visible failure state and
  capability-governed retry/edit-input actions reuse existing task/queue facts.
- **Unified Library:** project-owned media previews, prompt details, explicit
  relations and reuse in Create; referenced deletion is guarded.
- **Advanced Workflow Lab:** exact workflow version + recipe identities,
  version history and technical diagnostics remain in the advanced workspace.
- **System diagnostics:** read-only health, bounded task timelines and grouped
  failures. Missing telemetry is unknown/null, not a zero-duration success.
  Diagnostic ZIP export is local and excludes prompts, credentials, source
  workflow/recipe contents, database files, media bytes and absolute paths.

## Performance and ownership

Measured Runs/Library/Workflow paths reduce duplicate parent reads, redundant
refreshes and repeated preset work. This is not a claim that every workflow or
project is faster. SQLite and filesystem-backed media remain local; existing
repositories own persistence and typed Tauri transport owns frontend access.
There is no new Task, Queue, metrics database, remote telemetry or cloud sync.

## Project archives and compatibility

- Current export format is **Backup v20**.
- The reader explicitly accepts **v1–v20**; compatibility depends on valid
  archived contents. Older formats may omit later v2 data and surface warnings.
- Export, inspect and restore use explicit stored identities and project-owned
  ID remapping, not filenames, paths, timestamps or prompt-text inference.
- Current v20 roundtrip tests cover shot selection/prompts, binding identities,
  asset versions/relations, generation lineage and artifact reviews. v18/v19
  compatibility regressions also pass in the full Rust suite.
- Required media is validated before restore commits. Restore creates a new
  project; it must not overwrite the source project.
- Current database migration maximum is **42** (71 named business tables).
  Database upgrades must be tested on copies; never on a unique original.
- Existing compatibility recipe repairs retain historical versions and expose
  their status. They are not guessed provenance repair or autonomous tool use.

## Installation and acceptance

The candidate uses version `2.0.0-personal` consistently across frontend, Rust
and Tauri. NSIS is the primary Windows installer; MSI availability and actual
installation results must be reported separately. No models are bundled;
ComfyUI and runtime packages remain external prerequisites.

The release checklist and RC baseline record fresh install, upgrade, backup,
uninstall/reinstall, installed-app privacy, installer hashes and exact-source
CI evidence. Checks still marked NOT VERIFIED are not release claims. An
unsigned local installer may require a Windows trust prompt; it is not evidence
of signed publication or an automatic update channel.

## Known issues and non-goals

See `AI_STUDIO_V2_KNOWN_ISSUES.md` for P2 UI polish, explicit Tool Hub discovery,
Prompt statistics, search, unusually large-library tuning and media maintenance.
Release readiness requires no unresolved P0/P1 blockers; see the candidate
checklist for that decision, rather than inferring it from historical evidence.

This candidate does not add audio-input production, standalone audio generation,
new workflow modes, SaaS, cloud sync, multi-user permissions, an AI Agent or a
replacement workflow engine. No expensive new H3 video benchmark is required
by the installer gate; any skipped GPU smoke is explicitly reported.
