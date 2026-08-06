import { useCallback, useEffect, useMemo, useState } from "react";
import {
  checkOnboardingCapability,
  discardOnboarding,
  getOnboardingDraft,
  listWorkflowWorkspace,
  pickApiWorkflow,
  publishOnboarding,
  removeOnboardingInputMapping,
  setOnboardingInputMapping,
  setOnboardingMetadata,
  setOnboardingOutputMapping,
  validateOnboarding,
} from "../../services/tauriClient";
import { useWorkflowOnboardingStore, type WorkflowOnboardingStep } from "../../stores/workflowOnboardingStore";
import type {
  WorkflowFieldType,
  WorkflowInputView,
  WorkflowNodeView,
  WorkflowOnboardingDraftView,
  WorkflowOnboardingInputMappingRequest,
  WorkflowOnboardingOutputMappingRequest,
  WorkflowWorkspaceView,
} from "../../types/workflowOnboarding";

interface Props {
  onCatalogChanged: () => Promise<void>;
  onOpenStudio: (workflowId: string, recipeId: string) => Promise<void>;
}

const steps: Array<{ value: WorkflowOnboardingStep; label: string }> = [
  { value: "inspect", label: "Inspect" },
  { value: "compatibility", label: "Compatibility" },
  { value: "inputs", label: "Inputs" },
  { value: "outputs", label: "Outputs" },
  { value: "metadata", label: "Metadata" },
  { value: "validate", label: "Validate" },
  { value: "publish", label: "Publish" },
];

const fieldTypes: WorkflowFieldType[] = [
  "textarea",
  "integer",
  "seed",
  "image",
  "images",
  "video",
  "videos",
  "audio",
  "audios",
];

interface MappingDraft {
  semanticKey: string;
  fieldType: WorkflowFieldType;
  label: string;
  required: boolean;
  defaultValue: string;
  minValue: string;
  maxValue: string;
  minItems: string;
  maxItems: string;
  itemIndex: string;
}

interface OutputDraft {
  outputId: string;
  label: string;
  type: "image" | "video";
  nodeId: string;
  required: boolean;
}

interface MetadataDraft {
  workflowId: string;
  name: string;
  workflowVersion: string;
  recipeVersion: string;
  category: string;
  mode: string;
}

export function WorkflowWorkspace({ onCatalogChanged, onOpenStudio }: Props) {
  const [items, setItems] = useState<WorkflowWorkspaceView[]>([]);
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [workspaceError, setWorkspaceError] = useState<string>();
  const [mappingDrafts, setMappingDrafts] = useState<Record<string, MappingDraft>>({});
  const [outputDraft, setOutputDraft] = useState<OutputDraft>({
    outputId: "output_1",
    label: "Output",
    type: "image",
    nodeId: "",
    required: true,
  });
  const [metadataDraft, setMetadataDraft] = useState<MetadataDraft>();
  const [published, setPublished] = useState<{ workflowId: string; recipeId: string }>();
  const draft = useWorkflowOnboardingStore((state) => state.draft);
  const step = useWorkflowOnboardingStore((state) => state.step);
  const loading = useWorkflowOnboardingStore((state) => state.loading);
  const error = useWorkflowOnboardingStore((state) => state.error);
  const notice = useWorkflowOnboardingStore((state) => state.notice);
  const setDraft = useWorkflowOnboardingStore((state) => state.setDraft);
  const updateDraft = useWorkflowOnboardingStore((state) => state.updateDraft);
  const setStep = useWorkflowOnboardingStore((state) => state.setStep);
  const setLoading = useWorkflowOnboardingStore((state) => state.setLoading);
  const setError = useWorkflowOnboardingStore((state) => state.setError);
  const setNotice = useWorkflowOnboardingStore((state) => state.setNotice);
  const reset = useWorkflowOnboardingStore((state) => state.reset);

  const loadWorkspace = useCallback(async () => {
    setWorkspaceLoading(true);
    setWorkspaceError(undefined);
    try {
      setItems(await listWorkflowWorkspace());
    } catch (loadError: unknown) {
      setWorkspaceError(errorMessage(loadError));
    } finally {
      setWorkspaceLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadWorkspace();
  }, [loadWorkspace]);

  useEffect(() => {
    if (!draft) {
      setMetadataDraft(undefined);
      return;
    }
    setMetadataDraft({
      workflowId: draft.manifest.workflowId,
      name: draft.manifest.name,
      workflowVersion: draft.manifest.workflowVersion,
      recipeVersion: draft.manifest.recipeVersion,
      category: draft.manifest.category,
      mode: draft.manifest.mode,
    });
    const firstOutputNode = draft.nodes.find((node) => node.isOutputNode) ?? draft.nodes[0];
    setOutputDraft((current) => ({
      ...current,
      nodeId: firstOutputNode?.nodeId ?? "",
    }));
    setPublished(undefined);
  }, [draft?.draftId]);

  async function importWorkflow(existingWorkflowId?: string) {
    setLoading(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const imported = await pickApiWorkflow(existingWorkflowId);
      if (imported) {
        setDraft(imported);
        await loadWorkspace();
      }
    } catch (importError: unknown) {
      setError(errorMessage(importError));
    } finally {
      setLoading(false);
    }
  }

  async function checkCapability() {
    if (!draft) return;
    await runDraftAction(async () => {
      await checkOnboardingCapability(draft.draftId);
      updateDraft(await getOnboardingDraft(draft.draftId));
      setStep("compatibility");
    });
  }

  async function validateDraft() {
    if (!draft) return;
    await runDraftAction(async () => {
      const validation = await validateOnboarding(draft.draftId);
      updateDraft({ ...draft, validation });
      setStep("validate");
    });
  }

  async function publishDraft() {
    if (!draft || !draft.validation.readyToPublish) return;
    await runDraftAction(async () => {
      const result = await publishOnboarding(draft.draftId);
      setPublished({ workflowId: result.workflowId, recipeId: result.recipeId });
      setNotice(`Published ${result.packageName}. The runtime catalog was refreshed.`);
      await loadWorkspace();
      await onCatalogChanged();
      setStep("publish");
    });
  }

  async function discardDraft() {
    if (!draft) return;
    await runDraftAction(async () => {
      await discardOnboarding(draft.draftId);
      reset();
      setNotice("Draft discarded.");
    });
  }

  async function runDraftAction(action: () => Promise<void>) {
    setLoading(true);
    setError(undefined);
    try {
      await action();
    } catch (actionError: unknown) {
      setError(errorMessage(actionError));
    } finally {
      setLoading(false);
    }
  }

  async function saveMetadata() {
    if (!draft || !metadataDraft) return;
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingMetadata(draft.draftId, metadataDraft);
      updateDraft(nextDraft);
      setNotice("Metadata saved. Validate again before publishing.");
    });
  }

  async function bindInput(nodeId: string, input: WorkflowInputView) {
    if (!draft || input.isLinked || !input.bindable) return;
    const mapping = mappingDrafts[mappingKey(nodeId, input.name)] ?? defaultMapping(nodeId, input);
    const request: WorkflowOnboardingInputMappingRequest = {
      semanticKey: mapping.semanticKey,
      fieldType: mapping.fieldType,
      label: mapping.label,
      required: mapping.required,
      defaultValue: optionalText(mapping.defaultValue),
      minValue: optionalText(mapping.minValue),
      maxValue: optionalText(mapping.maxValue),
      minItems: optionalNumber(mapping.minItems),
      maxItems: optionalNumber(mapping.maxItems),
      itemIndex: optionalNumber(mapping.itemIndex),
      targetNode: nodeId,
      targetInput: input.name,
    };
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingInputMapping(draft.draftId, request);
      updateDraft(nextDraft);
      setNotice(`${mapping.label} is mapped to ${nodeId}.${input.name}.`);
    });
  }

  async function removeInput(mapping: WorkflowOnboardingDraftView["inputMappings"][number]) {
    if (!draft) return;
    await runDraftAction(async () => {
      const nextDraft = await removeOnboardingInputMapping(draft.draftId, {
        semanticKey: mapping.semanticKey,
        itemIndex: mapping.itemIndex,
      });
      updateDraft(nextDraft);
    });
  }

  async function addOutput() {
    if (!draft || !outputDraft.nodeId) return;
    const request: WorkflowOnboardingOutputMappingRequest = outputDraft;
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingOutputMapping(draft.draftId, request);
      updateDraft(nextDraft);
      setNotice(`${outputDraft.label} is now a ${outputDraft.type} output.`);
    });
  }

  const outputCandidates = useMemo(
    () => draft?.nodes.filter((node) => node.isOutputNode) ?? [],
    [draft],
  );

  return (
    <section className="workspace-panel workflow-workspace" aria-busy={loading || workspaceLoading}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">Workspace</span>
          <h2>Workflows</h2>
          <p className="section-description">Import a ComfyUI API workflow, map its safe inputs, then publish an atomic runtime package.</p>
        </div>
        <div className="workflow-workspace-actions">
          <button type="button" onClick={() => void loadWorkspace()} disabled={workspaceLoading}>Refresh</button>
          <button type="button" onClick={() => void importWorkflow()} disabled={loading}>Import API Workflow</button>
        </div>
      </div>

      {workspaceError && <p className="error-message" role="alert">{workspaceError}</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="workflow-notice" role="status">{notice}</p>}

      <div className="workflow-catalog" aria-label="Workflow packages">
        <div className="workflow-catalog-header">
          <span>Workflow Name</span><span>Version</span><span>Mode</span><span>Package Status</span><span>Capability</span><span>Successful Runs</span><span>Actions</span>
        </div>
        {items.map((item) => (
          <article className="workflow-catalog-row" key={`${item.workflowId}:${item.workflowVersion}`}>
            <strong>{item.name}</strong>
            <span>{item.workflowVersion}</span>
            <span>{item.mode}</span>
            <span>{item.packageStatus}</span>
            <span className={`workflow-capability workflow-capability-${item.capability.toLowerCase()}`}>{formatCapability(item.capability)}</span>
            <span>{item.hasSuccessfulRun ? "Yes" : "No"}</span>
            <button type="button" className="quiet-button" onClick={() => void importWorkflow(item.workflowId)} disabled={loading}>
              Create new version
            </button>
            <details className="workflow-catalog-detail">
              <summary>Details</summary>
              <div className="workflow-detail-grid">
                <span>SHA-256 <strong>{item.workflowSha256}</strong></span>
                <span>Nodes <strong>{item.nodeCount}</strong></span>
                <span>Classes <strong>{item.uniqueClassCount}</strong></span>
                <span>Input mappings <strong>{item.inputMappings.length}</strong></span>
                <span>Outputs <strong>{item.outputs.length}</strong></span>
              </div>
              {!!item.capabilityIssues.length && <IssueList issues={item.capabilityIssues} />}
            </details>
          </article>
        ))}
        {!items.length && !workspaceLoading && <p className="empty-state">No published workflow packages are available yet.</p>}
      </div>

      {draft && (
        <div className="workflow-onboarding-panel">
          <div className="workflow-onboarding-heading">
            <div>
              <span className="section-label">API workflow onboarding</span>
              <h3>{draft.manifest.name}</h3>
              <p className="section-description">{draft.originalFilename} · {draft.nodeCount} nodes · {draft.uniqueClassCount} classes</p>
            </div>
            <button type="button" className="quiet-button" onClick={() => void discardDraft()} disabled={loading}>Discard draft</button>
          </div>
          <div className="workflow-step-tabs" role="tablist" aria-label="Workflow onboarding steps">
            {steps.map((item) => (
              <button
                type="button"
                role="tab"
                key={item.value}
                aria-selected={step === item.value}
                className={step === item.value ? "workflow-step-active" : ""}
                onClick={() => setStep(item.value)}
              >
                {item.label}
              </button>
            ))}
          </div>

          {step === "inspect" && <InspectPane draft={draft} onContinue={() => setStep("compatibility")} />}
          {step === "compatibility" && (
            <CompatibilityPane draft={draft} loading={loading} onCheck={() => void checkCapability()} onContinue={() => setStep("inputs")} />
          )}
          {step === "inputs" && (
            <InputsPane
              draft={draft}
              mappingDrafts={mappingDrafts}
              onPatch={(key, patch) => setMappingDrafts((current) => ({ ...current, [key]: { ...(current[key] ?? emptyMapping()), ...patch } }))}
              onBind={(nodeId, input) => void bindInput(nodeId, input)}
              onRemove={(mapping) => void removeInput(mapping)}
              onContinue={() => setStep("outputs")}
            />
          )}
          {step === "outputs" && (
            <OutputsPane
              draft={draft}
              candidates={outputCandidates.length ? outputCandidates : draft.nodes}
              outputDraft={outputDraft}
              onChange={setOutputDraft}
              onAdd={() => void addOutput()}
              onContinue={() => setStep("metadata")}
            />
          )}
          {step === "metadata" && metadataDraft && (
            <MetadataPane draft={metadataDraft} onChange={setMetadataDraft} onSave={() => void saveMetadata()} onContinue={() => setStep("validate")} />
          )}
          {step === "validate" && (
            <ValidatePane draft={draft} loading={loading} onValidate={() => void validateDraft()} onPublish={() => setStep("publish")} />
          )}
          {step === "publish" && (
            <PublishPane
              draft={draft}
              published={published}
              loading={loading}
              onPublish={() => void publishDraft()}
              onOpenStudio={published ? () => void onOpenStudio(published.workflowId, published.recipeId) : undefined}
            />
          )}
        </div>
      )}
    </section>
  );
}

function InspectPane({ draft, onContinue }: { draft: WorkflowOnboardingDraftView; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-stats"><span>SHA-256 <strong>{draft.workflowSha256}</strong></span><span>Nodes <strong>{draft.nodeCount}</strong></span><span>Classes <strong>{draft.uniqueClassCount}</strong></span></div>
      <p className="section-description">Technical node IDs, class types and mappings are intentionally shown only in this onboarding workspace.</p>
      <div className="workflow-node-list">
        {draft.nodes.map((node) => <NodeCard key={node.nodeId} node={node} />)}
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>Check compatibility</button></div>
    </div>
  );
}

function NodeCard({ node }: { node: WorkflowNodeView }) {
  return (
    <details className="workflow-node-card">
      <summary><strong>Node {node.nodeId}</strong><span>{node.title}</span><code>{node.classType}</code></summary>
      <div className="workflow-node-inputs">
        {node.inputs.map((input) => <div key={input.name}><span>{input.name}</span><small>{input.currentValueSummary}{input.isLinked ? " · Connected" : ""}</small></div>)}
        {!node.inputs.length && <small>No literal inputs.</small>}
      </div>
    </details>
  );
}

function CompatibilityPane({ draft, loading, onCheck, onContinue }: { draft: WorkflowOnboardingDraftView; loading: boolean; onCheck: () => void; onContinue: () => void }) {
  const capability = draft.capability;
  return (
    <div className="workflow-onboarding-pane">
      <div className={`workflow-capability-banner workflow-capability-${capability.state.toLowerCase()}`}>
        <strong>{formatCapability(capability.state)}</strong>
        <span>{capability.checkedAt ? `Checked ${new Date(capability.checkedAt).toLocaleString()}` : "Capability has not been checked yet."}</span>
      </div>
      <button type="button" onClick={onCheck} disabled={loading}>{loading ? "Checking..." : "Check ComfyUI capability"}</button>
      {!!capability.issues.length && <IssueList issues={capability.issues} />}
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>Configure inputs</button></div>
    </div>
  );
}

function InputsPane({
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
      <p className="section-description">Choose the semantic field and confirm each mapping. Connected inputs remain protected and cannot be bound directly.</p>
      <div className="workflow-input-list">
        {draft.nodes.flatMap((node) => node.inputs.map((input) => {
          const key = mappingKey(node.nodeId, input.name);
          const mapping = mappingDrafts[key] ?? defaultMapping(node.nodeId, input);
          const existing = draft.inputMappings.find((candidate) => candidate.targetNode === node.nodeId && candidate.targetInput === input.name);
          return (
            <div className="workflow-input-card" key={key}>
              <div className="workflow-input-heading"><strong>{node.nodeId}.{input.name}</strong><span>{input.currentValueSummary}</span></div>
              {input.isLinked ? <p className="disabled-note">Connected input — not directly bindable</p> : (
                <div className="workflow-mapping-form">
                  <label>Semantic key<input value={mapping.semanticKey} onChange={(event) => onPatch(key, { semanticKey: event.target.value })} /></label>
                  <label>Field type<select value={mapping.fieldType} onChange={(event) => onPatch(key, { fieldType: event.target.value as WorkflowFieldType })}>{fieldTypes.map((type) => <option key={type} value={type}>{type}</option>)}</select></label>
                  <label>Label<input value={mapping.label} onChange={(event) => onPatch(key, { label: event.target.value })} /></label>
                  <label className="checkbox-label"><input type="checkbox" checked={mapping.required} onChange={(event) => onPatch(key, { required: event.target.checked })} /> Required</label>
                  {mapping.fieldType === "integer" || mapping.fieldType === "seed" ? <>
                    <label>Default<input value={mapping.defaultValue} onChange={(event) => onPatch(key, { defaultValue: event.target.value })} /></label>
                    <label>Min<input value={mapping.minValue} onChange={(event) => onPatch(key, { minValue: event.target.value })} inputMode="numeric" /></label>
                    <label>Max<input value={mapping.maxValue} onChange={(event) => onPatch(key, { maxValue: event.target.value })} inputMode="numeric" /></label>
                  </> : null}
                  {mapping.fieldType.endsWith("s") ? <label>Max items<input value={mapping.maxItems} onChange={(event) => onPatch(key, { maxItems: event.target.value })} inputMode="numeric" /></label> : null}
                  <button type="button" onClick={() => onBind(node.nodeId, input)} disabled={!input.bindable}>Confirm mapping</button>
                </div>
              )}
              {input.allowedOptions.length > 0 && <small className="field-hint">Available options: {input.allowedOptions.join(", ")}</small>}
              {existing && <div className="workflow-existing-mapping"><span>Mapped as <strong>{existing.label}</strong></span><button type="button" className="quiet-button" onClick={() => onRemove(existing)}>Remove</button></div>}
            </div>
          );
        }))}
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>Configure outputs</button></div>
    </div>
  );
}

function OutputsPane({ draft, candidates, outputDraft, onChange, onAdd, onContinue }: { draft: WorkflowOnboardingDraftView; candidates: WorkflowNodeView[]; outputDraft: OutputDraft; onChange: (value: OutputDraft) => void; onAdd: () => void; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <p className="section-description">Declare the user-facing outputs explicitly. Output IDs are stable snake_case keys used by the runtime package.</p>
      <div className="workflow-mapping-form workflow-output-form">
        <label>Output ID<input value={outputDraft.outputId} onChange={(event) => onChange({ ...outputDraft, outputId: event.target.value })} /></label>
        <label>Label<input value={outputDraft.label} onChange={(event) => onChange({ ...outputDraft, label: event.target.value })} /></label>
        <label>Type<select value={outputDraft.type} onChange={(event) => onChange({ ...outputDraft, type: event.target.value as "image" | "video" })}><option value="image">image</option><option value="video">video</option></select></label>
        <label>Output node<select value={outputDraft.nodeId} onChange={(event) => onChange({ ...outputDraft, nodeId: event.target.value })}>{candidates.map((node) => <option key={node.nodeId} value={node.nodeId}>Node {node.nodeId} · {node.classType}</option>)}</select></label>
        <label className="checkbox-label"><input type="checkbox" checked={outputDraft.required} onChange={(event) => onChange({ ...outputDraft, required: event.target.checked })} /> Required</label>
        <button type="button" onClick={onAdd} disabled={!outputDraft.nodeId}>Confirm output</button>
      </div>
      <div className="workflow-output-list">{draft.outputMappings.map((output) => <div key={output.outputId}><strong>{output.label}</strong><span>{output.outputId} · {output.type} · node {output.nodeId}</span></div>)}</div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>Set metadata</button></div>
    </div>
  );
}

function MetadataPane({ draft, onChange, onSave, onContinue }: { draft: MetadataDraft; onChange: (value: MetadataDraft) => void; onSave: () => void; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-mapping-form workflow-metadata-form">
        <label>Workflow ID<input value={draft.workflowId} onChange={(event) => onChange({ ...draft, workflowId: event.target.value })} /></label>
        <label>Name<input value={draft.name} onChange={(event) => onChange({ ...draft, name: event.target.value })} /></label>
        <label>Workflow version<input value={draft.workflowVersion} onChange={(event) => onChange({ ...draft, workflowVersion: event.target.value })} /></label>
        <label>Recipe version<input value={draft.recipeVersion} onChange={(event) => onChange({ ...draft, recipeVersion: event.target.value })} /></label>
        <label>Category<input value={draft.category} onChange={(event) => onChange({ ...draft, category: event.target.value })} /></label>
        <label>Mode<input value={draft.mode} onChange={(event) => onChange({ ...draft, mode: event.target.value })} /></label>
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onSave}>Save metadata</button><button type="button" onClick={onContinue}>Validate draft</button></div>
    </div>
  );
}

function ValidatePane({ draft, loading, onValidate, onPublish }: { draft: WorkflowOnboardingDraftView; loading: boolean; onValidate: () => void; onPublish: () => void }) {
  const validation = draft.validation;
  const checks = [
    ["API format", validation.apiFormat],
    ["Recipe", validation.recipe],
    ["Bindings", validation.bindings],
    ["Outputs", validation.outputs],
    ["Manifest", validation.manifest],
    ["Capability", validation.capability],
    ["Dry run", validation.dryRun],
  ] as const;
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-validation-grid">{checks.map(([label, valid]) => <span key={label} className={valid ? "workflow-check-pass" : "workflow-check-fail"}>{valid ? "✓" : "!"} {label}</span>)}</div>
      {!!validation.issues.length && <ul className="workflow-issue-list">{validation.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
      <details className="workflow-recipe-preview"><summary>Advanced: generated Recipe YAML</summary><pre>{draft.recipe.yaml ?? "Recipe is not valid yet."}</pre></details>
      <div className="workflow-pane-actions"><button type="button" onClick={onValidate} disabled={loading}>{loading ? "Validating..." : "Validate again"}</button><button type="button" onClick={onPublish} disabled={!validation.readyToPublish}>Continue to publish</button></div>
    </div>
  );
}

function PublishPane({ draft, published, loading, onPublish, onOpenStudio }: { draft: WorkflowOnboardingDraftView; published?: { workflowId: string; recipeId: string }; loading: boolean; onPublish: () => void; onOpenStudio?: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className={`workflow-publish-state ${draft.validation.readyToPublish ? "workflow-check-pass" : "workflow-check-fail"}`}>
        {draft.validation.readyToPublish ? "Ready to publish" : "Publishing is disabled until all checks pass."}
      </div>
      <button type="button" onClick={onPublish} disabled={loading || !draft.validation.readyToPublish}>{loading ? "Publishing..." : "Publish runtime package"}</button>
      {published && <div className="workflow-published-result"><strong>Published successfully</strong><span>Package is available in Studio after catalog refresh.</span><button type="button" onClick={onOpenStudio}>Open in Studio</button></div>}
    </div>
  );
}

function IssueList({ issues }: { issues: WorkflowOnboardingDraftView["capability"]["issues"] }) {
  return <ul className="workflow-issue-list">{issues.map((issue) => <li key={`${issue.code}:${issue.nodeId ?? ""}:${issue.inputName ?? ""}`}>{issue.message}</li>)}</ul>;
}

function defaultMapping(nodeId: string, input: WorkflowInputView): MappingDraft {
  const safeName = input.name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "value";
  return {
    semanticKey: `input_${nodeId}_${safeName}`,
    fieldType: (input.suggestedType as WorkflowFieldType | undefined) && fieldTypes.includes(input.suggestedType as WorkflowFieldType) ? input.suggestedType as WorkflowFieldType : "textarea",
    label: input.name,
    required: true,
    defaultValue: "",
    minValue: input.numericMin ?? "",
    maxValue: input.numericMax ?? "",
    minItems: "",
    maxItems: "",
    itemIndex: "",
  };
}

function emptyMapping(): MappingDraft {
  return { semanticKey: "input_value", fieldType: "textarea", label: "Value", required: true, defaultValue: "", minValue: "", maxValue: "", minItems: "", maxItems: "", itemIndex: "" };
}

function mappingKey(nodeId: string, inputName: string): string {
  return `${nodeId}:${inputName}`;
}

function optionalText(value: string): string | undefined {
  return value.trim() || undefined;
}

function optionalNumber(value: string): number | undefined {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : undefined;
}

function formatCapability(value: string): string {
  return value.replace(/_/g, " ").toLowerCase().replace(/(^| )\S/g, (letter: string) => letter.toUpperCase());
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
