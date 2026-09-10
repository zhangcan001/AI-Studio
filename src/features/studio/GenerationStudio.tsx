import { useCallback, useEffect, useMemo, useState } from "react";
import {
  createProductionQueue,
  createProjectTemplate,
  getPromptLibraryEntry,
  getProjectWorkflowConfig,
  refreshWorkflowLibrary,
  startProductionQueue,
} from "../../services/tauriClient";
import { useStudioStore } from "../../stores/studioStore";
import { useTaskStore } from "../../stores/taskStore";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type { ReusableGenerationDraft } from "../../types/history";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, workflowDisplayName } from "../../i18n/statusLabels";
import { DynamicFormRenderer, validateRecipeValues } from "./DynamicFormRenderer";
import {
  cloneGenerationValues,
} from "./batchDraft";
import { ProductionQueuePanel } from "./ProductionQueuePanel";
import { productionInteractionPolicy } from "./productionQueuePolicy";
import { CreationResultPanel } from "./CreationResultPanel";
import { GenerationActionBar } from "./GenerationActionBar";
import { generationBlockedReason } from "./generationBlockedReason";
import { StudioModeTabs, type StudioMode } from "./StudioModeTabs";
import { NoWorkflowGuide } from "./NoWorkflowGuide";
import { assignAssetToField, compatibleAssetFields } from "./assetIntent";
import { CreationModeHint } from "../runtime/CreationModeHint";
import { RuntimeParameterProfilePanel } from "../runtime/RuntimeParameterProfilePanel";
import { ExperimentPlannerPanel } from "../experiments/ExperimentPlannerPanel";
import { WorkflowBenchmarkPanel } from "../experiments/WorkflowBenchmarkPanel";
import { ProductionRunPanel } from "../production/ProductionRunPanel";
import { type ExperimentContext, type ExperimentDimension, type ExperimentPlan } from "../experiments/experimentPlanner";
import { PromptLibraryPanel } from "../prompts/PromptLibraryPanel";
import type { PromptVersionView } from "../../types/prompt";
import type { PromptEntryView } from "../../types/prompt";
import { applyPromptSnippetToStudio, applyPromptVersionToStudio } from "../prompts/promptLibrary";
import { CreationDashboard } from "../production/CreationDashboard";
import type { RecentWorkflowRecord } from "../production/productionUx";
import { KERA2_WORKFLOW_ID, kera2RecipeContract } from "../runtime/productRuntimeScope";
import { ResolutionControl } from "../runtime/ResolutionControl";
import { KREA2_RESOLUTION_PRESETS, resolutionPresetsForRecipe } from "../runtime/resolutionPresets";
import { WorkflowSelector } from "../runtime/WorkflowSelector";
import { useGenerationPresetController } from "./hooks/useGenerationPresetController";
import { useGenerationSubmissionController } from "./hooks/useGenerationSubmissionController";
import { useGenerationBatchController } from "./hooks/useGenerationBatchController";
import {
  filterImageRecipes,
  findRecipe,
  imageRecipeCapability,
  migrateGenerationValues,
  recipeRef,
  sameRecipeRef,
  type SelectedRecipeRef,
} from "../runtime/workflowCapabilities";

function fieldTypeLabel(type: RecipeField["type"]): string {
  switch (type) {
    case "image":
    case "images":
      return "图片";
    case "video":
    case "videos":
      return "视频";
    case "audio":
    case "audios":
      return "音频";
    case "textarea":
      return "文字";
    case "integer":
      return "数字";
    case "number":
      return "小数";
    case "seed":
      return "种子";
  }
}

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  taskEventsReady: boolean;
  taskEventError?: string;
  productionAdmission: ProductionAdmissionStatus;
  focusProductionBatchId?: string;
  onCatalogChanged: () => Promise<void>;
  onProductionAdmissionChanged: () => Promise<void>;
  onProductionBatchFocused: () => void;
  onOpenTask: (taskId: string) => void;
  onOpenWorkflows: () => void;
  onReconnectComfy: () => void;
}

export function GenerationStudio({
  projectId,
  catalog,
  comfyConnected,
  taskEventsReady,
  taskEventError,
  productionAdmission,
  focusProductionBatchId,
  onCatalogChanged,
  onProductionAdmissionChanged,
  onProductionBatchFocused,
  onOpenTask,
  onOpenWorkflows,
  onReconnectComfy,
}: Props) {
  const selectedWorkflow = useStudioStore((state) => state.selectedWorkflow);
  const productCatalog = useMemo(
    () => filterImageRecipes(catalog),
    [catalog],
  );
  const [manualSelection, setManualSelection] = useState<SelectedRecipeRef | undefined>(
    undefined,
  );
  const [projectWorkflowConfig, setProjectWorkflowConfig] = useState<ProjectWorkflowConfigView>();
  const recommendedWorkflow = useMemo(
    () => productCatalog.find((recipe) => recipe.workflowId === KERA2_WORKFLOW_ID && kera2RecipeContract(recipe).ok)
      ?? productCatalog[0],
    [productCatalog],
  );
  const manualWorkflow = useMemo(
    () => findRecipe(productCatalog, manualSelection),
    [manualSelection, productCatalog],
  );
  const projectDefaultWorkflow = useMemo(
    () => projectWorkflowConfig?.imageDefault?.available
      ? findRecipe(productCatalog, projectWorkflowConfig.imageDefault)
      : undefined,
    [productCatalog, projectWorkflowConfig],
  );
  const staleProjectDefault = Boolean(
    projectWorkflowConfig?.imageDefault
      && (!projectWorkflowConfig.imageDefault.available || !projectDefaultWorkflow),
  );
  const values = useStudioStore((state) => state.values);
  const draftDirty = useStudioStore((state) => state.draftDirty);
  const validationErrors = useStudioStore((state) => state.validationErrors);
  const pendingAssetIntent = useStudioStore((state) => state.pendingAssetIntent);
  const reuseProvenance = useStudioStore((state) => state.reuseProvenance);
  const setSelectedWorkflow = useStudioStore((state) => state.setSelectedWorkflow);
  const setValue = useStudioStore((state) => state.setValue);
  const removeValue = useStudioStore((state) => state.removeValue);
  const setValidationErrors = useStudioStore((state) => state.setValidationErrors);
  const currentTask = useTaskStore((state) => state.currentTask);
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [missingAssetFields, setMissingAssetFields] = useState<Set<string>>(new Set());
  const [studioMode, setStudioMode] = useState<StudioMode>("batch");
  const [experimentFocusBatchId, setExperimentFocusBatchId] = useState<string>();
  const [experimentContexts, setExperimentContexts] = useState<Record<string, ExperimentContext>>({});
  const [promptExperimentDimensions, setPromptExperimentDimensions] = useState<ExperimentDimension[]>([]);
  const [dashboardPromptTargetFieldKey, setDashboardPromptTargetFieldKey] = useState("");
  const [assetIntentTargets, setAssetIntentTargets] = useState<RecipeField[]>([]);
  const [templateEditorOpen, setTemplateEditorOpen] = useState(false);
  const [templateName, setTemplateName] = useState("");
  const [templateDescription, setTemplateDescription] = useState("");
  const [templateSaving, setTemplateSaving] = useState(false);
  const [templateError, setTemplateError] = useState<string>();
  const handleAssetAvailabilityChange = useCallback((key: string, available: boolean) => {
    setMissingAssetFields((current) => {
      const next = new Set(current);
      if (available) {
        if (!current.has(key)) return current;
        next.delete(key);
      } else {
        if (current.has(key)) return current;
        next.add(key);
      }
      return next;
    });
  }, []);
  const presetController = useGenerationPresetController({
    projectId,
    selectedWorkflow,
    onPresetApplied: () => setMissingAssetFields(new Set()),
    onNotice: setNotice,
  });

  useEffect(() => {
    // Project switches are reset by App.openProject before the new workspace
    // mounts. Keep the current store draft here so history/preset loading is
    // not cleared when the studio is mounted after navigation.
    setMissingAssetFields(new Set());
    setNotice(null);
    setStudioMode("batch");
    setExperimentFocusBatchId(undefined);
    setExperimentContexts({});
    setPromptExperimentDimensions([]);
    setDashboardPromptTargetFieldKey("");
    setTemplateEditorOpen(false);
    setManualSelection(undefined);
    setProjectWorkflowConfig(undefined);
    void getProjectWorkflowConfig(projectId)
      .then(setProjectWorkflowConfig)
      .catch(() => setProjectWorkflowConfig({ projectId, videoModeOverrides: [] }));
  }, [projectId]);

  async function saveProjectTemplate() {
    if (!selectedWorkflow || !templateName.trim()) return;
    setTemplateSaving(true); setTemplateError(undefined);
    try {
      await createProjectTemplate({ name: templateName, description: templateDescription.trim() || undefined, workflowVersionId: selectedWorkflow.workflowVersionId, recipeId: selectedWorkflow.recipeId, values });
      setTemplateEditorOpen(false); setTemplateName(""); setTemplateDescription("");
      setNotice("项目模板已保存；素材输入不会写入模板。");
    } catch (value) { setTemplateError(toUserMessage(value)); } finally { setTemplateSaving(false); }
  }

  useEffect(() => {
    const explicitDraft = selectedWorkflow
      ? productCatalog.find((recipe) => (
        recipe.workflowVersionId === selectedWorkflow.workflowVersionId
        && recipe.recipeId === selectedWorkflow.recipeId
      ))
      : undefined;
    const next = explicitDraft ?? manualWorkflow ?? projectDefaultWorkflow ?? recommendedWorkflow;
    if (
      next?.workflowVersionId !== selectedWorkflow?.workflowVersionId ||
      next?.recipeId !== selectedWorkflow?.recipeId
    ) {
      setSelectedWorkflow(next);
      setMissingAssetFields(new Set());
    }
  }, [manualWorkflow, productCatalog, projectDefaultWorkflow, recommendedWorkflow, selectedWorkflow, setSelectedWorkflow]);

  useEffect(() => {
    if (!manualSelection || manualWorkflow || !productCatalog.length) return;
    setManualSelection(undefined);
    setNotice("本次手动选择的工作流当前不可用，已切换到项目默认或推荐工作流。");
  }, [manualSelection, manualWorkflow, productCatalog.length]);

  function applyPendingAsset(field: RecipeField, replaceSingle: boolean) {
    if (!selectedWorkflow || !pendingAssetIntent) return;
    const result = assignAssetToField(field, useStudioStore.getState().values, pendingAssetIntent.assetId, replaceSingle);
    if (result.kind === "requires_confirmation") {
      if (window.confirm("当前输入已有素材，是否替换当前素材？")) {
        applyPendingAsset(field, true);
      }
      return;
    }
    setAssetIntentTargets([]);
    useStudioStore.getState().clearPendingAssetIntent();
    if (result.kind === "max_items") {
      setNotice(`“${field.label}”已达到素材数量上限。`);
      return;
    }
    if (result.kind !== "applied") {
      setNotice("当前工作流没有可使用此素材的输入项。");
      return;
    }
    useStudioStore.getState().loadDraft(selectedWorkflow, result.values);
    setMissingAssetFields((current) => {
      const next = new Set(current);
      next.delete(field.key);
      return next;
    });
    setNotice("已将素材加入创作。");
  }

  useEffect(() => {
    if (!pendingAssetIntent || !selectedWorkflow) return;
    if (pendingAssetIntent.projectId !== projectId) {
      useStudioStore.getState().clearPendingAssetIntent();
      setAssetIntentTargets([]);
      setNotice("素材属于其他项目，已取消使用。");
      return;
    }
    const targets = compatibleAssetFields(selectedWorkflow, pendingAssetIntent.assetType);
    if (!targets.length) {
      useStudioStore.getState().clearPendingAssetIntent();
      setAssetIntentTargets([]);
      setNotice("当前工作流没有可使用此素材的输入项。");
      return;
    }
    if (targets.length > 1) {
      setAssetIntentTargets(targets);
      return;
    }
    applyPendingAsset(targets[0], false);
  }, [pendingAssetIntent, projectId, selectedWorkflow]);

  const hasUnsupportedField = useMemo(
    () => selectedWorkflow?.fields.some((field) => !["textarea", "integer", "number", "seed", "image", "images", "video", "audio", "videos", "audios"].includes(field.type)) ?? false,
    [selectedWorkflow],
  );
  const imageCapability = selectedWorkflow ? imageRecipeCapability(selectedWorkflow) : undefined;
  const imagePrompt = imageCapability?.promptField;
  const krea2Contract = selectedWorkflow ? kera2RecipeContract(selectedWorkflow) : undefined;
  const krea2ConfigError = selectedWorkflow?.workflowId === KERA2_WORKFLOW_ID && krea2Contract && !krea2Contract.ok
    ? krea2Contract.reason
    : undefined;
  const genericImageNotice = selectedWorkflow && imageCapability && !imageCapability.batchPromptCompatible
    ? imageCapability.reason
    : undefined;
  const krea2ResolutionPresets = useMemo(
    () => selectedWorkflow && krea2Contract?.ok
      ? resolutionPresetsForRecipe(selectedWorkflow, KREA2_RESOLUTION_PRESETS)
      : [],
    [krea2Contract?.ok, selectedWorkflow],
  );
  const productionPolicy = productionInteractionPolicy(productionAdmission.busy);
  const errors = selectedWorkflow ? validateRecipeValues(selectedWorkflow, values) : {};
  const canGenerate = Boolean(
    comfyConnected &&
      taskEventsReady &&
      productionPolicy.canSubmitGeneration &&
      selectedWorkflow &&
      !krea2ConfigError &&
      !hasUnsupportedField &&
      missingAssetFields.size === 0 &&
      Object.keys(errors).length === 0,
  );
  const canAddToBatch = Boolean(
    selectedWorkflow
      && imageCapability?.batchPromptCompatible
      && !krea2ConfigError
      && !hasUnsupportedField
      && missingAssetFields.size === 0
      && Object.keys(errors).length === 0,
  );
  const canExperimentBase = Boolean(
    canAddToBatch &&
      comfyConnected &&
      taskEventsReady &&
      productionPolicy.canSubmitLocalBatch,
  );
  const blockedReason = generationBlockedReason({
    productionBusy: productionAdmission.busy,
    comfyConnected,
    taskEventsReady,
    taskEventError,
    missingAsset: missingAssetFields.size > 0,
    validationError: Object.keys(errors).length > 0,
    unsupportedField: hasUnsupportedField,
  });
  const generationController = useGenerationSubmissionController({
    projectId,
    selectedWorkflow,
    values,
    configurationError: krea2ConfigError,
    productionAdmission,
    comfyConnected,
    taskEventsReady,
    taskEventError,
    missingAsset: missingAssetFields.size > 0,
    unsupportedField: hasUnsupportedField,
    onValidationErrors: setValidationErrors,
    onNotice: setNotice,
  });

  const batchController = useGenerationBatchController({
    projectId,
    productCatalog,
    selectedWorkflow,
    values,
    configurationError: krea2ConfigError,
    genericImageNotice,
    hasUnsupportedField,
    missingAsset: missingAssetFields.size > 0,
    canSubmitLocalBatch: productionPolicy.canSubmitLocalBatch,
    comfyConnected,
    taskEventsReady,
    onValidationErrors: setValidationErrors,
    onBatchCreated: setExperimentFocusBatchId,
    onProductionAdmissionChanged,
  });

  async function refreshWorkflows() {
    setRefreshing(true);
    setNotice(null);
    try {
      await refreshWorkflowLibrary();
      await onCatalogChanged();
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setRefreshing(false);
    }
  }

  function updateKrea2Resolution(next: { width?: number; height?: number }) {
    if (next.width === undefined) removeValue("width");
    else setValue("width", { type: "integer", value: next.width });
    if (next.height === undefined) removeValue("height");
    else setValue("height", { type: "integer", value: next.height });
  }

  async function submitExperimentPlan(plan: ExperimentPlan) {
    if (!selectedWorkflow) return;
    if (!canExperimentBase) {
      setNotice(blockedReason ?? "当前基础草稿尚未满足实验队列提交条件。");
      return;
    }
    setNotice(null);
    try {
      const created = await createProductionQueue({
        projectId,
        name: `实验 · ${workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name)} · ${formatDateTime(new Date().toISOString())}`,
        continueOnFailure: true,
        items: plan.items.map((item) => ({
          workflowVersionId: plan.workflowVersionId,
          recipeId: plan.recipeId,
          values: cloneGenerationValues(item.values),
        })),
      });
      setExperimentContexts((current) => ({
        ...current,
        [created.id]: {
          recipe: selectedWorkflow,
          baseValues: cloneGenerationValues(plan.baseValues),
        },
      }));
      setExperimentFocusBatchId(created.id);
      setStudioMode("batch");
      try {
        await onProductionAdmissionChanged();
      } catch {
        // The queue is already persisted; a status refresh failure must not
        // prevent the normal queue runner from being started.
      }
      try {
        await startProductionQueue(projectId, created.id);
        setNotice(`实验队列已加入并开始执行，共 ${created.total} 项；任务将严格按顺序运行。`);
      } catch (startError: unknown) {
        setNotice(`实验队列已保存，共 ${created.total} 项；开始执行失败：${toUserMessage(startError)}。可在生产队列中手动开始。`);
      }
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    }
  }

  async function promoteExperimentWinner(
    draft: ReusableGenerationDraft,
    source: { batchName: string; taskId: string },
  ) {
    if (draft.projectId !== projectId) {
      setNotice("生产结果属于其他项目，无法加载到当前创作。");
      return;
    }
    const workflow = productCatalog.find(
      (recipe) => recipe.workflowVersionId === draft.workflowVersionId && recipe.recipeId === draft.recipeId,
    );
    if (!workflow) {
      setNotice("生产结果对应的工作流版本已不在运行目录中，请先刷新工作流列表。");
      return;
    }
    useStudioStore.getState().loadDraft(workflow, cloneGenerationValues(draft.values));
    useStudioStore.getState().setReuseProvenance({
      workflowName: draft.workflowName,
      createdAt: draft.createdAt,
      sourceBatchName: source.batchName,
      sourceTaskId: source.taskId,
    });
    setMissingAssetFields(new Set());
    setStudioMode("single");
    setNotice(draft.missingAssetIds.length
      ? "已将生产结果加载到 Studio，但部分素材缺失，请替换后再生成；未自动提交任务。"
      : "已将生产结果作为下一轮起点加载到 Studio，未自动提交生成任务。",
    );
  }

  function applyRuntimeProfile(nextValues: typeof values) {
    if (!selectedWorkflow) return;
    useStudioStore.getState().loadDraft(selectedWorkflow, nextValues);
    setMissingAssetFields(new Set());
  }

  function usePromptVersionsForExperiment(fieldKey: string, versions: PromptVersionView[]) {
    setPromptExperimentDimensions([{
      fieldKey,
      values: versions.map((version) => ({ type: "string", value: version.text })),
    }]);
    setStudioMode("experiment");
  }

  function selectWorkflowFromUx(workflow: RecipeViewModel) {
    if (workflow.workflowVersionId === selectedWorkflow?.workflowVersionId && workflow.recipeId === selectedWorkflow.recipeId) return;
    if (draftDirty && !window.confirm("当前 Studio 草稿有未保存修改，确认切换工作流吗？")) return;
    setManualSelection(recipeRef(workflow));
    const currentState = useStudioStore.getState();
    if (currentState.selectedWorkflow) {
      currentState.loadDraft(workflow, migrateGenerationValues(currentState.selectedWorkflow, workflow, currentState.values));
    } else {
      setSelectedWorkflow(workflow);
    }
    setAssetIntentTargets([]);
    setMissingAssetFields(new Set());
    presetController.setPresetEditorOpen(false);
    setPromptExperimentDimensions([]);
    setDashboardPromptTargetFieldKey("");
  }

  function restoreRecommendedWorkflow() {
    if (!recommendedWorkflow) return;
    if (draftDirty && !window.confirm("当前 Studio 草稿有未保存修改，确认恢复推荐工作流吗？")) return;
    setManualSelection(undefined);
    const fallbackWorkflow = projectDefaultWorkflow ?? recommendedWorkflow;
    if (!fallbackWorkflow) return;
    const currentState = useStudioStore.getState();
    if (currentState.selectedWorkflow) {
      currentState.loadDraft(fallbackWorkflow, migrateGenerationValues(currentState.selectedWorkflow, fallbackWorkflow, currentState.values));
    } else {
      setSelectedWorkflow(fallbackWorkflow);
    }
    setAssetIntentTargets([]);
    setMissingAssetFields(new Set());
    presetController.setPresetEditorOpen(false);
    setPromptExperimentDimensions([]);
    setDashboardPromptTargetFieldKey("");
  }

  function continueRecentWorkflow(_record: RecentWorkflowRecord, recipe?: RecipeViewModel) {
    if (!recipe) {
      setNotice("历史工作流当前不可用，无法创建新的创作入口。");
      return;
    }
    selectWorkflowFromUx(recipe);
  }

  async function useRecentPrompt(entry: PromptEntryView, fieldKey: string) {
    if (!selectedWorkflow || !fieldKey) {
      setNotice("请选择提示词要填入的文字字段。");
      return;
    }
    const field = selectedWorkflow.fields.find((item) => item.key === fieldKey && item.type === "textarea");
    if (!field) {
      setNotice("当前工作流没有这个文字字段。");
      return;
    }
    try {
      const detail = await getPromptLibraryEntry(projectId, entry.id);
      const version = detail.versions[detail.versions.length - 1];
      if (!version) {
        setNotice("该提示词还没有可应用的版本。");
        return;
      }
      const currentValue = values[fieldKey];
      const currentText = currentValue?.type === "string" ? currentValue.value : "";
      if (entry.kind === "prompt" && currentText && !window.confirm("目标文字输入已有内容，是否替换？")) return;
      const result = entry.kind === "snippet"
        ? applyPromptSnippetToStudio(selectedWorkflow, values, fieldKey, version.text, "append")
        : applyPromptVersionToStudio(selectedWorkflow, values, fieldKey, version);
      if (!result.values) {
        setNotice(result.issue ?? "无法应用提示词。");
        return;
      }
      useStudioStore.getState().loadDraft(selectedWorkflow, result.values);
      setMissingAssetFields(new Set());
      setNotice(`${entry.kind === "snippet" ? "片段已追加" : "提示词已应用到 Studio"}；未自动生成。`);
    } catch (value: unknown) {
      setNotice(toUserMessage(value));
    }
  }

  if (!productCatalog.length) {
    return (
      <NoWorkflowGuide
        refreshing={refreshing}
        notice={notice}
        onOpenWorkflows={onOpenWorkflows}
        onReconnectComfy={onReconnectComfy}
        onRefresh={() => void refreshWorkflows()}
      />
    );
  }

  return (
    <>
      <section className={`studio-panel${studioMode === "batch" ? " studio-panel-batch" : ""}`}>
        <section className="product-ready-banner" aria-label="图片工作流状态">
          <div>
            <span className="section-label">批量图片</span>
            <strong>{selectedWorkflow ? `${workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name)} 已就绪` : "图片工作流"}</strong>
          </div>
          <span>自动推荐稳定工作流；手动更换后，工作流版本和配方会随任务冻结。</span>
        </section>
        <StudioModeTabs mode={studioMode} onChange={setStudioMode} />
        <WorkflowSelector
          stage="image"
          candidates={productCatalog}
          selected={selectedWorkflow}
          recommended={projectDefaultWorkflow ?? recommendedWorkflow}
          selectionSource={selectedWorkflow && manualSelection && sameRecipeRef(selectedWorkflow, manualSelection) ? "manual" : selectedWorkflow && projectDefaultWorkflow && sameRecipeRef(selectedWorkflow, projectDefaultWorkflow) ? "project_default" : selectedWorkflow && recommendedWorkflow && sameRecipeRef(selectedWorkflow, recommendedWorkflow) ? "recommended" : "compatible"}
          onSelect={selectWorkflowFromUx}
          onRestoreRecommendation={restoreRecommendedWorkflow}
          onOpenWorkflows={onOpenWorkflows ? () => onOpenWorkflows() : undefined}
        />
        {staleProjectDefault && <p className="settings-warning" role="alert">项目图片默认工作流已失效，当前仅临时使用兼容/推荐工作流；请在项目设置中重新选择或清除绑定。</p>}
        {selectedWorkflow && (
          <>
            {studioMode !== "batch" && <>
            <div className="studio-input-heading">
              <div>
                <span className="section-label">公开参数</span>
                <h2>{workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name)}</h2>
              </div>
              <small>{workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name)}</small>
            </div>
            <CreationModeHint recipe={selectedWorkflow} />
            <CreationDashboard
              projectId={projectId}
              catalog={productCatalog}
              selectedWorkflow={selectedWorkflow}
              promptTargetFieldKey={dashboardPromptTargetFieldKey}
              onPromptTargetFieldChange={setDashboardPromptTargetFieldKey}
              onUsePrompt={(entry, fieldKey) => void useRecentPrompt(entry, fieldKey)}
              onContinueWorkflow={continueRecentWorkflow}
              onFocusQueue={(batchId) => { setStudioMode("batch"); setExperimentFocusBatchId(batchId); }}
              onAdmissionChanged={onProductionAdmissionChanged}
            />
            {reuseProvenance && (
              <div className="studio-provenance" role="status">
                <strong>{reuseProvenance.sourceBatchName ? "已从生产队列结果加载" : "已加载历史任务参数"}</strong>
                <span>
                  {reuseProvenance.sourceBatchName
                    ? `${reuseProvenance.sourceBatchName} · 任务 ${reuseProvenance.sourceTaskId ?? "未知"} · ${formatDateTime(reuseProvenance.createdAt)}`
                    : `${reuseProvenance.workflowName} · ${formatDateTime(reuseProvenance.createdAt)}`}
                </span>
              </div>
            )}
            {assetIntentTargets.length > 1 && pendingAssetIntent && (
              <section className="asset-intent-targets" aria-label="选择素材用途">
                <div>
                  <strong>选择素材用途</strong>
                  <p>请选择要填入的输入项，当前素材不会自动提交生成。</p>
                </div>
                <div className="asset-intent-target-list">
                  {assetIntentTargets.map((field) => (
                    <button key={field.key} type="button" onClick={() => applyPendingAsset(field, false)}>
                      {field.label} · {fieldTypeLabel(field.type)}
                    </button>
                  ))}
                  <button
                    type="button"
                    className="quiet-button"
                    onClick={() => {
                      useStudioStore.getState().clearPendingAssetIntent();
                      setAssetIntentTargets([]);
                    }}
                  >
                    取消
                  </button>
                </div>
              </section>
            )}
            <div className="preset-toolbar" aria-label="预设管理">
              <label>
                <span>预设</span>
                <select
                  aria-label="已保存预设"
                  value={presetController.selectedPresetId}
                  onChange={(event) => {
                    const preset = presetController.presets.find((item) => item.id === event.target.value);
                    if (preset) presetController.applyPreset(preset);
                    else presetController.clearSelection();
                  }}
                  disabled={presetController.presetLoading}
                >
                  <option value="">选择已保存的预设</option>
                  {presetController.presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}
                </select>
              </label>
              <div className="preset-actions">
                <button type="button" onClick={presetController.openEditor} disabled={presetController.presetLoading}>{presetController.selectedPresetId ? "另存为" : "保存当前"}</button>
                <button type="button" onClick={() => void presetController.savePresetChanges()} disabled={presetController.presetLoading || !presetController.selectedPresetId}>更新预设</button>
                <button type="button" className="quiet-button" onClick={() => void presetController.togglePreferredPreset()} disabled={presetController.presetLoading || !presetController.selectedPresetId}>
                  {presetController.preferredPresetId === presetController.selectedPresetId ? "取消默认" : "设为默认"}
                </button>
                <button type="button" className="quiet-button" onClick={() => void presetController.removePreset()} disabled={presetController.presetLoading || !presetController.selectedPresetId}>删除预设</button>
              </div>
            </div>
            {presetController.preferredPresetId && <p className="preset-default-note" role="status">当前工作流会优先加载默认预设。</p>}
            {presetController.presetEditorOpen && (
              <div className="preset-inline-editor" aria-label="保存预设">
                <label>
                  <span>预设名称</span>
                  <input aria-label="预设名称" autoFocus value={presetController.presetName} maxLength={80} onChange={(event) => presetController.setPresetName(event.target.value)} placeholder="例如：柔光人像" />
                </label>
                <button type="button" onClick={() => void presetController.savePreset()} disabled={presetController.presetLoading}>保存</button>
                <button type="button" className="quiet-button" onClick={() => presetController.setPresetEditorOpen(false)} disabled={presetController.presetLoading}>取消</button>
              </div>
            )}
            {presetController.presetError && <p className="error-message">预设：{presetController.presetError}</p>}
            <div className="project-template-toolbar">
              <button type="button" className="quiet-button" onClick={() => { setTemplateError(undefined); setTemplateEditorOpen(true); }}>保存为项目模板</button>
              <small>保存当前文字、数字和种子；不保存图片、视频或音频素材。</small>
            </div>
            {templateEditorOpen && (
              <section className="project-template-editor" aria-label="保存为项目模板">
                <label><span>模板名称</span><input autoFocus maxLength={80} value={templateName} placeholder="例如：Krea2 海报起点" onChange={(event) => setTemplateName(event.target.value)} /></label>
                <label><span>模板说明 <small>可选</small></span><textarea rows={2} maxLength={500} value={templateDescription} onChange={(event) => setTemplateDescription(event.target.value)} /></label>
                <div><button type="button" onClick={() => void saveProjectTemplate()} disabled={templateSaving || !templateName.trim()}>{templateSaving ? "正在保存..." : "保存模板"}</button><button type="button" className="quiet-button" onClick={() => setTemplateEditorOpen(false)} disabled={templateSaving}>取消</button></div>
                {templateError && <p className="error-message" role="alert">{templateError}</p>}
              </section>
            )}
            <RuntimeParameterProfilePanel recipe={selectedWorkflow} values={values} onApply={applyRuntimeProfile} />
            <PromptLibraryPanel
              projectId={projectId}
              recipe={selectedWorkflow}
              values={values}
              onApplyValues={(nextValues) => {
                useStudioStore.getState().loadDraft(selectedWorkflow, nextValues);
                setMissingAssetFields(new Set());
              }}
              onUseForExperiment={usePromptVersionsForExperiment}
            />
            <DynamicFormRenderer
              recipe={selectedWorkflow}
              values={values}
              validationErrors={validationErrors}
              hiddenFieldKeys={krea2Contract?.ok ? ["width", "height"] : []}
              onChange={(key, value) => (value ? setValue(key, value) : removeValue(key))}
              onGenerate={() => void generationController.generate()}
              projectId={projectId}
              onImageAssetAvailabilityChange={handleAssetAvailabilityChange}
            />
            </>}
            {krea2ConfigError && <p className="error-message" role="alert">{krea2ConfigError}</p>}
            {genericImageNotice && <p className="disabled-note" role="status">{genericImageNotice}</p>}
            {selectedWorkflow.workflowId === KERA2_WORKFLOW_ID && krea2Contract?.ok && (
              <ResolutionControl
                widthField={krea2Contract.contract.widthField}
                heightField={krea2Contract.contract.heightField}
                width={values.width?.type === "integer" ? values.width.value : undefined}
                height={values.height?.type === "integer" ? values.height.value : undefined}
                presets={krea2ResolutionPresets}
                disabled={generationController.creating || batchController.batchSubmitting}
                onChange={updateKrea2Resolution}
              />
            )}
            {studioMode === "production" && (
              <ProductionRunPanel
                projectId={projectId}
                catalog={catalog}
                baseRecipe={selectedWorkflow}
                baseValues={values}
                onOpenTask={onOpenTask}
                onAdmissionChanged={onProductionAdmissionChanged}
              />
            )}
            {studioMode === "experiment" && (
              <>
                <WorkflowBenchmarkPanel
                  projectId={projectId}
                  catalog={catalog}
                  baseRecipe={selectedWorkflow}
                  baseValues={values}
                  baseReady={canExperimentBase}
                  blockedReason={blockedReason}
                  onOpenTask={onOpenTask}
                  onAdmissionChanged={onProductionAdmissionChanged}
                  onCreated={(created) => {
                    setExperimentFocusBatchId(created.productionBatchId);
                    setNotice(`基准实验已保存：${created.candidates.length} 个候选已进入普通生产队列。`);
                  }}
                />
                <details className="legacy-experiment-planner">
                  <summary>参数变体草稿（兼容旧实验计划）</summary>
                  <ExperimentPlannerPanel
                    recipe={selectedWorkflow}
                    baseValues={values}
                    baseReady={canExperimentBase}
                    blockedReason={blockedReason}
                    initialDimensions={promptExperimentDimensions}
                    onSubmit={submitExperimentPlan}
                  />
                </details>
              </>
            )}
            {studioMode !== "batch" && (
              <GenerationActionBar
                creating={generationController.creating}
                canGenerate={canGenerate}
                canAddToBatch={canAddToBatch}
                blockedReason={blockedReason}
                batchCount={batchController.batchItems.length}
                onGenerate={() => void generationController.generate()}
                onAddToBatch={batchController.addCurrentToBatch}
              />
            )}
            {studioMode === "batch" && <section className="batch-production-view" aria-label="批量生产">
              <section className="batch-panel" aria-label="批量图片提示词">
                <div className="batch-panel-header">
                  <div>
                    <span className="section-label">提示词列表</span>
                  <p>{imageCapability?.batchPromptCompatible ? "每张提示词卡片都会创建一个图片任务；创建批次后工作流版本和参数会冻结。" : "当前工作流没有标准提示词输入，提示词列表批量生成不可用；请使用上方通用参数模式单次生成。"}</p>
                  </div>
                <div className="batch-actions">
                  <button type="button" className="quiet-button" onClick={batchController.addCurrentToBatch} disabled={batchController.batchSubmitting || !imageCapability?.batchPromptCompatible}>
                    添加当前提示词
                  </button>
                  <button type="button" className="quiet-button" onClick={batchController.addBlankPromptCard} disabled={batchController.batchSubmitting || !imageCapability?.batchPromptCompatible}>
                    添加提示词
                  </button>
                  <label className="quiet-button batch-file-button">
                    导入 JSON
                    <input
                      type="file"
                      accept="application/json,.json"
                      disabled={batchController.batchSubmitting}
                      onChange={(event) => {
                        const file = event.currentTarget.files?.[0];
                        event.currentTarget.value = "";
                        void batchController.importBatchTaskList(file);
                      }}
                    />
                  </label>
                  <button
                    type="button"
                    className="quiet-button"
                    onClick={batchController.clearBatch}
                    disabled={batchController.batchSubmitting || !batchController.batchItems.length}
                  >
                    清空
                  </button>
                </div>
              </div>
              <details className="batch-optional-parameters">
                <summary>
                  <span>
                    <span className="section-label">批量参数</span>
                    <strong>可选共享参数</strong>
                  </span>
                  <small>仅影响之后添加的项目</small>
                </summary>
                <p>提示词在下方列表中单独编辑；这里的数字和种子参数会作为新项目的默认值。</p>
                <DynamicFormRenderer
                  recipe={selectedWorkflow}
                  values={values}
                  validationErrors={validationErrors}
                  hiddenFieldKeys={krea2Contract?.ok ? [imagePrompt?.key ?? "prompt", "width", "height"] : [imagePrompt?.key ?? "prompt"]}
                  onChange={(key, value) => (value ? setValue(key, value) : removeValue(key))}
                  onGenerate={() => void generationController.generate()}
                  projectId={projectId}
                  onImageAssetAvailabilityChange={handleAssetAvailabilityChange}
                />
              </details>
              <div className="batch-prompt-paste">
                <label>
                  <span>直接粘贴提示词</span>
                  <textarea
                    rows={4}
                    value={batchController.batchPasteText}
                    onChange={(event) => batchController.setBatchPasteText(event.target.value)}
                    placeholder="每张提示词之间留一个空行……"
                    disabled={batchController.batchSubmitting}
                  />
                </label>
                  <button type="button" onClick={batchController.splitPastedPrompts} disabled={batchController.batchSubmitting || !imageCapability?.batchPromptCompatible || !batchController.batchPasteText.trim()}>
                  按空行拆分
                </button>
              </div>
              {batchController.batchItems.length ? (
                <ol className="batch-list">
                  {batchController.batchItems.map((item, index) => (
                    <li key={item.id} className="batch-prompt-card">
                      <div className="batch-prompt-card-header">
                        <strong>提示词 #{index + 1}</strong>
                        <span>{item.workflowName}</span>
                      </div>
                      <label>
                        <span>提示词内容</span>
                        <textarea
                          rows={4}
                          value={batchController.batchPrompt(item)}
                          onChange={(event) => batchController.updateBatchPrompt(item.id, event.target.value)}
                          disabled={batchController.batchSubmitting}
                          placeholder="输入这张图片的生成提示词……"
                        />
                      </label>
                      <div className="batch-prompt-card-actions">
                        <button type="button" className="quiet-button" onClick={() => batchController.copyBatchItem(item.id)} disabled={batchController.batchSubmitting}>复制</button>
                        <button type="button" className="quiet-button" onClick={() => batchController.moveBatchItem(item.id, -1)} disabled={batchController.batchSubmitting || index === 0}>上移</button>
                        <button type="button" className="quiet-button" onClick={() => batchController.moveBatchItem(item.id, 1)} disabled={batchController.batchSubmitting || index === batchController.batchItems.length - 1}>下移</button>
                        <button type="button" className="quiet-button danger-button" onClick={() => batchController.removeBatchItem(item.id)} disabled={batchController.batchSubmitting}>删除</button>
                      </div>
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="disabled-note">暂未添加提示词卡片。</p>
              )}
              <button
                type="button"
                onClick={() => void batchController.submitBatch()}
                disabled={
                  !batchController.batchItems.length ||
                  !imageCapability?.batchPromptCompatible ||
                  batchController.batchSubmitting ||
                  !comfyConnected ||
                  !taskEventsReady ||
                  !productionPolicy.canSubmitLocalBatch
                }
              >
                {batchController.batchSubmitting ? "正在创建..." : `创建图片批次（${batchController.batchItems.length}）`}
              </button>
              {batchController.batchNotice && <p className="disabled-note">{batchController.batchNotice}</p>}
              </section>
              <ProductionQueuePanel
                projectId={projectId}
                batchItems={batchController.batchItems}
                comfyConnected={comfyConnected}
                variant="inline"
                hideCreate
                focusBatchId={experimentFocusBatchId
                  ?? focusProductionBatchId
                  ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
                onAdmissionChanged={onProductionAdmissionChanged}
                onFocusedBatchOpened={() => {
                  setExperimentFocusBatchId(undefined);
                  onProductionBatchFocused();
                }}
                onOpenTask={onOpenTask}
                experimentContexts={experimentContexts}
                onPromoteWinner={promoteExperimentWinner}
              />
            </section>}
          </>
        )}
        {notice && <p className="studio-notice" role="status">{notice}</p>}
      </section>
      {studioMode !== "batch" && <CreationResultPanel
        projectId={projectId}
        task={currentTask}
        cancelling={generationController.cancelling}
        onCancel={() => void generationController.cancelCurrentTask()}
        onOpenTask={currentTask ? () => onOpenTask(currentTask.id) : undefined}
      />}
    </>
  );
}
