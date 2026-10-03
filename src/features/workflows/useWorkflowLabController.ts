import { formatCapability } from "../workflow-lab/labViewHelpers";
import { useCallback,useEffect,useMemo,useRef,useState } from "react";
import { toUserMessage } from "../../i18n/errorMessages";
import { getProjectWorkflowConfig, listModelVersions, listModels, listRuntimeProfiles, startProductionQueue, submitGeneration } from "../../services/workflowLabClient";
import { archiveWorkflowRecipe, clearWorkflowRecipePromotion, compareWorkflowVersions, deleteWorkflow, deleteWorkflowVersionOf, discardOnboarding, duplicateWorkflowRecipe, getSavedWorkflowVersionDetails, getWorkflowRecipeHistory, importWorkflowPackageBackup, inspectWorkflowDeletion, inspectWorkflowPurge, pickApiWorkflow, promoteWorkflowRecipe, purgeWorkflow, queryWorkflowWorkspace, recheckAllWorkflowCapabilities, recheckWorkflowCapability, removeWorkflow, renameWorkflow, repairBuiltinWorkflowPackage, restoreWorkflow, restoreWorkflowRecipe, restoreWorkflowVersion, setWorkflowCurrentVersion, setWorkflowEnabled } from "../../services/workflowLabClient";
import { useWorkflowOnboardingStore,type WorkflowOnboardingStep } from "../../stores/workflowOnboardingStore";
import type { GenerationValues,RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import type { RuntimeParameterProfile } from "../../types/settings";
import type { WorkflowRecipeHistoryView } from "../../types/workflowHistory";
import type {
WorkflowDeletionInspection,
WorkflowDeletionResult,
WorkflowProductionWorkspaceView,
WorkflowPurgeInspection,
WorkflowPurgeResult,
WorkflowRegistryRecipeView,
WorkflowRegistryVersionView,
WorkflowSavedVersionDetailsView,
WorkflowVersionDiffView
} from "../../types/workflowOnboarding";
import { type WorkflowDeletionMode } from "./WorkflowDeleteDialog";
import {
useWorkflowAdvancedOnboardingController
} from "./hooks/useWorkflowAdvancedOnboardingController";
import { useWorkflowParameterExposureController } from "./hooks/useWorkflowParameterExposureController";
import { useWorkflowSmartImportController } from "./hooks/useWorkflowSmartImportController";
import { buildProductionProfiles,buildWorkflowCenterSummary } from "./workflowCenterModel";
import {
normalizeWorkspaceItems,
resolveImplicitWorkflowRecipe,
type WorkflowWorkspaceItem,
} from "./workflowWorkspaceAdapters";
export { SeedModeSelect } from "../workflow-lab/labViewHelpers";

export { isExposableWorkflowInput } from "./workflowParameterExposureModel";
export {
latestCatalogRecipeForWorkflowItem,
normalizeWorkspaceItem,
resolveImplicitWorkflowRecipe
} from "./workflowWorkspaceAdapters";

export interface WorkflowWorkspaceProps {
  projectId?: string;
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  onCatalogChanged: () => Promise<void>;
  onOpenStudio: (workflowId: string, recipeId: string) => Promise<void>;
  onUseInProject: (workflowId: string, recipeId: string) => Promise<void>;
  onOpenProjectSettings?: () => void;
  onOpenTask?: (taskId: string) => void;
}

export const steps: Array<{ value: WorkflowOnboardingStep; label: string }> = [
  { value: "inspect", label: "检查工作流" },
  { value: "compatibility", label: "兼容性检查" },
  { value: "inputs", label: "输入映射" },
  { value: "outputs", label: "输出映射" },
  { value: "metadata", label: "基本信息" },
  { value: "validate", label: "校验" },
  { value: "publish", label: "发布" },
];

export { createDefaultOutputDraft } from "./hooks/useWorkflowAdvancedOnboardingController";

interface WorkflowDeletionTarget {
  item: WorkflowWorkspaceItem;
  inspection: WorkflowDeletionInspection | WorkflowPurgeInspection;
  mode: WorkflowDeletionMode;
}

export function useWorkflowLabController({ projectId, catalog, comfyConnected, onCatalogChanged, onOpenStudio, onUseInProject, onOpenProjectSettings, onOpenTask }: WorkflowWorkspaceProps) {
  const [items, setItems] = useState<WorkflowWorkspaceItem[]>([]);
  const [staging, setStaging] = useState<{ stagingId: string; status: string; inUse: boolean }[]>([]);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<"all" | "available" | "issues" | "archived">("all");
  const [selectedVersions, setSelectedVersions] = useState<string[]>([]);
  const [diff, setDiff] = useState<WorkflowVersionDiffView>();
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [workspaceError, setWorkspaceError] = useState<string>();
  const [checkingAll, setCheckingAll] = useState(false);
  const [quickTestingId, setQuickTestingId] = useState<string>();
  const quickTestSubmissionsRef = useRef(new Map<string, { idempotencyKey: string; batchId?: string }>());
  const quickTestInProgressRef = useRef(false);
  const [quickTestModelOptions, setQuickTestModelOptions] = useState<Array<{ id: string; label: string }>>([]);
  const [quickTestModelLoading, setQuickTestModelLoading] = useState(false);
  const [quickTestModelError, setQuickTestModelError] = useState<string>();
  const [selectedQuickTestModelVersionId, setSelectedQuickTestModelVersionId] = useState("");
  const [deletionTarget, setDeletionTarget] = useState<WorkflowDeletionTarget>();
  const [renameTarget, setRenameTarget] = useState<WorkflowWorkspaceItem>();
  const [renameValue, setRenameValue] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [projectWorkflowConfig, setProjectWorkflowConfig] = useState<ProjectWorkflowConfigView>();
  const [projectWorkflowLoading, setProjectWorkflowLoading] = useState(false);
  const [projectWorkflowError, setProjectWorkflowError] = useState<string>();
  const [runtimeProfiles, setRuntimeProfiles] = useState<RuntimeParameterProfile[]>([]);
  const [runtimeProfilesLoading, setRuntimeProfilesLoading] = useState(false);
  const [runtimeProfilesError, setRuntimeProfilesError] = useState<string>();
  const [recipeHistory, setRecipeHistory] = useState<WorkflowRecipeHistoryView>();
  const [recipeHistoryTarget, setRecipeHistoryTarget] = useState<{ workflowVersionId: string; recipeId: string }>();
  const [recipeHistoryLoading, setRecipeHistoryLoading] = useState(false);
  const [recipeHistoryError, setRecipeHistoryError] = useState<string>();
  const [savedVersionDetails, setSavedVersionDetails] = useState<WorkflowSavedVersionDetailsView>();
  const [showExecutionConfig, setShowExecutionConfig] = useState(false);
  const [savedVersionDetailsLoadingId, setSavedVersionDetailsLoadingId] = useState<string>();
  const [savedVersionDetailsError, setSavedVersionDetailsError] = useState<string>();
  const recipeHistoryRequestRef = useRef(0);
  const draft = useWorkflowOnboardingStore((state) => state.draft);
  const step = useWorkflowOnboardingStore((state) => state.step);
  const loading = useWorkflowOnboardingStore((state) => state.loading);
  const error = useWorkflowOnboardingStore((state) => state.error);
  const notice = useWorkflowOnboardingStore((state) => state.notice);
  const setStep = useWorkflowOnboardingStore((state) => state.setStep);
  const setLoading = useWorkflowOnboardingStore((state) => state.setLoading);
  const setError = useWorkflowOnboardingStore((state) => state.setError);
  const setNotice = useWorkflowOnboardingStore((state) => state.setNotice);
  const reset = useWorkflowOnboardingStore((state) => state.reset);
  const importBusyRef = useRef(false);
  const advancedControllerRef = useRef<ReturnType<typeof useWorkflowAdvancedOnboardingController> | undefined>(undefined);

  const loadWorkspace = useCallback(async (mode: "fast" | "refresh" = "fast") => {
    setWorkspaceLoading(true);
    setWorkspaceError(undefined);
    try {
      const workspace = await queryWorkflowWorkspace(mode === "refresh" ? "REFRESH" : "FAST");
      const nextItems = normalizeWorkspaceItems(workspace.items);
      setItems(nextItems);
      setStaging(workspace.staging);
    } catch (loadError: unknown) {
      setWorkspaceError(toUserMessage(loadError));
    } finally {
      setWorkspaceLoading(false);
    }
  }, []);

  const refreshWorkspace = useCallback(() => loadWorkspace("refresh"), [loadWorkspace]);

  const openRecipeHistory = useCallback(async (item: WorkflowWorkspaceItem, recipe: WorkflowRegistryRecipeView) => {
    const workflowVersionId = recipe.workflowVersionId ?? item.currentVersionId;
    if (!workflowVersionId) return;
    const requestId = ++recipeHistoryRequestRef.current;
    setRecipeHistoryTarget({ workflowVersionId, recipeId: recipe.recipeId });
    setRecipeHistory(undefined);
    setRecipeHistoryLoading(true);
    setRecipeHistoryError(undefined);
    try {
      const result = await getWorkflowRecipeHistory(workflowVersionId, recipe.recipeId);
      if (recipeHistoryRequestRef.current === requestId) setRecipeHistory(result);
    } catch (error: unknown) {
      if (recipeHistoryRequestRef.current === requestId) setRecipeHistoryError(toUserMessage(error));
    } finally {
      if (recipeHistoryRequestRef.current === requestId) setRecipeHistoryLoading(false);
    }
  }, []);

  const loadMoreRecipeHistory = useCallback(async () => {
    if (!recipeHistoryTarget || !recipeHistory?.taskPage.nextCursor || recipeHistoryLoading) return;
    const requestId = ++recipeHistoryRequestRef.current;
    setRecipeHistoryLoading(true);
    try {
      const next = await getWorkflowRecipeHistory(
        recipeHistoryTarget.workflowVersionId,
        recipeHistoryTarget.recipeId,
        recipeHistory.taskPage.nextCursor,
      );
      if (recipeHistoryRequestRef.current === requestId) {
        setRecipeHistory((current) => current ? {
          ...next,
          taskPage: { ...next.taskPage, items: [...current.taskPage.items, ...next.taskPage.items] },
        } : next);
      }
    } catch (error: unknown) {
      if (recipeHistoryRequestRef.current === requestId) setRecipeHistoryError(toUserMessage(error));
    } finally {
      if (recipeHistoryRequestRef.current === requestId) setRecipeHistoryLoading(false);
    }
  }, [recipeHistory, recipeHistoryLoading, recipeHistoryTarget]);

  const closeRecipeHistory = useCallback(() => {
    recipeHistoryRequestRef.current += 1;
    setRecipeHistoryTarget(undefined);
    setRecipeHistory(undefined);
    setRecipeHistoryError(undefined);
    setRecipeHistoryLoading(false);
  }, []);

  const openSavedVersionDetails = useCallback(async (workflowVersionId: string) => {
    setSavedVersionDetails(undefined);
    setShowExecutionConfig(false);
    setSavedVersionDetailsError(undefined);
    setSavedVersionDetailsLoadingId(workflowVersionId);
    try {
      setSavedVersionDetails(await getSavedWorkflowVersionDetails(workflowVersionId));
    } catch (detailsError: unknown) {
      setSavedVersionDetailsError(toUserMessage(detailsError));
    } finally {
      setSavedVersionDetailsLoadingId(undefined);
    }
  }, []);

  useEffect(() => {
    void loadWorkspace("fast");
  }, [loadWorkspace]);

  useEffect(() => {
    let active = true;
    setProjectWorkflowConfig(undefined);
    setProjectWorkflowError(undefined);
    if (!projectId) {
      setProjectWorkflowLoading(false);
      return () => { active = false; };
    }
    setProjectWorkflowLoading(true);
    void Promise.resolve()
      .then(() => getProjectWorkflowConfig(projectId))
      .then((config) => {
        if (active && config) setProjectWorkflowConfig(config);
      })
      .catch((value: unknown) => {
        if (active) setProjectWorkflowError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setProjectWorkflowLoading(false);
      });
    return () => { active = false; };
  }, [projectId]);

  useEffect(() => {
    let active = true;
    setRuntimeProfilesLoading(true);
    setRuntimeProfilesError(undefined);
    void Promise.resolve()
      .then(() => listRuntimeProfiles())
      .then((profiles) => {
        if (active) setRuntimeProfiles(profiles);
      })
      .catch((value: unknown) => {
        if (active) setRuntimeProfilesError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setRuntimeProfilesLoading(false);
      });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    let active = true;
    setQuickTestModelOptions([]);
    setSelectedQuickTestModelVersionId("");
    setQuickTestModelError(undefined);
    if (!projectId) {
      setQuickTestModelLoading(false);
      return () => { active = false; };
    }
    setQuickTestModelLoading(true);
    void listModels()
      .then(async (models) => {
        const versionResults = await Promise.allSettled(models.map(async (model) => {
          const versions = await listModelVersions(model.id);
          return versions.map((version) => ({ model, version }));
        }));
        if (!active) return;
        const options = versionResults.flatMap((result) => result.status === "fulfilled"
          ? result.value.map(({ model, version }) => ({
            id: version.id,
            label: `${model.provider} · ${model.name} · v${version.version}`,
          }))
          : []);
        setQuickTestModelOptions(options);
        if (versionResults.some((result) => result.status === "rejected")) {
          setQuickTestModelError("部分模型版本加载失败；未加载的版本不会被推断到快速测试。" );
        }
      })
      .catch((value: unknown) => {
        if (active) setQuickTestModelError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setQuickTestModelLoading(false);
      });
    return () => { active = false; };
  }, [projectId]);

  const discardReplacedDraft = async (previousDraftId: string | undefined, nextDraftId?: string) => {
    if (!previousDraftId || previousDraftId === nextDraftId) return;
    try {
      await discardOnboarding(previousDraftId);
    } catch {
      // A published or already discarded draft is safe to replace locally.
    }
  };

  const smartImportController = useWorkflowSmartImportController({
    workspaceItems: items,
    importBusyRef,
    onLoadWorkspace: loadWorkspace,
    onCatalogChanged,
    onDiscardReplacedDraft: discardReplacedDraft,
    onResetImportView: () => {
      advancedControllerRef.current?.resetSession();
    },
    onCloseAdvanced: () => advancedControllerRef.current?.hideAdvanced(),
    onAdvancedRequested: (nextDraft) => advancedControllerRef.current?.openAdvanced(nextDraft),
    onPublished: (nextPublished) => advancedControllerRef.current?.setPublished(nextPublished),
    onOpenStudio,
  });

  const advancedController = useWorkflowAdvancedOnboardingController({
    onLoadWorkspace: loadWorkspace,
    onCatalogChanged,
    onResetSmartImport: () => smartImportController.resetSession(),
  });
  advancedControllerRef.current = advancedController;

  const parameterExposureController = useWorkflowParameterExposureController({
    onError: setWorkspaceError,
    onNotice: setNotice,
    onWorkspaceRefresh: refreshWorkspace,
    onCatalogChanged,
    onBeforeOpen: advancedController.hideAdvanced,
  });

  async function returnToSmartImport() {
    advancedController.hideAdvanced();
    await smartImportController.reanalyzeCurrentDraft();
  }

  function resetImportViewForNewWorkflow() {
    reset();
    setLoading(true);
    smartImportController.resetSession();
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
    smartImportController.resetSession();
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
        advancedController.openAdvanced(imported);
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
        // Cancelling the picker must leave the current draft/session intact.
      }
    } catch (importError: unknown) {
      setError(toUserMessage(importError));
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
        setNotice(restored.status === "ALREADY_INSTALLED"
          ? `工作流版本 ${restored.workflowVersion} 已存在，已重新检查运行能力：${restored.capability}。`
          : `工作流备份已恢复：${restored.workflowVersion}。恢复的版本处于待审核（停用）状态，确认后请手动启用。`);
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
    setWorkspaceError(undefined);
    try {
      const inspection = await inspectWorkflowPurge(item.workflowId!);
      if (!inspection.canPurge) {
        setWorkspaceError(inspection.blockingReasons.join(" ") || "该工作流当前不能彻底删除。");
        return;
      }
      setDeletionTarget({ item, inspection, mode: "PURGE" });
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function confirmWorkflowDeletion() {
    if (!deletionTarget || deleting) return;
    setDeleting(true);
    setWorkspaceError(undefined);
    try {
      const { item, mode, inspection } = deletionTarget;
      if (mode === "PURGE" && item.workflowId) {
        const purgeResult: WorkflowPurgeResult = await purgeWorkflow(item.workflowId);
        setDeletionTarget(undefined);
        let refreshFailed = false;
        try {
          await loadWorkspace("refresh");
          await onCatalogChanged();
        } catch {
          refreshFailed = true;
        }
        setNotice([
          `${inspection.name} 已永久删除，无法恢复。`,
          purgeResult.cleanupPending && "部分隔离临时文件尚未清理，不影响删除结果。",
        ].filter((part): part is string => Boolean(part)).join(" "));
        if (refreshFailed) setWorkspaceError("工作流已永久删除，但页面刷新未完成，请手动刷新。");
        return;
      }

      const removeInspection = inspection as WorkflowDeletionInspection;
      let result: Array<WorkflowDeletionResult | { projectBindingCount?: number; action?: string }>;
      if (item.registryBacked && item.workflowId) {
        result = [await removeWorkflow(item.workflowId)];
      } else if (item.workflowId) {
        result = await deleteWorkflow(item.workflowId);
      } else {
        result = [];
      }
      setDeletionTarget(undefined);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      const projectBindingCount = result.reduce((total, entry) => total + (entry.projectBindingCount ?? 0), 0);
      const hasHistory = removeInspection.historicalTaskCount > 0
        || removeInspection.productionBatchItemCount > 0
        || removeInspection.benchmarkReferenceCount > 0;
      const message = [
        `${inspection.name} 已删除。`,
        projectBindingCount > 0 && `已解除 ${projectBindingCount} 项项目工作流配置。`,
        hasHistory && "历史生产记录仍然保留。",
        "已从工作流库移除，可在“已删除”中恢复。",
      ].filter((part): part is string => Boolean(part)).join(" ");
      setNotice(message);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    } finally {
      setDeleting(false);
    }
  }

  async function restoreArchivedWorkflow(item: WorkflowWorkspaceItem) {
    if ((!item.workflowVersionId && !item.workflowId) || (!item.archived && item.libraryState !== "REMOVED")) return;
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
    if (!smartImportController.plan?.existingWorkflowId) return;
    const existing = items.find((item) =>
      (item.archived || item.libraryState === "REMOVED")
      && item.workflowId === smartImportController.plan?.existingWorkflowId
      && (!smartImportController.plan?.existingWorkflowVersion || item.workflowVersion === smartImportController.plan?.existingWorkflowVersion),
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

  async function deleteVersion(item: WorkflowWorkspaceItem, version: WorkflowRegistryVersionView) {
    if (!item.registryBacked || !item.workflowId || !version.workflowVersionId || version.workflowVersionId === item.currentVersionId) return;
    const label = version.version ?? version.workflowVersion ?? "—";
    if (!window.confirm(`确定删除版本 ${label}？工作流及其他版本将保留。`)) return;
    try {
      await deleteWorkflowVersionOf(item.workflowId, version.workflowVersionId);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(`已删除版本 ${label}；工作流及其他版本保持不变。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function promoteRecipe(item: WorkflowWorkspaceItem, recipe: WorkflowRegistryRecipeView) {
    const workflowVersionId = recipe.workflowVersionId;
    if (!item.registryBacked || item.archived || !workflowVersionId) return;
    try {
      await promoteWorkflowRecipe(workflowVersionId, recipe.recipeId);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(`已将配方 ${recipe.version ?? recipe.recipeVersion ?? "—"} 设为该工作流版本的推广配方。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function clearRecipePromotion(item: WorkflowWorkspaceItem, recipe: WorkflowRegistryRecipeView) {
    const workflowVersionId = recipe.workflowVersionId ?? item.currentVersionId;
    if (!item.registryBacked || !workflowVersionId || !recipe.isPromoted) return;
    try {
      await clearWorkflowRecipePromotion(workflowVersionId, recipe.recipeId);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(`已取消配方 ${recipe.version ?? recipe.recipeVersion ?? "—"} 的推广状态。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function archiveRecipe(item: WorkflowWorkspaceItem, recipe: WorkflowRegistryRecipeView) {
    const workflowVersionId = recipe.workflowVersionId ?? item.currentVersionId;
    if (!item.registryBacked || !workflowVersionId || recipe.archived) return;
    try {
      await archiveWorkflowRecipe(workflowVersionId, recipe.recipeId);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(`已归档配方 ${recipe.version ?? recipe.recipeVersion ?? "—"}；历史引用保持不变。`);
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function restoreRecipe(item: WorkflowWorkspaceItem, recipe: WorkflowRegistryRecipeView) {
    const workflowVersionId = recipe.workflowVersionId ?? item.currentVersionId;
    if (!item.registryBacked || !workflowVersionId || !recipe.archived) return;
    try {
      await restoreWorkflowRecipe(workflowVersionId, recipe.recipeId);
      await loadWorkspace("refresh");
      await onCatalogChanged();
      setNotice(`已恢复配方 ${recipe.version ?? recipe.recipeVersion ?? "—"}；未自动推广。`);
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
      smartImportController.resetSession();
      advancedController.openAdvanced(duplicated, true);
      setStep("inputs");
      setNotice("配方已复制，请检查映射并发布新的配方版本。");
    } catch (actionError: unknown) {
      setWorkspaceError(toUserMessage(actionError));
    }
  }

  async function quickTest(item: WorkflowWorkspaceItem) {
    if (!projectId || !item.workflowVersionId) return;
    const recipe = resolveImplicitWorkflowRecipe(item, catalog);
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
      setNotice("高级验证需要素材，请在执行配置中补充必需输入。尚未提交队列或修改创作草稿。");
      await openSavedVersionDetails(recipe.workflowVersionId);
      setShowExecutionConfig(true);
      return;
    }
    if (quickTestInProgressRef.current) return;
    const submissionIdentity = JSON.stringify([
      projectId,
      recipe.workflowVersionId,
      recipe.recipeId,
      selectedQuickTestModelVersionId,
    ]);
    const pendingSubmission = quickTestSubmissionsRef.current.get(submissionIdentity) ?? {
      idempotencyKey: crypto.randomUUID(),
    };
    quickTestSubmissionsRef.current.set(submissionIdentity, pendingSubmission);
    quickTestInProgressRef.current = true;
    setQuickTestingId(item.workflowVersionId);
    setWorkspaceError(undefined);
    try {
      let batchId = pendingSubmission.batchId;
      if (!batchId) {
        const batch = await submitGeneration({
          projectId,
          workflowVersionId: recipe.workflowVersionId,
          recipeId: recipe.recipeId,
          values,
          submissionIdempotencyKey: pendingSubmission.idempotencyKey,
          ...(selectedQuickTestModelVersionId ? { modelVersionId: selectedQuickTestModelVersionId } : {}),
        });
        batchId = batch.id;
        pendingSubmission.batchId = batchId;
      }
      await startProductionQueue(projectId, batchId);
      quickTestSubmissionsRef.current.delete(submissionIdentity);
      setNotice(`快速测试已加入生产队列并开始处理：${batchId}`);
    } catch (testError: unknown) {
      setWorkspaceError(toUserMessage(testError));
    } finally {
      quickTestInProgressRef.current = false;
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

  const outputCandidates = useMemo(
    () => draft?.nodes.filter((node) => node.isOutputNode) ?? [],
    [draft],
  );
  const productionProfiles = useMemo(
    () => projectWorkflowConfig
      ? buildProductionProfiles(projectWorkflowConfig, catalog, items, runtimeProfiles)
      : [],
    [catalog, items, projectWorkflowConfig, runtimeProfiles],
  );
  const centerSummary = useMemo(
    () => buildWorkflowCenterSummary(items, runtimeProfiles, productionProfiles),
    [items, productionProfiles, runtimeProfiles],
  );

  return { loading, workspaceLoading, checkingAll, importBusyRef, loadWorkspace, smartImportController, recheckAllVersions, importWorkflow, importBackup, centerSummary, productionProfiles, projectId, comfyConnected, projectWorkflowLoading, projectWorkflowError, runtimeProfilesLoading, runtimeProfilesError, onOpenProjectSettings, onOpenStudio, workspaceError, error, notice, selectedQuickTestModelVersionId, setSelectedQuickTestModelVersionId, quickTestModelLoading, quickTestModelOptions, quickTestModelError, draft, onUseInProject, restoreExistingArchivedWorkflow, returnToWorkflowList, items, staging, catalog, search, filter, selectedVersions, quickTestingId, setSearch, setFilter, compareSelected, toggleSelected, setSelectedVersions, quickTest, inspectForDeletion, restoreArchivedWorkflow, openRename, recheckVersion, duplicateRecipe, parameterExposureController, openRecipeHistory, toggleVersion, inspectForPurge, repairBuiltinPackage, setCurrentVersion, deleteVersion, openSavedVersionDetails, promoteRecipe, clearRecipePromotion, archiveRecipe, restoreRecipe, diff, setDiff, savedVersionDetailsLoadingId, savedVersionDetailsError, savedVersionDetails, setSavedVersionDetails, setShowExecutionConfig, showExecutionConfig, onOpenTask, recipeHistoryLoading, recipeHistory, recipeHistoryError, closeRecipeHistory, loadMoreRecipeHistory, advancedController, returnToSmartImport, step, setStep, outputCandidates, deletionTarget, deleting, setDeletionTarget, confirmWorkflowDeletion, renameTarget, renameValue, setRenameValue, setRenameTarget, saveRename };
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
