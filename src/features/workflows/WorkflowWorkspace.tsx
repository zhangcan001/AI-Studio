import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  checkOnboardingCapability,
  analyzeWorkflowImport,
  cleanWorkflowStaging,
  commitWorkflowImport,
  compareWorkflowVersions,
  createGeneration,
  discardOnboarding,
  deleteWorkflow,
  deleteWorkflowVersion,
  duplicateWorkflowRecipe,
  exportWorkflowPackage,
  getOnboardingDraft,
  importWorkflowPackageBackup,
  inspectWorkflowDeletion,
  listWorkflowRegistry,
  listWorkflowProductionWorkspace,
  refreshWorkflowProductionWorkspace,
  repairBuiltinWorkflowPackage,
  pickApiWorkflow,
  autoConfirmOnboarding,
  publishOnboarding,
  recheckWorkflowCapability,
  recheckAllWorkflowCapabilities,
  removeWorkflow,
  renameWorkflow,
  rerecognizeWorkflow,
  removeOnboardingInputMapping,
  restoreWorkflowVersion,
  restoreWorkflow,
  purgeWorkflow,
  setWorkflowCurrentVersion,
  setWorkflowEnabled,
  setOnboardingInputMapping,
  setOnboardingMetadata,
  setOnboardingOutputMapping,
  validateOnboarding,
} from "../../services/tauriClient";
import { useWorkflowOnboardingStore, type WorkflowOnboardingStep } from "../../stores/workflowOnboardingStore";
import type {
  WorkflowFieldType,
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowDeletionInspection,
  WorkflowImportCommitAction,
  WorkflowInputView,
  WorkflowNodeView,
  WorkflowOnboardingDraftView,
  WorkflowOnboardingInputMappingRequest,
  WorkflowOnboardingOutputMappingRequest,
  WorkflowProductionWorkspaceView,
  WorkflowRegistryRecipeView,
  WorkflowRegistryVersionView,
  WorkflowRegistryView,
  WorkflowRegistryResponse,
  WorkflowDeletionResult,
  WorkflowVersionDiffView,
} from "../../types/workflowOnboarding";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import { formatUiError, toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, stagingStatusLabel, workflowDisplayName } from "../../i18n/statusLabels";
import { WorkflowSmartImport, workflowImportFormat } from "./WorkflowSmartImport";
import type { WorkflowImportErrorView } from "./WorkflowImportIssues";
import { WorkflowDeleteDialog } from "./WorkflowDeleteDialog";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";

interface Props {
  projectId?: string;
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  onCatalogChanged: () => Promise<void>;
  onOpenStudio: (workflowId: string, recipeId: string) => Promise<void>;
  onUseInProject: (workflowId: string, recipeId: string) => Promise<void>;
  onOpenTask?: (taskId: string) => void;
}

interface ParsedSemver {
  core: [number, number, number];
  prerelease: string[];
}

function parseSemver(value?: string): ParsedSemver | undefined {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(value?.trim() ?? "");
  if (!match) return undefined;
  const core = match.slice(1, 4).map(Number) as [number, number, number];
  if (core.some((part) => !Number.isSafeInteger(part))) return undefined;
  const prerelease = match[4]?.split(".") ?? [];
  if (prerelease.some((part) => /^\d+$/.test(part) && ((part.length > 1 && part.startsWith("0")) || !Number.isSafeInteger(Number(part))))) return undefined;
  return { core, prerelease };
}

function compareSemver(left: ParsedSemver, right: ParsedSemver): number {
  for (let index = 0; index < left.core.length; index += 1) {
    if (left.core[index] !== right.core[index]) return left.core[index] - right.core[index];
  }
  if (!left.prerelease.length || !right.prerelease.length) return left.prerelease.length === right.prerelease.length ? 0 : left.prerelease.length ? -1 : 1;
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

function nextWorkflowVersion(value: string): string {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(value.trim());
  if (!match) return "1.0.1";
  return `${match[1]}.${match[2]}.${Number(match[3]) + 1}`;
}

export function latestCatalogRecipeForWorkflowItem(item: WorkflowProductionWorkspaceView, catalog: RecipeViewModel[]): RecipeViewModel | undefined {
  if (!item.workflowVersionId) return undefined;
  const itemRecipes = new Map(item.recipes.map((recipe) => [recipe.recipeId, recipe.version]));
  return catalog
    .filter((candidate) => candidate.workflowVersionId === item.workflowVersionId && itemRecipes.has(candidate.recipeId))
    .map((candidate) => ({
      candidate,
      version: parseSemver(candidate.recipeVersion) ?? parseSemver(itemRecipes.get(candidate.recipeId)),
    }))
    .filter((entry): entry is { candidate: RecipeViewModel; version: ParsedSemver } => Boolean(entry.version))
    .sort((left, right) => compareSemver(right.version, left.version) || left.candidate.recipeId.localeCompare(right.candidate.recipeId))
    .map((entry) => entry.candidate)[0];
}

const steps: Array<{ value: WorkflowOnboardingStep; label: string }> = [
  { value: "inspect", label: "检查工作流" },
  { value: "compatibility", label: "兼容性检查" },
  { value: "inputs", label: "输入映射" },
  { value: "outputs", label: "输出映射" },
  { value: "metadata", label: "基本信息" },
  { value: "validate", label: "校验" },
  { value: "publish", label: "发布" },
];

const fieldTypes: WorkflowFieldType[] = [
  "textarea",
  "integer",
  "number",
  "seed",
  "image",
  "images",
  "video",
  "videos",
  "audio",
  "audios",
];

interface MappingDraft {
  semanticKey: string;
  fieldType: WorkflowFieldType;
  label: string;
  required: boolean;
  defaultValue: string;
  minValue: string;
  maxValue: string;
  minItems: string;
  maxItems: string;
  itemIndex: string;
  step: string;
}

type ParameterMappingEdit = {
  mapping: WorkflowOnboardingDraftView["inputMappings"][number];
  draft: MappingDraft;
};

interface OutputDraft {
  outputId: string;
  label: string;
  type: "image" | "video";
  nodeId: string;
  required: boolean;
}

export function createDefaultOutputDraft(): OutputDraft {
  return {
    outputId: "output_1",
    label: "输出结果",
    type: "image",
    nodeId: "",
    required: true,
  };
}

interface MetadataDraft {
  workflowId: string;
  name: string;
  workflowVersion: string;
  recipeVersion: string;
  category: string;
  mode: string;
}

interface WorkflowDeletionTarget {
  item: WorkflowWorkspaceItem;
  inspection: WorkflowDeletionInspection;
  mode: "REMOVE" | "PURGE";
}

interface WorkflowWorkspaceItem extends WorkflowProductionWorkspaceView {
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

function workflowImportErrorView(error: unknown): WorkflowImportErrorView {
  const formatted = formatUiError(error);
  const haystack = `${formatted.code ?? ""} ${formatted.technicalMessage}`.toUpperCase();
  if (/INVALID[_\s-]*JSON|JSON[_\s-]*(PARSE|INVALID)|MALFORMED[_\s-]*JSON/.test(haystack)) {
    return { kind: "INVALID_JSON", message: "无法读取这个文件，它不是有效的 JSON。" };
  }
  if (/UI[_\s-]*(FORMAT|WORKFLOW)|WORKFLOW[_\s-]*UI|UNSUPPORTED[_\s-]*UI/.test(haystack)) {
    return { kind: "UI_FORMAT", message: "检测到 ComfyUI 普通工作流 JSON，但这个格式不能安全地直接添加。" };
  }
  if (/\bUNKNOWN\b|UNRECOGNIZED|WORKFLOW_NOT_API_FORMAT/.test(haystack)) {
    return { kind: "UNKNOWN_FORMAT", message: "这个 JSON 不是可识别的 ComfyUI 工作流。" };
  }
  return {
    kind: "IMPORT_FAILED",
    message: formatted.message === "操作失败，请查看技术详情。"
      ? "工作流导入未完成，请查看详细原因后重试。"
      : formatted.message,
    code: formatted.code,
    technicalMessage: formatted.technicalMessage,
  };
}

function normalizedState(value: string | undefined, fallback: "ACTIVE" | "REMOVED"): string {
  const state = value?.trim().toUpperCase();
  return state === "REMOVED" ? "REMOVED" : state === "ACTIVE" ? "ACTIVE" : fallback;
}

function registryRecipeSummary(recipe: WorkflowRegistryRecipeView, workflowVersionId?: string): WorkflowWorkspaceItem["recipes"][number] {
  return {
    recipeId: recipe.recipeId,
    version: recipe.version ?? recipe.recipeVersion ?? "—",
    inputCount: recipe.inputCount ?? 0,
    outputCount: recipe.outputCount ?? 0,
    presetCount: recipe.presetCount,
    ...(workflowVersionId ? { workflowVersionId } : {}),
  };
}

function registryVersion(item: WorkflowRegistryView, version: WorkflowRegistryVersionView | undefined): WorkflowRegistryVersionView | undefined {
  if (version) return version;
  if (item.currentVersionId) return item.versions?.find((candidate) => candidate.workflowVersionId === item.currentVersionId);
  return item.versions?.[0];
}

function normalizeRegistryItem(item: WorkflowRegistryView): WorkflowWorkspaceItem {
  const versions = item.versions ?? [];
  const current = registryVersion(item, item.currentVersion ?? undefined);
  const allRecipes = item.recipes?.length
    ? item.recipes
    : versions.flatMap((version) => (version.recipes ?? []).map((recipe) => ({ ...recipe, workflowVersionId: recipe.workflowVersionId ?? version.workflowVersionId })));
  const currentVersionId = current?.workflowVersionId ?? item.currentVersionId ?? undefined;
  const currentRecipes = current?.recipes?.length
    ? current.recipes.map((recipe) => ({ ...recipe, workflowVersionId: recipe.workflowVersionId ?? currentVersionId }))
    : allRecipes.filter((recipe) => !recipe.workflowVersionId || recipe.workflowVersionId === currentVersionId);
  const currentRecipe = item.currentRecipe
    ?? currentRecipes[0]
    ?? allRecipes.find((recipe) => !recipe.workflowVersionId || recipe.workflowVersionId === currentVersionId);
  const sourceKind = item.sourceKind?.trim().toUpperCase() || "USER";
  const libraryState = normalizedState(item.libraryState, "ACTIVE");
  const packageName = currentRecipe?.packageName ?? current?.packageName ?? `${item.workflowId}-runtime`;
  const activeTasks = item.activeTaskCount ?? current?.activeTasks ?? 0;
  const totalTasks = item.totalTaskCount ?? current?.totalTasks ?? item.historyCount ?? 0;
  const capability = current?.capability ?? item.capability ?? "NOT_CHECKED";
  const readiness = current?.readiness ?? (capability === "READY" ? "READY" : "BLOCKED");
  const archived = libraryState === "REMOVED";
  return {
    packageName,
    builtin: sourceKind === "PRODUCT",
    source: sourceKind,
    archived,
    archivedAt: item.removedAt ?? undefined,
    packageStatus: currentRecipe?.packageStatus ?? current?.packageStatus ?? "VALID",
    workflowId: item.workflowId,
    workflowVersionId: currentVersionId,
    name: item.name,
    category: undefined,
    mode: undefined,
    workflowVersion: current?.version ?? current?.workflowVersion,
    workflowSha256: current?.rawSha256 ?? current?.workflowSha256 ?? "",
    recipeSha256: currentRecipe?.recipeSha256,
    enabled: currentRecipe?.enabled ?? current?.enabled ?? !archived,
    capability,
    readiness,
    readinessReasons: current?.readinessReasons ?? [],
    capabilityIssues: current?.capabilityIssues ?? item.capabilityIssues ?? [],
    nodeCount: current?.nodeCount ?? 0,
    recipes: currentRecipes.map((recipe) => registryRecipeSummary(recipe, currentVersionId)),
    activeTasks,
    totalTasks,
    hasSuccessfulRun: item.hasSuccessfulRun ?? current?.hasSuccessfulRun ?? false,
    latestSuccessAt: item.latestSuccessAt ?? current?.latestSuccessAt,
    latestFailureAt: item.latestFailureAt ?? current?.latestFailureAt,
    liveVerifiedAt: item.liveVerifiedAt ?? current?.liveVerifiedAt,
    diagnostics: current?.diagnostics ?? [],
    registryBacked: true,
    sourceKind,
    libraryState,
    currentVersionId,
    currentRecipe,
    versions,
    registryRecipes: allRecipes,
    projectUsageCount: item.projectUsageCount ?? 0,
    historyCount: item.historyCount ?? totalTasks,
    removedAt: item.removedAt ?? undefined,
  };
}

function normalizeLegacyItem(item: WorkflowProductionWorkspaceView): WorkflowWorkspaceItem {
  const workflowId = item.workflowId ?? item.packageName;
  const sourceKind = item.builtin || item.source?.trim().toUpperCase() === "PRODUCT" ? "PRODUCT" : "USER";
  const recipeList = item.recipes.map((recipe) => ({
    recipeId: recipe.recipeId,
    workflowVersionId: item.workflowVersionId,
    version: recipe.version,
    inputCount: recipe.inputCount,
    outputCount: recipe.outputCount,
    presetCount: recipe.presetCount,
  }));
  const version: WorkflowRegistryVersionView = {
    workflowVersionId: item.workflowVersionId ?? `${workflowId}:version`,
    workflowId,
    version: item.workflowVersion,
    workflowVersion: item.workflowVersion,
    rawSha256: item.workflowSha256,
    workflowSha256: item.workflowSha256,
    packageName: item.packageName,
    packageStatus: item.packageStatus,
    enabled: item.enabled,
    archived: item.archived,
    archivedAt: item.archivedAt,
    capability: item.capability,
    capabilityIssues: item.capabilityIssues,
    readiness: item.readiness,
    readinessReasons: item.readinessReasons,
    nodeCount: item.nodeCount,
    activeTasks: item.activeTasks,
    totalTasks: item.totalTasks,
    hasSuccessfulRun: item.hasSuccessfulRun,
    latestSuccessAt: item.latestSuccessAt,
    latestFailureAt: item.latestFailureAt,
    liveVerifiedAt: item.liveVerifiedAt,
    diagnostics: item.diagnostics,
    recipes: recipeList,
  };
  return {
    ...item,
    workflowId,
    registryBacked: false,
    sourceKind,
    libraryState: item.archived ? "REMOVED" : "ACTIVE",
    currentVersionId: item.workflowVersionId,
    currentRecipe: recipeList[recipeList.length - 1],
    versions: [version],
    registryRecipes: recipeList,
    projectUsageCount: 0,
    historyCount: item.totalTasks,
  };
}

function registryItems(response: WorkflowRegistryView[] | WorkflowRegistryResponse): WorkflowRegistryView[] {
  return Array.isArray(response) ? response : response.items;
}

export function WorkflowWorkspace({ projectId, catalog, comfyConnected, onCatalogChanged, onOpenStudio, onUseInProject, onOpenTask }: Props) {
  const cachedWorkspace = useWorkflowWorkspaceStore((state) => state.workspace);
  const setCachedWorkspace = useWorkflowWorkspaceStore((state) => state.setWorkspace);
  const [items, setItems] = useState<WorkflowWorkspaceItem[]>(() => (cachedWorkspace?.items ?? []).map(normalizeLegacyItem));
  const [staging, setStaging] = useState<{ stagingId: string; status: string; inUse: boolean }[]>(() => cachedWorkspace?.staging ?? []);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<"all" | "available" | "issues" | "archived">("all");
  const [selectedVersions, setSelectedVersions] = useState<string[]>([]);
  const [diff, setDiff] = useState<WorkflowVersionDiffView>();
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [workspaceError, setWorkspaceError] = useState<string>();
  const [checkingAll, setCheckingAll] = useState(false);
  const [quickTestingId, setQuickTestingId] = useState<string>();
  const [mappingDrafts, setMappingDrafts] = useState<Record<string, MappingDraft>>({});
  const [outputDraft, setOutputDraft] = useState<OutputDraft>(createDefaultOutputDraft);
  const [metadataDraft, setMetadataDraft] = useState<MetadataDraft>();
  const [published, setPublished] = useState<{ workflowId: string; recipeId: string }>();
  const [autoPlan, setAutoPlan] = useState<WorkflowAutoOnboardingPlanView>();
  const [analysisMode, setAnalysisMode] = useState<"V2" | "LEGACY">("V2");
  const [autoImportError, setAutoImportError] = useState<WorkflowImportErrorView>();
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [parameterDraft, setParameterDraft] = useState<WorkflowOnboardingDraftView>();
  const [parameterItem, setParameterItem] = useState<WorkflowProductionWorkspaceView>();
  const [parameterOriginalKeys, setParameterOriginalKeys] = useState<string[]>([]);
  const [parameterLoading, setParameterLoading] = useState(false);
  const [deletionTarget, setDeletionTarget] = useState<WorkflowDeletionTarget>();
  const [renameTarget, setRenameTarget] = useState<WorkflowWorkspaceItem>();
  const [renameValue, setRenameValue] = useState("");
  const [deleting, setDeleting] = useState(false);
  const draft = useWorkflowOnboardingStore((state) => state.draft);
  const step = useWorkflowOnboardingStore((state) => state.step);
  const loading = useWorkflowOnboardingStore((state) => state.loading);
  const error = useWorkflowOnboardingStore((state) => state.error);
  const notice = useWorkflowOnboardingStore((state) => state.notice);
  const setDraft = useWorkflowOnboardingStore((state) => state.setDraft);
  const updateDraft = useWorkflowOnboardingStore((state) => state.updateDraft);
  const setStep = useWorkflowOnboardingStore((state) => state.setStep);
  const setLoading = useWorkflowOnboardingStore((state) => state.setLoading);
  const setError = useWorkflowOnboardingStore((state) => state.setError);
  const setNotice = useWorkflowOnboardingStore((state) => state.setNotice);
  const reset = useWorkflowOnboardingStore((state) => state.reset);
  const importBusyRef = useRef(false);

  const loadWorkspace = useCallback(async (mode: "fast" | "refresh" = "fast") => {
    const cached = useWorkflowWorkspaceStore.getState().workspace;
    const hasCachedWorkspace = Boolean(cached);
    if (cached) {
      setItems(cached.items.map(normalizeLegacyItem));
      setStaging(cached.staging);
    }
    setWorkspaceLoading(mode === "refresh" || !hasCachedWorkspace);
    setWorkspaceError(undefined);
    try {
      try {
        const registry = await listWorkflowRegistry();
        const nextItems = registryItems(registry).map(normalizeRegistryItem);
        const nextStaging = Array.isArray(registry) ? [] : (registry.staging ?? []).map((entry) => ({ ...entry }));
        setItems(nextItems);
        setStaging(nextStaging);
        setCachedWorkspace({ items: nextItems, staging: nextStaging });
        return;
      } catch {
        // DEV-083 keeps the package workspace as a compatibility fallback until the V2 command is installed.
      }
      const workspace = mode === "refresh"
        ? await refreshWorkflowProductionWorkspace()
        : await listWorkflowProductionWorkspace();
      const nextItems = workspace.items.map(normalizeLegacyItem);
      setItems(nextItems);
      setStaging(workspace.staging);
      setCachedWorkspace({ items: nextItems, staging: workspace.staging });
    } catch (loadError: unknown) {
      if (!hasCachedWorkspace) {
        setWorkspaceError(toUserMessage(loadError));
      }
    } finally {
      setWorkspaceLoading(false);
    }
  }, [setCachedWorkspace]);

  useEffect(() => {
    void loadWorkspace("fast");
  }, [loadWorkspace]);

  useEffect(() => {
    if (!draft) {
      setMetadataDraft(undefined);
      return;
    }
    setMetadataDraft({
      workflowId: draft.manifest.workflowId,
      name: draft.manifest.name,
      workflowVersion: draft.manifest.workflowVersion,
      recipeVersion: draft.manifest.recipeVersion,
      category: draft.manifest.category,
      mode: draft.manifest.mode,
    });
    const firstOutputNode = draft.nodes.find((node) => node.isOutputNode) ?? draft.nodes[0];
    setOutputDraft((current) => ({
      ...current,
      nodeId: firstOutputNode?.nodeId ?? "",
    }));
    setPublished(undefined);
  }, [draft?.draftId]);

  async function discardReplacedDraft(previousDraftId: string | undefined, nextDraftId?: string) {
    if (!previousDraftId || previousDraftId === nextDraftId) return;
    try {
      await discardOnboarding(previousDraftId);
    } catch {
      // A published or already discarded draft is safe to replace locally.
    }
  }

  function resetImportViewForNewWorkflow() {
    reset();
    setLoading(true);
    setAutoPlan(undefined);
    setAnalysisMode("V2");
    setAutoImportError(undefined);
    setShowAdvanced(false);
    setPublished(undefined);
  }

  async function returnToWorkflowList() {
    if (importBusyRef.current) return;
    const draftId = draft?.draftId;
    if (draftId) {
      setLoading(true);
      try {
        await discardOnboarding(draftId);
      } catch {
        // Closing remains safe when the native service already consumed the draft.
      } finally {
        setLoading(false);
      }
    }
    reset();
    setAutoPlan(undefined);
    setAnalysisMode("V2");
    setAutoImportError(undefined);
    setShowAdvanced(false);
    setPublished(undefined);
  }

  async function importWorkflow(existingWorkflowId?: string) {
    if (loading || importBusyRef.current) return;
    const previousDraftId = draft?.draftId;
    importBusyRef.current = true;
    setLoading(true);
    try {
      const imported = await pickApiWorkflow(existingWorkflowId);
      if (imported) {
        await discardReplacedDraft(previousDraftId, imported.draftId);
      resetImportViewForNewWorkflow();
      setAutoPlan(undefined);
      setAnalysisMode("V2");
        setAutoImportError(undefined);
        setShowAdvanced(true);
        setDraft(imported);
        const validation = imported.validation;
        const failedChecks = [
          !validation.apiFormat && "API 格式",
          !validation.recipe && "配方",
          !validation.bindings && "输入映射",
          !validation.outputs && "输出映射",
        ].filter((value): value is string => Boolean(value));
        setNotice(failedChecks.length
          ? `导入质量初检完成：${imported.nodeCount} 个节点；待处理：${failedChecks.join("、")}。`
          : `导入质量初检通过：${imported.nodeCount} 个节点、${imported.uniqueClassCount} 种节点类型；请继续完成能力检查与试运行。`,
        );
        await loadWorkspace("refresh");
      } else {
        await discardReplacedDraft(previousDraftId);
        resetImportViewForNewWorkflow();
      }
    } catch (importError: unknown) {
      await discardReplacedDraft(previousDraftId);
      reset();
      setPublished(undefined);
      setAutoPlan(undefined);
      setAnalysisMode("V2");
      setAutoImportError(undefined);
      setShowAdvanced(false);
      setError(toUserMessage(importError));
    } finally {
      setLoading(false);
      importBusyRef.current = false;
    }
  }

  async function smartImportWorkflow(existingWorkflowId?: string) {
    if (loading || importBusyRef.current) return;
    const previousDraftId = draft?.draftId;
    importBusyRef.current = true;
    setLoading(true);
    try {
      const plan = await analyzeWorkflowImport(existingWorkflowId);
      if (plan) {
        await discardReplacedDraft(previousDraftId, plan.draftId);
        resetImportViewForNewWorkflow();
        setAutoPlan(plan);
        setAnalysisMode("V2");
        setShowAdvanced(false);
        const detectedFormat = workflowImportFormat(plan);
        if (plan.draftId && (!detectedFormat || detectedFormat === "API")) {
          try {
            const importedDraft = await getOnboardingDraft(plan.draftId);
            setDraft(importedDraft);
          } catch (draftError) {
            if (!plan.published) throw draftError;
          }
        } else if (plan.draftId && detectedFormat && detectedFormat !== "API") {
          await discardOnboarding(plan.draftId).catch(() => undefined);
        }
        if (plan.published) {
          setNotice("工作流已添加到列表，可在项目设置中选择。");
          await loadWorkspace("refresh");
          await onCatalogChanged();
        } else {
          setNotice(undefined);
        }
      } else {
        await discardReplacedDraft(previousDraftId);
        resetImportViewForNewWorkflow();
      }
    } catch (importError: unknown) {
      await discardReplacedDraft(previousDraftId);
      reset();
      setPublished(undefined);
      setAutoPlan(undefined);
      setAnalysisMode("V2");
      setShowAdvanced(false);
      setError(undefined);
      setAutoImportError(workflowImportErrorView(importError));
    } finally {
      setLoading(false);
      importBusyRef.current = false;
    }
  }

  async function importBackup() {
    setWorkspaceError(undefined);
    try {
      const restored = await importWorkflowPackageBackup();
      if (restored) {
        setNotice(`工作流备份已恢复：${restored.workflowVersion}`);
        await loadWorkspace("refresh");
        await onCatalogChanged();
      }
    } catch (importError: unknown) {
      setWorkspaceError(toUserMessage(importError));
    }
  }

  async function toggleVersion(item: WorkflowWorkspaceItem) {
    if (!item.workflowVersionId || item.archived) return;
    try {
      await setWorkflowEnabled(item.workflowVersionId, !item.enabled);
      await loadWorkspace("fast");
      await onCatalogChanged();
      setNotice(`${item.name ?? item.packageName} 已${item.enabled ? "停用" : "启用"}。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  function registryDeletionInspection(item: WorkflowWorkspaceItem): WorkflowDeletionInspection {
    const activeTaskCount = item.activeTasks ?? 0;
    const activeQueueItemCount = item.versions.reduce((total, version) => total + (version.activeQueueItemCount ?? 0), 0);
    return {
      workflowId: item.workflowId ?? item.packageName,
      workflowVersionId: item.workflowVersionId ?? "",
      name: item.name ?? item.packageName,
      builtin: item.sourceKind === "PRODUCT",
      enabled: item.enabled,
      archived: item.archived,
      archivedAt: item.archivedAt,
      activeTaskCount,
      activeQueueItemCount,
      historicalTaskCount: item.historyCount,
      productionBatchItemCount: 0,
      benchmarkReferenceCount: 0,
      projectBindingCount: item.projectUsageCount,
      canHardDelete: false,
      requiresArchive: true,
      blockingReasons: [
        activeTaskCount > 0 && `有 ${activeTaskCount} 个活动任务`,
        activeQueueItemCount > 0 && `有 ${activeQueueItemCount} 个活动队列项目`,
      ].filter((reason): reason is string => Boolean(reason)),
      deleteAction: activeTaskCount > 0 || activeQueueItemCount > 0 ? "BLOCKED" : "REMOVE",
      sourceKind: item.sourceKind,
      libraryState: item.libraryState,
      historyCount: item.historyCount,
    };
  }

  async function inspectForDeletion(item: WorkflowWorkspaceItem) {
    if (!item.workflowId && !item.workflowVersionId) return;
    setWorkspaceError(undefined);
    try {
      const inspection = item.registryBacked
        ? registryDeletionInspection(item)
        : await inspectWorkflowDeletion(item.workflowVersionId!);
      setDeletionTarget({ item, inspection, mode: "REMOVE" });
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function inspectForPurge(item: WorkflowWorkspaceItem) {
    if (!item.registryBacked || item.libraryState !== "REMOVED" || item.sourceKind !== "USER") return;
    const inspection = registryDeletionInspection(item);
    if (inspection.activeTaskCount || inspection.activeQueueItemCount || item.historyCount || item.projectUsageCount) {
      setWorkspaceError("该用户工作流仍有活动任务、历史或项目引用，不能彻底删除。");
      return;
    }
    setDeletionTarget({ item, inspection, mode: "PURGE" });
  }

  async function confirmWorkflowDeletion() {
    if (!deletionTarget || deleting) return;
    setDeleting(true);
    setWorkspaceError(undefined);
    try {
      const { item, mode, inspection } = deletionTarget;
      let result: Array<WorkflowDeletionResult | { projectBindingCount?: number; action?: string }>;
      if (mode === "PURGE" && item.workflowId) {
        result = [await purgeWorkflow(item.workflowId)];
      } else if (item.registryBacked && item.workflowId) {
        result = [await removeWorkflow(item.workflowId)];
      } else if (item.workflowId) {
        try {
          result = await deleteWorkflow(item.workflowId);
        } catch {
          result = item.workflowVersionId ? [await deleteWorkflowVersion(item.workflowVersionId)] : [];
        }
      } else {
        result = [];
      }
      setDeletionTarget(undefined);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      const projectBindingCount = result.reduce((total, entry) => total + (entry.projectBindingCount ?? 0), 0);
      const hasHistory = inspection.historicalTaskCount > 0
        || inspection.productionBatchItemCount > 0
        || inspection.benchmarkReferenceCount > 0;
      const permanentlyDeleted = mode === "PURGE";
      const message = [
        `${inspection.name} 已删除。`,
        projectBindingCount > 0 && `已解除 ${projectBindingCount} 项项目工作流配置。`,
        hasHistory && "历史生产记录仍然保留。",
        permanentlyDeleted ? "工作流已永久删除。" : "已从工作流库移除，可在“已删除”中恢复。",
      ].filter((part): part is string => Boolean(part)).join(" ");
      setNotice(message);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setDeleting(false);
    }
  }

  async function restoreArchivedWorkflow(item: WorkflowWorkspaceItem) {
    if ((!item.workflowVersionId && !item.workflowId) || !item.archived) return;
    try {
      const result = item.registryBacked && item.workflowId
        ? await restoreWorkflow(item.workflowId)
        : await restoreWorkflowVersion(item.workflowVersionId!);
      const name = item.name ?? item.packageName;
      const capability = (result.capability ?? "NOT_CHECKED").toUpperCase();
      const message = result.enabled !== false && capability === "READY"
        ? `${name} 已恢复并重新启用，现在可以正常使用。`
        : capability === "MISSING_NODES"
          ? `${name} 已恢复，但当前缺少 ComfyUI 节点，暂时保持停用。`
          : capability === "COMFY_OFFLINE"
            ? `${name} 已恢复，但当前 ComfyUI 离线，暂时保持停用。`
            : capability.includes("INCOMPATIBLE")
              ? `${name} 已恢复，但存在兼容性问题，暂时保持停用。`
              : `${name} 已恢复，完成检查后即可启用。`;
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(message);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function restoreExistingArchivedWorkflow() {
    if (!autoPlan?.existingWorkflowId) return;
    const existing = items.find((item) =>
      item.archived
      && item.workflowId === autoPlan.existingWorkflowId
      && (!autoPlan.existingWorkflowVersion || item.workflowVersion === autoPlan.existingWorkflowVersion),
    );
    if (existing) {
      await restoreArchivedWorkflow(existing);
    } else {
      setWorkspaceError("归档版本已经不存在，请重新选择工作流文件。");
    }
  }

  function openRename(item: WorkflowWorkspaceItem) {
    setRenameTarget(item);
    setRenameValue(item.name ?? item.packageName);
  }

  async function saveRename() {
    if (!renameTarget?.workflowId || !renameValue.trim()) return;
    try {
      await renameWorkflow(renameTarget.workflowId, renameValue.trim());
      setRenameTarget(undefined);
      setRenameValue("");
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice("工作流名称已更新；版本、Recipe 和项目绑定保持不变。");
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function setCurrentVersion(item: WorkflowWorkspaceItem, version: WorkflowRegistryVersionView) {
    if (!item.registryBacked || !item.workflowId || !version.workflowVersionId || version.workflowVersionId === item.currentVersionId) return;
    try {
      await setWorkflowCurrentVersion(item.workflowId, version.workflowVersionId);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(`已将版本 ${version.version ?? version.workflowVersion ?? "—"} 设为当前版本；已有项目绑定保持不变。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function recheckVersion(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId) return;
    try {
      const capability = await recheckWorkflowCapability(item.workflowVersionId);
      await loadWorkspace("fast");
      setNotice(`${item.name ?? item.packageName}: ${formatCapability(capability.state)}`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function reidentifyWorkflow(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowId || !item.workflowVersion || item.archived) return;
    await runDraftAction(async () => {
      const nextPlan = await rerecognizeWorkflow(item.workflowId!);
      setAutoPlan(nextPlan);
      setAutoImportError(undefined);
      setShowAdvanced(false);
      setDraft(await getOnboardingDraft(nextPlan.draftId));
      setNotice(nextPlan.message || "已重新识别当前版本；请明确选择添加新 Recipe 后再写入。");
    });
  }

  async function repairBuiltinPackage(item: WorkflowProductionWorkspaceView) {
    if (!item.builtin || !item.diagnostics.some((diagnostic) => diagnostic.code === "BUILTIN_PACKAGE_HASH_MISMATCH")) return;
    try {
      await repairBuiltinWorkflowPackage(item.packageName);
      await loadWorkspace("refresh");
      setNotice(`${item.packageName}: 已隔离不一致文件并恢复 immutable 内置包。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function recheckAllVersions() {
    setCheckingAll(true);
    setWorkspaceError(undefined);
    try {
      const checked = await recheckAllWorkflowCapabilities();
      await loadWorkspace("fast");
      setNotice(`已完成全部工作流兼容性检查，共 ${checked.length} 项；本次只请求一次 ComfyUI 能力信息。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setCheckingAll(false);
    }
  }

  async function duplicateRecipe(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId) return;
    try {
      const duplicated = await duplicateWorkflowRecipe(item.workflowVersionId, item.recipes[item.recipes.length - 1]?.recipeId);
      setAutoPlan(undefined);
      setShowAdvanced(true);
      setDraft(duplicated);
      setStep("inputs");
      setNotice("配方已复制，请检查映射并发布新的配方版本。");
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function openParameterExposure(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId || item.archived) return;
    setParameterLoading(true);
    setWorkspaceError(undefined);
    setShowAdvanced(false);
    try {
      const sourceRecipe = item.recipes[item.recipes.length - 1];
      const duplicated = await duplicateWorkflowRecipe(item.workflowVersionId, sourceRecipe?.recipeId);
      setParameterItem(item);
      setParameterDraft(duplicated);
      setParameterOriginalKeys(duplicated.inputMappings.map((mapping) => mapping.semanticKey));
      try {
        await checkOnboardingCapability(duplicated.draftId);
        setParameterDraft(await getOnboardingDraft(duplicated.draftId));
      } catch (capabilityError: unknown) {
        setWorkspaceError(`参数节点已加载，但暂时无法读取 ComfyUI /object_info：${toUserMessage(capabilityError)}`);
      }
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setParameterLoading(false);
    }
  }

  async function closeParameterExposure() {
    if (parameterDraft) {
      try {
        await discardOnboarding(parameterDraft.draftId);
      } catch {
        // The draft may already have been consumed by publish; closing remains safe.
      }
    }
    setParameterDraft(undefined);
    setParameterItem(undefined);
    setParameterOriginalKeys([]);
  }

  async function refreshParameterCapability() {
    if (!parameterDraft) return;
    setParameterLoading(true);
    setWorkspaceError(undefined);
    try {
      await checkOnboardingCapability(parameterDraft.draftId);
      setParameterDraft(await getOnboardingDraft(parameterDraft.draftId));
      setNotice("已刷新参数建议与 ComfyUI 输入范围。");
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setParameterLoading(false);
    }
  }

  async function exposeParameter(nodeId: string, input: WorkflowInputView) {
    if (!parameterDraft || !isExposableWorkflowInput(input)) return;
    const fieldType = supportedParameterFieldType(input);
    if (!fieldType) return;
    const mapping = defaultMapping(nodeId, input);
    setParameterLoading(true);
    setWorkspaceError(undefined);
    try {
      setParameterDraft(await setOnboardingInputMapping(parameterDraft.draftId, {
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
      setNotice(`${mapping.label} 已加入新配方草稿。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setParameterLoading(false);
    }
  }

  async function saveParameterMapping(mapping: MappingDraft, targetNode: string, targetInput: string) {
    if (!parameterDraft) return;
    setParameterLoading(true);
    setWorkspaceError(undefined);
    try {
      setParameterDraft(await setOnboardingInputMapping(parameterDraft.draftId, {
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
      setNotice("生产参数字段已保存到新配方草稿。");
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setParameterLoading(false);
    }
  }

  async function removeParameterMapping(mapping: WorkflowOnboardingDraftView["inputMappings"][number]) {
    if (!parameterDraft) return;
    setParameterLoading(true);
    try {
      setParameterDraft(await removeOnboardingInputMapping(parameterDraft.draftId, {
        semanticKey: mapping.semanticKey,
        itemIndex: mapping.itemIndex,
      }));
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setParameterLoading(false);
    }
  }

  async function publishParameterRecipe(edits: ParameterMappingEdit[] = []) {
    if (!parameterDraft) return;
    setParameterLoading(true);
    setWorkspaceError(undefined);
    try {
      let currentDraft = parameterDraft;
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
      setParameterDraft(currentDraft);
      const validation = await validateOnboarding(currentDraft.draftId);
      setParameterDraft((current) => current ? { ...current, validation } : current);
      if (!validation.readyToPublish) {
        setWorkspaceError(validation.issues.map(localizeWorkflowIssue).join("；"));
        return;
      }
      const result = await publishOnboarding(currentDraft.draftId);
      const publishedDraftId = currentDraft.draftId;
      try {
        await discardOnboarding(publishedDraftId);
      } catch {
        // Publishing has already committed the immutable package.
      }
      setParameterDraft(undefined);
      setParameterItem(undefined);
      setParameterOriginalKeys([]);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(`已保存为配方 ${result.recipeId}；工作流版本保持不变。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setParameterLoading(false);
    }
  }

  async function quickTest(item: WorkflowProductionWorkspaceView) {
    if (!projectId || !item.workflowVersionId) return;
    const latestRecipeId = item.recipes[item.recipes.length - 1]?.recipeId;
    const recipe = catalog.find((candidate) =>
      candidate.workflowVersionId === item.workflowVersionId && candidate.recipeId === latestRecipeId,
    );
    if (!recipe) {
      setWorkspaceError("当前配方尚未进入创作目录，请先刷新工作流。");
      return;
    }
    if (!comfyConnected) {
      setWorkspaceError("请先连接 ComfyUI，再执行快速测试。");
      return;
    }
    const values = quickTestValues(recipe);
    if (!values) {
      setNotice("该工作流需要图片、视频或音频素材，请打开创作页补充最低必需输入。");
      await onOpenStudio(recipe.workflowId, recipe.recipeId);
      return;
    }
    setQuickTestingId(item.workflowVersionId);
    setWorkspaceError(undefined);
    try {
      const task = await createGeneration({
        projectId,
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        values,
      });
      setNotice(`快速测试任务已创建：${task.id}`);
      onOpenTask?.(task.id);
    } catch (testError: unknown) {
      setWorkspaceError(toUserMessage(testError));
    } finally {
      setQuickTestingId(undefined);
    }
  }

  async function compareSelected() {
    if (selectedVersions.length !== 2) return;
    try {
      setDiff(await compareWorkflowVersions(selectedVersions[0], selectedVersions[1]));
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  function toggleSelected(item: WorkflowProductionWorkspaceView) {
    if (!item.workflowVersionId) return;
    setSelectedVersions((current) => current.includes(item.workflowVersionId!)
      ? current.filter((id) => id !== item.workflowVersionId)
      : current.length < 2 ? [...current, item.workflowVersionId!] : [current[1], item.workflowVersionId!]);
  }

  const visibleItems = items.filter((item) => {
    const matchesSearch = !search.trim() || (item.name ?? item.packageName).toLowerCase().includes(search.trim().toLowerCase());
    const removed = item.libraryState === "REMOVED" || item.archived;
    const needsAction = !removed && (
      !item.enabled
      || item.packageStatus !== "VALID"
      || item.capability !== "READY"
      || item.readiness !== "READY"
      || item.diagnostics.length > 0
    );
    const matchesFilter = filter === "archived"
      ? removed
      : filter === "available"
        ? !removed && !needsAction
        : filter === "issues"
          ? !removed && needsAction
          : !removed;
    return matchesSearch && matchesFilter;
  });

  async function checkCapability() {
    if (!draft) return;
    await runDraftAction(async () => {
      await checkOnboardingCapability(draft.draftId);
      updateDraft(await getOnboardingDraft(draft.draftId));
      setStep("compatibility");
    });
  }

  async function validateDraft() {
    if (!draft) return;
    await runDraftAction(async () => {
      const validation = await validateOnboarding(draft.draftId);
      updateDraft({ ...draft, validation });
      setStep("validate");
    });
  }

  async function publishDraft() {
    if (!draft || !draft.validation.readyToPublish) return;
    await runDraftAction(async () => {
      const result = await publishOnboarding(draft.draftId);
      setPublished({ workflowId: result.workflowId, recipeId: result.recipeId });
      setNotice(`已发布 ${result.packageName}，运行目录已刷新。`);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setStep("publish");
    });
  }

  async function discardDraft() {
    if (!draft) return;
    await runDraftAction(async () => {
      await discardOnboarding(draft.draftId);
      reset();
      setAutoPlan(undefined);
      setShowAdvanced(false);
      setNotice("草稿已丢弃。");
    });
  }

  async function runDraftAction(action: () => Promise<void>) {
    setLoading(true);
    setError(undefined);
    try {
      await action();
    } catch (actionError: unknown) {
      setError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }

  async function saveMetadata() {
    if (!draft || !metadataDraft) return;
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingMetadata(draft.draftId, metadataDraft);
      updateDraft(nextDraft);
      setNotice("基本信息已保存，请重新校验后再发布。");
    });
  }

  async function bindInput(nodeId: string, input: WorkflowInputView) {
    if (!draft || (!input.bindable && (!input.isLinked || !isExposableWorkflowInput(input)))) return;
    const mapping = mappingDrafts[mappingKey(nodeId, input.name)] ?? defaultMapping(nodeId, input);
    const request: WorkflowOnboardingInputMappingRequest = {
      semanticKey: mapping.semanticKey,
      fieldType: mapping.fieldType,
      label: mapping.label,
      required: mapping.required,
      defaultValue: optionalText(mapping.defaultValue),
      minValue: optionalText(mapping.minValue),
      maxValue: optionalText(mapping.maxValue),
      step: optionalText(mapping.step),
      minItems: optionalNumber(mapping.minItems),
      maxItems: optionalNumber(mapping.maxItems),
      itemIndex: optionalNumber(mapping.itemIndex),
      targetNode: nodeId,
      targetInput: input.name,
    };
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingInputMapping(draft.draftId, request);
      updateDraft(nextDraft);
      setNotice(`${mapping.label} 已绑定到 ${nodeId}.${input.name}。`);
    });
  }

  async function removeInput(mapping: WorkflowOnboardingDraftView["inputMappings"][number]) {
    if (!draft) return;
    await runDraftAction(async () => {
      const nextDraft = await removeOnboardingInputMapping(draft.draftId, {
        semanticKey: mapping.semanticKey,
        itemIndex: mapping.itemIndex,
      });
      updateDraft(nextDraft);
    });
  }

  async function addOutput() {
    if (!draft || !outputDraft.nodeId) return;
    const request: WorkflowOnboardingOutputMappingRequest = outputDraft;
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingOutputMapping(draft.draftId, request);
      updateDraft(nextDraft);
      setNotice(`${outputDraft.label} 已设置为${outputDraft.type === "video" ? "视频" : "图片"}输出。`);
    });
  }

  async function resumeAutoImport() {
    if (!autoPlan) return;
    await runDraftAction(async () => {
      if (analysisMode === "V2") {
        const nextPlan = await analyzeWorkflowImport(autoPlan.existingWorkflowId);
        if (!nextPlan) return;
        setAutoPlan(nextPlan);
        setNotice(nextPlan.message || "识别已刷新，请确认后再添加。");
        return;
      }
      const nextPlan = await autoConfirmOnboarding(autoPlan.draftId);
      setAutoPlan(nextPlan);
      setDraft(await getOnboardingDraft(nextPlan.draftId));
      setNotice(nextPlan.message);
      if (nextPlan.published) {
        await loadWorkspace("refresh");
        await onCatalogChanged();
      }
    });
  }

  async function regenerateExistingRecipe() {
    if (!autoPlan?.existingWorkflowId || !autoPlan.existingWorkflowVersion) return;
    await runDraftAction(async () => {
      const nextPlan = autoPlan.draftId
        ? autoPlan
        : await rerecognizeWorkflow(autoPlan.existingWorkflowId!);
      if (nextPlan.draftId !== autoPlan.draftId) {
        setAutoPlan(nextPlan);
      }
      const result = await commitWorkflowImport({
        draftId: nextPlan.draftId,
        action: "NEW_RECIPE",
        workflowId: nextPlan.existingWorkflowId,
        setCurrent: false,
      });
      const workflowId = result.workflowId ?? nextPlan.existingWorkflowId!;
      const recipeId = result.recipeId ?? nextPlan.metadata.recipeId;
      setPublished({ workflowId, recipeId });
      setAutoPlan({
        ...nextPlan,
        state: "AUTO_PUBLISHED",
        commitRequired: false,
        published: result,
      });
      setNotice(`已为现有工作流新增 Recipe ${result.recipeVersion ?? ""}。原工作流版本和旧 Recipe 保持不变。`);
      await loadWorkspace("refresh");
      await onCatalogChanged();
    });
  }

  async function resolveAutoIssue(issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) {
    if (!autoPlan || !draft) return;
    await runDraftAction(async () => {
      if (issue.code === "AMBIGUOUS_OUTPUT" && candidate.nodeId && candidate.outputType) {
        await setOnboardingOutputMapping(autoPlan.draftId, {
          outputId: candidate.outputId ?? "output_1",
          label: candidate.label,
          type: candidate.outputType as "image" | "video",
          nodeId: candidate.nodeId,
          required: true,
        });
      } else if (candidate.nodeId && candidate.inputName) {
        const node = draft.nodes.find((item) => item.nodeId === candidate.nodeId);
        const input = node?.inputs.find((item) => item.name === candidate.inputName);
        if (!input) return;
        const base = defaultMapping(candidate.nodeId, input);
        const fieldType = candidate.fieldType && fieldTypes.includes(candidate.fieldType as WorkflowFieldType)
          ? candidate.fieldType as WorkflowFieldType
          : base.fieldType;
        await setOnboardingInputMapping(autoPlan.draftId, {
          semanticKey: issue.field ?? base.semanticKey,
          fieldType,
          label: base.label,
          required: base.required,
          defaultValue: optionalText(base.defaultValue),
          minValue: optionalText(base.minValue),
          maxValue: optionalText(base.maxValue),
          step: input.numericStep,
          minItems: optionalNumber(base.minItems),
          maxItems: optionalNumber(base.maxItems),
          itemIndex: optionalNumber(base.itemIndex),
          targetNode: candidate.nodeId,
          targetInput: candidate.inputName,
        });
      }
      if (analysisMode === "V2") {
        setNotice("已记录这项选择，请重新分析工作流后再添加。");
        return;
      }
      const nextPlan = await autoConfirmOnboarding(autoPlan.draftId);
      setAutoPlan(nextPlan);
      setDraft(await getOnboardingDraft(nextPlan.draftId));
      setNotice(nextPlan.message);
      if (nextPlan.published) {
        await loadWorkspace("refresh");
        await onCatalogChanged();
      }
    });
  }

  async function commitAnalyzedImport(action: WorkflowImportCommitAction = "NEW_WORKFLOW") {
    if (!autoPlan || !autoPlan.draftId) return;
    await runDraftAction(async () => {
      const result = await commitWorkflowImport({
        draftId: autoPlan.draftId,
        action,
        workflowId: action === "NEW_VERSION" || action === "NEW_RECIPE" ? autoPlan.existingWorkflowId : undefined,
        setCurrent: action === "NEW_VERSION",
      });
      const publishedResult = result as typeof result & { workflowId?: string; recipeId?: string };
      const workflowId = publishedResult.workflowId ?? autoPlan.existingWorkflowId ?? autoPlan.metadata.workflowId;
      const recipeId = publishedResult.recipeId ?? autoPlan.metadata.recipeId;
      setPublished({ workflowId, recipeId });
      setAutoPlan({
        ...autoPlan,
        state: "AUTO_PUBLISHED",
        commitRequired: false,
        published: {
          ...result,
          workflowId,
          recipeId,
          workflowVersion: result.workflowVersion ?? autoPlan.metadata.workflowVersion,
          packageName: result.packageName ?? autoPlan.metadata.name,
          workflowSha256: result.workflowSha256 ?? autoPlan.workflowSha256,
          refreshed: result.refreshed ?? { packagesFound: 0, valid: 0, invalid: 0, inserted: 0, reused: 0, errors: [] },
        },
      });
      setNotice(action === "NEW_VERSION" ? "已添加为新版本；已有项目绑定保持不变。" : "工作流已添加到工作流库。只有明确点击添加后才会写入。" );
      await loadWorkspace("refresh");
      await onCatalogChanged();
    });
  }

  async function openAdvancedImport() {
    if (autoPlan) {
      try {
        setDraft(await getOnboardingDraft(autoPlan.draftId));
      } catch (actionError: unknown) {
        setError(toUserMessage(actionError));
      }
    }
    setShowAdvanced(true);
  }

  async function openStructuralVariantAsVersion() {
    if (!autoPlan?.existingWorkflowId || !autoPlan.existingWorkflowVersion) return;
    await runDraftAction(async () => {
      const currentDraft = draft ?? await getOnboardingDraft(autoPlan.draftId);
      const nextDraft = await setOnboardingMetadata(autoPlan.draftId, {
        workflowId: autoPlan.existingWorkflowId,
        name: currentDraft.manifest.name,
        workflowVersion: nextWorkflowVersion(autoPlan.existingWorkflowVersion!),
        recipeVersion: "1.0.0",
        category: currentDraft.manifest.category,
        mode: currentDraft.manifest.mode,
      });
      setDraft(nextDraft);
      setShowAdvanced(true);
      setNotice("已选择添加为现有工作流的新版本。请在高级编辑中确认映射后发布；旧版本不会被覆盖。");
    });
  }

  async function openExistingWorkflow() {
    if (!autoPlan?.existingWorkflowId) return;
    const existing = items.find((item) =>
      item.workflowId === autoPlan.existingWorkflowId
      && (!autoPlan.existingWorkflowVersion || item.workflowVersion === autoPlan.existingWorkflowVersion),
    );
    const currentVersion = existing?.versions.find((version) => version.workflowVersionId === existing.currentVersionId)
      ?? existing?.versions[0];
    const recipe = existing?.currentRecipe
      ?? currentVersion?.recipes?.[currentVersion.recipes.length - 1]
      ?? existing?.recipes[existing.recipes.length - 1];
    if (!recipe) {
      setNotice("该工作流已经导入，请在工作流列表中查看现有版本。");
      return;
    }
    await onOpenStudio(autoPlan.existingWorkflowId, recipe.recipeId);
  }

  const outputCandidates = useMemo(
    () => draft?.nodes.filter((node) => node.isOutputNode) ?? [],
    [draft],
  );

  return (
    <section className="workspace-panel workflow-workspace" aria-busy={loading || workspaceLoading}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">工作区</span>
          <h2>工作流管理</h2>
          <p className="section-description">导入 ComfyUI API 工作流，配置安全输入后发布工作流运行包。</p>
        </div>
        <div className="workflow-workspace-actions">
          <button type="button" onClick={() => void loadWorkspace("refresh")} disabled={workspaceLoading}>{workspaceLoading ? "正在刷新..." : "刷新"}</button>
          <button type="button" onClick={() => void smartImportWorkflow()} disabled={loading || importBusyRef.current}>+ 添加工作流</button>
          <details className="workflow-advanced-actions">
            <summary>更多</summary>
            <div className="workflow-advanced-actions-content">
              <button type="button" className="quiet-button" onClick={() => void recheckAllVersions()} disabled={checkingAll || workspaceLoading}>{checkingAll ? "检查中..." : "检查全部兼容性"}</button>
              <button type="button" className="quiet-button" onClick={() => void importWorkflow()} disabled={loading || importBusyRef.current}>手动配置工作流</button>
              <button type="button" className="quiet-button" onClick={() => void importBackup()} disabled={loading || importBusyRef.current}>导入工作流备份</button>
            </div>
          </details>
        </div>
      </div>

      <section className="workflow-import-quality" aria-label="工作流导入质量门">
        <div>
          <span className="section-label">导入质量门</span>
          <strong>选择 JSON → 自动识别 → 检查环境 → 确认添加</strong>
          <p>正常工作流只需一次操作；只有歧义、缺失节点或不兼容字段才会进入问题聚焦。</p>
        </div>
        <ul>
          <li>校验 JSON 根结构、节点类型与输入对象</li>
          <li>已连接 ComfyUI 时自动读取 /object_info</li>
          <li>不会自动提交 GPU 生成任务，快速测试仍由用户主动触发</li>
        </ul>
      </section>

      {workspaceError && <p className="error-message" role="alert">{workspaceError}</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="workflow-notice" role="status">{notice}</p>}

      <WorkflowSmartImport
        plan={autoPlan}
        importError={autoImportError}
        draft={draft}
        projectId={projectId}
        loading={loading}
        onResolve={(issue, candidate) => void resolveAutoIssue(issue, candidate)}
        onResume={() => void resumeAutoImport()}
        onOpenAdvanced={() => void openAdvancedImport()}
        onOpenExisting={() => void openExistingWorkflow()}
        onOpenExistingVersion={() => void openStructuralVariantAsVersion()}
        onUseInProject={(workflowId, recipeId) => void onUseInProject(workflowId, recipeId)}
        onRegenerateRecipe={() => void regenerateExistingRecipe()}
        onRestoreExisting={() => void restoreExistingArchivedWorkflow()}
        onCommitImport={(action) => void commitAnalyzedImport(action)}
        onOpenStudio={(workflowId, recipeId) => void onOpenStudio(workflowId, recipeId)}
        onRetry={() => void smartImportWorkflow()}
        onCancel={() => void returnToWorkflowList()}
        onReturnToList={() => void returnToWorkflowList()}
      />

      <section className="workflow-health-dashboard" aria-label="运行环境健康概览">
        <div><span>工作流总数</span><strong>{items.filter((item) => item.libraryState !== "REMOVED" && !item.archived).length}</strong></div>
        <div><span>生产就绪</span><strong>{items.filter((item) => item.libraryState !== "REMOVED" && !item.archived && item.readiness === "READY").length}</strong></div>
        <div><span>待验证</span><strong>{items.filter((item) => item.libraryState !== "REMOVED" && !item.archived && item.readiness === "DEGRADED").length}</strong></div>
        <div><span>阻塞诊断</span><strong>{items.filter((item) => item.libraryState !== "REMOVED" && !item.archived && item.readiness === "BLOCKED").length}</strong></div>
      </section>

      <div className="workflow-production-toolbar">
        <input aria-label="搜索工作流" placeholder="按名称搜索" value={search} onChange={(event) => setSearch(event.target.value)} />
        <select aria-label="工作流筛选" value={filter} onChange={(event) => setFilter(event.target.value as typeof filter)}>
          <option value="all">全部</option><option value="available">可用</option><option value="issues">需处理</option><option value="archived">已删除</option>
        </select>
        <button type="button" onClick={() => void compareSelected()} disabled={selectedVersions.length !== 2}>比较选中的版本</button>
      </div>
      <div className="workflow-catalog" aria-label="工作流运行包">
        <div className="workflow-catalog-header">
          <span>比较</span><span>工作流</span><span>当前版本</span><span>当前 Recipe</span><span>来源</span><span>可用性</span><span>运行记录</span><span>操作</span>
        </div>
        {workspaceLoading && !visibleItems.length && <p className="loading-state">正在读取已注册工作流…</p>}
        {visibleItems.map((item) => {
          const removed = item.libraryState === "REMOVED" || item.archived;
          const currentVersionId = item.currentVersionId ?? item.workflowVersionId;
          const currentRecipe = item.currentRecipe ?? item.recipes[item.recipes.length - 1];
          const versionsForDisplay: WorkflowRegistryVersionView[] = item.versions.length
            ? item.versions
            : currentVersionId
              ? [{ workflowVersionId: currentVersionId, version: item.workflowVersion, workflowId: item.workflowId, recipes: item.recipes }]
              : [];
          const recipesForDisplay = item.registryRecipes.length ? item.registryRecipes : item.recipes;
          const canPurge = item.registryBacked
            && removed
            && item.sourceKind === "USER"
            && item.historyCount === 0
            && item.projectUsageCount === 0
            && item.activeTasks === 0
            && item.versions.every((version) => (version.activeQueueItemCount ?? 0) === 0);
          return (
            <article className="workflow-catalog-row" key={item.workflowId ?? item.packageName}>
              <input type="checkbox" aria-label={`比较 ${workflowDisplayName(item.workflowId, item.name ?? item.packageName)}`} checked={currentVersionId ? selectedVersions.includes(currentVersionId) : false} onChange={() => toggleSelected(item)} disabled={!currentVersionId} />
              <div className="workflow-row-identity">
                <strong>{workflowDisplayName(item.workflowId, item.name ?? item.packageName)}</strong>
                <small>{workflowSourceLabel(item)} · {item.versions.length || 1} 个版本</small>
              </div>
              <span>{item.workflowVersion ?? "—"}{removed ? " · 已删除" : ""}</span>
              <span>{currentRecipe?.version ?? "—"} · {item.recipes.length} 个配方</span>
              <span>{workflowSourceLabel(item)}</span>
              <span className={`workflow-capability workflow-capability-${item.capability.toLowerCase()}`}>{formatCapability(item.capability)}</span>
              <span>{item.hasSuccessfulRun ? "已有成功运行" : `共 ${item.totalTasks} 个任务`}</span>
              <div className="workflow-row-actions">
                {!removed && currentVersionId && (() => {
                  const projectRecipe = latestCatalogRecipeForWorkflowItem(item, catalog);
                  return <button type="button" className="quiet-button" onClick={() => { if (projectRecipe) void onUseInProject(projectRecipe.workflowId, projectRecipe.recipeId); }} disabled={!projectId || !projectRecipe} title={!projectId ? "请先选择或创建一个项目" : !projectRecipe ? "该工作流尚未进入生产目录" : "在当前项目中配置这个工作流"}>用于当前项目</button>;
                })()}
                {currentVersionId && !removed && <button type="button" className="quiet-button" onClick={() => void quickTest(item)} disabled={quickTestingId === currentVersionId}>{quickTestingId === currentVersionId ? "测试中..." : "测试"}</button>}
                {!removed && <button type="button" className="quiet-button danger-button" onClick={() => void inspectForDeletion(item)} title="从工作流库移除这个工作流">删除</button>}
                {removed && <button type="button" className="quiet-button" onClick={() => void restoreArchivedWorkflow(item)}>恢复工作流</button>}
                <details className="workflow-row-menu">
                  <summary aria-label="更多工作流操作">⋯</summary>
                  <div className="workflow-row-menu-content">
                    {!removed && item.registryBacked && <button type="button" className="quiet-button" onClick={() => openRename(item)}>重命名</button>}
                    {!removed && currentVersionId && item.workflowId && item.workflowVersion && <button type="button" className="quiet-button" onClick={() => void reidentifyWorkflow(item)}>重新识别</button>}
                    {!removed && currentVersionId && <button type="button" className="quiet-button" onClick={() => void recheckVersion(item)}>重新检查</button>}
                    {!removed && currentVersionId && <button type="button" className="quiet-button" onClick={() => void duplicateRecipe(item)}>创建新 Recipe 版本</button>}
                    {!removed && currentVersionId && <button type="button" className="quiet-button workflow-parameter-button" onClick={() => void openParameterExposure(item)}>生产参数</button>}
                    {currentVersionId && <button type="button" className="quiet-button" onClick={() => void exportWorkflowPackage(currentVersionId)}>导出工作流</button>}
                    {!removed && currentVersionId && <button type="button" className="quiet-button" onClick={() => void toggleVersion(item)}>{item.enabled ? "停用" : "启用"}</button>}
                    {canPurge && <button type="button" className="quiet-button danger-button" onClick={() => void inspectForPurge(item)}>彻底删除</button>}
                  </div>
                </details>
              </div>
              <details className="workflow-catalog-detail">
                <summary>查看详情</summary>
                <div className="workflow-detail-grid">
                  <span>启用状态 <strong>{removed ? "已删除" : item.enabled ? "已启用" : "已停用"}</strong></span>
                  <span>工作流状态 <strong>{removed ? "已删除" : "活跃"}</strong></span>
                  <span>来源 <strong>{workflowSourceLabel(item)}</strong></span>
                  <span>工作流 SHA-256 <strong>{item.workflowSha256 || "—"}</strong></span>
                  <span>配方 SHA-256 <strong>{item.recipeSha256 ?? "—"}</strong></span>
                  <span>节点数量 <strong>{item.nodeCount}</strong></span>
                  <span>当前版本 <strong>{item.workflowVersion ?? "—"}</strong></span>
                  <span>当前 Recipe <strong>{currentRecipe?.version ?? "—"}</strong></span>
                  <span>项目使用数 <strong>{item.projectUsageCount}</strong></span>
                  <span>历史记录数 <strong>{item.historyCount}</strong></span>
                  <span>活动任务 <strong>{item.activeTasks}</strong></span>
                  <span>任务总数 <strong>{item.totalTasks}</strong></span>
                  <span>最近真实验证 <strong>{item.liveVerifiedAt ? formatDateTime(item.liveVerifiedAt) : "尚未验证"}</strong></span>
                </div>
                {!!item.readinessReasons.length && <ul className="workflow-issue-list">{item.readinessReasons.map((reason) => <li key={reason}>{reason}</li>)}</ul>}
                {!!item.capabilityIssues.length && <section className="workflow-dependency-diagnostics"><strong>依赖诊断 · 来源：ComfyUI /object_info</strong><IssueList issues={item.capabilityIssues} /></section>}
                {!item.capabilityIssues.length && item.packageStatus === "VALID" && <p className="disabled-note">未发现节点依赖问题。模型或文件依赖只有在运行包明确声明并有可验证来源时才会报告，AI Studio 不猜测未声明依赖。</p>}
                {!!item.diagnostics.length && <ul className="workflow-issue-list">{item.diagnostics.map((diagnostic) => <li key={diagnostic.code}>{toUserMessage({ code: diagnostic.code, message: diagnostic.message })}</li>)}</ul>}
                {item.diagnostics.some((diagnostic) => diagnostic.code === "BUILTIN_PACKAGE_HASH_MISMATCH") && <button type="button" className="quiet-button danger-button" onClick={() => void repairBuiltinPackage(item)}>修复内置包哈希</button>}
                <section className="workflow-registry-nested" aria-label="工作流版本">
                  <h4>Versions</h4>
                  {versionsForDisplay.map((version) => (
                    <div className="workflow-registry-version" key={version.workflowVersionId}>
                      <input type="checkbox" aria-label={`比较版本 ${version.version ?? version.workflowVersion ?? "—"}`} checked={selectedVersions.includes(version.workflowVersionId)} onChange={() => setSelectedVersions((current) => current.includes(version.workflowVersionId) ? current.filter((id) => id !== version.workflowVersionId) : current.length < 2 ? [...current, version.workflowVersionId] : [current[1], version.workflowVersionId])} />
                      <strong>{version.version ?? version.workflowVersion ?? "—"}{version.workflowVersionId === currentVersionId ? " · 当前" : ""}</strong>
                      <span>{(version.recipes ?? []).length || (version.workflowVersionId === currentVersionId ? item.recipes.length : 0)} 个 Recipe</span>
                      {item.registryBacked && !removed && version.workflowVersionId !== currentVersionId && <button type="button" className="quiet-button" onClick={() => void setCurrentVersion(item, version)}>设为当前版本</button>}
                    </div>
                  ))}
                </section>
                <section className="workflow-registry-nested" aria-label="工作流 Recipe">
                  <h4>Recipes</h4>
                  <div className="workflow-recipe-summary">
                    {recipesForDisplay.map((recipe) => <span key={`${recipe.workflowVersionId ?? currentVersionId ?? "version"}:${recipe.recipeId}`}>配方 {recipe.version ?? ("recipeVersion" in recipe ? recipe.recipeVersion : undefined) ?? "—"} · {recipe.inputCount ?? 0} 个输入 · {recipe.outputCount ?? 0} 个输出</span>)}
                  </div>
                </section>
              </details>
            </article>
          );
        })}
        {!visibleItems.length && !workspaceLoading && <p className="empty-state">当前筛选条件下没有找到工作流。</p>}
      </div>

      {!!staging.length && <div className="workflow-diagnostics-panel"><h3>运行诊断</h3>{staging.map((entry) => <div key={entry.stagingId}><span>{stagingStatusLabel(entry.status)}</span><code>{entry.stagingId}</code><button type="button" className="quiet-button" onClick={() => void cleanWorkflowStaging(entry.stagingId)} disabled={entry.inUse}>{entry.inUse ? "使用中" : "清理暂存"}</button></div>)}</div>}
      {diff && <VersionDiffPane diff={diff} onClose={() => setDiff(undefined)} />}

      {parameterDraft && parameterItem && (
        <ParameterExposurePane
          draft={parameterDraft}
          workflow={parameterItem}
          originalKeys={parameterOriginalKeys}
          loading={parameterLoading}
          onClose={() => void closeParameterExposure()}
          onRefresh={() => void refreshParameterCapability()}
          onExpose={(nodeId, input) => void exposeParameter(nodeId, input)}
          onSaveMapping={(mapping, nodeId, inputName) => void saveParameterMapping(mapping, nodeId, inputName)}
          onRemove={(mapping) => void removeParameterMapping(mapping)}
          onSave={(edits) => void publishParameterRecipe(edits)}
        />
      )}

      {showAdvanced && draft && (
        <div className="workflow-onboarding-panel">
          <div className="workflow-onboarding-heading">
            <div>
              <span className="section-label">高级工作流编辑</span>
              <h3>{draft.manifest.name}</h3>
              <p className="section-description">{draft.originalFilename} · {draft.nodeCount} 个节点 · {draft.uniqueClassCount} 种节点类型</p>
            </div>
            <div className="workflow-smart-actions">
              <button type="button" className="quiet-button" onClick={() => setShowAdvanced(false)}>返回智能导入</button>
              <button type="button" className="quiet-button" onClick={() => void discardDraft()} disabled={loading}>丢弃草稿</button>
            </div>
          </div>
          <div className="workflow-step-tabs" role="tablist" aria-label="工作流导入步骤">
            {steps.map((item) => (
              <button
                type="button"
                role="tab"
                key={item.value}
                aria-selected={step === item.value}
                className={step === item.value ? "workflow-step-active" : ""}
                onClick={() => setStep(item.value)}
              >
                {item.label}
              </button>
            ))}
          </div>

          {step === "inspect" && <InspectPane draft={draft} onContinue={() => setStep("compatibility")} />}
          {step === "compatibility" && (
            <CompatibilityPane draft={draft} loading={loading} onCheck={() => void checkCapability()} onContinue={() => setStep("inputs")} />
          )}
          {step === "inputs" && (
            <InputsPane
              draft={draft}
              mappingDrafts={mappingDrafts}
              onPatch={(key, patch) => setMappingDrafts((current) => ({ ...current, [key]: { ...(current[key] ?? emptyMapping()), ...patch } }))}
              onBind={(nodeId, input) => void bindInput(nodeId, input)}
              onRemove={(mapping) => void removeInput(mapping)}
              onContinue={() => setStep("outputs")}
            />
          )}
          {step === "outputs" && (
            <OutputsPane
              draft={draft}
              candidates={outputCandidates.length ? outputCandidates : draft.nodes}
              outputDraft={outputDraft}
              onChange={setOutputDraft}
              onAdd={() => void addOutput()}
              onContinue={() => setStep("metadata")}
            />
          )}
          {step === "metadata" && metadataDraft && (
            <MetadataPane draft={metadataDraft} onChange={setMetadataDraft} onSave={() => void saveMetadata()} onContinue={() => setStep("validate")} />
          )}
          {step === "validate" && (
            <ValidatePane draft={draft} loading={loading} onValidate={() => void validateDraft()} onPublish={() => setStep("publish")} />
          )}
          {step === "publish" && (
            <PublishPane
              draft={draft}
              published={published}
              loading={loading}
              onPublish={() => void publishDraft()}
              onOpenStudio={published ? () => void onOpenStudio(published.workflowId, published.recipeId) : undefined}
            />
          )}
        </div>
      )}

      {deletionTarget && (
        <WorkflowDeleteDialog
          item={deletionTarget.item}
          inspection={deletionTarget.inspection}
          deleting={deleting}
          onClose={() => setDeletionTarget(undefined)}
          onConfirm={() => void confirmWorkflowDeletion()}
        />
      )}
      {renameTarget && (
        <div className="workflow-rename-dialog" role="dialog" aria-modal="true" aria-label="重命名工作流">
          <div className="workflow-rename-card">
            <h3>重命名工作流</h3>
            <input aria-label="工作流名称" value={renameValue} onChange={(event) => setRenameValue(event.target.value)} autoFocus />
            <div className="workflow-smart-actions">
              <button type="button" className="quiet-button" onClick={() => setRenameTarget(undefined)}>取消</button>
              <button type="button" onClick={() => void saveRename()} disabled={!renameValue.trim()}>保存</button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

interface ParameterExposurePaneProps {
  draft: WorkflowOnboardingDraftView;
  workflow: WorkflowProductionWorkspaceView;
  originalKeys: string[];
  loading: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onExpose: (nodeId: string, input: WorkflowInputView) => void;
  onSaveMapping: (mapping: MappingDraft, nodeId: string, inputName: string) => void;
  onRemove: (mapping: WorkflowOnboardingDraftView["inputMappings"][number]) => void;
  onSave: (edits: ParameterMappingEdit[]) => void;
}

function ParameterExposurePane({
  draft,
  workflow,
  originalKeys,
  loading,
  onClose,
  onRefresh,
  onExpose,
  onSaveMapping,
  onRemove,
  onSave,
}: ParameterExposurePaneProps) {
  const [search, setSearch] = useState("");
  const [edits, setEdits] = useState<Record<string, MappingDraft>>({});

  useEffect(() => {
    setEdits(Object.fromEntries(draft.inputMappings.map((mapping) => [
      mappingKey(mapping.targetNode, mapping.targetInput),
      mappingToDraft(mapping),
    ])));
  }, [draft.inputMappings]);

  const mappingsByTarget = useMemo(
    () => new Set(draft.inputMappings.map((mapping) => mappingKey(mapping.targetNode, mapping.targetInput))),
    [draft.inputMappings],
  );
  const matches = (node: WorkflowNodeView, input: WorkflowInputView) => {
    const needle = search.trim().toLowerCase();
    if (!needle) return true;
    return `${node.nodeId} ${node.classType} ${node.title} ${input.name} ${input.suggestedSemanticKey ?? ""}`.toLowerCase().includes(needle);
  };
  const candidates = draft.nodes.flatMap((node) => node.inputs
    .filter((input) => matches(node, input)
      && !mappingsByTarget.has(mappingKey(node.nodeId, input.name))
      && isExposableWorkflowInput(input))
    .map((input) => ({ node, input })));
  const internal = draft.nodes.flatMap((node) => node.inputs
    .filter((input) => matches(node, input)
      && !mappingsByTarget.has(mappingKey(node.nodeId, input.name))
      && !isExposableWorkflowInput(input))
    .map((input) => ({ node, input })));
  const newKeys = new Set(draft.inputMappings.map((mapping) => mapping.semanticKey));
  const addedKeys = draft.inputMappings.map((mapping) => mapping.semanticKey).filter((key) => !originalKeys.includes(key));
  const removedKeys = originalKeys.filter((key) => !newKeys.has(key));
  const currentRecipe = workflow.recipes[workflow.recipes.length - 1];

  function patchMapping(mapping: WorkflowOnboardingDraftView["inputMappings"][number], patch: Partial<MappingDraft>) {
    const key = mappingKey(mapping.targetNode, mapping.targetInput);
    setEdits((current) => ({ ...current, [key]: { ...(current[key] ?? mappingToDraft(mapping)), ...patch } }));
  }

  return (
    <section className="workflow-parameter-exposure" aria-label="工作流生产参数">
      <header className="workflow-parameter-header">
        <div>
          <span className="section-label">工作流参数暴露</span>
          <h3>{workflow.name ?? workflow.packageName}</h3>
          <p className="section-description">只修改配方参数暴露，不修改工作流 API 图结构；内置运行包也只会复制为新配方。</p>
        </div>
        <div className="workflow-smart-actions">
          <button type="button" className="quiet-button" onClick={onRefresh} disabled={loading}>{loading ? "读取中..." : "刷新 ComfyUI 参数"}</button>
          <button type="button" className="quiet-button" onClick={onClose} disabled={loading}>取消</button>
          <button type="button" onClick={() => onSave(draft.inputMappings.flatMap((mapping) => {
            const edit = edits[mappingKey(mapping.targetNode, mapping.targetInput)];
            return edit ? [{ mapping, draft: edit }] : [];
        }))} disabled={loading}>{loading ? "保存中..." : "保存为新配方"}</button>
        </div>
      </header>

      <div className="workflow-parameter-summary">
        <span>工作流版本<strong>{workflow.workflowVersion ?? "—"}</strong></span>
        <span>当前配方<strong>{currentRecipe?.version ?? "—"} · {currentRecipe?.inputCount ?? 0} 项</strong></span>
        <span>新配方<strong>{draft.manifest.recipeVersion} · {draft.inputMappings.length} 项</strong></span>
        <span>图结构 SHA-256<strong>{draft.workflowSha256.slice(0, 16)}…</strong></span>
      </div>

      <div className="workflow-parameter-preview">
        <span>保存预览</span>
        <strong>{draft.inputMappings.length} 个生产参数</strong>
        <small>新增：{addedKeys.length ? addedKeys.join("、") : "无"}</small>
        <small>删除：{removedKeys.length ? removedKeys.join("、") : "无"}</small>
        <small>发布后直接使用现有预设 / 默认预设系统；旧配方的预设保持原作用域，不自动改写。</small>
      </div>

      <label className="workflow-parameter-search">搜索节点 / 输入 / 语义键
        <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="例如 steps、KSampler、节点 88" />
      </label>

      <section className="workflow-parameter-section">
        <div className="workflow-parameter-section-heading"><div><span className="section-label">已暴露参数</span><h4>{draft.inputMappings.length} 项</h4></div><small>修改只作用于新配方草稿</small></div>
        {draft.inputMappings.length ? draft.inputMappings.map((mapping) => {
          const key = mappingKey(mapping.targetNode, mapping.targetInput);
          const edit = edits[key] ?? mappingToDraft(mapping);
          return (
            <div className="workflow-parameter-field" key={`${mapping.semanticKey}:${mapping.itemIndex ?? ""}`}>
              <div className="workflow-parameter-field-heading"><div><strong>{mapping.label}</strong><code>{mapping.semanticKey}</code></div><span>节点 {mapping.targetNode} · {mapping.targetInput}</span><button type="button" className="quiet-button danger-button" onClick={() => onRemove(mapping)} disabled={loading}>移除</button></div>
              <div className="workflow-parameter-form">
                <label>显示名称<input value={edit.label} onChange={(event) => patchMapping(mapping, { label: event.target.value })} /></label>
                <label>语义键<input value={edit.semanticKey} onChange={(event) => patchMapping(mapping, { semanticKey: event.target.value })} /></label>
                <label>类型<select value={edit.fieldType} onChange={(event) => patchMapping(mapping, { fieldType: event.target.value as WorkflowFieldType })}>{fieldTypes.map((type) => <option key={type} value={type}>{fieldTypeLabel(type)}</option>)}</select></label>
                <label className="checkbox-label"><input type="checkbox" checked={edit.required} onChange={(event) => patchMapping(mapping, { required: event.target.checked })} /> 必填</label>
                {(edit.fieldType === "textarea" || edit.fieldType === "integer" || edit.fieldType === "number" || edit.fieldType === "seed") && <label>默认值<input value={edit.defaultValue} onChange={(event) => patchMapping(mapping, { defaultValue: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : undefined} /></label>}
                {(edit.fieldType === "integer" || edit.fieldType === "number" || edit.fieldType === "seed") && <>
                  <label>最小值<input value={edit.minValue} onChange={(event) => patchMapping(mapping, { minValue: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : "numeric"} /></label>
                  <label>最大值<input value={edit.maxValue} onChange={(event) => patchMapping(mapping, { maxValue: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : "numeric"} /></label>
                </>}
                {(edit.fieldType === "integer" || edit.fieldType === "number") && <label>步长<input value={edit.step} onChange={(event) => patchMapping(mapping, { step: event.target.value })} inputMode={edit.fieldType === "number" ? "decimal" : "numeric"} /></label>}
                {edit.fieldType.endsWith("s") && <label>最大数量<input value={edit.maxItems} onChange={(event) => patchMapping(mapping, { maxItems: event.target.value })} inputMode="numeric" /></label>}
                <button type="button" onClick={() => onSaveMapping(edit, mapping.targetNode, mapping.targetInput)} disabled={loading}>保存字段</button>
              </div>
            </div>
          );
        }) : <p className="disabled-note">当前配方没有可编辑的输入映射。</p>}
      </section>

      <section className="workflow-parameter-section">
        <div className="workflow-parameter-section-heading"><div><span className="section-label">可暴露参数</span><h4>{candidates.length} 项匹配</h4></div><small>仅显示字面量、可绑定且属于现有安全字段类型的输入</small></div>
        {candidates.map(({ node, input }) => {
          const fieldType = supportedParameterFieldType(input);
          return <div className="workflow-parameter-candidate" key={`${node.nodeId}:${input.name}`}><div><strong>节点 {node.nodeId} · {node.classType}</strong><span>{input.name} · 当前值 {input.currentValueSummary}</span><small>建议：{input.suggestedSemanticKey ?? "—"} · {fieldType ? fieldTypeLabel(fieldType) : "未支持"}</small></div><button type="button" onClick={() => onExpose(node.nodeId, input)} disabled={loading || !fieldType}>暴露</button></div>;
        })}
        {!candidates.length && <p className="disabled-note">没有匹配的可暴露字面量输入。</p>}
      </section>

      <details className="workflow-parameter-section workflow-parameter-internal">
        <summary><span className="section-label">工作流内部参数</span><strong>{internal.length} 项</strong></summary>
        <p className="disabled-note">链接输入、危险路径/模型/设备输入和暂不支持的字段类型保持内部状态，不会写入配方。</p>
        {internal.map(({ node, input }) => <div className="workflow-parameter-internal-row" key={`${node.nodeId}:${input.name}`}><span>节点 {node.nodeId} · {node.classType}</span><strong>{input.name}</strong><small>{input.isLinked ? "内部连接 · 不可作为生产参数" : isDangerousParameterName(input.name) ? "危险输入 · 后端禁止暴露" : "当前字段类型暂不支持"}</small></div>)}
      </details>
    </section>
  );
}

function mappingToDraft(mapping: WorkflowOnboardingDraftView["inputMappings"][number]): MappingDraft {
  return {
    semanticKey: mapping.semanticKey,
    fieldType: mapping.fieldType,
    label: mapping.label,
    required: mapping.required,
    defaultValue: mapping.defaultValue ?? "",
    minValue: mapping.minValue ?? "",
    maxValue: mapping.maxValue ?? "",
    minItems: mapping.minItems?.toString() ?? "",
    maxItems: mapping.maxItems?.toString() ?? "",
    itemIndex: mapping.itemIndex?.toString() ?? "",
    step: mapping.step ?? "",
  };
}

function InspectPane({ draft, onContinue }: { draft: WorkflowOnboardingDraftView; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-stats"><span>SHA-256 <strong>{draft.workflowSha256}</strong></span><span>节点数量 <strong>{draft.nodeCount}</strong></span><span>节点类型数量 <strong>{draft.uniqueClassCount}</strong></span></div>
      <p className="section-description">节点 ID、节点类型和映射仅在此导入向导中作为技术信息显示。</p>
      <div className="workflow-node-list">
        {draft.nodes.map((node) => <NodeCard key={node.nodeId} node={node} />)}
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>检查兼容性</button></div>
    </div>
  );
}

function NodeCard({ node }: { node: WorkflowNodeView }) {
  return (
    <details className="workflow-node-card">
      <summary><strong>节点 {node.nodeId}</strong><span>{node.title}</span><code>{node.classType}</code></summary>
      <div className="workflow-node-inputs">
        {node.inputs.map((input) => <div key={input.name}><span>{input.name}</span><small>{input.currentValueSummary}{input.isLinked ? " · 已连接" : ""}</small></div>)}
        {!node.inputs.length && <small>暂无字面量输入。</small>}
      </div>
    </details>
  );
}

function CompatibilityPane({ draft, loading, onCheck, onContinue }: { draft: WorkflowOnboardingDraftView; loading: boolean; onCheck: () => void; onContinue: () => void }) {
  const capability = draft.capability;
  return (
    <div className="workflow-onboarding-pane">
      <div className={`workflow-capability-banner workflow-capability-${capability.state.toLowerCase()}`}>
        <strong>{formatCapability(capability.state)}</strong>
        <span>{capability.checkedAt ? `检查时间：${formatDateTime(capability.checkedAt)}` : "尚未检查兼容状态。"}</span>
      </div>
      <button type="button" onClick={onCheck} disabled={loading}>{loading ? "正在检查..." : "检查 ComfyUI 兼容状态"}</button>
      {!!capability.issues.length && <IssueList issues={capability.issues} />}
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>配置输入</button></div>
    </div>
  );
}

function InputsPane({
  draft,
  mappingDrafts,
  onPatch,
  onBind,
  onRemove,
  onContinue,
}: {
  draft: WorkflowOnboardingDraftView;
  mappingDrafts: Record<string, MappingDraft>;
  onPatch: (key: string, patch: Partial<MappingDraft>) => void;
  onBind: (nodeId: string, input: WorkflowInputView) => void;
  onRemove: (mapping: WorkflowOnboardingDraftView["inputMappings"][number]) => void;
  onContinue: () => void;
}) {
  return (
    <div className="workflow-onboarding-pane">
      <p className="section-description">选择语义字段并确认每项映射。已连接的输入默认保持原图连接，也可以手动映射并在执行时覆盖。</p>
      <div className="workflow-input-list">
        {draft.nodes.flatMap((node) => node.inputs.map((input) => {
          const key = mappingKey(node.nodeId, input.name);
          const mapping = mappingDrafts[key] ?? defaultMapping(node.nodeId, input);
          const existing = draft.inputMappings.find((candidate) => candidate.targetNode === node.nodeId && candidate.targetInput === input.name);
          return (
            <div className="workflow-input-card" key={key}>
              <div className="workflow-input-heading"><strong>{node.nodeId}.{input.name}</strong><span>{input.currentValueSummary}</span></div>
              {input.isLinked && <p className="field-hint">此参数在执行时会覆盖当前节点连接输入，原始工作流不会修改。</p>}
              {(input.bindable || (input.isLinked && isExposableWorkflowInput(input))) ? (
                <div className="workflow-mapping-form">
                  <label>语义键<input value={mapping.semanticKey} onChange={(event) => onPatch(key, { semanticKey: event.target.value })} /></label>
                  <label>字段类型<select value={mapping.fieldType} onChange={(event) => onPatch(key, { fieldType: event.target.value as WorkflowFieldType })}>{fieldTypes.map((type) => <option key={type} value={type}>{fieldTypeLabel(type)}</option>)}</select></label>
                  <label>显示名称<input value={mapping.label} onChange={(event) => onPatch(key, { label: event.target.value })} /></label>
                  <label className="checkbox-label"><input type="checkbox" checked={mapping.required} onChange={(event) => onPatch(key, { required: event.target.checked })} /> 必填</label>
                  {mapping.fieldType === "integer" || mapping.fieldType === "number" || mapping.fieldType === "seed" ? <>
                    <label>默认值<input value={mapping.defaultValue} onChange={(event) => onPatch(key, { defaultValue: event.target.value })} /></label>
                    <label>最小值<input value={mapping.minValue} onChange={(event) => onPatch(key, { minValue: event.target.value })} inputMode={mapping.fieldType === "number" ? "decimal" : "numeric"} /></label>
                    <label>最大值<input value={mapping.maxValue} onChange={(event) => onPatch(key, { maxValue: event.target.value })} inputMode={mapping.fieldType === "number" ? "decimal" : "numeric"} /></label>
                  </> : null}
                  {mapping.fieldType === "integer" || mapping.fieldType === "number" ? <label>步长<input value={mapping.step} onChange={(event) => onPatch(key, { step: event.target.value })} inputMode={mapping.fieldType === "number" ? "decimal" : "numeric"} /></label> : null}
                  {mapping.fieldType.endsWith("s") ? <label>最大数量<input value={mapping.maxItems} onChange={(event) => onPatch(key, { maxItems: event.target.value })} inputMode="numeric" /></label> : null}
                  <button type="button" onClick={() => onBind(node.nodeId, input)} disabled={!input.bindable && !input.isLinked}>确认映射</button>
                </div>
              ) : <p className="disabled-note">当前输入不能作为生产参数。</p>}
              {input.allowedOptions.length > 0 && <small className="field-hint">可用选项：{input.allowedOptions.join(", ")}</small>}
              {existing && <div className="workflow-existing-mapping"><span>已映射为 <strong>{existing.label}</strong></span><button type="button" className="quiet-button" onClick={() => onRemove(existing)}>移除</button></div>}
            </div>
          );
        }))}
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>配置输出</button></div>
    </div>
  );
}

function OutputsPane({ draft, candidates, outputDraft, onChange, onAdd, onContinue }: { draft: WorkflowOnboardingDraftView; candidates: WorkflowNodeView[]; outputDraft: OutputDraft; onChange: (value: OutputDraft) => void; onAdd: () => void; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <p className="section-description">明确声明用户可见的输出结果。输出 ID 是运行包使用的稳定 snake_case 技术键。</p>
      <div className="workflow-mapping-form workflow-output-form">
        <label>输出 ID<input value={outputDraft.outputId} onChange={(event) => onChange({ ...outputDraft, outputId: event.target.value })} /></label>
        <label>显示名称<input value={outputDraft.label} onChange={(event) => onChange({ ...outputDraft, label: event.target.value })} /></label>
        <label>类型<select value={outputDraft.type} onChange={(event) => onChange({ ...outputDraft, type: event.target.value as "image" | "video" })}><option value="image">图片</option><option value="video">视频</option></select></label>
        <label>输出节点<select value={outputDraft.nodeId} onChange={(event) => onChange({ ...outputDraft, nodeId: event.target.value })}>{candidates.map((node) => <option key={node.nodeId} value={node.nodeId}>节点 {node.nodeId} · {node.classType}</option>)}</select></label>
        <label className="checkbox-label"><input type="checkbox" checked={outputDraft.required} onChange={(event) => onChange({ ...outputDraft, required: event.target.checked })} /> 必填</label>
        <button type="button" onClick={onAdd} disabled={!outputDraft.nodeId}>确认输出</button>
      </div>
      <div className="workflow-output-list">{draft.outputMappings.map((output) => <div key={output.outputId}><strong>{output.label}</strong><span>{output.outputId} · {output.type === "video" ? "视频" : "图片"} · 节点 {output.nodeId}</span></div>)}</div>
      <div className="workflow-pane-actions"><button type="button" onClick={onContinue}>设置基本信息</button></div>
    </div>
  );
}

function MetadataPane({ draft, onChange, onSave, onContinue }: { draft: MetadataDraft; onChange: (value: MetadataDraft) => void; onSave: () => void; onContinue: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-mapping-form workflow-metadata-form">
        <label>工作流 ID<input value={draft.workflowId} onChange={(event) => onChange({ ...draft, workflowId: event.target.value })} /></label>
        <label>名称<input value={draft.name} onChange={(event) => onChange({ ...draft, name: event.target.value })} /></label>
        <label>工作流版本<input value={draft.workflowVersion} onChange={(event) => onChange({ ...draft, workflowVersion: event.target.value })} /></label>
        <label>配方版本<input value={draft.recipeVersion} onChange={(event) => onChange({ ...draft, recipeVersion: event.target.value })} /></label>
        <label>分类<input value={draft.category} onChange={(event) => onChange({ ...draft, category: event.target.value })} /></label>
        <label>模式<input value={draft.mode} onChange={(event) => onChange({ ...draft, mode: event.target.value })} /></label>
      </div>
      <div className="workflow-pane-actions"><button type="button" onClick={onSave}>保存基本信息</button><button type="button" onClick={onContinue}>校验草稿</button></div>
    </div>
  );
}

function ValidatePane({ draft, loading, onValidate, onPublish }: { draft: WorkflowOnboardingDraftView; loading: boolean; onValidate: () => void; onPublish: () => void }) {
  const validation = draft.validation;
  const checks = [
    ["API 格式", validation.apiFormat],
    ["配方", validation.recipe],
    ["输入绑定", validation.bindings],
    ["输出", validation.outputs],
    ["清单", validation.manifest],
    ["兼容状态", validation.capability],
    ["试运行", validation.dryRun],
  ] as const;
  return (
    <div className="workflow-onboarding-pane">
      <div className="workflow-validation-grid">{checks.map(([label, valid]) => <span key={label} className={valid ? "workflow-check-pass" : "workflow-check-fail"}>{valid ? "✓" : "!"} {label}</span>)}</div>
      {!!validation.issues.length && <ul className="workflow-issue-list">{validation.issues.map((issue) => <li key={issue}>{localizeWorkflowIssue(issue)}</li>)}</ul>}
      <details className="workflow-recipe-preview"><summary>高级信息：生成的配方配置</summary><pre>{draft.recipe.yaml ?? "配方当前还未通过校验。"}</pre></details>
      <div className="workflow-pane-actions"><button type="button" onClick={onValidate} disabled={loading}>{loading ? "正在校验..." : "再次校验"}</button><button type="button" onClick={onPublish} disabled={!validation.readyToPublish}>继续发布</button></div>
    </div>
  );
}

function PublishPane({ draft, published, loading, onPublish, onOpenStudio }: { draft: WorkflowOnboardingDraftView; published?: { workflowId: string; recipeId: string }; loading: boolean; onPublish: () => void; onOpenStudio?: () => void }) {
  return (
    <div className="workflow-onboarding-pane">
      <div className={`workflow-publish-state ${draft.validation.readyToPublish ? "workflow-check-pass" : "workflow-check-fail"}`}>
        {draft.validation.readyToPublish ? "可以发布" : "所有检查通过后才能发布。"}
      </div>
      <button type="button" onClick={onPublish} disabled={loading || !draft.validation.readyToPublish}>{loading ? "正在发布..." : "发布工作流运行包"}</button>
      {published && <div className="workflow-published-result"><strong>发布成功</strong><span>刷新目录后即可在创作工作台使用该运行包。</span><button type="button" onClick={onOpenStudio}>在创作工作台中打开</button></div>}
    </div>
  );
}

function VersionDiffPane({ diff, onClose }: { diff: WorkflowVersionDiffView; onClose: () => void }) {
  return (
    <details className="workflow-diff-panel" open>
      <summary>版本差异 · {diff.versionA} 对比 {diff.versionB}</summary>
      <div className="workflow-detail-grid">
        <span>节点数量 <strong>{diff.nodeCountA} → {diff.nodeCountB}</strong></span>
        <span>新增节点 <strong>{diff.addedNodes.length}</strong></span>
        <span>移除节点 <strong>{diff.removedNodes.length}</strong></span>
        <span>类型变化 <strong>{diff.changedClassTypes.length}</strong></span>
        <span>字面量变化 <strong>{diff.changedLiteralInputs.length}</strong></span>
        <span>连接变化 <strong>{diff.changedLinks.length}</strong></span>
      </div>
      {!!diff.addedNodes.length && <p>新增节点：{diff.addedNodes.join(", ")}</p>}
      {!!diff.removedNodes.length && <p>移除节点：{diff.removedNodes.join(", ")}</p>}
      {!!diff.changedClassTypes.length && <ul className="workflow-issue-list">{diff.changedClassTypes.map((change) => <li key={change.nodeId}>节点 {change.nodeId}：{change.from} → {change.to}</li>)}</ul>}
      {!!diff.changedLiteralInputs.length && <ul className="workflow-issue-list">{diff.changedLiteralInputs.map((change) => <li key={`${change.nodeId}:${change.input}`}>节点 {change.nodeId}.{change.input}：{change.from} → {change.to}</li>)}</ul>}
      {!!diff.changedLinks.length && <ul className="workflow-issue-list">{diff.changedLinks.map((change) => <li key={`${change.nodeId}:${change.input}`}>节点连接已变化：{change.nodeId}.{change.input}</li>)}</ul>}
      {!!diff.recipeInputChanges.length && <p>配方输入：{diff.recipeInputChanges.join("；")}</p>}
      {!!diff.bindingChanges.length && <p>输入绑定：{diff.bindingChanges.join("；")}</p>}
      {!!diff.outputChanges.length && <p>输出：{diff.outputChanges.join("；")}</p>}
      <button type="button" className="quiet-button" onClick={onClose}>关闭差异</button>
    </details>
  );
}

function IssueList({ issues }: { issues: WorkflowOnboardingDraftView["capability"]["issues"] }) {
  return <ul className="workflow-issue-list">{issues.map((issue) => <li key={`${issue.code}:${issue.nodeId ?? ""}:${issue.inputName ?? ""}`}>{toUserMessage({ code: issue.code, message: issue.message })}</li>)}</ul>;
}

function defaultMapping(nodeId: string, input: WorkflowInputView): MappingDraft {
  const safeName = input.name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "value";
  const fieldType = supportedParameterFieldType(input) ?? "textarea";
  return {
    semanticKey: input.suggestedSemanticKey ?? `input_${nodeId}_${safeName}`,
    fieldType,
    label: fieldLabel(input.name),
    required: true,
    defaultValue: !input.isLinked && (fieldType === "textarea" || fieldType === "integer" || fieldType === "number" || fieldType === "seed")
      ? input.currentValueSummary === "random" ? "" : input.currentValueSummary
      : "",
    minValue: input.numericMin ?? "",
    maxValue: input.numericMax ?? "",
    minItems: "",
    maxItems: "",
    itemIndex: "",
    step: input.numericStep ?? "",
  };
}

function emptyMapping(): MappingDraft {
  return { semanticKey: "input_value", fieldType: "textarea", label: "值", required: true, defaultValue: "", minValue: "", maxValue: "", minItems: "", maxItems: "", itemIndex: "", step: "" };
}

function fieldLabel(value: string): string {
  return value
    .split(/[_-]+/g)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function supportedParameterFieldType(input: WorkflowInputView): WorkflowFieldType | undefined {
  if (input.suggestedType && fieldTypes.includes(input.suggestedType as WorkflowFieldType)) {
    return input.suggestedType as WorkflowFieldType;
  }
  return undefined;
}

export function isExposableWorkflowInput(input: WorkflowInputView): boolean {
  const linkedSemantic = input.suggestedSemanticKey?.toLowerCase();
  const graphSemantic = [
    "prompt", "negative_prompt", "width", "height", "duration_seconds", "seed",
    "reference_image", "reference_video", "reference_audio",
  ].includes(linkedSemantic ?? "");
  return (input.bindable || (input.isLinked && graphSemantic))
    && Boolean(supportedParameterFieldType(input))
    && !isDangerousParameterName(input.name);
}

function isDangerousParameterName(name: string): boolean {
  const lower = name.toLowerCase();
  return [
    "model_path", "filename_prefix", "output_directory", "output_dir", "filesystem_path", "file_path",
    "custom_python", "python_path", "backend_endpoint", "endpoint", "filename", "directory", "folder",
    "path", "prefix", "python", "device", "provider", "checkpoint", "ckpt", "unet", "vae", "clip", "lora", "model",
  ].some((token) => lower === token || lower.includes(token));
}

function mappingKey(nodeId: string, inputName: string): string {
  return `${nodeId}:${inputName}`;
}

function optionalText(value: string): string | undefined {
  return value.trim() || undefined;
}

function optionalNumber(value: string): number | undefined {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : undefined;
}

function quickTestValues(recipe: RecipeViewModel): GenerationValues | undefined {
  const values: GenerationValues = {};
  for (const field of recipe.fields) {
    switch (field.type) {
      case "textarea":
        values[field.key] = {
          type: "string",
          value: field.default || (field.required ? "AI Studio 快速测试" : ""),
        };
        break;
      case "integer":
        if (field.default !== undefined) {
          values[field.key] = { type: "integer", value: field.default };
        } else if (field.required) {
          values[field.key] = { type: "integer", value: field.min ?? 1 };
        }
        break;
      case "seed":
        values[field.key] = field.defaultMode === "fixed" && field.defaultValue
          ? { type: "seed_fixed", value: field.defaultValue }
          : { type: "seed_random" };
        break;
      case "image":
      case "images":
      case "video":
      case "videos":
      case "audio":
      case "audios":
        if (field.required) return undefined;
        break;
    }
  }
  return values;
}

function formatCapability(value: string): string {
  return {
    READY: "可用",
    MISSING_NODES: "缺少节点",
    INCOMPATIBLE_INPUT_VALUES: "输入值不兼容",
    COMFY_OFFLINE: "ComfyUI 离线",
    NOT_CHECKED: "尚未检查",
  }[value] ?? "未知状态";
}

function workflowSourceLabel(item: WorkflowProductionWorkspaceView): string {
  return item.builtin || item.source?.trim().toUpperCase() === "PRODUCT" ? "系统自带" : "用户导入";
}

function fieldTypeLabel(value: WorkflowFieldType): string {
  return {
    textarea: "多行文本",
    integer: "整数",
    number: "小数",
    seed: "随机种子",
    image: "图片",
    images: "多张图片",
    video: "视频",
    videos: "多个视频",
    audio: "音频",
    audios: "多个音频",
  }[value];
}

function localizeWorkflowIssue(value: string): string {
  const normalized = value.toLowerCase();
  if (normalized.includes("api") && normalized.includes("format")) return "该文件不是 ComfyUI API 格式工作流。";
  if (normalized.includes("recipe")) return "配方校验未通过，请检查输入映射和输出映射。";
  if (normalized.includes("binding")) return "输入绑定校验未通过，请检查每个输入映射。";
  if (normalized.includes("output")) return "输出校验未通过，请至少配置一个有效输出。";
  if (normalized.includes("manifest")) return "工作流基本信息校验未通过。";
  if (normalized.includes("capability")) return "ComfyUI 兼容性校验未通过。";
  if (normalized.includes("dry run")) return "工作流试运行未通过。";
  return "工作流校验未通过，请查看技术详情。";
}
