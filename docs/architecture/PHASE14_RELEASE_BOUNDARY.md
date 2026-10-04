# Phase14 scoped release boundary

Runtime baseline: `52ba662d3e4f0a5e14fe30028be6fcc5778ac1eb`.

This release gate permits release evidence/docs and isolated acceptance tools.
It does not authorize changes to the frozen Phase7–13 implementations or
historical manifests. No successor hashes are repinned for release docs.

Reviewed initial paths:

- README.md
- docs/RELEASE_NOTES_v2.0.0-personal.md
- docs/AI_STUDIO_V2_KNOWN_ISSUES.md
- docs/RELEASE_INVENTORY_v2.0.0-personal.md
- docs/architecture/PHASE14_RELEASE_BOUNDARY.md
- scripts/testing/release-isolation-fixture.py

Installer-only blocker repair:

- src-tauri/tauri.conf.json: `bundle.windows.wix.version=2.0.0` maps the
  unchanged product version `2.0.0-personal` onto MSI's numeric version
  contract. Bundle targets and application identifier remain unchanged.
- scripts/testing/release-config.test.ts: regression verifies product version
  alignment, the supported numeric WiX override and retained bundle identity.

Original `pnpm tauri build` compiled the executable but failed MSI bundling on
the nonnumeric prerelease label. The focused regression reproduced the missing
override before the fix. Both installers must be rebuilt after this fix.

Installed Native backup restore exposed a second blocker: the backend restored
the new project successfully, then the UI crashed because empty restore-report
issue arrays were omitted by Rust serialization but required by typed UI.
The minimal repair always serializes the seven existing issue lists (empty or
nonempty). It changes no archive/persistence/relationship or execution semantics.

The exact additional scope is project_backup_service.rs (DTO serialization and
its red/green regression), phase14-release-guard.mjs, its negative probes, and
the Phase13 guard integration. `phase14-release.json` pins the immutable Phase13
parent, exact reviewed paths, before/after hashes, whole Rust file set/count and
both baseline/live untouched aggregates. Historical Phase7–13 manifests remain
byte-identical. Only after real live bytes pass this release proof may the
historical guard read the immutable parent for the two reviewed existing paths.
Invalid proof falls back to real bytes and cannot conceal drift. Frontend,
privacy, migrations and all other historical checks continue reading live files.

Final evidence may add the release checklist and RC baseline document. Any
runtime/installer configuration fix requires separately recorded root cause,
regression coverage, necessary Native reacceptance and final-source rebuild.

The fixture builder reads only an explicitly selected owned TEMP fixture and
creates a new TEMP directory. It reconstructs the actual historical schema
from SQL, copies compatible persisted facts and reinstates write-side triggers
after import. It never downgrades migration markers in the source database.
Reconstructed fixtures must not be called official historical-binary evidence.

Installers, diagnostics, archives, databases, user media and generated caches
remain outside commits. No tag or GitHub Release is created. Production Queue
Start, Task validation/state, project isolation, binding OCC, workflow exact
identity, backup v20 and migration maximum42 remain unchanged.

Release validation infrastructure follow-up: two hosted runs completed the
static architecture scan with PASS output but exceeded the unchanged15-second
ordinary-frontend import deadline. Identical single-worker local full suite
completed that scan in5.3seconds. The scoped fix only adds `--maxWorkers=1` to
Frontend tests; all1017 cases, deadlines, assertions, Rust commands/threads,
job timeouts/cache and independent architecture step remain unchanged.

The release proof includes exact before/after workflow hashes and independently
requires its complete bytes to equal the immutable parent's one-line worker
replacement. Historical workflow review reads that parent only after validation.
A workflow-drift negative probe was added; no historical manifest was changed.
This is validation contention control, not a product-performance threshold.
