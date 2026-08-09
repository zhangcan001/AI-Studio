import { useCallback, useEffect, useMemo, useState } from "react";
import {
  cancelTask,
  createPreset,
  createGeneration,
  createGenerationBatch,
  deletePreset,
  listPresets,
  refreshWorkflowLibrary,
  updatePreset,
} from "../../services/tauriClient";
import { useStudioStore } from "../../stores/studioStore";
import { useTaskStore } from "../../stores/taskStore";
import type { RecipeViewModel } from "../../types/generation";
import type { PresetView } from "../../types/preset";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import { toUserMessage } from "../../i18n/errorMessages";
import { workflowDisplayName } from "../../i18n/statusLabels";
import { DynamicFormRenderer, validateRecipeValues } from "./DynamicFormRenderer";
import { cloneGenerationValues, retainFailedBatchItems, type BatchDraftItem } from "./batchDraft";
import { parseBatchTaskList } from "./batchImport";
import { ImageOutput } from "./ImageOutput";
import { ProductionQueuePanel } from "./ProductionQueuePanel";
import { productionInteractionPolicy } from "./productionQueuePolicy";
import { TaskProgressCard } from "./TaskProgressCard";

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
}: Props) {
  const selectedWorkflow = useStudioStore((state) => state.selectedWorkflow);
  const values = useStudioStore((state) => state.values);
  const validationErrors = useStudioStore((state) => state.validationErrors);
  const setSelectedWorkflow = useStudioStore((state) => state.setSelectedWorkflow);
  const setValue = useStudioStore((state) => state.setValue);
  const removeValue = useStudioStore((state) => state.removeValue);
  const setValidationErrors = useStudioStore((state) => state.setValidationErrors);
  const currentTask = useTaskStore((state) => state.currentTask);
  const adoptCreatedTask = useTaskStore((state) => state.adoptCreatedTask);
  const [creating, setCreating] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [missingAssetFields, setMissingAssetFields] = useState<Set<string>>(new Set());
  const [presets, setPresets] = useState<PresetView[]>([]);
  const [selectedPresetId, setSelectedPresetId] = useState("");
  const [presetName, setPresetName] = useState("");
  const [presetLoading, setPresetLoading] = useState(false);
  const [presetError, setPresetError] = useState<string>();
  const [batchItems, setBatchItems] = useState<BatchDraftItem[]>([]);
  const [batchSubmitting, setBatchSubmitting] = useState(false);
  const [batchNotice, setBatchNotice] = useState<string>();
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

  useEffect(() => {
    useStudioStore.getState().resetDraft();
    setMissingAssetFields(new Set());
    setNotice(null);
    setPresets([]);
    setSelectedPresetId("");
    setPresetName("");
    setPresetError(undefined);
    setBatchItems([]);
    setBatchSubmitting(false);
    setBatchNotice(undefined);
  }, [projectId]);

  useEffect(() => {
    const next = selectedWorkflow
      ? catalog.find(
          (recipe) =>
            recipe.workflowVersionId === selectedWorkflow.workflowVersionId &&
            recipe.recipeId === selectedWorkflow.recipeId,
        )
      : catalog[0];
    if (
      next?.workflowVersionId !== selectedWorkflow?.workflowVersionId ||
      next?.recipeId !== selectedWorkflow?.recipeId
    ) {
      setSelectedWorkflow(next);
      setMissingAssetFields(new Set());
    }
  }, [catalog, selectedWorkflow, setSelectedWorkflow]);

  const hasUnsupportedField = useMemo(
    () => selectedWorkflow?.fields.some((field) => !["textarea", "integer", "seed", "image", "images", "video", "audio", "videos", "audios"].includes(field.type)) ?? false,
    [selectedWorkflow],
  );
  const productionPolicy = productionInteractionPolicy(productionAdmission.busy);
  const errors = selectedWorkflow ? validateRecipeValues(selectedWorkflow, values) : {};
  const canGenerate = Boolean(
    comfyConnected &&
      taskEventsReady &&
      productionPolicy.canSubmitGeneration &&
      selectedWorkflow &&
      !hasUnsupportedField &&
      missingAssetFields.size === 0 &&
      Object.keys(errors).length === 0,
  );

  useEffect(() => {
    if (!selectedWorkflow) return;
    let active = true;
    setPresetLoading(true);
    setPresetError(undefined);
    setSelectedPresetId("");
    setPresetName("");
    void listPresets(projectId, selectedWorkflow.workflowVersionId, selectedWorkflow.recipeId)
      .then((nextPresets) => {
        if (active) setPresets(nextPresets);
      })
      .catch((loadError: unknown) => {
        if (active) setPresetError(toUserMessage(loadError));
      })
      .finally(() => {
        if (active) setPresetLoading(false);
      });
    return () => {
      active = false;
    };
  }, [projectId, selectedWorkflow]);

  function applyPreset(preset: PresetView) {
    if (!selectedWorkflow) return;
    useStudioStore.getState().loadDraft(selectedWorkflow, preset.values);
    setSelectedPresetId(preset.id);
    setPresetName(preset.name);
    setMissingAssetFields(new Set());
    setPresetError(undefined);
  }

  async function savePreset() {
    if (!selectedWorkflow) return;
    if (!presetName.trim()) {
      setPresetError("请输入预设名称后再保存。");
      return;
    }
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      const preset = await createPreset({
        projectId,
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        name: presetName,
        values,
      });
      setPresets((current) => [preset, ...current.filter((item) => item.id !== preset.id)]);
      applyPreset(preset);
    } catch (saveError: unknown) {
      setPresetError(toUserMessage(saveError));
    } finally {
      setPresetLoading(false);
    }
  }

  async function savePresetChanges() {
    if (!selectedPresetId) return savePreset();
    if (!presetName.trim()) {
      setPresetError("请输入预设名称后再保存。");
      return;
    }
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      const preset = await updatePreset({ projectId, presetId: selectedPresetId, name: presetName, values });
      setPresets((current) => current.map((item) => (item.id === preset.id ? preset : item)));
      applyPreset(preset);
    } catch (updateError: unknown) {
      setPresetError(toUserMessage(updateError));
    } finally {
      setPresetLoading(false);
    }
  }

  async function removePreset() {
    if (!selectedPresetId) return;
    if (!window.confirm("确定删除这个预设吗？")) return;
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      await deletePreset(projectId, selectedPresetId);
      setPresets((current) => current.filter((preset) => preset.id !== selectedPresetId));
      setSelectedPresetId("");
      setPresetName("");
    } catch (deleteError: unknown) {
      setPresetError(toUserMessage(deleteError));
    } finally {
      setPresetLoading(false);
    }
  }

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

  async function generate() {
    if (!selectedWorkflow) return;
    if (!productionPolicy.canSubmitGeneration) {
      setNotice("当前有生产队列正在运行，请等待完成或暂停后再开始生成。");
      return;
    }
    const nextErrors = validateRecipeValues(selectedWorkflow, values);
    setValidationErrors(nextErrors);
    if (
      Object.keys(nextErrors).length ||
      !comfyConnected ||
      !taskEventsReady ||
      hasUnsupportedField ||
      missingAssetFields.size > 0
    )
      return;

    setCreating(true);
    setNotice(null);
    try {
      const task = await createGeneration({
        projectId,
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        values,
      });
      adoptCreatedTask(task);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setCreating(false);
    }
  }

  function addCurrentToBatch() {
    if (!selectedWorkflow) return;
    const nextErrors = validateRecipeValues(selectedWorkflow, values);
    setValidationErrors(nextErrors);
    if (Object.keys(nextErrors).length || hasUnsupportedField || missingAssetFields.size > 0) {
      setBatchNotice("当前输入还未准备好，暂时无法添加到批量任务。");
      return;
    }
    if (batchItems.length >= 100) {
      setBatchNotice("已达到批量任务上限，最多支持 100 项。");
      return;
    }

    setBatchItems((current) => [
      ...current,
      {
        id: crypto.randomUUID(),
        workflowName: workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name),
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        values: cloneGenerationValues(values),
      },
    ]);
    setBatchNotice(undefined);
  }

  function removeBatchItem(id: string) {
    setBatchItems((current) => current.filter((item) => item.id !== id));
    setBatchNotice(undefined);
  }

  async function importBatchTaskList(file?: File) {
    if (!file) return;
    try {
      const imported = parseBatchTaskList(await file.text(), catalog);
      if (batchItems.length + imported.length > 100) {
        setBatchNotice("导入后将超过 100 项批量任务上限。");
        return;
      }
      setBatchItems((current) => [
        ...current,
        ...imported.map((item) => ({ ...item, id: crypto.randomUUID() })),
      ]);
      setBatchNotice(`已从 JSON 导入 ${imported.length} 个任务。`);
    } catch (importError: unknown) {
      setBatchNotice(toUserMessage(importError));
    }
  }

  async function submitBatch() {
    if (!batchItems.length) return;
    if (!productionPolicy.canSubmitLocalBatch) {
      setBatchNotice("当前有生产队列正在运行，请等待完成或暂停后再提交批量任务。");
      return;
    }
    if (!comfyConnected || !taskEventsReady) {
      setBatchNotice("请先连接 ComfyUI 并恢复任务事件通道，再提交批量任务。");
      return;
    }

    setBatchSubmitting(true);
    setBatchNotice(undefined);
    try {
      const result = await createGenerationBatch({
        projectId,
        items: batchItems.map((item) => ({
          workflowVersionId: item.workflowVersionId,
          recipeId: item.recipeId,
          values: item.values,
        })),
      });
      result.created.forEach(({ task }) => adoptCreatedTask(task));
      const failedIndexes = result.failed.map((item) => item.index);
      setBatchItems((current) => retainFailedBatchItems(current, failedIndexes));
      const failureSummary = result.failed.length
        ? ` 失败项：${result.failed.map((item) => `第 ${item.index + 1} 项（${item.code}）`).join("、")}。`
        : "";
      setBatchNotice(
        `批量提交完成：成功创建 ${result.created.length} 个任务，${result.failed.length} 个任务提交失败。${failureSummary}`,
      );
    } catch (batchError: unknown) {
      setBatchNotice(toUserMessage(batchError));
    } finally {
      setBatchSubmitting(false);
    }
  }

  async function cancelCurrentTask() {
    if (!currentTask) return;
    setCancelling(true);
    setNotice(null);
    try {
      const task = await cancelTask(projectId, currentTask.id);
      useTaskStore.getState().upsertTask(task);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setCancelling(false);
    }
  }

  if (!catalog.length) {
    return (
      <section className="studio-empty">
        <span className="section-label">创作工作台</span>
        <h2>暂无可用工作流</h2>
        <p>请先安装或发布经过校验的工作流，然后刷新列表。</p>
        <code>&lt;AIStudioData&gt;/workflow_library/</code>
        <button type="button" onClick={() => void refreshWorkflows()} disabled={refreshing}>
          {refreshing ? "正在刷新..." : "刷新工作流"}
        </button>
        {notice && <p className="error-message">{notice}</p>}
      </section>
    );
  }

  return (
    <>
      <section className="studio-panel">
        <div className="workflow-selector">
          <label>
            <span>工作流</span>
            <select
              value={selectedWorkflow ? `${selectedWorkflow.workflowVersionId}:${selectedWorkflow.recipeId}` : ""}
              onChange={(event) => {
                const [workflowVersionId, recipeId] = event.target.value.split(":");
                const next = catalog.find(
                  (recipe) =>
                    recipe.workflowVersionId === workflowVersionId && recipe.recipeId === recipeId,
                );
                setSelectedWorkflow(next);
                setMissingAssetFields(new Set());
              }}
            >
              {catalog.map((recipe) => (
                <option key={`${recipe.workflowVersionId}:${recipe.recipeId}`} value={`${recipe.workflowVersionId}:${recipe.recipeId}`}>
                  {workflowDisplayName(recipe.workflowId, recipe.name)}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className="quiet-button" onClick={() => void refreshWorkflows()} disabled={refreshing}>
            {refreshing ? "正在刷新..." : "刷新"}
          </button>
        </div>
        {selectedWorkflow && (
          <>
            <div className="preset-toolbar" aria-label="预设管理">
              <label>
                <span>预设</span>
                <select
                  aria-label="已保存预设"
                  value={selectedPresetId}
                  onChange={(event) => {
                    const preset = presets.find((item) => item.id === event.target.value);
                    if (preset) applyPreset(preset);
                    else {
                      setSelectedPresetId("");
                      setPresetName("");
                    }
                  }}
                  disabled={presetLoading}
                >
                  <option value="">选择已保存的预设</option>
                  {presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}
                </select>
              </label>
              <label>
                <span>预设名称</span>
                <input
                  aria-label="预设名称"
                  value={presetName}
                  maxLength={80}
                  onChange={(event) => setPresetName(event.target.value)}
                  placeholder="例如：柔光人像"
                />
              </label>
              <div className="preset-actions">
                <button type="button" onClick={() => void savePreset()} disabled={presetLoading}>{selectedPresetId ? "另存为新预设" : "保存预设"}</button>
                <button type="button" onClick={() => void savePresetChanges()} disabled={presetLoading || !selectedPresetId}>更新预设</button>
                <button type="button" className="quiet-button" onClick={() => void removePreset()} disabled={presetLoading || !selectedPresetId}>删除预设</button>
              </div>
            </div>
            {presetError && <p className="error-message">预设：{presetError}</p>}
            {selectedWorkflow.workflowId === "wfl_minimax_h3_reference_video" && (
              <p className="disabled-note">当前 16GB 显存配置已验证：0.1MP、最长 5 秒、4 步采样、单任务执行。</p>
            )}
            <DynamicFormRenderer
              recipe={selectedWorkflow}
              values={values}
              validationErrors={validationErrors}
              onChange={(key, value) => (value ? setValue(key, value) : removeValue(key))}
              onGenerate={() => void generate()}
              projectId={projectId}
              onImageAssetAvailabilityChange={handleAssetAvailabilityChange}
            />
            {!comfyConnected && <p className="disabled-note">请先连接 ComfyUI，再开始生成。</p>}
            {!taskEventsReady && (
              <p className="disabled-note">
                {taskEventError ?? "正在准备任务事件通道..."}
              </p>
            )}
            {productionAdmission.busy && (
              <p className="disabled-note">生产队列正在运行，开始生成和提交批量任务暂时不可用。</p>
            )}
            {hasUnsupportedField && <p className="disabled-note">当前工作流包含暂不支持的输入类型。</p>}
            {missingAssetFields.size > 0 && (
              <p className="disabled-note">缺少媒体素材，请重新选择后再生成。</p>
            )}
              <section className="batch-panel" aria-label="临时批量任务">
              <div className="batch-panel-header">
                <div>
                  <span className="section-label">临时批量任务</span>
                  <p>将当前 Kera2 或 MiniMax H3 输入保存为独立任务。</p>
                </div>
                <div className="batch-actions">
                  <button type="button" className="quiet-button" onClick={addCurrentToBatch} disabled={batchSubmitting}>
                    添加当前任务
                  </button>
                  <label className="quiet-button batch-file-button">
                    导入 JSON
                    <input
                      type="file"
                      accept="application/json,.json"
                      disabled={batchSubmitting}
                      onChange={(event) => {
                        const file = event.currentTarget.files?.[0];
                        event.currentTarget.value = "";
                        void importBatchTaskList(file);
                      }}
                    />
                  </label>
                  <button
                    type="button"
                    className="quiet-button"
                    onClick={() => {
                      setBatchItems([]);
                      setBatchNotice(undefined);
                    }}
                    disabled={batchSubmitting || !batchItems.length}
                  >
                    清空
                  </button>
                </div>
              </div>
              {batchItems.length ? (
                <ol className="batch-list">
                  {batchItems.map((item, index) => (
                    <li key={item.id}>
                      <span>#{index + 1}</span>
                      <strong>{item.workflowName}</strong>
                      <button
                        type="button"
                        className="quiet-button"
                        onClick={() => removeBatchItem(item.id)}
                        disabled={batchSubmitting}
                      >
                        移除
                      </button>
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="disabled-note">暂未添加任务。</p>
              )}
              <button
                type="button"
                onClick={() => void submitBatch()}
                disabled={
                  !batchItems.length ||
                  batchSubmitting ||
                  !comfyConnected ||
                  !taskEventsReady ||
                  !productionPolicy.canSubmitLocalBatch
                }
              >
                {batchSubmitting ? "正在提交..." : `提交批量任务（${batchItems.length}）`}
              </button>
              {batchNotice && <p className="disabled-note">{batchNotice}</p>}
            </section>
            <ProductionQueuePanel
              projectId={projectId}
              batchItems={batchItems}
              comfyConnected={comfyConnected}
              focusBatchId={focusProductionBatchId}
              onAdmissionChanged={onProductionAdmissionChanged}
              onFocusedBatchOpened={onProductionBatchFocused}
              onOpenTask={onOpenTask}
            />
            <button type="button" className="generate-button" onClick={() => void generate()} disabled={!canGenerate || creating}>
              {creating ? "正在创建任务..." : "开始生成"}
            </button>
          </>
        )}
        {notice && <p className="error-message">{notice}</p>}
      </section>
      <TaskProgressCard
        task={currentTask}
        cancelling={cancelling}
        onCancel={() => void cancelCurrentTask()}
      />
      <ImageOutput projectId={projectId} task={currentTask} />
    </>
  );
}
