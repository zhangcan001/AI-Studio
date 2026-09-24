# Workflow Recognition V3 synthetic identity recovery ledger

## Scope and result

The **current verifiable synthetic corpus is 69 named cases**: 57
Phase2A/2B/2C/readiness contracts and 12 explicitly named Phase1 contracts.
All 69 pass the current replay. An older report claims 72 synthetic cases,
with a reported Phase1 subtotal of 15, but the historical 72-member set is
unavailable. The arithmetic difference of three is an **unresolved legacy
count delta**, not three identified tests or a verified 69/72 set comparison.
This is a provenance exception, not a recognition-semantic failure.

## Current baseline and legacy exception

| Field | Status |
| --- | --- |
| `CURRENT_AUTHORITATIVE_SYNTHETIC_COUNT` | `69` named cases |
| `CURRENT_AUTHORITATIVE_SYNTHETIC_REPLAY` | `69/69_PASS` |
| `V3_REAL_CORPUS_BASELINE` | `15/15_MATCH` (14 semantic passes, one expected evidence conflict, zero semantic failures) |
| `LEGACY_REPORTED_SYNTHETIC_COUNT` | `72` (derived summary count only) |
| `LEGACY_72_MEMBER_SET` | `UNAVAILABLE` |
| `LEGACY_COUNT_NOT_USED_AS_ACCEPTANCE_CRITERION` | `YES` |
| `V3_REPLAY_INFRASTRUCTURE_STATUS` | `CLOSED_WITH_LEGACY_PROVENANCE_EXCEPTION` |
| `WORKFLOW_RECOGNITION_V3_STATUS` | `ARCHITECTURE_COMPLETE_REPLAYED_BASELINE` |
| `SEMANTIC_RECOGNITION_ARCHITECTURE_COMPLETION` | `100%` for the replayed architecture baseline, not production/runtime validation |
| `WORKFLOW_RECOGNITION_V3_CODE_FREEZE` | `YES` |

The 69-case registry is the current acceptance baseline. The replay runner's
historical 72 and Phase1 15 fields preserve the frozen legacy evidence; they
do not require a 72/72 replay or prove three specific missing identities.

## Provenance and evidence rating of the legacy count

- The earliest **dated repository occurrence** of the synthetic 72 count is
  commit `6ed79c4ca4b49b4d84e072abfb3b8845af77b096` at
  2026-09-24 19:02:12 +0800. It added
  `src-tauri/tests/fixtures/workflow_recognition_v3/synthetic_registry.json`
  (a summary count and 69-name registry), `real/manifest.json` (the same
  count and `synthetic_membership_verified: false`), and `README.md`.
  These are later derivatives of a reported baseline, not a contemporaneous
  72-member identity register. The cited Final Architecture Review hash is
  present, but the original report/member inventory is not repository-owned.
- Source type: **SUMMARY_COUNT_ONLY**. Evidence class: **C — DERIVED_COUNT**.
  The registry records 57 named Phase2/readiness members plus a reported
  Phase1 subtotal of 15, yielding 72. Only 12 of those Phase1 identities
  are named. The original 15-count provenance is not independently
  verifiable as a distinct member set in the repository.
- Search covered all reachable commits, local/remote branches, tags, commit
  messages, reflog, stash, deleted/renamed paths, docs, README files, test
  and generated reports, fixture manifests, scripts, and read-only Git
  objects. Remote `origin` exposes only `master` plus tags, and no remote
  authoritative record. There is no complete or partial authoritative list
  for the three unresolved positions. Consequently, no deterministic
  72-versus-69 member-set comparison is possible.
- Acceptance baseline: **69 verifiable cases, 69/69 replay passing**. The
  legacy reported count remains 72 for traceability, with
  `LEGACY_MEMBER_SET=UNAVAILABLE` and `UNRESOLVED_LEGACY_COUNT_DELTA=3`.
  Do not use 72 as a current acceptance criterion or call 69/72 verified.

## Evidence examined

- `src-tauri/tests/fixtures/workflow_recognition_v3/synthetic_registry.json`
  records the 69 names, the reported Phase1 count of 15, and source-prompt hashes;
  it does not supply names or hashes for the remaining three Phase1 members.
  The registry first appears in commit `6ed79c4`.
- `git rev-list --all`, `git log --all`, `git log --follow`, `git show`,
  `git ls-tree`, and history searches found no earlier repository-owned V3
  synthetic registry, 72-member manifest, fixture index, or deleted/renamed
  V3 synthetic fixture path. The pre-V3 and V3 completion trees likewise
  contain no such inventory. Sixteen unreachable commits found by `git fsck`
  all predate V3. A keyword scan of all 1,536 unreachable blobs for V3
  Phase1 and registry-count markers found only two V3 replay drafts with the
  same 69-member state, not an authoritative 72-member record.
- `dacafca3` introduced three terminal-media tests in
  `workflow_analysis_service.rs`: `generic_typed_terminal_media_sink_is_output_root`,
  `terminal_unknown_node_without_media_evidence_is_not_root`, and
  `generic_serialized_terminal_media_sink_is_output_root`. Git blame proves
  their introduction, but neither that commit nor a historical registry or
  Phase1 review names them as members of the 15. Two use the Phase2A test
  helper. Their count and semantic similarity cannot establish membership.

## Unresolved identity ledger

The row keys below are **count-delta slots, not synthetic IDs**. No row is mapped
to one of the terminal-media candidates by position or content similarity.

| Ledger slot | historical_identity / expected_identity | current_identity | historical_path / current_path | historical_sha256 / current_sha256 | known_case_name / workflow / fixture_reference | first_seen_commit / transition_commit / last_known_commit | transition_type / root cause | confidence | recovered |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Unresolved slot 1 | UNKNOWN / UNKNOWN | UNKNOWN | UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN / UNKNOWN | K — UNRECOVERABLE from current repository evidence; historical member identity absent | INSUFFICIENT | NO |
| Unresolved slot 2 | UNKNOWN / UNKNOWN | UNKNOWN | UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN / UNKNOWN | K — UNRECOVERABLE from current repository evidence; historical member identity absent | INSUFFICIENT | NO |
| Unresolved slot 3 | UNKNOWN / UNKNOWN | UNKNOWN | UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN / UNKNOWN | UNKNOWN / UNKNOWN / UNKNOWN | K — UNRECOVERABLE from current repository evidence; historical member identity absent | INSUFFICIENT | NO |

For each slot, `synthetic_id`, expected source origin, known filename/hash,
historical workflow, and recovery mapping are **UNKNOWN**. The arithmetic
72 − 69 = 3 establishes only the reported count difference; it cannot prove
why any particular test belongs to a historical 72-member set. `UNRECOVERABLE` here
means *not recoverable from the available repository evidence*, not proof
that a historical case never existed. Only a future explicit historical
identity record can promote a member to `PROVEN` and increase the verified
count. Do not modify production semantics or replay bytes to fill this gap.
