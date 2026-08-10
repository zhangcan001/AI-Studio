# AI Studio 0.3.0 Final Release Gate

Date: 2026-08-10
Audit baseline: `6ae12dc5d92fcb6735b9baf771864d71ad4753c2`
Scope: 0.3.0 final hardening only; no new feature pack.

## Final status

`AI STUDIO 0.3.0 = RELEASE CANDIDATE / LIVE GATE PENDING`

代码、数据兼容和安装包门可以通过，但在当前桌面上下文无法获得可控的 Tauri WebView 操作与截图证据，因此不把 GPU/UI 实际生成链写成 PASS，也不创建 tag、GitHub Release 或上传二进制。

## RH03 gate matrix

| Gate | Result | Evidence / boundary |
| --- | --- | --- |
| RH03-01 Version consistency | PASS | `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 均为 `0.3.0`。 |
| RH03-02 Fresh DB 001 → 010 | PASS | `infrastructure/database/pool.rs` temporary SQLite migration test；22 个最终表、foreign keys 和 WAL 均校验。 |
| RH03-03 Existing DB 008 → 009 → 010 | PASS | 既有本地 DB migration smoke 与 Pack09/Pack10 upgrade evidence；本轮只读核验 `%LOCALAPPDATA%\AIStudio\AIStudioData\app.db` 的 `_sqlx_migrations` 为 1–10，当前 schema 保持 010。 |
| RH03-04 Backup v1–v4 | PASS | Backup service 接受 v1/v2/v3/v4；v4 Shot/Batch roundtrip 覆盖项目数据、关系和 ID remap。 |
| RH03-05 Exact Runtime scope | PASS | Rust/TypeScript scope helpers；精确 ID 与 `wfl_other` / `wfl_fake` 伪名称回归通过。 |
| RH03-06 Kera2 keyframe path | CODE PASS / LIVE PENDING | Planner、普通 Task、Snapshot、Asset、人工选图路径已覆盖；真实 3 Shot GPU/UI 链未在本轮声明通过。 |
| RH03-07 MiniMax H3 path | CODE PASS / LIVE PENDING | H3 绑定、严格顺序、参数边界和原生视频资产路径已覆盖；本轮未新增真实 H3 运行证据。 |
| RH03-08 Shot / Batch | PASS | Atomic Shot link binding、batch item freeze、one-item/one-link、progress and project isolation tests pass. |
| RH03-09 Human review / Safe retry | PASS | Kera2 不自动进入 H3；图片/视频候选需人工选择；失败保留、显式 retry 和 selected-result precedence 已覆盖。 |
| RH03-10 Recovery / restart | PASS | Existing task/queue recovery and uncertain-dispatch protection remain on the production chain；Pack10 hook-failure E2E pass. |
| RH03-11 Prompt Library | PASS | Project-scoped entries/versions、compare、apply、backup and privacy tests pass；apply 不自动生成。 |
| RH03-12 Experiment | CODE PASS / LIVE PENDING | 变体与二项 Kera2 queue 代码测试通过；Prompt v1/v2 → Studio 不自动提交的真实桌面链待补。 |
| RH03-13 Asset organization | PASS | 项目级收藏/标签/批量边界和跨项目拒绝测试 pass。 |
| RH03-14 Chinese UI / 1000×700 | PASS | 普通产品新增文案为简体中文；Tauri 最小尺寸为 `1000×700`；Endpoint 等技术标识不作为普通标题。 |
| RH03-15 Safe Exit | PASS | idle 不提示、active task/production 提示、activity query 失败保守提示均有测试；native close handler 保留确认。 |
| RH03-16 Single Instance | PASS | `tauri-plugin-single-instance` 已注册；二次启动聚焦现有窗口。 |
| RH03-17 Diagnostics privacy | PASS | diagnostics bundle 不包含 prompt、绝对路径、项目/资产内容；Rust 与 frontend boundary tests pass。 |
| RH03-18 Windows installer | PASS | `pnpm tauri build` 生成 NSIS、MSI 和 standalone `ai-studio.exe`；版本均为 0.3.0。 |
| RH03-19 Release docs / checksums | PASS | Scope Freeze、Release Notes、Final Gate 和本地 SHA-256 manifest 已闭环；无上传、Tag 或 GitHub Release。 |

## Integrated Live Gate (required evidence)

以下步骤是 0.3.0 从候选线升级为 Ready for Release 的必要证据，不以单元测试替代：

1. 创建项目 `Release Gate 0.3.0`，创建 3 个 Shot。
2. 配置 Kera2，生成 3 项严格顺序 batch；确认 3 个 Task、Snapshot 和至少 3 个图片 Asset，手动为每个 Shot 选择关键帧，并确认没有 H3 Task 自动出现。
3. 配置 MiniMax H3；选择 2 个 Shot，使用最小支持时长、`0.1MP`、`4 steps`，生成 2 项严格顺序 H3 batch；确认输入图片被冻结、视频 Asset 可播放，并手动选择最终视频。
4. 让后续项目任务失败，确认已经 `COMPLETED` 的 Shot 不被回滚；重启应用，确认已完成项、失败项、冻结配置和队列状态可恢复。
5. 执行 Pack06/07 低成本链：Prompt v1/v2、比较、v2 → Studio 手动生成、v1/v2 二项 Kera2 Experiment；保存/比较/加载均不自动生成。
6. 执行 Backup v4 UI export → inspect → restore roundtrip，核对 Shot、Prompt、Experiment/Queue、Task、Snapshot、Asset 和项目边界，确认 ID remap 后关系仍正确。

当前实机边界：本轮已启动正式 release executable，ComfyUI `http://127.0.0.1:8188/system_stats` 返回 HTTP 200，当前数据库只读核验无活动 Task 且 migration 仍为 1–10；但 Computer Use 的 `launch_app` 两次返回 `node_repl exec context not found`，重新枚举到真实窗口后再次捕获/激活仍返回同一错误。因此无法操作项目、表单、批次或媒体控件，以上集成步骤保持 `LIVE PENDING`，没有编造 Task/Asset/Playback 数量或成功结果。

## Regression commands

最终收口命令与结果：

```text
cargo fmt --all -- --check
cargo check
cargo test -- --test-threads=1
pnpm test
pnpm build
git diff --check
pnpm tauri build
```

- `cargo fmt --all -- --check` — PASS
- `cargo check` — PASS
- `cargo test -- --test-threads=1` — PASS，316 passed / 0 failed
- `pnpm test` — PASS，33 test files / 101 tests
- `pnpm build` — PASS
- `git diff --check` — PASS
- `pnpm tauri build` — PASS，NSIS + MSI + standalone EXE

NSIS、MSI、standalone EXE 的本地 SHA-256 写入 [`docs/RELEASE_SHA256_0.3.0.txt`](RELEASE_SHA256_0.3.0.txt)。这些文件只用于本地候选核验，不上传到 GitHub Release。
