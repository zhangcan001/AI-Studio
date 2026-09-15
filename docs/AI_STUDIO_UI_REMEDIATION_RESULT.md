# AI Studio UI 整改落地结果

> 执行范围：`AI_STUDIO_UI_REMEDIATION_PLAN.md` 的 P0、P1、P2 发布门槛；不改变 Queue、Task、Generation、Provenance 或归档数据语义。

## 结论

```text
P0=COMPLETE
P1=COMPLETE
P2=COMPLETE
P3=OPTIONAL_NOT_BLOCKING
QUEUE_AUTHORITY_PRESERVED=YES
PRODUCTION_SEMANTICS_CHANGED=NO
```

## P0 — 错误与文案地基

- 统一复用 `UiErrorNotice` 与 `formatUiError`，保留用户可读信息、错误代码和可展开的技术详情。
- 补齐 ComfyUI 超时、能力刷新失败、离线、缺节点、工作流格式和归档恢复相关中文映射。
- 队列“连续运行已暂停”现在展示人话原因、下一步提示、`重试启动`、`打开连接设置`和技术详情；不会自动提交新任务。
- 工作流导入区分普通 UI JSON、非 API 格式、无效 JSON 和未知格式；导入前明确 API 格式要求。
- Prompt Studio、Local Tool Hub、资产预览、任务详情和审核工作区区分加载失败、未绑定、未知、未记录和历史未保存。
- 主路径工作区完成一轮中文优先标签清理；ComfyUI、H3 等专有产品名保留。

## P1 — 主路径与反馈

- Production 页面保留“单次生成 / 生产包 / 批量生产 / 项目生产”并列入口；单次生成仍只准备队列数据，只有“开始生产”执行。
- 生产工作区常驻显示 ComfyUI 连接状态和节点能力缓存，并提供“刷新节点能力”。
- 工作流导入前展示 `Export API` 说明，明确支持子图节点 ID（如 `105:11`），拒绝首尾冒号、空段和字母。
- 项目归档恢复结果展示资产、版本、生成、工具溯源、资产版本溯源、缺失文件及未解析 ID；未知项可见但不被伪装为成功绑定。
- 空态、加载态、错误态均保留语义边界；旧数据缺少历史溯源时展示“尚未记录/未建立显式关系”，不猜测关系。

## P2 — 视觉和信息密度

- 统一 action/state token：主操作、安静操作、危险操作、运行中、暂停、失败、成功。
- 收敛工作区面板、列表、筛选器、状态徽章、错误卡片、空态和焦点环样式。
- 全局导航 rail 保持固定，生产队列保持底部状态条形态；结构管理默认收起，高级操作保留在工作区内。
- 补齐 `aria-busy`、`role="alert"`、`role="status"`、`aria-live` 和减少动态效果等基础可用性反馈。

## P3 — 非阻塞项

以下不作为本轮发布阻塞项，后续可单独安排：

- 全量键盘快捷键设计和首次导入一次性引导。
- 关键页面截图级视觉回归基线。
- 更细粒度的多语言资源抽取和搜索增强。

## 验收与验证

- 组件回归覆盖：工作流导入、队列暂停、单次生成、Prompt Studio、Tool Hub、资产使用、归档恢复、审核状态和镜头就绪度。
- 需要在本轮提交前执行并记录：

```text
pnpm test
pnpm exec tsc --noEmit
pnpm build
git diff --check
```

- 本轮只修改前端、样式、测试和本结果文档；工作区中原有的 Rust 修改不属于本整改，不纳入提交。

## 明确不做

- 不新增 Queue、Executor、Task、Generation 或 Result 系统。
- 不改变 Queue Start 唯一执行门禁。
- 不自动猜测 Prompt、Model、Tool、Asset 或 Provenance 关系。
- 不引入云同步、多用户、SaaS 或 AI Agent 能力。
