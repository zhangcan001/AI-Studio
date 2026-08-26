# DEV-045B FINAL — AI Studio 0.6.2 Release Evidence

## Final decision

**AI Studio 0.6.2 is published. C UI release complete.**

GitHub Release: [AI Studio 0.6.2](https://github.com/zhangcan001/AI-Studio/releases/tag/v0.6.2)

- Published: `2026-08-26T01:54:24Z`
- Draft: `false`
- Prerelease: `false`
- Assets: `4`

## Baseline and tag repair

- `DEV045B_START_SHA`: `542524feff8d4d65e8657125e6907898ca835bef`
- Baseline branch: `master`
- Baseline working tree: clean
- Baseline `HEAD == origin/master`: PASS
- GitHub v0.6.2 Release before repair: **NOT FOUND**
- `OLD_V062_TAG_OBJECT_SHA`: `79e85b796595e48ba818589de23568fee6a4db4b`
- `OLD_V062_PEELED_SHA`: `7931671058d4f39fba4bfc815148feb32b2323b2`
- Old v0.6.2 tag: deleted locally and remotely before replacement
- `SOURCE_RC_SHA`: `542524feff8d4d65e8657125e6907898ca835bef`
- `NEW_TAG_OBJECT_SHA`: `73a46aaad6b11ac75cc708bd3a21935dd79868a6`
- `NEW_TAG_PEELED_SHA`: `542524feff8d4d65e8657125e6907898ca835bef`

The old-tag-to-source diff contained only the two accepted UI commits:

- `491f95f48854f6087bfced920b464db42e0054b4` — restore compact studio workspace layout
- `542524feff8d4d65e8657125e6907898ca835bef` — polish studio workspace UI

Changed files were limited to `App.tsx`, `StudioShell.css`, `ProductionBatchRunbookPanel.css`, and `uiPolish.css`. No database, migration, generation, queue, production, or review semantics changed.

## Version identity

- `package.json`: `0.6.2`
- `src-tauri/Cargo.toml`: `0.6.2`
- `src-tauri/Cargo.lock` application package: `0.6.2`
- `src-tauri/tauri.conf.json`: `0.6.2`
- Migration: `021`
- `BACKUP_VERSION`: `12`
- Project manifest: `1`
- Migration `022`: absent

## Regression

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: PASS
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS
- Rust tests: `629 passed / 0 failed / 1 ignored`
- Ignored Rust test: the explicit external/live ComfyUI gate only
- `pnpm test`: `80 test files / 289 tests passed / 0 todo`
- `pnpm build`: PASS
- `git diff --check`: PASS
- `pnpm tauri build` from detached `SOURCE_RC_SHA`: PASS

## UI smoke

Tauri dev was launched from the final master without GPU generation. The current WebView accessibility tree verified:

- Chinese studio shell and status UI
- C-scheme studio workspace
- Project Tree and Shot Workspace
- Main Preview
- Fit, 100%, zoom in, and zoom out controls
- Candidate preview with 19 candidates
- Single manual candidate confirmation
- Compact Inspector
- Production queue
- Separate Creation, Production, Review, Analysis, and Settings navigation

The same accepted source had already passed Playwright visual checks at 2048×1088, 1200×800, and 900×700. `ZoomableImagePreview` tests cover Fit, 100%, button zoom steps, and pan bounds. Direct Windows coordinate injection in this run was unavailable because the capture layer returned `SetIsBorderRequired: interface not supported`; this was recorded as an environment tooling limitation, not a product failure.

Runtime preflight: `http://127.0.0.1:8188` was not started, recorded as `RUNTIME_NOT_STARTED`. No ComfyUI installation, update, or GPU generation was performed.

## Frozen artifacts

Local frozen directory: `C:\Users\ADMIN\AppData\Local\Temp\AI-Studio-0.6.2-Release-542524fe`

| Local filename | Published filename | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `ai-studio.exe` | `ai-studio.exe` | 41,373,696 | `56653CE566A287F8F8A28CA3247DB978D802D6D552134B0C2923E9AD55ADE607` |
| `AI Studio_0.6.2_x64-setup.exe` | `AI.Studio_0.6.2_x64-setup.exe` | 9,219,913 | `AC0FAC6D19C81130EB883840D1B8E4768556B3FCC3575A3D23292223E5E06DB1` |
| `AI Studio_0.6.2_x64_en-US.msi` | `AI.Studio_0.6.2_x64_en-US.msi` | 13,582,336 | `2FDA858B4ECD97D9452DF0AF2052C43BA1A8C99570BBA950C99872168B056E24` |
| `RELEASE_SHA256_0.6.2.txt` | `RELEASE_SHA256_0.6.2.txt` | 465 | `1D2A157D88E4E01F3DB632E084EEEF2D31F3ADAD8D146F04870BB2150BE133E7` |

The published assets were downloaded into an independent empty directory and matched the frozen local files byte-for-byte and SHA256-for-SHA256.

## Portable validation

- 0.6.2 portable fresh launch: PASS
- 0.6.2 portable restart: PASS
- Isolated data root created `app.db`, `projects`, `workflow_library`, `workflow_staging`, `config`, `cache`, and `logs`
- Product/FileVersion: `0.6.2`
- Fresh database max migration: `21`
- 0.6.1 official portable → 0.6.2 portable: PASS
- Sentinel preserved: Project, Series, Episode, Scene, Shot, Reference Anchor, Prompt, Prompt Version, and Asset
- Upgrade database max migration: `21`
- Upgrade restart: PASS

## Installer validation

### NSIS

- 0.6.2 fresh install: PASS, silent installer exit `0`
- Fresh launch/restart: PASS
- Fresh uninstall: PASS, exit `0`
- Isolated data remained after uninstall: PASS
- Official 0.6.1 NSIS → 0.6.2 NSIS upgrade: PASS, installer exit `0`
- Upgrade launch/restart: PASS
- Sentinel preservation after upgrade: PASS
- Upgrade uninstall: PASS, exit `0`

### MSI

Static inspection:

- ProductName: `AI Studio`
- 0.6.2 ProductVersion: `0.6.2`
- 0.6.1/0.6.2 UpgradeCode: `{D254323C-FE50-56FC-BE8E-86830497F401}`
- 0.6.2 ProductCode: `{FE7FD62C-D8E3-4ED7-A531-924E347F0100}`

Installation attempts:

- Official 0.6.1 control: exit `1603`, Windows Installer Error `1925`
- 0.6.2 candidate: exit `1603`, Windows Installer Error `1925`
- Classification: `MSI=ENV_BLOCKED`, P2; both control and candidate failed identically in this environment

## Findings

- P0: `0`
- P1: `0`
- P2: `1` — MSI environment privilege block (`Error 1925`)
- Non-blocking existing warnings: favicon 404, passive event listener warning, Rust dead-code warnings, Vite chunk-size warning, and unavailable local ComfyUI runtime

## Previous release protection

The protected tags were unchanged:

- `v0.4.0^{}` → `94918f6322ce690ff7b1630961abb56b8a31ed11`
- `v0.5.0^{}` → `02e67cff50f5da1d207478071636af166048820c`
- `v0.6.0^{}` → `e3d7181f23a9b7285a426efb20ead4db17198757`
- `v0.6.1^{}` → `62e406865e2998584f8cd3265b706b4e003aab0e`

## Final Gate

- Remote `v0.6.2^{}`: `542524feff8d4d65e8657125e6907898ca835bef`
- Remote Release: published, non-draft, non-prerelease, 4 assets
- Product version: `0.6.2`
- Migration: `021`
- Backup: `12`
- Manifest: `1`

Publication is complete. Development stops at this release boundary.
