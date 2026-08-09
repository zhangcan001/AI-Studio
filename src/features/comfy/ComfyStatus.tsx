import type { ComfyStatus as ComfyStatusView } from "../../types/comfy";
import { comfyStatusLabel, formatFileSize } from "../../i18n/statusLabels";

interface Props {
  status?: ComfyStatusView;
  connectionLoading: boolean;
  capabilityLoading: boolean;
  onReconnect: () => void;
  onRefreshCapabilities: () => void;
}

export function ComfyStatus({
  status,
  connectionLoading,
  capabilityLoading,
  onReconnect,
  onRefreshCapabilities,
}: Props) {
  const devices = status?.devices ?? [];
  return (
    <section className="comfy-status-card" aria-label="ComfyUI 状态">
      <div className="comfy-status-heading">
        <div>
          <span className="section-label">运行环境</span>
          <h2>ComfyUI 状态</h2>
        </div>
        <span className={`status-pill comfy-${status?.status?.toLowerCase() ?? "offline"}`}>
          {comfyStatusLabel(status?.status)}
        </span>
      </div>
      <div className="comfy-status-grid">
        <div><span>接口地址</span><strong>{status?.endpoint ?? "--"}</strong></div>
        <div><span>版本</span><strong>{status?.comfyuiVersion ?? "--"}</strong></div>
        <div><span>GPU</span><strong>{devices.length ? devices.map((device) => device.name ?? "未命名 GPU").join(" · ") : "--"}</strong></div>
        <div><span>显存</span><strong>{devices.length ? devices.map((device) => `${device.vramFree === undefined ? "--" : formatFileSize(device.vramFree)} 空闲 / ${device.vramTotal === undefined ? "--" : formatFileSize(device.vramTotal)} 总量`).join(" · ") : "--"}</strong></div>
        <div><span>节点数量</span><strong>{status?.capability?.nodeCount ?? "--"}</strong></div>
      </div>
      <div className="comfy-status-actions">
        <button type="button" onClick={onReconnect} disabled={connectionLoading}>
          {connectionLoading ? "正在检查..." : "测试连接"}
        </button>
        <button type="button" onClick={onRefreshCapabilities} disabled={status?.status !== "CONNECTED" || capabilityLoading}>
          {capabilityLoading ? "正在刷新..." : "刷新节点"}
        </button>
      </div>
    </section>
  );
}
