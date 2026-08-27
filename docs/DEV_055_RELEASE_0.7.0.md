# DEV-055 — AI Studio 0.7.0 Release Gate

状态：Source RC 验收进行中，尚未发布
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

This live result predates the 0.7.0 version-only/docs-only RC changes; a post-RC connection/capability/preflight recheck remains required.

## 8. UI smoke status

The browser smoke used an isolated Tauri API mock and was not counted as a live generation result. It verified the Chinese global rail, project/creation/shot consistency surfaces, Image → Video context labels, and no horizontal overflow:

| viewport | result |
| --- | --- |
| 2048×1088 | PASS; document width 2048 |
| 1200×800 | PASS; document width 1200 |
| 1000×700 | PASS; document width 1000 |

Upper-scope copy and Shot stage context were visible in the snapshots. Full Assets → Creation → Production → Review → Audit route smoke remains part of the RC/final UI gate.

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

Pending RC actions: Source RC commit/push, clean RC build, artifact identity/SHA-256, portable and installer gates, GitHub Actions result, annotated tag, draft-release download verification, publication, and post-publication docs commit.

## 10. Findings and release decision

Current findings before RC build:

- P0: none observed.
- P1: none observed.
- P2: official binary/installer environment gates and GitHub runner status are pending; the compatibility fixture may report `ENV_BLOCKED/P2` when no official binary is supplied.

No release decision is made by this pre-RC record. The final decision is valid only after P0=0, P1=0, Krea2 and H3 live PASS, full regression PASS, upgrade PASS, artifact hash verification PASS, and a published GitHub Release with four independently verified assets.
