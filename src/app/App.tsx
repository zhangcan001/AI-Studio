import { useEffect, useState } from "react";
import {
  getComfyStatus,
  listGenerationCatalog,
  listRecentTasks,
  refreshComfyCapabilities,
} from "../services/tauriClient";
import { subscribeTaskUpdates } from "../services/taskEvents";
import { useTaskStore } from "../stores/taskStore";
import type { RecipeViewModel } from "../types/generation";
import type { ComfyDeviceInfo, ComfyStatus } from "../types/comfy";
import { GenerationStudio } from "../features/studio/GenerationStudio";
import { bootstrap, type BootstrapState } from "./bootstrap";
import "./App.css";

function formatBytes(bytes: number | undefined): string {
  if (bytes === undefined) return "--";
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function formatVram(device: ComfyDeviceInfo | undefined): string {
  if (!device || (device.vramFree === undefined && device.vramTotal === undefined)) return "--";
  return `${formatBytes(device.vramFree)} / ${formatBytes(device.vramTotal)}`;
}

function connectionLabel(status: ComfyStatus["status"]): string {
  switch (status) {
    case "CONNECTED": return "Connected";
    case "INCOMPATIBLE": return "Incompatible";
    default: return "Offline";
  }
}

function App() {
  const [bootstrapState, setBootstrapState] = useState<BootstrapState | null>(null);
  const [catalog, setCatalog] = useState<RecipeViewModel[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connectionLoading, setConnectionLoading] = useState(false);
  const [capabilityLoading, setCapabilityLoading] = useState(false);
  const setRecentTasks = useTaskStore((state) => state.setRecentTasks);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void subscribeTaskUpdates((task) => useTaskStore.getState().upsertTask(task)).then((cleanup) => {
      if (cancelled) cleanup();
      else unlisten = cleanup;
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

  const comfy = bootstrapState?.comfy;
  const firstDevice = comfy?.devices[0];
  const isConnected = comfy?.status === "CONNECTED";

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <p className="eyebrow">M0 Generation Studio</p>
          <h1>AI Studio</h1>
        </div>
        {comfy && (
          <div className="header-status">
            <span className={`status-dot status-${comfy.status.toLowerCase()}`} />
            <span>ComfyUI {connectionLabel(comfy.status)}</span>
            <small>{firstDevice?.name ?? "GPU unavailable"}</small>
          </div>
        )}
      </header>

      <section className="runtime-strip" aria-live="polite">
        <div><span>Backend</span><strong>{bootstrapState?.ping ?? "Connecting..."}</strong></div>
        <div><span>Database</span><strong>{bootstrapState?.status.database === "ready" ? "Ready" : "Connecting..."}</strong></div>
        <div><span>VRAM</span><strong>{formatVram(firstDevice)}</strong></div>
        <div><span>Nodes</span><strong>{comfy?.capability?.nodeCount ?? "--"}</strong></div>
        <button type="button" onClick={() => void reconnectComfy()} disabled={connectionLoading}>
          {connectionLoading ? "Checking..." : "Test Connection"}
        </button>
        <button type="button" onClick={() => void refreshCapabilities()} disabled={!isConnected || capabilityLoading}>
          {capabilityLoading ? "Refreshing..." : "Refresh Nodes"}
        </button>
      </section>

      <section className="studio-layout">
        <GenerationStudio
          catalog={catalog}
          comfyConnected={isConnected}
          onCatalogChanged={reloadCatalog}
        />
      </section>

      {error && <p className="error-message global-error">Notice: {error}</p>}
      {bootstrapState && <p className="version">Version {bootstrapState.status.version}</p>}
    </main>
  );
}

export default App;
