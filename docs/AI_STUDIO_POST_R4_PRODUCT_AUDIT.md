# AI Studio r4 发布后产品审计

日期：2026-10-04。任务性质：产品审计与规划，不是开发或重新验收 Architecture Reset。

## 1. Baseline 与方法

```text
PRODUCT_VERSION=2.0.0-personal
STABLE_RELEASE_BASELINE=r4
STABLE_TAG=v2.0.0-personal-r4
STABLE_HEAD=8a503f6ab45bedb2b7f272d1f755abe915d7b70a
STABLE_CI=37176528504_COMPLETED_SUCCESS
AUDIT_SOURCE_HEAD=8a503f6ab45bedb2b7f272d1f755abe915d7b70a
ORIGIN_MASTER_AT_AUDIT=8a503f6ab45bedb2b7f272d1f755abe915d7b70a
DELTA_FROM_R4=NONE
WORKTREE_START=CLEAN
```

发布权威：[r4 Release](https://github.com/zhangcan001/AI-Studio/releases/tag/v2.0.0-personal-r4)。
先执行 status、fetch tags、HEAD/origin 核对及 r4 差异检查。本文完成后的文档 commit 不改变审计所引用的源码；不可把文档 commit 当成已运行全部测试的源码 HEAD。

证据类型：

- **N：本轮 Native 观察**。使用 Phase14 已验收安装版，一次独立 TEMP 数据目录及独立 WebView 目录会话；启动时没有其他 AI Studio 进程。观察默认项目、空概览、创建镜头、Create、Runs、Library、Settings、Generators、Tool Hub、Workflow Lab、Prompt Studio，然后正常退出。只写本轮拥有的临时项目/镜头，不操作真实项目、不提交生成。
- **S：当前源码确认**。仅读取对应页面/controller/facade 的相关范围。TypeScript 使用 Serena overview，Rust LSP 不可用时用文件范围读取；未扫描全库。
- **H：复用 Phase14/15 已完成证据**。图片/视频历史预览、失败/暂停运行、复用、删除检查、归档恢复及隐私验收，不冒充本轮重测。
- **I：规划推断**。频率、痛感、预计规模及工作量不是遥测或真实用户研究；性能风险不是 benchmark 结果。

Native 来源限制：复用 Phase14 安装验收所用程序；本轮没有重新验证安装载荷与源码的逐字节对应。已安装程序与当前 loose `target/release` 程序的哈希不同，后者不能替代 NSIS 发布载荷的来源证明；差异原因未在本轮确认，不据此宣称发布损坏或二者等价。发布安装包来源与校验沿用 Phase14/15 的验收记录，当前代码判断以 S 证据为准。

本轮未跑全量 suite、build、profiling、GPU 生成、安装/卸载、备份恢复；源码没有变化，因此不重复这些 Gate。没有做“失败后 10 秒内理解”计时用户研究，也没有伪造缺节点/缺模型的 Native 故障。离线抽样使用设置页对不可连接的 loopback 测试地址执行只读连接测试；看到明确离线说明。**这不等于证明离线地址已保存为应用环境**，已保存环境与测试草稿必须区分。没有释放或重启用户的 ComfyUI。

## 2. 结论

在本轮范围内没有发现新的 P0 或 P1；不是对所有未测环境的无条件保证。r4 的主要问题是“已有能力分散、原因与下一步脱节”，不是 Queue/数据库/工作流引擎需要重写。

普通生成至少仍需要理解项目、镜头、图片/视频阶段、生成器/模式、提示词、输入素材、参数、准备状态与队列/结果，约 8–9 组概念（产品分析，不是统计）。Workflow/Recipe/Node 通常已藏在高级入口，生成器版本仍露出；普通用户最难的是分清“连接成功、包存在、生成器可用、本次输入通过”四件不同的事。

建议 2.1 只做四个有顺序的主题：运行准备与可操作阻断 → 日常 Create/Runs 恢复 → 模块找回与复用 → 保守的媒体检查。第一项先修 **Create readiness 的动作去向**，1–3 个逻辑提交，不做 setup framework。

## 3. 当前产品状态

| Area | 观察到的能力 | 缺口与边界 | 证据 |
| --- | --- | --- | --- |
| Project / Overview | 项目选择、空项目第一镜头 CTA、进度、运行与结果入口 | 首次显示运行环境“待检查”，没有就地 setup 步骤；默认项目名称有中英不一致 | N；`src/app/v3/ProjectOverviewPage.tsx`、`AppShellV3.tsx` |
| Create | 创建/选择镜头、阶段、生成器、提示词、参数、候选选用、显式长期参考 | readiness 动作不区分输入与环境；首次输入素材需要跳到 Library；近期选择器不等于完整查找 | N；`CreatePage.tsx`、`CreateInputs.tsx`、`CreateResults.tsx`、`CreateController.ts` |
| Runs | 状态筛选、进度、部分成功、能力驱动 Start/Pause/Retry/Edit、结果与高级诊断 | 普通列表不支持历史 cursor；恢复文字常是“检查输入/高级恢复”而不是具体去向 | N 空列表；S `RunDetail.tsx`、`run_facade/workspace.rs`；H 有历史运行 |
| Library | 分类/名称搜索、详情预览、版本、关系、复用和删除影响 | 列表是文字卡，收藏/标签在高级 Assets；近期摘要不是全量；预览失败无维护动作 | N 空分类；S `LibraryPage.tsx`、`LibraryDetail.tsx`；H 实际媒体/关系/删除检查 |
| Search | Library 媒体/提示词分页；高级 Assets 名称/原始文件名、收藏、标签；高级 Prompt 名称/标签/类型 | Runs 无自由文本/历史分页，Project/Shot 主要下拉选择；Create 近期素材 100 项、提示词 20 项 | S `library_facade/list.rs`、`AssetLibrary.tsx`、`PromptStudio.tsx`、`creation_facade/context.rs` |
| Prompt Studio | 列表、标签/类型搜索、版本/模型信息、项目近期任务和显式溯源 | 项目近期 20 个任务、显示其中 6 个，不是所选提示词全量使用次数；Create 提示词 picker 无搜索 | N 空提示词/模型；S `PromptStudio.tsx`、`CreateInputs.tsx` |
| Tool Hub | Tool/Instance/Version/Capabilities/已记录健康元数据可读 | 本轮 Comfy 已连接时工具登记仍为 0；刷新登记列表不等于发现或主动健康探测 | N；S `LocalToolHub.tsx` |
| Media maintenance | 预览失败保留关系与历史；现有删除/导入仍有保护 | 缺少明确 scan/verify/locate 产品流；不能把“预览失败”自动判定为文件丢失/校验失败 | S `LibraryDetail.tsx`；H 数据保护 |
| Workflow Lab | 导入、质量门、版本、映射、诊断、返回原页面和高级验证入口 | 技术密度高但符合 Advanced 定位；本轮仅浏览，不导入/发布/运行 Benchmark | N；S `WorkflowLabPage.tsx` |
| Diagnostics | 导出按钮靠前，50 项窗口/未知明确显示，失败可跳运行 | 默认展开大量遥测术语和耗时信息；技术时间线仍可按 ID 查询 | N 空健康；S `DiagnosticsExecutionPanel.tsx`；H export/privacy |
| Settings | 全局 Comfy 环境与项目生成器分层，binding OCC 冲突刷新而不自动覆盖 | System Settings 的环境配置在长诊断区域后；包“可用”容易被误解为本次可生成 | N；S `SettingsWorkspace.tsx`、`GeneratorSettingsPage.tsx` |
| First run | 默认项目和内置 5 个包；空概览/Create 都有下一步 | 不自带模型；连接、节点/模型要求、刷新与验证缺少连贯解释 | N；发布文档 |
| Navigation | 四主入口、项目上下文、返回/高级隔离均可访问 | 部分普通术语与旧组件词汇有不一致；无需新一级导航或第二套路由 | N；S `AppShellV3.tsx` |

这里“technically works 但不完整”的代表：环境检测能做却难找到下一步；媒体能管理却难视觉找回；失败能保留历史却不够指向具体修复；高级收藏/标签已存在却普通入口难发现。

## 4. Findings

频率为 **I：预计使用频率**，不是发生率。努力/风险为 1–5 相对等级；S/M/L 为工程量等级。13 项：P0=0、P1=0、P2=8、P3=5。F13 是明确的测量缺口/风险候选，**不是已经复现的卡顿**。

### F01 — 首次运行环境准备路径不连贯
- **AREA**：Overview / First Run / Settings；**PROBLEM**：概览“待检查”，连接/预检/节点要求分散在长设置页，包数量与运行准备容易混淆。
- **USER_IMPACT**：装好应用仍不知道如何完成第一张图；**FREQUENCY**：每次首次配置或环境变更，估计 4/5；**SEVERITY**：P2；**EVIDENCE**：N 空概览、设置顶部诊断后才出现 Comfy 配置；S `ProjectOverviewPage.tsx`、`SettingsWorkspace.tsx`。
- **ROOT_CAUSE_CLASS**：DISCOVERABILITY / UX；**RECOMMENDATION**：M1 连贯准备清单，区分连接/包/依赖/输入，用现有事实和路由；**EFFORT**：M (3)；**RISK**：2；**TARGET_VERSION**：2.1。

### F02 — Create 环境错误的“修改输入”缺少有效去向
- **AREA**：Create / Error UX；**PROBLEM**：`GenerateBar` 对所有 issue 只执行 `focus(issue.details.field)`；无 field 的环境 issue 没有设置页动作。TRY_LATER 只改变按钮文字，不改变动作。
- **USER_IMPACT**：看见阻断但点按钮没有帮助；**FREQUENCY**：离线/环境缺失或输入错误时，估计 4/5；**SEVERITY**：P2；**EVIDENCE**：S `CreateResults.tsx`、`product/errors.ts`。生成前 `CreateController.generate` 再检查 readiness，未发现绕过准入。
- **ROOT_CAUSE_CLASS**：ERROR_RECOVERY；**RECOMMENDATION**：第一个实施任务：按已知错误契约区分字段定位、检查环境、重选生成器、查看运行，未知原因不猜动作；**EFFORT**：S (2)；**RISK**：2；**TARGET_VERSION**：2.1 / M1 首个 checkpoint。

### F03 — Runs 恢复能力正确，但解释不够具体
- **AREA**：Runs / Error UX；**PROBLEM**：粗粒度 errorSummary、自动重试不可用和“高级恢复”说明不足以总是回答应改哪项；历史 input.errorMessage 还可能是技术文字。
- **USER_IMPACT**：用户需要阅读多处输入、状态、详情才决定改输入还是恢复；**FREQUENCY**：失败/部分成功后，估计 3/5；**SEVERITY**：P2；**EVIDENCE**：S `RunDetail.tsx`、`CreateResults.tsx`；H 失败/暂停路径已有验收，不重复执行 retry。
- **ROOT_CAUSE_CLASS**：ERROR_RECOVERY / UX；**RECOMMENDATION**：M2 为可识别原因提供一句原因和一个主动作，明确重试旧快照、新草稿新运行及旧结果保留；**EFFORT**：M (3)；**RISK**：3；**TARGET_VERSION**：2.1。

### F04 — 普通 Runs 找不到完整旧历史
- **AREA**：Runs / Search；**PROBLEM**：当前 coverage 明确是最近 50 任务、生产运行及未归档队列；cursor 被拒绝，普通页无自由文本搜索。
- **USER_IMPACT**：旧任务不在普通列表时需高级任务入口；不等于数据被删除；**FREQUENCY**：日常累计后找历史，估计 4/5；**SEVERITY**：P2；**EVIDENCE**：N coverage 提示；S `RunsPage.tsx`、`run_facade/workspace.rs`。
- **ROOT_CAUSE_CLASS**：MISSING_PRODUCT_CAPABILITY；**RECOMMENDATION**：M3 先给明确完整历史跳转与模块筛选；混合来源分页必须先设计稳定排序/归并契约，不简单扩 50 为无限；**EFFORT**：M–L (4)；**RISK**：3；**TARGET_VERSION**：2.1 分层交付。

### F05 — 普通 Library 难视觉找回，已有组织能力藏得深
- **AREA**：Library / Search；**PROBLEM**：卡片只显示名称、类型、时间，无缩略图；普通查询固定 favorite_only=false、tag_id=None，收藏/标签在高级 Assets。
- **USER_IMPACT**：面对自动命名的图/视频，靠文字选择低效；**FREQUENCY**：每天找结果/复用，估计 5/5；**SEVERITY**：P2；**EVIDENCE**：S `LibraryPage.tsx`、`library_facade/list.rs`、`AssetLibrary.tsx`；N 分类/近期摘要与高级入口。
- **ROOT_CAUSE_CLASS**：UX / DISCOVERABILITY；**RECOMMENDATION**：M3 有界可见缩略图、已有收藏/标签筛选上移；保留覆盖说明和分类 keyset，不造全局索引；**EFFORT**：M (3)；**RISK**：2；**TARGET_VERSION**：2.1。

### F06 — 媒体不可预览后缺少安全维护动作
- **AREA**：Local Media Maintenance；**PROBLEM**：预览失败只提示“媒体无法预览；使用关系和历史仍可查看”，没有检查/定位去向。
- **USER_IMPACT**：用户不知是 codec、读权限、文件缺失还是损坏；**FREQUENCY**：少见但痛，估计 2/5；**SEVERITY**：P2；**EVIDENCE**：S `LibraryDetail.tsx`，已知问题文档；**未在 Native 人为损坏文件**。
- **ROOT_CAUSE_CLASS**：DATA_MAINTENANCE / ERROR_RECOVERY；**RECOMMENDATION**：M4 先只读检查，分开存在/可读/hash/预览解码，保留 identity。导入通常复制进应用存储，原始外部文件移动不应被默认认定为已导入媒体丢失；受管理文件位置与真实现象需 discovery 确认。
- **EFFORT**：M (3)；**RISK**：3；**TARGET_VERSION**：2.1 有界只读检查；写入 relink 延后独立审查。

### F07 — Create 内复用提示词/素材只有近期选择，缺少查找说明
- **AREA**：Create / Prompt Studio / Search；**PROBLEM**：prompt_choices 仅取近期 20 条的最新版本，picker 无搜索；media_inputs 通常为近期 100 项加明确镜头关联。
- **USER_IMPACT**：旧提示词/素材不在选择器时误以为丢失；**FREQUENCY**：每天复用，估计 4/5；**SEVERITY**：P2；**EVIDENCE**：S `creation_facade/context.rs`、`CreateInputs.tsx`、`CreateController.ts`。Library 的显式 asset intent 会按 ID 载入非近期素材，已有安全绕行，不是彻底不能复用。
- **ROOT_CAUSE_CLASS**：DISCOVERABILITY / UX；**RECOMMENDATION**：M2/M3 写清近期覆盖、连接 Library 的模块查找；保留显式版本选择和源记录，不能由正文推断 Prompt identity；**EFFORT**：M (3)；**RISK**：2；**TARGET_VERSION**：2.1。

### F08 — Tool Hub 登记与实际连接不是同一个概念
- **AREA**：Tool Hub；**PROBLEM**：Comfy 状态可为已连接，同时 Tool Hub 显示 0 登记；该页只列登记/观察元数据，无发现或登记 CTA。
- **USER_IMPACT**：以为 AI Studio 未识别已连上的本地工具；**FREQUENCY**：首次配置/工具更新，估计 2/5；**SEVERITY**：P2；**EVIDENCE**：N 同会话已连接状态与 0 登记；S `LocalToolHub.tsx`。
- **ROOT_CAUSE_CLASS**：DISCOVERABILITY；**RECOMMENDATION**：解释“已连接 ≠ 已登记”“最近记录 ≠ 现在健康”；合并进 M1 的说明。自动 safe discovery 独立 DEFER，不自动安装/启停/执行。
- **EFFORT**：说明 S (1)，发现 M–L (4)；**RISK**：说明 1，发现 3；**TARGET_VERSION**：2.1 仅说明；自动发现未排期。

### F09 — Diagnostics 默认技术信息密度高
- **AREA**：Diagnostics / Settings；**PROBLEM**：普通环境配置前展开任务窗口、遥测完整度、各阶段中位数；定位故障仍涉及 code/任务 ID。
- **USER_IMPACT**：普通用户寻找环境配置时容易被统计术语打断；**FREQUENCY**：排查问题，估计 3/5；**SEVERITY**：P3；**EVIDENCE**：N 设置页；S `DiagnosticsExecutionPanel.tsx`、`SettingsWorkspace.tsx`。
- **ROOT_CAUSE_CLASS**：UX；**RECOMMENDATION**：M1/M2 摘要先行、技术折叠，保留导出与已存在跳转；未知不改成正常；**EFFORT**：S (2)；**RISK**：2；**TARGET_VERSION**：2.1 合并交付，不重写 observability。

### F10 — 普通项目上下文名称不一致
- **AREA**：Navigation；**PROBLEM**：Native Overview 标题为“默认项目”，侧栏/面包屑为“Default Project”。
- **USER_IMPACT**：轻微项目上下文辨识成本；**FREQUENCY**：默认项目访问，估计 4/5；**SEVERITY**：P3；**EVIDENCE**：N；S Overview 用 projectDisplayName，Shell 展示传入 projectName。
- **ROOT_CAUSE_CLASS**：UX；**RECOMMENDATION**：M2 同一展示规则，不改用户保存的项目名、ID、路由；**EFFORT**：S (1)；**RISK**：1；**TARGET_VERSION**：2.1 合并 polish。

### F11 — 通用素材说明暗示未支持音频输入
- **AREA**：Create；**PROBLEM**：普通图片 Create 也显示“首帧、尾帧及视频/音频输入…”的通用说明。
- **USER_IMPACT**：与只做图片/H3 的产品范围不一致；**FREQUENCY**：Create 页面，估计 5/5；**SEVERITY**：P3；**EVIDENCE**：N；S `CreateInputs.tsx`。这不表示音频输入生产被重新启用。
- **ROOT_CAUSE_CLASS**：UX；**RECOMMENDATION**：M2 按实际字段呈现帮助；历史/imported audio Library 保留，不增加音频生成或快速预览；**EFFORT**：S (1)；**RISK**：1；**TARGET_VERSION**：2.1 合并 polish。

### F12 — 仓库首页还把已发布 r4 写成候选阶段
- **AREA**：Documentation；**PROBLEM**：README/候选文件仍保留“in progress/no new release/pending CI”等阶段时点描述。
- **USER_IMPACT**：难分辨 Release 真正权威；**FREQUENCY**：下载/查文档时，估计 2/5；**SEVERITY**：P3；**EVIDENCE**：README 当前状态段与 r4 Release 对照；候选证据自身可保留其写作时点。
- **ROOT_CAUSE_CLASS**：DOCUMENTATION；**RECOMMENDATION**：下次产品文档 checkpoint 更新 live 首页，候选记录明确为历史。**本轮只写两份审计/路线图，不顺手改 README**；**EFFORT**：S (1)；**RISK**：1；**TARGET_VERSION**：2.1 文档 checkpoint。

### F13 — 大库风险有结构依据，但缺少规模证据
- **AREA**：Large Library / Performance；**PROBLEM**：不知道真实个人库规模/延迟分布。Library 每 5 秒刷新已加载页数，loadMore 累积 DOM，Profiles/ReferenceSets 是整类列表；输入每次变化会触发查询。历史工作削减不代表此规模已测。
- **USER_IMPACT**：潜在累计页数成本，不是已复现卡死；**FREQUENCY**：I 取决于规模，暂记 2/5；**SEVERITY**：P3 测量债务；**EVIDENCE**：S `LibraryController.ts`、`library_facade/list.rs`；I，无新 benchmark。
- **ROOT_CAUSE_CLASS**：PERFORMANCE；**RECOMMENDATION**：MEASURE_BEFORE_OPTIMIZE；M3 基准 discovery，未过预算再选择一个 seam；**EFFORT**：测量 S–M (2)，优化待定；**RISK**：测量 1；**TARGET_VERSION**：2.1 测量，50k/100k 专项工程 DEFER。

## 5. 已知 P2 重新取舍

| Known issue | REAL_USER_VALUE | FREQUENCY / PAIN | COST / ARCHITECTURE_RISK | 决定 | 2.1 priority |
| --- | --- | --- | --- | --- | --- |
| UI detail polish | 只有减少歧义/找回成本的改动有价值 | 日常 / 低–中 | 小 / 低 | SPLIT + MERGE：F05/F09–11 并入对应主题；纯美化 DEFER | 随 M1–M3，不独立主题 |
| Tool discovery | 本地工具可解释价值高，通用发现价值未证实 | 配置期 / 中 | 中–大 / 中 | SPLIT：说明 KEEP+MERGE M1；只读候选发现 DEFER | 不作为首项 |
| Prompt statistics | 精确 usage count/recent use 或有帮助；质量评分无证据 | 低 / 低 | 聚合中 / identity 覆盖风险 | DEFER 精确事实统计；DROP 自动评分/优化/最佳决策 | 不进 2.1 独立开发 |
| Search | 先找图片、提示词、旧 Run，Project/Shot 为其次；Workflow 在 Lab | 日常 / 高 | 模块改进中 / 低–中 | KEEP+MERGE M3，Create 近期入口说明与 M2 协作 | 第三主题 |
| Large-library performance | 个人收藏增长后才有价值 | 未测 / 未测 | 未测 / 中 | MEASURE_BEFORE_OPTIMIZE；DEFER 大型工程 | M3 测量子项 |
| Local media maintenance | 读失败时保护项目可信度 | 低 / 高 | 中 / 数据写入高 | SPLIT：KEEP 只读检查；DEFER 重连/修复写入 | 第四主题，带范围闸门 |

KEEP：可操作准备状态、失败恢复、模块找回、只读媒体检查。
MERGE：有价值 polish、Tool 登记解释、提示词复用 → 前三主题。
DEFER：通用工具发现、精确统计、未经测量的大库优化、写入 relink。
DROP：自动 Prompt 质量判断、自动安装/启停/执行、全局搜索平台和无目标美化。
新增/细化发现：F01–04、F07–08、F10–12；F05/F06/F09/F13 是已有 P2 的具体化，而不是新引擎需求。

## 6. Error UX 抽样结论

| 样本 | 证据 / 结论 | 规划动作 |
| --- | --- | --- |
| Comfy offline | N 只读连接测试显示启动/重连说明；S Create runtime issue 按字段定位不够 | 指向现有环境页，分清测试地址与已保存地址 |
| Missing model / custom node | S 预检保留 missingNodes/reason/workflow 明细；本轮 Native NOT VERIFIED | 复用现有已知事实，分开原因/验证动作；UNKNOWN 不猜缺什么 |
| Invalid input | S 字段定位机制和稳定产品错误码已存在 | 保留 focus，并指出字段和允许范围 |
| Asset unavailable | S 重新选择/无法预览文案，关系仍保留 | 检查/重新选择，不猜身份，不自动删除 |
| Failed / partial run | S capability 驱动按钮，H 安全重试/结果保留 | 显示可重试与新草稿动作差别；不静默重试 |
| Binding conflict | S GeneratorSettings conflict 后只刷新，不重试写入 | 保留 OCC，给用户比较/重新确认入口 |
| Backup issue | H v20 严格检查/恢复已验收；本轮不重跑 | 清晰列出缺失项与保留源项目说明；不放宽 archive validation |

10 秒内理解原因/动作是未来的验收目标，不是本轮已验证结论。

## 7. 规模判断与测量计划

- 初步个人版 benchmark：1k assets 是基准，10k 是增长压力点；50k/100k 只做可选探索，无用户规模证据前不承诺支持时延。
- 当前 media/prompt 是 30 默认、100 上限 keyset；All 是各类别 12 项近期摘要。Profiles/ReferenceSets 仍由现有服务整类返回并过滤，不能把它们描述成 SQLite 搜索分页。
- Library 列表暂不请求卡片缩略图，因此没有“当前全量缩略图并发”证据；增加缩略图时必须仅加载可见窗口，管理 Blob URL 生命周期。详情才读取媒体。
- refresh 读已加载页数、DOM 累计、搜索查询、关系/版本详情是应测量的路径；不能仅凭 for-loop 决定 cache/virtualization。
- 测 1/5/20 页、同数量短/长名称、图片/视频比例、Profiles/ReferenceSets 规模、冷/热查询和快速输入。记录 p50/p95、IPC 次数/bytes、DOM 数、首个可见预览与内存，Windows 单 runner，无 GPU 生成。
- 对现有 SQLite repository 查询形状/索引执行受控 EXPLAIN 和基准后再改。没有本轮 SQL 计划或速度数字；这些是下一轮 discovery。
- Prompt Studio 每页逐条读详情、Create 近期 Prompt 逐条 get 只是 S 风险线索，不能无 profiling 就开性能重构。

## 8. Native 观察边界与停止条件

这次一次 session 已覆盖入口/空态/配置可发现性；所有关键页面能打开，没有出现白屏或新的产品不可用症状。系统诊断导出入口易找到，Lab 导入/版本/诊断仍存在，Advanced 定位应保留。主导航不应重做。

Native 空 Runs/Library/Prompt 不提供已有记录错误、关系或大集合的本轮体验证据；相关结论分别引用 S/H。不把覆盖窗口误写成全历史，工具登记空也不是 Comfy 离线。缺模型/节点、资源损坏、实际失败重试、本轮 Project 切换/dirty draft 未重测，均 NOT VERIFIED IN THIS NATIVE SESSION；Phase14 已冻结证据保持有效。

若未来发现数据损坏/安全问题/发布版本不可用，停止规划并升级 P0 blocker；当前没有该证据。P1=NONE_OBSERVED，P2 如上按价值排，不自动变成实施授权。

## 9. 下一步

详见 [2.1 Roadmap](AI_STUDIO_2_1_ROADMAP.md)。本轮只产生这两份文档，版本仍为 2.0.0-personal，schema max=42、Backup v20、Queue/Task/OCC/项目隔离及 Phase12/13 边界不变。不开始 2.1 编码。
