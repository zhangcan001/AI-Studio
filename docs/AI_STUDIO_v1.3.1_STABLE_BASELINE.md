# AI Studio v1.3.1 Stable Baseline

```text
VERSION=1.3.1
STABLE_SHA=81b204566605c73f840b231b7b981f2c377813cd
FEATURE_COMPLETE=YES
PRODUCTION_FLOW_COMPLETE=YES
PERSONAL_USE_READY=YES
KNOWN_LIMITATIONS=CI-113-01; FE-113-03; PERF-113-02
NEXT_MAJOR_DIRECTION=AI_STUDIO_v2_PERSONAL_EDITION
```

## Baseline meaning

AI Studio v1.3.1 is the stable maintenance baseline after the DEV-116
reliability gate. Its runtime/source baseline is the DEV-116 implementation
head above; DEV-117 adds only release-closeout documentation and the release
metadata around that stable source.

The existing Production Queue remains the only execution authority. Project
isolation, Studio Store authority, exact `workflowVersionId + recipeId`
identity, historical references, and database compatibility remain frozen.

## Readiness

- **Feature complete:** the v1.3 product scope is closed; no DEV-117 feature
  work is included.
- **Production flow complete:** Prepare → Queue → Start → Monitor → Review →
  Rework remains the validated path, with Queue Start as the execution gate.
- **Personal use ready:** local Windows desktop use is the target operating
  baseline; no SaaS or multi-user service assumptions are added.

## Limitations

The three known non-blocking limitations remain documented rather than expanded:
manual Source-only CI dispatch (`CI-113-01`), technical diagnostic vocabulary
(`FE-113-03`), and bounded large-collection wayfinding (`PERF-113-02`).
