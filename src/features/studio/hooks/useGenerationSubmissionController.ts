import { useCallback, useRef, useState } from "react";
import { cancelTask, createGeneration } from "../../../services/tauriClient";
import { toUserMessage } from "../../../i18n/errorMessages";
import { useTaskStore } from "../../../stores/taskStore";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import type { ProductionAdmissionStatus } from "../../../types/productionQueue";
import { validateRecipeValues } from "../DynamicFormRenderer";
import { generationBlockedReason } from "../generationBlockedReason";

interface UseGenerationSubmissionControllerOptions {
  projectId: string;
  selectedWorkflow?: RecipeViewModel;
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
  const adoptCreatedTask = useTaskStore((state) => state.adoptCreatedTask);
  const [creating, setCreating] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const generationRequestIdRef = useRef<string | undefined>(undefined);

  const generate = useCallback(async () => {
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
    setCreating(true);
    onNotice(null);
    try {
      const task = await createGeneration({
        projectId,
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        values,
        submissionIdempotencyKey,
      });
      adoptCreatedTask(task);
    } catch (error: unknown) {
      onNotice(toUserMessage(error));
    } finally {
      setCreating(false);
      if (generationRequestIdRef.current === submissionIdempotencyKey) {
        generationRequestIdRef.current = undefined;
      }
    }
  }, [adoptCreatedTask, comfyConnected, configurationError, missingAsset, onNotice, onValidationErrors, productionAdmission.busy, projectId, selectedWorkflow, taskEventError, taskEventsReady, unsupportedField, values]);

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
