import { useState } from "react";
import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
} from "../../types/workflowOnboarding";

interface Props {
  plan: WorkflowAutoOnboardingPlanView;
  loading: boolean;
  onResolve: (issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) => void;
  onResume: () => void;
  onOpenAdvanced: () => void;
  onOpenExisting: () => void;
  onRestoreExisting?: () => void;
}

export function WorkflowImportIssues({ plan, loading, onResolve, onResume, onOpenAdvanced, onOpenExisting, onRestoreExisting }: Props) {
  const [selected, setSelected] = useState<Record<string, number>>({});
  const waiting = plan.state === "WAITING_FOR_COMFY_UI";
  return (
    <section className="workflow-smart-issues" aria-label="工作流导入问题">
      <div className="workflow-smart-issues-heading">
        <div>
          <span className="section-label">智能导入暂停</span>
          <h3>{waiting ? "等待 ComfyUI 连接" : plan.state === "BLOCKED" ? "工作流暂时无法发布" : "工作流需要确认"}</h3>
          <p>{plan.message}</p>
        </div>
        <span className={`workflow-smart-state workflow-smart-state-${plan.state.toLowerCase()}`}>{waiting ? "等待中" : plan.state === "BLOCKED" ? "已阻断" : "待确认"}</span>
      </div>
      {!!plan.issues.length && (
        <div className="workflow-smart-issue-list">
          {plan.issues.map((issue, issueIndex) => {
            const choice = selected[issue.code] ?? -1;
            return (
              <article className="workflow-smart-issue" key={`${issue.code}-${issueIndex}`}>
                <div>
                  <strong>{issue.code === "MISSING_NODES" ? "缺少节点" : issue.code === "AMBIGUOUS_OUTPUT" ? "输出无法唯一判断" : issue.code === "FLOAT_INPUT_NEEDS_REVIEW" ? "浮点输入需要确认" : "输入无法唯一判断"}</strong>
                  <p>{issue.message}</p>
                </div>
                {!!issue.candidates.length && (
                  <fieldset>
                    <legend>请选择候选项</legend>
                    {issue.candidates.map((candidate, candidateIndex) => (
                      <label key={`${candidate.label}-${candidateIndex}`}>
                        <input
                          type="radio"
                          name={`${issue.code}-${issueIndex}`}
                          checked={choice === candidateIndex}
                          onChange={() => setSelected((current) => ({ ...current, [issue.code]: candidateIndex }))}
                        />
                        <span>{candidate.label}</span>
                      </label>
                    ))}
                  </fieldset>
                )}
                {!!issue.candidates.length && (
                  <button
                    type="button"
                    className="quiet-button"
                    disabled={loading || choice < 0}
                    onClick={() => onResolve(issue, issue.candidates[choice])}
                  >
                    应用选择并继续
                  </button>
                )}
              </article>
            );
          })}
        </div>
      )}
      <div className="workflow-smart-actions">
        {waiting && <button type="button" onClick={onResume} disabled={loading}>{loading ? "正在检查..." : "继续自动确认"}</button>}
        {(plan.state === "ALREADY_EXISTS" || plan.state === "ALREADY_EXISTS_ARCHIVED") && <button type="button" onClick={onOpenExisting}>打开现有工作流</button>}
        {plan.state === "ALREADY_EXISTS_ARCHIVED" && onRestoreExisting && <button type="button" onClick={onRestoreExisting} disabled={loading}>恢复归档工作流</button>}
        <button type="button" className="quiet-button" onClick={onOpenAdvanced}>高级编辑</button>
      </div>
    </section>
  );
}
