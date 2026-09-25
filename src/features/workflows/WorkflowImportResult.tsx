import type {
  RuntimeImportStatus,
  SemanticCapabilityStatus,
  WorkflowAutoOnboardingPlanView,
} from "../../types/workflowOnboarding";

interface Props {
  plan: WorkflowAutoOnboardingPlanView;
  projectId?: string;
  onOpenAdvanced: () => void;
  onOpenStudio?: (workflowId: string, recipeId: string) => void;
  onUseInProject?: (workflowId: string, recipeId: string) => void;
  onReturnToList?: () => void;
}

function workflowTypeLabel(kind: string): string {
  const normalized = kind.trim().toUpperCase();
  if (normalized === "VIDEO") return "视频生成";
  if (normalized === "MIXED") return "图片与视频";
  if (normalized === "IMAGE") return "图片生成";
  return "需要确认";
}

function workflowPurposeLabel(category: string, kind: string): string {
  const normalizedCategory = category.trim().toUpperCase();
  if (normalizedCategory.includes("VIDEO")) return "视频生成";
  if (normalizedCategory.includes("IMAGE")) return "图片生成";
  return workflowTypeLabel(kind);
}

function semanticCapabilityLabel(state: SemanticCapabilityStatus): string {
  switch (state) {
    case "READY":
      return "已识别并支持";
    case "NEEDS_REVIEW":
      return "部分能力需要确认";
    case "UNSUPPORTED":
      return "当前不支持";
    default:
      return "尚未评估";
  }
}

function runtimeImportLabel(state: RuntimeImportStatus): string {
  switch (state) {
    case "READY":
      return "当前环境可运行";
    case "NEEDS_REVIEW":
      return "可保存，运行前需处理";
    case "BLOCKED":
      return "当前环境阻止运行";
    default:
      return "当前环境尚未验证";
  }
}

function missingNodeLabels(plan: WorkflowAutoOnboardingPlanView): string[] {
  return plan.capability.issues
    .filter((issue) => issue.code === "MISSING_NODE" || issue.code === "MISSING_NODES")
    .map((issue) => issue.classType ?? issue.message)
    .filter((value, index, values) => value.trim().length > 0 && values.indexOf(value) === index);
}

export function WorkflowImportResult({ plan, projectId, onOpenAdvanced, onOpenStudio, onUseInProject, onReturnToList }: Props) {
  const published = plan.published;
  if (!published) return null;
  const inputLabels = plan.inputMappings.map((mapping) => mapping.label).join("、") || "无字面量输入";
  const outputLabels = plan.outputMappings.map((mapping) => mapping.type === "video" ? "视频" : "图片").join("、") || "—";
  const missingNodes = missingNodeLabels(plan);

  return (
    <section className="workflow-smart-result" aria-label="工作流添加结果" role="status">
      <div className="workflow-smart-result-mark" aria-hidden="true">✓</div>
      <div className="workflow-smart-result-copy">
        <span className="section-label">添加完成</span>
        <h3>✓ 工作流已添加</h3>
        <p>工作流已加入列表，现在可以在项目设置中选择，或直接开始创作。</p>
        <div className="workflow-smart-result-grid">
          <span>名称<strong>{plan.metadata.name}</strong></span>
          <span>类型<strong>{workflowTypeLabel(plan.workflowKind)}</strong></span>
          <span>用途<strong>{workflowPurposeLabel(plan.metadata.category, plan.workflowKind)}</strong></span>
          <span>工作流版本<strong>{plan.metadata.workflowVersion}</strong></span>
          <span>输入<strong>{inputLabels}</strong></span>
          <span>输出<strong>{outputLabels}</strong></span>
          <span>导入状态<strong>可加入工作流库</strong></span>
          <span>语义能力<strong>{semanticCapabilityLabel(plan.semanticCapabilityStatus)}</strong></span>
          <span>运行导入<strong>{runtimeImportLabel(plan.runtimeImportStatus)}</strong></span>
          {plan.runtimeImportBlockers.length > 0 && <span>运行阻塞项<strong>{plan.runtimeImportBlockers.length} 项</strong></span>}
        </div>
        {plan.runtimeImportStatus === "NOT_EVALUATED" && <p className="workflow-import-result-warning">⚠ 当前运行环境尚未完成验证；连接 ComfyUI 后可重新检查。</p>}
        {!!missingNodes.length && <p className="workflow-import-result-warning">⚠ 当前 ComfyUI 缺少 {missingNodes.length} 个节点：{missingNodes.join("、")}。工作流已经保存，安装节点后即可运行。</p>}
        <div className="workflow-smart-actions">
          {projectId && onUseInProject && <button type="button" onClick={() => onUseInProject(published.workflowId, published.recipeId)}>用于当前项目</button>}
          {onOpenStudio && <button type="button" className="quiet-button" onClick={() => onOpenStudio(published.workflowId, published.recipeId)}>打开生成页面</button>}
          <button type="button" className="quiet-button" onClick={onOpenAdvanced}>查看高级详情</button>
          {onReturnToList && <button type="button" className="quiet-button" onClick={onReturnToList}>返回工作流列表</button>}
        </div>
      </div>
    </section>
  );
}
