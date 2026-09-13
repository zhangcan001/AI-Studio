# DEV-125 — Prompt Studio Integration & Hardening

```text
TASK=DEV-125
BASELINE=f02f519e9dd58f9439bf57fe08e2b41333570dce
SCOPE=INTEGRATION_AND_HARDENING
NEW_PROMPT_SYSTEM=NO
NEW_EXECUTOR=NO
NEW_QUEUE=NO
AI_OPTIMIZATION=NO
AUTO_GENERATION=NO
```

## 1. Outcome

Prompt Studio MVP is hardened for long-term personal use without introducing a
second Prompt system or changing the Production Core execution boundary. The
work is limited to integration tests, data-consistency checks, performance
fixtures, and frontend failure-state handling.

The existing Production Queue remains the only execution gate. Queue Start,
Task, Review, and the production flow were not changed.

## 2. Project isolation

### Prompt list, detail, and versions

The Prompt Library remains project-scoped at both the service and repository
boundaries. Every list, detail, and version read carries the caller's
`project_id`; a Prompt ID from another project is not returned. Existing
service coverage verifies list/detail isolation and append-only version history.

The performance fixture also seeds 1,000 Prompts in project A and a sentinel
Prompt in project B. Project A receives only its own page and detail; project B
cannot read project A's Prompt and sees only its sentinel.

### References

`ReferenceAnchor` and `ReferenceSet` remain the reference authorities. Anchor
reads are project-scoped, and the service validates that referenced Assets
belong to the same project. The integration test creates a project-B anchor and
confirms it is absent from project A's list/detail reads while remaining
available in project B.

### Frontend boundary

Prompt Studio passes the active `projectId` to Prompt list/detail and generation
history calls. Frontend tests assert those calls so a future UI change cannot
silently widen the read scope.

## 3. Data consistency

### Prompt versions

Prompt version creation is append-only. The hardening test confirms that after
creating versions 1 and 2, both historical texts and version numbers remain
available; the newer version does not overwrite version 1. There is no new
Prompt-version deletion path in this task.

### Model versions

`ModelVersion` is the canonical immutable provenance record for a model
revision. The service test snapshots a ModelVersion, updates the parent Model's
metadata, and verifies the ModelVersion is unchanged. Model and ModelVersion
operations continue to use their existing repository/service boundaries.

### References and deletion behavior

Reference anchors preserve ordered Asset membership. Deleting an anchor removes
its membership rows but does not delete the referenced Assets or historical
production records. Prompt Studio does not create a parallel Prompt-reference
table; it continues to consume the existing reference authorities.

## 4. Provenance review

The current MVP provenance path is:

```text
PromptVersion (optional model_version_id)
        ↓
ModelVersion
        ↓
existing Task / generation_snapshots
        ↓
task_output_assets
        ↓
Asset / Result
```

### Decision: `PROMPT_GENERATION_LINK=DEFER`

Do not add a standalone `PromptGenerationLink` now.

Reasons:

1. Prompt Library, Model Registry, Task, GenerationSnapshot, output mapping, and
   Asset are already authoritative in their bounded contexts.
2. Prompt Studio MVP is read-only and the existing Task/Result history is the
   safe integration path; a new link table would create another provenance
   authority before an exact prompt-level filtering requirement exists.
3. The current chain is sufficient to display ModelVersion provenance and task
   history, but it does not claim that every historical Task can be filtered by
   an exact PromptVersion.

If exact PromptVersion-to-generation history becomes a demonstrated requirement,
extend the existing generation snapshot/Task authority additively. Do not add a
parallel executor or reinterpret workflow identity; the exact workflow pair
remains `workflowVersionId + recipeId`.

## 5. Performance hardening

The repository tests use a fresh migrated SQLite database and deterministic
fixtures:

| Fixture | Coverage | Result |
| --- | --- | --- |
| 1,000 Prompts / 5,000 Prompt Versions | Project-scoped list, bounded keyset page, keyword+tag filter, detail, version query | PASS |
| 10,000 reference records (`reference_anchors`) | Project-scoped list, detail lookup, and cross-project isolation | PASS |

The Prompt list is asserted to remain bounded at 50 rows and to return a
continuation cursor. Filtered results are checked for the expected Prompt and
version count. Detail queries return all five versions for the selected Prompt.
The reference fixture checks all 10,000 records without introducing a search
engine rewrite or machine-dependent timing threshold, avoiding flaky tests on
different personal machines.

## 6. Frontend robustness

Prompt Studio now clears stale detail errors when a new list request starts and
surfaces a rejected detail request instead of silently swallowing it through
`Promise.allSettled`.

The frontend test suite covers:

- list loading state;
- list empty state;
- list error state;
- detail error state;
- model registry error state;
- generation-history error state;
- project-scoped Prompt/detail/history calls;
- Prompt list, detail, model, version history, and provenance rendering.

Raw backend errors continue to be converted to the existing user-facing error
contract; technical details remain available through the established app error
handling path.

## 7. Validation

```text
FRONTEND_TEST=PASS (152 files, 835 tests)
TSC=PASS
BUILD=PASS
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_TEST=PASS (full cargo test; unit and integration suites passed)
REMOTE_CI_RUN=DISPATCHED_AFTER_PUSH
REMOTE_CI_STATUS=PENDING
```

Commands executed locally:

```text
pnpm test
pnpm exec tsc --noEmit
pnpm build
cargo fmt --check
cargo check
cargo test
```

Source-only CI is dispatched manually after the final commit is pushed because
`.github/workflows/ci.yml` runs on tags, pull requests, or
`workflow_dispatch`, not on an ordinary push to `master`. The run ID, URL, and
final result will be recorded here after the remote run completes.

## 8. Files changed

### Hardening implementation/tests

- `src/features/prompts/PromptStudio.tsx` — expose detail-load failures and
  clear stale detail errors.
- `src/features/prompts/PromptStudio.test.tsx` — loading, empty, error, and
  project-scope frontend coverage.
- `src-tauri/src/application/prompt_library_service.rs` — historical Prompt
  version assertions.
- `src-tauri/src/application/model_service.rs` — non-destructive ModelVersion
  assertion.
- `src-tauri/src/application/reference_anchor_service.rs` — project-isolated
  reference anchor assertion.
- `src-tauri/src/infrastructure/database/repositories/prompt_library.rs` —
  1,000 Prompt / 5,000 version fixture.
- `src-tauri/src/infrastructure/database/repositories/reference_anchor.rs` —
  10,000 reference fixture.

No Queue, Task, Review, Production Core, database migration, or new Prompt
domain was introduced.

## 9. Known limitations

- PromptVersion currently stores optional ModelVersion provenance but does not
  expose a direct PromptVersion-to-GenerationSnapshot link for exact historical
  Prompt filtering.
- Prompt Studio does not invent Prompt-specific reference IDs. Existing
  ReferenceAnchor, ReferenceSet, shot-reference, and generation-input
  authorities remain responsible for reference identity and ordering.
- No AI tagging, vector search, cloud synchronization, automatic generation,
  or Agent behavior is part of DEV-125.

## 10. Decision summary

```text
PROJECT_ISOLATION=PASS
DATA_CONSISTENCY=PASS
PROVENANCE_REVIEW=DEFER
PERFORMANCE_TEST=PASS
FRONTEND_ROBUSTNESS=PASS
P0=NONE
P1=NONE
P2=NONE
```
