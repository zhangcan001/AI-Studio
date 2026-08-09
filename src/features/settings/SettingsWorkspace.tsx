import { useCallback, useEffect, useState } from "react";
import {
  exportDiagnostics,
  getDiagnosticsSummary,
} from "../../services/tauriClient";
import type { DiagnosticsSummary } from "../../types/diagnostics";
import type { ComfyStatus } from "../../types/comfy";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";
import { formatFileSize } from "../../i18n/statusLabels";
import { ComfyStatus as ComfyStatusCard } from "../comfy/ComfyStatus";

interface Props {
  comfy?: ComfyStatus;
  connectionLoading: boolean;
  capabilityLoading: boolean;
  onReconnect: () => void;
  onRefreshCapabilities: () => void;
}

export function SettingsWorkspace({
  comfy,
  connectionLoading,
  capabilityLoading,
  onReconnect,
  onRefreshCapabilities,
}: Props) {
  const [summary, setSummary] = useState<DiagnosticsSummary>();
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<unknown>();
  const [notice, setNotice] = useState<string>();

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(undefined);
    try {
      setSummary(await getDiagnosticsSummary());
    } catch (nextError) {
      setError(nextError);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function exportBundle() {
    setExporting(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const exported = await exportDiagnostics();
      if (exported) setNotice(`诊断包已保存：${exported.fileName}`);
    } catch (exportError) {
      setError(exportError);
    } finally {
      setExporting(false);
    }
  }

  return (
    <section className="workspace-panel settings-workspace" aria-labelledby="settings-title">
      <div className="workspace-heading section-heading">
        <div>
          <span className="section-label">应用设置</span>
          <h2 id="settings-title">设置与诊断</h2>
          <p className="section-description">查看本地运行状态，或导出不包含私密内容的诊断摘要。</p>
        </div>
        <div className="settings-actions">
          <button type="button" onClick={() => void refresh()} disabled={loading}>
            {loading ? "正在刷新……" : "刷新诊断"}
          </button>
          <button type="button" className="primary-action" onClick={() => void exportBundle()} disabled={exporting}>
            {exporting ? "正在导出……" : "导出诊断包"}
          </button>
        </div>
      </div>

      {error !== undefined && <UiErrorNotice error={error} />}
      {notice && <p className="settings-notice" role="status">{notice}</p>}

      <div className="settings-grid">
        <section className="settings-card" aria-labelledby="settings-app-info">
          <h3 id="settings-app-info">应用信息</h3>
          <dl className="settings-list">
            <div><dt>应用版本</dt><dd>{summary?.appVersion ?? "--"}</dd></div>
            <div><dt>运行平台</dt><dd>{summary ? `${summary.platform} · ${summary.architecture}` : "--"}</dd></div>
            <div><dt>运行模式</dt><dd>{summary?.runMode ?? "--"}</dd></div>
          </dl>
        </section>

        <section className="settings-card" aria-labelledby="settings-runtime-info">
          <h3 id="settings-runtime-info">运行状态</h3>
          <dl className="settings-list">
            <div><dt>本地数据库</dt><dd>{summary ? (summary.databaseHealthy ? "正常" : "暂不可用") : "--"}</dd></div>
            <div><dt>工作流包</dt><dd>{summary ? `${summary.validWorkflowPackages} 个可用 / ${summary.workflowPackages} 个总计` : "--"}</dd></div>
            <div><dt>活动任务</dt><dd>{summary?.activeTaskCount ?? "--"}</dd></div>
            <div><dt>生产队列</dt><dd>{summary ? (summary.productionBusy ? "运行中" : "空闲") : "--"}</dd></div>
            <div><dt>日志</dt><dd>{summary ? (summary.loggingAvailable ? `可用，保留 ${summary.logRetentionDays} 天` : "不可用") : "--"}</dd></div>
          </dl>
        </section>
      </div>

      <section className="settings-card settings-comfy-card" aria-labelledby="settings-comfy-title">
        <div className="settings-card-heading">
          <div>
            <h3 id="settings-comfy-title">ComfyUI 运行环境</h3>
            <p>接口地址只读显示，连接操作沿用创作页的运行环境检查。</p>
          </div>
          {summary && <span className={`status-pill comfy-${summary.comfyStatus.toLowerCase()}`}>{comfyStatusLabel(summary.comfyStatus)}</span>}
        </div>
        <ComfyStatusCard
          status={comfy}
          connectionLoading={connectionLoading}
          capabilityLoading={capabilityLoading}
          onReconnect={onReconnect}
          onRefreshCapabilities={onRefreshCapabilities}
        />
        <dl className="settings-list settings-comfy-summary">
          <div><dt>版本</dt><dd>{summary?.comfyVersion ?? comfy?.comfyuiVersion ?? "--"}</dd></div>
          <div><dt>GPU</dt><dd>{summary?.gpuName ?? "--"}</dd></div>
          <div><dt>VRAM</dt><dd>{formatVram(summary?.vramFree, summary?.vramTotal)}</dd></div>
        </dl>
      </section>
    </section>
  );
}

function formatVram(free?: number, total?: number): string {
  if (free === undefined && total === undefined) return "--";
  return `${free === undefined ? "--" : formatFileSize(free)} 空闲 / ${total === undefined ? "--" : formatFileSize(total)} 总量`;
}

function comfyStatusLabel(status: DiagnosticsSummary["comfyStatus"]): string {
  if (status === "CONNECTED") return "已连接";
  if (status === "INCOMPATIBLE") return "版本不兼容";
  return "离线";
}
