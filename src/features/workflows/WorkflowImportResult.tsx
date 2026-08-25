import type { WorkflowAutoOnboardingPlanView } from "../../types/workflowOnboarding";

interface Props {
  plan: WorkflowAutoOnboardingPlanView;
  onOpenAdvanced: () => void;
  onOpenStudio?: (workflowId: string, recipeId: string) => void;
}

export function WorkflowImportResult({ plan, onOpenAdvanced, onOpenStudio }: Props) {
  const published = plan.published;
  if (!published) return null;
  const inputLabels = plan.inputMappings.map((mapping) => mapping.label).join("、") || "无字面量输入";
  const outputLabels = plan.outputMappings.map((mapping) => mapping.type === "video" ? "视频" : "图片").join("、") || "—";

  return (
    <section className="workflow-smart-result" aria-label="智能导入结果">
      <div className="workflow-smart-result-mark" aria-hidden="true">通过</div>
      <div className="workflow-smart-result-copy">
        <span className="section-label">智能导入完成</span>
        <h3>工作流导入成功</h3>
        <p>{plan.message}</p>
        <div className="workflow-smart-result-grid">
          <span>名称<strong>{plan.metadata.name}</strong></span>
          <span>类型<strong>{plan.workflowKind === "VIDEO" ? "视频生成" : plan.workflowKind === "MIXED" ? "混合输出" : "图片生成"}</strong></span>
          <span>节点<strong>{plan.nodeCount}</strong></span>
          <span>输入<strong>{inputLabels}</strong></span>
          <span>输出<strong>{outputLabels}</strong></span>
          <span>兼容性<strong>通过</strong></span>
          <span>配方<strong>已自动生成</strong></span>
          <span>状态<strong>已启用</strong></span>
        </div>
        <div className="workflow-smart-actions">
          {onOpenStudio && <button type="button" onClick={() => onOpenStudio(published.workflowId, published.recipeId)}>直接使用</button>}
          <button type="button" className="quiet-button" onClick={onOpenAdvanced}>查看详情</button>
        </div>
      </div>
    </section>
  );
}
