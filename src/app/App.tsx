import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import {
  getComfyStatus,
  getAsset,
  getRuntimeActivityStatus,
  getProductionAdmissionStatus,
  getConsistencyScopeBinding,
  getShotConsistencyBinding,
  getShotContextDraft,
  listGenerationCatalog,
  listConsistencyProfiles,
  listCostumeVariants,
  listReferenceSets,
  listProjects,
  listRecentTasks,
  listShots,
  reconcileActiveTasks,
  refreshComfyCapabilities,
  replaceConsistencyScopeBinding,
  getProjectWorkflowConfig,
  upsertProjectWorkflowBinding,
  replaceShotConsistencyBinding,
} from "../services/tauriClient";
import { subscribeTaskUpdates } from "../services/taskEvents";
import { useTaskStore } from "../stores/taskStore";
import { useProjectStore } from "../stores/projectStore";
import { useWorkspaceResumeStore } from "../stores/workspaceResumeStore";
import type { RecipeViewModel } from "../types/generation";
import type { AssetView } from "../types/asset";
import type { TemplateProjectResult } from "../types/organization";
import type { ProjectCommandCenterNavigationRequest } from "../features/projects/ProjectCommandCenter";
import { bootstrap, type BootstrapState } from "./bootstrap";
import { WorkspaceErrorBoundary } from "./WorkspaceErrorBoundary";
import { useStudioStore } from "../stores/studioStore";
import type { ReusableGenerationDraft } from "../types/history";
import type { StudioAssetType } from "../types/generation";
import type { ProjectView } from "../types/project";
import type { ProductionAdmissionStatus } from "../types/productionQueue";
import type { ShotStage } from "../types/shot";
import type {
  ProjectWorkflowBindingInput,
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigView,
} from "../types/projectWorkflow";
import type {
  ConsistencyContextPreview,
  ConsistencyBindingReplaceInput,
  ConsistencyCostumeOption,
  ConsistencyProfileOption,
  ConsistencyReferenceSetOption,
  ConsistencyScopeRef,
} from "../types/consistencyBindings";
import { type Workspace } from "../types/workspaceResume";
import { toUserMessage } from "../i18n/errorMessages";
import { comfyStatusLabel, projectDisplayName } from "../i18n/statusLabels";
import { StartupScreen } from "./StartupScreen";
import { normalCreate } from "../features/create/createModel";
import { ShellHost, readShellMode, SHELL_MODE_KEY } from "./ShellHost";
import { ProjectOverviewPage } from "./v3/ProjectOverviewPage";
import type { StudioBreadcrumbItem } from "../components/studio/StudioTopBar";
import {
  defaultStudioSectionForWorkspace,
  shotWorkspaceModeForSection,
  studioRouteForSection,
  type StudioSection,
} from "./studioNavigation";
import { useAppRoute } from "./routes/useAppRoute";
import { useDraftConfirmation } from "./routes/useDraftConfirmation";
import { fromLegacyLocation, toLegacyLocation, type LegacyLocation } from "./routes/legacyAdapter";
import { readRouteResume, resolveResume, validateResumeChildren } from "./routes/resumeAdapter";
import { productClient } from "../product/client";
import { routeProjectId } from "./routes/types";
import { routeTitle } from "./routes/selectors";
import "./App.css";
import "../styles/studioTokens.css";
import "../styles/uiPolish.css";
import "../styles/studioQuality.css";

const GenerationStudio = lazy(() => import("../features/studio/GenerationStudio").then(({ GenerationStudio }) => ({ default: GenerationStudio })));
const AssetWorkspace = lazy(() => import("../features/assets/AssetWorkspace").then(({ AssetWorkspace }) => ({ default: AssetWorkspace })));
const PromptStudio = lazy(() => import("../features/prompts/PromptStudio").then(({ PromptStudio }) => ({ default: PromptStudio })));
const LocalToolHub = lazy(() => import("../features/tools/LocalToolHub").then(({ LocalToolHub }) => ({ default: LocalToolHub })));
const AssetVideoBatchWorkspace = lazy(() => import("../features/assets/AssetVideoBatchWorkspace").then(({ AssetVideoBatchWorkspace }) => ({ default: AssetVideoBatchWorkspace })));
const TaskHistory = lazy(() => import("../features/tasks/TaskHistory").then(({ TaskHistory }) => ({ default: TaskHistory })));
const ProjectWorkspace = lazy(() => import("../features/projects/ProjectWorkspace").then(({ ProjectWorkspace }) => ({ default: ProjectWorkspace })));
const ProjectCommandCenter = lazy(() => import("../features/projects/ProjectCommandCenter").then(({ ProjectCommandCenter }) => ({ default: ProjectCommandCenter })));
const WorkflowWorkspace = lazy(() => import("../features/workflows/WorkflowWorkspace").then(({ WorkflowWorkspace }) => ({ default: WorkflowWorkspace })));
const SettingsWorkspace = lazy(() => import("../features/settings/SettingsWorkspace").then(({ SettingsWorkspace }) => ({ default: SettingsWorkspace })));
const CreatePage = lazy(() => import("../features/create/CreatePage").then(({ CreatePage }) => ({ default: CreatePage })));
const ShotWorkspace = lazy(() => import("../features/shots/ShotWorkspace").then(({ ShotWorkspace }) => ({ default: ShotWorkspace })));

export function workflowUseProjectDestination(
  catalog: readonly Pick<RecipeViewModel, "workflowId" | "recipeId">[],
  workflowId: string,
  recipeId: string,
): "projects" | undefined {
  return catalog.some((recipe) => recipe.workflowId === workflowId && recipe.recipeId === recipeId)
    ? "projects"
    : undefined;
}

export { resolveProjectCommandCenterNavigation } from "./routes/commandCenterAdapter";
import { resolveProjectCommandCenterNavigation } from "./routes/commandCenterAdapter";

export type WorkflowDefaultStage = "IMAGE" | "VIDEO";
export type WorkflowDefaultSelection = WorkflowDefaultStage | "DUAL" | "NONE";

export function workflowDefaultSelection(recipe: Pick<RecipeViewModel, "outputTypes">): WorkflowDefaultSelection {
  const hasImage = recipe.outputTypes?.includes("image") ?? false;
  const hasVideo = recipe.outputTypes?.includes("video") ?? false;
  if (hasImage && hasVideo) return "DUAL";
  if (hasImage) return "IMAGE";
  if (hasVideo) return "VIDEO";
  return "NONE";
}

function bindingInput(binding: ProjectWorkflowBindingView): ProjectWorkflowBindingInput {
  return {
    stage: binding.stage,
    mode: binding.mode,
    workflowVersionId: binding.workflowVersionId,
    recipeId: binding.recipeId,
  };
}

export function projectWorkflowBindingsForRecipe(
  config: ProjectWorkflowConfigView,
  stage: WorkflowDefaultStage,
  recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">,
): ProjectWorkflowBindingInput[] {
  const replacement: ProjectWorkflowBindingInput = {
    stage,
    mode: "DEFAULT",
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
  };
  return [
    stage === "IMAGE" ? replacement : config.imageDefault ? bindingInput(config.imageDefault) : undefined,
    stage === "VIDEO" ? replacement : config.videoDefault ? bindingInput(config.videoDefault) : undefined,
    ...config.videoModeOverrides.map(bindingInput),
  ].filter((binding): binding is ProjectWorkflowBindingInput => Boolean(binding));
}

function keepsNativeContextMenu(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || target.closest('[contenteditable="true"]') !== null;
}

function App() {
  const { confirm: confirmDraftDiscard, dialog: draftConfirmation } = useDraftConfirmation();
  const { route, navigate: dispatchNavigate, restore, back, switchProject } = useAppRoute();
  const [shellMode, setShellMode] = useState(readShellMode);
  const location = useMemo(() => toLegacyLocation(route), [route]);
  const workspace = location.workspace;
  const activeStudioSection = location.section ?? defaultStudioSectionForWorkspace(workspace);
  const resumeShotId = location.shotId;
  const focusedTaskId = location.taskId;
  const focusedProductionBatchId = location.batchId;
  const focusedAssetId = location.assetId;
  const focusedCollectionFilter = location.collectionFilter;
  const [shotDraftDirty, setShotDraftDirty] = useState(false);
  const [videoBatchAssets, setVideoBatchAssets] = useState<AssetView[]>([]);
  const [bootstrapState, setBootstrapState] = useState<BootstrapState | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [startupAttempt, setStartupAttempt] = useState(0);
  const [catalog, setCatalog] = useState<RecipeViewModel[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [taskEventsReady, setTaskEventsReady] = useState(false);
  const [taskEventError, setTaskEventError] = useState<string | undefined>();
  const [connectionLoading, setConnectionLoading] = useState(false);
  const [capabilityLoading, setCapabilityLoading] = useState(false);
  const [reconciling, setReconciling] = useState(false);
  const [recoveryNotice, setRecoveryNotice] = useState<string | null>(null);
  const [workflowNotice, setWorkflowNotice] = useState<string | null>(null);
  const [projectContextLoading, setProjectContextLoading] = useState(false);
  const [consistencyProfiles, setConsistencyProfiles] = useState<ConsistencyProfileOption[]>([]);
  const [consistencyReferenceSets, setConsistencyReferenceSets] = useState<ConsistencyReferenceSetOption[]>([]);
  const [consistencyCostumes, setConsistencyCostumes] = useState<Record<string, ConsistencyCostumeOption[]>>({});
  const [consistencyLoading, setConsistencyLoading] = useState(false);
  const [consistencyError, setConsistencyError] = useState<string>();
  const [productionAdmission, setProductionAdmission] = useState<ProductionAdmissionStatus>({ busy: false });
  const projects = useProjectStore((state) => state.projects);
  const activeProjectId = routeProjectId(route);
  const activeProject = projects.find((project) => project.id === activeProjectId);
  const projectLoading = useProjectStore((state) => state.loading);
  const projectError = useProjectStore((state) => state.error);
  const setProjects = useProjectStore((state) => state.setProjects);
  const setProjectLoading = useProjectStore((state) => state.setLoading);
  const setProjectError = useProjectStore((state) => state.setError);
  const setRecentTasks = useTaskStore((state) => state.setRecentTasks);
  const recentTasks = useTaskStore((state) => state.recentTasks);
  const loadWorkspaceResume = useWorkspaceResumeStore((state) => state.load);
  useEffect(() => {
    if (activeProjectId) useProjectStore.getState().setActiveProject(activeProjectId);
  }, [activeProjectId]);

  const refreshProductionAdmission = useCallback(async () => {
    try {
      setProductionAdmission(await getProductionAdmissionStatus());
    } catch (admissionError: unknown) {
      setError(toUserMessage(admissionError));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let admissionRefreshTimer: number | undefined;

    void subscribeTaskUpdates((task) => {
      if (admissionRefreshTimer !== undefined) window.clearTimeout(admissionRefreshTimer);
      admissionRefreshTimer = window.setTimeout(() => void refreshProductionAdmission(), 1_000);
      const currentProjectId = useProjectStore.getState().activeProjectId;
      if (!currentProjectId || task.projectId !== currentProjectId) return;
      useTaskStore.getState().upsertTask(task);
    })
      .then((cleanup) => {
        if (cancelled) cleanup();
        else {
          unlisten = cleanup;
          setTaskEventsReady(true);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setTaskEventsReady(false);
          setTaskEventError("任务事件通道不可用");
        }
      });

    void Promise.all([bootstrap(), listGenerationCatalog()])
      .then(([state, recipes]) => {
        if (!cancelled) {
          setBootstrapState(state);
          setStartupError(null);
          setCatalog(recipes);
          void getRuntimeActivityStatus()
            .then((activity) => {
              if (cancelled || (activity.activeTaskCount === 0 && !activity.productionBusy)) return;
              setRecoveryNotice("正在同步上次未完成的任务……");
              return reconcileActiveTasks().then((report) => {
                if (!cancelled) setRecoveryNotice(`已同步 ${report.examined} 个任务。`);
              });
            })
            .catch(() => {
              if (!cancelled) setRecoveryNotice("上次任务状态暂时无法确认，请稍后重新同步。");
            });
        }
      })
      .catch((bootstrapError: unknown) => {
        if (!cancelled) {
          setStartupError(toUserMessage(bootstrapError));
        }
      });

    return () => {
      cancelled = true;
      if (admissionRefreshTimer !== undefined) window.clearTimeout(admissionRefreshTimer);
      unlisten?.();
    };
  }, [refreshProductionAdmission, startupAttempt]);

  useEffect(() => {
    void refreshProductionAdmission();
  }, [refreshProductionAdmission]);

  useEffect(() => {
    let cancelled = false;
    setProjectLoading(true);
    void Promise.all([listProjects(), loadWorkspaceResume()])
      .then(async ([nextProjects, resume]) => {
        if (cancelled) return;
        const initial = resolveResume(readRouteResume(), resume, nextProjects.map((project) => project.id));
        const initialProjectId = routeProjectId(initial);
        setProjects(nextProjects, initialProjectId ?? "");
        const checked = await validateResumeChildren(initial, {
          shotIds: async (projectId) => (await listShots(projectId)).map((shot) => shot.id),
          runExists: async (projectId, run) => { await productClient.run.get(projectId, run); },
          assetExists: async (projectId, assetId) => { await getAsset(projectId, assetId); },
        }).catch((resumeError: unknown) => {
          if (!cancelled) setError(toUserMessage(resumeError));
          return initial;
        });
        if (!cancelled) restore(checked);
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          const message = toUserMessage(loadError);
          setProjectError(message);
          setError(message);
        }
      })
      .finally(() => {
        if (!cancelled) setProjectLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [loadWorkspaceResume, setProjectError, setProjectLoading, setProjects, restore]);

  useEffect(() => {
    setVideoBatchAssets([]);
  }, [activeProjectId]);

  useEffect(() => {
    if (!activeProjectId) return;
    const requestedProjectId = activeProjectId;
    let cancelled = false;
    setProjectContextLoading(true);
    void listRecentTasks(requestedProjectId, 10)
      .then((tasks) => {
        if (!cancelled && useProjectStore.getState().activeProjectId === requestedProjectId) {
          setRecentTasks(tasks);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled && useProjectStore.getState().activeProjectId === requestedProjectId) {
          setError(toUserMessage(loadError));
        }
      })
      .finally(() => {
        if (!cancelled) setProjectContextLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeProjectId, setRecentTasks]);

  useEffect(() => {
    if (!activeProjectId) {
      setConsistencyProfiles([]);
      setConsistencyReferenceSets([]);
      setConsistencyCostumes({});
      setConsistencyError(undefined);
      setConsistencyLoading(false);
      return;
    }
    const requestedProjectId = activeProjectId;
    let cancelled = false;
    setConsistencyLoading(true);
    setConsistencyError(undefined);
    void Promise.all([
      listConsistencyProfiles(requestedProjectId),
      listReferenceSets(requestedProjectId),
    ])
      .then(async ([profiles, referenceSets]) => {
        const characterProfiles = profiles.filter((profile) => profile.profileType === "CHARACTER");
        const costumeEntries = await Promise.all(
          characterProfiles.map(async (profile) => [
            profile.id,
            await listCostumeVariants(requestedProjectId, profile.id).catch(() => []),
          ] as const),
        );
        if (cancelled) return;
        setConsistencyProfiles(profiles.map((profile) => ({
          id: profile.id,
          projectId: profile.projectId,
          profileType: profile.profileType,
          name: profile.name,
          description: profile.description,
        })));
        setConsistencyReferenceSets(referenceSets.map((referenceSet) => ({
          id: referenceSet.id,
          projectId: referenceSet.projectId,
          name: referenceSet.name,
          purpose: referenceSet.purpose,
          itemCount: referenceSet.itemCount,
          imageCount: referenceSet.imageCount,
        })));
        setConsistencyCostumes(Object.fromEntries(costumeEntries.map(([profileId, costumes]) => [
          profileId,
          costumes.map((costume) => ({
            id: costume.id,
            characterProfileId: costume.characterProfileId,
            name: costume.name,
            promptFragment: costume.promptFragment,
            referenceSetId: costume.referenceSetId,
            isDefault: costume.isDefault,
          })),
        ])));
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setConsistencyError(toUserMessage(loadError));
      })
      .finally(() => {
        if (!cancelled) setConsistencyLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeProjectId]);

  const loadConsistencyBindingPack = useCallback(async (scope: ConsistencyScopeRef) => {
    if (scope.scopeType === "SHOT") {
      return getShotConsistencyBinding(activeProjectId ?? scope.scopeId, scope.scopeId);
    }
    return getConsistencyScopeBinding(activeProjectId ?? scope.scopeId, scope.scopeType, scope.scopeId);
  }, [activeProjectId]);

  const saveConsistencyBindingPack = useCallback(async (input: ConsistencyBindingReplaceInput) => {
    if (input.scopeType === "SHOT") {
      await replaceShotConsistencyBinding(input);
    } else {
      await replaceConsistencyScopeBinding(input);
    }
  }, []);

  const loadConsistencyContext = useCallback(async (scope: ConsistencyScopeRef, stage: ShotStage): Promise<ConsistencyContextPreview | null> => {
    if (scope.scopeType !== "SHOT") {
      return null;
    }
    return getShotContextDraft(activeProjectId ?? scope.scopeId, scope.scopeId, stage);
  }, [activeProjectId]);

  async function allowProjectSwitch(projectId?: string) {
    if (!projectId || projectId === activeProjectId) return true;
    const studio = useStudioStore.getState();
    if (studio.draftDirty && !await confirmDraftDiscard("当前项目有未保存的创作草稿。切换项目会放弃该草稿，是否继续？")) return false;
    studio.resetDraft();
    useTaskStore.getState().clear();
    setVideoBatchAssets([]);
    useProjectStore.getState().setActiveProject(projectId);
    return true;
  }
  async function navigate(next: import("./routes/types").AppRoute) {
    const nextLegacy = toLegacyLocation(next);
    if (shotDraftDirty && (nextLegacy.workspace !== "shots" || nextLegacy.projectId !== activeProjectId || nextLegacy.shotId !== resumeShotId || nextLegacy.section !== activeStudioSection)) {
      if (!await confirmDraftDiscard("镜头有未保存的修改。离开会放弃这些修改，是否继续？")) return false;
    }
    if (!await allowProjectSwitch(routeProjectId(next))) return false;
    dispatchNavigate(next);
    return true;
  }
  function navigateLegacy(target: LegacyLocation) {
    const projectId = target.projectId ?? activeProjectId ?? useProjectStore.getState().activeProjectId;
    if (projectId && !projects.some((project) => project.id === projectId)) {
      setError("目标项目不存在或已不可用。");
      return;
    }
    // Explicit user navigation event, not an effect syncing legacy state back.
    const next = fromLegacyLocation({ ...target, projectId });
    return navigate(next.kind === "system-settings" ? { ...next, returnTo: route.kind === "system-settings" ? route.returnTo : route } : next);
  }

  function navigateToWorkspace(nextWorkspace: Workspace) {
    navigateLegacy({ workspace: nextWorkspace });
  }

  function navigateToStudioSection(section: StudioSection) {
    const target = studioRouteForSection(section);
    navigateLegacy(target);
  }

  function openTask(taskId: string) { navigateLegacy({ workspace: "tasks", section: "review", taskId }); }
  function openShot(shotId: string, section: StudioSection = "creation") { navigateLegacy({ workspace: "shots", section, shotId }); }
  function handleShotSelected(shotId?: string) {
    if (route.kind === "create") navigate({ ...route, shotId });
  }
  async function openProject(projectId: string, destination?: Workspace, section?: StudioSection) {
    if (!projects.some((project) => project.id === projectId)) return;
    if (destination) { await navigateLegacy({ projectId, workspace: destination, section }); return; }
    if (shotDraftDirty && projectId !== activeProjectId && !await confirmDraftDiscard("镜头有未保存的修改。切换项目会放弃这些修改，是否继续？")) return;
    if (!await allowProjectSwitch(projectId)) return;
    setError(null);
    switchProject(projectId);
  }
  function openProductionQueue() {
    navigateLegacy({ projectId: productionAdmission.projectId, workspace: "shots", section: "production", batchId: productionAdmission.batchId });
  }
  function openProductionQueueFromShot(batchId?: string) {
    navigateLegacy({ workspace: "shots", section: "production", batchId });
  }

  async function reconnectComfy() {
    setConnectionLoading(true);
    setError(null);
    try {
      const comfy = await getComfyStatus();
      setBootstrapState((current) => (current ? { ...current, comfy } : current));
    } catch (connectionError: unknown) {
      setError(toUserMessage(connectionError));
    } finally {
      setConnectionLoading(false);
    }
  }

  function retryStartup() {
    setBootstrapState(null);
    setStartupError(null);
    setError(null);
    setStartupAttempt((attempt) => attempt + 1);
  }

  async function refreshCapabilities() {
    setCapabilityLoading(true);
    setError(null);
    try {
      const capability = await refreshComfyCapabilities();
      setBootstrapState((current) =>
        current ? { ...current, comfy: { ...current.comfy, capability } } : current,
      );
    } catch (refreshError: unknown) {
      setError(toUserMessage(refreshError));
    } finally {
      setCapabilityLoading(false);
    }
  }

  async function refreshRuntimeAfterEndpoint() {
    try {
      const [nextComfy, nextCatalog] = await Promise.all([getComfyStatus(), listGenerationCatalog()]);
      setBootstrapState((current) => (current ? { ...current, comfy: nextComfy } : current));
      setCatalog(nextCatalog);
    } catch (refreshError: unknown) {
      setError(toUserMessage(refreshError));
    }
  }

  async function reloadCatalog() {
    setCatalog(await listGenerationCatalog());
  }

  async function openPublishedWorkflow(workflowId: string, recipeId: string) {
    try {
      const nextCatalog = await listGenerationCatalog();
      setCatalog(nextCatalog);
      const workflow = nextCatalog.find((recipe) => recipe.workflowId === workflowId && recipe.recipeId === recipeId)
        ?? nextCatalog.find((recipe) => recipe.workflowId === workflowId);
      if (!workflow) {
        setError("发布的工作流暂时还没有出现在运行目录中。");
        return;
      }
      useStudioStore.getState().setSelectedWorkflow(workflow);
      navigateToWorkspace("studio");
      setError(null);
    } catch (openError: unknown) {
      setError(toUserMessage(openError));
    }
  }

  async function openWorkflowForProject(workflowId: string, recipeId: string) {
    setError(null);
    setWorkflowNotice(null);
    try {
      const nextCatalog = await listGenerationCatalog();
      setCatalog(nextCatalog);
      const recipe = nextCatalog.find((candidate) => candidate.workflowId === workflowId && candidate.recipeId === recipeId);
      if (!recipe) {
        setError("刚添加的工作流暂时还没有出现在项目工作流列表中，请刷新后重试。");
        return;
      }
      const currentProject = useProjectStore.getState().activeProject();
      if (!currentProject) {
        setError("当前项目不可用，无法绑定工作流。");
        return;
      }

      const selection = workflowDefaultSelection(recipe);
      if (selection === "NONE") {
        setError("该工作流没有图片或视频输出，无法绑定为项目默认工作流。");
        return;
      }
      if (selection === "DUAL") {
        openProject(currentProject.id, "projects");
        setWorkflowNotice("该工作流同时输出图片和视频，未自动绑定。请在项目工作流设置中明确选择图片或视频默认工作流。");
        return;
      }

      const currentConfig = await getProjectWorkflowConfig(currentProject.id);
      const currentBinding = selection === "IMAGE" ? currentConfig.imageDefault : currentConfig.videoDefault;
      const nextConfig = await upsertProjectWorkflowBinding(currentProject.id, {
        stage: selection,
        mode: "DEFAULT",
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        expectedBindingInstanceId: currentBinding?.bindingInstanceId ?? null,
        expectedRevision: currentBinding?.revision ?? null,
      });
      const confirmedBinding = selection === "IMAGE" ? nextConfig.imageDefault : nextConfig.videoDefault;
      if (
        !confirmedBinding
        || confirmedBinding.workflowVersionId !== recipe.workflowVersionId
        || confirmedBinding.recipeId !== recipe.recipeId
      ) {
        setError("项目工作流绑定写入后校验失败，未确认目标工作流，请重试。");
        return;
      }

      openProject(currentProject.id, "projects");
      setWorkflowNotice(
        `已设为当前项目${selection === "IMAGE" ? "图片" : "视频"}默认工作流：${recipe.name}`,
      );
      setError(null);
    } catch (openError: unknown) {
      setError(toUserMessage(openError));
    }
  }

  async function reconcileTasks() {
    if (!activeProjectId) return;
    setReconciling(true);
    setRecoveryNotice(null);
    try {
      const report = await reconcileActiveTasks();
      setRecentTasks(await listRecentTasks(activeProjectId, 10));
      setRecoveryNotice(
        `已检查 ${report.examined} 个任务：${report.succeeded} 个已更新，${report.deferred} 个等待后续同步，${report.unresolved} 个状态未确定。`,
      );
    } catch (recoveryError: unknown) {
      setRecoveryNotice(toUserMessage(recoveryError));
    } finally {
      setReconciling(false);
    }
  }

  function loadHistoricalInputs(draft: ReusableGenerationDraft) {
    if (!activeProjectId || draft.projectId !== activeProjectId) {
      setError("当前任务属于其他项目，请先切换到对应项目。");
      return;
    }
    const workflow = catalog.find(
      (recipe) =>
        recipe.workflowVersionId === draft.workflowVersionId && recipe.recipeId === draft.recipeId,
    );
    if (!workflow) {
      setError("当前工作流版本已不在运行目录中，请刷新工作流列表。");
      return;
    }
    useStudioStore.getState().loadDraft(
      workflow,
      draft.values,
      draft.modelVersionId ?? undefined,
      draft.promptVersionId ?? undefined,
    );
    useStudioStore.getState().setReuseProvenance({
      workflowName: draft.workflowName,
      createdAt: draft.createdAt,
    });
    setError(null);
    navigateToWorkspace("studio");
  }

  function useAssetInStudio(asset: AssetView) {
    if (!activeProjectId) return;
    const assetType = asset.assetType === "video" || asset.category.endsWith("_video")
      ? "video"
      : asset.assetType === "audio" || asset.category === "source_audio"
        ? "audio"
        : "image";
    useStudioStore.getState().setPendingAssetIntent({
      projectId: activeProjectId,
      assetId: asset.id,
      assetType: assetType as StudioAssetType,
    });
    setError(null);
    if (shellMode === "v3") void navigate({ kind: "create", projectId: activeProjectId, stage: assetType === "image" ? "image" : "video" });
    else navigateToWorkspace("studio");
  }

  function handleProjectUpdated(project: ProjectView) {
    useProjectStore.getState().upsertProject(project);
    setError(null);
  }

  function handleProjectRestored(project: ProjectView) {
    useProjectStore.getState().upsertProject(project);
    navigate(fromLegacyLocation({ projectId: project.id, workspace: "shots", section: "creation" }));
  }

  async function handleTemplateProjectCreated(result: TemplateProjectResult) {
    useProjectStore.getState().upsertProject(result.project);
    if (!await navigate({ kind: "create", projectId: result.project.id, stage: "image", surface: "batch" })) return;
    const workflow = catalog.find((item) => item.workflowVersionId === result.workflowVersionId && item.recipeId === result.recipeId);
    if (!workflow) {
      setError("模板项目已创建，但工作流当前不可用。");
      return;
    }
    useStudioStore.getState().loadDraft(workflow, result.values);
    setError(null);
  }

  function openVideoBatch(assets: AssetView[]) {
    setVideoBatchAssets(assets);
    navigateToWorkspace("video");
    setError(null);
  }

  function openAssetFromShot(assetId: string) { navigateLegacy({ workspace: "assets", assetId }); }
  function openShotFromAsset(shotId: string) { openShot(shotId); }
  function navigateFromCommandCenter(request: ProjectCommandCenterNavigationRequest) {
    navigateLegacy(resolveProjectCommandCenterNavigation(request));
  }

  const comfy = bootstrapState?.comfy;
  const isConnected = comfy?.status === "CONNECTED";
  const hasComfyCapabilityIssue = isConnected && !comfy?.capability;
  const showComfyWarning = Boolean(comfy && (!isConnected || hasComfyCapabilityIssue));
  const hasActiveTasks = recentTasks.some((task) =>
    ["CREATED", "VALIDATING", "PREPARING", "QUEUED", "RUNNING", "CANCEL_REQUESTED", "COLLECTING"]
    .includes(task.status),
  );


  const breadcrumbs: StudioBreadcrumbItem[] = activeProject
    ? [{ label: projectDisplayName(activeProject.id, activeProject.name), onClick: () => navigate({ kind: "project", projectId: activeProject.id, page: "overview" }) }, { label: routeTitle(route), current: true }]
    : [{ label: routeTitle(route), current: true }];

  const projectSelector = (
    <select
      aria-label="当前项目"
      value={activeProjectId ?? ""}
      onChange={(event) => openProject(event.target.value)}
      disabled={projectLoading || !projects.length || projectContextLoading}
    >
      {!activeProjectId && <option value="">{projectLoading ? "正在加载项目..." : "选择项目"}</option>}
      {projects.map((project) => <option key={project.id} value={project.id}>{projectDisplayName(project.id, project.name)}</option>)}
    </select>
  );

  if (!bootstrapState) {
    return <StartupScreen error={startupError} onRetry={retryStartup} />;
  }

  return (
    <div
      className="studio-context-guard"
      onContextMenu={(event) => {
        if (!keepsNativeContextMenu(event.target)) event.preventDefault();
      }}
    >
      {draftConfirmation}
      <ShellHost
        mode={shellMode}
        onModeChange={async (mode) => {
          if (shotDraftDirty && !await confirmDraftDiscard("创作有未保存的修改。切换界面会放弃这些修改，是否继续？")) return;
          setShellMode(mode);
          try { localStorage.setItem(SHELL_MODE_KEY, mode); } catch { /* Optional local preference. */ }
        }}
        route={route}
        navigate={navigate}
        back={async () => { if (!shotDraftDirty || await confirmDraftDiscard("镜头有未保存的修改。返回会放弃这些修改，是否继续？")) back(); }}
        className={`app-workspace-${workspace}`}
        workspace={workspace}
        project={activeProject ? { id: activeProject.id, name: projectDisplayName(activeProject.id, activeProject.name) } : undefined}
        projectSelector={projectSelector}
        comfyStatus={comfy}
        comfyLoading={connectionLoading}
        breadcrumbs={breadcrumbs}
        currentSection={activeStudioSection}
        onNavigate={(_destination, item) => {
          if (item.id === "production") {
            openProductionQueue();
            return;
          }
          navigateToStudioSection(item.id);
        }}
        onSearch={() => navigateToStudioSection("creation")}
        searchLabel="搜索镜头 / 场景"
        searchShortcut="Ctrl K"
        onSettings={() => navigateToStudioSection("settings")}
        onBrandClick={() => navigateToStudioSection("project")}
      >
        <div className="app-main-content" id="app-main-content" tabIndex={-1}>

      {(workspace === "studio" || workspace === "video") && productionAdmission.busy && (
        <section className="production-admission-banner" role="status" aria-live="polite">
          <div>
            <span className="section-label">生产队列正在运行</span>
            <strong>{productionAdmission.batchName ?? "生产队列"}</strong>
            <p>当前 GPU 正在执行生产任务{productionAdmission.activeTaskId ? `（任务 ${productionAdmission.activeTaskId}）` : ""}；新的生成任务暂时不可提交。服务端执行准入会保持 GPU 串行。</p>
          </div>
          <button
            type="button"
            className="quiet-button"
            onClick={openProductionQueue}
            disabled={!productionAdmission.batchId || !productionAdmission.projectId}
          >
            查看队列
          </button>
        </section>
      )}

      {projectContextLoading && activeProject && (
        <p className="project-loading" role="status">正在加载项目...</p>
      )}
      {workspace !== "settings" && workspace !== "prompts" && workspace !== "tools" && showComfyWarning && (
        <section className="comfy-status-warning" role="status" aria-live="polite">
          <div>
            <span className="section-label">运行环境提醒</span>
            <strong>ComfyUI {comfyStatusLabel(comfy?.status)}</strong>
            <p>{hasComfyCapabilityIssue ? "已连接但节点能力尚未确认，请先完成运行时预检。" : "当前生成工作区不可用，请检查运行时端点。"}</p>
          </div>
          <button type="button" className="quiet-button" onClick={() => navigateToStudioSection("settings")}>打开设置</button>
        </section>
      )}

      {(hasActiveTasks || recoveryNotice) && (
        <section className="task-recovery-bar" aria-live="polite">
          <div>
            <span className="section-label">任务恢复</span>
            <p>{recoveryNotice ?? "启动后检测到尚未结束的任务。"}</p>
          </div>
          <button type="button" onClick={() => void reconcileTasks()} disabled={reconciling}>
            {reconciling ? "正在同步..." : "重新同步任务"}
          </button>
        </section>
      )}

      {!activeProject && projectError && <p className="error-message global-error">项目加载失败：{projectError}</p>}
      <Suspense fallback={<p className="workspace-loading" role="status">正在加载工作区...</p>}>
        {workspace === "command-center" && (
          <WorkspaceErrorBoundary
            resetKey={activeProject?.id ?? "no-project"}
            onBackToAssets={() => navigateToWorkspace("assets")}
            onRetry={() => navigateToWorkspace("command-center")}
          >
            {shellMode === "v3" && activeProject ? <ProjectOverviewPage key={activeProject.id} projectId={activeProject.id} navigate={navigate} /> : <ProjectCommandCenter project={activeProject} onNavigate={navigateFromCommandCenter} />}
          </WorkspaceErrorBoundary>
        )}
        {activeProject && workspace === "studio" && (
          <section className="studio-layout">
            <GenerationStudio
              projectId={activeProject.id}
              catalog={catalog}
              comfyConnected={isConnected}
              taskEventsReady={taskEventsReady}
              taskEventError={taskEventError}
              productionAdmission={productionAdmission}
              focusProductionBatchId={focusedProductionBatchId}
              onCatalogChanged={reloadCatalog}
              onProductionAdmissionChanged={refreshProductionAdmission}
              onProductionBatchFocused={() => undefined}
              onOpenWorkflows={() => navigateToWorkspace("workflows")}
              onReconnectComfy={() => void reconnectComfy()}
              onOpenTask={(taskId) => {
                openTask(taskId);
              }}
              onOpenProductionQueue={(batchId) => {
                openProductionQueueFromShot(batchId);
              }}
            />
          </section>
        )}
        {activeProject && workspace === "assets" && (
          <AssetWorkspace
            projectId={activeProject.id}
            initialAssetId={focusedAssetId}
            onUseInStudio={useAssetInStudio}
            onOpenVideoBatch={openVideoBatch}
            onOpenTask={(taskId) => {
              openTask(taskId);
            }}
            onOpenShot={openShotFromAsset}
          />
        )}
        {activeProject && workspace === "prompts" && (
          <PromptStudio
            projectId={activeProject.id}
            onOpenTaskHistory={() => navigateToWorkspace("tasks")}
          />
        )}
        {workspace === "tools" && <LocalToolHub />}
        {activeProject && workspace === "shots" && (
          <WorkspaceErrorBoundary
            resetKey={activeProject.id}
            onBackToAssets={() => navigateToWorkspace("assets")}
            onRetry={() => navigateToWorkspace("shots")}
          >
            {normalCreate(route, shellMode) && route.kind === "create" ? <CreatePage key={activeProject.id} route={route} navigate={navigate} onDirtyChange={setShotDraftDirty} /> : <ShotWorkspace
              projectId={activeProject.id}
              projectName={activeProject.name}
              catalog={catalog}
              initialSelectedShotId={resumeShotId}
              showCreationLanding={route.kind === "create"}
              onDraftDirtyChange={setShotDraftDirty}
              onStageSelected={(stage) => { if (route.kind === "create") dispatchNavigate({ ...route, stage }); }}
              initialCollectionFilter={focusedCollectionFilter}
              mode={shotWorkspaceModeForSection(activeStudioSection)}
              onShotSelected={handleShotSelected}
              onOpenAsset={openAssetFromShot}
              onOpenTask={(taskId) => {
                openTask(taskId);
              }}
              onNavigate={navigateFromCommandCenter}
              focusProductionBatchId={focusedProductionBatchId}
              onOpenProductionQueue={openProductionQueueFromShot}
              comfyStatus={comfy}
              capabilityLoading={capabilityLoading}
              onRefreshComfyCapabilities={() => void refreshCapabilities()}
              onOpenSettings={() => navigateToStudioSection("settings")}
              consistencyWorkspace={{
                profiles: consistencyProfiles,
                referenceSets: consistencyReferenceSets,
                costumesByCharacter: consistencyCostumes,
                loading: consistencyLoading,
                error: consistencyError,
                loadBindingPack: loadConsistencyBindingPack,
                onSaveBindingPack: saveConsistencyBindingPack,
                loadContext: loadConsistencyContext,
                onOpenAssets: () => navigateToWorkspace("assets"),
              }}
            />}
          </WorkspaceErrorBoundary>
        )}
        {activeProject && workspace === "video" && (
          <WorkspaceErrorBoundary
            resetKey={`${activeProject.id}:${videoBatchAssets.map((asset) => asset.id).join(",")}`}
            onBackToAssets={() => navigateToWorkspace("assets")}
            onRetry={() => {
              setVideoBatchAssets([]);
              navigateToWorkspace("video");
            }}
          >
            <AssetVideoBatchWorkspace
              projectId={activeProject.id}
              catalog={catalog}
              initialAssets={videoBatchAssets}
              comfyConnected={isConnected}
              taskEventsReady={taskEventsReady}
              productionAdmission={productionAdmission}
              focusProductionBatchId={focusedProductionBatchId}
              onAdmissionChanged={refreshProductionAdmission}
              onProductionBatchFocused={() => undefined}
              onOpenTask={(taskId) => {
                openTask(taskId);
              }}
              onOpenProductionQueue={(batchId) => {
                openProductionQueueFromShot(batchId);
              }}
              onBackToAssets={() => navigateToWorkspace("assets")}
              onOpenWorkflows={() => navigateToWorkspace("workflows")}
            />
          </WorkspaceErrorBoundary>
        )}
        {activeProject && workspace === "tasks" && (
          <TaskHistory
            projectId={activeProject.id}
            comfyConnected={isConnected}
            productionBusy={productionAdmission.busy}
            focusTaskId={focusedTaskId}
            initialFilter={focusedCollectionFilter?.kind === "tasks" ? focusedCollectionFilter.status : undefined}
            onLoadInputs={loadHistoricalInputs}
            onOpenShot={(shotId) => openShot(shotId)}
          />
        )}
        {workspace === "projects" && (
          <ProjectWorkspace
            projects={projects}
            activeProjectId={activeProjectId}
            catalog={catalog}
            onOpen={openProject}
            onProjectUpdated={handleProjectUpdated}
            onProjectRestored={handleProjectRestored}
            onTemplateProjectCreated={handleTemplateProjectCreated}
          />
        )}
        {workspace === "workflows" && (
          <WorkflowWorkspace
            projectId={activeProject?.id}
            catalog={catalog}
            comfyConnected={isConnected}
            onCatalogChanged={reloadCatalog}
            onOpenStudio={openPublishedWorkflow}
            onUseInProject={openWorkflowForProject}
            onOpenProjectSettings={() => navigateToWorkspace("projects")}
            onOpenTask={(taskId) => {
              openTask(taskId);
            }}
          />
        )}
        {workspace === "settings" && (
          <SettingsWorkspace
            comfy={comfy}
            connectionLoading={connectionLoading}
            capabilityLoading={capabilityLoading}
            onReconnect={() => void reconnectComfy()}
            onRefreshCapabilities={() => void refreshCapabilities()}
            onEndpointApplied={() => void refreshRuntimeAfterEndpoint()}
          />
        )}
      </Suspense>

      {taskEventError && <p className="error-message global-error">{taskEventError}</p>}
      {workflowNotice && <p className="workflow-notice" role="status">{workflowNotice}</p>}
      {error && <p className="error-message global-error">提示：{error}</p>}
        </div>
      </ShellHost>
    </div>
  );
}

export default App;
