import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  listToolCapabilities,
  listToolInstances,
  listTools,
  listToolVersions,
} from "../../services/tauriClient";
import { formatDateTime } from "../../i18n/statusLabels";
import { toUserMessage } from "../../i18n/errorMessages";
import type {
  ToolCapabilityView,
  ToolHealthStatus,
  ToolInstanceView,
  ToolVersionView,
  ToolView,
} from "../../types/tool";
import "./LocalToolHub.css";

interface ToolDetail {
  tool: ToolView;
  instances: ToolInstanceView[];
  versions: ToolVersionView[];
  capabilities: ToolCapabilityView[];
}

const healthLabels: Record<ToolHealthStatus, string> = {
  AVAILABLE: "可用",
  MISSING: "缺失",
  UNKNOWN: "未知",
};

function formatJson(value: unknown): string {
  if (value === undefined || value === null) return "暂无记录";
  try {
    return JSON.stringify(value, null, 2) ?? "暂无记录";
  } catch {
    return "数据暂不可读取";
  }
}

function formatLocation(instance?: Pick<ToolInstanceView, "path" | "endpoint">): string {
  if (!instance) return "未登记位置";
  const locations = [instance.path, instance.endpoint].filter(
    (location): location is string => Boolean(location?.trim()),
  );
  return locations.join(" · ") || "未登记位置";
}

function resolveHealthStatus(instances: readonly ToolInstanceView[]): ToolHealthStatus {
  if (instances.some((instance) => instance.status === "AVAILABLE")) return "AVAILABLE";
  if (instances.some((instance) => instance.status === "MISSING")) return "MISSING";
  return "UNKNOWN";
}

function healthClass(status: ToolHealthStatus): string {
  return "local-tool-health-badge local-tool-health-badge--" + status.toLowerCase();
}

function detailStateLabel(detail: ToolDetail | undefined, error: string | undefined, loading: boolean): { label: string; className: string } {
  if (error) return { label: "加载失败", className: "local-tool-health-badge local-tool-health-badge--error" };
  if (loading) return { label: "读取中…", className: "local-tool-health-badge local-tool-health-badge--unknown" };
  if (!detail) return { label: "尚未读取", className: "local-tool-health-badge local-tool-health-badge--unknown" };
  const status = resolveHealthStatus(detail.instances);
  return { label: healthLabels[status], className: healthClass(status) };
}

function latestVersion(versions: readonly ToolVersionView[]): ToolVersionView | undefined {
  return versions.length ? versions[versions.length - 1] : undefined;
}

async function loadToolDetail(tool: ToolView): Promise<ToolDetail> {
  const [instances, versions, capabilities] = await Promise.all([
    listToolInstances(tool.id),
    listToolVersions(tool.id),
    listToolCapabilities(tool.id),
  ]);
  return { tool, instances, versions, capabilities };
}

export function LocalToolHub() {
  const [tools, setTools] = useState<ToolView[]>([]);
  const [detailsById, setDetailsById] = useState<Record<string, ToolDetail>>({});
  const [detailErrorsById, setDetailErrorsById] = useState<Record<string, string>>({});
  const [selectedToolId, setSelectedToolId] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [detailLoadingId, setDetailLoadingId] = useState<string>();
  const [error, setError] = useState<string>();
  const requestVersion = useRef(0);

  const loadTools = useCallback(async () => {
    const request = ++requestVersion.current;
    setLoading(true);
    setError(undefined);
    setDetailErrorsById({});
    try {
      const nextTools = await listTools();
      if (requestVersion.current !== request) return;
      setTools(nextTools);
      setSelectedToolId((current) => (
        current && nextTools.some((tool) => tool.id === current)
          ? current
          : nextTools[0]?.id
      ));
      setDetailsById({});

      const results = await Promise.allSettled(nextTools.map(loadToolDetail));
      if (requestVersion.current !== request) return;
      const nextDetails: Record<string, ToolDetail> = {};
      const nextErrors: Record<string, string> = {};
      results.forEach((result, index) => {
        const tool = nextTools[index];
        if (result.status === "fulfilled") nextDetails[tool.id] = result.value;
        else nextErrors[tool.id] = toUserMessage(result.reason);
      });
      setDetailsById(nextDetails);
      setDetailErrorsById(nextErrors);
    } catch (value: unknown) {
      if (requestVersion.current === request) setError(toUserMessage(value));
    } finally {
      if (requestVersion.current === request) setLoading(false);
    }
  }, []);

  const retryToolDetail = useCallback(async (toolId: string) => {
    const tool = tools.find((candidate) => candidate.id === toolId);
    if (!tool) return;
    setDetailLoadingId(toolId);
    setDetailErrorsById((current) => {
      const next = { ...current };
      delete next[toolId];
      return next;
    });
    try {
      const detail = await loadToolDetail(tool);
      setDetailsById((current) => ({ ...current, [toolId]: detail }));
    } catch (value: unknown) {
      setDetailErrorsById((current) => ({ ...current, [toolId]: toUserMessage(value) }));
    } finally {
      setDetailLoadingId((current) => (current === toolId ? undefined : current));
    }
  }, [tools]);

  useEffect(() => {
    void loadTools();
    return () => {
      requestVersion.current += 1;
    };
  }, [loadTools]);

  const selectedTool = useMemo(
    () => tools.find((tool) => tool.id === selectedToolId),
    [selectedToolId, tools],
  );
  const selectedDetail = selectedToolId ? detailsById[selectedToolId] : undefined;
  const selectedDetailError = selectedToolId ? detailErrorsById[selectedToolId] : undefined;

  function selectTool(toolId: string) {
    setSelectedToolId(toolId);
    if (!detailsById[toolId] && !detailLoadingId && !loading) void retryToolDetail(toolId);
  }

  return (
    <section
      className="workspace-panel local-tool-hub-workspace"
      aria-label="本地工具中心"
      aria-busy={loading || Boolean(detailLoadingId)}
    >
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">v2 工作台</span>
          <h2>本地工具中心</h2>
          <p className="section-description">查看个人 AI 工具、实例位置、能力和最近一次健康状态记录。</p>
        </div>
        <div className="local-tool-hub-actions">
          <span className="local-tool-hub-scope">本地元数据</span>
          <button type="button" className="quiet-button" onClick={() => void loadTools()} disabled={loading}>
            {loading ? "正在刷新…" : "刷新"}
          </button>
        </div>
      </div>

      {error && <p className="error-message" role="alert">本地工具列表加载失败：{error}</p>}

      <div className="local-tool-hub-layout">
        <section className="local-tool-hub-list-panel" aria-label="工具列表">
          <div className="local-tool-hub-panel-heading">
            <div><span className="section-label">工具登记</span><h3>工具列表</h3></div>
            <span className="status-pill">{tools.length} 个</span>
          </div>
          {loading && !tools.length && <p className="disabled-note" role="status">正在加载本地工具…</p>}
          {!loading && !error && !tools.length && <p className="empty-state">暂无本地工具登记。</p>}
          {tools.length > 0 && (
            <div className="local-tool-hub-table-wrap">
              <table className="local-tool-hub-table">
                <caption className="sr-only">本地工具列表</caption>
                <thead>
                  <tr>
                    <th scope="col">名称</th>
                    <th scope="col">类型</th>
                    <th scope="col">状态</th>
                    <th scope="col">版本</th>
                    <th scope="col">位置</th>
                  </tr>
                </thead>
                <tbody>
                  {tools.map((tool) => {
                    const detail = detailsById[tool.id];
                    const state = detailStateLabel(detail, detailErrorsById[tool.id], loading || detailLoadingId === tool.id);
                    const version = detail ? latestVersion(detail.versions)?.version : undefined;
                    return (
                      <tr key={tool.id} className={tool.id === selectedToolId ? "active" : undefined}>
                        <th scope="row">
                          <button
                            type="button"
                            className="local-tool-hub-row-button"
                            onClick={() => selectTool(tool.id)}
                            aria-pressed={tool.id === selectedToolId}
                          >
                            {tool.name}
                          </button>
                        </th>
                        <td>{tool.type}</td>
                        <td><span className={state.className}>{state.label}</span></td>
                        <td>{version ? "v" + version : state.label}</td>
                        <td className="local-tool-hub-location-cell" title={detail ? formatLocation(detail.instances[0]) : undefined}>
                          {detail ? formatLocation(detail.instances[0]) : state.label}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </section>

        <section className="local-tool-hub-detail-panel" aria-label="工具详情">
          <div className="local-tool-hub-panel-heading">
            <div><span className="section-label">当前工具</span><h3>{selectedTool?.name ?? "工具详情"}</h3></div>
            {selectedDetail && <span className={healthClass(resolveHealthStatus(selectedDetail.instances))}>{healthLabels[resolveHealthStatus(selectedDetail.instances)]}</span>}
          </div>
          {!selectedTool && loading && <p className="disabled-note" role="status">正在读取工具详情…</p>}
          {!selectedTool && !loading && !error && <p className="empty-state">请选择一个工具查看详情。</p>}
          {selectedTool && !selectedDetail && detailLoadingId === selectedTool.id && <p className="disabled-note" role="status">正在读取工具详情…</p>}
          {selectedTool && selectedDetailError && (
            <div className="local-tool-hub-detail-error">
              <p className="error-message" role="alert">工具详情加载失败：{selectedDetailError}</p>
              {selectedDetail && <p className="local-tool-hub-note">以下内容为上次成功读取的结果。</p>}
              <button type="button" className="quiet-button" onClick={() => void retryToolDetail(selectedTool.id)} disabled={detailLoadingId === selectedTool.id}>
                {detailLoadingId === selectedTool.id ? "正在重试…" : "重试"}
              </button>
            </div>
          )}
          {selectedDetail && (
            <div className="local-tool-hub-detail-content">
              <p className="local-tool-hub-description">{selectedDetail.tool.description || "暂无工具说明。"}</p>
              {resolveHealthStatus(selectedDetail.instances) === "MISSING" && (
                <p className="local-tool-hub-missing-note" role="status">工具位置缺失或最近一次记录不可用；本页面不会自动启动或安装工具。</p>
              )}
              <dl className="local-tool-hub-metadata">
                <div><dt>名称</dt><dd>{selectedDetail.tool.name}</dd></div>
                <div><dt>类型</dt><dd>{selectedDetail.tool.type}</dd></div>
                <div><dt>创建时间</dt><dd>{formatDateTime(selectedDetail.tool.createdAt)}</dd></div>
              </dl>
              <section className="local-tool-hub-subpanel" aria-label="本地实例">
                <div className="local-tool-hub-panel-heading"><h4>实例与位置</h4><span className="status-pill">{selectedDetail.instances.length} 个</span></div>
                {!selectedDetail.instances.length && <p className="empty-state">尚未登记本地实例。</p>}
                <div className="local-tool-hub-instance-list">
                  {selectedDetail.instances.map((instance) => (
                    <article key={instance.id} className="local-tool-hub-instance">
                      <div className="local-tool-hub-instance-heading">
                        <strong>{formatLocation(instance)}</strong>
                        <span className={healthClass(instance.status)}>{healthLabels[instance.status]}</span>
                      </div>
                      <small>{instance.lastChecked ? "最近记录：" + formatDateTime(instance.lastChecked) : "尚未记录健康状态"}</small>
                    </article>
                  ))}
                </div>
              </section>
              <section className="local-tool-hub-subpanel" aria-label="健康状态">
                <div className="local-tool-hub-panel-heading"><h4>健康状态</h4><span className="local-tool-hub-readonly-label">只读展示</span></div>
                <p className="local-tool-hub-note">状态来自最近一次显式记录，不执行端点探测、进程启动或停止。</p>
                <div className="local-tool-hub-health-summary">
                  <strong className={healthClass(resolveHealthStatus(selectedDetail.instances))}>{healthLabels[resolveHealthStatus(selectedDetail.instances)]}</strong>
                  <span>{selectedDetail.instances.length ? "以实例记录为准" : "尚未登记实例"}</span>
                </div>
              </section>
              <section className="local-tool-hub-subpanel" aria-label="能力视图">
                <div className="local-tool-hub-panel-heading"><h4>能力</h4><span className="status-pill">{selectedDetail.capabilities.length} 项</span></div>
                {!selectedDetail.capabilities.length && <p className="empty-state">暂无能力记录。</p>}
                <ul className="local-tool-hub-capability-list">
                  {selectedDetail.capabilities.map((capability) => (
                    <li key={capability.capabilityName}>
                      <strong>{capability.capabilityName}</strong>
                      <pre>{formatJson(capability.metadata)}</pre>
                    </li>
                  ))}
                </ul>
              </section>
              <section className="local-tool-hub-subpanel" aria-label="版本视图">
                <div className="local-tool-hub-panel-heading"><h4>观察版本历史</h4><span className="local-tool-hub-readonly-label">只读</span></div>
                {!selectedDetail.versions.length && <p className="empty-state">暂无版本观察记录。</p>}
                <ol className="local-tool-hub-version-list">
                  {selectedDetail.versions.slice().reverse().map((version, index) => (
                    <li key={version.id}>
                      <div>
                        <strong>v{version.version}</strong>
                        {index === 0 && <span className="status-pill">当前</span>}
                      </div>
                      <small>{formatDateTime(version.observedAt)}</small>
                      <pre>{formatJson(version.metadata)}</pre>
                    </li>
                  ))}
                </ol>
              </section>
              <section className="local-tool-hub-subpanel" aria-label="工具元数据">
                <div className="local-tool-hub-panel-heading"><h4>工具元数据</h4></div>
                <pre className="local-tool-hub-json">{formatJson(selectedDetail.tool.metadata)}</pre>
              </section>
            </div>
          )}
        </section>
      </div>
    </section>
  );
}
