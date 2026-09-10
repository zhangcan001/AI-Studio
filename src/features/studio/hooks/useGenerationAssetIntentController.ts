import { useCallback, useEffect, useRef, useState } from "react";
import { useStudioStore } from "../../../stores/studioStore";
import type { RecipeField, RecipeViewModel } from "../../../types/generation";
import { assignAssetToField, compatibleAssetFields } from "../assetIntent";

export interface UseGenerationAssetIntentControllerOptions {
  projectId: string;
  selectedWorkflow?: RecipeViewModel;
  onNotice: (message: string | null) => void;
  onAssetFieldResolved: (fieldKey: string) => void;
}

export function useGenerationAssetIntentController({
  projectId,
  selectedWorkflow,
  onNotice,
  onAssetFieldResolved,
}: UseGenerationAssetIntentControllerOptions) {
  const pendingAssetIntent = useStudioStore((state) => state.pendingAssetIntent);
  const [assetIntentTargets, setAssetIntentTargets] = useState<RecipeField[]>([]);
  const selectedWorkflowRef = useRef(selectedWorkflow);
  const onNoticeRef = useRef(onNotice);
  const onAssetFieldResolvedRef = useRef(onAssetFieldResolved);
  selectedWorkflowRef.current = selectedWorkflow;
  onNoticeRef.current = onNotice;
  onAssetFieldResolvedRef.current = onAssetFieldResolved;
  const workflowFingerprint = selectedWorkflow ? JSON.stringify(selectedWorkflow) : "";

  const clearIntent = useCallback(() => {
    useStudioStore.getState().clearPendingAssetIntent();
    setAssetIntentTargets([]);
  }, []);

  const applyToTarget = useCallback((field: RecipeField, replaceSingle = false) => {
    const intent = useStudioStore.getState().pendingAssetIntent;
    const workflow = selectedWorkflowRef.current;
    if (!workflow || !intent) return;

    const result = assignAssetToField(
      field,
      useStudioStore.getState().values,
      intent.assetId,
      replaceSingle,
    );
    if (result.kind === "requires_confirmation") {
      if (window.confirm("当前输入已有素材，是否替换当前素材？")) {
        applyToTarget(field, true);
      }
      return;
    }

    clearIntent();
    if (result.kind === "max_items") {
      onNoticeRef.current(`“${field.label}”已达到素材数量上限。`);
      return;
    }
    if (result.kind !== "applied") {
      onNoticeRef.current("当前工作流没有可使用此素材的输入项。");
      return;
    }

    useStudioStore.getState().loadDraft(workflow, result.values);
    onAssetFieldResolvedRef.current(field.key);
    onNoticeRef.current("已将素材加入创作。");
  }, [clearIntent]);

  useEffect(() => {
    if (!pendingAssetIntent) {
      setAssetIntentTargets([]);
      return;
    }
    if (pendingAssetIntent.projectId !== projectId) {
      clearIntent();
      onNoticeRef.current("素材属于其他项目，已取消使用。");
      return;
    }
    const workflow = selectedWorkflowRef.current;
    if (!workflow) {
      setAssetIntentTargets([]);
      return;
    }

    const targets = compatibleAssetFields(workflow, pendingAssetIntent.assetType);
    if (!targets.length) {
      clearIntent();
      onNoticeRef.current("当前工作流没有可使用此素材的输入项。");
      return;
    }
    if (targets.length > 1) {
      setAssetIntentTargets(targets);
      return;
    }

    setAssetIntentTargets([]);
    applyToTarget(targets[0]);
  }, [applyToTarget, clearIntent, pendingAssetIntent, projectId, workflowFingerprint]);

  return {
    pendingAssetIntent,
    assetIntentTargets,
    applyToTarget,
    cancel: clearIntent,
  };
}
