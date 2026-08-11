import { useCallback, useEffect, useState } from "react";
import {
  exportDiagnostics,
  freeComfyMemory,
  getComfySettings,
  getDiagnosticsSummary,
  saveComfyEndpoint,
  testComfyConnection,
} from "../../services/tauriClient";
import type { DiagnosticsSummary } from "../../types/diagnostics";
import type { ComfyStatus } from "../../types/comfy";
import type { ComfyEndpointTest, ComfySettingsView } from "../../types/settings";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";
import { formatFileSize } from "../../i18n/statusLabels";
import { ComfyStatus as ComfyStatusCard } from "../comfy/ComfyStatus";

interface Props {
  comfy?: ComfyStatus;
  connectionLoading: boolean;
  capabilityLoading: boolean;
  onReconnect: () => void;
  onRefreshCapabilities: () => void;
  onEndpointApplied?: () => void;
}

export function SettingsWorkspace({
  comfy,
  connectionLoading,
  capabilityLoading,
  onReconnect,
  onRefreshCapabilities,
  onEndpointApplied,
}: Props) {
  const [summary, setSummary] = useState<DiagnosticsSummary>();
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<unknown>();
  const [notice, setNotice] = useState<string>();
  const [settings, setSettings] = useState<ComfySettingsView>();
  const [endpointDraft, setEndpointDraft] = useState("");
  const [endpointTesting, setEndpointTesting] = useState(false);
  const [endpointApplying, setEndpointApplying] = useState(false);
  const [endpointTest, setEndpointTest] = useState<ComfyEndpointTest>();
  const [memoryReleasing, setMemoryReleasing] = useState(false);

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

  useEffect(() => {
    let cancelled = false;
    void getComfySettings()
      .then((loaded) => {
        if (!cancelled) {
          setSettings(loaded);
          setEndpointDraft(loaded.endpoint);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setError(loadError);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function testEndpoint() {
    setEndpointTesting(true);
    setError(undefined);
    setNotice(undefined);
    setEndpointTest(undefined);
    try {
      setEndpointTest(await testComfyConnection(endpointDraft));
    } catch (testError: unknown) {
      setError(testError);
    } finally {
      setEndpointTesting(false);
    }
  }

  async function applyEndpoint() {
    setEndpointApplying(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const next = await saveComfyEndpoint(endpointDraft);
      setSettings(next);
      setEndpointDraft(next.endpoint);
      setEndpointTest(undefined);
      setNotice("ComfyUI 地址已保存并应用。");
      onEndpointApplied?.();
    } catch (applyError: unknown) {
      setError(applyError);
    } finally {
      setEndpointApplying(false);
    }
  }

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

  async function releaseComfyMemory() {
    setMemoryReleasing(true);
    setError(undefined);
    setNotice(undefined);
    try {
      await freeComfyMemory();
      setNotice("已请求 ComfyUI 卸载模型并释放显存/内存。模型文件不会删除。");
      window.setTimeout(() => onReconnect(), 250);
    } catch (releaseError: unknown) {
      setError(releaseError);
    } finally {
      setMemoryReleasing(false);
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
            <p>可保存本机或局域网 ComfyUI 地址，应用后立即生效。</p>
          </div>
          <div className="settings-card-heading-status">
            {settings && endpointDraft.trim() !== settings.endpoint && <span className="settings-dirty-pill">尚未应用</span>}
            {summary && <span className={`status-pill comfy-${summary.comfyStatus.toLowerCase()}`}>{comfyStatusLabel(summary.comfyStatus)}</span>}
          </div>
        </div>
        <ComfyStatusCard
          status={comfy}
          connectionLoading={connectionLoading}
          capabilityLoading={capabilityLoading}
          onReconnect={onReconnect}
          onRefreshCapabilities={onRefreshCapabilities}
        />
        <div className="settings-endpoint-form">
          <label htmlFor="comfy-endpoint">ComfyUI 地址</label>
          <div className="settings-endpoint-row">
            <input
              id="comfy-endpoint"
              value={endpointDraft || settings?.endpoint || comfy?.endpoint || ""}
              onChange={(event) => setEndpointDraft(event.target.value)}
              placeholder="http://127.0.0.1:8188"
              spellCheck={false}
              disabled={endpointTesting || endpointApplying}
            />
            <button type="button" onClick={() => void testEndpoint()} disabled={endpointTesting || endpointApplying || !endpointDraft.trim()}>
              {endpointTesting ? "正在测试……" : "测试连接"}
            </button>
            <button type="button" className="primary-action" onClick={() => void applyEndpoint()} disabled={endpointTesting || endpointApplying || !endpointDraft.trim()}>
              {endpointApplying ? "正在保存……" : "保存并应用"}
            </button>
          </div>
          {settings?.warning && <p className="settings-warning" role="status">{settings.warning}</p>}
          {endpointTest && (
            <p className="settings-notice" role="status">
              已连接：{endpointTest.version ?? "版本未知"} · GPU {endpointTest.gpu.join("、") || "不可用"} · 节点 {endpointTest.nodeCount}
            </p>
          )}
        </div>
        <section className="comfy-memory-release" aria-labelledby="comfy-memory-release-title">
          <div>
            <h4 id="comfy-memory-release-title">释放模型内存</h4>
            <p>仅在任务和 ComfyUI 队列空闲时执行。会调用官方内存释放接口，模型文件不会删除。</p>
          </div>
          <button
            type="button"
            className="danger-button"
            onClick={() => void releaseComfyMemory()}
            disabled={memoryReleasing || connectionLoading || comfy?.status !== "CONNECTED"}
          >
            {memoryReleasing ? "正在释放……" : "释放显存/内存"}
          </button>
        </section>
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
