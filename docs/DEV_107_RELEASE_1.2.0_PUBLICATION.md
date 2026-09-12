# DEV-107 — AI Studio 1.2.0 Publication Record

```text
TASK=DEV-107
VERSION=1.2.0
BASELINE_SHA=b97da6dfd343e887caf24c54de2130b849f63b1a
RELEASE_CODE_SHA=7e43cc5a841fc6d6f6036ec7c96a0d30b18200ea
RELEASE_COMMIT_SHA=7e43cc5a841fc6d6f6036ec7c96a0d30b18200ea
TAG=v1.2.0
TAG_PEELED=7e43cc5a841fc6d6f6036ec7c96a0d30b18200ea
RELEASE_ID=387503703
RELEASE_URL=https://github.com/zhangcan001/AI-Studio/releases/tag/v1.2.0
RELEASE_PUBLISHED_AT=2026-09-12T08:22:26Z
RELEASE_ASSET_COUNT=4

FEATURE_FREEZE=PASS
FRONTEND_TEST=PASS (148 files; 799 tests)
TSC=PASS
FRONTEND_BUILD=PASS
RUST_FMT=PASS
RUST_CHECK=PASS
RUST_TEST=PASS (1042 passed; 0 failed; 3 ignored)
ARCHITECTURE_GUARD=PASS
HANDOFF_JSON_SCHEMA=PASS
HANDOFF_SCHEMA_PARITY=PASS
500_SHOT_REGRESSION=PASS

FRESH_DB_001_TO_032=PASS
UPGRADE_1_0_TO_032=PASS (reconstructed 1.0 fixture 026→032)
UPGRADE_1_1_TO_032=PASS (reconstructed 031→032)
BACKUP_V18_ROUNDTRIP=PASS
LEGACY_V17_RESTORE=PASS
LEGACY_BACKUP_RESTORE=PASS
HANDOFF_PROVENANCE_RESTORE=PASS
ASSET_RELATION_RESTORE=PASS

TAURI_BUILD=PASS
MSI_BUILD=PASS
NSIS_BUILD=PASS
ISOLATED_APP_LAUNCH=PASS
FRESH_INSTALL=PASS (isolated 1.2.0 NSIS install)
REAL_1_1_TO_1_2_INSTALLER_UPGRADE=PASS
REAL_COMFY_SMOKE=NOT_RUN_ENVIRONMENT_UNAVAILABLE

REMOTE_CI_RUN=34681778931
REMOTE_CI_URL=https://github.com/zhangcan001/AI-Studio/actions/runs/34681778931
REMOTE_CI_STATUS=SUCCESS (Frontend 03:07; Rust 13:49)
TAG_CI_RUN=34682447829
TAG_CI_URL=https://github.com/zhangcan001/AI-Studio/actions/runs/34682447829
TAG_CI_STATUS=SUCCESS (Frontend 02:54; Rust 13:50)

P0=NONE
P1_RELEASE_BLOCKER=NONE
AI_STUDIO_1_2_RELEASE=PASS
AI_STUDIO_1_2_PUBLISHED=YES
POST_1_2_DEVELOPMENT_STARTED=NO
AI_STUDIO_1_3_STARTED=NO
AUTO_NEXT_TASK=NO
```

## Version and compatibility evidence

The release version is synchronized in `package.json`, `src-tauri/Cargo.toml`,
`src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. The existing
consistency gate now checks the package, Rust, and Tauri application versions
are all `1.2.0`; no third-party dependency versions were changed.

The full local release gate passed before the release-code push. It included
the frontend suite, TypeScript, frontend build, Rust format/check/tests,
architecture guard, handoff schema validation, 500-Shot/preparation/Queue/
Review/migration/backup regressions, and the Tauri release build. The final
release-code commit was pushed before Source-only CI run `34681778931`; the
tag push independently triggered run `34682447829`. Both runs matched the
release code SHA and both Frontend/Rust jobs succeeded.

The real installer smoke used the official published v1.1.0 NSIS installer
and the final v1.2.0 NSIS installer in isolated directories with an isolated
`AI_STUDIO_DATA_ROOT`. The 1.1 application created migration 031 data. A
controlled Project, Shot, Asset, workflow, Recipe, Production Batch, Task,
Review, selected output, and Shot/Asset relation survived installation of the
1.2.0 installer, which launched and reported migration 032. No formal user
installation, database, or registry path was used. A separate clean 1.2.0
NSIS install also launched and initialized a fresh migration-032 database.

The release artifacts were built from `RELEASE_CODE_SHA`:

| Artifact | Local build name | Published name | Bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| Portable | `ai-studio.exe` | `ai-studio.exe` | 52,007,936 | `6E7B79FB8544F5347A470787C85EF644566E018BD4103FC181B9FE60A4E5A7DB` |
| NSIS | `AI Studio_1.2.0_x64-setup.exe` | `AI.Studio_1.2.0_x64-setup.exe` | 11,260,704 | `BF67C256EA7BC59D832CB763AB1BD2C02F81F370557B8D69330AE3E3D105D7E2` |
| MSI | `AI Studio_1.2.0_x64_en-US.msi` | `AI.Studio_1.2.0_x64_en-US.msi` | 16,625,664 | `FA02B48D4046E3413BFB2A67BEE6FFC237D487F0127C0CD05FAA9DF0E019838B` |
| Checksums | `SHA256SUMS.txt` | `SHA256SUMS.txt` | 275 | `3BA18D44A052431EB22384F2CA415EE149472BE708906795789FA56862B99A83` |

MSI ProductVersion and portable/NSIS FileVersion/ProductVersion both report
`1.2.0`. All four published assets were downloaded into an external
verification directory and matched the local bytes and SHA-256 values.

## Release boundary

AI Studio 1.2.0 keeps the existing Production Queue as the sole explicit
execution authority. Handoff confirmation, preparation, and review rework do
not create an automatic Task or start ComfyUI. Migration remains 032 and
Backup remains V18. Real ComfyUI smoke was not run because both
`127.0.0.1:8188/system_stats` and `/object_info` were unavailable; this is
explicitly non-blocking under the approved release gate.

Deferred after publication: a live ComfyUI smoke when that runtime is
available and the existing frontend chunk-size warning. No P0 or P1 release
blocker remains. The next development phase is intentionally not started.
