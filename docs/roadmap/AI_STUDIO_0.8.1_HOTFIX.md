# AI Studio 0.8.1 — Production Queue Hotfix Publication

状态：**PUBLISHED**

AI Studio 0.8.1 是针对 0.8.0 真实 Production Package UAT P1 问题的最小修复发布。问题表现为 generic Production Package batch 创建后无法稳定出现在普通 Production Queue；修复后 batch 可见、可 focus、可手动 Start，打开 Queue 不会自动启动生产。

## Release record

| 项目 | 值 |
| --- | --- |
| Product | `0.8.1` |
| Source RC | `78180e2136a39d6b739d086bf5610c90f4e11240` |
| Tag | `v0.8.1` |
| Tag object | `a032778f3fc4a40ae67f3f9dead148472db5d34b` |
| Tag peeled SHA | `78180e2136a39d6b739d086bf5610c90f4e11240` |
| Release ID | `RE_kwDOTuxMh84WmWAU` |
| GitHub Release | <https://github.com/zhangcan001/AI-Studio/releases/tag/v0.8.1> |
| Source-only CI | `33227945425` — success |
| Release state | `draft=false`, `prerelease=false` |
| Assets | `4` |

## Scope retained from 0.7.0 / DEV-061B

- DEV-061A Bulk Import Dry-Run is explicitly `OPTIONAL`.
- DEV-061B remains the Bulk Production Hardening baseline: partial `COMPLETE/PARTIAL` truth, created/remaining IDs, 500 items in 5 batches of 100, restart persistence, `COMFY_OFFLINE` recovery, retry lineage and idempotency.
- Queue creation remains task-free and Comfy-submit-free until the user performs Manual Start.
- Retry uses the imported Project Asset and frozen values; it does not reread the external package original.
- Existing ProductionQueue / recovery architecture remains the sole execution path.
- Migration `025`, Backup `15`, Manifest `2`; Migration `026` is absent.

## Verification evidence

- Official installed 0.8.1 UAT: package selection, inspection, creation, queue visibility, manual Start and real H3/ComfyUI video: PASS.
- Full Rust all-target regression: PASS; no failed tests.
- Frontend tests: `97` files / `382` tests PASS.
- TypeScript and Vite production build: PASS.
- Portable smoke: exit `0`; NSIS isolated install: exit `0`, ProductVersion `0.8.1`; MSI ProductVersion: `0.8.1`.
- GitHub Release download verification: all `4` assets downloaded from the Release and matched the local SHA-256 manifest.

## Published assets

| Asset | SHA-256 |
| --- | --- |
| `ai-studio.exe` | `1D66E4A52A07468CB8E087F5A9CF03A9559289674273C91FE8B7DA31D5A11CA1` |
| `AI.Studio_0.8.1_x64-setup.exe` | `653D6763AA34DEC6099729077BA9831477043F692D340F73CC2E0221964AD05C` |
| `AI.Studio_0.8.1_x64_en-US.msi` | `B944CAA5F7614EC86DB25A874854246B07CB5666A89609F342537EDC4FEEE5A4` |
| `RELEASE_SHA256_0.8.1.txt` | `87348DA79EDEDF564FEE13C7EAE7488545949EDBE3FCEF4945ED60B7009F45F7` |

本记录不修改 0.8.0 tag、Release 或历史发布文档。DEV-067 的发布前置条件已满足：`DEV-067_RELEASE_GUARD = UNLOCKED`。
