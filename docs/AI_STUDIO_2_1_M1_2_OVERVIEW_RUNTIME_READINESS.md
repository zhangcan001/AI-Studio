# M1-2 — Overview 运行准备

## 范围与契约

仅解释 Product Facade 的 `ProjectOverview.runtimeReadiness` cached facts。ProjectCommandCenterService 继续是统计/readiness authority；`overviewAction()` 继续翻译后端 nextAction，前端不重新计算项目优先级。

- 独立展示连接、运行预检、生产工作流、运行资源。
- `status=null` 时 workflow 的 0 和资源的 false/0 是占位值，一律显示“未检查”；CONNECTED 不代表已预检。
- READY/ WARNING/ BLOCKED 分别显示“已通过/有警告/已阻断”。未知字符串不猜在线或 READY。
- known preflight 下才展示 workflow 可用计数及资源忙碌/空闲；任一 busy flag 或活动任务为正即显示忙碌。
- 需关注时提供 secondary “检查运行环境”，canonical System Settings general，returnTo 是当前项目 Overview。
- 空项目离线仍以“创建第一个镜头”为 primary；COMFY_BLOCKED primary 与 secondary destination 一致。
- Overview 不执行预检，不加刷新假按钮、polling、IPC、Rust 服务或 store；真正预检仍由 Settings 提供。

## Freeze successor

父基线为 M1-1 `a51e1f697ab4dc5997e36ff080fddb48d9ebfe2b`。

新增 scoped `m1-2-overview-readiness.json` 与 guard，仅允许 Overview 页面、纯 presentation helper、focused/boundary tests、M1-1 guard 的最小 successor wiring。验证 before/after live hashes、完整文件集合与 untouched aggregates 后，向 M1-1 提供 parent projection。

M1-1 manifest、Phase8/10/12/13 历史 manifest 未改，未扩大 wildcard/忽略未验证文件。负向测试覆盖错误 parent、路径越界、before/after drift、untouched/file count、script/style aggregate 以及新增 authority/polling。

## 本地验证

- Overview focused 17 项、既有 v3 routing 6 项、M1-1 boundary 2 项、M1-2 boundary 1 项：26 passed。
- 既有 style/backend boundary 11 项、frontend consolidation/Phase12 successor 10 项：21 passed。
- TypeScript、`pnpm build`、统一 architecture guards：PASS；未重新 profiling。
- Rust 源码无修改，本地不重复完整 Rust suite；最终 exact-head Source-only CI 承担完整 Gate，结果以聊天报告为准。
- 每个 runner 前检查 RAM/VRAM，无并发 build/test，未终止用户应用。

## 隔离 Native

一个新 owned data root + WebView session，标准 `pnpm tauri dev` 启动当前源码。未改用户 ComfyUI。

1. 空项目 Overview 显示已连接 + 尚未预检；workflow/resources 均为未检查，而非 0/0 可用或空闲。
2. Overview secondary action 进入 Settings general；正式保存应用 task-owned loopback endpoint。Helper 仅 GET 本机真实 schema/stats 的只读副本，不向真实 Comfy 转发生成请求。
3. 仅令 helper 返回 503，在现有 Settings 执行“立即预检”。返回同一项目 Overview 显示离线/已阻断/0 of 5 可用；这是实际 cached preflight，不是缺失预检占位值。
4. 离线空项目 primary 仍可进入 Create，并实际创建一个 owned 镜头；没有触发生成。返回 Overview 后 COMFY_BLOCKED primary 仍进入相同 Settings route。
5. 恢复 helper，Settings 立即预检后返回 Overview，实际显示已连接/有警告/0 of 5 可用/空闲及 secondary settings action。未把 warning 说成失败；0/5 没有改写为 READY。
6. Owned DB 结束时 shots=1、tasks=0、production batches/items=0；helper POST=0。正常关闭任务窗口并停止 helper。

Native connection=null 未单独构造；未预检 UNKNOWN 及 CONNECTED_NO_PREFLIGHT 已实际观察。READY 和 INCOMPATIBLE 由 contract tests 验证，本次 Native 不伪造这两个状态。

## 未改变

Queue/task/workflow/binding OCC/draft authority、Rust/IPC、Settings layout、Create、产品版本、发布/tag 均未改变。Migration max=42，按 pool.rs 正式表名单计数=71，无 043，Backup v20，产品版本 2.0.0-personal。

本 checkpoint 完成后停止，M1-3/M2/M3/M4 均未开始。
