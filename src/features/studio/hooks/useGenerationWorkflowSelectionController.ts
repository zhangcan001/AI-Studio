import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getProjectWorkflowConfig } from "../../../services/tauriClient";
import type { RecipeViewModel } from "../../../types/generation";
import type { ProjectWorkflowConfigView } from "../../../types/projectWorkflow";
import type { RecentWorkflowRecord } from "../../production/productionUx";
import { KERA2_WORKFLOW_ID, kera2RecipeContract } from "../../runtime/productRuntimeScope";
import {
  findRecipe,
  migrateGenerationValues,
  recipeRef,
  sameRecipeRef,
  type SelectedRecipeRef,
} from "../../runtime/workflowCapabilities";
import { useStudioStore } from "../../../stores/studioStore";

export interface UseGenerationWorkflowSelectionControllerOptions {
  projectId: string;
  productCatalog: RecipeViewModel[];
  selectedWorkflow?: RecipeViewModel;
  draftDirty: boolean;
  onNotice: (notice: string | null) => void;
  onWorkflowChanged: () => void;
}

export function useGenerationWorkflowSelectionController({
  projectId,
  productCatalog,
  selectedWorkflow,
  draftDirty,
  onNotice,
  onWorkflowChanged,
}: UseGenerationWorkflowSelectionControllerOptions) {
  const [manualSelection, setManualSelection] = useState<SelectedRecipeRef>();
  const [projectWorkflowConfig, setProjectWorkflowConfig] = useState<ProjectWorkflowConfigView>();
  const [projectWorkflowConfigLoading, setProjectWorkflowConfigLoading] = useState(false);
  const projectConfigRequestVersion = useRef(0);
  const mountedRef = useRef(true);
  const onNoticeRef = useRef(onNotice);
  const onWorkflowChangedRef = useRef(onWorkflowChanged);
  onNoticeRef.current = onNotice;
  onWorkflowChangedRef.current = onWorkflowChanged;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    const requestVersion = ++projectConfigRequestVersion.current;
    setManualSelection(undefined);
    setProjectWorkflowConfig(undefined);
    setProjectWorkflowConfigLoading(true);
    void getProjectWorkflowConfig(projectId)
      .then((config) => {
        if (!mountedRef.current || projectConfigRequestVersion.current !== requestVersion) return;
        setProjectWorkflowConfig(config);
      })
      .catch(() => {
        if (!mountedRef.current || projectConfigRequestVersion.current !== requestVersion) return;
        setProjectWorkflowConfig({ projectId, videoModeOverrides: [] });
      })
      .finally(() => {
        if (!mountedRef.current || projectConfigRequestVersion.current !== requestVersion) return;
        setProjectWorkflowConfigLoading(false);
      });
  }, [projectId]);

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

  useEffect(() => {
    const explicitDraft = selectedWorkflow
      ? findRecipe(productCatalog, recipeRef(selectedWorkflow))
      : undefined;
    const next = explicitDraft ?? manualWorkflow ?? projectDefaultWorkflow ?? recommendedWorkflow;
    if (sameRecipeRef(next, selectedWorkflow)) return;
    useStudioStore.getState().setSelectedWorkflow(next);
    onWorkflowChangedRef.current();
  }, [manualWorkflow, productCatalog, projectDefaultWorkflow, recommendedWorkflow, selectedWorkflow]);

  useEffect(() => {
    if (!manualSelection || manualWorkflow || !productCatalog.length) return;
    setManualSelection(undefined);
    onNoticeRef.current("本次手动选择的工作流当前不可用，已切换到项目默认或推荐工作流。");
  }, [manualSelection, manualWorkflow, productCatalog.length]);

  const selectWorkflow = useCallback((workflow: RecipeViewModel) => {
    if (sameRecipeRef(workflow, selectedWorkflow)) return;
    if (draftDirty && !window.confirm("当前 Studio 草稿有未保存修改，确认切换工作流吗？")) return;
    setManualSelection(recipeRef(workflow));
    const currentState = useStudioStore.getState();
    if (currentState.selectedWorkflow) {
      currentState.loadDraft(workflow, migrateGenerationValues(currentState.selectedWorkflow, workflow, currentState.values));
    } else {
      currentState.setSelectedWorkflow(workflow);
    }
    onWorkflowChangedRef.current();
  }, [draftDirty, selectedWorkflow]);

  const restoreRecommendedWorkflow = useCallback(() => {
    if (!recommendedWorkflow) return;
    if (draftDirty && !window.confirm("当前 Studio 草稿有未保存修改，确认恢复推荐工作流吗？")) return;
    const fallbackWorkflow = projectDefaultWorkflow ?? recommendedWorkflow;
    if (sameRecipeRef(fallbackWorkflow, selectedWorkflow)) {
      setManualSelection(undefined);
      return;
    }
    setManualSelection(undefined);
    const currentState = useStudioStore.getState();
    if (currentState.selectedWorkflow) {
      currentState.loadDraft(fallbackWorkflow, migrateGenerationValues(currentState.selectedWorkflow, fallbackWorkflow, currentState.values));
    } else {
      currentState.setSelectedWorkflow(fallbackWorkflow);
    }
    onWorkflowChangedRef.current();
  }, [draftDirty, projectDefaultWorkflow, recommendedWorkflow, selectedWorkflow]);

  const continueRecentWorkflow = useCallback((_record: RecentWorkflowRecord, recipe?: RecipeViewModel) => {
    if (!recipe) {
      onNoticeRef.current("历史工作流当前不可用，无法创建新的创作入口。");
      return;
    }
    selectWorkflow(recipe);
  }, [selectWorkflow]);

  const selectionSource: "manual" | "project_default" | "recommended" | "compatible" = selectedWorkflow && manualSelection && sameRecipeRef(selectedWorkflow, manualSelection)
    ? "manual"
    : selectedWorkflow && projectDefaultWorkflow && sameRecipeRef(selectedWorkflow, projectDefaultWorkflow)
      ? "project_default"
      : selectedWorkflow && recommendedWorkflow && sameRecipeRef(selectedWorkflow, recommendedWorkflow)
        ? "recommended"
        : "compatible";

  return {
    manualWorkflow,
    projectWorkflowConfig,
    projectWorkflowConfigLoading,
    recommendedWorkflow,
    projectDefaultWorkflow,
    staleProjectDefault,
    selectionSource,
    selectWorkflow,
    restoreRecommendedWorkflow,
    continueRecentWorkflow,
  };
}
