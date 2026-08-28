# DEV-061A — Project Bulk Import Dry-Run

状态：**PASS / OPTIONAL**

> 本文记录的是 Project Bulk Import Dry-Run 能力，不是 Production Package Bulk Production Hardening。DEV-061B 单独记录于 `docs/DEV_061B_BULK_PRODUCTION_HARDENING.md`。

## 仓库检查结论

- 已有真实批量导入 API：`preview_shot_bulk_import` 与 `commit_shot_bulk_import`。
- preview 是只读预检；commit 使用现有事务路径，并且是 CREATE ONLY，不更新或删除已有 Shot。
- 未发现通用 Project / Episode / Scene / Character 导入 API，也没有通用 dry-run API。
- 现有 JSON/TSV Shot parser 已在后端；仓库没有可靠 CSV parser，因此本任务没有宣称或新增 CSV 支持。

## 本次实现

入口位于项目指挥中心，工作区流程为：

```text
选择 JSON / TSV/TXT
  → 读取文件并去除 UTF-8 BOM
  → 调用现有 Shot preview API
  → 显示摘要、记录概览、错误/警告/阻塞项
  → Dry-Run 预计变化
  → 用户二次确认
  → 调用现有 Shot commit API，或在阻塞时保持禁用
```

工作区不会在选文件或运行预检时写入项目，也不会调用 Queue、ComfyUI 或自动生成。执行中会禁用重复提交；失败状态不会显示成功提示。重新选择/清除文件会清理旧的解析、预检和执行状态。

因为当前 API 只覆盖 Shot，所以本任务不伪造 Episode、Scene、Character、Review 或引用关系导入；项目关系校验由现有 Shot backend preview 在其真实支持范围内完成。

## 验证

- `npm test -- --runInBand`：Vitest 3 不接受 Jest 的 `--runInBand` 参数，命令按仓库现有 runner 规则失败（未知参数）。
- `pnpm test`：96 个测试文件、377 个测试通过。
- `npm run build`：通过；保留既有 bundle size warning。
- `pnpm exec tsc --noEmit`：通过。
- `git diff --check`：通过。

新增测试覆盖：空状态、项目级入口、JSON/TSV 文件读取、BOM、摘要和记录概览、阻塞错误、后端 malformed/empty/unsupported 错误、重置、确认后单次正式导入、正式导入失败且无 false success，以及 dry-run readiness 映射。

## 回归边界

DEV-060 Production Package Workspace、Project Settings、既有 Studio 导航和 deep-link 解析代码未被改动；完整前端回归通过。当前项目采用 state-driven workspace 而非 URL router，因此没有新增独立 import URL deep link。

## 后续

本能力保持可选，不扩展为通用 Project / Episode / Scene / Character 导入系统。生产包批量生产稳定性由 DEV-061B 负责；DEV-062 在 DEV-061B 通过后进入最终发布门禁。
