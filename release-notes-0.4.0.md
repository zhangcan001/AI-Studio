# AI Studio 0.4.0

AI Studio 0.4.0 delivers the Production Run workflow from Krea2 image generation through manual selection and H3 video generation.

## Highlights

- Production Run orchestration with Krea2 multi-candidate seed diversification.
- Deterministic fixed-seed candidates with frozen BatchItem and Snapshot evidence.
- Manual asset selection and H3 FAST / QUALITY generation.
- Restart recovery, duplicate submission protection, retry, and full asset lineage.
- Benchmark 2.0 support.
- Database migration 018 and Backup v9 compatibility.

## Validation

- Source RC: `94918f6322ce690ff7b1630961abb56b8a31ed11`
- Rust regression: `464 passed`
- Frontend regression: `46 files / 152 tests passed`
- H3 FAST clean-restart smoke: PASS with a real MP4 asset at `864×480`.

## Known non-blockers

- REF2VA Production Run live/UI validation remains post-0.4.0.
- Full isolated installer fresh/upgrade smoke remains post-0.4.0 because Windows Tauri data-root redirection is not available in the current environment.
