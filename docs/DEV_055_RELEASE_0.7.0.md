# DEV-055 — AI Studio 0.7.0 Release Gate

状态：已发布（2026-08-27）
产品目标：AI Studio 0.7.0 — Narrative Production V1

本文件记录 DEV-055 的可复核证据。没有实际执行或仍受环境限制的门禁明确标为 pending / ENV_BLOCKED，不以测试夹具或历史资产代替发布证据。

## 1. Baseline and safety

| 项目 | 结果 |
| --- | --- |
| DEV055_START_SHA | `ee7a28376617dcc3a97d5177e265ef450f0623a4` |
| branch | `master` |
| start working tree | clean |
| `HEAD == origin/master` at start | PASS |
| local/remote `v0.7.0` before work | ABSENT |
| GitHub Release `v0.7.0` before work | ABSENT (`gh release view` returned `release not found`) |

Historical peeled tag verification at start:

| tag | peeled SHA | result |
| --- | --- | --- |
| `v0.4.0` | `94918f6322ce690ff7b1630961abb56b8a31ed11` | UNCHANGED |
| `v0.5.0` | `02e67cff50f5da1d207478071636af166048820c` | UNCHANGED |
| `v0.6.0` | `e3d7181f23a9b7285a426efb20ead4db17198757` | UNCHANGED |
| `v0.6.1` | `62e406865e2998584f8cd3265b706b4e003aab0e` | UNCHANGED |
| `v0.6.2` | `542524feff8d4d65e8657125e6907898ca835bef` | UNCHANGED |

## 2. Multi-agent execution

DEV-055 used four parallel child agents with one writer per owned file and no nested agents. Each completed child was closed before final integration.

| agent | scope | status |
| --- | --- | --- |
| A | 500-shot performance and source-only GitHub Actions | DONE |
| B | migration, backup, manifest, and compatibility | DONE |
| C | real ComfyUI smoke | DONE |
| D | upper-scope and stage-aware UI truth | DONE |

`MULTI_AGENT_EXECUTION = CONFIRMED`
`ACTIVE_SUBAGENTS = 0`

## 3. DEV-054 closure

- Upper Project / Series / Episode / Scene scopes now show binding and inheritance truth only. They do not claim a final Shot `ResolvedShotContext` or invent a context hash.
- The explicit boundary copy is: “最终生成上下文在镜头层计算；当前页面展示本层配置和上级继承关系。”
- Shot consistency uses the real `shot_context_draft_get` path. Image and video stages have separate context labels and hashes.
- The historical DEV-054 document was corrected without claiming undocumented DEV-054 multi-agent evidence.

| closure gate | result |
| --- | --- |
| upper-scope fake context removed | PASS |
| upper-scope final resolver label | PASS |
| Shot image context | PASS (`HASH_IMAGE` in UI fixture) |
| Shot video context | PASS (`HASH_VIDEO` in UI fixture) |
| stage switch reload | PASS (Image → Video → Image) |

## 4. Pre-release and RC regression

The complete regression was run before changing the product identity:

| command | result |
| --- | --- |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | PASS — Rust lib 640 passed / 0 failed / 1 ignored; all integration targets passed |
| `pnpm test` | PASS — 92 files / 350 tests / 0 failed / 0 todo |
| `pnpm exec tsc --noEmit` | PASS |
| `pnpm build` | PASS — 199 modules; chunk-size warning only |
| `git diff --check` | PASS |

DEV-055 targeted evidence:

- Performance: 4 passed.
- Compatibility: 6 passed.
- UI closure tests: 14 passed.
- The live Comfy test is `#[ignore]` and is run explicitly only for the release gate.

After the identity bump to 0.7.0, the same full regression was run again:

| command | result |
| --- | --- |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS |
| `cargo check --manifest-path src-tauri/Cargo.toml` | PASS — `ai-studio v0.7.0`; warnings only |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | PASS — Rust lib 640 passed / 0 failed / 1 ignored; all integration targets passed |
| `pnpm test` | PASS — 92 files / 350 tests / 0 failed / 0 todo |
| `pnpm exec tsc --noEmit` | PASS |
| `pnpm build` | PASS — 199 modules; chunk-size warning only |
| `git diff --check` | PASS |

## 5. 500-shot and bulk evidence

Agent A created 500 distinct Shot identities in one isolated project and exercised the resolver, readiness/preflight, review, Command Center, and Audit paths. The 501-shot resolver limit was also asserted.

Latest local timings:

| measurement | value |
| --- | ---: |
| `500_SHOT_LIST_MS` | 11 ms |
| `500_CONTEXT_RESOLVE_MS` | 135 ms |
| `500_READINESS_MS` | 211 ms |
| performance risk | `P1_RISK=none` |

Latest bulk counters included:

```text
context: project_find=1 structure_load=1 shot_list_bulk=1
scope_profile_bulk=1 scope_reference_bulk=1
shot_profile_bulk=1 shot_reference_bulk=1
profile_list_bulk=4 costume_bulk=1 revision_bulk=1
reference_set_bulk=1 reference_item_bulk=1 asset_bulk=1
all single-shot fallbacks=0

readiness: comfy_health=1 comfy_object_info=2 workflow_source=1
runtime_find=0 definition_single=0 definition_bulk=1

review-100: task_single=0 asset_single=0 review_find=0
lineage_single=0 snapshot_single=0; bulk reads bounded
```

Command Center and Audit use identity/set-based reads for the 500-shot fixture and do not resolve 500 contexts or fetch 500 snapshot payloads.

## 6. Migration, backup, and manifest compatibility

| gate | result |
| --- | --- |
| fresh migration 001 → 024 | PASS |
| existing 021 → 024 | PASS |
| existing 023 → 024 | PASS |
| maximum migration | 24 |
| migration 025 | ABSENT |
| Backup v12 inspect/restore | PASS |
| Backup v13 restore | PASS |
| Backup v14 consistency + preparation snapshot roundtrip | PASS |
| Manifest v1 read/import | PASS |
| Manifest v2 consistency roundtrip | PASS |
| Preparation Snapshot in Manifest | NO — intentionally excluded |

## 7. Real ComfyUI gate

The live test ran through the existing `ProductionQueueService → GenerationService → WorkflowCompiler → Comfy adapter → ComfyUI` path. It did not call `POST /prompt` directly and did not interrupt or clear the shared ComfyUI queue.

Environment evidence:

- endpoint: `http://127.0.0.1:8188`
- ComfyUI: `0.33.4`
- `/system_stats`: PASS
- `/object_info`: PASS; 4525 nodes observed
- queue was idle before the run: PASS
- Krea2 capability: READY
- MiniMax H3 I2V capability: READY
- REF2VA capability: READY

The explicit live run produced new DEV-055 isolated artifacts:

| flow | task | asset | SHA-256 | result |
| --- | --- | --- | --- | --- |
| Krea2 image | `tsk_a207aed6-5b25-40d8-abbc-3802b8d12d91` | `ast_d2e005c4-df5f-4a33-8e1e-f19aac64cff7` | `2e5c71c6e279ed50cebd0bf91512bad91dc07a3bf39d358b7d370e1f0a8062d9` | PASS |
| H3 I2V | `tsk_f41a5a91-eb05-4900-8139-260154f60483` | `ast_4bb452bb-b2e2-4d7b-a664-25da1e732ae7` | `1c8be8026619c3620850c8dda82c8d422cfd698c960ac2e09f4bcbd6a3c5d2fa` | PASS |
| REF2VA | `tsk_006e39fe-3576-489f-b1da-d32eecf6292a` | `ast_a3a7ff93-e544-4117-a633-73384b1edab1` | `c83671a0bcb9f5e840c9608bae17190284f096ea4f3f043fd0754836bcbe0933` | PASS |

Manual image selection was explicit and passed. The H3 stage input asset was the selected Krea2 image with the same SHA-256. Snapshot, physical file, dimensions, and Comfy execution identities were verified by the live test.

The live result was captured before the final source-audit-only follow-up commits. No product runtime code changed after the live run; the frozen 0.7.0 artifacts were rebuilt from `SOURCE_RC_SHA=e4a643d4b31329e291c2fb40002f1554e8a1ab34`. A post-RC lightweight recheck returned `/system_stats` HTTP 200 and `/object_info` HTTP 200 with ComfyUI 0.33.4, 4525 nodes, and the required KSampler, LoadImage, and CLIPTextEncode nodes.

## 8. UI smoke status

The browser smoke used an isolated Tauri API mock and was not counted as a live generation result. It verified the Chinese global rail, project/creation/shot consistency surfaces, Image → Video context labels, and no horizontal overflow:

| viewport | result |
| --- | --- |
| 2048×1088 | PASS; document width 2048 |
| 1200×800 | PASS; document width 1200 |
| 1000×700 | PASS; document width 1000 |

Upper-scope copy and Shot stage context were visible in the snapshots. The full Assets → Creation → Production → Review → Audit route smoke passed at all three required viewports; no horizontal overflow was observed.

## 9. Version identity and RC checklist

The product identity bump and its post-bump regression are complete. The following identity is recorded before the Source RC commit:

```text
package.json       0.7.0
src-tauri/Cargo.toml 0.7.0
src-tauri/Cargo.lock 0.7.0 (ai-studio package only)
src-tauri/tauri.conf.json 0.7.0
Migration          024
Backup             14
Manifest           2
```

Completed release actions:

- Source RC and follow-up test-stability/source-audit commits were pushed to `master`; final `SOURCE_RC_SHA` is `e4a643d4b31329e291c2fb40002f1554e8a1ab34`.
- Clean 0.7.0 build completed. The frozen executable reports ProductName `AI Studio`, ProductVersion `0.7.0`, and FileVersion `0.7.0`.
- Annotated tag `v0.7.0` was pushed with message `AI Studio v0.7.0 - Narrative Production V1`; tag object `d4b0b0cc8e706571857cfd844cd52caea891ca3c` peels to the source RC.
- The draft release had exactly four assets. Independent downloads matched all frozen artifact names, byte counts, and SHA-256 values before publication.
- The GitHub Release was published, and this record is the required post-publication docs-only commit.

## 10. Findings and release decision

Final findings:

- P0: none observed.
- P1: none observed.
- P2: MSI install execution was `ENV_BLOCKED/P2` because Windows returned Error 1925/1603 for both old and candidate installers; static upgrade-code/version checks passed. GitHub Actions reported only a Node 20 action deprecation annotation, not a product failure. REF2VA live execution passed and has no P2 finding.

Release decision: `RELEASED`.

The decision gates are satisfied: P0=0, P1=0, Krea2 and MiniMax H3 I2V live PASS, full regression PASS, isolated upgrade/backup/manifest PASS, artifact identity and independent hash verification PASS, and a published GitHub Release with exactly four assets.

## 11. Published release evidence

| item | result |
| --- | --- |
| source RC | `e4a643d4b31329e291c2fb40002f1554e8a1ab34` |
| annotated tag | `v0.7.0` → `e4a643d4b31329e291c2fb40002f1554e8a1ab34` |
| tag object | `d4b0b0cc8e706571857cfd844cd52caea891ca3c` |
| GitHub Release | [AI Studio 0.7.0 — Narrative Production V1](https://github.com/zhangcan001/AI-Studio/releases/tag/v0.7.0) |
| published at | `2026-08-27T09:25:32Z` |
| draft / prerelease | `false / false` |
| asset count | `4` |

Frozen assets and independently verified downloads:

| asset | bytes | SHA-256 |
| --- | ---: | --- |
| `ai-studio.exe` | 47374848 | `1BA8E43D0B5FC1762C346E3283E7A4A5E0AE14347C09B6E51209DF63A4440AF5` |
| `AI.Studio_0.7.0_x64-setup.exe` | 10324107 | `F68866FF364E31952372F721DD451AF8BC9D375FC509E0B206162FD22081BAB1` |
| `AI.Studio_0.7.0_x64_en-US.msi` | 16396288 | `5693A354AB74EB5BDB196AB2A5F4BBD6B1E57CBBBF98E50F58FFACB310F0A14F` |
| `RELEASE_SHA256_0.7.0.txt` | 402 | `D68F94F8EF8C0FBA85AE191EC60EC2E931E60DC5BF56DA67B7563FC758C731C6` |

The checksum manifest records the three binary hashes and `SOURCE_RC_SHA` above. No release asset was overwritten during the final four-asset verification.

## 12. Compatibility and execution boundaries

- Official 0.6.2 → 0.7.0 portable upgrade passed in an isolated data root: migration `024`, sentinel project and shot preserved, migration `025` absent; restart also passed.
- Official NSIS upgrade, fresh install, and silent uninstall passed. MSI static checks confirmed the same UpgradeCode and 0.7.0 product version; MSI execution remains the documented environment-only P2.
- The live Krea2, H3 I2V, and REF2VA runs used `ProductionQueueService → GenerationService → WorkflowCompiler → Comfy adapter → ComfyUI`. There was no direct `POST /prompt` bypass and no second execution engine.
- Manual gate, candidate selection, queue start, and rework remain explicit user actions. Review regeneration paths use `autoStart=false`.

## 13. Final CI record

The final Windows Source-only CI run [33056416415](https://github.com/zhangcan001/AI-Studio/actions/runs/33056416415) passed all Rust, frontend test, TypeScript, and build steps: Rust library 640 passed / 0 failed / 1 ignored, all integration targets passed, frontend 92 files / 350 tests passed, TypeScript passed, and the frontend build passed. The only annotation was the non-blocking Node 20 action deprecation notice.
