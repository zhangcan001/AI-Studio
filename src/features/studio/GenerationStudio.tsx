import { useCallback, useEffect, useMemo, useState } from "react";
import {
  cancelTask,
  createGeneration,
  refreshWorkflowLibrary,
} from "../../services/tauriClient";
import { useStudioStore } from "../../stores/studioStore";
import { useTaskStore } from "../../stores/taskStore";
import type { RecipeViewModel } from "../../types/generation";
import { DynamicFormRenderer, validateRecipeValues } from "./DynamicFormRenderer";
import { ImageOutput } from "./ImageOutput";
import { TaskProgressCard } from "./TaskProgressCard";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  taskEventsReady: boolean;
  taskEventError?: string;
  onCatalogChanged: () => Promise<void>;
}

export function GenerationStudio({
  projectId,
  catalog,
  comfyConnected,
  taskEventsReady,
  taskEventError,
  onCatalogChanged,
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
  const [missingImageFields, setMissingImageFields] = useState<Set<string>>(new Set());
  const handleImageAvailabilityChange = useCallback((key: string, available: boolean) => {
    setMissingImageFields((current) => {
      const next = new Set(current);
      if (available) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  useEffect(() => {
    setMissingImageFields(new Set());
    setNotice(null);
  }, [projectId]);

  useEffect(() => {
    const next = selectedWorkflow && catalog.some(
      (recipe) =>
        recipe.workflowVersionId === selectedWorkflow.workflowVersionId &&
        recipe.recipeId === selectedWorkflow.recipeId,
    )
      ? selectedWorkflow
      : catalog[0];
    if (
      next?.workflowVersionId !== selectedWorkflow?.workflowVersionId ||
      next?.recipeId !== selectedWorkflow?.recipeId
    ) {
      setSelectedWorkflow(next);
      setMissingImageFields(new Set());
    }
  }, [catalog, selectedWorkflow, setSelectedWorkflow]);

  const hasUnsupportedField = useMemo(
    () => selectedWorkflow?.fields.some((field) => !["textarea", "integer", "seed", "image"].includes(field.type)) ?? false,
    [selectedWorkflow],
  );
  const errors = selectedWorkflow ? validateRecipeValues(selectedWorkflow, values) : {};
  const canGenerate = Boolean(
    comfyConnected &&
      taskEventsReady &&
      selectedWorkflow &&
      !hasUnsupportedField &&
      missingImageFields.size === 0 &&
      Object.keys(errors).length === 0,
  );

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
      missingImageFields.size > 0
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
                setMissingImageFields(new Set());
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
            <DynamicFormRenderer
              recipe={selectedWorkflow}
              values={values}
              validationErrors={validationErrors}
              onChange={(key, value) => (value ? setValue(key, value) : removeValue(key))}
              onGenerate={() => void generate()}
              projectId={projectId}
              onImageAssetAvailabilityChange={handleImageAvailabilityChange}
            />
            {!comfyConnected && <p className="disabled-note">Connect ComfyUI before generating.</p>}
            {!taskEventsReady && (
              <p className="disabled-note">
                {taskEventError ?? "Preparing task event channel..."}
              </p>
            )}
            {hasUnsupportedField && <p className="disabled-note">This Workflow has an unsupported field type.</p>}
            {missingImageFields.size > 0 && (
              <p className="disabled-note">Missing image asset. Choose a replacement before generating.</p>
            )}
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
