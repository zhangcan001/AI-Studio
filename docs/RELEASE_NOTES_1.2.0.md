# AI Studio 1.2.0

AI Studio 1.2 makes the path from structured production input to reviewable
output easier to follow while keeping execution explicit and user-controlled.

## What's new

### External Agent → AI Studio Production Handoff

Import a validated `ProductionHandoffV1` JSON document from an external agent.
Preview the hierarchy, prompts, references, and exact workflow/recipe pairing
before explicitly confirming the import.

### Project Command Center

See preparation, daily production, queue, review, selected results, and
deliverable navigation from one bounded project view.

### Asset and Reference Continuity

Follow imported references, generated outputs, and selected results through
the existing project-scoped Asset Library and Shot surfaces.

### Daily Production and Bulk Preparation

Understand what is ready, running, waiting for review, or complete. Prepare
large Shot selections safely without silently chunking or starting execution.

### Production Queue and Review Inbox

Production Queue remains the sole explicit execution boundary. The bounded
Review Inbox exposes exact Shot, Task, Asset, and selected-result targets.

### Reliability

- Migration compatibility through schema 032.
- Backup V18 and legacy backup restore compatibility.
- 500-Shot regression coverage.
- Expanded frontend and Rust regression coverage.
- Windows MSI, NSIS, and portable application artifacts.

## Upgrade notes

AI Studio 1.2.0 preserves existing project data and upgrades the database from
the 1.1 migration state through migration 032. Back up important projects
before upgrading as a normal precaution.

AI Studio does not start production automatically after handoff, preparation,
or review rework. Start work explicitly from Production Queue.
