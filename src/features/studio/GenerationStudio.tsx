import { useEffect, useMemo, useState } from "react";
import {
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
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  taskEventsReady: boolean;
  taskEventError?: string;
  onCatalogChanged: () => Promise<void>;
}

export function GenerationStudio({
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
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    const next = selectedWorkflow && catalog.some(
      (recipe) => recipe.workflowVersionId === selectedWorkflow.workflowVersionId,
    )
      ? selectedWorkflow
      : catalog[0];
    if (next?.workflowVersionId !== selectedWorkflow?.workflowVersionId) {
      setSelectedWorkflow(next);
    }
  }, [catalog, selectedWorkflow, setSelectedWorkflow]);

  const hasUnsupportedField = useMemo(
    () => selectedWorkflow?.fields.some((field) => !["textarea", "integer", "seed"].includes(field.type)) ?? false,
    [selectedWorkflow],
  );
  const errors = selectedWorkflow ? validateRecipeValues(selectedWorkflow, values) : {};
  const canGenerate = Boolean(
    comfyConnected &&
      taskEventsReady &&
      selectedWorkflow &&
      !hasUnsupportedField &&
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
      hasUnsupportedField
    )
      return;

    setCreating(true);
    setNotice(null);
    try {
      const task = await createGeneration({
        projectId: "prj_default",
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
              value={selectedWorkflow?.workflowVersionId ?? ""}
              onChange={(event) => {
                const next = catalog.find((recipe) => recipe.workflowVersionId === event.target.value);
                setSelectedWorkflow(next);
              }}
            >
              {catalog.map((recipe) => (
                <option key={recipe.workflowVersionId} value={recipe.workflowVersionId}>
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
            />
            {!comfyConnected && <p className="disabled-note">Connect ComfyUI before generating.</p>}
            {!taskEventsReady && (
              <p className="disabled-note">
                {taskEventError ?? "Preparing task event channel..."}
              </p>
            )}
            {hasUnsupportedField && <p className="disabled-note">This Workflow has an unsupported field type.</p>}
            <button type="button" className="generate-button" onClick={() => void generate()} disabled={!canGenerate || creating}>
              {creating ? "Creating Task..." : "Generate"}
            </button>
          </>
        )}
        {notice && <p className="error-message">{notice}</p>}
      </section>
      <TaskProgressCard task={currentTask} />
      <ImageOutput task={currentTask} />
    </>
  );
}
