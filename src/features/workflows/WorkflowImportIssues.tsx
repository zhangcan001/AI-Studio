import { useEffect, useState } from "react";
import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
} from "../../types/workflowOnboarding";

export type WorkflowImportErrorKind = "UI_FORMAT" | "INVALID_JSON" | "UNKNOWN_FORMAT";

export interface WorkflowImportErrorView {
  kind: WorkflowImportErrorKind;
  message: string;
}

export function workflowIssueSelectionKey(issue: WorkflowAutoIssueView, issueIndex: number): string {
  return [issue.code, issue.field ?? "NONE", String(issueIndex)].join(":");
}

interface Props {
  plan: WorkflowAutoOnboardingPlanView;
  loading: boolean;
  onResolve: (issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) => void;
  onResume: () => void;
  onOpenAdvanced: () => void;
  onOpenExisting: () => void;
  onRestoreExisting?: () => void;
  onCancel?: () => void;
}

interface FormatIssueProps {
  issue: WorkflowImportErrorView;
  loading: boolean;
  onRetry?: () => void;
  onCancel?: () => void;
}

export function WorkflowImportFormatIssue({ issue, loading, onRetry, onCancel }: FormatIssueProps) {
  const [showGuide, setShowGuide] = useState(false);
  const isUi = issue.kind === "UI_FORMAT";
  const title = isUi
    ? "检测到 ComfyUI 普通工作流 JSON"
    : issue.kind === "INVALID_JSON"
      ? "无法读取这个文件"
      : "无法识别这个工作流";

  return (
    <section className="workflow-smart-issues workflow-import-format-issue" aria-label="工作流添加结果" role="status">
      <div className="workflow-smart-issues-heading">
        <div>
          <span className="section-label">添加工作流</span>
          <h3>{title}</h3>
          <p>{issue.message}</p>
          {isUi && <p>请在 ComfyUI 中将该工作流导出为 API Format JSON，然后重新选择该文件。</p>}
        </div>
        <span className="workflow-smart-state workflow-smart-state-blocked">未添加</span>
      </div>
      {isUi && showGuide && (
        <div className="workflow-import-guidance">
          <strong>导出方法</strong>
          <p>在 ComfyUI 中打开工作流，使用“导出 API 格式”或“Export (API)”保存为 JSON，再回到这里重新选择。</p>
        </div>
      )}
      <div className="workflow-smart-actions">
        {isUi && <button type="button" className="quiet-button" onClick={() => setShowGuide((current) => !current)} aria-expanded={showGuide}>{showGuide ? "收起导出方法" : "查看导出方法"}</button>}
        {onRetry && <button type="button" onClick={onRetry} disabled={loading}>选择另一个文件</button>}
        {onCancel && <button type="button" className="quiet-button" onClick={onCancel} disabled={loading}>返回工作流列表</button>}
      </div>
    </section>
  );
}

function planMessage(plan: WorkflowAutoOnboardingPlanView): string {
  if (plan.message.trim()) return plan.message;
  if (plan.state === "WAITING_FOR_COMFY_UI") return "正在检查当前 ComfyUI 环境，请稍候。";
  if (plan.state === "ALREADY_EXISTS" || plan.state === "ALREADY_EXISTS_ARCHIVED") return "这个工作流已经存在，请打开现有版本或返回列表。";
  if (plan.state === "BLOCKED") return "工作流可以保存，但当前环境还不满足运行条件。";
  if (plan.issues.length) return `还有 ${plan.issues.length} 项需要确认，确认后即可添加。`;
  return "请确认工作流信息后继续添加。";
}

function issueTitle(code: string): string {
  switch (code) {
    case "MISSING_NODES":
      return "当前环境缺少节点";
    case "AMBIGUOUS_OUTPUT":
      return "输出类型需要确认";
    case "FLOAT_INPUT_NEEDS_REVIEW":
      return "数值参数需要确认";
    default:
      return "参数用途需要确认";
  }
}

export function WorkflowImportIssues({ plan, loading, onResolve, onResume, onOpenAdvanced, onOpenExisting, onRestoreExisting, onCancel }: Props) {
  const [selected, setSelected] = useState<Record<string, number>>({});
  const waiting = plan.state === "WAITING_FOR_COMFY_UI";
  const issueFingerprint = plan.issues.map((issue, issueIndex) => workflowIssueSelectionKey(issue, issueIndex)).join("|");
  useEffect(() => setSelected({}), [plan.draftId, plan.state, issueFingerprint]);
  return (
    <section className="workflow-smart-issues" aria-label="工作流导入问题">
      <div className="workflow-smart-issues-heading">
        <div>
          <span className="section-label">添加工作流</span>
          <h3>{waiting ? "等待 ComfyUI 连接" : plan.state === "BLOCKED" ? "工作流暂时无法添加" : "需要确认后添加"}</h3>
          <p>{planMessage(plan)}</p>
        </div>
        <span className={`workflow-smart-state workflow-smart-state-${plan.state.toLowerCase()}`}>{waiting ? "等待中" : plan.state === "BLOCKED" ? "暂不可用" : "需要确认"}</span>
      </div>
      {!!plan.issues.length && (
        <div className="workflow-smart-issue-list">
          {plan.issues.map((issue, issueIndex) => {
            const issueKey = workflowIssueSelectionKey(issue, issueIndex);
            const choice = selected[issueKey] ?? -1;
            return (
              <article className="workflow-smart-issue" key={issueKey}>
                <div>
                  <strong>{issueTitle(issue.code)}</strong>
                  <p>{issue.message}</p>
                </div>
                {!!issue.candidates.length && (
                  <fieldset>
                    <legend>请选择候选项</legend>
                    {issue.candidates.map((candidate, candidateIndex) => (
                      <label key={`${candidate.label}-${candidateIndex}`}>
                        <input
                          type="radio"
                          name={issueKey}
                          checked={choice === candidateIndex}
                          onChange={() => setSelected((current) => ({ ...current, [issueKey]: candidateIndex }))}
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
                    确认这项并继续
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
        {onCancel && <button type="button" className="quiet-button" onClick={onCancel} disabled={loading}>取消添加</button>}
      </div>
    </section>
  );
}
