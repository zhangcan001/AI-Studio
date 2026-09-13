# AI Studio v2 Direction

This document records direction only. It does not start v2 development and
does not change the v1.3.1 product, domain model, schema, migrations, queue, or
production workflow.

## Product direction

```text
NEXT_MAJOR=AI_STUDIO_v2_PERSONAL_EDITION
COMMERCIAL_SAAS=NO
MULTI_USER=NO
AI_AGENT_PLATFORM=NO
```

AI Studio v2 should remain a local, personal production workbench rather than
become a commercial SaaS product, a multi-user collaboration service, or an AI
Agent platform.

## Core areas

- **Asset Library** — a durable local home for source, generated, and reference
  media.
- **Prompt Studio** — a personal workspace for reusable prompt material and
  production inputs, without changing the current external-authoring boundary
  until a future plan explicitly permits it.
- **Local Tool Hub** — a local integration surface for the user's installed
  production tools and runtimes.
- **Project Archive** — dependable local project preservation, restore, and
  retrieval.

## Guardrails for future planning

- Keep local-first operation and project isolation as defaults.
- Preserve the Production Queue as the single execution authority.
- Do not infer a v2 schema, workflow, queue, task, or AI capability from this
  direction note.
- Begin implementation only through a separately approved v2 planning task.
