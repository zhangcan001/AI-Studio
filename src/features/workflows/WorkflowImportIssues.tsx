import { useEffect, useState } from "react";
import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowImportCommitAction,
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

function isExistingRecipeOutdated(plan: WorkflowAutoOnboardingPlanView): boolean {
  return plan.issues.some((issue) => issue.code.trim().toUpperCase() === "EXISTING_RECIPE_OUTDATED");
}

function existingWorkflowName(plan: WorkflowAutoOnboardingPlanView): string {
  return plan.existingWorkflowName?.trim()
    || plan.metadata.name?.trim()
    || plan.existingPackageName?.trim()
    || plan.existingWorkflowId
    || "现有工作流";
}

function existingWorkflowSource(plan: WorkflowAutoOnboardingPlanView): string {
  const source = (plan.existingWorkflowSourceKind ?? plan.existingWorkflowSource)?.trim();
  if (source) {
    const normalized = source.toUpperCase();
    if (normalized.includes("BUILTIN") || normalized.includes("PRODUCT") || source.includes("内置")) return "系统自带";
    if (normalized.includes("USER") || normalized.includes("IMPORT") || source.includes("用户")) return "用户导入";
    return source;
  }
  return "现有工作流";
}

function matchTypeLabel(matchType?: string): string | undefined {
  const normalized = matchType?.trim().toUpperCase();
  if (normalized === "SEMANTIC_SHA") return "语义匹配";
  if (normalized === "RAW_SHA") return "原始 SHA 匹配";
  if (normalized === "STRUCTURAL_SHA") return "结构相似";
  return matchType?.trim() || undefined;
}

function recognitionIdentity(plan: WorkflowAutoOnboardingPlanView): string | undefined {
  return plan.identity?.trim().toUpperCase() || plan.recognition?.identity;
}

function isExactDuplicate(plan: WorkflowAutoOnboardingPlanView): boolean {
  const identity = recognitionIdentity(plan);
  return identity === "EXACT_RAW"
    || identity === "EXACT_SEMANTIC"
    || plan.state === "ALREADY_EXISTS"
    || plan.state === "ALREADY_EXISTS_ARCHIVED";
}

function isStructuralVariant(plan: WorkflowAutoOnboardingPlanView): boolean {
  return recognitionIdentity(plan) === "STRUCTURAL_VARIANT";
}

function isArchivedDuplicate(plan: WorkflowAutoOnboardingPlanView): boolean {
  const state = (plan.existingWorkflowLibraryState ?? "").trim().toUpperCase();
  return plan.state === "ALREADY_EXISTS_ARCHIVED" || state === "REMOVED";
}

function canCommitImport(plan: WorkflowAutoOnboardingPlanView): boolean {
  return Boolean(
    plan.autoPublishable
      || plan.validation.readyToPublish
      || plan.importability?.trim().toUpperCase() === "IMPORTABLE"
      || plan.recognition?.importable,
  );
}

function analysisInputLabels(plan: WorkflowAutoOnboardingPlanView): string {
  const labels = plan.inputMappings.map((mapping) => mapping.label || mapping.semanticKey);
  if (labels.length) return labels.join("、");
  const inferred = plan.inferences.map((inference) => inference.field).filter(Boolean);
  return inferred.length ? inferred.join("、") : "未识别到可配置输入";
}

function analysisOutputLabels(plan: WorkflowAutoOnboardingPlanView): string {
  const labels = plan.outputMappings.map((mapping) => mapping.type === "video" ? "视频" : "图片");
  const outputs = plan.recognition?.outputs?.map((output) => output.type === "video" ? "视频" : "图片") ?? [];
  return [...new Set([...labels, ...outputs])].join("、") || "待确认";
}

function suggestedRecipeVersion(plan: WorkflowAutoOnboardingPlanView): string {
  if (plan.suggestedRecipeVersion?.trim()) return plan.suggestedRecipeVersion.trim();
  const versions = (plan.existingRecipes ?? [])
    .map((recipe) => recipe.recipeVersion.trim())
    .filter((version) => /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version));
  const current = versions.sort((left, right) => left.localeCompare(right, undefined, { numeric: true }))[versions.length - 1]
    ?? plan.metadata.recipeVersion;
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(current.trim());
  return match ? `${match[1]}.${match[2]}.${Number(match[3]) + 1}` : "下一版本";
}

function existingRecipeLabel(plan: WorkflowAutoOnboardingPlanView): string {
  const recipes = (plan.existingRecipes ?? [])
    .map((recipe) => `${recipe.recipeVersion} · ${recipe.recipeId}`);
  if (recipes.length) return recipes.join("、");
  return `${plan.metadata.recipeVersion} · ${plan.metadata.recipeId}`;
}

interface Props {
  plan: WorkflowAutoOnboardingPlanView;
  loading: boolean;
  onResolve: (issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) => void;
  onResume: () => void;
  onOpenAdvanced: () => void;
  onOpenExisting: () => void;
  onOpenExistingVersion?: () => void;
  onUseInProject?: (workflowId: string, recipeId: string) => void;
  onRegenerateRecipe?: () => void;
  onRestoreExisting?: () => void;
  onCommitImport?: (action: WorkflowImportCommitAction) => void;
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
  if (isExistingRecipeOutdated(plan)) {
    if (plan.capability.state === "COMFY_OFFLINE") return "已识别为现有工作流，但当前 ComfyUI 离线；连接 ComfyUI 后可重新生成 Recipe。";
    if (plan.capability.state === "MISSING_NODES" || plan.capability.state === "INCOMPATIBLE_INPUT_VALUES") return "已识别为现有工作流，但当前 ComfyUI 依赖尚未满足；修复节点或输入后可重新生成 Recipe。";
    return "已识别为现有工作流，当前 Recipe 需要升级。重新生成只会新增 Recipe，不会修改原工作流或旧 Recipe。";
  }
  if (isStructuralVariant(plan)) return "检测到一个结构相似的工作流。它可能只是参数不同，不会自动合并，请选择如何保存。";
  if (isArchivedDuplicate(plan)) return "该工作流已删除。可以恢复现有工作流，不会创建重复数据。";
  if (isExactDuplicate(plan)) return "该工作流已经存在。不会创建重复数据。";
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
    case "EXISTING_RECIPE_OUTDATED":
      return "现有 Recipe 需要升级";
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

export function WorkflowImportIssues({ plan, draft, loading, onResolve, onResume, onOpenAdvanced, onOpenExisting, onOpenExistingVersion, onUseInProject, onRegenerateRecipe, onRestoreExisting, onCommitImport, onCancel }: Props) {
  const [selected, setSelected] = useState<Record<string, number>>({});
  const waiting = plan.state === "WAITING_FOR_COMFY_UI";
  const outdated = isExistingRecipeOutdated(plan);
  const exactDuplicate = isExactDuplicate(plan);
  const archivedDuplicate = isArchivedDuplicate(plan);
  const structuralVariant = isStructuralVariant(plan);
  const exactActive = exactDuplicate && !archivedDuplicate;
  const importable = canCommitImport(plan);
  const hasExistingWorkflow = Boolean(plan.existingWorkflowId || plan.existingPackageName || exactDuplicate);
  const matchType = matchTypeLabel(plan.existingMatchType);
  const issueFingerprint = plan.issues.map((issue, issueIndex) => workflowIssueSelectionKey(issue, issueIndex)).join("|");
  useEffect(() => setSelected({}), [plan.draftId, plan.state, issueFingerprint]);
  return (
    <section className="workflow-smart-issues" aria-label="工作流导入问题">
      <div className="workflow-smart-issues-heading">
        <div>
          <span className="section-label">添加工作流</span>
          <h3>{outdated ? "检测到现有工作流，需要更新配置" : structuralVariant ? "发现结构相似工作流" : archivedDuplicate ? "该工作流已删除" : exactActive ? "该工作流已经存在" : waiting ? "等待 ComfyUI 连接" : plan.state === "BLOCKED" ? "工作流可以保存，但当前不能运行" : "识别完成"}</h3>
          <p>{planMessage(plan)}</p>
        </div>
        <span className={`workflow-smart-state workflow-smart-state-${plan.state.toLowerCase()}`}>{outdated ? (waiting || plan.capability.state === "COMFY_OFFLINE" ? "等待连接" : "需要升级") : archivedDuplicate ? "已删除" : exactActive ? "已存在" : structuralVariant ? "需选择" : waiting ? "等待中" : importable ? "可添加" : plan.state === "BLOCKED" ? "暂不可用" : "待确认"}</span>
      </div>
      {hasExistingWorkflow && (
        <div className="workflow-import-guidance" aria-label="现有工作流信息">
          <strong>{structuralVariant ? "结构相似工作流" : "现有工作流信息"}</strong>
          <div className="workflow-detail-grid">
            <span>工作流名称<strong>{existingWorkflowName(plan)}</strong></span>
            <span>来源<strong>{existingWorkflowSource(plan)}</strong></span>
            <span>工作流版本<strong>{plan.existingWorkflowVersion ?? "—"}</strong></span>
            <span>旧 Recipe<strong>{existingRecipeLabel(plan)}</strong></span>
            {outdated && <span>建议新版本<strong>{suggestedRecipeVersion(plan)}</strong></span>}
          </div>
          {structuralVariant
            ? <p>结构相似只用于提示，不会自动合并或覆盖现有工作流。</p>
            : archivedDuplicate
              ? <p>恢复只会激活工作流，不会恢复项目绑定。</p>
              : <p>更新配置只会新增版本，不会修改系统自带工作流或旧配置。</p>}
          {!!(plan.structuralChanges ?? plan.recognition?.structuralChanges)?.length && (
            <ul className="workflow-issue-list">
              {(plan.structuralChanges ?? plan.recognition?.structuralChanges ?? []).map((change, index) => <li key={`${change.message}-${index}`}>{change.message}</li>)}
            </ul>
          )}
          {matchType && (
            <details className="technical-error-details" open>
              <summary>技术详情</summary>
              <code>{`matchType=${matchType}`}</code>
            </details>
          )}
        </div>
      )}
      {!!plan.issues.length && (
        <div className="workflow-smart-issue-list">
          {plan.issues.map((issue, issueIndex) => {
            const issueKey = workflowIssueSelectionKey(issue, issueIndex);
            const choice = selected[issueKey] ?? -1;
            const details = issueCapabilityDetails(plan, issue, draft);
            const normalizedCode = issue.code.trim().toUpperCase();
            const showCandidateFallback = normalizedCode !== "MISSING_NODES"
              && normalizedCode !== "MISSING_NODE"
              && normalizedCode !== "EXISTING_RECIPE_OUTDATED"
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
      {!hasExistingWorkflow && (
        <div className="workflow-import-guidance" aria-label="工作流识别结果">
          <strong>识别完成</strong>
          <div className="workflow-detail-grid">
            <span>名称<strong>{plan.metadata.name}</strong></span>
            <span>类型<strong>{plan.workflowKind}</strong></span>
            <span>模式<strong>{plan.metadata.mode}</strong></span>
            <span>识别输入<strong>{analysisInputLabels(plan)}</strong></span>
            <span>输出<strong>{analysisOutputLabels(plan)}</strong></span>
            <span>运行能力<strong>{plan.capability.state === "READY" ? "当前可运行" : "可保存，运行前需处理"}</strong></span>
          </div>
        </div>
      )}
      <div className="workflow-smart-actions">
        {waiting && <button type="button" onClick={onResume} disabled={loading}>{loading ? "正在检查..." : "继续自动确认"}</button>}
        {exactActive && <button type="button" onClick={onOpenExisting}>打开工作流</button>}
        {exactActive && plan.existingWorkflowId && onUseInProject && <button type="button" onClick={() => onUseInProject(plan.existingWorkflowId!, plan.existingRecipes?.[0]?.recipeId ?? plan.metadata.recipeId)}>用于当前项目</button>}
        {structuralVariant && <button type="button" onClick={() => onCommitImport ? onCommitImport("NEW_WORKFLOW") : onOpenAdvanced()}>添加为新工作流</button>}
        {structuralVariant && (onCommitImport || onOpenExistingVersion) && <button type="button" className="quiet-button" onClick={() => onCommitImport ? onCommitImport("NEW_VERSION") : onOpenExistingVersion?.()}>作为新版本添加</button>}
        {outdated && onRegenerateRecipe && <button type="button" onClick={onRegenerateRecipe} disabled={loading}>更新工作流配置</button>}
        {archivedDuplicate && onRestoreExisting && <button type="button" onClick={onRestoreExisting} disabled={loading}>恢复工作流</button>}
        {!exactDuplicate && !structuralVariant && !archivedDuplicate && onCommitImport && <button type="button" onClick={() => onCommitImport("NEW_WORKFLOW")} disabled={loading || !importable}>添加工作流</button>}
        <button type="button" className="quiet-button" onClick={onOpenAdvanced}>高级编辑</button>
        {onCancel && <button type="button" className="quiet-button" onClick={onCancel} disabled={loading}>取消添加</button>}
      </div>
    </section>
  );
}
