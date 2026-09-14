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

```text
P0=NONE
P1=NONE
UNKNOWN≠FAILURE_WHEN_VISIBLE
NO_AUTO_REPAIR=YES
```

Known issues must remain visible in future release notes when they affect a
user decision. They must not be silently converted into new product scope or
used to weaken the Queue, Task, provenance, project-isolation, or archive
boundaries.
