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
