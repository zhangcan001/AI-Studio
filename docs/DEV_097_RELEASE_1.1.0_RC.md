# DEV-097 — AI Studio 1.1.0 Release Candidate Evidence

Date: 2026-09-11

## Release decision

```text
TASK=DEV-097
TARGET_VERSION=1.1.0
BASELINE=654c9e767fe6e5f04ef0f2828f2a24616389d262
SOURCE_RC_SHA=4ed89892a810ac7c1c0ce1cd84d5b58be28ca524
SOURCE_RC_CI_RUN=34574156594
SOURCE_RC_CI=GREEN
SOURCE_FREEZE=YES
LATEST_MIGRATION=031
```

The 1.1.0 source RC is the version-only release-boundary update plus the
existing consistency-test expectation update required by the version bump.
No product feature, queue/executor/task model, scheduler, telemetry, or
migration 032 was added. Historical 1.0.0 evidence remains unchanged.

## Source and schema gates

```text
FRESH_DB=PASS
UPGRADE_1_0_TO_031=PASS
PRAGMA_FOREIGN_KEYS=1
MIGRATION_032=ABSENT
BACKUP_VERSION=17
BACKUP_COMPATIBILITY=PASS
RUNTIME_PACKAGE_COMPATIBILITY=PASS
WORKFLOW_REGISTRY=PASS
RECIPE_LIFECYCLE=PASS
PROJECT_WORKFLOW_BINDING=PASS
PRODUCTION_PACKAGE_V1=PASS
PRODUCTION_QUEUE=PASS
TYPED_IPC=PASS
ARCHITECTURE_GUARD=PASS
NO_NEW_QUEUE=YES
NO_NEW_EXECUTOR=YES
NO_NEW_TASK_MODEL=YES
AUTO_START_ON_CREATE=NO
AUTO_RETRY=NO
```

Evidence:

- Migration sequence is complete from 001 through 031; migration 032 is absent.
- The release compatibility suite passed 7 tests, including fresh migration,
  legacy/consistency projects, manifest compatibility, the migration matrix,
  the official 062→070 isolation guard, Backup 17, and the reconstructed
  1.0 fixture upgrade from 026 to 031.
- Backup compatibility passed 32 tests, covering legacy imports, current
  export/import and roundtrip, ID remap, byte/relation preservation, rollback,
  Zip Slip rejection, and invalid archives.
- Focused release regression passed for compatibility, Workflow Registry V2,
  runtime artifact reconciliation, production admission, queue recovery,
  production package and hardening, unified workflow library, workflow add,
  IPC error contract, application/backup boundaries, and runtime integration.
- `node scripts/dev088-architecture-guard.mjs` passed every sentinel, including
  zero application direct SQLx, typed IPC parity, exact recipe identity, and
  no new queue/executor/task authority.
- The existing version consistency gate was updated from the historical 1.0.0
  expectation to the active 1.1.0 version and then passed.

## Full local gates

```text
FULL_FRONTEND=PASS
FRONTEND_TEST=PASS (147 files, 790 tests)
TSC=PASS
FRONTEND_BUILD=PASS (257 modules)
RUST_FMT=PASS
RUST_CHECK=PASS
FULL_RUST=PASS (808 passed, 0 failed, 1 ignored)
SECRET_SCAN=PASS
DIFF_CHECK=PASS
```

Remote Source-only CI run 34574156594 matched `SOURCE_RC_SHA` and completed
with both `Rust source checks` and `Frontend source checks` successful.

## Windows artifacts

```text
TAURI_BUILD=PASS
EMBEDDED_COMMIT=PASS (4ed89892a810ac7c1c0ce1cd84d5b58be28ca524)
PORTABLE_SMOKE=PASS
NSIS_SMOKE=PASS
MSI_SMOKE=PASS
```

Artifacts were built from the exact source RC and staged outside the repository
at:

`C:\Users\ADMIN\Documents\ChatGPT\AI-Studio-1.1.0-STAGING-20260911-154127`

| Artifact | Bytes | SHA-256 | Version evidence |
| --- | ---: | --- | --- |
| `ai-studio.exe` | 51593216 | `5464F9BAA3B6ED0BAE12BB68ED65FBC247645864690D2D6F0AAC31B7FDE4121D` | FileVersion/ProductVersion 1.1.0 |
| `AI Studio_1.1.0_x64-setup.exe` | 11093998 | `C81796C12366CAB2B4D158B0D3A180DBC35DA634E7D871953F9C402C4A6E68AB` | FileVersion/ProductVersion 1.1.0 |
| `AI Studio_1.1.0_x64_en-US.msi` | 16490496 | `ACC0F5EC3E161B5A70C95EDF84EB46A2538FCBE6BE13C88DA842D5807A2FC519` | MSI ProductVersion 1.1.0; installed executable 1.1.0 |

Checksum file:

```text
RELEASE_SHA256_1.1.0.txt
BYTES=357
SHA256=316B031B0EDEF15A120F7848AE494D7E29E5A346DFDCAA1303B97B231491BFDF
```

Portable smoke launched the isolated executable, observed a responding process
and 1.1.0 file/product version, then closed it. NSIS smoke installed to an
isolated directory, launched the installed executable, verified 1.1.0, and
uninstalled with exit 0 and no installed files remaining. MSI smoke used an
isolated `INSTALLDIR`; the non-elevated probe correctly returned Error 1925,
then the same clean install/uninstall path passed under explicit elevation.
No formal user installation or registry path was used; the only registry
cleanup was a verified stale key whose value pointed to the isolated NSIS test
path.

## Official v1.0.0 and upgrade evidence

```text
OFFICIAL_1_0_ARTIFACT_VERIFY=PASS
UPGRADE_SMOKE_1_0_TO_1_1=PASS
FORMAL_USER_DATA_TOUCHED=NO
```

All four downloaded official v1.0.0 assets matched the hashes and sizes in
`docs/DEV_074_RELEASE_1.0.0.md`:

- `ai-studio.exe`: 48568832 bytes,
  `785E3FA95EFD737CCD725D4145E26051E6C6376821EBB70BA229BFF5AEC622F7`.
- `AI.Studio_1.0.0_x64-setup.exe`: 10547492 bytes,
  `39C560FCC422B28A4740A9C527BFAAA0835486001B8A448D8859200411D57002`.
- `AI.Studio_1.0.0_x64_en-US.msi`: 16789504 bytes,
  `21BB08B6E5AE20E40EBAAC062FE5C81C5BB144820676500437252AD91FD53872`.
- `RELEASE_SHA256_1.0.0.txt`: 502 bytes,
  `C62ECF269443BA9D9BBA7B042B0DE0925FAD655124C50D77E94A7106F9B6A501`.

The official 1.0.0 NSIS installer was installed in an isolated directory and
launched with `AI_STUDIO_DATA_ROOT` set to an isolated data root. The resulting
controlled fixture was schema 026 with project, shot, asset, task, snapshot,
queue, production package binding, workflow/version, recipe, preset, project
template, benchmark, and production run/stage rows. The 1.1.0 RC NSIS
installer upgraded that isolated installation in place. The app launched and
responded; the database migrated to schema 031 with foreign keys enabled and
all fixture identities remained readable. Missing project binding, promotion,
and recipe archive rows remained absent with the frozen safe defaults:
existing behavior, not promoted, and recipe active. The isolated installation
was uninstalled after verification while the database evidence remained
outside the repository.

## Optional ComfyUI preflight

```text
COMFY_PREFLIGHT=UNAVAILABLE
REAL_GENERATION=NOT_RUN
```

`http://127.0.0.1:8188/system_stats` and
`http://127.0.0.1:8188/object_info` were unavailable. No user ComfyUI
configuration was changed. This is non-blocking under the release train
because the 1.0.0 release contains the formal real-production evidence and
1.1.0 changes are lifecycle/compatibility hardening.

## RC conclusion

```text
BLOCKING_RELEASE_ISSUES=NONE
AI_STUDIO_1_1_0_RC=PASS
```

The source RC is ready for the formal tag and publication phases. The release
tag must point to `SOURCE_RC_SHA`, not this documentation commit.
