# DEV-069 — Multi-Package Production Board V1

状态：实现已接入；最终回归/构建/真人 UAT 待完成

验收标记：`PENDING HUMAN UAT`

本文是 DEV-069 V1 的可执行验收契约。实现已接入当前工作树；最终回归、桌面构建和真人 UAT 尚待主线程完成。本文件不声明这些门禁已通过。

## 1. Scope / Non-goals

### Scope

- 发现一个用户选择的根目录下的多个 `production-package.json`，逐包执行现有 Production Package inspection。
- 以一个 board 展示各包的 inspection 状态，并聚合已有 `ProductionBatch` / `ProductionQueue` truth。
- 仅在用户点击后，按确定顺序串行创建已选择的 package items；创建结果落入现有 ProductionBatch/ProductionQueue。
- 兼容单包工作区和现有 Queue/Monitor；board 不创造第二套 batch、task、asset 或 queue 数据。

### Non-goals（硬边界）

- 不自动 Start，不提供或实现 `Start All`。
- 不实现 auto-next、自动 retry、scheduler、后台轮询生产或新的 executor。
- 不直接调用 ComfyUI `POST /prompt`，不绕过现有唯一 Start 闸门。
- 不修改现有 Queue 的执行、retry、resume 或 task/asset lineage 语义。
- 不因文件夹移动、重命名或删除而删除数据库 provenance、ProductionBatch 或历史 item。

## 2. Discovery contract

### Root、深度和上限

- Root 由用户通过既有原生文件夹选择器提供；root 自身允许作为一个 package 目录（深度 `0`）。
- 递归发现深度为 `0..4`，即 root 与最多四层子目录；深度 `4` 的目录仍可作为 package。若深度 `4` 的非-package 目录仍需继续发现子目录，必须返回显式 `DISCOVERY_MAX_DEPTH_EXCEEDED`，不能静默忽略。
- 最多发现 `100` 个 package，最多访问 `5000` 个目录。超过任一上限必须返回显式错误，禁止静默截断或只展示前 N 个。
- 目录含 `production-package.json` 时即为 package 目录，并停止继续扫描该目录的子目录；manifest 无效仍要显示该 package 的错误，不把其子目录当作独立包。

### Canonical path、symlink boundary、visited

- 每个待访问目录先解析为 canonical absolute path；不能 canonicalize、无权限访问或路径不存在都产生 package/root 级显式错误。
- symlink、junction 或 reparse point 先 canonicalize；解析后越出用户选择的 canonical root 时不跟随、跳过其内容，不能以 lexical path 检查替代真实边界检查。
- 以 canonical directory identity 做 `visited` 集合，跳过重复别名与循环；重复访问不得重复计数或造成无限递归。
- 所有返回路径为 canonical absolute path 的受控展示值；创建请求只能引用 inspection 返回的 package/item identity，不能接受前端任意路径。

### Determinism、hash、错误

- package 按 canonical path 的自然排序返回；数字片段按数值比较，因此 `EP2 < EP10`，同名时以 canonical path 稳定打破平局。
- manifest 的 SHA-256 必须读取 manifest 的真实 UTF-8 bytes 计算；禁止使用 mtime、文件大小或伪 hash 代替。
- 错误必须包含稳定 code、scope（root/package/item）、可读 message 和必要的相对字段；至少覆盖 root 无效、目录访问失败、manifest 无效、`DISCOVERY_MAX_DEPTH_EXCEEDED`、目录上限、package 上限和 item 上限。symlink/junction 越界按跳过处理，不伪造错误。错误不应泄露完整 prompt 或媒体内容。
- 单包仍遵循 DEV-059 的 schema、媒体、路径和最多 500 items 约束；board 总 item cap 为 `10000`，超过时显式阻止创建。

## 3. Durable provenance

Migration 026 新增 `production_package_batch_bindings`，只承载 board 创建的可追溯关系；不改写旧 ProductionBatch/ProductionQueue 的执行模型。

### Package key

`package_key = SHA256(normalized_absolute_root + NUL + manifest_sha256)`。

其中 `normalized_absolute_root` 是实现 `normalize_package_root` 生成的稳定 absolute path 表示：按路径 components 规范化并使用 `\\` 分隔符；`manifest_sha256` 使用小写 hex。连接符是一个真实的 `NUL` byte，不是字符串 `"\\0"`。

因此 package 被移动/重命名，或 manifest bytes 发生变化，都会产生新的 package key。旧 key 与其既有 binding 仍保留。

### 最小持久化事实

Migration 026 的 `production_package_batch_bindings` 实际持久化以下字段，并带 `project_id` 隔离：

| Provenance | 内容 |
| --- | --- |
| source kind | 固定值 `PRODUCTION_PACKAGE` |
| package identity | `package_key`、外部 `package_id`、`package_name` |
| source root/proof | `package_root`、`manifest_sha256` |
| created lineage | `batch_id`、`chunk_index`、`chunk_count`、`package_item_ids_json` |
| audit timing | `created_at` |

数据库唯一性准确为 `project_id + package_key + batch_id`。Repository 写入路径另外按同一 `project_id + package_key` 读取既有 `package_item_ids_json`，对重复 package item ID 做保护；这不是额外的数据库 unique constraint。

- 源目录被移动、删除或不可读时，只影响后续 discovery/inspection；不删除 DB truth，不级联删除 batch、batch item、task 或 asset。
- 所有 binding、batch、chunk、package item ID 查询必须带当前 `project_id`；跨项目 package key、package item ID 或 batch ID 必须返回 not-found/forbidden 语义，不能被推断或复用。
- 删除 project 时遵循现有 project-owned foreign-key cascade 规则；删除 source directory 不触发 cascade。Migration 026 的 down/恢复操作必须保留可恢复备份，不得用“重新扫描目录”重建历史 lineage。

## 4. Runtime flow and creation semantics

正式生成链保持：

```text
ProductionPackage → ProductionQueue → GenerationService → WorkflowCompiler → ComfyUI
```

board 是 discovery/inspection/provenance/aggregation 的 UI 与协调层，没有 executor；它不直接调用 `GenerationService`、WorkflowCompiler 或 ComfyUI。

用户点击创建后：

1. 冻结本次选择的 package key、manifest SHA-256、source item IDs 和 inspection identity，并在服务端重新验证。
2. 按自然排序的 package 顺序串行创建；package 内沿用现有 item 顺序与下游 chunk 上限，chunk 也串行提交。
3. 首个失败即停止后续未开始的 package/chunk，并返回已成功创建的 lineage 与第一个失败的稳定 error code。
4. 不做跨 package rollback；已经写入的 binding/batch 保留并在 board 中显示。单次数据库 transaction 只保护其自身的 binding 与 batch/item 写入。
5. Resume 只处理未绑定 item；Repository 发现同一 `project_id + package_key` 的 `package_item_ids_json` 已包含该 item ID 时必须跳过，不可重复创建；若选择项全部已绑定，则返回明确的 all-items-already-bound 错误。
6. 创建结束后不自动打开 Queue、不自动 Start；用户可手动打开既有 Queue，并在目标 batch 上点击唯一的 Start。

## 5. UI requirements and state contract

### Selection、filters、caps

- board package 行显示实现中的 `READY`、`WARNING`、`BLOCKED`、`NOT_CREATED`、`UPDATED`、`CREATING`、`CREATE_FAILED`、`CREATED`、`RUNNING`、`COMPLETED`、`COMPLETED_WITH_FAILURE`；底层 item 的 succeeded/failed/pending 作为现有 batch 聚合计数展示。
- `READY` 默认 checked；`WARNING` 默认 unchecked 但可由用户逐项选择；`BLOCKED` disabled 且必须展示 blocker；已 `CREATED` 默认不重复勾选。
- 支持当前实现的“全部/未创建/问题/已创建/运行中/已完成”筛选；筛选不能丢失选择。
- hard cap 为 `100 packages / 10000 items`。cap 违反时显示总数和稳定错误，禁止用分页掩盖超限。

### Board states and manual actions

| State | 语义与允许动作 |
| --- | --- |
| `EMPTY` | 尚未选择 root 或 root 无 manifest；显示选择器和明确空态，不调用创建。 |
| `DISCOVERING` | 扫描 root；显示已访问目录/package 计数，禁止重复扫描和创建。 |
| `INSPECTING` | 逐包 inspection；显示当前包与进度，禁止创建。 |
| `READY` | 至少一个 READY item 可选；用户可刷新、检查详情、选择项、点击串行创建。 |
| `WARNING` | 有 warning 或混合结果；warning 未经人工选择不得创建，READY 仍可按选择创建。 |
| `BLOCKED` | 存在不可生产 package/item 或 root error；blocked 项不可选，必要时只允许修复后重新发现。 |
| `CREATING` | 串行创建进行中；锁定重复提交，显示当前 package/chunk、已创建数、失败数。 |
| `CREATED` | 已有 binding/batch；显示 batch IDs 与现有聚合进度，以及“打开生产队列”手动动作；chunk/package item 详情留在 provenance read model。 |
| `RUNNING` / `COMPLETED` / `COMPLETED_WITH_FAILURE` | 从现有 ProductionBatch/ProductionQueue 聚合；显示真实计数、失败原因和“打开队列/手动 retry”入口（retry 仍由既有 Queue 执行）。 |
| `ERROR` | 请求失败或 stale inspection；保留可用历史摘要，显示稳定 code，并提供“重新发现/重新检查”，不自动重试或重复创建。 |

单个 package 的 summary 至少包含 name、root/relative path、item total、READY/WARNING/BLOCKED、已绑定 batch 数、batch IDs 和现有 queue 聚合状态；chunk/package item 详情由 provenance read model 提供，不以路径输入替代选择器。

### Empty、progress、summary、error

- Empty 必须区分“未选择 root”“扫描后无 package”“package 全部 blocked”，不得都显示为 0/0 成功。
- Progress 显示 discovery 与 inspection 的当前阶段、计数和取消/重新发现语义；卸载页面不得留下异步状态写入。
- Summary 同时展示 package total、item total、可选数、warning、blocked、已绑定、running、completed、failed；计数来自本次 snapshot 与数据库聚合，不由前端猜测。
- Error 必须保留已创建成功的结果；“部分创建”明确列出成功 package/batch 与 first failure，并提供 resume，不提供 rollback 按钮伪装成可逆操作。

### Refresh timer

- 全 board 最多一个 board-level timer（默认 5 秒），不得按 package/item 建立 timer。父层仅在 `multi-package` tab 且存在 `CREATED` 或 `RUNNING` package 时启用它。
- 页面不可见时跳过 refresh；in-flight guard 防止请求重叠或排队。实现不承诺 visible 时立即 refresh，也不由 timer 内部宣称 terminal 自动停止。
- root、project、board snapshot 切换和 unmount 都必须清理 timer、取消/忽略过期响应；不会因旧 package 响应覆盖新 board。
- board 自身不因 terminal state 自动停止 timer；是否启用由父层的 `multi-package` tab 与 `CREATED`/`RUNNING` 条件控制。刷新只读取现有 ProductionBatch/ProductionQueue，不触发 Start、retry 或 ComfyUI 请求。

### Five-package fixture（不提交 demo fixture）

以下只是验收测试内存 fixture 的期望形状，不应作为 demo 数据提交：

| Package | 初始 board 状态 | item 示例 | 用户动作/后续状态 |
| --- | --- | --- | --- |
| `EP1` | `READY` | 2 × `READY`（默认 checked） | 可选并等待创建 |
| `EP2` | `WARNING` | 1 × `READY`、1 × `WARNING`（warning 默认 unchecked） | 人工勾选 warning 后才可纳入 |
| `EP3` | `BLOCKED` | 1 × `BLOCKED` | disabled，不能创建 |
| `EP4` | `RUNNING` | 真实 batch 聚合：`RUNNING`、`COMPLETED` | timer 期间反映真实 queue |
| `EP5` | `COMPLETED` | 真实 batch 聚合：全部完成 | 展示既有 batch；有失败时使用 `COMPLETED_WITH_FAILURE` |

Fixture 还必须验证 EP4/EP5 的 batch、chunk 与 package item IDs 可追溯，EP3 不产生 binding；`CREATED`、`CREATE_FAILED` 和 `COMPLETED_WITH_FAILURE` 由当前 board 的创建/错误及既有 batch 聚合用例覆盖；fixture 文件不得进入仓库。

## 6. Test matrix

以下是当前仓库已有 target/filter，最终回归由主线程执行；本节只规定应观察的断言，不记录当前执行结果。

| Area | Targeted command | Minimum assertions |
| --- | --- | --- |
| Rust discovery | `cargo test --manifest-path src-tauri/Cargo.toml --test dev069_production_package_discovery -- --test-threads=1` | root depth 0、manifest stop、canonical 后越界 symlink 跳过、100 package/5000 directory caps、自然排序 `EP2 < EP10`、真实 manifest SHA-256、稳定 `DISCOVERY_*` errors；depth 4 package 发现与继续向下的 `DISCOVERY_MAX_DEPTH_EXCEEDED` 需作为最终验收断言。 |
| Rust package/provenance | `cargo test --manifest-path src-tauri/Cargo.toml --test dev059_production_package -- --test-threads=1` and `cargo test --manifest-path src-tauri/Cargo.toml provenance_key_changes_with_manifest_or_normalized_root -- --test-threads=1` | 既有 package inspection/mapping/chunk contract 不回归；key 随 manifest/root 变化，Migration 026 实际字段与 `PRODUCTION_PACKAGE` source kind 可读。 |
| Rust partial creation | `cargo test --manifest-path src-tauri/Cargo.toml --test dev061b_production_package_hardening -- --test-threads=1` | 500 item chunk、后续 chunk 失败的 partial truth、已创建结果不 rollback、session 不隐式重试；创建仍不 Start 或提交 Comfy。 |
| Frontend board | `pnpm test -- MultiPackageProductionBoard` | 五包 summary、READY 默认选择、WARNING unchecked、BLOCKED/CREATED disabled、问题/运行中筛选、100/10000 caps、按顺序只调用一次 create、失败不隐式 retry、进度与 blocked actions。 |
| Frontend parent/queue compatibility | `pnpm test -- ShotWorkspace.production` and `pnpm test -- ShotWorkspace` | multi-package tab 接入、package discovery/inspection/create 串行协调、既有 Queue 手动 Start 边界、无 Start All/board executor；DEV-068 Queue/Monitor 回归保持可测。 |
| Existing package/queue UI | `pnpm test -- ProductionPackageWorkspace ProductionQueueDrawer` | 单包工作区、Queue Drawer 和现有手动生产路径不回归。 |
| Timer lifecycle | `pnpm test -- MultiPackageProductionBoard` and `pnpm test -- ShotWorkspace.production` | board 默认 5 秒、hidden 跳过、in-flight guard、卸载清理；父层只在 `multi-package` tab 且有 `CREATED`/`RUNNING` 时传入 polling enabled。 |
| Migration compatibility | `cargo test --manifest-path src-tauri/Cargo.toml --test dev055_release_compatibility dev055_migration_matrix_reaches_026 -- --test-threads=1` | 025 → 026 可升级；旧 project/batch/queue 数据可读；026 新表可启动；binding FK/unique/cascade 规则正确；无旧表语义破坏。 |
| Rust format/check | `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo check --manifest-path src-tauri/Cargo.toml` | format 与 type/check 无错误。 |
| Type/build | `pnpm exec tsc --noEmit` and `pnpm build` | frontend typecheck/build 成功，board wire fields 与 Tauri command 类型一致。 |
| Full regression | `cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1` and `pnpm test` | 既有 Rust/Frontend 全量测试无回归；结果必须记录真实 passed/failed/ignored 数。 |
| Desktop build | `pnpm tauri build` | 仅验证当前 0.8.1 desktop artifact 可构建；不因此创建 release/tag。 |
| Diff gate | `git diff --check -- docs/DEV_069_MULTI_PACKAGE_PRODUCTION_BOARD.md` | 文档无 whitespace/error；提交前确认本次 revision 只触及本文件，其他 agent 的工作树改动保留。 |

无 ComfyUI/GPU 的 discovery/provenance/creation safety 测试不得写成真实生成 PASS。若另行执行真人生成，只能通过既有手动 Queue Start 路径并单独记录环境与证据。

## 7. Human UAT checklist

状态标记：`PENDING HUMAN UAT`

### Prerequisites

- Windows desktop build 基于产品版本 `0.8.1`，数据库可从 Migration 025 升级到 026。
- 已准备一个测试 root，包含 5 个 package（含 nested depth、自然排序、一个 symlink/junction boundary、一个重复 canonical alias）和总量在 caps 内的真实 `production-package.json`；不得把该 fixture 提交到仓库。
- 已准备可验证的既有 batch truth：至少一个 CREATED、一个 RUNNING、一个 COMPLETED、一个 FAILED item/batch；若要验证真实生产，ComfyUI endpoint、workflow、模型和 GPU 必须由人工确认可用。
- DEV-068 的真实 Production Monitor/Queue UAT 可作为 queue/monitor 基线前置条件，参考 [DEV_068_PRODUCTION_MONITOR_DELIVERABLES.md](</C:/Users/ADMIN/Documents/ChatGPT/AI%20Studio/docs/DEV_068_PRODUCTION_MONITOR_DELIVERABLES.md>)；它不是 DEV-069 board 的通过证明。

### Steps and evidence placeholders

- [ ] 选择 root：root 自身 package、depth 0..4、manifest stop、自然排序 `EP2 < EP10` 正确。
- [ ] 确认 package/item 摘要、真实 manifest SHA-256、READY 默认 checked、WARNING unchecked、BLOCKED disabled。
- [ ] 验证 filters、100 packages/10000 items cap、空态、进度态、错误态和 partial summary 文案。
- [ ] 点击创建：观察 package/chunk 串行顺序、首次失败停止后续、已成功 batch 保留、无自动打开/Start/Start All。
- [ ] Resume：确认已绑定 item 被跳过且不重复创建，失败项可进入既有 Queue 的人工处理。
- [ ] 在既有 Queue 手动点击一个 batch 的 Start；确认 board 只聚合 CREATED/RUNNING/COMPLETED/COMPLETED_WITH_FAILURE，不产生第二 executor 或直接 Comfy POST。
- [ ] 删除/移动 source directory 或修改 manifest 后重新发现；确认出现新 key/新 inspection，旧 DB binding/batch/item truth 仍可读。
- [ ] 切换 project/root、隐藏窗口、恢复窗口、离开页面；确认只有一个 timer、无重复请求、无旧响应污染。

证据占位（完成真人 UAT 后填写，当前不得填 PASS）：

```text
UAT_STATUS = PENDING HUMAN UAT
BUILD_ARTIFACT = [待填写：0.8.1 artifact/path]
DB_MIGRATION = [待填写：实际 migration evidence]
DISCOVERY_EVIDENCE = [待填写：截图/日志/fixture 摘要]
PROVENANCE_EVIDENCE = [待填写：project/package/batch/chunk/package item IDs]
SERIAL_CREATE_EVIDENCE = [待填写：顺序与 first-failure 记录]
QUEUE_EVIDENCE = [待填写：既有 Queue 手动 Start 记录]
TIMER_EVIDENCE = [待填写：visibility/in-flight/unmount 记录]
COMFYUI_LIVE = [NOT RUN / 待填写：若人工执行，注明不是 board executor]
```

## 8. Risks, rollback and observability

- 风险：大目录扫描、权限变化、symlink/junction、manifest 变化、跨项目 ID 误用、部分创建和旧 batch 聚合不一致。所有风险必须落为稳定 error/state；不能以“跳过”隐藏。
- 回滚：若发现 board 回归，先禁用 board 入口/调用，继续使用既有单包/Queue；保留 Migration 026 binding、已创建 batch/item、task/asset 和 manifest proof。V1 不提供删除 provenance 的 destructive rollback；修复采用 forward migration/代码修复，数据库恢复只按受控备份流程执行。
- 可观测性：记录结构化 event（project ID、package key、source kind、manifest hash、package/chunk/package item/batch IDs、阶段、计数、duration、error code、first failure）；日志不得写完整 prompt、媒体 bytes、secret 或不必要的绝对路径。必要时对 path 做受控脱敏，DB provenance 仍保存 canonical path。
- 诊断需能区分 `DISCOVERY`、`INSPECTION`、`CREATING`、`AGGREGATING` 与 `QUEUE`；尤其要能证明 board 没有调用 Start、retry、scheduler 或 Comfy `/prompt`。

## 9. Release guard

- Product/package/Cargo 版本保持 `0.8.1`。
- DEV-069 不创建 `v0.8.2`、`v0.9.0`、任何 release、tag 或 push。
- 若未来在 push 前运行 CI，必须是 source-only CI，并记录真实 URL/SHA/status；本文件不预填 CI、build 或 UAT 结果。
- `pnpm tauri build` 只是本地验证命令，不构成 release 证据。
