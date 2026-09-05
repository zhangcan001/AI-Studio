import type { RecipeViewModel } from "../../types/generation";
import type {
  WorkflowProductionWorkspaceView,
  WorkflowRegistryRecipeView,
  WorkflowRegistryVersionView,
  WorkflowStagingView,
} from "../../types/workflowOnboarding";

export type WorkflowWorkspaceQueryMode = "FAST" | "REFRESH";

/** Static Registry truth returned by workflow_workspace_query. */
export interface WorkflowWorkspaceRegistryView {
  workflowId: string;
  name: string;
  sourceKind: string;
  libraryState: string;
  currentVersionId?: string | null;
  currentVersion?: WorkflowRegistryVersionView | null;
  currentRecipe?: WorkflowRegistryRecipeView | null;
  versions: WorkflowRegistryVersionView[];
  recipes: WorkflowRegistryRecipeView[];
  projectUsageCount: number;
  historyCount: number;
}

/** Runtime truth for one exact (workflowVersionId, recipeId) pair. */
export interface WorkflowWorkspaceRuntimeView {
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  name: string;
  category: string;
  mode: string;
  workflowVersion: string;
  recipeVersion: string;
  workflowSha256: string;
  recipeSha256: string;
  artifactId?: string | null;
  artifactSourceKind?: string | null;
  packageName?: string | null;
  packageSourcePath?: string | null;
  artifactStatus: string;
  packageStatus: string;
  libraryState: string;
  enabled: boolean;
  archived: boolean;
  archivedAt?: string | null;
  capability: string;
  capabilityIssues: WorkflowProductionWorkspaceView["capabilityIssues"];
  readiness: string;
  readinessReasons: string[];
  diagnostics: WorkflowProductionWorkspaceView["diagnostics"];
  nodeCount: number;
  liveVerifiedAt?: string | null;
  hasSuccessfulRun: boolean;
  latestSuccessAt?: string | null;
  latestFailureAt?: string | null;
  activeTasks: number;
  totalTasks: number;
}

/** Exact backend response. There is intentionally no flat/legacy union. */
export interface WorkflowWorkspaceQueryItem {
  registry: WorkflowWorkspaceRegistryView;
  runtime: WorkflowWorkspaceRuntimeView[];
  /** Test-only marker used by the legacy mock bridge; native responses omit it. */
  legacyTestAdapter?: true;
}

export interface WorkflowWorkspaceQueryResponse {
  items: WorkflowWorkspaceQueryItem[];
  staging: WorkflowStagingView[];
}

/** The renderer's existing row shape, flattened from the exact query item. */
export interface WorkflowWorkspaceItem extends WorkflowProductionWorkspaceView {
  /** False only for the test-only legacy response adapter. */
  registryBacked: boolean;
  sourceKind: string;
  libraryState: string;
  currentVersionId?: string;
  currentRecipe?: WorkflowRegistryRecipeView;
  versions: WorkflowRegistryVersionView[];
  registryRecipes: WorkflowRegistryRecipeView[];
  projectUsageCount: number;
  historyCount: number;
  removedAt?: string;
}

function parseSemver(value?: string): { core: [number, number, number]; prerelease: string[] } | undefined {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(value?.trim() ?? "");
  if (!match) return undefined;
  const core = match.slice(1, 4).map(Number) as [number, number, number];
  if (core.some((part) => !Number.isSafeInteger(part))) return undefined;
  const prerelease = match[4]?.split(".") ?? [];
  if (prerelease.some((part) => /^\d+$/.test(part) && ((part.length > 1 && part.startsWith("0")) || !Number.isSafeInteger(Number(part))))) return undefined;
  return { core, prerelease };
}

function compareSemver(
  left: { core: [number, number, number]; prerelease: string[] },
  right: { core: [number, number, number]; prerelease: string[] },
): number {
  for (let index = 0; index < left.core.length; index += 1) {
    if (left.core[index] !== right.core[index]) return left.core[index] - right.core[index];
  }
  if (!left.prerelease.length || !right.prerelease.length) {
    return left.prerelease.length === right.prerelease.length ? 0 : left.prerelease.length ? -1 : 1;
  }
  for (let index = 0; index < Math.min(left.prerelease.length, right.prerelease.length); index += 1) {
    const leftPart = left.prerelease[index];
    const rightPart = right.prerelease[index];
    if (leftPart === rightPart) continue;
    const leftNumeric = /^\d+$/.test(leftPart);
    const rightNumeric = /^\d+$/.test(rightPart);
    if (leftNumeric && rightNumeric) return Number(leftPart) - Number(rightPart);
    if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1;
    return leftPart < rightPart ? -1 : 1;
  }
  return left.prerelease.length - right.prerelease.length;
}

function registryRecipeSummary(recipe: WorkflowRegistryRecipeView, workflowVersionId: string): WorkflowProductionWorkspaceView["recipes"][number] {
  return {
    recipeId: recipe.recipeId,
    workflowVersionId,
    version: recipe.version ?? recipe.recipeVersion ?? "—",
    inputCount: recipe.inputCount ?? 0,
    outputCount: recipe.outputCount ?? 0,
    presetCount: recipe.presetCount,
  };
}

/** Pick the newest recipe that is explicitly present in the production catalog. */
export function latestCatalogRecipeForWorkflowItem(
  item: Pick<WorkflowWorkspaceItem, "workflowVersionId" | "recipes">,
  catalog: RecipeViewModel[],
): RecipeViewModel | undefined {
  if (!item.workflowVersionId) return undefined;
  const itemRecipes = new Map(item.recipes.map((recipe) => [recipe.recipeId, recipe.version]));
  return catalog
    .filter((candidate) => candidate.workflowVersionId === item.workflowVersionId && itemRecipes.has(candidate.recipeId))
    .map((candidate) => ({
      candidate,
      version: parseSemver(candidate.recipeVersion) ?? parseSemver(itemRecipes.get(candidate.recipeId)),
    }))
    .filter((entry): entry is { candidate: RecipeViewModel; version: { core: [number, number, number]; prerelease: string[] } } => Boolean(entry.version))
    .sort((left, right) => compareSemver(right.version, left.version) || left.candidate.recipeId.localeCompare(right.candidate.recipeId))
    .map((entry) => entry.candidate)[0];
}

function exactRuntime(
  item: WorkflowWorkspaceQueryItem,
  workflowVersionId: string | undefined,
  recipeId: string | undefined,
): WorkflowWorkspaceRuntimeView | undefined {
  if (!workflowVersionId || !recipeId) return undefined;
  return item.runtime.find((runtime) => runtime.workflowVersionId === workflowVersionId && runtime.recipeId === recipeId);
}

/**
 * Flatten the nested unified backend response without deriving identity from a
 * package name or scanning a legacy workspace. Runtime evidence is selected
 * only by the Registry's explicit current version and current recipe IDs.
 */
export function normalizeWorkspaceItem(item: WorkflowWorkspaceQueryItem): WorkflowWorkspaceItem {
  const { registry } = item;
  const currentVersionId = registry.currentVersionId ?? undefined;
  const currentVersion = registry.currentVersion ?? undefined;
  const currentRecipe = registry.currentRecipe ?? undefined;
  const runtime = exactRuntime(item, currentVersionId, currentRecipe?.recipeId);
  const sourceKind = registry.sourceKind.trim().toUpperCase();
  const libraryState = registry.libraryState.trim().toUpperCase();
  const currentRecipes = currentVersion
    ? (currentVersion.recipes ?? []).map((recipe) => registryRecipeSummary(recipe, currentVersion.workflowVersionId))
    : currentVersionId
      ? registry.recipes
        .filter((recipe) => recipe.workflowVersionId === currentVersionId)
        .map((recipe) => registryRecipeSummary(recipe, currentVersionId))
      : [];
  const missingRuntime = !runtime && currentRecipe
    ? [{ code: "RUNTIME_PACKAGE_MISSING", message: "the exact recipe runtime artifact is not registered" }]
    : [];

  return {
    packageName: runtime?.packageName ?? "",
    builtin: sourceKind === "PRODUCT",
    source: sourceKind,
    archived: runtime?.archived ?? libraryState === "REMOVED",
    archivedAt: runtime?.archivedAt ?? undefined,
    packageStatus: runtime?.packageStatus ?? "MISSING",
    workflowId: registry.workflowId,
    workflowVersionId: currentVersionId,
    name: registry.name,
    category: runtime?.category,
    mode: runtime?.mode,
    workflowVersion: runtime?.workflowVersion ?? currentVersion?.version,
    workflowSha256: runtime?.workflowSha256 ?? currentVersion?.workflowSha256 ?? "",
    recipeSha256: runtime?.recipeSha256 ?? currentRecipe?.recipeSha256,
    enabled: runtime?.enabled ?? false,
    capability: runtime?.capability ?? "NOT_CHECKED",
    readiness: runtime?.readiness ?? "BLOCKED",
    readinessReasons: runtime?.readinessReasons ?? [],
    capabilityIssues: runtime?.capabilityIssues ?? [],
    nodeCount: runtime?.nodeCount ?? 0,
    recipes: currentRecipes,
    activeTasks: runtime?.activeTasks ?? 0,
    totalTasks: runtime?.totalTasks ?? 0,
    hasSuccessfulRun: runtime?.hasSuccessfulRun ?? false,
    latestSuccessAt: runtime?.latestSuccessAt ?? undefined,
    latestFailureAt: runtime?.latestFailureAt ?? undefined,
    liveVerifiedAt: runtime?.liveVerifiedAt ?? undefined,
    diagnostics: runtime?.diagnostics ?? missingRuntime,
    registryBacked: item.legacyTestAdapter !== true,
    sourceKind,
    libraryState,
    currentVersionId,
    currentRecipe,
    versions: registry.versions,
    registryRecipes: registry.recipes,
    projectUsageCount: registry.projectUsageCount,
    historyCount: registry.historyCount,
    removedAt: runtime?.archivedAt ?? (libraryState === "REMOVED" ? undefined : undefined),
  };
}

export function normalizeWorkspaceItems(items: WorkflowWorkspaceQueryItem[]): WorkflowWorkspaceItem[] {
  return items.map(normalizeWorkspaceItem);
}

export function canPurgeWorkflow(item: WorkflowWorkspaceItem): boolean {
  return item.libraryState === "REMOVED"
    && item.sourceKind === "USER"
    && item.historyCount === 0
    && item.projectUsageCount === 0
    && item.activeTasks === 0
    && item.versions.every((version) => (version.activeQueueItemCount ?? 0) === 0);
}
