import { useEffect, useState } from "react";
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
  const offline = status?.status !== "CONNECTED";
  const [expanded, setExpanded] = useState(offline);

  useEffect(() => {
    setExpanded(offline);
  }, [offline]);

  const primaryDevice = devices[0];
  return (
    <section className={`comfy-status-card runtime-status-card${offline ? " runtime-status-offline" : ""}`} aria-label="ComfyUI 状态">
      <div className="runtime-status-summary">
        <div className="runtime-status-identity">
          <span className={`status-dot status-${status?.status?.toLowerCase() ?? "offline"}`} aria-hidden="true" />
          <div>
            <span className="section-label">运行环境</span>
            <strong>ComfyUI {comfyStatusLabel(status?.status)}</strong>
          </div>
        </div>
        <div className="runtime-status-metrics">
          <span>{primaryDevice?.name ?? "GPU 未连接"}</span>
          <span>{primaryDevice ? `${formatFileSize(primaryDevice.vramFree ?? 0)} / ${formatFileSize(primaryDevice.vramTotal ?? 0)} 显存` : "等待连接"}</span>
        </div>
        <button type="button" className="quiet-button runtime-details-button" aria-expanded={expanded} onClick={() => setExpanded((current) => !current)}>
          {expanded ? "收起详情" : "运行环境详情"}
        </button>
      </div>
      {expanded && <div className="runtime-status-details">
        <div className="comfy-status-heading">
        <div>
          <span className="section-label">连接详情</span>
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
      </div>}
    </section>
  );
}
