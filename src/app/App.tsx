import { useEffect, useState } from "react";
import {
  getComfyStatus,
  listGenerationCatalog,
  listRecentTasks,
  reconcileActiveTasks,
  refreshComfyCapabilities,
} from "../services/tauriClient";
import { subscribeTaskUpdates } from "../services/taskEvents";
import { useTaskStore } from "../stores/taskStore";
import type { RecipeViewModel } from "../types/generation";
import { GenerationStudio } from "../features/studio/GenerationStudio";
import { AssetLibrary } from "../features/assets/AssetLibrary";
import { TaskHistory } from "../features/tasks/TaskHistory";
import { ComfyStatus as ComfyStatusCard } from "../features/comfy/ComfyStatus";
import { bootstrap, type BootstrapState } from "./bootstrap";
import { useStudioStore } from "../stores/studioStore";
import type { ReusableGenerationDraft } from "../types/history";
import "./App.css";

function App() {
  type Workspace = "studio" | "assets" | "tasks";
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
  const setRecentTasks = useTaskStore((state) => state.setRecentTasks);
  const recentTasks = useTaskStore((state) => state.recentTasks);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void subscribeTaskUpdates((task) => useTaskStore.getState().upsertTask(task))
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

    void Promise.all([bootstrap(), listGenerationCatalog(), listRecentTasks(10)])
      .then(([state, recipes, tasks]) => {
        if (!cancelled) {
          setBootstrapState(state);
          setCatalog(recipes);
          setRecentTasks(tasks);
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
  }, [setRecentTasks]);

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

  async function reconcileTasks() {
    setReconciling(true);
    setRecoveryNotice(null);
    try {
      const report = await reconcileActiveTasks();
      setRecentTasks(await listRecentTasks(10));
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
        {comfy && (
          <div className="header-status">
            <span className={`status-dot status-${comfy.status.toLowerCase()}`} />
            <span>ComfyUI {comfy.status === "CONNECTED" ? "Connected" : comfy.status === "INCOMPATIBLE" ? "Incompatible" : "Offline"}</span>
            <small>{comfy.devices[0]?.name ?? "GPU unavailable"}</small>
          </div>
        )}
      </header>

      <nav className="workspace-nav" aria-label="Workspace">
        {([
          ["studio", "Studio"],
          ["assets", "Assets"],
          ["tasks", "Tasks"],
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

      {workspace === "studio" && (
        <section className="studio-layout">
          <GenerationStudio
            catalog={catalog}
            comfyConnected={isConnected}
            taskEventsReady={taskEventsReady}
            taskEventError={taskEventError}
            onCatalogChanged={reloadCatalog}
          />
        </section>
      )}
      {workspace === "assets" && <AssetLibrary />}
      {workspace === "tasks" && <TaskHistory onLoadInputs={loadHistoricalInputs} />}

      {taskEventError && <p className="error-message global-error">{taskEventError}</p>}
      {error && <p className="error-message global-error">Notice: {error}</p>}
      {bootstrapState && <p className="version">Version {bootstrapState.status.version}</p>}
    </main>
  );
}

export default App;
