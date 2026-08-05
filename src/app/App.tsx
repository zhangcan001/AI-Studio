import { useEffect, useState } from "react";
import {
  getComfyStatus,
  refreshComfyCapabilities,
} from "../services/tauriClient";
import type { ComfyDeviceInfo, ComfyStatus } from "../types/comfy";
import { bootstrap, type BootstrapState } from "./bootstrap";
import "./App.css";

function formatBytes(bytes: number | undefined): string {
  if (bytes === undefined) {
    return "--";
  }

  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function formatVram(device: ComfyDeviceInfo | undefined): string {
  if (!device || (device.vramFree === undefined && device.vramTotal === undefined)) {
    return "--";
  }

  return `${formatBytes(device.vramFree)} / ${formatBytes(device.vramTotal)}`;
}

function connectionLabel(status: ComfyStatus["status"]): string {
  switch (status) {
    case "CONNECTED":
      return "Connected";
    case "INCOMPATIBLE":
      return "Incompatible";
    default:
      return "Offline";
  }
}

function App() {
  const [bootstrapState, setBootstrapState] = useState<BootstrapState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connectionLoading, setConnectionLoading] = useState(false);
  const [capabilityLoading, setCapabilityLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;

    void bootstrap()
      .then((state) => {
        if (!cancelled) {
          setBootstrapState(state);
        }
      })
      .catch((bootstrapError: unknown) => {
        if (!cancelled) {
          setError(bootstrapError instanceof Error ? bootstrapError.message : String(bootstrapError));
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

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
        current
          ? { ...current, comfy: { ...current.comfy, capability } }
          : current,
      );
    } catch (refreshError: unknown) {
      setError(refreshError instanceof Error ? refreshError.message : String(refreshError));
    } finally {
      setCapabilityLoading(false);
    }
  }

  const comfy = bootstrapState?.comfy;
  const firstDevice = comfy?.devices[0];
  const isConnected = comfy?.status === "CONNECTED";

  return (
    <main className="app-shell">
      <section className="status-card" aria-live="polite">
        <p className="eyebrow">M0 Development Build</p>
        <h1>AI Studio</h1>

        <dl className="status-list">
          <div>
            <dt>Rust Backend</dt>
            <dd>{bootstrapState?.ping ?? "Connecting..."}</dd>
          </div>
          <div>
            <dt>Database</dt>
            <dd>{bootstrapState?.status.database === "ready" ? "Ready" : "Connecting..."}</dd>
          </div>
          <div>
            <dt>Data Directory</dt>
            <dd className="path-value">{bootstrapState?.status.data_root ?? "Resolving..."}</dd>
          </div>
        </dl>

        <section className="comfy-panel" aria-busy={connectionLoading}>
          <div className="section-heading">
            <div>
              <p className="section-label">Connection</p>
              <h2>ComfyUI</h2>
            </div>
            {comfy && (
              <span className={`status-pill status-${comfy.status.toLowerCase()}`}>
                {connectionLabel(comfy.status)}
              </span>
            )}
          </div>

          {!comfy ? (
            <p className="loading-message">Connecting...</p>
          ) : (
            <>
              <dl className="comfy-details">
                <div>
                  <dt>Endpoint</dt>
                  <dd className="path-value">{comfy.endpoint}</dd>
                </div>
                <div>
                  <dt>Version</dt>
                  <dd>{comfy.comfyuiVersion ?? "--"}</dd>
                </div>
                <div>
                  <dt>GPU</dt>
                  <dd>{firstDevice?.name ?? "--"}</dd>
                </div>
                <div>
                  <dt>VRAM</dt>
                  <dd>{formatVram(firstDevice)}</dd>
                </div>
                <div>
                  <dt>Nodes</dt>
                  <dd>{comfy.capability?.nodeCount ?? "--"}</dd>
                </div>
              </dl>

              {!isConnected && (
                <p className="offline-message">
                  {comfy.status === "INCOMPATIBLE"
                    ? "The endpoint did not return a compatible ComfyUI API response."
                    : "Unable to connect to local ComfyUI."}
                </p>
              )}

              <div className="actions">
                <button type="button" onClick={() => void reconnectComfy()} disabled={connectionLoading}>
                  {connectionLoading ? "Connecting..." : isConnected ? "Test Connection" : "Reconnect"}
                </button>
                <button
                  type="button"
                  onClick={() => void refreshCapabilities()}
                  disabled={!isConnected || capabilityLoading}
                >
                  {capabilityLoading ? "Refreshing..." : "Refresh Node Capabilities"}
                </button>
              </div>
            </>
          )}
        </section>

        {bootstrapState && <p className="version">Version {bootstrapState.status.version}</p>}
        {error && <p className="error-message">Notice: {error}</p>}
      </section>
    </main>
  );
}

export default App;
