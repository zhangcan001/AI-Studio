# AI Studio 0.5.0 Publication Evidence

## Publication identity

- `SOURCE_RC_SHA`: `02e67cff50f5da1d207478071636af166048820c`
- `RC_EVIDENCE_COMMIT`: `8a985432771e7d7d3fbf9b6ebdcfed96834796ab`
- `TAG_OBJECT_SHA`: `87a43fb79c7668fe838f58fa4e5491342569daa9`
- `TAG_PEELED_SHA`: `02e67cff50f5da1d207478071636af166048820c`
- Tag target is the Source RC, not the RC evidence commit or this publication evidence commit.

## GitHub Release

- Release title: `AI Studio 0.5.0`
- Tag: `v0.5.0`
- URL: <https://github.com/zhangcan001/AI-Studio/releases/tag/v0.5.0>
- Draft: `false`
- Prerelease: `false`
- Published: `2026-08-17T07:12:44Z`

## Artifacts

| Release-upload filename | Bytes | SHA256 | Remote status |
| --- | ---: | --- | --- |
| `ai-studio.exe` | 36982784 | `587201EA1C95D3BB1E9268747F36D595D121069B3C7908F747B43BBAFC7C5FC7` | uploaded; remote digest matches |
| `AI Studio_0.5.0_x64-setup.exe` | 8400244 | `A598D358734FBFDFDA4814245E62F42F35C768ED7FEE47527213B89F4E895992` | uploaded; remote digest matches |
| `AI Studio_0.5.0_x64_en-US.msi` | 12349440 | `037FC1D973BA11E083CAE66A706D265CA64FD3E96479316D46A1106AD584A62B` | uploaded; remote digest matches |
| `RELEASE_SHA256_0.5.0.txt` | 472 | `75d5ebb08a32f57ee436bcbe7dea15d57d37d6d6fd513c24309a56abf0566924` | uploaded; remote digest matches the pre-publication checksum file |

GitHub canonicalizes spaces in the two Tauri bundle asset names to dots in the remote `name` field (`AI.Studio_...`); this is also the established naming behavior of the existing v0.4.0 release. The local frozen filenames, sizes, and SHA256 values above are unchanged.

The remote release contains exactly four uploaded assets. No database, log, model, ComfyUI, runtime package, source archive, or temporary evidence was uploaded.

## Version and validation evidence

- Product version: `0.5.0`
- Migration: `019`
- Backup format: `10`, with v1–v9 restore compatibility
- Rust regression: `485 passed / 0 failed / 1 ignored live GPU harness`
- Frontend regression: `52 files / 169 tests passed`
- Fresh RC startup: PASS
- v0.4 → v0.5 upgrade smoke: PASS
- Representative Krea2 → H3 production pipeline: PASS
- Frozen artifact embedded version: `0.5.0`
- Frozen artifact embedded build commit: `02e67cff50f5da1d207478071636af166048820c`

The previous release remains unchanged:

- `git rev-parse "v0.4.0^{}"`: `94918f6322ce690ff7b1630961abb56b8a31ed11`

## Checksum status

The repository checksum document retains the same `SOURCE_RC_SHA`, filenames, byte counts, and artifact SHA256 values. Its publication status is updated after the release upload to:

`PUBLISH_STATUS=PUBLISHED`

The post-publication working-tree checksum file is 465 bytes with SHA256 `E254353D3487EFF12FD3147A935ABD3B3DAB95162AAFAE65236DAD256567B6CD`; the remote checksum asset intentionally records the pre-publication file uploaded during the prescribed release sequence.
