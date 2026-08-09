# AI Studio 0.2.0 Post-Release Integrity Audit

审计日期：2026-08-10（Asia/Shanghai）

## 结论

```text
AI STUDIO v0.2.0 = RELEASED / VERIFIED
```

本轮只审计已经公开的 v0.2.0；没有移动、删除、重建或覆盖 Git tag、GitHub Release 或 Release 资产。

## Release 身份

| 项目 | 实际结果 |
| --- | --- |
| master HEAD | `6934368eb04e46cf8534f76919f795e682c7fc57` |
| v0.2.0 tag | annotated；tag object `ad2ec0de68468a4add5dd2a1a2cf7f5350839ff4` |
| tag target | `6934368eb04e46cf8534f76919f795e682c7fc57` |
| Release title | `AI Studio 0.2.0` |
| Draft / Prerelease | NO / NO |
| Published | YES；2026-08-09 23:06:27 UTC |
| Release URL | <https://github.com/zhangcan001/AI-Studio/releases/tag/v0.2.0> |

## 源码与版本

- `master` 与 `origin/master` 均指向 `6934368…`。
- `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 均为 `0.2.0`。
- 数据库迁移为 `001`–`008`，没有 `009`。
- `src-tauri/migrations/008_organization.sql` SHA-256 为
  `DB952B13F6D788E23701A29CB229BEAE4C36950AD69A367C4B108A5F2F819B20`。

## Release 资产

从 GitHub Release 重新下载 NSIS、MSI、裸 EXE 和 checksum 清单后，重新计算结果如下。GitHub 返回的存储名对空格做了点号规范化，但内容和摘要一致。

| 资产 | 实际 SHA-256 | 结果 |
| --- | --- | --- |
| `ai-studio.exe` | `CEE816A343978DB26BFFFA2A1D66D6D28391C9A8FF73DDB4762689FA1161FBAC` | PASS |
| `AI.Studio_0.2.0_x64-setup.exe` | `C61C532C400D61F106F6E959249B390647E0A325542228C53BAA7EAE8531B130` | PASS |
| `AI.Studio_0.2.0_x64_en-US.msi` | `E8DB7CA8FD9001DDC4A09C3F4221130FE0BC6D9CB782256E340C448F9064825C` | PASS |
| `RELEASE_SHA256_0.2.0.txt` | `4BD5FCE64C6D803A90A643C8284F5FB3175D3EC309BA87A7DA7717A7EECA6D4B` | PASS |

正式推荐安装方式仍为 NSIS；MSI 和裸 EXE 仍是 additional artifacts。

## 本机运行与数据证据

- 当前运行的 `src-tauri/target/release/ai-studio.exe` 响应正常，SHA-256 与公开裸 EXE 相同。
- `http://127.0.0.1:8188` 可用；ComfyUI `0.30.2`，检测到 NVIDIA GeForce RTX 5060 Ti，运行时能力返回 4485 个节点。
- 当前数据库只读盘点包含 projects、tasks、assets、presets、production batches、workflow packages 和 organization 数据；迁移表记录 001–008 全部成功。
- 现有启动日志反复记录 `database migration completed`、`runtime workflow library synchronized packages=6 valid=6 invalid=0`、`startup task recovery completed`；并记录过 `safe exit confirmed`。
- 现有真实任务历史包含 Kera2 成功任务和 MiniMax H3 历史成功任务；Pack 04 未修改 H3 runtime/compiler/task submission 核心，因此本轮没有重复 GPU H3 smoke。

## 自动化与构建回归

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check` | PASS |
| `cargo test -- --test-threads=1` | PASS；294 passed |
| `pnpm test` | PASS；71 passed / 26 files |
| `pnpm build` | PASS |
| `git diff --check` | PASS |
| `pnpm tauri build` | PASS；使用隔离临时 target，NSIS/MSI 均生成 |

直接使用默认 target 的一次构建只因当前运行中的同一 release EXE 被 Windows 占用而无法替换；没有覆盖该文件，随后隔离 target 构建完整通过。这不是用户安装、启动或数据风险。

## 边界与后续

- v0.2.0 Tag、GitHub Release、Release binaries 和 checksum 均冻结，不纳入本审计之后的新 commit。
- 当前本机尚未提供可直接导入并完成真实 API task 的第三 Runtime package；没有自动下载模型。第三 Runtime onboarding 状态为 `THIRD_RUNTIME_INPUT_REQUIRED`，不阻塞 Pack 05 的通用 UX、配置、导入质量和队列兼容性开发。
- 0.3.0 开发线从本审计之后的 master commit 开始，不创建 v0.3.0 tag 或 release。
