import { useCallback, useState } from "react";
import {
  checkOnboardingCapability,
  commitWorkflowImport,
  discardOnboarding,
  duplicateWorkflowRecipe,
  getOnboardingDraft,
  removeOnboardingInputMapping,
  setOnboardingInputMapping,
  validateOnboarding,
} from "../../../services/workflowClient";
import type {
  WorkflowInputView,
  WorkflowOnboardingDraftView,
  WorkflowProductionWorkspaceView,
} from "../../../types/workflowOnboarding";
import { toUserMessage } from "../../../i18n/errorMessages";
import {
  defaultMapping,
  isExposableWorkflowInput,
  localizeWorkflowIssue,
  optionalNumber,
  optionalText,
  supportedParameterFieldType,
  type MappingDraft,
  type ParameterMappingEdit,
} from "../workflowParameterExposureModel";

export type { MappingDraft, ParameterMappingEdit } from "../workflowParameterExposureModel";

export interface UseWorkflowParameterExposureControllerOptions {
  onError: (message: string | undefined) => void;
  onNotice: (message: string | undefined) => void;
  onWorkspaceRefresh: () => Promise<void>;
  onCatalogChanged: () => Promise<void>;
  onBeforeOpen?: () => void;
}

export function useWorkflowParameterExposureController({
  onError,
  onNotice,
  onWorkspaceRefresh,
  onCatalogChanged,
  onBeforeOpen,
}: UseWorkflowParameterExposureControllerOptions) {
  const [draft, setDraft] = useState<WorkflowOnboardingDraftView>();
  const [item, setItem] = useState<WorkflowProductionWorkspaceView>();
  const [originalKeys, setOriginalKeys] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const clearSession = useCallback(() => {
    setDraft(undefined);
    setItem(undefined);
    setOriginalKeys([]);
  }, []);

  const open = useCallback(async (nextItem: WorkflowProductionWorkspaceView) => {
    if (!nextItem.workflowVersionId || nextItem.archived) return;
    onBeforeOpen?.();
    setLoading(true);
    onError(undefined);
    try {
      const sourceRecipe = nextItem.recipes[nextItem.recipes.length - 1];
      const duplicated = await duplicateWorkflowRecipe(nextItem.workflowVersionId, sourceRecipe?.recipeId);
      setItem(nextItem);
      setDraft(duplicated);
      setOriginalKeys(duplicated.inputMappings.map((mapping) => mapping.semanticKey));
      try {
        await checkOnboardingCapability(duplicated.draftId);
        setDraft(await getOnboardingDraft(duplicated.draftId));
      } catch (capabilityError: unknown) {
        onError(`参数节点已加载，但暂时无法读取 ComfyUI /object_info：${toUserMessage(capabilityError)}`);
      }
    } catch (actionError: unknown) {
      onError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }, [onBeforeOpen, onError]);

  const close = useCallback(async () => {
    const currentDraft = draft;
    if (currentDraft) {
      try {
        await discardOnboarding(currentDraft.draftId);
      } catch {
        // The draft may already have been consumed by publish; closing remains safe.
      }
    }
    clearSession();
  }, [clearSession, draft]);

  const refreshCapability = useCallback(async () => {
    if (!draft) return;
    setLoading(true);
    onError(undefined);
    try {
      await checkOnboardingCapability(draft.draftId);
      setDraft(await getOnboardingDraft(draft.draftId));
      onNotice("已刷新参数建议与 ComfyUI 输入范围。");
    } catch (actionError: unknown) {
      onError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }, [draft, onError, onNotice]);

  const exposeParameter = useCallback(async (nodeId: string, input: WorkflowInputView) => {
    if (!draft || !isExposableWorkflowInput(input)) return;
    const fieldType = supportedParameterFieldType(input);
    if (!fieldType) return;
    const mapping = defaultMapping(nodeId, input);
    setLoading(true);
    onError(undefined);
    try {
      setDraft(await setOnboardingInputMapping(draft.draftId, {
        semanticKey: mapping.semanticKey,
        fieldType,
        label: mapping.label,
        required: mapping.required,
        defaultValue: optionalText(mapping.defaultValue),
        minValue: optionalText(mapping.minValue),
        maxValue: optionalText(mapping.maxValue),
        step: optionalText(input.numericStep ?? ""),
        minItems: fieldType.endsWith("s") ? 0 : undefined,
        maxItems: fieldType.endsWith("s") ? 8 : undefined,
        targetNode: nodeId,
        targetInput: input.name,
      }));
      onNotice(`${mapping.label} 已加入新配方草稿。`);
    } catch (actionError: unknown) {
      onError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }, [draft, onError, onNotice]);

  const saveMapping = useCallback(async (mapping: MappingDraft, targetNode: string, targetInput: string) => {
    if (!draft) return;
    setLoading(true);
    onError(undefined);
    try {
      setDraft(await setOnboardingInputMapping(draft.draftId, {
        semanticKey: mapping.semanticKey,
        fieldType: mapping.fieldType,
        label: mapping.label,
        required: mapping.required,
        defaultValue: optionalText(mapping.defaultValue),
        minValue: optionalText(mapping.minValue),
        maxValue: optionalText(mapping.maxValue),
        step: optionalText(mapping.step ?? ""),
        minItems: optionalNumber(mapping.minItems),
        maxItems: optionalNumber(mapping.maxItems),
        itemIndex: optionalNumber(mapping.itemIndex),
        targetNode,
        targetInput,
      }));
      onNotice("生产参数字段已保存到新配方草稿。");
    } catch (actionError: unknown) {
      onError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }, [draft, onError, onNotice]);

  const removeMapping = useCallback(async (mapping: WorkflowOnboardingDraftView["inputMappings"][number]) => {
    if (!draft) return;
    setLoading(true);
    try {
      setDraft(await removeOnboardingInputMapping(draft.draftId, {
        semanticKey: mapping.semanticKey,
        itemIndex: mapping.itemIndex,
      }));
    } catch (actionError: unknown) {
      onError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }, [draft, onError]);

  const publish = useCallback(async (edits: ParameterMappingEdit[] = []) => {
    if (!draft) return;
    setLoading(true);
    onError(undefined);
    try {
      let currentDraft = draft;
      for (const edit of edits) {
        currentDraft = await setOnboardingInputMapping(currentDraft.draftId, {
          semanticKey: edit.draft.semanticKey,
          fieldType: edit.draft.fieldType,
          label: edit.draft.label,
          required: edit.draft.required,
          defaultValue: optionalText(edit.draft.defaultValue),
          minValue: optionalText(edit.draft.minValue),
          maxValue: optionalText(edit.draft.maxValue),
          step: optionalText(edit.draft.step),
          minItems: optionalNumber(edit.draft.minItems),
          maxItems: optionalNumber(edit.draft.maxItems),
          itemIndex: optionalNumber(edit.draft.itemIndex),
          targetNode: edit.mapping.targetNode,
          targetInput: edit.mapping.targetInput,
        });
      }
      setDraft(currentDraft);
      const validation = await validateOnboarding(currentDraft.draftId);
      setDraft((current) => current ? { ...current, validation } : current);
      if (!validation.readyToPublish) {
        onError(validation.issues.map(localizeWorkflowIssue).join("；"));
        return;
      }
      const result = await commitWorkflowImport({
        draftId: currentDraft.draftId,
        action: "NEW_RECIPE",
        workflowId: currentDraft.manifest.workflowId,
        setCurrent: false,
      });
      try {
        await discardOnboarding(currentDraft.draftId);
      } catch {
        // Publishing has already committed the immutable package.
      }
      clearSession();
      await onWorkspaceRefresh();
      await onCatalogChanged();
      onNotice(`已保存为配方 ${result.recipeId}；工作流版本保持不变。`);
    } catch (actionError: unknown) {
      onError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }, [clearSession, draft, onCatalogChanged, onError, onNotice, onWorkspaceRefresh]);

  return {
    draft,
    item,
    originalKeys,
    loading,
    open,
    close,
    refreshCapability,
    exposeParameter,
    saveMapping,
    removeMapping,
    publish,
  };
}
