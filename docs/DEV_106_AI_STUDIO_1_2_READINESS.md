# DEV-106 — AI Studio 1.2 readiness gate

```text
TASK=DEV-106
BASELINE=87d286d9e4068a168f47fa2e8ab361c835dc6252
READINESS_CODE_HEAD=5308f3ca71b7df5d7923bcd2b336c9242a171d38

PRODUCT_POLISH=PASS
HANDOFF_JSON_SCHEMA=PASS
HANDOFF_EXAMPLE=PASS
HANDOFF_PROMPT_TEMPLATE=PASS
HANDOFF_SCHEMA_PARITY=PASS
500_SHOT_UAT=PASS
EXTERNAL_HANDOFF_E2E=PASS
DAILY_PRODUCTION=PASS
ASSET_CONTINUITY=PASS
BULK_PREPARATION=PASS
PRODUCTION_QUEUE=PASS
REVIEW_INBOX=PASS
REWORK_EXPLICIT_START=PASS

LATEST_MIGRATION=032
BACKUP_VERSION=18
FRESH_DB_001_TO_032=PASS
UPGRADE_1_0_TO_032=PASS
UPGRADE_1_1_TO_032=PASS
BACKUP_V18_ROUNDTRIP=PASS
LEGACY_V17_RESTORE=PASS
LEGACY_BACKUP_RESTORE=PASS
HANDOFF_PROVENANCE_RESTORE=PASS
ASSET_RELATION_RESTORE=PASS

FRONTEND_TEST=PASS (148 files; 799 tests)
TSC=PASS
FRONTEND_BUILD=PASS
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_TEST=PASS (1042 passed; 0 failed; 3 ignored)
TAURI_BUILD=PASS (MSI and NSIS produced; not published)
ISOLATED_APP_LAUNCH=PASS (fresh DB migrated to 032)
ARCHITECTURE_GUARD=PASS

REMOTE_CI_RUN=34679501578
REMOTE_CI_URL=https://github.com/zhangcan001/AI-Studio/actions/runs/34679501578
REMOTE_CI_EVENT=workflow_dispatch
REMOTE_CI_HEAD=5308f3ca71b7df5d7923bcd2b336c9242a171d38
REMOTE_CI_STATUS=SUCCESS (Frontend source checks; Rust source checks)

REAL_COMFY_SMOKE=NOT_RUN_ENVIRONMENT_UNAVAILABLE
P0=NONE
P1_RELEASE_BLOCKER=NONE
AI_STUDIO_1_2_READINESS=PASS
DEV_107=READY
DEV_107_STARTED=NO
```

## Product phase and synthetic regression

| Product slice | Gate evidence |
| --- | --- |
| Production Continuity (DEV-099) | Project Command Center derives existing exact navigation targets, with 500-Shot set-based summary regression. |
| External Agent Handoff (DEV-101) | V1 JSON preview → explicit confirm → formal hierarchy/provenance, replay and project isolation; new 500-Shot fixture confirms zero Queue batches/Tasks and 501 rejection. |
| Asset/Reference Continuity (DEV-102) | Handoff Asset reference appears in existing reverse usage; backup preserves and remaps references; selected result opens its exact Asset. |
| Daily Production (DEV-103) | Existing derived five-bucket projection is bounded to 20 items per bucket; 500-Shot Command Center regression retains true totals. |
| Bulk Preparation (DEV-104) | 500-Shot preflight uses one bounded batch path; 101 selection rejects without writes/chunking; 100 READY Shots create one batch, zero Tasks/Comfy submits. |
| Execution/Review (DEV-105) | Production Queue remains sole explicit Start authority; existing Task cancellation/recovery/review regressions pass; Review Inbox is bounded/paged, exact-target and read-only; rework creates READY batch only. |
| Agent tooling (DEV-106) | Draft 2020-12 schema, canonical example, provider-neutral instruction template, Rust contract parity test, JSON-Schema validation, and architecture-guard sentinels pass. Server-only aggregate/UTF-8/identity rules remain documented as `SERVER_VALIDATED`. |
| Product polish (DEV-106) | [Journey audit](DEV_106_PRODUCT_POLISH_AUDIT.md), handoff format/copy/path help, business-language import entry, and exact selected-result link with technical IDs folded away. |

The synthetic UAT is a regression **matrix** over public handoff, backup,
preparation, Queue, review, and navigation boundaries; it is not represented
as one continuous live ComfyUI run. It uses test/fake runtime paths and does
not mutate a user's production project. The real endpoints
`/system_stats` and `/object_info` at `127.0.0.1:8188` were unavailable.

## Compatibility, architecture, and installer boundary

- Fresh migration 001→032, reconstructed 1.0 migration 026→032, and
  reconstructed 1.1 migration 031→032 passed through the real initializer.
- Backup V18 export/inspect/restore passed with handoff identity and four
  source-to-formal mapping remaps. Legacy V17 restore compatibility and Asset
  relation restore remain covered by the full Rust gate.
- Guard confirms no new queue/executor/task model, no resurrected internal
  authoring, no raw frontend invoke, no application-layer direct SQLx, and
  exact `workflowVersionId + recipeId` identity. Handoff/prepare/rework do
  not auto-start or auto-rebind.
- The current `1.1.0` development binary built both installers. A hidden
  launch with `AI_STUDIO_DATA_ROOT` pointing to an isolated temporary root
  stayed running and created a fresh database at migration 032. Neither
  installer was installed, published, or exercised as a 1.1→1.2 upgrade;
  that release-specific validation belongs to DEV-107.
- Frontend's existing large-chunk build warning and three pre-existing
  ignored Rust tests are non-blocking. A live 500-Shot desktop performance
  profile and wider copy polish remain P2; no P0/P1 release blocker was found.

```text
AI_STUDIO_1_2_FEATURE_FREEZE=YES
POST_1_1_PRODUCT_PHASE=CLOSED
MANIFEST_VERSION=1.1.0
VERSION_BUMP=NO
TAG_CREATED=NO
GITHUB_RELEASE_CREATED=NO
RELEASE_ASSETS_UPLOADED=NO
AUTO_RELEASE=NO
AUTO_NEXT_TASK=NO
STOP_AFTER_DEV_106=YES
```
