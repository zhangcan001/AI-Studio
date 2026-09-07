import type { RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import type { RuntimeParameterProfile } from "../../types/settings";
import {
  preflightProjectWorkflow,
  type ProjectWorkflowPreflightItem,
} from "../runtime/projectWorkflowPreflight";
import type { WorkflowWorkspaceItem } from "./workflowWorkspaceAdapters";

export type WorkflowCenterProfileStatus = "READY" | "ATTENTION" | "BLOCKED";

export interface WorkflowCenterSummary {
  availableWorkflowCount: number;
  issueWorkflowCount: number;
  projectUsedWorkflowCount: number;
  runtimeProfileCount: number;
}

export interface WorkflowProductionProfile {
  key: string;
  label: string;
  purpose: string;
  recipe?: RecipeViewModel;
  status: WorkflowCenterProfileStatus;
  sourceLabel: string;
  configured: boolean;
  staleConfiguredBinding: boolean;
  usingFallback: boolean;
  runtimeProfileCount: number;
  technicalIdentity?: {
    workflowVersionId: string;
    recipeId: string;
  };
  issue?: string;
}

const VIDEO_DEFAULT_KEY = "VIDEO_DEFAULT";

const PATH_LABELS: Record<string, string> = {
  IMAGE: "图片默认",
  FL2VA_TEXT_TO_VIDEO: "文生视频",
  FL2VA_IMAGE_TO_VIDEO: "图生视频",
  FL2VA_FIRST_LAST: "首尾帧视频",
  REF2VA_IMAGE: "参考图视频",
  REF2VA_AUDIO: "参考音频视频",
  REF2VA_IMAGE_AUDIO: "参考图 + 音频",
  REF2VA_VIDEO_IMAGE: "参考视频 + 参考图",
};

function exactRecipe(
  catalog: RecipeViewModel[],
  workflowVersionId: string | undefined,
  recipeId: string | undefined,
): RecipeViewModel | undefined {
  if (!workflowVersionId || !recipeId) return undefined;
  return catalog.find((recipe) => (
    recipe.workflowVersionId === workflowVersionId
      && recipe.recipeId === recipeId
  ));
}

export function runtimeProfilesForRecipe(
  profiles: RuntimeParameterProfile[],
  recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId"> | undefined,
): RuntimeParameterProfile[] {
  if (!recipe) return [];
  return profiles.filter((profile) => (
    profile.workflowVersionId === recipe.workflowVersionId
      && profile.recipeId === recipe.recipeId
  ));
}

function workspaceItemForRecipe(
  items: WorkflowWorkspaceItem[],
  recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId"> | undefined,
): WorkflowWorkspaceItem | undefined {
  if (!recipe) return undefined;
  return items.find((item) => (
    item.currentVersionId === recipe.workflowVersionId
      && item.currentRecipe?.recipeId === recipe.recipeId
  ));
}

function readinessForRecipe(
  items: WorkflowWorkspaceItem[],
  recipe: RecipeViewModel | undefined,
): { status?: WorkflowCenterProfileStatus; issue?: string } {
  const item = workspaceItemForRecipe(items, recipe);
  if (!item) return {};
  if (item.readiness === "BLOCKED" || item.packageStatus !== "VALID") {
    return { status: "BLOCKED", issue: item.readinessReasons[0] ?? item.diagnostics[0]?.message };
  }
  if (
    !item.enabled
    || item.capability !== "READY"
    || item.readiness !== "READY"
    || item.diagnostics.length > 0
  ) {
    return { status: "ATTENTION", issue: item.readinessReasons[0] ?? item.diagnostics[0]?.message };
  }
  return { status: "READY" };
}

function sourceLabel(item: ProjectWorkflowPreflightItem): string {
  if (item.staleConfiguredBinding) return "项目绑定失效";
  switch (item.source) {
    case "project_mode": return "模式专用";
    case "project_default": return "项目默认";
    case "recommended": return "系统推荐";
    case "compatible": return "兼容回退";
    default: return item.configuredRef ? "项目配置" : "未设置";
  }
}

function preflightStatus(item: ProjectWorkflowPreflightItem): WorkflowCenterProfileStatus {
  if (item.status === "BLOCKED") return "BLOCKED";
  if (item.status === "WARNING") return "ATTENTION";
  return "READY";
}

function profileFromPreflight(
  item: ProjectWorkflowPreflightItem,
  items: WorkflowWorkspaceItem[],
  profiles: RuntimeParameterProfile[],
): WorkflowProductionProfile {
  const recipe = item.recipe;
  const runtimeProfiles = runtimeProfilesForRecipe(profiles, recipe);
  const workspaceReadiness = readinessForRecipe(items, recipe);
  const status = workspaceReadiness.status ?? preflightStatus(item);
  return {
    key: item.path,
    label: PATH_LABELS[item.path] ?? item.path,
    purpose: item.path === "IMAGE" ? "项目默认图片生产" : "项目视频生产路径",
    recipe,
    status,
    sourceLabel: sourceLabel(item),
    configured: Boolean(item.configuredRef),
    staleConfiguredBinding: item.staleConfiguredBinding,
    usingFallback: item.usingFallback,
    runtimeProfileCount: runtimeProfiles.length,
    technicalIdentity: recipe
      ? { workflowVersionId: recipe.workflowVersionId, recipeId: recipe.recipeId }
      : undefined,
    issue: workspaceReadiness.issue ?? item.message,
  };
}

function videoDefaultProfile(
  config: ProjectWorkflowConfigView,
  catalog: RecipeViewModel[],
  items: WorkflowWorkspaceItem[],
  profiles: RuntimeParameterProfile[],
): WorkflowProductionProfile {
  const binding = config.videoDefault;
  const recipe = exactRecipe(catalog, binding?.workflowVersionId, binding?.recipeId);
  const stale = Boolean(binding && (!binding.available || !recipe));
  const workspaceReadiness = readinessForRecipe(items, recipe);
  const status = workspaceReadiness.status ?? (stale ? "ATTENTION" : recipe ? "READY" : "BLOCKED");
  return {
    key: VIDEO_DEFAULT_KEY,
    label: "视频默认",
    purpose: "项目默认视频生产",
    recipe,
    status,
    sourceLabel: stale ? "项目绑定失效" : recipe ? "项目默认" : "未设置",
    configured: Boolean(binding),
    staleConfiguredBinding: stale,
    usingFallback: false,
    runtimeProfileCount: runtimeProfilesForRecipe(profiles, recipe).length,
    technicalIdentity: recipe
      ? { workflowVersionId: recipe.workflowVersionId, recipeId: recipe.recipeId }
      : binding
        ? { workflowVersionId: binding.workflowVersionId, recipeId: binding.recipeId }
        : undefined,
    issue: workspaceReadiness.issue ?? (stale
      ? "项目绑定不可用，请重新选择或清除绑定。"
      : "尚未设置视频默认工作流；下方模式路径仍可使用系统推荐。"),
  };
}

export function buildProductionProfiles(
  config: ProjectWorkflowConfigView,
  catalog: RecipeViewModel[],
  items: WorkflowWorkspaceItem[],
  profiles: RuntimeParameterProfile[],
): WorkflowProductionProfile[] {
  const report = preflightProjectWorkflow(config, catalog);
  const image = report.items[0];
  return [
    ...(image ? [profileFromPreflight(image, items, profiles)] : []),
    videoDefaultProfile(config, catalog, items, profiles),
    ...report.items.slice(1).map((item) => profileFromPreflight(item, items, profiles)),
  ];
}

function isProductionReady(item: WorkflowWorkspaceItem): boolean {
  return (
    item.libraryState !== "REMOVED"
      && !item.archived
      && item.enabled
      && item.packageStatus === "VALID"
      && item.capability === "READY"
      && item.readiness === "READY"
      && item.diagnostics.length === 0
  );
}

export function buildWorkflowCenterSummary(
  items: WorkflowWorkspaceItem[],
  profiles: RuntimeParameterProfile[],
  productionProfiles: WorkflowProductionProfile[] = [],
): WorkflowCenterSummary {
  const active = items.filter((item) => item.libraryState !== "REMOVED" && !item.archived);
  const projectRecipes = new Set(
    productionProfiles
      .filter((profile) => profile.configured && profile.recipe)
      .map((profile) => `${profile.recipe!.workflowVersionId}:${profile.recipe!.recipeId}`),
  );
  return {
    availableWorkflowCount: active.filter(isProductionReady).length,
    issueWorkflowCount: active.filter((item) => !isProductionReady(item)).length,
    projectUsedWorkflowCount: projectRecipes.size,
    runtimeProfileCount: profiles.length,
  };
}
