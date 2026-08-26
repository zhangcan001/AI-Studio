import { useCallback, useEffect, useState } from "react";
import {
  getComfyStatus,
  getRuntimeActivityStatus,
  getProductionAdmissionStatus,
  listGenerationCatalog,
  listProjects,
  listRecentTasks,
  listShots,
  reconcileActiveTasks,
  refreshComfyCapabilities,
} from "../services/tauriClient";
import { subscribeTaskUpdates } from "../services/taskEvents";
import { useTaskStore } from "../stores/taskStore";
import { useProjectStore } from "../stores/projectStore";
import { useWorkspaceResumeStore } from "../stores/workspaceResumeStore";
import type { RecipeViewModel } from "../types/generation";
import type { AssetView } from "../types/asset";
import type { TemplateProjectResult } from "../types/organization";
import { GenerationStudio } from "../features/studio/GenerationStudio";
import { AssetWorkspace } from "../features/assets/AssetWorkspace";
import { AssetVideoBatchWorkspace } from "../features/assets/AssetVideoBatchWorkspace";
import { TaskHistory } from "../features/tasks/TaskHistory";
import { ProjectWorkspace } from "../features/projects/ProjectWorkspace";
import { ProjectCommandCenter, type ProjectCommandDestination } from "../features/projects/ProjectCommandCenter";
import { WorkflowWorkspace } from "../features/workflows/WorkflowWorkspace";
import { SettingsWorkspace } from "../features/settings/SettingsWorkspace";
import { ShotWorkspace, type ShotContextPathItem } from "../features/shots/ShotWorkspace";
import { bootstrap, type BootstrapState } from "./bootstrap";
import { WorkspaceErrorBoundary } from "./WorkspaceErrorBoundary";
import { useStudioStore } from "../stores/studioStore";
import type { ReusableGenerationDraft } from "../types/history";
import type { StudioAssetType } from "../types/generation";
import type { ProjectView } from "../types/project";
import type { ProductionAdmissionStatus } from "../types/productionQueue";
import { resolveWorkspaceNavigation, type Workspace } from "../types/workspaceResume";
import { toUserMessage } from "../i18n/errorMessages";
import { comfyStatusLabel, projectDisplayName } from "../i18n/statusLabels";
import { StartupScreen } from "./StartupScreen";
import { StudioShell } from "./StudioShell";
import type { StudioBreadcrumbItem } from "../components/studio/StudioTopBar";
import {
  defaultStudioSectionForWorkspace,
  shotWorkspaceModeForSection,
  studioRouteForSection,
  type StudioSection,
} from "./studioNavigation";
import "./App.css";
import "../styles/studioTokens.css";
import "../styles/uiPolish.css";

const workspaceLabels: Record<Workspace, string> = {
  "command-center": "项目中心",
  studio: "批量图片",
  video: "批量视频",
  shots: "镜头生产",
  assets: "资产库",
  tasks: "任务",
  projects: "项目",
  workflows: "工作流",
  settings: "设置",
};

function keepsNativeContextMenu(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || target.closest('[contenteditable="true"]') !== null;
}

function App() {
  const [workspace, setWorkspace] = useState<Workspace>("command-center");
  const [activeStudioSection, setActiveStudioSection] = useState<StudioSection>("project");
  const [shotContextPath, setShotContextPath] = useState<ShotContextPathItem[]>([]);
  const [shotContextTarget, setShotContextTarget] = useState<ShotContextPathItem>();
  const [resumeShotId, setResumeShotId] = useState<string>();
  const [videoBatchAssets, setVideoBatchAssets] = useState<AssetView[]>([]);
  const [focusedTaskId, setFocusedTaskId] = useState<string>();
  const [focusedProductionBatchId, setFocusedProductionBatchId] = useState<string>();
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
  const [projectContextLoading, setProjectContextLoading] = useState(false);
  const [productionAdmission, setProductionAdmission] = useState<ProductionAdmissionStatus>({ busy: false });
  const projects = useProjectStore((state) => state.projects);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => state.activeProject());
  const projectLoading = useProjectStore((state) => state.loading);
  const projectError = useProjectStore((state) => state.error);
  const setProjects = useProjectStore((state) => state.setProjects);
  const setProjectLoading = useProjectStore((state) => state.setLoading);
  const setProjectError = useProjectStore((state) => state.setError);
  const setRecentTasks = useTaskStore((state) => state.setRecentTasks);
  const recentTasks = useTaskStore((state) => state.recentTasks);
  const loadWorkspaceResume = useWorkspaceResumeStore((state) => state.load);
  const recordWorkspaceChange = useWorkspaceResumeStore((state) => state.recordWorkspaceChange);
  const recordProjectChange = useWorkspaceResumeStore((state) => state.recordProjectChange);
  const recordShotChange = useWorkspaceResumeStore((state) => state.recordShotChange);

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
        // An invalid project is deliberately passed through as an explicit empty
        // preference so legacy localStorage cannot resurrect a deleted resume.
        setProjects(nextProjects, resume.lastProjectId ?? "");
        let shotIds: string[] | undefined;
        if (
          resume.lastWorkspace === "shots"
          && resume.lastProjectId
          && nextProjects.some((project) => project.id === resume.lastProjectId)
        ) {
          shotIds = await listShots(resume.lastProjectId).then((shots) => shots.map((shot) => shot.id)).catch(() => []);
        }
        if (cancelled) return;
        const navigation = resolveWorkspaceNavigation(nextProjects, resume, shotIds);
        setResumeShotId(navigation.shotId);
        setWorkspace(navigation.workspace);
        setActiveStudioSection(defaultStudioSectionForWorkspace(navigation.workspace));
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
  }, [loadWorkspaceResume, setProjectError, setProjectLoading, setProjects]);

  useEffect(() => {
    setFocusedTaskId(undefined);
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

  function navigateToRoute(nextWorkspace: Workspace, nextSection = defaultStudioSectionForWorkspace(nextWorkspace)) {
    if (nextWorkspace === workspace && nextSection === activeStudioSection) return;
    setWorkspace(nextWorkspace);
    setActiveStudioSection(nextSection);
    if (nextWorkspace !== "shots") {
      setShotContextPath([]);
      setShotContextTarget(undefined);
    }
    void recordWorkspaceChange(nextWorkspace, activeProjectId);
  }

  function navigateToWorkspace(nextWorkspace: Workspace) {
    navigateToRoute(nextWorkspace);
  }

  function navigateToStudioSection(section: StudioSection) {
    const route = studioRouteForSection(section);
    navigateToRoute(route.workspace, route.section);
  }

  function handleShotSelected(shotId?: string) {
    setResumeShotId(shotId);
    void recordShotChange(shotId);
  }

  function openProject(
    projectId: string,
    preserveProductionBatch = false,
    destination: Workspace = preserveProductionBatch ? "studio" : "command-center",
    section: StudioSection = defaultStudioSectionForWorkspace(destination),
  ) {
    if (projectId === activeProjectId) {
      navigateToRoute(destination, section);
      return;
    }
    useTaskStore.getState().clear();
    useStudioStore.getState().resetDraft();
    useStudioStore.getState().clearPendingAssetIntent();
    if (!preserveProductionBatch) setFocusedProductionBatchId(undefined);
    setVideoBatchAssets([]);
    useProjectStore.getState().setActiveProject(projectId);
    setProjectContextLoading(true);
    setError(null);
    setResumeShotId(undefined);
    setWorkspace(destination);
    setActiveStudioSection(section);
    if (destination !== "shots") {
      setShotContextPath([]);
      setShotContextTarget(undefined);
    }
    void recordProjectChange(projectId, destination);
  }

  function openProductionQueue() {
    const { batchId, projectId } = productionAdmission;
    if (batchId && projectId) {
      setFocusedProductionBatchId(batchId);
      if (projectId !== activeProjectId) openProject(projectId, true, "shots", "production");
      else navigateToRoute("shots", "production");
      return;
    }
    navigateToStudioSection("production");
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
    useStudioStore.getState().loadDraft(workflow, draft.values);
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
    navigateToWorkspace("studio");
  }

  function handleProjectUpdated(project: ProjectView) {
    useProjectStore.getState().upsertProject(project);
    setError(null);
  }

  function handleProjectRestored(project: ProjectView) {
    useProjectStore.getState().upsertProject(project);
    setResumeShotId(undefined);
    setWorkspace("shots");
    setActiveStudioSection("creation");
    void recordProjectChange(project.id, "shots");
  }

  function handleTemplateProjectCreated(result: TemplateProjectResult) {
    useProjectStore.getState().upsertProject(result.project);
    setResumeShotId(undefined);
    setWorkspace("shots");
    setActiveStudioSection("creation");
    void recordProjectChange(result.project.id, "shots");
    const workflow = catalog.find((item) => item.workflowVersionId === result.workflowVersionId && item.recipeId === result.recipeId);
    if (!workflow) {
      setError("模板项目已创建，但工作流当前不可用。");
      return;
    }
    useStudioStore.getState().loadDraft(workflow, result.values);
    navigateToWorkspace("studio");
    setError(null);
  }

  function openVideoBatch(assets: AssetView[]) {
    setVideoBatchAssets(assets);
    navigateToWorkspace("video");
    setError(null);
  }

  function navigateFromCommandCenter(destination: ProjectCommandDestination) {
    if (destination === "studio" || destination === "shots") {
      navigateToStudioSection("creation");
      return;
    }
    navigateToWorkspace(destination);
  }

  const comfy = bootstrapState?.comfy;
  const isConnected = comfy?.status === "CONNECTED";
  const hasComfyCapabilityIssue = isConnected && !comfy?.capability;
  const showComfyWarning = Boolean(comfy && (!isConnected || hasComfyCapabilityIssue));
  const hasActiveTasks = recentTasks.some((task) =>
    ["CREATED", "VALIDATING", "PREPARING", "QUEUED", "RUNNING", "CANCEL_REQUESTED", "COLLECTING"]
    .includes(task.status),
  );

  const handleShotContextPathChange = useCallback((path: ShotContextPathItem[]) => {
    setShotContextPath(path);
  }, []);
  const handleShotContextPathSelect = useCallback((item: ShotContextPathItem) => {
    setShotContextTarget({ ...item });
  }, []);

  const breadcrumbs: StudioBreadcrumbItem[] = activeProject
    ? [
      {
        label: projectDisplayName(activeProject.id, activeProject.name),
        onClick: () => navigateToStudioSection("project"),
      },
      ...(workspace === "shots"
        ? shotContextPath.length
          ? shotContextPath.map((item, index) => ({
            label: item.label,
            current: index === shotContextPath.length - 1,
            onClick: index === shotContextPath.length - 1 ? undefined : () => handleShotContextPathSelect(item),
          }))
          : [{ label: "镜头生产", current: true }]
        : [{ label: workspaceLabels[workspace], current: true }]),
    ]
    : [{ label: "项目", current: true }];

  const projectSelector = (
    <select
      aria-label="当前项目"
      value={activeProjectId ?? ""}
      onChange={(event) => openProject(event.target.value)}
      disabled={projectLoading || !projects.length || projectContextLoading}
    >
      {!activeProjectId && <option value="">正在加载项目...</option>}
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
      <StudioShell
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
            <p>当前 GPU 正在执行生产任务，新的生成任务暂时不可提交。</p>
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
      {workspace !== "settings" && showComfyWarning && (
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
      {workspace === "command-center" && (
        <WorkspaceErrorBoundary
          resetKey={activeProject?.id ?? "no-project"}
          onBackToAssets={() => navigateToWorkspace("assets")}
          onRetry={() => navigateToWorkspace("command-center")}
        >
          <ProjectCommandCenter project={activeProject} onNavigate={navigateFromCommandCenter} />
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
            onProductionBatchFocused={() => setFocusedProductionBatchId(undefined)}
            onOpenWorkflows={() => navigateToWorkspace("workflows")}
            onReconnectComfy={() => void reconnectComfy()}
            onOpenTask={(taskId) => {
              setFocusedTaskId(taskId);
              navigateToWorkspace("tasks");
            }}
          />
        </section>
      )}
      {activeProject && workspace === "assets" && (
        <AssetWorkspace
          projectId={activeProject.id}
          onUseInStudio={useAssetInStudio}
          onOpenVideoBatch={openVideoBatch}
          onOpenTask={(taskId) => {
            setFocusedTaskId(taskId);
            navigateToWorkspace("tasks");
          }}
        />
      )}
      {activeProject && workspace === "shots" && (
        <WorkspaceErrorBoundary
          resetKey={activeProject.id}
          onBackToAssets={() => navigateToWorkspace("assets")}
          onRetry={() => navigateToWorkspace("shots")}
        >
          <ShotWorkspace
            projectId={activeProject.id}
            projectName={activeProject.name}
            projectDescription={activeProject.description}
            catalog={catalog}
            initialSelectedShotId={resumeShotId}
            mode={shotWorkspaceModeForSection(activeStudioSection)}
            onShotSelected={handleShotSelected}
            onContextPathChange={handleShotContextPathChange}
            contextPathTarget={shotContextTarget}
            onOpenTask={(taskId) => {
              setFocusedTaskId(taskId);
              navigateToWorkspace("tasks");
            }}
            onOpenProductionQueue={() => navigateToStudioSection("production")}
          />
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
            onProductionBatchFocused={() => setFocusedProductionBatchId(undefined)}
            onOpenTask={(taskId) => {
              setFocusedTaskId(taskId);
              navigateToWorkspace("tasks");
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
          onLoadInputs={loadHistoricalInputs}
          onOpenShot={() => navigateToWorkspace("shots")}
        />
      )}
      {workspace === "projects" && (
        <ProjectWorkspace
          projects={projects}
          activeProjectId={activeProjectId}
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
          onOpenTask={(taskId) => {
            setFocusedTaskId(taskId);
            navigateToWorkspace("tasks");
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

      {taskEventError && <p className="error-message global-error">{taskEventError}</p>}
      {error && <p className="error-message global-error">提示：{error}</p>}
        </div>
      </StudioShell>
    </div>
  );
}

export default App;
