import { formatDateTime } from "../../i18n/statusLabels";
import type { WorkflowSavedVersionDetailsView,WorkflowVersionDiffView } from "../../types/workflowOnboarding";
export function SavedVersionDetailsPane({ details, onClose, onRun }: { details: WorkflowSavedVersionDetailsView; onClose: () => void; onRun: () => void }) {
  const recognition = details.recognition;
  return (
    <section className="workflow-saved-details workflow-diff-panel" role="region" aria-label="已保存工作流版本详情">
      <div className="workflow-smart-issues-heading">
        <div><span className="section-label">工作流库 · 已保存版本</span><h3>{details.name}</h3></div>
        <div><button type="button" onClick={onRun}>运行工作流</button><button type="button" className="quiet-button" onClick={onClose}>关闭</button></div>
      </div>
      <div className="workflow-detail-grid">
        <span>Workflow ID<strong><code>{details.workflowId}</code></strong></span>
        <span>WorkflowVersion ID<strong><code>{details.workflowVersionId}</code></strong></span>
        <span>版本<strong>{details.workflowVersion}</strong></span>
        <span>类型 / 分类<strong>{details.category}</strong></span>
        <span>模式<strong>{details.mode}</strong></span>
        <span>Workflow SHA-256<strong>{details.workflowSha256}</strong></span>
        <span>节点数量<strong>{storedWorkflowNodeCount(details.workflowJson)}</strong></span>
        <span>原始导入 payload<strong>{details.sourceWorkflowPreserved ? "已保留" : "旧版本无独立原始副本"}</strong></span>
        <span>识别引擎<strong>{recognition ? `${recognition.recognitionEngine} v${recognition.recognitionEngineVersion}` : "旧版本无识别元数据"}</strong></span>
        {recognition && <span>识别时间<strong>{formatDateTime(recognition.recognizedAt)}</strong></span>}
        {recognition && <span>Schema 来源<strong>{recognition.schemaSource}</strong></span>}
        {recognition && <span>语义就绪<strong>{recognition.semanticCapabilityStatus}</strong></span>}
        {recognition && <span>运行就绪<strong>{recognition.runtimeImportStatus}</strong></span>}
        {recognition && <span>输出根状态<strong>{recognition.outputRootState}</strong></span>}
      </div>
      {recognition?.roots.length ? <section className="workflow-registry-nested"><h4>输出根</h4><ul>{recognition.roots.map((root) => <li key={`${root.outputId}:${root.nodeId}`}>{root.outputId} · {root.outputType} · 节点 {root.nodeId} · 证据层级 {root.evidenceTier}</li>)}</ul></section> : null}
      {recognition?.evidenceSummary.length ? <section className="workflow-registry-nested"><h4>识别证据摘要</h4><ul>{recognition.evidenceSummary.map((evidence, index) => <li key={`${evidence.source}:${evidence.nodeId}:${evidence.target}:${index}`}>{evidence.source} · {evidence.kind} · {evidence.target} · 权重 {evidence.weight}</li>)}</ul></section> : null}
      {recognition?.inputMappingDecisions?.length ? <section className="workflow-registry-nested"><h4>输入映射确认</h4><ul>{recognition.inputMappingDecisions.map((decision) => <li key={`${decision.semanticKey}:${decision.itemIndex ?? 0}`}>{decision.semanticKey}{decision.itemIndex != null ? ` #${decision.itemIndex + 1}` : ""} · {decision.mappingSource} · {decision.inferredMapping ? `推断 节点 ${decision.inferredMapping.nodeId}.${decision.inferredMapping.inputName} → ` : ""}最终 节点 {decision.finalMapping.nodeId}.{decision.finalMapping.inputName}</li>)}</ul></section> : null}
      {recognition?.userOverride && <section className="workflow-registry-nested"><h4>用户确认 / 覆盖</h4><p>{recognition.userOverride.status}</p><ul>{recognition.userOverride.workflowType && <li>类型：{recognition.userOverride.workflowType.inferredValue} → {recognition.userOverride.workflowType.selectedValue}</li>}{recognition.userOverride.mode && <li>模式：{recognition.userOverride.mode.inferredValue} → {recognition.userOverride.mode.selectedValue}</li>}</ul></section>}
      {recognition?.runtimeBlockers.length ? <section className="workflow-registry-nested"><h4>运行阻塞项</h4><ul>{recognition.runtimeBlockers.map((blocker, index) => <li key={`${blocker.code}:${blocker.nodeId ?? ""}:${index}`}>{blocker.code}{blocker.classType ? ` · ${blocker.classType}` : ""}{blocker.nodeId ? ` · 节点 ${blocker.nodeId}` : ""}{blocker.inputName ? ` · 输入 ${blocker.inputName}` : ""}</li>)}</ul></section> : null}
      <p className="disabled-note">版本详情从本地工作流库读取；无需在线 ComfyUI。工作流 JSON 已加载用于复用，但为避免泄露参数内容，此处不展示原始 JSON。</p>
    </section>
  );
}

export function storedWorkflowNodeCount(workflow: unknown): number {
  return workflow && typeof workflow === "object" && !Array.isArray(workflow)
    ? Object.keys(workflow).length
    : 0;
}

export function VersionDiffPane({ diff, onClose }: { diff: WorkflowVersionDiffView; onClose: () => void }) {
  return (
    <details className="workflow-diff-panel" open>
      <summary>版本差异 · {diff.versionA} 对比 {diff.versionB}</summary>
      <div className="workflow-detail-grid">
        <span>节点数量 <strong>{diff.nodeCountA} → {diff.nodeCountB}</strong></span>
        <span>新增节点 <strong>{diff.addedNodes.length}</strong></span>
        <span>移除节点 <strong>{diff.removedNodes.length}</strong></span>
        <span>类型变化 <strong>{diff.changedClassTypes.length}</strong></span>
        <span>字面量变化 <strong>{diff.changedLiteralInputs.length}</strong></span>
        <span>连接变化 <strong>{diff.changedLinks.length}</strong></span>
      </div>
      {!!diff.addedNodes.length && <p>新增节点：{diff.addedNodes.join(", ")}</p>}
      {!!diff.removedNodes.length && <p>移除节点：{diff.removedNodes.join(", ")}</p>}
      {!!diff.changedClassTypes.length && <ul className="workflow-issue-list">{diff.changedClassTypes.map((change) => <li key={change.nodeId}>节点 {change.nodeId}：{change.from} → {change.to}</li>)}</ul>}
      {!!diff.changedLiteralInputs.length && <ul className="workflow-issue-list">{diff.changedLiteralInputs.map((change) => <li key={`${change.nodeId}:${change.input}`}>节点 {change.nodeId}.{change.input}：{change.from} → {change.to}</li>)}</ul>}
      {!!diff.changedLinks.length && <ul className="workflow-issue-list">{diff.changedLinks.map((change) => <li key={`${change.nodeId}:${change.input}`}>节点连接已变化：{change.nodeId}.{change.input}</li>)}</ul>}
      {!!diff.recipeInputChanges.length && <p>配方输入：{diff.recipeInputChanges.join("；")}</p>}
      {!!diff.bindingChanges.length && <p>输入绑定：{diff.bindingChanges.join("；")}</p>}
      {!!diff.outputChanges.length && <p>输出：{diff.outputChanges.join("；")}</p>}
      <button type="button" className="quiet-button" onClick={onClose}>关闭差异</button>
    </details>
  );
}
