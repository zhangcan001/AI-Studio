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
import { DynamicFormRenderer, validateRecipeValues } from "./DynamicFormRenderer";
import { cloneGenerationValues, retainFailedBatchItems, type BatchDraftItem } from "./batchDraft";
import { parseBatchTaskList } from "./batchImport";
import { ImageOutput } from "./ImageOutput";
import { ProductionQueuePanel } from "./ProductionQueuePanel";
import { TaskProgressCard } from "./TaskProgressCard";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  taskEventsReady: boolean;
  taskEventError?: string;
  onCatalogChanged: () => Promise<void>;
  onOpenTask: (taskId: string) => void;
}

export function GenerationStudio({
  projectId,
  catalog,
  comfyConnected,
  taskEventsReady,
  taskEventError,
  onCatalogChanged,
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
  const errors = selectedWorkflow ? validateRecipeValues(selectedWorkflow, values) : {};
  const canGenerate = Boolean(
    comfyConnected &&
      taskEventsReady &&
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
        if (active) setPresetError(loadError instanceof Error ? loadError.message : String(loadError));
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
      setPresetError("PRESET_NAME_REQUIRED: enter a name before saving.");
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
      setPresetError(saveError instanceof Error ? saveError.message : String(saveError));
    } finally {
      setPresetLoading(false);
    }
  }

  async function savePresetChanges() {
    if (!selectedPresetId) return savePreset();
    if (!presetName.trim()) {
      setPresetError("PRESET_NAME_REQUIRED: enter a name before saving.");
      return;
    }
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      const preset = await updatePreset({ projectId, presetId: selectedPresetId, name: presetName, values });
      setPresets((current) => current.map((item) => (item.id === preset.id ? preset : item)));
      applyPreset(preset);
    } catch (updateError: unknown) {
      setPresetError(updateError instanceof Error ? updateError.message : String(updateError));
    } finally {
      setPresetLoading(false);
    }
  }

  async function removePreset() {
    if (!selectedPresetId) return;
    if (!window.confirm("Delete this preset?")) return;
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      await deletePreset(projectId, selectedPresetId);
      setPresets((current) => current.filter((preset) => preset.id !== selectedPresetId));
      setSelectedPresetId("");
      setPresetName("");
    } catch (deleteError: unknown) {
      setPresetError(deleteError instanceof Error ? deleteError.message : String(deleteError));
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
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setRefreshing(false);
    }
  }

  async function generate() {
    if (!selectedWorkflow) return;
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
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setCreating(false);
    }
  }

  function addCurrentToBatch() {
    if (!selectedWorkflow) return;
    const nextErrors = validateRecipeValues(selectedWorkflow, values);
    setValidationErrors(nextErrors);
    if (Object.keys(nextErrors).length || hasUnsupportedField || missingAssetFields.size > 0) {
      setBatchNotice("Current inputs are not ready to add to the batch.");
      return;
    }
    if (batchItems.length >= 100) {
      setBatchNotice("Batch limit reached: maximum 100 items.");
      return;
    }

    setBatchItems((current) => [
      ...current,
      {
        id: crypto.randomUUID(),
        workflowName: selectedWorkflow.name,
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
        setBatchNotice("Batch import would exceed the 100 item limit.");
        return;
      }
      setBatchItems((current) => [
        ...current,
        ...imported.map((item) => ({ ...item, id: crypto.randomUUID() })),
      ]);
      setBatchNotice(`Imported ${imported.length} task${imported.length === 1 ? "" : "s"} from JSON.`);
    } catch (importError: unknown) {
      setBatchNotice(importError instanceof Error ? importError.message : String(importError));
    }
  }

  async function submitBatch() {
    if (!batchItems.length) return;
    if (!comfyConnected || !taskEventsReady) {
      setBatchNotice("Connect ComfyUI and restore the task event channel before submitting the batch.");
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
        ? ` Failed: ${result.failed.map((item) => `#${item.index + 1} ${item.code}`).join(", ")}.`
        : "";
      setBatchNotice(
        `Batch submitted: ${result.created.length} created, ${result.failed.length} failed.${failureSummary}`,
      );
    } catch (batchError: unknown) {
      setBatchNotice(batchError instanceof Error ? batchError.message : String(batchError));
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
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setCancelling(false);
    }
  }

  if (!catalog.length) {
    return (
      <section className="studio-empty">
        <span className="section-label">Generation Studio</span>
        <h2>No runnable Workflow installed</h2>
        <p>Place a validated Workflow Package in the runtime directory, then refresh.</p>
        <code>&lt;AIStudioData&gt;/workflow_library/</code>
        <button type="button" onClick={() => void refreshWorkflows()} disabled={refreshing}>
          {refreshing ? "Refreshing..." : "Refresh Workflow"}
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
            <span>Workflow</span>
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
                  {recipe.name}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className="quiet-button" onClick={() => void refreshWorkflows()} disabled={refreshing}>
            {refreshing ? "Refreshing..." : "Refresh"}
          </button>
        </div>
        {selectedWorkflow && (
          <>
            <div className="preset-toolbar" aria-label="Preset Studio">
              <label>
                <span>Preset</span>
                <select
                  aria-label="Saved presets"
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
                  <option value="">Select a saved preset</option>
                  {presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}
                </select>
              </label>
              <label>
                <span>Preset name</span>
                <input
                  aria-label="Preset name"
                  value={presetName}
                  maxLength={80}
                  onChange={(event) => setPresetName(event.target.value)}
                  placeholder="e.g. Portrait soft light"
                />
              </label>
              <div className="preset-actions">
                <button type="button" onClick={() => void savePreset()} disabled={presetLoading}>{selectedPresetId ? "Save as new" : "Save"}</button>
                <button type="button" onClick={() => void savePresetChanges()} disabled={presetLoading || !selectedPresetId}>Update</button>
                <button type="button" className="quiet-button" onClick={() => void removePreset()} disabled={presetLoading || !selectedPresetId}>Delete</button>
              </div>
            </div>
            {presetError && <p className="error-message">Preset: {presetError}</p>}
            <DynamicFormRenderer
              recipe={selectedWorkflow}
              values={values}
              validationErrors={validationErrors}
              onChange={(key, value) => (value ? setValue(key, value) : removeValue(key))}
              onGenerate={() => void generate()}
              projectId={projectId}
              onImageAssetAvailabilityChange={handleAssetAvailabilityChange}
            />
            {!comfyConnected && <p className="disabled-note">Connect ComfyUI before generating.</p>}
            {!taskEventsReady && (
              <p className="disabled-note">
                {taskEventError ?? "Preparing task event channel..."}
              </p>
            )}
            {hasUnsupportedField && <p className="disabled-note">This Workflow has an unsupported field type.</p>}
            {missingAssetFields.size > 0 && (
              <p className="disabled-note">Missing media asset. Choose a replacement before generating.</p>
            )}
            <section className="batch-panel" aria-label="Batch queue">
              <div className="batch-panel-header">
                <div>
                  <span className="section-label">Batch queue</span>
                  <p>Freeze the current Kera2 or MiniMax H3 inputs as an independent task.</p>
                </div>
                <div className="batch-actions">
                  <button type="button" className="quiet-button" onClick={addCurrentToBatch} disabled={batchSubmitting}>
                    Add current
                  </button>
                  <label className="quiet-button batch-file-button">
                    Import JSON
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
                    Clear
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
                        Remove
                      </button>
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="disabled-note">No items added yet.</p>
              )}
              <button
                type="button"
                onClick={() => void submitBatch()}
                disabled={!batchItems.length || batchSubmitting || !comfyConnected || !taskEventsReady}
              >
                {batchSubmitting ? "Submitting Batch..." : `Submit Batch (${batchItems.length})`}
              </button>
              {batchNotice && <p className="disabled-note">{batchNotice}</p>}
            </section>
            <ProductionQueuePanel
              projectId={projectId}
              batchItems={batchItems}
              comfyConnected={comfyConnected}
              onOpenTask={onOpenTask}
            />
            <button type="button" className="generate-button" onClick={() => void generate()} disabled={!canGenerate || creating}>
              {creating ? "Creating Task..." : "Generate"}
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
