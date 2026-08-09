import { useCallback, useEffect, useMemo, useState } from "react";
import {
  checkOnboardingCapability,
  cleanWorkflowStaging,
  compareWorkflowVersions,
  createGeneration,
  discardOnboarding,
  duplicateWorkflowRecipe,
  exportWorkflowPackage,
  getOnboardingDraft,
  importWorkflowPackageBackup,
  listWorkflowProductionWorkspace,
  pickApiWorkflow,
  publishOnboarding,
  recheckWorkflowCapability,
  removeOnboardingInputMapping,
  setWorkflowEnabled,
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
  WorkflowProductionWorkspaceView,
  WorkflowVersionDiffView,
} from "../../types/workflowOnboarding";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, stagingStatusLabel, workflowDisplayName, workflowModeLabel } from "../../i18n/statusLabels";

interface Props {
  projectId?: string;
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  onCatalogChanged: () => Promise<void>;
  onOpenStudio: (workflowId: string, recipeId: string) => Promise<void>;
  onOpenTask?: (taskId: string) => void;
}

const steps: Array<{ value: WorkflowOnboardingStep; label: string }> = [
  { value: "inspect", label: "检查工作流" },
  { value: "compatibility", label: "兼容性检查" },
  { value: "inputs", label: "输入映射" },
  { value: "outputs", label: "输出映射" },
  { value: "metadata", label: "基本信息" },
  { value: "validate", label: "校验" },
  { value: "publish", label: "发布" },
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

export function createDefaultOutputDraft(): OutputDraft {
  return {
    outputId: "output_1",
    label: "输出结果",
    type: "image",
    nodeId: "",
    required: true,
  };
}

interface MetadataDraft {
  workflowId: string;
  name: string;
  workflowVersion: string;
  recipeVersion: string;
  category: string;
  mode: string;
}

export function WorkflowWorkspace({ projectId, catalog, comfyConnected, onCatalogChanged, onOpenStudio, onOpenTask }: Props) {
  const [items, setItems] = useState<WorkflowProductionWorkspaceView[]>([]);
  const [staging, setStaging] = useState<{ stagingId: string; status: string; inUse: boolean }[]>([]);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<"all" | "enabled" | "disabled" | "issues">("all");
  const [selectedVersions, setSelectedVersions] = useState<string[]>([]);
  const [diff, setDiff] = useState<WorkflowVersionDiffView>();
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [workspaceError, setWorkspaceError] = useState<string>();
  const [quickTestingId, setQuickTestingId] = useState<string>();
  const [mappingDrafts, setMappingDrafts] = useState<Record<string, MappingDraft>>({});
  const [outputDraft, setOutputDraft] = useState<OutputDraft>(createDefaultOutputDraft);
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
      const workspace = await listWorkflowProductionWorkspace();
      setItems(workspace.items);
      setStaging(workspace.staging);
    } catch (loadError: unknown) {
      setWorkspaceError(toUserMessage(loadError));
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
      setError(toUserMessage(importError));
    } finally {
      setLoading(false);
    }
  }

  async function importBackup() {
    setWorkspaceError(undefined);
    try {
      const restored = await importWorkflowPackageBackup();
      if (restored) {
        setNotice(`工作流备份已恢复：${restored.workflowVersion}`);
        await loadWorkspace();
        await onCatalogChanged();
      }
    } catch (importError: unknown) {
      setWorkspaceError(toUserMessage(importError));
    }
  }

  async function toggleVersion(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId) return;
    try {
      await setWorkflowEnabled(item.workflowVersionId, !item.enabled);
      await loadWorkspace();
      await onCatalogChanged();
      setNotice(`${item.name ?? item.packageName} 已${item.enabled ? "停用" : "启用"}。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function recheckVersion(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId) return;
    try {
      const capability = await recheckWorkflowCapability(item.workflowVersionId);
      await loadWorkspace();
      setNotice(`${item.name ?? item.packageName}: ${formatCapability(capability.state)}`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function duplicateRecipe(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId) return;
    try {
      const duplicated = await duplicateWorkflowRecipe(item.workflowVersionId, item.recipes[item.recipes.length - 1]?.recipeId);
      setDraft(duplicated);
      setStep("inputs");
      setNotice("配方已复制，请检查映射并发布新的配方版本。");
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function quickTest(item: WorkflowProductionWorkspaceView) {
    if (!projectId || !item.workflowVersionId) return;
    const latestRecipeId = item.recipes[item.recipes.length - 1]?.recipeId;
    const recipe = catalog.find((candidate) =>
      candidate.workflowVersionId === item.workflowVersionId && candidate.recipeId === latestRecipeId,
    );
    if (!recipe) {
      setWorkspaceError("当前配方尚未进入创作目录，请先刷新工作流。");
      return;
    }
    if (!comfyConnected) {
      setWorkspaceError("请先连接 ComfyUI，再执行快速测试。");
      return;
    }
    const values = quickTestValues(recipe);
    if (!values) {
      setNotice("该工作流需要图片、视频或音频素材，请打开创作页补充最低必需输入。");
      await onOpenStudio(recipe.workflowId, recipe.recipeId);
      return;
    }
    setQuickTestingId(item.workflowVersionId);
    setWorkspaceError(undefined);
    try {
      const task = await createGeneration({
        projectId,
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        values,
      });
      setNotice(`快速测试任务已创建：${task.id}`);
      onOpenTask?.(task.id);
    } catch (testError: unknown) {
      setWorkspaceError(toUserMessage(testError));
    } finally {
      setQuickTestingId(undefined);
    }
  }

  async function compareSelected() {
    if (selectedVersions.length !== 2) return;
    try {
      setDiff(await compareWorkflowVersions(selectedVersions[0], selectedVersions[1]));
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  function toggleSelected(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId) return;
    setSelectedVersions((current) => current.includes(item.workflowVersionId!)
      ? current.filter((id) => id !== item.workflowVersionId)
      : current.length < 2 ? [...current, item.workflowVersionId!] : [current[1], item.workflowVersionId!]);
  }

  const visibleItems = items.filter((item) => {
    const matchesSearch = !search.trim() || (item.name ?? item.packageName).toLowerCase().includes(search.trim().toLowerCase());
    const matchesFilter = filter === "all"
      || (filter === "enabled" && item.enabled)
      || (filter === "disabled" && !item.enabled)
      || (filter === "issues" && (item.packageStatus !== "VALID" || item.diagnostics.length > 0 || item.capability !== "READY"));
    return matchesSearch && matchesFilter;
  });

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
      setNotice(`已发布 ${result.packageName}，运行目录已刷新。`);
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
      setNotice("草稿已丢弃。");
    });
  }

  async function runDraftAction(action: () => Promise<void>) {
    setLoading(true);
    setError(undefined);
    try {
      await action();
    } catch (actionError: unknown) {
      setError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }

  async function saveMetadata() {
    if (!draft || !metadataDraft) return;
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingMetadata(draft.draftId, metadataDraft);
      updateDraft(nextDraft);
      setNotice("基本信息已保存，请重新校验后再发布。");
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
      setNotice(`${mapping.label} 已绑定到 ${nodeId}.${input.name}。`);
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
      setNotice(`${outputDraft.label} 已设置为${outputDraft.type === "video" ? "视频" : "图片"}输出。`);
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
          <span className="section-label">工作区</span>
          <h2>工作流管理</h2>
          <p className="section-description">导入 ComfyUI API 工作流，配置安全输入后发布工作流运行包。</p>
        </div>
        <div className="workflow-workspace-actions">
          <button type="button" onClick={() => void loadWorkspace()} disabled={workspaceLoading}>{workspaceLoading ? "正在刷新..." : "刷新"}</button>
          <button type="button" onClick={() => void importWorkflow()} disabled={loading}>导入 API 工作流</button>
          <button type="button" onClick={() => void importBackup()} disabled={loading}>导入工作流备份</button>
      </div>
      </div>

      {workspaceError && <p className="error-message" role="alert">{workspaceError}</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="workflow-notice" role="status">{notice}</p>}

      <section className="workflow-health-dashboard" aria-label="运行环境健康概览">
        <div><span>运行包总数</span><strong>{items.length}</strong></div>
        <div><span>生产就绪</span><strong>{items.filter((item) => item.readiness === "READY").length}</strong></div>
        <div><span>待验证</span><strong>{items.filter((item) => item.readiness === "DEGRADED").length}</strong></div>
        <div><span>阻塞诊断</span><strong>{items.filter((item) => item.readiness === "BLOCKED").length}</strong></div>
      </section>

      <div className="workflow-production-toolbar">
        <input aria-label="搜索工作流" placeholder="按名称搜索" value={search} onChange={(event) => setSearch(event.target.value)} />
        <select aria-label="工作流筛选" value={filter} onChange={(event) => setFilter(event.target.value as typeof filter)}>
          <option value="all">全部</option><option value="enabled">已启用</option><option value="disabled">已停用</option><option value="issues">有问题</option>
        </select>
        <button type="button" onClick={() => void compareSelected()} disabled={selectedVersions.length !== 2}>比较选中的版本</button>
      </div>
      <div className="workflow-catalog" aria-label="工作流运行包">
        <div className="workflow-catalog-header">
          <span>比较</span><span>工作流名称</span><span>版本</span><span>模式</span><span>运行包</span><span>兼容状态</span><span>就绪状态</span><span>运行记录</span><span>操作</span>
        </div>
        {visibleItems.map((item) => (
          <article className="workflow-catalog-row" key={`${item.packageName}:${item.workflowVersionId ?? "invalid"}`}>
            <input type="checkbox" aria-label={`比较 ${workflowDisplayName(item.workflowId, item.name ?? item.packageName)}`} checked={item.workflowVersionId ? selectedVersions.includes(item.workflowVersionId) : false} onChange={() => toggleSelected(item)} disabled={!item.workflowVersionId} />
            <strong>{workflowDisplayName(item.workflowId, item.name ?? item.packageName)}</strong>
            <span>{item.workflowVersion ?? "—"}</span>
            <span>{item.mode ? workflowModeLabel(item.mode) : "—"}</span>
            <span>{packageStatusLabel(item.packageStatus)}</span>
            <span className={`workflow-capability workflow-capability-${item.capability.toLowerCase()}`}>{formatCapability(item.capability)}</span>
            <span className={`workflow-readiness workflow-readiness-${item.readiness.toLowerCase()}`}>{formatReadiness(item.readiness)}</span>
            <span>{item.hasSuccessfulRun ? "已有成功运行" : `共 ${item.totalTasks} 个任务`}</span>
            <div className="workflow-row-actions">
              {item.workflowVersionId && <button type="button" className="quiet-button" onClick={() => void toggleVersion(item)}>{item.enabled ? "停用" : "启用"}</button>}
              {item.workflowVersionId && <button type="button" className="quiet-button" onClick={() => void recheckVersion(item)}>重新检查</button>}
              {item.workflowVersionId && <button type="button" className="quiet-button" onClick={() => void duplicateRecipe(item)}>复制配方</button>}
              {item.workflowVersionId && <button type="button" className="quiet-button" onClick={() => void quickTest(item)} disabled={quickTestingId === item.workflowVersionId}>{quickTestingId === item.workflowVersionId ? "测试中..." : "快速测试"}</button>}
              {item.workflowVersionId && <button type="button" className="quiet-button" onClick={() => void exportWorkflowPackage(item.workflowVersionId!)}>导出工作流</button>}
              {item.workflowId && <button type="button" className="quiet-button" onClick={() => void importWorkflow(item.workflowId)}>创建新版本</button>}
            </div>
            <details className="workflow-catalog-detail">
              <summary>查看详情</summary>
              <div className="workflow-detail-grid">
                <span>启用状态 <strong>{item.enabled ? "已启用" : "已停用"}</strong></span>
                <span>工作流 SHA-256 <strong>{item.workflowSha256 ?? "—"}</strong></span>
                <span>配方 SHA-256 <strong>{item.recipeSha256 ?? "—"}</strong></span>
                <span>节点数量 <strong>{item.nodeCount}</strong></span>
                <span>配方数量 <strong>{item.recipes.length}</strong></span>
                <span>活动任务 <strong>{item.activeTasks}</strong></span>
                <span>任务总数 <strong>{item.totalTasks}</strong></span>
                <span>最近真实验证 <strong>{item.liveVerifiedAt ? formatDateTime(item.liveVerifiedAt) : "尚未验证"}</strong></span>
              </div>
              {!!item.readinessReasons.length && <ul className="workflow-issue-list">{item.readinessReasons.map((reason) => <li key={reason}>{reason}</li>)}</ul>}
              {!!item.capabilityIssues.length && <section className="workflow-dependency-diagnostics"><strong>依赖诊断 · 来源：ComfyUI /object_info</strong><IssueList issues={item.capabilityIssues} /></section>}
              {!item.capabilityIssues.length && item.packageStatus === "VALID" && <p className="disabled-note">未发现节点依赖问题。模型或文件依赖只有在运行包明确声明并有可验证来源时才会报告，AI Studio 不猜测未声明依赖。</p>}
              {!!item.diagnostics.length && <ul className="workflow-issue-list">{item.diagnostics.map((diagnostic) => <li key={diagnostic.code}>{toUserMessage({ code: diagnostic.code, message: diagnostic.message })}</li>)}</ul>}
              {!!item.recipes.length && <div className="workflow-recipe-summary">{item.recipes.map((recipe) => <span key={recipe.recipeId}>配方 {recipe.version} · {recipe.inputCount} 个输入 · {recipe.outputCount} 个输出</span>)}</div>}
            </details>
          </article>
        ))}
        {!visibleItems.length && !workspaceLoading && <p className="empty-state">当前筛选条件下没有找到工作流。</p>}
      </div>

      {!!staging.length && <div className="workflow-diagnostics-panel"><h3>运行诊断</h3>{staging.map((entry) => <div key={entry.stagingId}><span>{stagingStatusLabel(entry.status)}</span><code>{entry.stagingId}</code><button type="button" className="quiet-button" onClick={() => void cleanWorkflowStaging(entry.stagingId)} disabled={entry.inUse}>{entry.inUse ? "使用中" : "清理暂存"}</button></div>)}</div>}
      {diff && <VersionDiffPane diff={diff} onClose={() => setDiff(undefined)} />}

      {draft && (
        <div className="workflow-onboarding-panel">
          <div className="workflow-onboarding-heading">
            <div>
              <span className="section-label">API 工作流导入向导</span>
              <h3>{draft.manifest.name}</h3>
              <p className="section-description">{draft.originalFilename} · {draft.nodeCount} 个节点 · {draft.uniqueClassCount} 种节点类型</p>
            </div>
            <button type="button" className="quiet-button" onClick={() => void discardDraft()} disabled={loading}>丢弃草稿</button>
          </div>
          <div className="workflow-step-tabs" role="tablist" aria-label="工作流导入步骤">
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
      <div className="workflow-stats"><span>SHA-256 <strong>{draft.workflowSha256}</strong></span><span>节点数量 <strong>{draft.nodeCount}</strong></span><span>节点类型数量 <strong>{draft.uniqueClassCount}</strong></span></div>
      <p className="section-description">节点 ID、节点类型和映射仅在此导入向导中作为技术信息显示。</p>
      <div className="workflow-node-list">
        {draft.nodes.map((node) => <NodeCard key={node.nodeId} node={node} />)}
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>检查兼容性</button></div>
    </div>
  );
}

function NodeCard({ node }: { node: WorkflowNodeView }) {
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

function CompatibilityPane({ draft, loading, onCheck, onContinue }: { draft: WorkflowOnboardingDraftView; loading: boolean; onCheck: () => void; onContinue: () => void }) {
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
      <p className="section-description">选择语义字段并确认每项映射。已连接的输入受到保护，不能直接绑定。</p>
      <div className="workflow-input-list">
        {draft.nodes.flatMap((node) => node.inputs.map((input) => {
          const key = mappingKey(node.nodeId, input.name);
          const mapping = mappingDrafts[key] ?? defaultMapping(node.nodeId, input);
          const existing = draft.inputMappings.find((candidate) => candidate.targetNode === node.nodeId && candidate.targetInput === input.name);
          return (
            <div className="workflow-input-card" key={key}>
              <div className="workflow-input-heading"><strong>{node.nodeId}.{input.name}</strong><span>{input.currentValueSummary}</span></div>
              {input.isLinked ? <p className="disabled-note">已连接输入，不能直接绑定。</p> : (
                <div className="workflow-mapping-form">
                  <label>语义键<input value={mapping.semanticKey} onChange={(event) => onPatch(key, { semanticKey: event.target.value })} /></label>
                  <label>字段类型<select value={mapping.fieldType} onChange={(event) => onPatch(key, { fieldType: event.target.value as WorkflowFieldType })}>{fieldTypes.map((type) => <option key={type} value={type}>{fieldTypeLabel(type)}</option>)}</select></label>
                  <label>显示名称<input value={mapping.label} onChange={(event) => onPatch(key, { label: event.target.value })} /></label>
                  <label className="checkbox-label"><input type="checkbox" checked={mapping.required} onChange={(event) => onPatch(key, { required: event.target.checked })} /> 必填</label>
                  {mapping.fieldType === "integer" || mapping.fieldType === "seed" ? <>
                    <label>默认值<input value={mapping.defaultValue} onChange={(event) => onPatch(key, { defaultValue: event.target.value })} /></label>
                    <label>最小值<input value={mapping.minValue} onChange={(event) => onPatch(key, { minValue: event.target.value })} inputMode="numeric" /></label>
                    <label>最大值<input value={mapping.maxValue} onChange={(event) => onPatch(key, { maxValue: event.target.value })} inputMode="numeric" /></label>
                  </> : null}
                  {mapping.fieldType.endsWith("s") ? <label>最大数量<input value={mapping.maxItems} onChange={(event) => onPatch(key, { maxItems: event.target.value })} inputMode="numeric" /></label> : null}
                  <button type="button" onClick={() => onBind(node.nodeId, input)} disabled={!input.bindable}>确认映射</button>
                </div>
              )}
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

function OutputsPane({ draft, candidates, outputDraft, onChange, onAdd, onContinue }: { draft: WorkflowOnboardingDraftView; candidates: WorkflowNodeView[]; outputDraft: OutputDraft; onChange: (value: OutputDraft) => void; onAdd: () => void; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <p className="section-description">明确声明用户可见的输出结果。输出 ID 是运行包使用的稳定 snake_case 技术键。</p>
      <div className="workflow-mapping-form workflow-output-form">
        <label>输出 ID<input value={outputDraft.outputId} onChange={(event) => onChange({ ...outputDraft, outputId: event.target.value })} /></label>
        <label>显示名称<input value={outputDraft.label} onChange={(event) => onChange({ ...outputDraft, label: event.target.value })} /></label>
        <label>类型<select value={outputDraft.type} onChange={(event) => onChange({ ...outputDraft, type: event.target.value as "image" | "video" })}><option value="image">图片</option><option value="video">视频</option></select></label>
        <label>输出节点<select value={outputDraft.nodeId} onChange={(event) => onChange({ ...outputDraft, nodeId: event.target.value })}>{candidates.map((node) => <option key={node.nodeId} value={node.nodeId}>节点 {node.nodeId} · {node.classType}</option>)}</select></label>
        <label className="checkbox-label"><input type="checkbox" checked={outputDraft.required} onChange={(event) => onChange({ ...outputDraft, required: event.target.checked })} /> 必填</label>
        <button type="button" onClick={onAdd} disabled={!outputDraft.nodeId}>确认输出</button>
      </div>
      <div className="workflow-output-list">{draft.outputMappings.map((output) => <div key={output.outputId}><strong>{output.label}</strong><span>{output.outputId} · {output.type === "video" ? "视频" : "图片"} · 节点 {output.nodeId}</span></div>)}</div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>设置基本信息</button></div>
    </div>
  );
}

function MetadataPane({ draft, onChange, onSave, onContinue }: { draft: MetadataDraft; onChange: (value: MetadataDraft) => void; onSave: () => void; onContinue: () => void }) {
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

function ValidatePane({ draft, loading, onValidate, onPublish }: { draft: WorkflowOnboardingDraftView; loading: boolean; onValidate: () => void; onPublish: () => void }) {
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
      <details className="workflow-recipe-preview"><summary>高级信息：生成的 Recipe YAML</summary><pre>{draft.recipe.yaml ?? "配方当前还未通过校验。"}</pre></details>
      <div className="workflow-pane-actions"><button type="button" onClick={onValidate} disabled={loading}>{loading ? "正在校验..." : "再次校验"}</button><button type="button" onClick={onPublish} disabled={!validation.readyToPublish}>继续发布</button></div>
    </div>
  );
}

function PublishPane({ draft, published, loading, onPublish, onOpenStudio }: { draft: WorkflowOnboardingDraftView; published?: { workflowId: string; recipeId: string }; loading: boolean; onPublish: () => void; onOpenStudio?: () => void }) {
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

function VersionDiffPane({ diff, onClose }: { diff: WorkflowVersionDiffView; onClose: () => void }) {
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

function IssueList({ issues }: { issues: WorkflowOnboardingDraftView["capability"]["issues"] }) {
  return <ul className="workflow-issue-list">{issues.map((issue) => <li key={`${issue.code}:${issue.nodeId ?? ""}:${issue.inputName ?? ""}`}>{toUserMessage({ code: issue.code, message: issue.message })}</li>)}</ul>;
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
  return { semanticKey: "input_value", fieldType: "textarea", label: "值", required: true, defaultValue: "", minValue: "", maxValue: "", minItems: "", maxItems: "", itemIndex: "" };
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

function quickTestValues(recipe: RecipeViewModel): GenerationValues | undefined {
  const values: GenerationValues = {};
  for (const field of recipe.fields) {
    switch (field.type) {
      case "textarea":
        values[field.key] = {
          type: "string",
          value: field.default || (field.required ? "AI Studio 快速测试" : ""),
        };
        break;
      case "integer":
        if (field.default !== undefined) {
          values[field.key] = { type: "integer", value: field.default };
        } else if (field.required) {
          values[field.key] = { type: "integer", value: field.min ?? 1 };
        }
        break;
      case "seed":
        values[field.key] = field.defaultMode === "fixed" && field.defaultValue
          ? { type: "seed_fixed", value: field.defaultValue }
          : { type: "seed_random" };
        break;
      case "image":
      case "images":
      case "video":
      case "videos":
      case "audio":
      case "audios":
        if (field.required) return undefined;
        break;
    }
  }
  return values;
}

function formatCapability(value: string): string {
  return {
    READY: "可用",
    MISSING_NODES: "缺少节点",
    INCOMPATIBLE_INPUT_VALUES: "输入值不兼容",
    COMFY_OFFLINE: "ComfyUI 离线",
    NOT_CHECKED: "尚未检查",
  }[value] ?? "未知状态";
}

function formatReadiness(value: string): string {
  return {
    READY: "生产就绪",
    DEGRADED: "待验证",
    BLOCKED: "已阻塞",
  }[value] ?? "未知状态";
}

function packageStatusLabel(value: string): string {
  return {
    VALID: "有效",
    INVALID: "无效",
    MISSING: "缺失",
    STAGED: "待发布",
  }[value] ?? "未知状态";
}

function fieldTypeLabel(value: WorkflowFieldType): string {
  return {
    textarea: "多行文本",
    integer: "整数",
    seed: "随机种子",
    image: "图片",
    images: "多张图片",
    video: "视频",
    videos: "多个视频",
    audio: "音频",
    audios: "多个音频",
  }[value];
}

function localizeWorkflowIssue(value: string): string {
  const normalized = value.toLowerCase();
  if (normalized.includes("api") && normalized.includes("format")) return "该文件不是 ComfyUI API 格式工作流。";
  if (normalized.includes("recipe")) return "配方校验未通过，请检查输入映射和输出映射。";
  if (normalized.includes("binding")) return "输入绑定校验未通过，请检查每个输入映射。";
  if (normalized.includes("output")) return "输出校验未通过，请至少配置一个有效输出。";
  if (normalized.includes("manifest")) return "工作流基本信息校验未通过。";
  if (normalized.includes("capability")) return "ComfyUI 兼容性校验未通过。";
  if (normalized.includes("dry run")) return "工作流试运行未通过。";
  return "工作流校验未通过，请查看技术详情。";
}
