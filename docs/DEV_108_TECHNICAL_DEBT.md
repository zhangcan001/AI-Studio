# DEV-108 — AI Studio Technical Debt Register

~~~text
TASK=DEV-108
MODE=PRODUCT_AUDIT
CURRENT_RELEASE=1.2.0
P0=NONE
P1=NONE
P1_RELEASE_BLOCKER=NONE
CI_ACCEPTABLE=YES
REAL_COMFY_RUNTIME_AVAILABLE=NO
DO_NOW_POLICY=NO_UNLESS_P0_OR_P1
~~~

## Debt policy

本文件与 Product Friction Inventory 分开：产品摩擦描述用户感受和产品优先级；本文件记录实现、验证、构建、性能和文档上的长期成本。DEV-108 只登记，不顺手修复。当前没有 P0/P1，因此所有条目的 DO_NOW=NO。

## BUILD

### TD-108-001

~~~text
ID=TD-108-001
AREA=BUILD
SEVERITY=P3
IMPACT=Frontend build continues to emit the existing large chunk warning, increasing first-load and cache invalidation cost as the application grows.
RECOMMENDATION=Only revisit after a measured startup/performance profile identifies a user-visible regression; prefer a small route-level split over speculative bundler architecture.
DO_NOW=NO
~~~

证据：DEV-107/1.2 build passed；warning 已知但不是功能或 release blocker。

## CI

### TD-108-002

~~~text
ID=TD-108-002
AREA=CI
SEVERITY=P3
IMPACT=Source-only CI is approximately Frontend 3 minutes and Rust 14 minutes, so feedback is not instant but remains acceptable for the current release cadence.
RECOMMENDATION=Keep the current reliable gates. Do not introduce a complex matrix, fragile cache, or broad parallelization merely to save a few minutes.
DO_NOW=NO
~~~

CI_ACCEPTABLE=YES。DEV-108 不以 CI 时长为 1.3 第一主题。

## FRONTEND

### TD-108-003

~~~text
ID=TD-108-003
AREA=FRONTEND
SEVERITY=P2
IMPACT=App.tsx、StudioShell、ShotWorkspace 和多个 production surfaces 共同编排 mode、section、focus 与 drawer，局部改动容易产生导航语义不一致。
RECOMMENDATION=在需要时建立小型 typed navigation/focus contract，并用现有组件逐步收敛；避免一次性重写 App 或引入第二套 router。
DO_NOW=NO
~~~

### TD-108-004

~~~text
ID=TD-108-004
AREA=FRONTEND
SEVERITY=P2
IMPACT=task-only daily target 当前存在 destination/section precedence 风险，可能把用户带到正确项目但错误 workspace。
RECOMMENDATION=作为 DEV-109 的第一条 route regression 修复；要求 task target、batch target、asset target 各自有 focused assertion。
DO_NOW=NO
~~~

## RUST

### TD-108-005

~~~text
ID=TD-108-005
AREA=RUST
SEVERITY=P3
IMPACT=Command Center 为派生桶读取项目事实后再把每桶列表限制为 20 项；在当前 500-shot 证据上可用，但未来更大项目可能增加读取和序列化成本。
RECOMMENDATION=先建立真实规模 profile，再决定 query projection、分页或索引调整；不要因静态担心提前引入新 read model。
DO_NOW=NO
~~~

## DATABASE

### TD-108-006

~~~text
ID=TD-108-006
AREA=DATABASE
SEVERITY=P3
IMPACT=当前 migration 032 和项目/生产关系稳定，没有已确认的数据库债务；未来导航增强若绕开现有 repository 可能重新引入一致性风险。
RECOMMENDATION=保持 repository ports、项目隔离和向后兼容；只有出现真实 schema 需求时再单独评估 migration。
DO_NOW=NO
~~~

这里记录的是“无主动数据库债务但有边界守护成本”，不是要求新增 migration。

## BACKUP

### TD-108-007

~~~text
ID=TD-108-007
AREA=BACKUP
SEVERITY=P3
IMPACT=V18 roundtrip 和 legacy V17 restore 已通过，但后续若新增导航所需持久化字段，会增加 backup/remap 兼容面。
RECOMMENDATION=1.3 第一阶段只使用现有事实和 transient focus；保持 V18/legacy 兼容，不为 UX convenience 改 backup version。
DO_NOW=NO
~~~

## TEST

### TD-108-008

~~~text
ID=TD-108-008
AREA=TEST
SEVERITY=P2
IMPACT=没有单一 App+Rust 端到端测试覆盖完整生产旅程，跨 surface 的连续性回归主要靠 focused tests 和人工审计。
RECOMMENDATION=在后续交付中建立一个最小 happy-path contract；只覆盖 authority、target 和不自动 Start 的关键断言。
DO_NOW=NO
~~~

### TD-108-009

~~~text
ID=TD-108-009
AREA=TEST
SEVERITY=P2
IMPACT=task-only daily card 的 route precedence 没有专门 regression，导航修复容易被后续 section mapping 改回。
RECOMMENDATION=为 task、batch、shot 三类 target 各增加一个 focused navigation assertion。
DO_NOW=NO
~~~

### TD-108-010

~~~text
ID=TD-108-010
AREA=TEST
SEVERITY=P2
IMPACT=没有自动断言最终 selected result、Asset 入口和文件位置在 Project Center/Shot/Asset/Monitor 之间保持一致。
RECOMMENDATION=增加只读 cross-surface acceptance fixture；不要求先建 deliverable model。
DO_NOW=NO
~~~

### TD-108-011

~~~text
ID=TD-108-011
AREA=TEST
SEVERITY=P3
IMPACT=topbar search focus、Queue drawer 可发现性、键盘操作、长列表滚动和视觉层级没有专门 regression。
RECOMMENDATION=结合具体 UX slice 增加少量 accessibility/focus assertions，避免一开始建立大而脆弱的 screenshot suite。
DO_NOW=NO
~~~

## PERFORMANCE

### TD-108-012

~~~text
ID=TD-108-012
AREA=PERFORMANCE
SEVERITY=P3
IMPACT=已有 500-shot admission/backend 证据，但没有真实桌面 500-shot 渲染、滚动、输入响应和内存时间 profile。
RECOMMENDATION=准备可重复的大项目 fixture 和明确预算，只有 profile 证明瓶颈后再优化组件、query 或 bundling。
DO_NOW=NO
~~~

ComfyUI 缺失限制了 live production smoke，但不应将环境缺失误记为产品性能债务。

## DOCUMENTATION

### TD-108-013

~~~text
ID=TD-108-013
AREA=DOCUMENTATION
SEVERITY=P3
IMPACT=DEV-099～DEV-107 是分阶段历史记录，旧 roadmap 中的“进行中”描述可能与 1.2 已发布状态产生阅读冲突。
RECOMMENDATION=以本 DEV-108 审计和 1.2 publication 文档为当前事实入口；后续只做小范围历史链接/状态标记，不重写历史记录。
DO_NOW=NO
~~~

## Summary

~~~text
TECHNICAL_DEBT_P0=NONE
TECHNICAL_DEBT_P1=NONE
TECHNICAL_DEBT_P2=5
TECHNICAL_DEBT_P3=8
CI_ACCEPTABLE=YES
REAL_COMFY_SMOKE=NOT_RUN_ENVIRONMENT_UNAVAILABLE
NEW_MIGRATION=NO
BACKUP_VERSION_CHANGE=NO
~~~

这些债务不会自动生成 DEV-109，也不会触发当前代码修改。它们只为后续优先级、测试补强和规模验证提供可追踪记录。