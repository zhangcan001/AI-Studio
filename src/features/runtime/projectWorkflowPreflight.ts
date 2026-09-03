import type { RecipeViewModel } from "../../types/generation";
import type {
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigView,
} from "../../types/projectWorkflow";
import {
  filterImageRecipes,
  filterVideoRecipes,
  findRecipe,
  recipeRef,
  recipesForVideoMode,
  type H3CompatibleMode,
  type SelectedRecipeRef,
} from "./workflowCapabilities";
import {
  resolveProjectImageWorkflow,
  resolveProjectVideoWorkflow,
  type ProjectWorkflowResolutionSource,
} from "./projectWorkflowResolution";
import {
  H3_QUALITY_PROFILE,
  KERA2_WORKFLOW_ID,
  h3RecipeForMode,
  kera2RecipeContract,
} from "./productRuntimeScope";

export type ProjectWorkflowProductionPath = "IMAGE" | H3CompatibleMode;

export type ProjectWorkflowPreflightStatus = "READY" | "WARNING" | "BLOCKED";

export type ProjectWorkflowPreflightSource = Exclude<ProjectWorkflowResolutionSource, "explicit">;

export type ProjectWorkflowOverallStatus = "READY" | "PARTIAL" | "BLOCKED";

export interface ProjectWorkflowPreflightItem {
  path: ProjectWorkflowProductionPath;
  status: ProjectWorkflowPreflightStatus;
  recipe?: RecipeViewModel;
  source?: ProjectWorkflowPreflightSource;
  configuredRef?: SelectedRecipeRef;
  staleConfiguredBinding: boolean;
  usingFallback: boolean;
  message: string;
}

export interface ProjectWorkflowPreflightReport {
  overallStatus: ProjectWorkflowOverallStatus;
  readyCount: number;
  warningCount: number;
  blockedCount: number;
  totalCount: 8;
  items: ProjectWorkflowPreflightItem[];
}

export const PROJECT_WORKFLOW_VIDEO_MODES = [
  "FL2VA_TEXT_TO_VIDEO",
  "FL2VA_IMAGE_TO_VIDEO",
  "FL2VA_FIRST_LAST",
  "REF2VA_IMAGE",
  "REF2VA_AUDIO",
  "REF2VA_IMAGE_AUDIO",
  "REF2VA_VIDEO_IMAGE",
] as const satisfies readonly H3CompatibleMode[];

export const PROJECT_WORKFLOW_PRODUCTION_PATHS = [
  "IMAGE",
  ...PROJECT_WORKFLOW_VIDEO_MODES,
] as const satisfies readonly ProjectWorkflowProductionPath[];

const IMAGE_PATH: ProjectWorkflowProductionPath = "IMAGE";

function bindingRef(binding: ProjectWorkflowBindingView | null | undefined): SelectedRecipeRef | undefined {
  return binding
    ? recipeRef(binding)
    : undefined;
}

function availableBindingRef(binding: ProjectWorkflowBindingView | null | undefined): SelectedRecipeRef | undefined {
  return binding?.available ? bindingRef(binding) : undefined;
}

function imageRecommendation(catalog: RecipeViewModel[]): RecipeViewModel | undefined {
  return catalog.find((recipe) => recipe.workflowId === KERA2_WORKFLOW_ID && kera2RecipeContract(recipe).ok)
    ?? catalog[0];
}

function videoRecommendation(catalog: RecipeViewModel[], mode: H3CompatibleMode): RecipeViewModel | undefined {
  return h3RecipeForMode(catalog, mode, H3_QUALITY_PROFILE);
}

function sourceLabel(source: ProjectWorkflowPreflightSource | undefined): string {
  switch (source) {
    case "project_mode": return "模式专用";
    case "project_default": return "项目默认";
    case "recommended": return "系统推荐";
    case "compatible": return "兼容回退";
    default: return "无";
  }
}

function itemFromResolution(
  path: ProjectWorkflowProductionPath,
  resolution: { recipe?: RecipeViewModel; source?: ProjectWorkflowResolutionSource },
  configuredRef: SelectedRecipeRef | undefined,
  staleConfiguredBinding: boolean,
): ProjectWorkflowPreflightItem {
  const source = resolution.source === "explicit" ? undefined : resolution.source;
  const usingFallback = Boolean(
    resolution.recipe
      && (staleConfiguredBinding || (source !== "project_mode" && source !== "project_default")),
  );
  if (!resolution.recipe) {
    return {
      path,
      status: "BLOCKED",
      configuredRef,
      staleConfiguredBinding,
      usingFallback: false,
      message: "当前无兼容工作流。",
    };
  }
  if (staleConfiguredBinding) {
    return {
      path,
      status: "WARNING",
      recipe: resolution.recipe,
      source,
      configuredRef,
      staleConfiguredBinding,
      usingFallback,
      message: `项目绑定不可用，当前使用${sourceLabel(source)}。建议重新选择或清除失效绑定。`,
    };
  }
  return {
    path,
    status: "READY",
    recipe: resolution.recipe,
    source,
    configuredRef,
    staleConfiguredBinding,
    usingFallback,
    message: `当前路径可生产，来源：${sourceLabel(source)}。`,
  };
}

function imagePreflight(
  config: ProjectWorkflowConfigView,
  catalog: RecipeViewModel[],
): ProjectWorkflowPreflightItem {
  const candidates = filterImageRecipes(catalog);
  const binding = config.imageDefault;
  const configuredRef = bindingRef(binding);
  const staleConfiguredBinding = Boolean(
    binding && (!binding.available || !findRecipe(candidates, configuredRef)),
  );
  const resolution = resolveProjectImageWorkflow(
    candidates,
    undefined,
    availableBindingRef(binding),
    imageRecommendation(candidates),
  );
  return itemFromResolution(IMAGE_PATH, resolution, configuredRef, staleConfiguredBinding);
}

function videoPreflight(
  config: ProjectWorkflowConfigView,
  catalog: RecipeViewModel[],
  mode: H3CompatibleMode,
): ProjectWorkflowPreflightItem {
  const videoCatalog = filterVideoRecipes(catalog);
  const modeCandidates = recipesForVideoMode(videoCatalog, mode);
  const override = config.videoModeOverrides.find((binding) => binding.mode === mode);
  const videoDefault = config.videoDefault;
  const configuredRef = bindingRef(override ?? videoDefault);
  const staleOverride = Boolean(
    override && (!override.available || !findRecipe(modeCandidates, bindingRef(override))),
  );
  const staleDefault = Boolean(
    videoDefault && (!videoDefault.available || !findRecipe(videoCatalog, bindingRef(videoDefault))),
  );
  const resolution = resolveProjectVideoWorkflow(
    videoCatalog,
    mode,
    undefined,
    availableBindingRef(override),
    availableBindingRef(videoDefault),
    videoRecommendation(videoCatalog, mode),
    { allowGenericFallback: false },
  );
  return itemFromResolution(
    mode,
    resolution,
    configuredRef,
    staleOverride || (!override && staleDefault),
  );
}

export function preflightProjectWorkflow(
  config: ProjectWorkflowConfigView,
  catalog: RecipeViewModel[],
): ProjectWorkflowPreflightReport {
  const items = [
    imagePreflight(config, catalog),
    ...PROJECT_WORKFLOW_VIDEO_MODES.map((mode) => videoPreflight(config, catalog, mode)),
  ];
  const warningCount = items.filter((item) => item.status === "WARNING").length;
  const blockedCount = items.filter((item) => item.status === "BLOCKED").length;
  const readyCount = items.filter((item) => item.recipe !== undefined).length;
  const overallStatus: ProjectWorkflowOverallStatus = blockedCount === 0
    ? "READY"
    : readyCount > 0
      ? "PARTIAL"
      : "BLOCKED";
  return {
    overallStatus,
    readyCount,
    warningCount,
    blockedCount,
    totalCount: 8,
    items,
  };
}
