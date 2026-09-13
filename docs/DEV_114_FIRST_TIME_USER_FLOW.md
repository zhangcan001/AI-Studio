# DEV-114 First-Time User Flow

## Empty application

When no project is active, the Project Command Center shows **开始 AI 生产** and tells the user to create a project. **创建第一个项目** and **管理项目** both use the existing Projects workspace route. No production command is called from the guide.

## Empty active project

After creating or opening a project with no shots or assets, the same guide remains visible with the first step marked complete. The user can choose either:

- **导入 Production Handoff** — invokes the existing project-scoped `ProjectImportDryRunWorkspace`, then the existing External Agent Handoff preflight and explicit confirmation flow.
- **开始创作** — navigates to the existing Shot creation workspace with `actionKind=NO_SHOTS`.

## Production handoff path

The intended first delivery path is:

```text
Project Command Center
  → 导入 Production Handoff
  → 运行交接预检
  → 明确确认并写入
  → 打开项目结构 / 生产准备
  → 创建待启动批次
  → 生产队列
  → 开始生产
  → 生产监控 / 审核
  → 最终结果
```

The import action is still read-only until the existing explicit confirmation. Creating a batch only prepares queue work; only the existing Queue Start action submits real production tasks.

## Creation path

```text
Project Command Center
  → 开始创作
  → 建立镜头与配置
  → 生产准备
  → 创建待启动批次
  → 生产队列 / 开始生产
```

## Success criterion

The empty state answers **where am I**, **what do I do**, **what happens next**, and **when does production really start** without introducing a tutorial system or a second source of truth.

