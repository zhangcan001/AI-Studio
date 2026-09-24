# Workflow Recognition V3 Replay Baseline V1

This directory is a repository-owned, offline replay corpus for the frozen
Workflow Recognition V3 baseline. It is evidence/test infrastructure only; it
does not change recognition, normalization, compiler, execution, or UI
compatibility production behavior.

## Running the replay modes

Run from `src-tauri` with at most two Cargo jobs:

```powershell
cargo test --lib application::workflow_recognition_v3_replay::replay_real_corpus --jobs 2 -- --nocapture
cargo test --lib application::workflow_recognition_v3_replay::replay_synthetic_verified --jobs 2 -- --nocapture
cargo test --lib application::workflow_recognition_v3_replay::replay_all_verified --jobs 2 -- --nocapture
```

These are the `real`, `synthetic-verified`, and `all-verified` modes. The
`synthetic-verified` mode covers the current authoritative 69-case baseline.
No `synthetic-baseline-complete` mode asserts the unavailable legacy 72-member
set. The runner has no network, GPU, live ComfyUI, or live `/object_info`
dependency. It loads only these locked fixtures and calls the production
analysis / semantic-graph code.

## Real corpus and golden policy

- `real/manifest.json` is the portable repository-owned derivative of
  `PRIMARY_BLACK_BOX_MANIFEST_V2`; every fixture path is relative to this
  directory. The source manifest SHA-256 is
  `5566941f90b18c7838d97c79ed417fdb86293921faf9578e695c0b78a7f4232b`.
- `real/object_info.json` is the single shared schema snapshot (SHA-256
  `0b2ca3bd2544925654422ceb269d32752f195366bdcc642a29bc12af31a681d1`). Each
  sample retains its exact UI/API pair and provenance JSON; copied bytes are
  hash-checked against the source manifest values.
- R10's full UI SHA-256 was read directly from the source manifest, not from a
  truncated report: `9646b9b9b72f6ccb5db717a2ceee7b32aae7f7a0ce2149c8354ebd4fc173fe0d`.
- Hard golden fields are identity, UI/API/object_info/provenance hashes,
  category, mode, root count, evidence status, and semantic counting status.
  Each hard field must be `Attested` or `IndependentlyDerived` with a recorded
  provenance source. Root counts come from the source manifest's explicit
  ground-truth descriptions (R09=3, R12=2, R13=5, R14=5, R15=31; all other
  samples=1).
- R11 is the sole expected evidence conflict: the manifest mode is
  `image_to_image`, paired API/profile evidence says `text_to_image`, and it is
  excluded from semantic pass/failure. Its actual recognizer category/mode is
  printed for diagnosis, never used to rewrite the expected values.
- Actual root IDs, capability profiles, and readiness are `OBSERVED_ONLY` and
  do not affect semantic pass/fail. Current recognizer output is never used to
  generate or update expected golden values.
- No fixture or runner path depends on a user's Temp directory.

## Synthetic registry and membership

`synthetic_registry.json` contains 69 stable, name-based identities: 57
explicit Phase2A/2B/2C/readiness contracts plus 12 explicitly named Phase1
contracts. Phase1's reported total is 15, but only 12 Phase1 identities are
explicitly verified. The legacy overall reported count is 72 without a
reconstructible member list; the current authoritative corpus is 69, with
69/69 passing. The legacy count is retained for provenance only, not as an
acceptance criterion. Existing tests are called through small `#[cfg(test)]`
dispatchers; the replay does not copy the recognition algorithm. Test-binary
discovery must match the registered wrapper set exactly, deterministically.

Three later terminal-media tests remain separate V2-delta candidates, not V1
identities:

| Test | Introduced | At review baseline | Named by Phase1 review | V1 membership |
| --- | --- | --- | --- | --- |
| `generic_typed_terminal_media_sink_is_output_root` | `dacafca3ef1f7ba0900d4507555641c0bf10bfd7` | No | No | Unproven |
| `terminal_unknown_node_without_media_evidence_is_not_root` | `dacafca3ef1f7ba0900d4507555641c0bf10bfd7` | No | No | Unproven |
| `generic_serialized_terminal_media_sink_is_output_root` | `dacafca3ef1f7ba0900d4507555641c0bf10bfd7` | No | No | Unproven |

Their common presence/grouping in one commit does not prove Phase1 membership.
Do not report a 72/72 member set or 75 as a V1 baseline.

## Self-tests and evidence separation

Eight replay self-tests guard missing fixture, byte hash corruption, duplicate
real identity, wrong fixed real count, duplicate synthetic identity,
unregistered synthetic wrapper, deterministic discovery, and rejection of an
unattested hard golden expectation. They validate the verified registry; they
do not assume an unproven 72-case registry.

UI/API exact execution-graph equivalence is a separate V1 compatibility gate,
not part of V3 semantic replay. A fresh sample-neutral graph-isomorphism
comparison on the same 15 repository fixtures and current normalizer passed
15/15 with zero differences. It ignores only `_meta`, structurally maps node
IDs, and compares classes, all runtime literals, non-`_meta` extras, directed
edges, input names, and source output slots. There are no provider/workflow
specific rules; the R05 seed remained exact.

## Current fresh replay observation

The 15 real baseline cases replay with 14 semantic passes, zero semantic
failures, and one expected evidence conflict (R11). R02, R04, R09, R12, and
R13 now match their original hard goldens without changing fixture bytes or
expected modes. R11 remains excluded from semantic pass/failure counting.

All 69 currently verifiable synthetic cases pass. The legacy reported count
is 72, but its member set is unavailable; the difference of three is a
provenance exception, not three identified missing cases. Replay infrastructure
is closed with this legacy provenance exception. The Recognition V3 architecture
has a replayed baseline; production/runtime validation is not claimed by the replay.
