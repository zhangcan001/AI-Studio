# AI Studio 2.0.0-personal release inventory

This is the Phase14 starting inventory, not a publication or completed release
gate. All outstanding checks below remain NOT VERIFIED until real evidence is
recorded in the candidate checklist.

## Source baseline

- Branch: `master`; clean local/remote HEAD:
  `52ba662d3e4f0a5e14fe30028be6fcc5778ac1eb`.
- Exact source CI: `37165597077`, completed success.
- package.json, src-tauri/Cargo.toml and src-tauri/tauri.conf.json:
  `2.0.0-personal`; no version bump planned.
- SQL migration chain: 001–042; 71 named business tables; no 043.
- Current logical project backup: v20. The reader explicitly accepts v1–v20;
  older archives do not imply preservation of fields their format did not store.
- Baseline frontend CI: 186 files / 1008 tests passed.
- Baseline Rust CI: 47 target summaries / 1695 passed / 3 pre-existing ignored.
  The ignored live/environment-dependent tests are not new release omissions.

## Packaging and runtime

- Windows bundling is active with `targets: all` (NSIS and MSI expected).
- Standard build: `pnpm tauri build`; frontend prebuild: `pnpm build`.
- Runtime workflow packages reside under the configured data root's `workflow_library`. Installed Native evidence subsequently confirmed five built-in image/H3 definitions are seeded when their directories are absent; models, ComfyUI and custom nodes are not bundled. Package directories alone are not proof of active/published catalog availability.
- Existing per-user installer registration points to a previous isolated
  temporary acceptance installation. Preserve it until safe installation
  isolation is established; do not uninstall an unknown user installation.
- No GPU generation is required by this release gate. Production Queue Start
  remains the only execution gate.

## Existing documentation and evidence

- Release notes: `docs/RELEASE_NOTES_v2.0.0-personal.md` (stale v19 claims).
- Known issues: `docs/AI_STUDIO_V2_KNOWN_ISSUES.md` (P2 follow-ups; the
  unconditional `NO_AUTO_REPAIR=YES` claim needs checking against recipe repairs).
- Baseline: `docs/AI_STUDIO_V2_PERSONAL_BASELINE.md` records a historical release,
  not this newly verified candidate.
- README contains historical multi-entry UI and runtime quality claims; current
  navigation and current/historical evidence need clearly separating.
- Latest Native and observability gate: Phase13, isolated real Tauri acceptance,
  diagnostic privacy export/cancel/restart and unchanged business-table data.
- Latest performance gate: Phase12 plus exact Phase13 successor guards,
  Runs SQL amplification, Library refresh/lifetime and Workflow read guards.
- Current candidate release checklist and RC baseline: not yet created.

## Outstanding release checks

Fresh database, legal old-database upgrade copy, v20 export/inspect/isolated
restore, historical compatibility, final-source installer build/fingerprints,
installed product smoke, single instance, safe exit, uninstall/data retention,
reinstall, installed diagnostics privacy, local full suites and final exact-HEAD
CI are NOT VERIFIED for Phase14 at inventory creation.

No feature/authority/schema changes, Git tag or GitHub Release are authorized.
