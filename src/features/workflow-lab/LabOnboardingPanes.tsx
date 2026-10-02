import { formatDateTime } from "../../i18n/statusLabels";
import type { WorkflowFieldType,WorkflowInputView,WorkflowNodeView,WorkflowOnboardingDraftView } from "../../types/workflowOnboarding";
import type { MetadataDraft,OutputDraft } from "../workflows/hooks/useWorkflowAdvancedOnboardingController";
import { defaultMapping,isExposableWorkflowInput,localizeWorkflowIssue,mappingKey,parameterFieldTypes,type MappingDraft } from "../workflows/workflowParameterExposureModel";
import { fieldTypeLabel,formatCapability,IssueList,SeedModeSelect } from "./labViewHelpers";
export function InspectPane({ draft, onContinue }: { draft: WorkflowOnboardingDraftView; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-stats"><span>SHA-256 <strong>{draft.workflowSha256}</strong></span><span>节点数量 <strong>{draft.nodeCount}</strong></span><span>节点类型数量 <strong>{draft.uniqueClassCount}</strong></span></div>
      <p className="section-description">节点 ID、节点类型和映射仅在此导入向导中作为技术信息显示。</p>
      <div className="workflow-node-list">
        {draft.nodes.map((node) => <NodeCard key={node.nodeId} node={node} />)}
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>检查兼容性</button></div>
    </div>
  );
}

export function NodeCard({ node }: { node: WorkflowNodeView }) {
  return (
    <details className="workflow-node-card">
      <summary><strong>节点 {node.nodeId}</strong><span>{node.title}</span><code>{node.classType}</code></summary>
      <div className="workflow-node-inputs">
        {node.inputs.map((input) => <div key={input.name}><span>{input.name}</span><small>{input.currentValueSummary}{input.isLinked ? " · 已连接" : ""}</small></div>)}
        {!node.inputs.length && <small>暂无字面量输入。</small>}
      </div>
    </details>
  );
}

export function CompatibilityPane({ draft, loading, onCheck, onContinue }: { draft: WorkflowOnboardingDraftView; loading: boolean; onCheck: () => void; onContinue: () => void }) {
  const capability = draft.capability;
  return (
    <div className="workflow-onboarding-pane">
      <div className={`workflow-capability-banner workflow-capability-${capability.state.toLowerCase()}`}>
        <strong>{formatCapability(capability.state)}</strong>
        <span>{capability.checkedAt ? `检查时间：${formatDateTime(capability.checkedAt)}` : "尚未检查兼容状态。"}</span>
      </div>
      <button type="button" onClick={onCheck} disabled={loading}>{loading ? "正在检查..." : "检查 ComfyUI 兼容状态"}</button>
      {!!capability.issues.length && <IssueList issues={capability.issues} />}
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>配置输入</button></div>
    </div>
  );
}

export function InputsPane({
  draft,
  mappingDrafts,
  onPatch,
  onBind,
  onRemove,
  onContinue,
}: {
  draft: WorkflowOnboardingDraftView;
  mappingDrafts: Record<string, MappingDraft>;
  onPatch: (key: string, patch: Partial<MappingDraft>) => void;
  onBind: (nodeId: string, input: WorkflowInputView) => void;
  onRemove: (mapping: WorkflowOnboardingDraftView["inputMappings"][number]) => void;
  onContinue: () => void;
}) {
  return (
    <div className="workflow-onboarding-pane">
      <p className="section-description">选择语义字段并确认每项映射。已连接的输入默认保持原图连接，也可以手动映射并在执行时覆盖。</p>
      <div className="workflow-input-list">
        {draft.nodes.flatMap((node) => node.inputs.map((input) => {
          const key = mappingKey(node.nodeId, input.name);
          const mapping = mappingDrafts[key] ?? defaultMapping(node.nodeId, input);
          const existing = draft.inputMappings.find((candidate) => candidate.targetNode === node.nodeId && candidate.targetInput === input.name);
          return (
            <div className="workflow-input-card" key={key}>
              <div className="workflow-input-heading"><strong>{node.nodeId}.{input.name}</strong><span>{input.currentValueSummary}</span></div>
              {input.isLinked && <p className="field-hint">此参数在执行时会覆盖当前节点连接输入，原始工作流不会修改。</p>}
              {(input.bindable || (input.isLinked && isExposableWorkflowInput(input))) ? (
                <div className="workflow-mapping-form">
                  <label>语义键<input value={mapping.semanticKey} onChange={(event) => onPatch(key, { semanticKey: event.target.value })} /></label>
                  <label>字段类型<select value={mapping.fieldType} onChange={(event) => onPatch(key, { fieldType: event.target.value as WorkflowFieldType })}>{parameterFieldTypes.map((type) => <option key={type} value={type}>{fieldTypeLabel(type)}</option>)}</select></label>
                  <label>显示名称<input value={mapping.label} onChange={(event) => onPatch(key, { label: event.target.value })} /></label>
                  <label className="checkbox-label"><input type="checkbox" checked={mapping.required} onChange={(event) => onPatch(key, { required: event.target.checked })} /> 必填</label>
                  {mapping.fieldType === "integer" || mapping.fieldType === "number" || mapping.fieldType === "seed" ? <>
                    <label>默认值<input value={mapping.defaultValue} onChange={(event) => onPatch(key, { defaultValue: event.target.value })} /></label>
                    {mapping.fieldType === "seed" && <SeedModeSelect value={mapping.seedMode} defaultValue={mapping.defaultValue} onChange={(seedMode) => onPatch(key, { seedMode })} />}
                    <label>最小值<input value={mapping.minValue} onChange={(event) => onPatch(key, { minValue: event.target.value })} inputMode={mapping.fieldType === "number" ? "decimal" : "numeric"} /></label>
                    <label>最大值<input value={mapping.maxValue} onChange={(event) => onPatch(key, { maxValue: event.target.value })} inputMode={mapping.fieldType === "number" ? "decimal" : "numeric"} /></label>
                  </> : null}
                  {mapping.fieldType === "integer" || mapping.fieldType === "number" ? <label>步长<input value={mapping.step} onChange={(event) => onPatch(key, { step: event.target.value })} inputMode={mapping.fieldType === "number" ? "decimal" : "numeric"} /></label> : null}
                  {mapping.fieldType.endsWith("s") ? <label>最大数量<input value={mapping.maxItems} onChange={(event) => onPatch(key, { maxItems: event.target.value })} inputMode="numeric" /></label> : null}
                  <button type="button" onClick={() => onBind(node.nodeId, input)} disabled={!input.bindable && !input.isLinked}>确认映射</button>
                </div>
              ) : <p className="disabled-note">当前输入不能作为生产参数。</p>}
              {input.allowedOptions.length > 0 && <small className="field-hint">可用选项：{input.allowedOptions.join(", ")}</small>}
              {existing && <div className="workflow-existing-mapping"><span>已映射为 <strong>{existing.label}</strong></span><button type="button" className="quiet-button" onClick={() => onRemove(existing)}>移除</button></div>}
            </div>
          );
        }))}
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>配置输出</button></div>
    </div>
  );
}

export function OutputsPane({ draft, candidates, outputDraft, onChange, onAdd, onContinue }: { draft: WorkflowOnboardingDraftView; candidates: WorkflowNodeView[]; outputDraft: OutputDraft; onChange: (value: OutputDraft) => void; onAdd: () => void; onContinue: () => void }) {
  const selectedIsCandidate = candidates.some(node => node.nodeId === outputDraft.nodeId);
  return (
    <div className="workflow-onboarding-pane">
      <p className="section-description">明确声明用户可见的输出结果。输出 ID 是运行包使用的稳定 snake_case 技术键。</p>
      <div className="workflow-mapping-form workflow-output-form">
        <label>输出 ID<input value={outputDraft.outputId} onChange={(event) => onChange({ ...outputDraft, outputId: event.target.value })} /></label>
        <label>显示名称<input value={outputDraft.label} onChange={(event) => onChange({ ...outputDraft, label: event.target.value })} /></label>
        <label>类型<select value={outputDraft.type} onChange={(event) => onChange({ ...outputDraft, type: event.target.value as "image" | "video" | "audio" })}><option value="image">图片</option><option value="video">视频</option><option value="audio">音频</option></select></label>
        <label>输出节点<select value={selectedIsCandidate ? outputDraft.nodeId : ""} onChange={(event) => onChange({ ...outputDraft, nodeId: event.target.value })}><option value="">请选择有效输出节点</option>{candidates.map((node) => <option key={node.nodeId} value={node.nodeId}>节点 {node.nodeId} · {node.classType}</option>)}</select></label>
        <label className="checkbox-label"><input type="checkbox" checked={outputDraft.required} onChange={(event) => onChange({ ...outputDraft, required: event.target.checked })} /> 必填</label>
        <button type="button" onClick={onAdd} disabled={!selectedIsCandidate}>确认输出</button>
      </div>
      <div className="workflow-output-list">{draft.outputMappings.map((output) => <div key={output.outputId}><strong>{output.label}</strong><span>{output.outputId} · {output.type === "video" ? "视频" : output.type === "audio" ? "音频" : "图片"} · 节点 {output.nodeId}</span></div>)}</div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>设置基本信息</button></div>
    </div>
  );
}

export function MetadataPane({ draft, onChange, onSave, onContinue }: { draft: MetadataDraft; onChange: (value: MetadataDraft) => void; onSave: () => void; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-mapping-form workflow-metadata-form">
        <label>工作流 ID<input value={draft.workflowId} onChange={(event) => onChange({ ...draft, workflowId: event.target.value })} /></label>
        <label>名称<input value={draft.name} onChange={(event) => onChange({ ...draft, name: event.target.value })} /></label>
        <label>工作流版本<input value={draft.workflowVersion} onChange={(event) => onChange({ ...draft, workflowVersion: event.target.value })} /></label>
        <label>配方版本<input value={draft.recipeVersion} onChange={(event) => onChange({ ...draft, recipeVersion: event.target.value })} /></label>
        <label>分类<input value={draft.category} onChange={(event) => onChange({ ...draft, category: event.target.value })} /></label>
        <label>模式<input value={draft.mode} onChange={(event) => onChange({ ...draft, mode: event.target.value })} /></label>
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onSave}>保存基本信息</button><button type="button" onClick={onContinue}>校验草稿</button></div>
    </div>
  );
}

export function ValidatePane({ draft, loading, onValidate, onPublish }: { draft: WorkflowOnboardingDraftView; loading: boolean; onValidate: () => void; onPublish: () => void }) {
  const validation = draft.validation;
  const checks = [
    ["API 格式", validation.apiFormat],
    ["配方", validation.recipe],
    ["输入绑定", validation.bindings],
    ["输出", validation.outputs],
    ["清单", validation.manifest],
    ["兼容状态", validation.capability],
    ["试运行", validation.dryRun],
  ] as const;
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-validation-grid">{checks.map(([label, valid]) => <span key={label} className={valid ? "workflow-check-pass" : "workflow-check-fail"}>{valid ? "✓" : "!"} {label}</span>)}</div>
      {!!validation.issues.length && <ul className="workflow-issue-list">{validation.issues.map((issue) => <li key={issue}>{localizeWorkflowIssue(issue)}</li>)}</ul>}
      <details className="workflow-recipe-preview"><summary>高级信息：生成的配方配置</summary><pre>{draft.recipe.yaml ?? "配方当前还未通过校验。"}</pre></details>
      <div className="workflow-pane-actions"><button type="button" onClick={onValidate} disabled={loading}>{loading ? "正在校验..." : "再次校验"}</button><button type="button" onClick={onPublish} disabled={!validation.readyToPublish}>继续发布</button></div>
    </div>
  );
}

export function PublishPane({ draft, published, loading, onPublish, onOpenStudio }: { draft: WorkflowOnboardingDraftView; published?: { workflowId: string; recipeId: string }; loading: boolean; onPublish: () => void; onOpenStudio?: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className={`workflow-publish-state ${draft.validation.readyToPublish ? "workflow-check-pass" : "workflow-check-fail"}`}>
        {draft.validation.readyToPublish ? "可以发布" : "所有检查通过后才能发布。"}
      </div>
      <button type="button" onClick={onPublish} disabled={loading || !draft.validation.readyToPublish}>{loading ? "正在发布..." : "发布工作流运行包"}</button>
      {published && <div className="workflow-published-result"><strong>发布成功</strong><span>刷新目录后即可在创作工作台使用该运行包。</span><button type="button" onClick={onOpenStudio}>在创作工作台中打开</button></div>}
    </div>
  );
}
