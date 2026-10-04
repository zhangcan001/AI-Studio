# 2.1 M1-1 — Create readiness action routing

基线：`master@3feefee642eb88c496bf9bcb603e95c4bb1eb03d`。稳定发布仍是 r4；不 bump 产品版本，不发布。

## 契约与边界

| typed action | Create 行为 |
| --- | --- |
| EDIT_INPUT + 有效 field | 修改此输入；展开已有高级参数区域并聚焦控件 |
| EDIT_INPUT + 缺失/未知/不可聚焦 field | 聚焦有稳定 ID 的创作输入区域，保留错误 |
| SELECT_GENERATOR | 聚焦生成器选择，不自动切换 |
| OPEN_RUNTIME_SETTINGS | canonical system-settings/general，returnTo 精确当前 Create |
| TRY_LATER | readinessGet 只读重查，不提交、不创建/启动队列、不重试生产 |
| OPEN_PROJECTS | canonical project-list，仍遵守现有离开确认 |
| 未知/本 checkpoint 不处理的 action | 稳定说明，不猜测、不产生无效按钮；OPEN_RUN 原有入口保留 |

GenerationServiceError 的 Comfy/ExecutionFailed runtime 分支使用 OPEN_RUNTIME_SETTINGS；已有 unavailable-definition 特例保留 SELECT_GENERATOR；admission busy 仍是 TRY_LATER。RUNTIME_BLOCKED code 不变，不解析错误消息。

Settings 返回复用 `creationLabReturn` 与 StudioStore 值/dirty：保存 selectionRef、runRef、accepted。App 离开确认只对已保存 scope 与当前项目/镜头/阶段/surface 全部一致的 general/advanced-workflows 返回路径豁免，不扩大到其他项目或目的地。

自动与显式 readiness 检查共享读取路径。owner epoch、request epoch、当前 draft identity 和卸载保护拒绝迟到结果；生成时失效旧只读检查。显式重查固定 readiness-only key，不生成生产 attempt。accepted 不等于 succeeded，启动失败的 RunRef/详情入口保持。

## 冻结 successor

不修改 Phase8/10/12/13 历史 manifest。当前 `m1-1-readiness.json` 只记录本 checkpoint 的合法差异；guard 验证 immutable parent、精确路径集合、新增文件集合、各域 before/after/untouched aggregate、全部 authority invariants。验证 live bytes 后才为历史验证器提供 parent projection；无效 review 不投影。新增 guard 的负向测试覆盖篡改 parent、范围、hash、文件数、untouched aggregate 和 authority。

没有 CSS 重写、新 store/router、SQL production write、migration、IPC command、Queue/Task/工作流/OCC 改动。Backup v20、migration42 保持。

## 验证策略

针对性 Create 测试：真实字段、生成器、Settings return 的提示词/媒体/参数/dirty/accepted/RunRef、只读重查、缺失/未知字段、未知 action、项目切换成功/失败响应竞态。

Rust：typed runtime 与 field action 单测、现有 creation submission integration；owned fixture 的 RUNNING batch 验证 admission busy 为 TRY_LATER 且无生产写入。首次 fixture 误用 Item 的 PENDING 状态导致失败，已修为正式 Batch 的 READY，未改业务枚举或弱化断言。

Static：TypeScript、frontend build、Rust fmt/all-targets check；现有 architecture、style/backend/frontend successor 与 Phase12/13 guards。Native 使用隔离数据，不修改用户 ComfyUI、不生成媒体；未实际覆盖的场景必须单独标 NOT VERIFIED，不能由 contract tests 冒充 Native。

最终以本 checkpoint exact-head Source-only CI 为完整 Gate，不复用 r4 CI。完成后停止，不自动启动 M1-2。

## 已完成本地与 Native 证据

- Create 21 项测试及 M1 successor/精确离开豁免 2 项通过；style/backend/frontend successor、Phase12 successor 合计 21 项通过；architecture command、TypeScript、frontend build、Rust fmt/all-targets check 通过。
- Rust typed action 单测 1 项、creation submission integration 2 项通过。前述 fixture 状态错误已定因修正后复验。
- 一次隔离 development Native session，使用本次编译的 debug 程序与当前 Vite 源码；正常关闭并停止本任务拥有的只读 loopback helper，未关闭用户程序。
- 必需提示词为空 → 点击“修改此输入” → UIA 确认提示词获得焦点。
- 输入提示词与宽度；owned mock Comfy 端点仅提供本机真实 schema/system_stats 的只读副本，拒绝 POST。通过正式 Settings 应用到隔离配置，随后仅让 helper 返回 503；没有删除真实模型或节点。
- 仅在隔离 DB 插入一条 RUNNING batch fixture，不启动执行。点击“重新检查”仍显示忙碌；把 fixture 改为 COMPLETED 后再点击同一按钮显示“可以生成”，证明不是静态 no-op。warm schema cache 不保证每次重查都发 HTTP，不能用 HTTP 数量代替 readiness IPC 次数。
- helper schema cache 到期后的失败产生“检查运行环境”；点击进入 System Settings general，不出现放弃草稿确认；点击“返回”回到原镜头/图片阶段，提示词、宽度 1040、高度 1280、生成器仍一致。
- 仅把隔离工作流 library_state 临时设 REMOVED；缓存的 Create 选择遇到真实 availability 检查后显示“重新选择生成器”，点击后 UIA 确认选择器获得焦点、值未自动切换。随后恢复 fixture 状态。
- Native 结束时 tasks=0、batch items=0、非 fixture batches=0、helper POST=0。一条 batch 来自明确 fixture 写入，不是 readiness 动作创建。
- Native 未制造缺模型/缺节点；媒体输入、accepted/RunRef、dirty 精确值保留由上述 frontend regression 证明，本轮 Native 没有伪造 accepted 或新生成。
- 隔离数据库 max migration=42；按 pool.rs 正式领域表名单计数=71（不能与 SQLite 全部内部/辅助表计数混淆）；043 不存在，Backup v20 不变。

本地全量 suite 未重复运行，按本任务政策由提交后的 exact-head CI 承担完整 Gate；最终 CI 状态以聊天报告为准。
