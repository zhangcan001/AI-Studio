import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getPromptLibraryEntry,
  getProjectWorkflowConfig,
  refreshWorkflowLibrary,
} from "../../services/tauriClient";
import { useStudioStore } from "../../stores/studioStore";
import { useTaskStore } from "../../stores/taskStore";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, workflowDisplayName } from "../../i18n/statusLabels";
import { DynamicFormRenderer, validateRecipeValues } from "./DynamicFormRenderer";
import { ProductionQueuePanel } from "./ProductionQueuePanel";
import { productionInteractionPolicy } from "./productionQueuePolicy";
import { CreationResultPanel } from "./CreationResultPanel";
import { GenerationActionBar } from "./GenerationActionBar";
import { generationBlockedReason } from "./generationBlockedReason";
import { StudioModeTabs, type StudioMode } from "./StudioModeTabs";
import { NoWorkflowGuide } from "./NoWorkflowGuide";
import { CreationModeHint } from "../runtime/CreationModeHint";
import { RuntimeParameterProfilePanel } from "../runtime/RuntimeParameterProfilePanel";
import { ExperimentPlannerPanel } from "../experiments/ExperimentPlannerPanel";
import { WorkflowBenchmarkPanel } from "../experiments/WorkflowBenchmarkPanel";
import { ProductionRunPanel } from "../production/ProductionRunPanel";
import { PromptLibraryPanel } from "../prompts/PromptLibraryPanel";
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
import { useGenerationExperimentController } from "./hooks/useGenerationExperimentController";
import { useGenerationProjectTemplateController } from "./hooks/useGenerationProjectTemplateController";
import { useGenerationAssetIntentController } from "./hooks/useGenerationAssetIntentController";
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
  const [dashboardPromptTargetFieldKey, setDashboardPromptTargetFieldKey] = useState("");
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
  const handleAssetFieldResolved = useCallback((key: string) => {
    setMissingAssetFields((current) => {
      if (!current.has(key)) return current;
      const next = new Set(current);
      next.delete(key);
      return next;
    });
  }, []);
  const presetController = useGenerationPresetController({
    projectId,
    selectedWorkflow,
    onPresetApplied: () => setMissingAssetFields(new Set()),
    onNotice: setNotice,
  });
  const assetIntentController = useGenerationAssetIntentController({
    projectId,
    selectedWorkflow,
    onNotice: setNotice,
    onAssetFieldResolved: handleAssetFieldResolved,
  });

  useEffect(() => {
    // Project switches are reset by App.openProject before the new workspace
    // mounts. Keep the current store draft here so history/preset loading is
    // not cleared when the studio is mounted after navigation.
    setMissingAssetFields(new Set());
    setNotice(null);
    setStudioMode("batch");
    setDashboardPromptTargetFieldKey("");
    setManualSelection(undefined);
    setProjectWorkflowConfig(undefined);
    void getProjectWorkflowConfig(projectId)
      .then(setProjectWorkflowConfig)
      .catch(() => setProjectWorkflowConfig({ projectId, videoModeOverrides: [] }));
  }, [projectId]);

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

  const experimentController = useGenerationExperimentController({
    projectId,
    selectedWorkflow,
    productCatalog,
    canExperimentBase,
    blockedReason,
    onNotice: setNotice,
    onProductionAdmissionChanged,
    onStudioModeChange: setStudioMode,
    onClearMissingAssetFields: () => setMissingAssetFields(new Set()),
  });

  const projectTemplateController = useGenerationProjectTemplateController({
    projectId,
    selectedWorkflow,
    values,
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
    onBatchCreated: experimentController.focusBatch,
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

  function applyRuntimeProfile(nextValues: typeof values) {
    if (!selectedWorkflow) return;
    useStudioStore.getState().loadDraft(selectedWorkflow, nextValues);
    setMissingAssetFields(new Set());
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
    setMissingAssetFields(new Set());
    presetController.setPresetEditorOpen(false);
    experimentController.clearPromptExperimentDimensions();
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
    setMissingAssetFields(new Set());
    presetController.setPresetEditorOpen(false);
    experimentController.clearPromptExperimentDimensions();
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
              onFocusQueue={(batchId) => { setStudioMode("batch"); experimentController.focusBatch(batchId); }}
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
            {assetIntentController.assetIntentTargets.length > 1 && assetIntentController.pendingAssetIntent && (
              <section className="asset-intent-targets" aria-label="选择素材用途">
                <div>
                  <strong>选择素材用途</strong>
                  <p>请选择要填入的输入项，当前素材不会自动提交生成。</p>
                </div>
                <div className="asset-intent-target-list">
                  {assetIntentController.assetIntentTargets.map((field) => (
                    <button key={field.key} type="button" onClick={() => assetIntentController.applyToTarget(field)}>
                      {field.label} · {fieldTypeLabel(field.type)}
                    </button>
                  ))}
                  <button
                    type="button"
                    className="quiet-button"
                    onClick={assetIntentController.cancel}
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
              <button type="button" className="quiet-button" onClick={projectTemplateController.openEditor}>保存为项目模板</button>
              <small>保存当前文字、数字和种子；不保存图片、视频或音频素材。</small>
            </div>
            {projectTemplateController.templateEditorOpen && (
              <section className="project-template-editor" aria-label="保存为项目模板">
                <label><span>模板名称</span><input autoFocus maxLength={80} value={projectTemplateController.templateName} placeholder="例如：Krea2 海报起点" onChange={(event) => projectTemplateController.setTemplateName(event.target.value)} /></label>
                <label><span>模板说明 <small>可选</small></span><textarea rows={2} maxLength={500} value={projectTemplateController.templateDescription} onChange={(event) => projectTemplateController.setTemplateDescription(event.target.value)} /></label>
                <div><button type="button" onClick={() => void projectTemplateController.save()} disabled={projectTemplateController.templateSaving || !projectTemplateController.templateName.trim()}>{projectTemplateController.templateSaving ? "正在保存..." : "保存模板"}</button><button type="button" className="quiet-button" onClick={projectTemplateController.closeEditor} disabled={projectTemplateController.templateSaving}>取消</button></div>
                {projectTemplateController.templateError && <p className="error-message" role="alert">{projectTemplateController.templateError}</p>}
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
              onUseForExperiment={experimentController.usePromptVersionsForExperiment}
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
                  onCreated={experimentController.handleBenchmarkCreated}
                />
                <details className="legacy-experiment-planner">
                  <summary>参数变体草稿（兼容旧实验计划）</summary>
                  <ExperimentPlannerPanel
                    recipe={selectedWorkflow}
                    baseValues={values}
                    baseReady={canExperimentBase}
                    blockedReason={blockedReason}
                    initialDimensions={experimentController.promptExperimentDimensions}
                    onSubmit={experimentController.submitExperimentPlan}
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
                focusBatchId={experimentController.experimentFocusBatchId
                  ?? focusProductionBatchId
                  ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
                onAdmissionChanged={onProductionAdmissionChanged}
                onFocusedBatchOpened={() => {
                  experimentController.clearFocusBatch();
                  onProductionBatchFocused();
                }}
                onOpenTask={onOpenTask}
                experimentContexts={experimentController.experimentContexts}
                onPromoteWinner={experimentController.promoteExperimentWinner}
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
