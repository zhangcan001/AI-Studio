import { useCallback, useRef, useState } from "react";
import { cancelTask, startProductionQueue, submitGeneration } from "../../../services/tauriClient";
import { toUserMessage } from "../../../i18n/errorMessages";
import { useTaskStore } from "../../../stores/taskStore";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import type { ProductionAdmissionStatus } from "../../../types/productionQueue";
import { validateRecipeValues } from "../DynamicFormRenderer";
import { generationBlockedReason } from "../generationBlockedReason";

interface UseGenerationSubmissionControllerOptions {
  projectId: string;
  selectedWorkflow?: RecipeViewModel;
  modelVersionId?: string;
  promptVersionId?: string;
  values: GenerationValues;
  configurationError?: string;
  productionAdmission: ProductionAdmissionStatus;
  comfyConnected: boolean;
  taskEventsReady: boolean;
  taskEventError?: string;
  missingAsset: boolean;
  unsupportedField: boolean;
  onValidationErrors: (errors: Record<string, string>) => void;
  onNotice: (message: string | null) => void;
}

export function useGenerationSubmissionController({
  projectId,
  selectedWorkflow,
  modelVersionId,
  promptVersionId,
  values,
  configurationError,
  productionAdmission,
  comfyConnected,
  taskEventsReady,
  taskEventError,
  missingAsset,
  unsupportedField,
  onValidationErrors,
  onNotice,
}: UseGenerationSubmissionControllerOptions) {
  const currentTask = useTaskStore((state) => state.currentTask);
  const [creating, setCreating] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const generationRequestIdRef = useRef<string | undefined>(undefined);
  const pendingBatchIdRef = useRef<string | undefined>(undefined);
  const creatingRef = useRef(false);

  const generate = useCallback(async () => {
    if (creatingRef.current) return;
    if (!selectedWorkflow) return;
    if (configurationError) {
      onNotice(configurationError);
      return;
    }

    const nextErrors = validateRecipeValues(selectedWorkflow, values);
    onValidationErrors(nextErrors);
    const reason = generationBlockedReason({
      productionBusy: productionAdmission.busy,
      comfyConnected,
      taskEventsReady,
      taskEventError,
      missingAsset,
      validationError: Object.keys(nextErrors).length > 0,
      unsupportedField,
    });
    if (reason) {
      onNotice(reason);
      return;
    }

    const submissionIdempotencyKey = generationRequestIdRef.current ??= crypto.randomUUID();
    creatingRef.current = true;
    setCreating(true);
    onNotice(null);
    try {
      let batchId = pendingBatchIdRef.current;
      if (!batchId) {
        const batch = await submitGeneration({
          projectId,
          workflowVersionId: selectedWorkflow.workflowVersionId,
          recipeId: selectedWorkflow.recipeId,
          values,
          ...(modelVersionId ? { modelVersionId } : {}),
          ...(promptVersionId ? { promptVersionId } : {}),
          submissionIdempotencyKey,
        });
        batchId = batch.id;
        pendingBatchIdRef.current = batchId;
      }
      await startProductionQueue(projectId, batchId);
      pendingBatchIdRef.current = undefined;
      generationRequestIdRef.current = undefined;
      onNotice("已加入生产队列并开始处理。");
    } catch (error: unknown) {
      onNotice(toUserMessage(error));
    } finally {
      creatingRef.current = false;
      setCreating(false);
    }
  }, [comfyConnected, configurationError, missingAsset, modelVersionId, onNotice, onValidationErrors, productionAdmission.busy, projectId, promptVersionId, selectedWorkflow, taskEventError, taskEventsReady, unsupportedField, values]);

  const cancelCurrentTask = useCallback(async () => {
    if (!currentTask) return;
    setCancelling(true);
    onNotice(null);
    try {
      const task = await cancelTask(projectId, currentTask.id);
      useTaskStore.getState().upsertTask(task);
    } catch (error: unknown) {
      onNotice(toUserMessage(error));
    } finally {
      setCancelling(false);
    }
  }, [currentTask, onNotice, projectId]);

  return { creating, cancelling, generate, cancelCurrentTask };
}
