import type { ComfyStatus as ComfyStatusView } from "../../types/comfy";

interface Props {
  status?: ComfyStatusView;
  connectionLoading: boolean;
  capabilityLoading: boolean;
  onReconnect: () => void;
  onRefreshCapabilities: () => void;
}

function formatBytes(bytes: number | undefined): string {
  if (bytes === undefined) return "--";
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

export function ComfyStatus({
  status,
  connectionLoading,
  capabilityLoading,
  onReconnect,
  onRefreshCapabilities,
}: Props) {
  const devices = status?.devices ?? [];
  const label = status?.status === "CONNECTED"
    ? "Connected"
    : status?.status === "INCOMPATIBLE"
      ? "Incompatible"
      : "Offline";
  return (
    <section className="comfy-status-card" aria-label="ComfyUI status">
      <div className="comfy-status-heading">
        <div>
          <span className="section-label">Runtime</span>
          <h2>ComfyUI status</h2>
        </div>
        <span className={`status-pill comfy-${status?.status?.toLowerCase() ?? "offline"}`}>
          {label}
        </span>
      </div>
      <div className="comfy-status-grid">
        <div><span>Endpoint</span><strong>{status?.endpoint ?? "--"}</strong></div>
        <div><span>Version</span><strong>{status?.comfyuiVersion ?? "--"}</strong></div>
        <div><span>GPU</span><strong>{devices.length ? devices.map((device) => device.name ?? "Unnamed GPU").join(" · ") : "--"}</strong></div>
        <div><span>VRAM</span><strong>{devices.length ? devices.map((device) => `${formatBytes(device.vramFree)} free / ${formatBytes(device.vramTotal)}`).join(" · ") : "--"}</strong></div>
        <div><span>Node Count</span><strong>{status?.capability?.nodeCount ?? "--"}</strong></div>
      </div>
      <div className="comfy-status-actions">
        <button type="button" onClick={onReconnect} disabled={connectionLoading}>
          {connectionLoading ? "Checking..." : "Test Connection"}
        </button>
        <button type="button" onClick={onRefreshCapabilities} disabled={status?.status !== "CONNECTED" || capabilityLoading}>
          {capabilityLoading ? "Refreshing..." : "Refresh Nodes"}
        </button>
      </div>
    </section>
  );
}
