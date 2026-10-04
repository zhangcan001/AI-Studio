# AI Studio v2 Personal Edition Known Issues

```makefile
VERSION=v2.0.0-personal
P0=NONE
P1=NONE
STATUS=NON_BLOCKING_FOLLOW_UP
```

These items are known quality or convenience improvements. They do not block
the local-first Personal Edition baseline and are not part of the frozen
release scope.

## Tracked follow-ups

| Priority | Item | Current behavior | Boundary |
| --- | --- | --- | --- |
| P2 | UI detail polish | Some list/detail surfaces can be made denser and easier to scan | No domain or execution change needed |
| P2 | Tool Hub discovery | Tools are registered/observed explicitly; automatic discovery is not provided | Must not install, start, stop, or execute a process implicitly |
| P2 | Prompt statistics | Prompt Studio does not yet provide aggregate usage or quality statistics | Must not add AI optimization or automatic decisions |
| P2 | Search enhancement | Cross-module search remains basic and module-oriented | Measure real personal collections before adding a search subsystem |
| P2 | Large-library performance | Additional indexing and pagination tuning may help unusually large libraries | Preserve SQLite/local-first storage and project isolation |
| P2 | Local media maintenance | Moved or unavailable external files require explicit user maintenance | Never repair provenance from a path or filename guess |

## Explicit non-blocking policy

The production execution-authority P0 is closed: all three compatibility
submission commands now enqueue through the existing Production Queue, and
only Queue Start dispatches product work to the retained `GenerationService`
worker. This corrects execution authority, not the number of internal executor
implementations (`single executor != single execution authority`). Evidence is
the `production_execution_requires_queue_start` architecture test and the
queue-before-task/Comfy, Shot-linkage, and retry-lineage behavior tests in
`src-tauri/tests/dev052_runtime_integration.rs`.

```text
P0=NONE
P1=NONE
UNKNOWN≠FAILURE_WHEN_VISIBLE
NO_GUESSED_RELATION_REPAIR=YES
```

Known issues must remain visible in future release notes when they affect a
user decision. They must not be silently converted into new product scope or
used to weaken the Queue, Task, provenance, project-isolation, or archive
boundaries.

Recipe compatibility repair jobs are a separate, existing startup/import
mechanism: they retain historical recipe versions, publish repaired versions,
and may retarget affected bindings/publication through the existing lifecycle
contract. Their status is visible in System Settings. The no-guessing policy
does not mean these repair jobs are disabled. Tool Hub does not implicitly
install or start external tools.

The Architecture Reset release candidate is not a new tag or GitHub Release.
Its installer, upgrade and privacy acceptance must be tracked separately in
the candidate checklist; historical P0/P1 closure is not proof those checks
have already been completed for a new bundle.

## Candidate setup boundaries

The five seeded image/H3 workflow definitions still require an explicit runtime
capability/node refresh, compatible ComfyUI nodes and external model weights
before becoming production-ready. Fresh package presence is not readiness.
Unavailable runtime/package reads remain visibly UNKNOWN rather than healthy.

Generic advanced panels may retain unavailable audio-related mode labels, and
the Library can view imported/historical audio. These do not promise audio-input
production, standalone audio generation or image-plus-audio generation in the
shipped image/H3 scope. Removing dormant generic labels is a P2 presentation
follow-up, not a new release feature.

The Windows candidate is unsigned. NSIS installation/uninstallation/reinstallation
is the validated path. MSI generation and numeric metadata are validated, but
MSI installation is NOT VERIFIED in this gate; do not describe it as tested.

Two candidate blockers were repaired: nonnumeric personal prerelease metadata
prevented MSI bundling; omitted empty restore-report arrays crashed the typed
UI after successful archive restoration. Both have red/green regressions; the
final-source NSIS app reaccepted restoration without a white screen. Final
publication still requires the candidate's exact-HEAD CI authority.
