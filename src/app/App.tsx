import { useProjectTaskRecovery } from "../features/tasks/useProjectTaskRecovery";
import { NormalProductPages } from "./NormalProductPages";
import { useShotConsistencyController } from "../features/shots/useShotConsistencyController";
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import {
  getComfyStatus,
  listRecentTasks,
  getConsistencyScopeBinding,
  getShotConsistencyBinding,
  getShotContextDraft,
  listConsistencyProfiles,
  listCostumeVariants,
  listReferenceSets,
  replaceConsistencyScopeBinding,
  replaceShotConsistencyBinding,

  getAsset,
  getRuntimeActivityStatus,
  getProductionAdmissionStatus,
  listGenerationCatalog,
  listProjects,
  listShots,
  reconcileActiveTasks,
  refreshComfyCapabilities,
  getProjectWorkflowConfig,
  upsertProjectWorkflowBinding,
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
import type {
  ProjectWorkflowBindingInput,
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigView,
} from "../types/projectWorkflow";
import { type Workspace } from "../types/workspaceResume";
import { toUserMessage } from "../i18n/errorMessages";
import { comfyStatusLabel, projectDisplayName } from "../i18n/statusLabels";
import { StartupScreen } from "./StartupScreen";
import { invalidateRuns } from "../product/runInvalidation";
import { ShellHost } from "./ShellHost";
import { ProjectOverviewPage } from "./v3/ProjectOverviewPage";
import {
  defaultStudioSectionForWorkspace,
  studioRouteForSection,
  type StudioSection,
} from "./studioNavigation";
import { useAppRoute } from "./routes/useAppRoute";
import { useDraftConfirmation } from "./routes/useDraftConfirmation";
import { fromLegacyLocation, toLegacyLocation, type LegacyLocation } from "./routes/legacyAdapter";
import { readRouteResume, resolveResume, validateResumeChildren } from "./routes/resumeAdapter";
import { productClient } from "../product/client";
import { routeProjectId, type AppRoute } from "./routes/types";
import { labCreationSelection } from "../services/workflowLabClient";
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
const WorkflowLabPage = lazy(() => import("../features/workflow-lab/WorkflowLabPage").then(({ WorkflowLabPage }) => ({ default: WorkflowLabPage })));
const GeneratorSettingsPage = lazy(() => import("../features/generators/GeneratorSettingsPage").then(({ GeneratorSettingsPage }) => ({ default: GeneratorSettingsPage })));
const SettingsWorkspace = lazy(() => import("../features/settings/SettingsWorkspace").then(({ SettingsWorkspace }) => ({ default: SettingsWorkspace })));
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

// Reuse the existing compatibility facade at its established application importer.
// Stable use-case ports are composition dependencies, not a new IPC/client layer.
const taskRecoveryServices = { listRecentTasks, reconcileActiveTasks };
const shotConsistencyServices = { getConsistencyScopeBinding, getShotConsistencyBinding, getShotContextDraft, listConsistencyProfiles, listCostumeVariants, listReferenceSets, replaceConsistencyScopeBinding, replaceShotConsistencyBinding };

export function preservesCreateDraftForSettings(current: AppRoute, next: AppRoute, savedScope?: string) {
  return current.kind === "create" && next.kind === "system-settings"
    && (next.section === "advanced-workflows" || next.section === "general")
    && next.returnTo?.kind === "create"
    && next.returnTo.projectId === current.projectId && next.returnTo.shotId === current.shotId
    && next.returnTo.stage === current.stage && next.returnTo.surface === current.surface
    && savedScope === `${current.projectId}:${current.shotId ?? ""}:${current.stage}`;
}

function App() {
  const { confirm: confirmDraftDiscard, dialog: draftConfirmation } = useDraftConfirmation();
  const { route, navigate: dispatchNavigate, restore, back, switchProject } = useAppRoute();
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
  const [workflowNotice, setWorkflowNotice] = useState<string | null>(null);
  const [productionAdmission, setProductionAdmission] = useState<ProductionAdmissionStatus>({ busy: false });
  const projects = useProjectStore((state) => state.projects);
  const activeProjectId = routeProjectId(route);
  const activeProject = projects.find((project) => project.id === activeProjectId);
  const { recentTasks, projectContextLoading, reconciling, recoveryNotice, setRecoveryNotice, reconcileTasks } = useProjectTaskRecovery(activeProjectId, setError, taskRecoveryServices);
  const { consistencyProfiles, consistencyReferenceSets, consistencyCostumes, consistencyLoading, consistencyError, loadConsistencyBindingPack, saveConsistencyBindingPack, loadConsistencyContext } = useShotConsistencyController(activeProjectId, shotConsistencyServices);
  const projectLoading = useProjectStore((state) => state.loading);
  const projectError = useProjectStore((state) => state.error);
  const setProjects = useProjectStore((state) => state.setProjects);
  const setProjectLoading = useProjectStore((state) => state.setLoading);
  const setProjectError = useProjectStore((state) => state.setError);
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
      invalidateRuns(task.projectId);
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
          resourceExists: async (projectId, resource) => { await productClient.library.get(projectId, resource); },
          assetExists: async (projectId, assetId) => { await getAsset(projectId, assetId); },
        }).catch((resumeError: unknown) => {
          if (!cancelled) setError(toUserMessage(resumeError));
          return initial;
        });
        if (!cancelled) {
          if (initial.kind === "runs" && initial.run && checked.kind === "runs" && !checked.run) setError("运行已不存在或不可访问。请从列表选择其他运行。");
          if (initial.kind === "library" && initial.resource && checked.kind === "library" && !checked.resource) setError("资源不存在或不可访问，已返回资源列表。");
          restore(checked);
        }
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
    const preserveCreateDraft = preservesCreateDraftForSettings(route, next, useStudioStore.getState().creationLabReturn?.scope);
    if (!preserveCreateDraft && shotDraftDirty && (nextLegacy.workspace !== "shots" || nextLegacy.projectId !== activeProjectId || nextLegacy.shotId !== resumeShotId || nextLegacy.section !== activeStudioSection)) {
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
    void navigate({ kind: "create", projectId: activeProjectId, stage: assetType === "image" ? "image" : "video" });
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
        route={route}
        navigate={navigate}
        back={async () => { if (!shotDraftDirty || await confirmDraftDiscard("镜头有未保存的修改。返回会放弃这些修改，是否继续？")) back(); }}
        projectName={activeProject?.name}
        projectSelector={projectSelector}
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
        <NormalProductPages project={activeProject} route={route} navigate={navigate} onDirtyChange={setShotDraftDirty} />
        {activeProject && (route.kind === "project" || (route.kind === "project-settings" && route.section === "advanced-project")) && (
          <WorkspaceErrorBoundary
            resetKey={activeProject?.id ?? "no-project"}
            onBackToAssets={() => navigateToWorkspace("assets")}
            onRetry={() => navigateToWorkspace("command-center")}
          >
            {route.kind === "project" ? <ProjectOverviewPage key={activeProject.id} projectId={activeProject.id} navigate={navigate} /> : <ProjectCommandCenter project={activeProject} onNavigate={navigateFromCommandCenter} />}
          </WorkspaceErrorBoundary>
        )}
        {activeProject && route.kind === "create" && route.surface === "batch" && route.stage === "image" && (
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
        {activeProject && route.kind === "library" && route.filter === "advanced-assets" && (
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
        {activeProject && route.kind === "library" && route.filter === "advanced-prompts" && (
          <PromptStudio
            projectId={activeProject.id}
            onOpenTaskHistory={() => navigateToWorkspace("tasks")}
          />
        )}
        {route.kind === "system-settings" && route.section === "advanced-tools" && <LocalToolHub />}
        {activeProject && route.kind === "project-settings" && ["advanced-shots", "advanced-production", "advanced-review"].includes(route.section) && (
          <WorkspaceErrorBoundary
            resetKey={activeProject.id}
            onBackToAssets={() => navigateToWorkspace("assets")}
            onRetry={() => navigateToWorkspace("shots")}
          >
            <ShotWorkspace
              projectId={activeProject.id}
              projectName={activeProject.name}
              catalog={catalog}
              initialSelectedShotId={resumeShotId}
              showCreationLanding={false}
              onDraftDirtyChange={setShotDraftDirty}
              initialCollectionFilter={focusedCollectionFilter}
              mode={route.section === "advanced-review" ? "review" : route.section === "advanced-production" ? "production" : "creation"}
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
            />
          </WorkspaceErrorBoundary>
        )}
        {activeProject && route.kind === "create" && route.surface === "batch" && route.stage === "video" && (
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
        {activeProject && route.kind === "project-settings" && route.section === "advanced-tasks" && (
          <TaskHistory
            projectId={activeProject.id}
            comfyConnected={isConnected}
            productionBusy={productionAdmission.busy}
            focusTaskId={focusedTaskId}
            initialAuditTaskId={route.auditTaskId}
            initialFilter={focusedCollectionFilter?.kind === "tasks" ? focusedCollectionFilter.status : undefined}
            onLoadInputs={loadHistoricalInputs}
            onOpenShot={(shotId) => openShot(shotId)}
          />
        )}
        {route.kind === "project-settings" && route.section === "generators" && <GeneratorSettingsPage key={route.projectId} projectId={route.projectId} navigate={navigate} />}
        {(route.kind === "project-list" || (route.kind === "project-settings" && !["generators", "advanced-workflows", "advanced-project", "advanced-shots", "advanced-production", "advanced-review", "advanced-tasks"].includes(route.section))) && (
          <ProjectWorkspace
            showWorkflowSettings={false}
            projects={projects}
            activeProjectId={activeProjectId}
            catalog={catalog}
            onOpen={openProject}
            onProjectUpdated={handleProjectUpdated}
            onProjectRestored={handleProjectRestored}
            onTemplateProjectCreated={handleTemplateProjectCreated}
          />
        )}
        {(route.kind === "system-settings" || route.kind === "project-settings") && route.section === "advanced-workflows" && (
          <WorkflowLabPage
            initialDiagnosticsOpen={route.kind === "system-settings" && route.returnTo?.kind === "system-settings" && route.returnTo.section !== "advanced-tools"}
            projectId={activeProject?.id}
            catalog={catalog}
            comfyConnected={isConnected}
            onCatalogChanged={reloadCatalog}
            onOpenStudio={async (workflowId, recipeId) => {
              const candidates = catalog.filter(item => item.workflowId === workflowId && item.recipeId === recipeId);
              if (!activeProject || candidates.length !== 1) throw new Error("无法确定生成器的确切版本，请刷新后重新选择。");
              const selected = candidates[0];
              const destination = route.kind === "system-settings" && route.returnTo?.kind === "create" ? { ...route.returnTo, stage: selected.outputTypes?.includes("video") ? "video" as const : "image" as const } : { kind: "create" as const, projectId: activeProject.id, stage: selected.outputTypes?.includes("video") ? "video" as const : "image" as const };
              useStudioStore.getState().setPendingRunIntent({ projectId: activeProject.id, stage: destination.stage, selectionRef: labCreationSelection(selected.workflowVersionId, selected.recipeId), values: useStudioStore.getState().values });
              await navigate(destination);
            }}
            onUseInProject={openWorkflowForProject}
            onOpenProjectSettings={() => void navigate({ kind: "project-settings", projectId: activeProject!.id, section: "generators" })}
            onOpenTask={(taskId) => { openTask(taskId); }}
            returnTo={route.kind === "system-settings" ? route.returnTo : route.kind === "project-settings" ? { kind: "project-settings", projectId: route.projectId, section: "generators" } : undefined}
            navigate={navigate}
          />
        )}
        {route.kind === "system-settings" && !["advanced-tools", "advanced-workflows"].includes(route.section) && (
          <SettingsWorkspace
            showWorkflowRepairStatus={true}
            projectId={activeProject?.id}
            initialTaskId={route.returnTo?.kind === "runs" && route.returnTo.run?.source === "task" ? route.returnTo.run.id : undefined}
            onOpenRun={(projectId, taskId) => void navigate({ kind: "runs", projectId, run: { source: "task", id: taskId } })}
            onOpenAudit={(projectId, taskId) => void navigate({ kind: "project-settings", projectId, section: "advanced-tasks", auditTaskId: taskId })}
            onOpenWorkflowDiagnostics={() => void navigate({ kind: "system-settings", section: "advanced-workflows", returnTo: route })}
            onOpenProjectGenerators={(projectId) => void navigate({ kind: "project-settings", projectId, section: "generators" })}
            onOpenToolHub={() => void navigate({ kind: "system-settings", section: "advanced-tools", returnTo: route })}
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
