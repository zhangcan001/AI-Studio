# AI Studio v1.3.1 Release Checklist

```text
TASK=DEV-117
VERSION=1.3.1
BASELINE_SHA=81b204566605c73f840b231b7b981f2c377813cd
```

## Version

- [x] `package.json` is aligned to `1.3.1`.
- [x] `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and
      `src-tauri/tauri.conf.json` are aligned to `1.3.1`.
- [x] README, changelog, release notes, and DEV-116 health metadata identify
      the stable `1.3.1` release.
- [x] The existing version consistency gate remains aligned.

## Tests

- [x] Frontend regression suite passed: 150 files, 824 tests.
- [x] TypeScript no-emit check passed.
- [x] Production build passed with no Vite >500 kB warning.
- [x] Rust format, check, and all-target tests passed.

## CI

- [x] Source-only CI run `34748866820` passed against implementation head
      `13e56ebb8daec48091f024a16e0306b2011818c3`.
- [x] The prior timeout was corrected by bounding the production-orchestrator
      test polling helper; no CI gate was disabled or hidden.

## Artifacts

- [x] No new installer artifact is required for this documentation-only
      closeout; the existing Tauri bundle configuration and artifact process
      remain unchanged.
- [x] No schema, migration, workflow, queue, task, or production execution
      artifact changed in DEV-117.

## Documentation

- [x] User-facing notes: `docs/RELEASE_NOTES_v1.3.1.md`.
- [x] Stability evidence: `docs/DEV_116_STABILITY_REPORT.md`.
- [x] Release health: `docs/DEV_116_RELEASE_HEALTH.md`.
- [x] Stable baseline: `docs/AI_STUDIO_v1.3.1_STABLE_BASELINE.md`.
- [x] v2 direction: `docs/AI_STUDIO_v2_DIRECTION.md`.

## Known Issues

- [x] `CI-113-01` remains a manual documentation-only Source-only CI dispatch
      tradeoff.
- [x] `FE-113-03` remains technical diagnostic vocabulary by design.
- [x] `PERF-113-02` remains a bounded large-collection wayfinding cost.
- [x] No P0 or P1 issue remains.

## Tag

- [x] Tag `v1.3.1` is created and pushed at the stable release head.
- [x] The tag is not overwritten if it already exists; it was absent before
      this closeout.

## Release

- [x] GitHub Release `AI Studio v1.3.1` is published from tag `v1.3.1`.
- [x] The GitHub Release includes the user-facing release notes, known issues,
      and verification status.

## Evidence

```text
VERSION_ALIGNMENT=PASS
RELEASE_NOTES=PASS
CHECKLIST=PASS
STABLE_BASELINE=PASS
V2_DIRECTION_DOC=PASS
REMOTE_CI_RUN=34748866820
REMOTE_CI_STATUS=PASS
TAG=v1.3.1
GITHUB_RELEASE=https://github.com/zhangcan001/AI-Studio/releases/tag/v1.3.1
P0=NONE
P1=NONE
KNOWN_ISSUES=CI-113-01; FE-113-03; PERF-113-02
```
