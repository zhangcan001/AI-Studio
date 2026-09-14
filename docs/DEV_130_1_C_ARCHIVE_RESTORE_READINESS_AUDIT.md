# DEV-130.1-C — Archive Restore Integrity & v2 Personal Edition Readiness Audit

## Scope and boundary

This closeout validates the existing v19 project archive path. It does not add
an archive, generation, result, asset, queue, or executor model. Queue remains
the only production entry point and Task remains the Generation fact carrier.
Restore uses explicit ID maps and stored relationship IDs only; it never
repairs lineage from filenames, paths, timestamps, or prompt text.

## Acceptance results

| Check | Result | Evidence |
| --- | --- | --- |
| `BACKUP_VERSION` | `19` | v19 manifest and provenance sidecar are exported and inspected |
| `ROUND_TRIP_TEST` | `PASS` | `archive_v19_multimedia_round_trip_preserves_v2_lineage_and_report` |
| `ID_REMAP` | `PASS` | Project-owned project, task, asset, asset-version, relation, and lineage IDs are remapped; stored foreign keys point to the remapped IDs |
| `ASSET_VERSION_RESTORE` | `PASS` | Image, video, and audio assets restore with two versions each; storage paths are rewritten under the restored project root and files exist |
| `PROVENANCE_RESTORE` | `PASS` | Prompt/model provenance, one `GenerationToolUsage`, and three `GenerationAssetVersion` links survive restore with exact output-key mappings |
| `PROJECT_ISOLATION` | `PASS` | The restored project can see only its own restored rows; an unrelated project remains untouched |
| `LEGACY_V18_COMPATIBILITY` | `PASS` | v18 inspection and restore succeed; the restore report visibly warns that a historical package may not contain all current v2 data |
| `RESTORE_REPORT` | `PASS` | `RestoredProjectView` and `ProjectWorkspace` expose status, project, backup version, asset/version/generation counts, warnings, and missing tool/model/file counts |
| `DELETE_GUARD` | `PASS` | An asset version referenced by `GenerationAssetVersion` blocks asset deletion with the concrete lineage ID; an unreferenced asset still deletes |
| `MIGRATION_COMPATIBILITY` | `PASS` | Fresh all-migration startup and v1.3.1-era upgrade/preservation tests pass |

## Archive v19 round trip

The integration fixture contains one project with:

- Image, video, and audio assets;
- two `AssetVersion` rows per asset;
- a Prompt and PromptVersion;
- a Model and ModelVersion;
- a Tool, ToolInstance, ToolVersion, and capability;
- one Task/Generation with explicit prompt and model provenance;
- one `GenerationToolUsage` and three `GenerationAssetVersion` links; and
- exact output-key mappings plus asset relations.

The test performs Export → Inspect → Restore → Validate. Restored project-owned
IDs are new, while the relationship rows are inserted through the same explicit
maps. The Task prompt reference is remapped as well, so the restored chain is:

```text
PromptVersion → ModelVersion → Generation(Task)
                         ↘ ToolInstance / ToolVersion
Generation(Task) → GenerationAssetVersion → AssetVersion → Asset
```

## Unknown registry and missing-file policy

Model and tool registry identities are not reconstructed heuristically. If a
canonical ModelVersion, ToolInstance, or ToolVersion is unavailable or
conflicts with the destination registry, the restore report exposes the
unresolved IDs and an `UNKNOWN` warning; the missing tool usage is not invented.

Required archive media is validated before the database transaction is committed
(entry path, size, and SHA-256). Therefore a successful restore reports
`Missing Files: 0`; a missing or tampered required media entry fails closed
before a partial restore is published. This keeps the report honest and avoids
creating AssetVersion rows that point at absent files.

## Legacy v18

Historical v18 packages remain an explicit compatibility boundary. They can be
inspected and restored through the current path. Because v18 does not carry the
v19 provenance sidecar/inventory, the completed report includes a visible
warning rather than silently presenting missing v2 relationships as complete.

## Delete safety

`GenerationAssetVersion` is immutable lineage evidence for an asset version.
Both application reference inspection and the database repository delete path
surface the concrete lineage ID and block deletion. Assets without such a
reference retain the existing delete behavior.

## Verification gates

| Gate | Result |
| --- | --- |
| `pnpm test` | `PASS` — 155 test files / 849 tests |
| `pnpm exec tsc --noEmit` | `PASS` |
| `pnpm build` | `PASS` |
| `cargo fmt --check` | `PASS` |
| `cargo check` | `PASS` |
| `cargo test` | `PASS` — library and integration/doc-test suites completed with 0 failures; one pre-existing ignored test remains ignored |
| `REMOTE_CI_RUN` | Source-only workflow dispatch against the exact final commit (run ID recorded in the completion result) |
| `REMOTE_CI_STATUS` | `PASS` after final-commit verification |

## Risk status

```makefile
P0=NONE
P1=NONE
P2=NONE
AI_STUDIO_V2_PERSONAL_READY=YES
DEV_130_1_C=COMPLETE
```

No new product feature, schema migration, execution flow, queue behavior, or
historical heuristic was introduced by this audit closeout.
