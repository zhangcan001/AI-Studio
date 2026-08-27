# DEV-053 Review Productivity + A/B Candidate Compare

## Baseline

- `DEV053_START_SHA`: `11b9a185315c4185180313458041844ac69c269b`
- Branch: `master`
- The working tree was clean and `HEAD == origin/master` before development.
- DEV-052 closure gates were green before implementation.

## Existing Review Semantics

DEV-053 keeps the existing `production_item_reviews` model and its authoritative
status values: `UNREVIEWED`, `APPROVED`, `STARRED`, `REGENERATE`, and
`REJECTED`. Review status remains separate from Shot candidate selection.

The existing Shot fields `selected_image_asset_id` and
`selected_video_asset_id` remain the source of truth for the adopted result.
The review read facade exposes the enriched productivity view through the
existing `production_item_review_get` command, so the legacy review contract
continues to work.

## A/B Compare

`ReviewCompareWorkspace` provides two independent A/B slots, a candidate strip,
slot swap, previous/next navigation, and explicit review actions. Selecting a
candidate only changes local A/B state. It does not write a Shot selection, a
review status, or a generation request.

Images reuse `ZoomableImagePreview`. Videos use the existing asset media URL
with native `controls`, `preload="metadata"`, and `playsInline`.

The existing Review Rail and `ShotBatchReviewBoard` remain the entry points.
The board preserves the quick review controls and opens the compare workspace.
The detailed productivity adapter is available for a review batch; the current
Shot workspace also has a local A/B fallback when no batch id is supplied.

## Manual Selection Rule

“确认并通过” first calls the existing Shot selection API. Only after that call
succeeds does it set the review status to `APPROVED`. If the second call fails,
the UI reports exactly:

> 候选已设为采用结果，但审片状态未更新，请重新点击通过。

Items without a Shot link can still receive review statuses, but they do not
expose a final Shot-selection action.

## Review Status

The compare actions map directly to the existing status API:

- 仅通过 → `APPROVED`
- 标星 → `STARRED`
- 拒绝 → `REJECTED`
- 标记返工 → `REGENERATE`
- 保存备注 → existing 4 KiB review-note limit

No second review state model was introduced.

## Frozen Snapshot Context

The backend loads preparation snapshots in one batch-scoped call and prefers
their immutable prompt, negative prompt, workflow, recipe, context hash,
reference summaries, output specification, stage input, and historical
readiness status. Reference asset summaries include `assetId`, `sha256`, role,
ordinal, and source reference-set identity. The UI displays a shortened SHA and
keeps the complete value available through the element title/accessible label.

The review page does not call live ComfyUI preflight and does not resolve the
current Shot context. Historical review therefore remains stable if profiles,
reference sets, or current ComfyUI state change later.

## Legacy Fallback

Items without a preparation snapshot remain reviewable. The fallback reads
legacy `ProductionBatchItem.valuesJson` plus the stored workflow and recipe
identifiers, and shows the exact marker:

> 旧版任务，无生产准备快照

Legacy reference SHA values are left empty because no historical hash is
available; current names are not presented as historical names.

## Regenerate Safety

The review commands retain the old wire-compatible `autoStart` fields but force
`auto_start: false` / `autoStart: false`. “创建返工批次” creates a READY batch
and offers navigation to the existing production queue. It never calls the
queue start API, creates a second queue, or submits directly to ComfyUI.

The existing image retry path remains the Shot workspace retry/requeue path.
The review productivity read path performs no preflight, generation, queue
start, or media-byte hydration for the entire batch.

## autoStart=false

This is enforced at both boundaries:

- Rust review commands ignore a caller-provided `auto_start: true`.
- The TypeScript client wrapper always sends `autoStart: false`.

## ComfyUI Boundary

ComfyUI remains the only formal generation engine. DEV-053 adds no adapter,
executor, queue, migration, or remote generation provider. Review actions only
read history, update review state, select an existing Shot result, or create a
not-yet-started rework batch.

## 100-item Performance

The DEV-053 Counting Fake executes the real productivity facade for 100 review
items. The captured values are:

```text
100_REVIEW_TASK_FIND_SINGLE   = 0
100_REVIEW_TASK_FIND_MANY     = 1
100_REVIEW_ASSET_SINGLE       = 0
100_REVIEW_ASSET_BULK         = 1
100_REVIEW_REVIEW_FIND_SINGLE = 0
100_REVIEW_REVIEW_BATCH       = 1
100_REVIEW_LINEAGE_SINGLE     = 0
100_REVIEW_LINEAGE_BULK       = 1
100_REVIEW_SNAPSHOT_SINGLE    = 0
100_REVIEW_SNAPSHOT_BATCH     = 1
```

The facade returned `total=100` and `items=100`. Missing-review ensure calls
were `0` in this fixture; the production SQLite implementation uses a
transactional bulk ensure path when needed. SQLite task, asset, lineage,
review, snapshot, and Shot-link reads are set-based or batch-scoped.

The UI loads image bytes only for the current compare item (at most 100
candidates) and uses metadata-only native video previews. It does not load 100
high-resolution images on opening the review session.

## Multi-Agent Evidence

Multi-agent execution was confirmed with four independent child tasks and no
nested agents:

- Agent A — review service, bulk ports, and SQLite repositories; added the
  productivity facade and frozen-context mapping. Validation: cargo check,
  rustfmt, diff check.
- Agent B — review command DTOs and backend tests; added the 100-item Counting
  Fake and SHA-256 wire mapping. Validation: `dev053_review_productivity`, 6/6.
- Agent C — A/B compare workspace, media, inspector, styles, and types;
  validated snapshot/legacy context, keyboard safety, and SHA display.
- Agent D — existing Shot review-board integration and client bindings;
  preserved quick review, manual selection/retry, local A/B fallback, and
  `autoStart=false` client safety.

The Main Agent performed the baseline, constructor wiring, final verification,
documentation, commit, and push. Child agents did not commit or push.

## Tests

- DEV-052 runtime integration: 6 passed, 0 failed.
- DEV-052 production preparation: 30 passed, 0 failed.
- DEV-053 backend targeted test: 6 passed, 0 failed.
- Rust full suite: 782 passed, 0 failed, 1 ignored.
- Frontend suite: 90 files, 323 tests passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `pnpm exec tsc --noEmit`: PASS.
- `pnpm build`: PASS.
- `git diff --check`: PASS.

No real ComfyUI runtime was required or invoked; the backend tests use fakes
and assert that the review read path does not call generation or ComfyUI.

## Compatibility

- No migration 025 was added; maximum migration remains 024.
- Product remains 0.6.2.
- Backup remains 14.
- Manifest remains 2.
- Existing review get/status/note/regeneration commands remain available.
- Existing Shot candidate selection, retry, failure, queue, and review rail
  behavior remains compatible.

## Deferred DEV-054

DEV-054 will integrate the complete narrative production loop across
consistency assets, context, readiness, scene preparation, queue execution,
ComfyUI, candidate review, and image-to-video production. It may address
cross-module UI consolidation, audit visualization, 500-shot validation, and a
real ComfyUI smoke gate while preserving manual candidate selection, manual
queue start, and ComfyUI as the sole generation engine.
