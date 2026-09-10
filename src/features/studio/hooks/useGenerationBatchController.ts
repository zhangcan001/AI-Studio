import { useCallback, useEffect, useState } from "react";
import { createProductionQueue, startProductionQueue } from "../../../services/tauriClient";
import { toUserMessage } from "../../../i18n/errorMessages";
import { formatDateTime, workflowDisplayName } from "../../../i18n/statusLabels";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import { splitPromptBlocks } from "../../assets/assetVideoBatch";
import { imageRecipeCapability } from "../../runtime/workflowCapabilities";
import { validateRecipeValues } from "../DynamicFormRenderer";
import {
  cloneGenerationValues,
  copyBatchDraftItem,
  moveBatchDraftItem,
  removeBatchDraftItem,
  type BatchDraftItem,
} from "../batchDraft";
import { parseBatchTaskList } from "../batchImport";

export interface UseGenerationBatchControllerOptions {
  projectId: string;
  productCatalog: RecipeViewModel[];
  selectedWorkflow?: RecipeViewModel;
  values: GenerationValues;
  configurationError?: string;
  genericImageNotice?: string;
  hasUnsupportedField: boolean;
  missingAsset: boolean;
  canSubmitLocalBatch: boolean;
  comfyConnected: boolean;
  taskEventsReady: boolean;
  onValidationErrors: (errors: Record<string, string>) => void;
  onBatchCreated: (batchId: string) => void;
  onProductionAdmissionChanged: () => Promise<void>;
}

export function useGenerationBatchController({
  projectId,
  productCatalog,
  selectedWorkflow,
  values,
  configurationError,
  genericImageNotice,
  hasUnsupportedField,
  missingAsset,
  canSubmitLocalBatch,
  comfyConnected,
  taskEventsReady,
  onValidationErrors,
  onBatchCreated,
  onProductionAdmissionChanged,
}: UseGenerationBatchControllerOptions) {
  const [batchItems, setBatchItems] = useState<BatchDraftItem[]>([]);
  const [batchSubmitting, setBatchSubmitting] = useState(false);
  const [batchNotice, setBatchNotice] = useState<string>();
  const [batchPasteText, setBatchPasteText] = useState("");

  useEffect(() => {
    setBatchItems([]);
    setBatchSubmitting(false);
    setBatchNotice(undefined);
    setBatchPasteText("");
  }, [projectId]);

  const promptFieldForRecipe = useCallback((recipe?: RecipeViewModel) => (
    recipe ? imageRecipeCapability(recipe).promptField : undefined
  ), []);

  const setBatchPrompt = useCallback((valuesToUpdate: GenerationValues, promptText: string, recipe = selectedWorkflow) => {
    const promptField = promptFieldForRecipe(recipe);
    if (!promptField) return valuesToUpdate;
    return {
      ...valuesToUpdate,
      [promptField.key]: { type: "string" as const, value: promptText },
    };
  }, [promptFieldForRecipe, selectedWorkflow]);

  const batchPrompt = useCallback((item: BatchDraftItem) => {
    const recipe = productCatalog.find((candidate) => (
      candidate.workflowVersionId === item.workflowVersionId && candidate.recipeId === item.recipeId
    ));
    const promptField = promptFieldForRecipe(recipe);
    if (!promptField) return "";
    const value = item.values[promptField.key];
    return value?.type === "string" ? value.value : "";
  }, [productCatalog, promptFieldForRecipe]);

  const addCurrentToBatch = useCallback(() => {
    const capability = selectedWorkflow ? imageRecipeCapability(selectedWorkflow) : undefined;
    if (!selectedWorkflow || !capability?.batchPromptCompatible || !capability.promptField) {
      setBatchNotice(configurationError ?? genericImageNotice ?? "当前工作流没有可识别的标准提示词输入，暂时无法添加到图片批次。");
      return;
    }
    const nextErrors = validateRecipeValues(selectedWorkflow, values);
    onValidationErrors(nextErrors);
    if (Object.keys(nextErrors).length || hasUnsupportedField || missingAsset) {
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
  }, [batchItems.length, configurationError, genericImageNotice, hasUnsupportedField, missingAsset, onValidationErrors, selectedWorkflow, values]);

  const addBlankPromptCard = useCallback(() => {
    const capability = selectedWorkflow ? imageRecipeCapability(selectedWorkflow) : undefined;
    if (!selectedWorkflow || !capability?.batchPromptCompatible) {
      setBatchNotice(genericImageNotice ?? "当前工作流没有可识别的标准提示词输入，不能创建提示词列表批次。");
      return;
    }
    if (batchItems.length >= 100) {
      setBatchNotice("已达到图片批次上限，最多支持 100 项。");
      return;
    }
    setBatchItems((current) => [...current, {
      id: crypto.randomUUID(),
      workflowName: workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name),
      workflowVersionId: selectedWorkflow.workflowVersionId,
      recipeId: selectedWorkflow.recipeId,
      values: setBatchPrompt(cloneGenerationValues(values), "", selectedWorkflow),
    }]);
    setBatchNotice("已添加空白提示词卡片，请填写后再创建图片批次。");
  }, [batchItems.length, genericImageNotice, selectedWorkflow, setBatchPrompt, values]);

  const updateBatchPrompt = useCallback((id: string, promptText: string) => {
    setBatchItems((current) => current.map((item) => {
      if (item.id !== id) return item;
      const recipe = productCatalog.find((candidate) => (
        candidate.workflowVersionId === item.workflowVersionId && candidate.recipeId === item.recipeId
      ));
      return { ...item, values: setBatchPrompt(cloneGenerationValues(item.values), promptText, recipe) };
    }));
  }, [productCatalog, setBatchPrompt]);

  const copyBatchItem = useCallback((id: string) => {
    if (batchItems.length >= 100) {
      setBatchNotice("已达到图片批次上限，最多支持 100 项。");
      return;
    }
    setBatchItems((current) => copyBatchDraftItem(current, id, crypto.randomUUID()));
    setBatchNotice(undefined);
  }, [batchItems.length]);

  const moveBatchItem = useCallback((id: string, direction: -1 | 1) => {
    setBatchItems((current) => moveBatchDraftItem(current, id, direction));
  }, []);

  const splitPastedPrompts = useCallback(() => {
    const capability = selectedWorkflow ? imageRecipeCapability(selectedWorkflow) : undefined;
    if (!selectedWorkflow || !capability?.batchPromptCompatible) {
      setBatchNotice(genericImageNotice ?? "当前工作流没有可识别的标准提示词输入，不能拆分提示词列表。");
      return;
    }
    const parsed = splitPromptBlocks(batchPasteText);
    if (!parsed.length) {
      setBatchNotice("请先粘贴提示词；多个提示词之间使用空行分隔。");
      return;
    }
    if (batchItems.length + parsed.length > 100) {
      setBatchNotice(`拆分后将超过 100 项上限，还可添加 ${100 - batchItems.length} 项。`);
      return;
    }
    setBatchItems((current) => [
      ...current,
      ...parsed.map((promptText) => ({
        id: crypto.randomUUID(),
        workflowName: workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name),
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        values: setBatchPrompt(cloneGenerationValues(values), promptText, selectedWorkflow),
      })),
    ]);
    setBatchPasteText("");
    setBatchNotice(`已按空行拆分 ${parsed.length} 张提示词卡片。`);
  }, [batchItems.length, batchPasteText, genericImageNotice, selectedWorkflow, setBatchPrompt, values]);

  const removeBatchItem = useCallback((id: string) => {
    setBatchItems((current) => removeBatchDraftItem(current, id));
    setBatchNotice(undefined);
  }, []);

  const importBatchTaskList = useCallback(async (file?: File) => {
    if (!file) return;
    try {
      const imported = parseBatchTaskList(await file.text(), productCatalog);
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
  }, [batchItems.length, productCatalog]);

  const clearBatch = useCallback(() => {
    setBatchItems([]);
    setBatchNotice(undefined);
  }, []);

  const submitBatch = useCallback(async () => {
    if (!batchItems.length) return;
    if (!selectedWorkflow) {
      setBatchNotice("当前没有可用的图片工作流。");
      return;
    }
    if (!canSubmitLocalBatch) {
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
      const invalidIndexes = batchItems.flatMap((item, index) => {
        const recipe = productCatalog.find(
          (candidate) => candidate.workflowVersionId === item.workflowVersionId && candidate.recipeId === item.recipeId,
        );
        if (!recipe || !imageRecipeCapability(recipe).batchPromptCompatible || Object.keys(validateRecipeValues(recipe, item.values)).length > 0) return [index];
        return [];
      });
      if (invalidIndexes.length) {
        setBatchNotice(`第 ${invalidIndexes.map((index) => index + 1).join("、")} 项还未填写完整，请补齐提示词和必需输入。`);
        return;
      }
      const created = await createProductionQueue({
        projectId,
        name: `批量图片 · ${formatDateTime(new Date().toISOString())}`,
        continueOnFailure: true,
        items: batchItems.map((item) => ({
          workflowVersionId: item.workflowVersionId,
          recipeId: item.recipeId,
          values: cloneGenerationValues(item.values),
        })),
      });
      onBatchCreated(created.id);
      setBatchItems([]);
      try {
        await onProductionAdmissionChanged();
      } catch {
        // The queue is persisted even if the status refresh is temporarily unavailable.
      }
      try {
        await startProductionQueue(projectId, created.id);
        setBatchNotice(`图片批次已创建并开始执行，共 ${created.total} 项；提示词和参数已冻结，队列严格串行。`);
      } catch (startError: unknown) {
        setBatchNotice(`图片批次已创建，共 ${created.total} 项；开始执行失败：${toUserMessage(startError)}。可在队列中手动开始。`);
      }
    } catch (batchError: unknown) {
      setBatchNotice(toUserMessage(batchError));
    } finally {
      setBatchSubmitting(false);
    }
  }, [batchItems, canSubmitLocalBatch, comfyConnected, onBatchCreated, onProductionAdmissionChanged, productCatalog, projectId, selectedWorkflow, taskEventsReady]);

  return {
    batchItems,
    batchSubmitting,
    batchNotice,
    batchPasteText,
    setBatchPasteText,
    addCurrentToBatch,
    addBlankPromptCard,
    updateBatchPrompt,
    copyBatchItem,
    moveBatchItem,
    splitPastedPrompts,
    removeBatchItem,
    importBatchTaskList,
    clearBatch,
    submitBatch,
    batchPrompt,
  };
}
