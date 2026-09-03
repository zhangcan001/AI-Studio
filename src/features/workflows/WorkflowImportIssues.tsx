import { useEffect, useState } from "react";
import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowOnboardingDraftView,
} from "../../types/workflowOnboarding";

export type WorkflowImportErrorKind = "UI_FORMAT" | "INVALID_JSON" | "UNKNOWN_FORMAT" | "IMPORT_FAILED";

export interface WorkflowImportErrorView {
  kind: WorkflowImportErrorKind;
  message: string;
  code?: string;
  technicalMessage?: string;
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
  draft?: WorkflowOnboardingDraftView;
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
      : issue.kind === "UNKNOWN_FORMAT"
        ? "无法识别这个工作流"
        : "工作流未能导入";
  const technicalMessage = issue.technicalMessage?.trim();
  const hasTechnicalDetails = Boolean(technicalMessage && technicalMessage !== issue.message);

  return (
    <section className="workflow-smart-issues workflow-import-format-issue" aria-label="工作流添加结果" role="status">
      <div className="workflow-smart-issues-heading">
        <div>
          <span className="section-label">添加工作流</span>
          <h3>{title}</h3>
          <p>{issue.message}</p>
          {isUi && <p>请在 ComfyUI 中将该工作流导出为 API Format JSON，然后重新选择该文件。</p>}
          {hasTechnicalDetails && (
            <details className="technical-error-details">
              <summary>查看详细原因</summary>
              <code>{technicalMessage}</code>
            </details>
          )}
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
  switch (code.trim().toUpperCase()) {
    case "MISSING_NODES":
      return "当前环境缺少节点";
    case "MISSING_NODE":
      return "当前环境缺少节点";
    case "INPUT_OPTION_UNAVAILABLE":
      return "输入选项不可用";
    case "AMBIGUOUS_OUTPUT":
      return "检测到多个输出节点";
    case "AMBIGUOUS_DURATION_SOURCE":
      return "无法自动确认视频时长来源";
    case "FLOAT_INPUT_NEEDS_REVIEW":
      return "数值参数需要确认";
  }
  const normalized = code.trim().toUpperCase();
  if (normalized.includes("MULTIPLE_OUTPUT") || normalized.includes("OUTPUT_AMBIGUOUS")) return "检测到多个输出节点";
  if (normalized.includes("DURATION")) return "无法自动确认视频时长来源";
  if (normalized.includes("GRAPH") || normalized.includes("INFERENCE")) return "工作流图推断需要确认";
  if (normalized.includes("OUTPUT")) return "输出节点需要确认";
  if (normalized.includes("INPUT")) return "输入来源需要确认";
  return "参数用途需要确认";
}

function matchesCapabilityIssue(issueCode: string, capabilityCode: string): boolean {
  const issue = issueCode.trim().toUpperCase();
  const capability = capabilityCode.trim().toUpperCase();
  return issue === capability || (issue === "MISSING_NODES" && capability === "MISSING_NODE");
}

function issueCapabilityDetails(plan: WorkflowAutoOnboardingPlanView, issue: WorkflowAutoIssueView, draft?: WorkflowOnboardingDraftView) {
  const capabilityIssues = plan.capability.issues.filter((candidate) => (
    matchesCapabilityIssue(issue.code, candidate.code)
      && (!issue.field || candidate.inputName === issue.field)
  ));
  const nodeIds = new Set<string>();
  const inputNames = new Set<string>();
  const nodeTypes = new Set<string>();
  const currentValues = new Set<string>();
  const allowedOptions = new Set<string>();

  for (const candidate of issue.candidates) {
    if (candidate.nodeId) nodeIds.add(candidate.nodeId);
    if (candidate.inputName) inputNames.add(candidate.inputName);
  }
  for (const candidate of capabilityIssues) {
    if (candidate.nodeId) nodeIds.add(candidate.nodeId);
    candidate.affectedNodeIds.forEach((nodeId) => nodeIds.add(nodeId));
    if (candidate.inputName) inputNames.add(candidate.inputName);
    if (candidate.classType) nodeTypes.add(candidate.classType);
    if (candidate.currentValue) currentValues.add(candidate.currentValue);
  }
  for (const node of draft?.nodes ?? []) {
    if (!nodeIds.has(node.nodeId) || !inputNames.size) continue;
    for (const input of node.inputs) {
      if (inputNames.size && !inputNames.has(input.name)) continue;
      input.allowedOptions.forEach((option) => allowedOptions.add(option));
    }
  }
  const inferredCandidates = plan.inferences
    .filter((inference) => inference.field === issue.field || (issue.code.toUpperCase().includes("DURATION") && inference.field === "duration_seconds"))
    .flatMap((inference) => inference.alternatives);

  return {
    nodeIds: [...nodeIds],
    inputNames: [...inputNames],
    nodeTypes: [...nodeTypes],
    currentValues: [...currentValues],
    allowedOptions: [...allowedOptions],
    inferredCandidates: [...new Set(inferredCandidates)],
  };
}

export function WorkflowImportIssues({ plan, draft, loading, onResolve, onResume, onOpenAdvanced, onOpenExisting, onRestoreExisting, onCancel }: Props) {
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
            const details = issueCapabilityDetails(plan, issue, draft);
            const normalizedCode = issue.code.trim().toUpperCase();
            const showCandidateFallback = normalizedCode !== "MISSING_NODES"
              && normalizedCode !== "MISSING_NODE"
              && !issue.candidates.length
              && !details.allowedOptions.length
              && !details.inferredCandidates.length;
            return (
              <article className="workflow-smart-issue" key={issueKey}>
                <div>
                  <strong>{issueTitle(issue.code)}</strong>
                  <p>{issue.message}</p>
                  {issue.field && <small className="field-hint">字段：{issue.field}</small>}
                  {!!details.nodeIds.length && <small className="field-hint">节点：{details.nodeIds.join("、")}</small>}
                  {!!details.inputNames.length && <small className="field-hint">输入：{details.inputNames.join("、")}</small>}
                  {!!details.nodeTypes.length && <small className="field-hint">节点类型：{details.nodeTypes.join("、")}</small>}
                  {!!details.currentValues.length && <small className="field-hint">当前值：{details.currentValues.join("、")}</small>}
                  {!!details.allowedOptions.length && <small className="field-hint">候选：{details.allowedOptions.join("、")}</small>}
                  {!!details.inferredCandidates.length && <small className="field-hint">候选：{[...new Set(details.inferredCandidates)].join("、")}</small>}
                  {showCandidateFallback && <small className="disabled-note">候选：后端未返回候选项，请打开高级编辑查看可用节点或输入。</small>}
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
                        {(candidate.nodeId || candidate.inputName || candidate.outputType) && <small>{[candidate.nodeId && `节点 ${candidate.nodeId}`, candidate.inputName && `输入 ${candidate.inputName}`, candidate.outputType && `输出类型 ${candidate.outputType}`].filter(Boolean).join(" · ")}</small>}
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
