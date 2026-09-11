# DEV-098 — AI Studio 1.1.0 Publication Record

Date: 2026-09-11

## Published release

```text
TASK=DEV-098
AI_STUDIO_VERSION=1.1.0
SOURCE_RC_SHA=4ed89892a810ac7c1c0ce1cd84d5b58be28ca524
TAG=v1.1.0
TAG_PEELED=4ed89892a810ac7c1c0ce1cd84d5b58be28ca524
SOURCE_RC_CI_RUN=34574156594
SOURCE_RC_CI=GREEN
TAG_CI_RUN=34576670834
TAG_CI=GREEN
RELEASE_ID=RE_kwDOTuxMh84XD0E2
RELEASE_PUBLISHED_AT=2026-09-11T08:12:32Z
RELEASE_ASSET_COUNT=4
REMOTE_HASH_VERIFY=PASS
PUBLISHED_PORTABLE_SMOKE=PASS
PUBLISHED_NSIS_SMOKE=PASS
PUBLISHED_UPGRADE_SMOKE=PASS
FORMAL_USER_DATA_TOUCHED=NO
P0=NONE
P1=NONE
AI_STUDIO_1_1_0=PUBLISHED
```

GitHub Release: `v1.1.0 — AI Studio 1.1.0 — AI Production Workbench`.
The release is published (`draft=false`, `prerelease=false`) and contains only
the four formal assets:

- `ai-studio.exe` — 51593216 bytes,
  `5464F9BAA3B6ED0BAE12BB68ED65FBC247645864690D2D6F0AAC31B7FDE4121D`.
- `AI.Studio_1.1.0_x64-setup.exe` — 11093998 bytes,
  `C81796C12366CAB2B4D158B0D3A180DBC35DA634E7D871953F9C402C4A6E68AB`.
- `AI.Studio_1.1.0_x64_en-US.msi` — 16490496 bytes,
  `ACC0F5EC3E161B5A70C95EDF84EB46A2538FCBE6BE13C88DA842D5807A2FC519`.
- `RELEASE_SHA256_1.1.0.txt` — 357 bytes,
  `316B031B0EDEF15A120F7848AE494D7E29E5A346DFDCAA1303B97B231491BFDF`.

The checksum file records the three binary assets and their bytes/SHA-256.
GitHub's normalized `AI.Studio_...` names are the remote representation of the
local Tauri installer names with spaces.

## Remote verification

The four release assets were downloaded from GitHub into the external
verification directory
`C:\Users\ADMIN\Documents\ChatGPT\AI-Studio-1.1.0-REMOTE-VERIFY-20260911-1613`.
All four remote sizes and SHA-256 values matched the local RC staging directory
exactly. The release API digests also matched each local hash:

```text
REMOTE_PORTABLE_HASH=PASS
REMOTE_NSIS_HASH=PASS
REMOTE_MSI_HASH=PASS
REMOTE_CHECKSUM_HASH=PASS
REMOTE_HASH_VERIFY=PASS
```

## Published smoke evidence

- The downloaded published portable executable launched with an isolated data
  root, responded, reported FileVersion/ProductVersion 1.1.0, and was closed.
- The downloaded published NSIS installer installed into an isolated directory,
  launched the installed 1.1.0 executable, and uninstalled with exit 0 and no
  installed files remaining.
- The official v1.0.0 NSIS installer was installed into a fresh isolated
  directory and launched against a controlled schema-026 fixture. The just
  published v1.1.0 NSIS installer upgraded that installation in place. After
  launch, the database reported schema 031 with foreign keys enabled and the
  controlled Project, Task, Asset, Queue, Workflow, Recipe, and Production
  Package binding rows remained readable. Missing project binding, promotion,
  and archive rows remained the frozen safe defaults. The isolated installation
  was uninstalled after verification.

No formal user database, formal user installation, or formal registry path was
touched. Installer smokes used only directories under the external verification
roots, and any cleanup was restricted to registry keys verified to point to
those test roots.

## Final repository state

- `master` contains the publication record and README truth update.
- The annotated `v1.1.0` tag peels to the immutable source RC commit, not the
  RC or publication documentation commits.
- `docs/DEV_074_RELEASE_1.0.0.md` and the v1.0.0 tag/assets were not modified.
- The next development task is intentionally not started.
