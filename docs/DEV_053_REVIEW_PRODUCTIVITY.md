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

## Closure Fix

### DEV053B_START_SHA

`b69791f4a310869576ca39a99de18564b4cad777` (`master`, clean and equal to
`origin/master` at task start).

### Legacy GET Compatibility

`production_item_review_get` is restored to the legacy
`ProductionItemReviewService::get()` path and the original
`ProductionBatchReview` to `ProductionBatchReviewView` conversion. The
compatibility fixture uses a real batch/item/task/review with version `3`,
lineage key `lineage-test`, parent batch/item IDs, a concrete `finishedAt`,
prompt, fixed seed, duration, width, and height. The targeted test asserts the
actual values for `lineageKey`, `parentBatchId`, `parentItemId`, `seed`,
`finishedAt`, `promptText`, `version`, `preferred`, and `outputAssets`; status
and note mutations retain the same wire values.

### Productivity Command

The legacy `production_item_review_get` command remains registered and
compatible. The independent `production_item_review_productivity_get` command
is registered separately and serves the enriched productivity DTO. The client
keeps `getProductionBatchReview` on the legacy command and routes
`getProductionBatchReviewProductivity` to the new command.

### Filter Closure

The review board exposes `ALL`, `UNREVIEWED`, `APPROVED`, `STARRED`,
`REGENERATE`, `REJECTED`, and `FAILED`. Counts are derived from the loaded
items as `unreviewed`, `approved`, `starred`, `regenerate`, `rejected`, and
`failed`. `NEEDS_REVIEW` is not an external filter label or runtime match.

### Image/Video Rework Boundary

Creating a review rework batch is available only for a video-stage item that is
reviewable and has successful production/task statuses. Image review never
calls the review regenerate command; it retains the existing Shot workspace
`onRetry` path. `REGENERATE` remains a separate review marker and does not
create a batch by itself.

### Rework Confirmation

The batch creation path requires the exact confirmation text:

> 确定创建返工批次吗？\n创建后不会自动开始，仍需前往生产队列手动启动。

Cancel performs no regeneration, queue start, or navigation. Confirm sends
`autoStart: false`, opens the existing production queue, and never invokes the
queue-start command or ComfyUI.

### DOM Interaction Tests

The mandated `@testing-library/react` plus DOM-runtime test environment is not
present in this repository (`package.json` has no `@testing-library/react`,
jsdom, happy-dom, or linkedom), and DEV-053B forbids adding a dependency for
this case. Agent C therefore reports `BLOCKED`; no custom test harness is
claimed as a substitute. Supplemental real-browser Playwright checks passed
for candidate-only selection, `1/2`, arrow navigation, Enter/Space safety,
dirty-note cancel/confirm navigation, explicit confirm-and-approve, rework
cancel/confirm, `autoStart: false`, and image-stage disablement.

### 100 Item Performance Recheck

The real productivity facade returned `total=100` and `items=100` with these
exact counters:

```text
Task single=0, bulk=1
Asset source single=0, bulk=1
Review find=0, list_batch=1, ensure_many=0
Lineage single=0, bulk=1
Snapshot single=0, batch=1
```

The read path did not call ComfyUI, preflight, queue start, or media-byte
hydration for the batch.

### Full Regression

- DEV-052 production preparation: 30 passed, 0 failed.
- DEV-052 runtime integration: 6 passed, 0 failed.
- DEV-053B backend targeted test: 7 passed, 0 failed.
- Rust full serial suite: 783 passed, 0 failed, 1 ignored.
- Frontend full suite: 90 files, 327 tests passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `pnpm build`: PASS.
- `git diff --check`: PASS.

### Multi-Agent Evidence

Exactly three parallel child tasks were used with no nested agents:

- Agent A — legacy command restoration, independent productivity command, and
  compatibility/mutation/performance backend tests: `DONE`.
- Agent B — review-board filters, counts, rework safety, confirmation, and
  board tests: `DONE`.
- Agent C — required `@testing-library/react` real DOM interaction tests:
  `BLOCKED` because the mandated dependencies are absent.

`ACTIVE_SUBAGENTS = 0`. No child agent committed or pushed.

### Versions

- Product: `0.6.2`.
- Migrations: `001–024`; no migration `025`.
- Backup: `14`.
- Manifest: `2`.
- `GITHUB_CI = NOT_CONFIGURED`; local validation was used.

### Final Decision

`DEV-053B BLOCKED — 现有仓库未提供任务规定的 @testing-library/react 与 DOM runtime，且任务要求不新增依赖，无法形成合规的真实 DOM Vitest 交互测试。`

## DEV-053C Final Closure

### DEV053C_START_SHA

`83833e98052a668d6641a887cee33d49638c03af`

### Why Test Dependencies Were Allowed

DEV-053C explicitly authorizes the three dev-only packages required to close
the A/B review interaction coverage:

- `@testing-library/react` `16.3.2`
- `@testing-library/user-event` `14.6.6`
- `jsdom` `29.1.1`

No runtime dependency was added, no global jsdom environment was configured,
and both interaction suites opt in with a per-file
`// @vitest-environment jsdom` directive. The former custom DOM runtime was
removed from the two suites.

### Real DOM Tests

`ReviewCompareWorkspace.test.tsx` passes 14/14 tests covering local-only
candidates, A/B slot movement and swap, Arrow navigation, `1`/`2` focus
shortcuts, Enter/Space safety, explicit confirm-and-approve without
auto-advance, dirty-note cancel/confirm, the 4 KiB UTF-8 boundary, native
video metadata controls, partial failure, and context inspection.

`ShotBatchReviewBoard.test.tsx` passes 12/12 tests covering all public review
filters, image/video rework boundaries, exact rework confirmation and cancel
safety, `autoStart: false`, Shot selection before approval, selection/status
failure handling, no auto-next, and the bounded current-item image read path.

### Compatibility and Regression

The backend, commands, client wire fields, and ComfyUI generation behavior
were not changed. Review mutation ordering remains Shot selection before
review status; review reads remain generation-free. DEV-052 targeted tests
pass 30/30 and 6/6. DEV-053 productivity passes 7/7, including the 100-item
bulk counters: task single=0/bulk=1, asset source single=0/bulk=1, review
find=0/list_batch=1/ensure_many=0, lineage single=0/bulk=1, and snapshot
single=0/batch=1.

### Full Regression

- Rust serial suite: 783 passed, 0 failed, 1 ignored.
- Frontend suite: 90 files, 333 tests passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `pnpm exec tsc --noEmit`: PASS.
- `pnpm build`: PASS (final validation below).
- `git diff --check`: PASS (final validation below).

### Multi-Agent Execution

Exactly two parallel child tasks were used with no nested agents:

- Agent A — `package.json` and `pnpm-lock.yaml` dependency installation:
  `DONE`.
- Agent B — `ReviewCompareWorkspace.test.tsx` real DOM interaction coverage:
  `DONE`.

Main owned `ShotBatchReviewBoard.test.tsx`, this closure record, final
validation, commit, and push. `MULTI_AGENT_EXECUTION = CONFIRMED`.

### Frozen Compatibility Values

- Product: `0.6.2`.
- Migrations: `001–024`; no migration `025`.
- Backup: `14`.
- Manifest: `2`.
- `COMFYUI_CORE = YES`.
- `REVIEW_SUBMIT = 0`.
- `SECOND_ENGINE = NO`.
- `GITHUB_CI = NOT_CONFIGURED`.
- `LOCAL_VALIDATION = PASS`.

### Final Decision

`DEV-053 REVIEW PRODUCTIVITY CLOSED`

`NEXT: DEV-054 — AI Studio 0.7.0 Narrative Production V1 Integration`
