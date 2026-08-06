import { useEffect, useState } from "react";
import {
  getComfyStatus,
  listGenerationCatalog,
  listProjects,
  listRecentTasks,
  reconcileActiveTasks,
  refreshComfyCapabilities,
} from "../services/tauriClient";
import { subscribeTaskUpdates } from "../services/taskEvents";
import { useTaskStore } from "../stores/taskStore";
import { useProjectStore } from "../stores/projectStore";
import type { RecipeViewModel } from "../types/generation";
import { GenerationStudio } from "../features/studio/GenerationStudio";
import { AssetLibrary } from "../features/assets/AssetLibrary";
import { TaskHistory } from "../features/tasks/TaskHistory";
import { ProjectWorkspace } from "../features/projects/ProjectWorkspace";
import { WorkflowWorkspace } from "../features/workflows/WorkflowWorkspace";
import { ComfyStatus as ComfyStatusCard } from "../features/comfy/ComfyStatus";
import { bootstrap, type BootstrapState } from "./bootstrap";
import { useStudioStore } from "../stores/studioStore";
import type { ReusableGenerationDraft } from "../types/history";
import type { ProjectView } from "../types/project";
import "./App.css";

type Workspace = "studio" | "assets" | "tasks" | "projects" | "workflows";

function App() {
  const [workspace, setWorkspace] = useState<Workspace>("studio");
  const [bootstrapState, setBootstrapState] = useState<BootstrapState | null>(null);
  const [catalog, setCatalog] = useState<RecipeViewModel[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [taskEventsReady, setTaskEventsReady] = useState(false);
  const [taskEventError, setTaskEventError] = useState<string | undefined>();
  const [connectionLoading, setConnectionLoading] = useState(false);
  const [capabilityLoading, setCapabilityLoading] = useState(false);
  const [reconciling, setReconciling] = useState(false);
  const [recoveryNotice, setRecoveryNotice] = useState<string | null>(null);
  const [projectContextLoading, setProjectContextLoading] = useState(false);
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

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void subscribeTaskUpdates((task) => {
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
          setTaskEventError("Task event channel unavailable");
        }
      });

    void Promise.all([bootstrap(), listGenerationCatalog()])
      .then(([state, recipes]) => {
        if (!cancelled) {
          setBootstrapState(state);
          setCatalog(recipes);
        }
      })
      .catch((bootstrapError: unknown) => {
        if (!cancelled) {
          setError(bootstrapError instanceof Error ? bootstrapError.message : String(bootstrapError));
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    setProjectLoading(true);
    void listProjects()
      .then((nextProjects) => {
        if (!cancelled) setProjects(nextProjects);
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          const message = loadError instanceof Error ? loadError.message : String(loadError);
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
  }, [setProjectError, setProjectLoading, setProjects]);

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
          setError(loadError instanceof Error ? loadError.message : String(loadError));
        }
      })
      .finally(() => {
        if (!cancelled) setProjectContextLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeProjectId, setRecentTasks]);

  function openProject(projectId: string) {
    if (projectId === activeProjectId) return;
    useTaskStore.getState().clear();
    useStudioStore.getState().resetDraft();
    useProjectStore.getState().setActiveProject(projectId);
    setProjectContextLoading(true);
    setError(null);
    setWorkspace("studio");
  }

  async function reconnectComfy() {
    setConnectionLoading(true);
    setError(null);
    try {
      const comfy = await getComfyStatus();
      setBootstrapState((current) => (current ? { ...current, comfy } : current));
    } catch (connectionError: unknown) {
      setError(connectionError instanceof Error ? connectionError.message : String(connectionError));
    } finally {
      setConnectionLoading(false);
    }
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
      setError(refreshError instanceof Error ? refreshError.message : String(refreshError));
    } finally {
      setCapabilityLoading(false);
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
        setError("The published workflow is not available in the runtime catalog yet.");
        return;
      }
      useStudioStore.getState().setSelectedWorkflow(workflow);
      setWorkspace("studio");
      setError(null);
    } catch (openError: unknown) {
      setError(openError instanceof Error ? openError.message : String(openError));
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
        `Reconciled ${report.examined} task${report.examined === 1 ? "" : "s"}: ` +
          `${report.succeeded} updated, ${report.deferred} deferred, ${report.unresolved} unresolved.`,
      );
    } catch (recoveryError: unknown) {
      setRecoveryNotice(recoveryError instanceof Error ? recoveryError.message : String(recoveryError));
    } finally {
      setReconciling(false);
    }
  }

  function loadHistoricalInputs(draft: ReusableGenerationDraft) {
    if (!activeProjectId || draft.projectId !== activeProjectId) {
      setError("PROJECT_CONTEXT_CHANGED: open the task from its project before loading inputs.");
      return;
    }
    const workflow = catalog.find(
      (recipe) =>
        recipe.workflowVersionId === draft.workflowVersionId && recipe.recipeId === draft.recipeId,
    );
    if (!workflow) {
      setError("This workflow version is no longer available in the runtime catalog.");
      return;
    }
    useStudioStore.getState().loadDraft(workflow, draft.values);
    setError(null);
    setWorkspace("studio");
  }

  function handleProjectUpdated(project: ProjectView) {
    useProjectStore.getState().upsertProject(project);
    setError(null);
  }

  const comfy = bootstrapState?.comfy;
  const isConnected = comfy?.status === "CONNECTED";
  const hasActiveTasks = recentTasks.some((task) =>
    ["CREATED", "VALIDATING", "PREPARING", "QUEUED", "RUNNING", "CANCEL_REQUESTED", "COLLECTING"]
      .includes(task.status),
  );

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <p className="eyebrow">LOCAL WORKBENCH</p>
          <h1>AI Studio</h1>
        </div>
        <div className="header-context-group">
          <div className="project-selector">
            <label htmlFor="active-project">Project</label>
            <select
              id="active-project"
              value={activeProjectId ?? ""}
              onChange={(event) => openProject(event.target.value)}
              disabled={projectLoading || !projects.length || projectContextLoading}
            >
              {!activeProjectId && <option value="">Loading projects...</option>}
              {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
            </select>
          </div>
          <button type="button" className="quiet-button header-new-project" onClick={() => setWorkspace("projects")}>
            New Project
          </button>
          {comfy && (
            <div className="header-status">
              <span className={`status-dot status-${comfy.status.toLowerCase()}`} />
              <span>ComfyUI {comfy.status === "CONNECTED" ? "Connected" : comfy.status === "INCOMPATIBLE" ? "Incompatible" : "Offline"}</span>
              <small>{comfy.devices[0]?.name ?? "GPU unavailable"}</small>
            </div>
          )}
        </div>
      </header>

      <nav className="workspace-nav" aria-label="Workspace">
        {([
          ["studio", "Studio"],
          ["assets", "Assets"],
          ["tasks", "Tasks"],
          ["projects", "Projects"],
          ["workflows", "Workflows"],
        ] as const).map(([value, label]) => (
          <button
            type="button"
            key={value}
            className={workspace === value ? "workspace-nav-button workspace-nav-button-active" : "workspace-nav-button"}
            onClick={() => setWorkspace(value)}
            aria-current={workspace === value ? "page" : undefined}
          >
            {label}
          </button>
        ))}
      </nav>

      {projectContextLoading && activeProject && (
        <p className="project-loading" role="status">Loading project…</p>
      )}
      {workspace === "studio" && (
        <ComfyStatusCard
          status={comfy}
          connectionLoading={connectionLoading}
          capabilityLoading={capabilityLoading}
          onReconnect={() => void reconnectComfy()}
          onRefreshCapabilities={() => void refreshCapabilities()}
        />
      )}

      {(hasActiveTasks || recoveryNotice) && (
        <section className="task-recovery-bar" aria-live="polite">
          <div>
            <span className="section-label">Task recovery</span>
            <p>{recoveryNotice ?? "Active tasks were found after startup."}</p>
          </div>
          <button type="button" onClick={() => void reconcileTasks()} disabled={reconciling}>
            {reconciling ? "Reconciling..." : "Reconcile tasks"}
          </button>
        </section>
      )}

      {!activeProject && projectError && <p className="error-message global-error">Unable to load projects: {projectError}</p>}
      {activeProject && workspace === "studio" && (
        <section className="studio-layout">
          <GenerationStudio
            projectId={activeProject.id}
            catalog={catalog}
            comfyConnected={isConnected}
            taskEventsReady={taskEventsReady}
            taskEventError={taskEventError}
            onCatalogChanged={reloadCatalog}
          />
        </section>
      )}
      {activeProject && workspace === "assets" && <AssetLibrary projectId={activeProject.id} />}
      {activeProject && workspace === "tasks" && (
        <TaskHistory projectId={activeProject.id} onLoadInputs={loadHistoricalInputs} />
      )}
      {workspace === "projects" && (
        <ProjectWorkspace
          projects={projects}
          activeProjectId={activeProjectId}
          onOpen={openProject}
          onProjectUpdated={handleProjectUpdated}
        />
      )}
      {workspace === "workflows" && (
        <WorkflowWorkspace onCatalogChanged={reloadCatalog} onOpenStudio={openPublishedWorkflow} />
      )}

      {taskEventError && <p className="error-message global-error">{taskEventError}</p>}
      {error && <p className="error-message global-error">Notice: {error}</p>}
      {bootstrapState && <p className="version">Version {bootstrapState.status.version}</p>}
    </main>
  );
}

export default App;
