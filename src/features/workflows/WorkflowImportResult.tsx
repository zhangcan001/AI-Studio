import type { WorkflowAutoOnboardingPlanView } from "../../types/workflowOnboarding";

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

function capabilityLabel(state: string): string {
  switch (state) {
    case "READY":
      return "当前环境可用";
    case "MISSING_NODES":
      return "已添加，当前环境缺少节点";
    case "COMFY_OFFLINE":
      return "已添加，等待 ComfyUI 检查";
    case "INCOMPATIBLE_INPUT_VALUES":
      return "已添加，需要检查输入";
    default:
      return "已添加，待检查";
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
  const useInProject = onUseInProject ?? onOpenStudio;
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
          <span>可用状态<strong>{capabilityLabel(plan.capability.state)}</strong></span>
        </div>
        {!!missingNodes.length && <p className="workflow-import-result-warning">⚠ 当前 ComfyUI 缺少 {missingNodes.length} 个节点：{missingNodes.join("、")}。工作流已保存，修复节点前不能生产。</p>}
        <div className="workflow-smart-actions">
          {projectId && useInProject && <button type="button" onClick={() => useInProject(published.workflowId, published.recipeId)}>用于当前项目</button>}
          {onOpenStudio && <button type="button" className="quiet-button" onClick={() => onOpenStudio(published.workflowId, published.recipeId)}>打开生成页面</button>}
          <button type="button" className="quiet-button" onClick={onOpenAdvanced}>查看高级详情</button>
          {onReturnToList && <button type="button" className="quiet-button" onClick={onReturnToList}>返回工作流列表</button>}
        </div>
      </div>
    </section>
  );
}
