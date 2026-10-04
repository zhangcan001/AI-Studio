# AI Studio 2.0.0-personal RC Baseline

## Source and CI authority

- Parent Phase13: `52ba662d3e4f0a5e14fe30028be6fcc5778ac1eb`; parent CI37165597077 success.
- Installer source: `03ee97e17f5061f87b45573b9220cc7f1a81a939`.
- Final documentation closeout is this document’s containing commit. No runtime, transport, migration, bundle/config or historical-manifest bytes change after installer source; audit with `git diff <installer-source> <final-doc-head> -- src src-tauri scripts .github`.
- Exact-final-HEAD Source-only CI is pending at documentation commit creation. The final chat report records the actual final SHA, run and completed conclusion; parent/local success is not substituted for it.

## Windows artifacts

Built by standard `pnpm tauri build` with one Cargo build job; optimized compilation9m46s, both bundles completed. Product version2.0.0-personal; MSI numeric metadata2.0.0.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| AI Studio_2.0.0-personal_x64_en-US.msi | 20422656 | `7DF6068933913D76DDA1C2C067258866F1802DC40AF8C7BC538E85DDF64BF8DD` |
| AI Studio_2.0.0-personal_x64-setup.exe | 13005085 | `4F9835CA70E92739DE2EF6391C03B511C78D34894DCBAC16CA0F889A5C905A4D` |

NSIS was installed, run, uninstalled and reinstalled in isolated TEMP directories. MSI installation NOT VERIFIED; MSI build and numeric-version regression passed. No executable/MSI/media/DB/archive/diagnostic bundle is committed. Unsigned local candidate, not a signed published release.

## Frozen data and local gates

- Migration maximum42;71 named business tables; no043; backup20, reader accepts1–20.
- Frontend188 files/1016 PASS; Rust47 targets/1696 PASS/3 existing ignored.
- TypeScript/build/fmt/check/architecture/performance/observability PASS.
- Queue Start only execution authority, unchanged Task state/binding OCC/workflow engine; no remote telemetry.
- Fresh installed data root and reconstructed39→42 upgrade copied fixture passed. Exact identities/media/business facts preserved; see checklist for counts and bounded coverage.
- Final installed v20 export/inspect/new-project restore passed; empty-issue-array UI crash fixed and reaccepted. Historical compatibility is service-test coverage, not invented historical-binary Native proof.
- Safe close warning/cancel/confirm, single instance, uninstall/reinstall and existing data reopening passed. Full logical table digests/media/archive hashes unchanged across verified lifecycle.
- Installed product smoke and positive-control diagnostics ZIP privacy passed.
- GPU smoke SKIPPED_BY_POLICY; explicitly inspected execution core/runtime-package paths unchanged from c916ea2 quality baseline.

## Reviewed release changes

1. Isolated historical-schema fixture and factual current UI/backup documentation.
2. Numeric WiX override fixing real MSI bundler failure; product semantic version retained.
3. Always serialize seven existing restore issue arrays, with actual Rust serde red/green regression; no archive schema/relationship changes.
4. Exact Phase14 successor proof and negative probes retain immutable historical Phase7–13 reviews while verifying the actual live release fix.
5. Final evidence-only docs, no runtime changes after installer build.

## Remaining limits / publication

Known Issues records existing P2 follow-ups, dormant generic audio labels, external model/node capability setup, unsigned installer and unverified MSI installation. No unresolved P0/P1 was observed in tested gates. Ready/publication decision still requires exact final CI. No tag/GitHub Release/next feature phase is authorized.

## Local evidence (not checked in)

Owned TEMP evidence: phase14-artifacts-final.json; phase14-migration-final.json; phase14-upgrade-before.json/result.json; phase14-backup-data-final.json/extra-facts.json; phase14-diagnostics-privacy-evidence.json; phase14-safe-exit-fixture.json; phase14-before-uninstall-verified.json/after-uninstall-verified.json/after-reinstall-launch.json; final local test/build logs. These are local acceptance artifacts, not runtime dependencies.
