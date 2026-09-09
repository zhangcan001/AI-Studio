import { useCallback, useEffect, useMemo, useState } from "react";
import { getProjectWorkflowConfig } from "../../../services/tauriClient";
import type { RecipeViewModel } from "../../../types/generation";
import type { ProjectWorkflowConfigView } from "../../../types/projectWorkflow";
import { h3RecipeForMode, type H3QualityProfile } from "../../runtime/productRuntimeScope";
import {
  filterVideoRecipes,
  findRecipe,
  recipeRef,
  videoRecipeCapability,
  type H3CompatibleMode,
  type SelectedRecipeRef,
} from "../../runtime/workflowCapabilities";
import {
  resolveProjectFolderWorkflow,
  resolveProjectVideoWorkflow,
  type ProjectWorkflowResolutionSource,
} from "../../runtime/projectWorkflowResolution";
import type { H3GenerationMode } from "../assetVideoBatch";

export interface UseAssetVideoWorkflowControllerOptions {
  projectId: string;
  catalog: RecipeViewModel[];
  generationMode: H3GenerationMode;
  qualityProfile: H3QualityProfile;
  projectModes: readonly H3CompatibleMode[];
}

export interface ResolvedProjectVideoRecipe {
  mode: H3CompatibleMode;
  recipe?: RecipeViewModel;
  source: "manual" | "recommended" | "compatible";
  staleManualSelection: boolean;
}

export function videoWorkflowCandidatesForMode(
  catalog: RecipeViewModel[],
  mode: H3GenerationMode,
): RecipeViewModel[] {
  return catalog.filter((candidate) => videoRecipeCapability(candidate).supportedModes.includes(mode));
}

export function useAssetVideoWorkflowController({
  projectId,
  catalog,
  generationMode,
  qualityProfile,
  projectModes,
}: UseAssetVideoWorkflowControllerOptions) {
  const [manualVideoSelection, setManualVideoSelection] = useState<SelectedRecipeRef>();
  const [projectWorkflowConfig, setProjectWorkflowConfig] = useState<ProjectWorkflowConfigView>();
  const [projectWorkflowStrategy, setProjectWorkflowStrategy] = useState<"AUTO" | "MANUAL">("AUTO");
  const [projectManualOverrides, setProjectManualOverrides] = useState<Partial<Record<H3CompatibleMode, SelectedRecipeRef>>>({});
  const [workflowSelectionNotice, setWorkflowSelectionNotice] = useState<string>();

  const videoCatalog = useMemo(() => filterVideoRecipes(catalog), [catalog]);
  const recommendedRecipe = useMemo(
    () => h3RecipeForMode(videoCatalog, generationMode, qualityProfile),
    [generationMode, qualityProfile, videoCatalog],
  );
  const projectVideoDefault = useMemo(
    () => projectWorkflowConfig?.videoDefault?.available
      ? recipeRef(projectWorkflowConfig.videoDefault)
      : undefined,
    [projectWorkflowConfig],
  );
  const projectModeOverride = useMemo(
    () => {
      const binding = projectWorkflowConfig?.videoModeOverrides.find((candidate) => candidate.mode === generationMode);
      return binding?.available ? recipeRef(binding) : undefined;
    },
    [generationMode, projectWorkflowConfig],
  );
  const resolvedVideoRecipe = useMemo(
    () => resolveProjectVideoWorkflow(
      videoCatalog,
      generationMode,
      manualVideoSelection,
      projectModeOverride,
      projectVideoDefault,
      recommendedRecipe,
      { allowGenericFallback: false },
    ),
    [generationMode, manualVideoSelection, projectModeOverride, projectVideoDefault, recommendedRecipe, videoCatalog],
  );
  const staleManualVideoSelection = Boolean(
    manualVideoSelection && !findRecipe(videoWorkflowCandidatesForMode(videoCatalog, generationMode), manualVideoSelection),
  );
  const staleProjectVideoBinding = Boolean(
    (projectWorkflowConfig?.videoDefault
      && (!projectWorkflowConfig.videoDefault.available
        || !findRecipe(videoCatalog, projectWorkflowConfig.videoDefault)))
      || (() => {
        const binding = projectWorkflowConfig?.videoModeOverrides.find((candidate) => candidate.mode === generationMode);
        return Boolean(binding
          && (!binding.available
            || !findRecipe(videoWorkflowCandidatesForMode(videoCatalog, generationMode), binding)));
      })(),
  );
  const workflowSelectionSource: ProjectWorkflowResolutionSource | "manual" | undefined = resolvedVideoRecipe.source === "explicit"
    ? "manual"
    : resolvedVideoRecipe.source;
  const recipe = resolvedVideoRecipe.recipe;
  const resolvedProjectRecipes = useMemo<ResolvedProjectVideoRecipe[]>(
    () => projectModes.map((mode) => {
      const configured = projectWorkflowConfig?.videoModeOverrides.find((binding) => binding.mode === mode);
      const resolution = resolveProjectFolderWorkflow(
        videoCatalog,
        mode,
        projectWorkflowStrategy === "MANUAL" ? projectManualOverrides[mode] : undefined,
        configured?.available ? recipeRef(configured) : undefined,
        projectVideoDefault,
        h3RecipeForMode(videoCatalog, mode, qualityProfile),
      );
      const manual = projectWorkflowStrategy === "MANUAL" && Boolean(projectManualOverrides[mode]);
      return {
        mode,
        recipe: resolution.recipe,
        source: resolution.source === "explicit" || resolution.source === "project_mode" || resolution.source === "project_default"
          ? "recommended" as const
          : resolution.source ?? "compatible" as const,
        staleManualSelection: manual && resolution.source !== "explicit",
      };
    }),
    [projectManualOverrides, projectModes, projectVideoDefault, projectWorkflowConfig, projectWorkflowStrategy, qualityProfile, videoCatalog],
  );
  const projectRecommendations = useMemo(
    () => Object.fromEntries(projectModes.map((mode) => {
      const resolved = resolvedProjectRecipes.find((item) => item.mode === mode);
      return [mode, resolved?.recipe ? recipeRef(resolved.recipe) : undefined];
    })) as Partial<Record<H3CompatibleMode, SelectedRecipeRef>>,
    [projectModes, resolvedProjectRecipes],
  );

  useEffect(() => {
    setManualVideoSelection(undefined);
    setProjectWorkflowConfig(undefined);
    void getProjectWorkflowConfig(projectId)
      .then(setProjectWorkflowConfig)
      .catch(() => setProjectWorkflowConfig({ projectId, videoModeOverrides: [] }));
    setProjectWorkflowStrategy("AUTO");
    setProjectManualOverrides({});
    setWorkflowSelectionNotice(undefined);
  }, [projectId]);

  useEffect(() => {
    if (!staleManualVideoSelection || !manualVideoSelection) return;
    setManualVideoSelection(undefined);
    setWorkflowSelectionNotice("当前工作流不支持此生成模式，已切换到兼容工作流。");
  }, [manualVideoSelection, staleManualVideoSelection]);

  useEffect(() => {
    if (!resolvedVideoRecipe.staleProjectBinding && !staleProjectVideoBinding) return;
    setWorkflowSelectionNotice("项目工作流绑定已失效，当前仅临时使用兼容/推荐工作流；请在项目设置中重新选择或清除绑定。");
  }, [resolvedVideoRecipe.staleProjectBinding, staleProjectVideoBinding]);

  const selectVideoWorkflow = useCallback((nextRecipe: RecipeViewModel) => {
    setManualVideoSelection(recipeRef(nextRecipe));
    setWorkflowSelectionNotice(undefined);
  }, []);

  const restoreRecommendedVideoWorkflow = useCallback(() => {
    if (!recommendedRecipe) return;
    setManualVideoSelection(undefined);
    setWorkflowSelectionNotice(undefined);
  }, [recommendedRecipe]);

  const setProjectWorkflowOverride = useCallback((mode: H3CompatibleMode, ref: SelectedRecipeRef | undefined) => {
    setProjectManualOverrides((current) => {
      const next = { ...current };
      if (ref) next[mode] = ref;
      else delete next[mode];
      return next;
    });
  }, []);

  return {
    videoCatalog,
    recipe,
    recommendedRecipe,
    resolvedVideoRecipe,
    workflowSelectionSource,
    staleManualVideoSelection,
    staleProjectVideoBinding,
    workflowSelectionNotice,
    projectWorkflowConfig,
    projectWorkflowStrategy,
    setProjectWorkflowStrategy,
    projectManualOverrides,
    resolvedProjectRecipes,
    projectRecommendations,
    selectVideoWorkflow,
    restoreRecommendedVideoWorkflow,
    setProjectWorkflowOverride,
  };
}
