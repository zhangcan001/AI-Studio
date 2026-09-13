# AI Studio v1.3.0 Release Checklist

## Code

- [x] `master` starts from the DEV-114 final head and is synchronized with
      `origin/master` before release work.
- [x] Product version is aligned to `1.3.0` in the package, Tauri, and Rust
      package metadata.
- [x] Cargo lock metadata and the existing version consistency gate track
      `1.3.0`.
- [x] No separate checked-in installer metadata exists; Tauri bundle metadata
      is derived from `src-tauri/tauri.conf.json`.
- [x] No production business code, schema, migration, queue, task, review,
      asset, or workflow engine change was made for release preparation.

## Product

- [x] DEV-109 PASS
- [x] DEV-110 PASS
- [x] DEV-111 PASS
- [x] DEV-112 PASS
- [x] DEV-113 PASS
- [x] DEV-114 PASS

## Tests

- [x] Frontend regression suite — 150 files, 823 tests passed.
- [x] TypeScript check — passed.
- [x] Production build — passed; existing main-chunk warning only.
- [x] Rust checks — format, check, and all-target tests passed; release
      metadata and the existing version consistency gate changed only.
- [x] Source-only CI — run `34743034346` passed on the exact pushed release
      head `cbf2f1853c2553bd02e646adf386da53b6aff87e`.

## Installer

- [x] Tauri release build verified with version `1.3.0`, product name `AI
      Studio`, existing identifier, icons, and generated MSI/NSIS artifacts.
- [x] Installer flow/configuration unchanged.

## Release

- [ ] Tag `v1.3.0` created and pushed.
- [ ] GitHub Release `AI Studio v1.3.0` created with the release notes and
      known issues.

## Evidence

```text
BASELINE_SHA=35d75b7d0a319aa3220741935c56be20a0247975
FINAL_SHA=cbf2f1853c2553bd02e646adf386da53b6aff87e (release code head)
VERSION=1.3.0
FRONTEND_TEST=PASS (150 files, 823 tests)
TSC=PASS
BUILD=PASS (existing >500 kB main chunk warning only)
RUST_CHANGED=YES (package metadata and existing version gate only)
REMOTE_CI_RUN=34743034346
REMOTE_CI_STATUS=PASS
TAG=TO_BE_RECORDED
GITHUB_RELEASE=TO_BE_RECORDED
P0=NONE
P1=NONE
KNOWN_ISSUES=Existing non-blocking P2/P3 items only
```
