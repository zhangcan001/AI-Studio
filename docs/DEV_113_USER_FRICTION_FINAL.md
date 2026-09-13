# DEV-113 — Final User Friction Register

This register records the friction observed during the first-time producer journey. It intentionally does not turn P2/P3 observations into implementation work during the release-candidate audit.

ID=F-113-01
AREA=APP_ENTRY
USER_PROBLEM=打开应用后不知道 AI Studio 的第一步是创建项目、导入外部生产交接，还是进入手动创作。
CURRENT_BEHAVIOR=启动页只说明本地环境准备；准备完成后全局 rail 提供项目、创作、资产、生产、审核等入口，但没有 production-first 起步提示。
SEVERITY=P2
RECOMMENDATION=增加简短的首屏起步说明或首个项目的下一步提示，不改变现有导航和执行模型。
ENTER_1_3_RELEASE=NO

ID=F-113-02
AREA=PROJECT_CREATION
USER_PROBLEM=创建项目后不确定是否应先配置工作流、导入 Handoff，还是手动创建镜头。
CURRENT_BEHAVIOR=项目表单只有名称和说明；创建成功后打开项目/创作工作区，项目说明文字没有生产路径建议。
SEVERITY=P2
RECOMMENDATION=在创建成功后的项目空状态提供手动创作、Handoff 导入和工作流检查的顺序提示。
ENTER_1_3_RELEASE=NO

ID=F-113-03
AREA=HANDOFF
USER_PROBLEM=第一次使用时难以判断“批量导入预检”是否就是外部 Agent 生产交接入口。
CURRENT_BEHAVIOR=Command Center 先显示“批量导入预检”，进入后才看到“导入外部生产数据”和 Handoff schemaVersion 1。
SEVERITY=P2
RECOMMENDATION=保留现有 dry-run/confirm 安全链路，只改善入口名称、短说明和成功后的下一步提示。
ENTER_1_3_RELEASE=NO

ID=F-113-04
AREA=PROJECT_STRUCTURE
USER_PROBLEM=导入后需要补建 Series/Episode/Scene 时，不容易发现层级创建入口。
CURRENT_BEHAVIOR=结构树右上角的紧凑“新建”菜单提供新建系列/集/场景/镜头；更完整的排序、重命名、归档在二级结构管理面板。
SEVERITY=P2
RECOMMENDATION=在空结构或未归档镜头状态旁显示一次性的创建层级提示，继续复用现有结构管理。
ENTER_1_3_RELEASE=NO

ID=F-113-05
AREA=PREPARATION
USER_PROBLEM=不知道生产模式默认的“生产包”页与项目镜头准备页的关系。
CURRENT_BEHAVIOR=生产模式包含“生产包 / 批量生产包 / 项目生产”标签；项目准备入口位于项目生产面板，生产包页默认可见。
SEVERITY=P2
RECOMMENDATION=在项目生产 tab 上增加面向新用户的“从项目镜头准备”短说明，不改变 tab 或数据模型。
ENTER_1_3_RELEASE=NO

ID=F-113-06
AREA=QUEUE
USER_PROBLEM=离开准备页后不确定在哪里找到唯一的真实开始动作。
CURRENT_BEHAVIOR=准备和生产包创建后提供打开队列；Queue 同时以生产 rail、Runbook 和底部 drawer 出现，开始按钮在队列内。
SEVERITY=P2
RECOMMENDATION=强化现有 Queue CTA 与“只有这里的开始生产才会提交任务”的邻接关系。
ENTER_1_3_RELEASE=NO

ID=F-113-07
AREA=REVIEW
USER_PROBLEM=Command Center 的待审核摘要不等于可执行的完整审片工作区，第一次使用时可能以为摘要就是审核入口。
CURRENT_BEHAVIOR=Review Inbox summary 提供计数、选中结果和 View All；完整审核/返工动作位于 review workspace 和 batch review surface。
SEVERITY=P2
RECOMMENDATION=在摘要卡上明确“只读摘要 / 打开完整审核”的差异，并保留现有 exact target 导航。
ENTER_1_3_RELEASE=NO

ID=F-113-08
AREA=FINAL_RESULT
USER_PROBLEM=完成生产后，接受结果、对应 Shot、对应 Asset 和文件位置不一定能在一个界面内同时确认。
CURRENT_BEHAVIOR=Review 可打开精确 Asset/Shot/Task；Asset preview 提供使用情况；Monitor 在数据库存在 local path 时提供打开文件位置/成品文件夹，但没有统一的 final delivery surface。
SEVERITY=P2
RECOMMENDATION=把现有精确链接和文件位置能力收敛为一条结果确认路径，不新增第二个生产执行入口。
ENTER_1_3_RELEASE=NO

ID=F-113-09
AREA=LARGE_PROJECT
USER_PROBLEM=500 个镜头的异常定位虽然可用，但用户仍需理解 View All、过滤条件、分页和结构树之间的分工。
CURRENT_BEHAVIOR=Command Center 只显示前 20 项；大集合进入完整项目列表，生产监控和生产包使用分页/筛选，结构树对当前场景和未归档镜头保持有界显示。
SEVERITY=P2
RECOMMENDATION=保持 bounded 设计，在各入口统一“当前显示范围 / 完整集合 / 过滤结果”的解释。
ENTER_1_3_RELEASE=NO

ID=F-113-10
AREA=TERMINOLOGY
USER_PROBLEM=Production Package、Asset Usage、Parse / Validate、Preview 和 WorkflowVersion/Recipe 等中英混合术语增加理解成本。
CURRENT_BEHAVIOR=核心生产状态已统一为待启动、运行中、失败，需要处理、待审核；部分导入、资产和诊断细节仍保留技术术语或 ID。
SEVERITY=P3
RECOMMENDATION=在不移除技术诊断的情况下，为面向生产者的主文案增加中文释义。
ENTER_1_3_RELEASE=NO

ID=F-113-11
AREA=DELIVERY_PRESENTATION
USER_PROBLEM=结果已存在时，首要操作仍可能显示为查看生成任务、查看素材或打开文件位置，用户需要自己拼接交付上下文。
CURRENT_BEHAVIOR=系统保留 exact Asset/Task/Shot 目标和失败历史，成功结果可分别从 Monitor、Review、Asset 使用情况继续打开。
SEVERITY=P3
RECOMMENDATION=统一成功结果卡片的主次动作和交付摘要，不改变历史记录或项目边界。
ENTER_1_3_RELEASE=NO

## Summary

```text
P0=0
P1=0
P2=9
P3=2
ENTER_1_3_RELEASE=NO_FOR_ALL_REGISTERED_FRICTION
```

None of these findings is a release blocker. The safe production gate, exact target navigation, bounded large-project behavior, and review/rework continuation are stronger than the remaining wayfinding friction.
