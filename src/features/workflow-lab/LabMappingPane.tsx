import { useEffect,useMemo,useState } from "react";
import type { WorkflowFieldType,WorkflowInputView,WorkflowNodeView,WorkflowOnboardingDraftView,WorkflowProductionWorkspaceView } from "../../types/workflowOnboarding";
import { isDangerousParameterName,isExposableWorkflowInput,mappingKey,mappingToDraft,parameterFieldTypes,supportedParameterFieldType,type MappingDraft,type ParameterMappingEdit } from "../workflows/workflowParameterExposureModel";
import { fieldTypeLabel,SeedModeSelect } from "./labViewHelpers";
interface ParameterExposurePaneProps {
  draft: WorkflowOnboardingDraftView;
  workflow: WorkflowProductionWorkspaceView;
  originalKeys: string[];
  loading: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onExpose: (nodeId: string, input: WorkflowInputView) => void;
  onSaveMapping: (mapping: MappingDraft, nodeId: string, inputName: string) => void;
  onRemove: (mapping: WorkflowOnboardingDraftView["inputMappings"][number]) => void;
  onSave: (edits: ParameterMappingEdit[]) => void;
}

export function ParameterExposurePane({
  draft,
  workflow,
  originalKeys,
  loading,
  onClose,
  onRefresh,
  onExpose,
  onSaveMapping,
  onRemove,
  onSave,
}: ParameterExposurePaneProps) {
  const [search, setSearch] = useState("");
  const [edits, setEdits] = useState<Record<string, MappingDraft>>({});

  useEffect(() => {
    setEdits(Object.fromEntries(draft.inputMappings.map((mapping) => [
      mappingKey(mapping.targetNode, mapping.targetInput),
      mappingToDraft(mapping),
    ])));
  }, [draft.inputMappings]);

  const mappingsByTarget = useMemo(
    () => new Set(draft.inputMappings.map((mapping) => mappingKey(mapping.targetNode, mapping.targetInput))),
    [draft.inputMappings],
  );
  const matches = (node: WorkflowNodeView, input: WorkflowInputView) => {
    const needle = search.trim().toLowerCase();
    if (!needle) return true;
    return `${node.nodeId} ${node.classType} ${node.title} ${input.name} ${input.suggestedSemanticKey ?? ""}`.toLowerCase().includes(needle);
  };
  const candidates = draft.nodes.flatMap((node) => node.inputs
    .filter((input) => matches(node, input)
      && !mappingsByTarget.has(mappingKey(node.nodeId, input.name))
      && isExposableWorkflowInput(input))
    .map((input) => ({ node, input })));
  const internal = draft.nodes.flatMap((node) => node.inputs
    .filter((input) => matches(node, input)
      && !mappingsByTarget.has(mappingKey(node.nodeId, input.name))
      && !isExposableWorkflowInput(input))
    .map((input) => ({ node, input })));
  const newKeys = new Set(draft.inputMappings.map((mapping) => mapping.semanticKey));
  const addedKeys = draft.inputMappings.map((mapping) => mapping.semanticKey).filter((key) => !originalKeys.includes(key));
  const removedKeys = originalKeys.filter((key) => !newKeys.has(key));
  const currentRecipe = workflow.recipes[workflow.recipes.length - 1];

  function patchMapping(mapping: WorkflowOnboardingDraftView["inputMappings"][number], patch: Partial<MappingDraft>) {
    const key = mappingKey(mapping.targetNode, mapping.targetInput);
    setEdits((current) => ({ ...current, [key]: { ...(current[key] ?? mappingToDraft(mapping)), ...patch } }));
  }

  return (
    <section className="workflow-parameter-exposure" aria-label="工作流生产参数">
      <header className="workflow-parameter-header">
        <div>
          <span className="section-label">工作流参数暴露</span>
          <h3>{workflow.name ?? workflow.packageName}</h3>
          <p className="section-description">只修改配方参数暴露，不修改工作流 API 图结构；内置运行包也只会复制为新配方。</p>
        </div>
        <div className="workflow-smart-actions">
          <button type="button" className="quiet-button" onClick={onRefresh} disabled={loading}>{loading ? "读取中..." : "刷新 ComfyUI 参数"}</button>
          <button type="button" className="quiet-button" onClick={onClose} disabled={loading}>取消</button>
          <button type="button" onClick={() => onSave(draft.inputMappings.flatMap((mapping) => {
            const edit = edits[mappingKey(mapping.targetNode, mapping.targetInput)];
            return edit ? [{ mapping, draft: edit }] : [];
        }))} disabled={loading}>{loading ? "保存中..." : "保存为新配方"}</button>
        </div>
      </header>

      <div className="workflow-parameter-summary">
        <span>工作流版本<strong>{workflow.workflowVersion ?? "—"}</strong></span>
        <span>当前配方<strong>{currentRecipe?.version ?? "—"} · {currentRecipe?.inputCount ?? 0} 项</strong></span>
        <span>新配方<strong>{draft.manifest.recipeVersion} · {draft.inputMappings.length} 项</strong></span>
        <span>图结构 SHA-256<strong>{draft.workflowSha256.slice(0, 16)}…</strong></span>
      </div>

      <div className="workflow-parameter-preview">
        <span>保存预览</span>
        <strong>{draft.inputMappings.length} 个生产参数</strong>
        <small>新增：{addedKeys.length ? addedKeys.join("、") : "无"}</small>
        <small>删除：{removedKeys.length ? removedKeys.join("、") : "无"}</small>
        <small>发布后直接使用现有预设 / 默认预设系统；旧配方的预设保持原作用域，不自动改写。</small>
      </div>

      <label className="workflow-parameter-search">搜索节点 / 输入 / 语义键
        <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="例如 steps、KSampler、节点 88" />
      </label>

      <section className="workflow-parameter-section">
        <div className="workflow-parameter-section-heading"><div><span className="section-label">已暴露参数</span><h4>{draft.inputMappings.length} 项</h4></div><small>修改只作用于新配方草稿</small></div>
        {draft.inputMappings.length ? draft.inputMappings.map((mapping) => {
          const key = mappingKey(mapping.targetNode, mapping.targetInput);
          const edit = edits[key] ?? mappingToDraft(mapping);
          return (
            <div className="workflow-parameter-field" key={`${mapping.semanticKey}:${mapping.itemIndex ?? ""}`}>
              <div className="workflow-parameter-field-heading"><div><strong>{mapping.label}</strong><code>{mapping.semanticKey}</code></div><span>节点 {mapping.targetNode} · {mapping.targetInput}</span><button type="button" className="quiet-button danger-button" onClick={() => onRemove(mapping)} disabled={loading}>移除</button></div>
              <div className="workflow-parameter-form">
                <label>显示名称<input value={edit.label} onChange={(event) => patchMapping(mapping, { label: event.target.value })} /></label>
                <label>语义键<input value={edit.semanticKey} onChange={(event) => patchMapping(mapping, { semanticKey: event.target.value })} /></label>
                <label>类型<select value={edit.fieldType} onChange={(event) => patchMapping(mapping, { fieldType: event.target.value as WorkflowFieldType })}>{parameterFieldTypes.map((type) => <option key={type} value={type}>{fieldTypeLabel(type)}</option>)}</select></label>
                <label className="checkbox-label"><input type="checkbox" checked={edit.required} onChange={(event) => patchMapping(mapping, { required: event.target.checked })} /> 必填</label>
                {(edit.fieldType === "textarea" || edit.fieldType === "integer" || edit.fieldType === "number" || edit.fieldType === "seed") && <label>默认值<input value={edit.defaultValue} onChange={(event) => patchMapping(mapping, { defaultValue: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : undefined} /></label>}
                {edit.fieldType === "seed" && <SeedModeSelect value={edit.seedMode} defaultValue={edit.defaultValue} onChange={(seedMode) => patchMapping(mapping, { seedMode })} />}
                {(edit.fieldType === "integer" || edit.fieldType === "number" || edit.fieldType === "seed") && <>
                  <label>最小值<input value={edit.minValue} onChange={(event) => patchMapping(mapping, { minValue: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : "numeric"} /></label>
                  <label>最大值<input value={edit.maxValue} onChange={(event) => patchMapping(mapping, { maxValue: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : "numeric"} /></label>
                </>}
                {(edit.fieldType === "integer" || edit.fieldType === "number") && <label>步长<input value={edit.step} onChange={(event) => patchMapping(mapping, { step: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : "numeric"} /></label>}
                {edit.fieldType.endsWith("s") && <label>最大数量<input value={edit.maxItems} onChange={(event) => patchMapping(mapping, { maxItems: event.target.value })} inputMode="numeric" /></label>}
                <button type="button" onClick={() => onSaveMapping(edit, mapping.targetNode, mapping.targetInput)} disabled={loading}>保存字段</button>
              </div>
            </div>
          );
        }) : <p className="disabled-note">当前配方没有可编辑的输入映射。</p>}
      </section>

      <section className="workflow-parameter-section">
        <div className="workflow-parameter-section-heading"><div><span className="section-label">可暴露参数</span><h4>{candidates.length} 项匹配</h4></div><small>仅显示字面量、可绑定且属于现有安全字段类型的输入</small></div>
        {candidates.map(({ node, input }) => {
          const fieldType = supportedParameterFieldType(input);
          return <div className="workflow-parameter-candidate" key={`${node.nodeId}:${input.name}`}><div><strong>节点 {node.nodeId} · {node.classType}</strong><span>{input.name} · 当前值 {input.currentValueSummary}</span><small>建议：{input.suggestedSemanticKey ?? "—"} · {fieldType ? fieldTypeLabel(fieldType) : "未支持"}</small></div><button type="button" onClick={() => onExpose(node.nodeId, input)} disabled={loading || !fieldType}>暴露</button></div>;
        })}
        {!candidates.length && <p className="disabled-note">没有匹配的可暴露字面量输入。</p>}
      </section>

      <details className="workflow-parameter-section workflow-parameter-internal">
        <summary><span className="section-label">工作流内部参数</span><strong>{internal.length} 项</strong></summary>
        <p className="disabled-note">链接输入、危险路径/模型/设备输入和暂不支持的字段类型保持内部状态，不会写入配方。</p>
        {internal.map(({ node, input }) => <div className="workflow-parameter-internal-row" key={`${node.nodeId}:${input.name}`}><span>节点 {node.nodeId} · {node.classType}</span><strong>{input.name}</strong><small>{input.isLinked ? "内部连接 · 不可作为生产参数" : isDangerousParameterName(input.name) ? "危险输入 · 后端禁止暴露" : "当前字段类型暂不支持"}</small></div>)}
      </details>
    </section>
  );
}
