import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createGeneration, getProjectWorkflowConfig, listRuntimeProfiles } from "../../services/tauriClient";
import {
  cleanWorkflowStaging,
  compareWorkflowVersions,
  discardOnboarding,
  deleteWorkflow,
  deleteWorkflowVersion,
  duplicateWorkflowRecipe,
  exportWorkflowPackage,
  importWorkflowPackageBackup,
  inspectWorkflowDeletion,
  inspectWorkflowPurge,
  repairBuiltinWorkflowPackage,
  pickApiWorkflow,
  recheckWorkflowCapability,
  recheckAllWorkflowCapabilities,
  removeWorkflow,
  renameWorkflow,
  restoreWorkflowVersion,
  restoreWorkflow,
  purgeWorkflow,
  setWorkflowCurrentVersion,
  setWorkflowEnabled,
  queryWorkflowWorkspace,
  promoteWorkflowRecipe,
} from "../../services/workflowClient";
import { useWorkflowOnboardingStore, type WorkflowOnboardingStep } from "../../stores/workflowOnboardingStore";
import type {
  WorkflowFieldType,
  WorkflowDeletionInspection,
  WorkflowInputView,
  WorkflowNodeView,
  WorkflowOnboardingDraftView,
  WorkflowProductionWorkspaceView,
  WorkflowPurgeInspection,
  WorkflowPurgeResult,
  WorkflowRegistryVersionView,
  WorkflowRegistryRecipeView,
  WorkflowDeletionResult,
  WorkflowVersionDiffView,
} from "../../types/workflowOnboarding";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import type { RuntimeParameterProfile } from "../../types/settings";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime } from "../../i18n/statusLabels";
import { WorkflowImportController } from "./WorkflowImportController";
import { useWorkflowSmartImportController } from "./hooks/useWorkflowSmartImportController";
import {
  useWorkflowAdvancedOnboardingController,
  type MetadataDraft,
  type OutputDraft,
} from "./hooks/useWorkflowAdvancedOnboardingController";
import { useWorkflowParameterExposureController } from "./hooks/useWorkflowParameterExposureController";
import { WorkflowDeleteDialog, type WorkflowDeletionMode } from "./WorkflowDeleteDialog";
import {
  normalizeWorkspaceItems,
  resolveImplicitWorkflowRecipe,
  type WorkflowWorkspaceItem,
} from "./workflowWorkspaceAdapters";
import { WorkflowRegistryActions } from "./WorkflowRegistryActions";
import { WorkflowWorkspaceList } from "./WorkflowWorkspaceList";
import { WorkflowCenterOverview } from "./WorkflowCenterOverview";
import { buildProductionProfiles, buildWorkflowCenterSummary } from "./workflowCenterModel";
import {
  defaultMapping,
  isDangerousParameterName,
  isExposableWorkflowInput,
  localizeWorkflowIssue,
  mappingKey,
  mappingToDraft,
  parameterFieldTypes,
  supportedParameterFieldType,
  type MappingDraft,
  type ParameterMappingEdit,
} from "./workflowParameterExposureModel";

export {
  latestCatalogRecipeForWorkflowItem,
  normalizeWorkspaceItem,
  resolveImplicitWorkflowRecipe,
} from "./workflowWorkspaceAdapters";
export { isExposableWorkflowInput } from "./workflowParameterExposureModel";

interface Props {
  projectId?: string;
  catalog: RecipeViewModel[];
  comfyConnected: boolean;
  onCatalogChanged: () => Promise<void>;
  onOpenStudio: (workflowId: string, recipeId: string) => Promise<void>;
  onUseInProject: (workflowId: string, recipeId: string) => Promise<void>;
  onOpenProjectSettings?: () => void;
  onOpenTask?: (taskId: string) => void;
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

export { createDefaultOutputDraft } from "./hooks/useWorkflowAdvancedOnboardingController";

interface WorkflowDeletionTarget {
  item: WorkflowWorkspaceItem;
  inspection: WorkflowDeletionInspection | WorkflowPurgeInspection;
  mode: WorkflowDeletionMode;
}

export function WorkflowWorkspace({ projectId, catalog, comfyConnected, onCatalogChanged, onOpenStudio, onUseInProject, onOpenProjectSettings, onOpenTask }: Props) {
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
        await discardReplacedDraft(previousDraftId);
        resetImportViewForNewWorkflow();
      }
    } catch (importError: unknown) {
      await discardReplacedDraft(previousDraftId);
      reset();
      smartImportController.resetSession();
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
    if (!smartImportController.plan?.existingWorkflowId) return;
    const existing = items.find((item) =>
      item.archived
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
      advancedController.openAdvanced(duplicated);
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

  return (
    <section className="workspace-panel workflow-workspace" aria-busy={loading || workspaceLoading}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">工作区</span>
          <h2>工作流管理</h2>
          <p className="section-description">导入 ComfyUI API 工作流，配置安全输入后发布工作流运行包。</p>
        </div>
        <WorkflowRegistryActions
          loading={loading}
          workspaceLoading={workspaceLoading}
          checkingAll={checkingAll}
          importBusy={importBusyRef.current}
          onRefresh={() => void loadWorkspace("refresh")}
          onAdd={() => void smartImportController.smartImport()}
          onCheckAll={() => void recheckAllVersions()}
          onManualImport={() => void importWorkflow()}
          onImportBackup={() => void importBackup()}
        />
      </div>

      <WorkflowCenterOverview
        summary={centerSummary}
        profiles={productionProfiles}
        projectId={projectId}
        comfyConnected={comfyConnected}
        workspaceLoading={workspaceLoading}
        projectConfigLoading={projectWorkflowLoading}
        projectConfigError={projectWorkflowError}
        runtimeProfilesLoading={runtimeProfilesLoading}
        runtimeProfilesError={runtimeProfilesError}
        onOpenProjectSettings={onOpenProjectSettings ?? (() => undefined)}
        onManageParameters={(recipe) => void onOpenStudio(recipe.workflowId, recipe.recipeId)}
      />

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

      <WorkflowImportController
        plan={smartImportController.plan}
        importError={smartImportController.importError}
        draft={draft}
        projectId={projectId}
        loading={loading}
        onResolve={(issue, candidate) => void smartImportController.resolveIssue(issue, candidate)}
        onResume={() => void smartImportController.resume()}
        onOpenAdvanced={() => void smartImportController.openAdvanced()}
        onOpenExisting={() => void smartImportController.openExisting()}
        onOpenExistingVersion={() => void smartImportController.openExistingVersion()}
        onUseInProject={(workflowId, recipeId) => void onUseInProject(workflowId, recipeId)}
        onRegenerateRecipe={() => void smartImportController.regenerateRecipe()}
        onRestoreExisting={() => void restoreExistingArchivedWorkflow()}
        onCommitImport={(action) => void smartImportController.commit(action)}
        onOpenStudio={(workflowId, recipeId) => void onOpenStudio(workflowId, recipeId)}
        onRetry={() => void smartImportController.smartImport()}
        onReturnToList={() => void returnToWorkflowList()}
      />

      <WorkflowWorkspaceList
        items={items}
        staging={staging}
        catalog={catalog}
        projectId={projectId}
        search={search}
        filter={filter}
        selectedVersions={selectedVersions}
        workspaceLoading={workspaceLoading}
        quickTestingId={quickTestingId}
        onSearchChange={setSearch}
        onFilterChange={setFilter}
        onCompareSelected={() => void compareSelected()}
        onToggleSelected={toggleSelected}
        onToggleVersionSelection={(workflowVersionId) => setSelectedVersions((current) => current.includes(workflowVersionId) ? current.filter((id) => id !== workflowVersionId) : current.length < 2 ? [...current, workflowVersionId] : [current[1], workflowVersionId])}
        onUseInProject={(workflowId, recipeId) => void onUseInProject(workflowId, recipeId)}
        onQuickTest={(item) => void quickTest(item)}
        onInspectForDeletion={(item) => void inspectForDeletion(item)}
        onRestore={(item) => void restoreArchivedWorkflow(item)}
        onRename={openRename}
        onReidentify={(item) => void smartImportController.reidentify(item)}
        onRecheck={(item) => void recheckVersion(item)}
        onDuplicateRecipe={(item) => void duplicateRecipe(item)}
        onOpenParameters={(item) => void parameterExposureController.open(item)}
        onExport={(item) => { if (item.currentVersionId) void exportWorkflowPackage(item.currentVersionId); }}
        onToggle={(item) => void toggleVersion(item)}
        onPurge={(item) => void inspectForPurge(item)}
        onRepairBuiltinPackage={(item) => void repairBuiltinPackage(item)}
        onSetCurrentVersion={(item, version) => void setCurrentVersion(item, version)}
        onPromoteRecipe={(item, recipe) => void promoteRecipe(item, recipe)}
        onCleanStaging={(stagingId) => void cleanWorkflowStaging(stagingId)}
      />
      {diff && <VersionDiffPane diff={diff} onClose={() => setDiff(undefined)} />}

      {parameterExposureController.draft && parameterExposureController.item && (
        <ParameterExposurePane
          draft={parameterExposureController.draft}
          workflow={parameterExposureController.item}
          originalKeys={parameterExposureController.originalKeys}
          loading={parameterExposureController.loading}
          onClose={() => void parameterExposureController.close()}
          onRefresh={() => void parameterExposureController.refreshCapability()}
          onExpose={(nodeId, input) => void parameterExposureController.exposeParameter(nodeId, input)}
          onSaveMapping={(mapping, nodeId, inputName) => void parameterExposureController.saveMapping(mapping, nodeId, inputName)}
          onRemove={(mapping) => void parameterExposureController.removeMapping(mapping)}
          onSave={(edits) => void parameterExposureController.publish(edits)}
        />
      )}

      {advancedController.showAdvanced && draft && (
        <div className="workflow-onboarding-panel">
          <div className="workflow-onboarding-heading">
            <div>
              <span className="section-label">高级工作流编辑</span>
              <h3>{draft.manifest.name}</h3>
              <p className="section-description">{draft.originalFilename} · {draft.nodeCount} 个节点 · {draft.uniqueClassCount} 种节点类型</p>
            </div>
            <div className="workflow-smart-actions">
              <button type="button" className="quiet-button" onClick={advancedController.hideAdvanced}>返回智能导入</button>
              <button type="button" className="quiet-button" onClick={() => void advancedController.discardDraft()} disabled={loading}>丢弃草稿</button>
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
            <CompatibilityPane draft={draft} loading={loading} onCheck={() => void advancedController.checkCapability()} onContinue={() => setStep("inputs")} />
          )}
          {step === "inputs" && (
            <InputsPane
              draft={draft}
              mappingDrafts={advancedController.mappingDrafts}
              onPatch={advancedController.patchMapping}
              onBind={(nodeId, input) => void advancedController.bindInput(nodeId, input)}
              onRemove={(mapping) => void advancedController.removeInput(mapping)}
              onContinue={() => setStep("outputs")}
            />
          )}
          {step === "outputs" && (
            <OutputsPane
              draft={draft}
              candidates={outputCandidates.length ? outputCandidates : draft.nodes}
              outputDraft={advancedController.outputDraft}
              onChange={advancedController.setOutputDraft}
              onAdd={() => void advancedController.addOutput()}
              onContinue={() => setStep("metadata")}
            />
          )}
          {step === "metadata" && advancedController.metadataDraft && (
            <MetadataPane draft={advancedController.metadataDraft} onChange={advancedController.setMetadataDraft} onSave={() => void advancedController.saveMetadata()} onContinue={() => setStep("validate")} />
          )}
          {step === "validate" && (
            <ValidatePane draft={draft} loading={loading} onValidate={() => void advancedController.validateDraft()} onPublish={() => setStep("publish")} />
          )}
          {step === "publish" && (
            <PublishPane
              draft={draft}
              published={advancedController.published}
              loading={loading}
              onPublish={() => void advancedController.publishDraft()}
              onOpenStudio={advancedController.published ? () => void onOpenStudio(advancedController.published!.workflowId, advancedController.published!.recipeId) : undefined}
            />
          )}
        </div>
      )}

      {deletionTarget && (
        <WorkflowDeleteDialog
          item={deletionTarget.item}
          inspection={deletionTarget.inspection}
          mode={deletionTarget.mode}
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
                <label>类型<select value={edit.fieldType} onChange={(event) => patchMapping(mapping, { fieldType: event.target.value as WorkflowFieldType })}>{parameterFieldTypes.map((type) => <option key={type} value={type}>{fieldTypeLabel(type)}</option>)}</select></label>
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
                  <label>字段类型<select value={mapping.fieldType} onChange={(event) => onPatch(key, { fieldType: event.target.value as WorkflowFieldType })}>{parameterFieldTypes.map((type) => <option key={type} value={type}>{fieldTypeLabel(type)}</option>)}</select></label>
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
