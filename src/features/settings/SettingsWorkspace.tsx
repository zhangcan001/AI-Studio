import { useCallback, useEffect, useState } from "react";
import {
  applyComfyEnvironmentProfile,
  deleteComfyEnvironmentProfile,
  exportDiagnostics,
  freeComfyMemory,
  getComfyPreflight,
  getComfySettings,
  getDiagnosticsSummary,
  listComfyEnvironmentProfiles,
  saveComfyEndpoint,
  saveComfyEnvironmentProfile,
  testComfyConnection,
} from "../../services/tauriClient";
import type { DiagnosticsSummary } from "../../types/diagnostics";
import type { ComfyStatus } from "../../types/comfy";
import type {
  ComfyEndpointTest,
  ComfyEnvironmentProfile,
  ComfyPreflightReport,
  ComfySettingsView,
} from "../../types/settings";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";
import { formatFileSize, formatDateTime } from "../../i18n/statusLabels";
import { formatUiError } from "../../i18n/errorMessages";
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
  const [environmentProfiles, setEnvironmentProfiles] = useState<ComfyEnvironmentProfile[]>([]);
  const [profilesLoading, setProfilesLoading] = useState(true);
  const [profileSaving, setProfileSaving] = useState(false);
  const [profileDeletingId, setProfileDeletingId] = useState<string>();
  const [profileTestingId, setProfileTestingId] = useState<string>();
  const [profileTests, setProfileTests] = useState<Record<string, ComfyEndpointTest | undefined>>({});
  const [profileTestErrors, setProfileTestErrors] = useState<Record<string, string>>({});
  const [editingProfileId, setEditingProfileId] = useState<string>();
  const [profileNameDraft, setProfileNameDraft] = useState("");
  const [profileEndpointDraft, setProfileEndpointDraft] = useState("");
  const [preflight, setPreflight] = useState<ComfyPreflightReport>();
  const [preflightLoading, setPreflightLoading] = useState(false);

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

  useEffect(() => {
    let cancelled = false;
    setProfilesLoading(true);
    void listComfyEnvironmentProfiles()
      .then((loaded) => {
        if (!cancelled) setEnvironmentProfiles(loaded);
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setError(loadError);
      })
      .finally(() => {
        if (!cancelled) setProfilesLoading(false);
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

  async function testProfile(profile: ComfyEnvironmentProfile) {
    setProfileTestingId(profile.id);
    setProfileTestErrors((current) => ({ ...current, [profile.id]: "" }));
    try {
      const result = await testComfyConnection(profile.endpoint);
      setProfileTests((current) => ({ ...current, [profile.id]: result }));
    } catch (testError: unknown) {
      setProfileTests((current) => ({ ...current, [profile.id]: undefined }));
      setProfileTestErrors((current) => ({ ...current, [profile.id]: formatUiError(testError).message }));
    } finally {
      setProfileTestingId(undefined);
    }
  }

  async function saveEnvironmentProfile() {
    const name = profileNameDraft.trim();
    const endpoint = profileEndpointDraft.trim();
    if (!name || name.length > 80 || /[\r\n]/.test(name)) {
      setNotice("环境名称必须是 1–80 个字符且不能换行。");
      return;
    }
    if (!endpoint) {
      setNotice("请输入 ComfyUI 地址。");
      return;
    }
    setProfileSaving(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const now = new Date().toISOString();
      const saved = await saveComfyEnvironmentProfile({
        id: editingProfileId ?? newProfileId(),
        name,
        endpoint,
        createdAt: environmentProfiles.find((profile) => profile.id === editingProfileId)?.createdAt ?? now,
        updatedAt: now,
      });
      setEnvironmentProfiles((current) => [saved, ...current.filter((profile) => profile.id !== saved.id)]);
      setEditingProfileId(saved.id);
      setProfileNameDraft(saved.name);
      setProfileEndpointDraft(saved.endpoint);
      setNotice("ComfyUI 环境已保存。");
    } catch (saveError: unknown) {
      setError(saveError);
    } finally {
      setProfileSaving(false);
    }
  }

  async function deleteEnvironmentProfile(profile: ComfyEnvironmentProfile) {
    if (typeof globalThis.confirm === "function" && !globalThis.confirm(`删除环境“${profile.name}”？`)) return;
    setProfileDeletingId(profile.id);
    setError(undefined);
    setNotice(undefined);
    try {
      await deleteComfyEnvironmentProfile(profile.id);
      setEnvironmentProfiles((current) => current.filter((item) => item.id !== profile.id));
      if (editingProfileId === profile.id) resetProfileForm();
      setNotice(`已删除环境“${profile.name}”。`);
    } catch (deleteError: unknown) {
      setError(deleteError);
    } finally {
      setProfileDeletingId(undefined);
    }
  }

  async function applyEnvironmentProfile(profile: ComfyEnvironmentProfile) {
    setProfileSaving(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const next = await applyComfyEnvironmentProfile(profile.id);
      setSettings(next);
      setEndpointDraft(next.endpoint);
      setEndpointTest(undefined);
      setNotice(`已切换到 ${profile.name}。`);
      onEndpointApplied?.();
      void runPreflight();
    } catch (applyError: unknown) {
      setError(applyError);
    } finally {
      setProfileSaving(false);
    }
  }

  async function runPreflight() {
    setPreflightLoading(true);
    setError(undefined);
    try {
      setPreflight(await getComfyPreflight());
    } catch (preflightError: unknown) {
      setError(preflightError);
    } finally {
      setPreflightLoading(false);
    }
  }

  function editEnvironmentProfile(profile: ComfyEnvironmentProfile) {
    setEditingProfileId(profile.id);
    setProfileNameDraft(profile.name);
    setProfileEndpointDraft(profile.endpoint);
    setNotice(undefined);
  }

  function resetProfileForm() {
    setEditingProfileId(undefined);
    setProfileNameDraft("");
    setProfileEndpointDraft("");
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
        <section className="settings-endpoint-form" aria-labelledby="settings-environment-profiles-title">
          <div className="settings-card-heading">
            <div>
              <h4 id="settings-environment-profiles-title">已保存环境</h4>
              <p>保存多个 ComfyUI 地址；测试只读，应用会继续经过安全切换与忙碌检查。</p>
            </div>
            {environmentProfiles.length >= 20 && <span className="settings-dirty-pill">最多 20 个</span>}
          </div>
          {profilesLoading ? <p className="settings-notice" role="status">正在读取环境档案……</p> : (
            <div className="settings-grid">
              {environmentProfiles.map((profile) => {
                const isCurrent = normalizeEndpoint(profile.endpoint) === normalizeEndpoint(settings?.endpoint ?? comfy?.endpoint ?? endpointDraft);
                const tested = profileTests[profile.id];
                return (
                  <article className="settings-card" key={profile.id} aria-label={`ComfyUI 环境 ${profile.name}`}>
                    <div className="settings-card-heading">
                      <div>
                        <h4>{profile.name}</h4>
                        <p>{profile.endpoint}</p>
                      </div>
                      {isCurrent && <span className="status-pill comfy-connected">当前环境</span>}
                    </div>
                    <div className="settings-endpoint-row">
                      <button type="button" onClick={() => void testProfile(profile)} disabled={profileTestingId === profile.id || profileSaving || profileDeletingId === profile.id}>
                        {profileTestingId === profile.id ? "正在测试……" : "测试"}
                      </button>
                      <button type="button" className="primary-action" onClick={() => void applyEnvironmentProfile(profile)} disabled={profileSaving || profileDeletingId === profile.id}>
                        {profileSaving ? "正在应用……" : "应用"}
                      </button>
                      <button type="button" className="quiet-button" onClick={() => editEnvironmentProfile(profile)} disabled={profileSaving || profileDeletingId === profile.id}>编辑</button>
                      <button type="button" className="quiet-button" onClick={() => void deleteEnvironmentProfile(profile)} disabled={profileSaving || profileDeletingId === profile.id}>
                        {profileDeletingId === profile.id ? "正在删除……" : "删除"}
                      </button>
                    </div>
                    {profileTestErrors[profile.id] && <p className="settings-warning" role="alert">{profileTestErrors[profile.id]}</p>}
                    {tested && <p className="settings-notice" role="status">已连接 · ComfyUI {tested.version ?? "版本未知"} · GPU {tested.gpu.join("、") || "不可用"} · VRAM {formatVram(tested.vramFree, tested.vramTotal)} · 节点 {tested.nodeCount}</p>}
                  </article>
                );
              })}
            </div>
          )}
          {!profilesLoading && !environmentProfiles.length && <p>还没有保存的 ComfyUI 环境。</p>}
          <div className="settings-endpoint-row">
            <input aria-label="环境名称" value={profileNameDraft} onChange={(event) => setProfileNameDraft(event.target.value)} maxLength={80} placeholder="环境名称，例如：WorkFisher H3" disabled={profileSaving} />
            <input aria-label="环境 ComfyUI 地址" value={profileEndpointDraft} onChange={(event) => setProfileEndpointDraft(event.target.value)} placeholder="http://127.0.0.1:8188" spellCheck={false} disabled={profileSaving} />
            <button type="button" className="primary-action" onClick={() => void saveEnvironmentProfile()} disabled={profileSaving || environmentProfiles.length >= 20 && !editingProfileId}>
              {profileSaving ? "正在保存……" : editingProfileId ? "更新环境" : "保存环境"}
            </button>
            {editingProfileId && <button type="button" className="quiet-button" onClick={resetProfileForm} disabled={profileSaving}>取消编辑</button>}
          </div>
        </section>
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

      <section className="settings-card" aria-labelledby="settings-preflight-title">
        <div className="settings-card-heading">
          <div>
            <h3 id="settings-preflight-title">运行预检</h3>
            <p>只读检查当前 ComfyUI、GPU、节点和生产工作流可用性，不会切换环境或启动生成。</p>
          </div>
          <button type="button" className="primary-action" onClick={() => void runPreflight()} disabled={preflightLoading}>
            {preflightLoading ? "正在预检……" : "立即预检"}
          </button>
        </div>
        {!preflight && !preflightLoading && <p>尚未运行预检。</p>}
        {preflight && <PreflightReportView report={preflight} />}
      </section>
    </section>
  );
}

function PreflightReportView({ report }: { report: ComfyPreflightReport }) {
  const workflowSummary = report.workflowSummary;
  return (
    <div>
      <p className="settings-notice" role="status">
        <span className={`status-pill ${preflightStatusClass(report.status)}`}>{preflightStatusLabel(report.status)}</span>{" "}
        {preflightStatusDescription(report.status)} · 检查于 {formatDateTime(report.checkedAt)}
      </p>
      <dl className="settings-list settings-comfy-summary">
        <div><dt>环境</dt><dd>{report.endpoint}</dd></div>
        <div><dt>ComfyUI</dt><dd>{report.comfyuiVersion ?? "--"}</dd></div>
        <div><dt>Python</dt><dd>{report.pythonVersion ?? "--"}</dd></div>
        <div><dt>连接</dt><dd>{report.connection}</dd></div>
        <div><dt>GPU</dt><dd>{report.gpu ?? "不可用"}</dd></div>
        <div><dt>VRAM</dt><dd>{formatVram(report.vramFree, report.vramTotal)}</dd></div>
        <div><dt>节点数量</dt><dd>{report.nodeCount ?? "--"}</dd></div>
        <div><dt>活动任务</dt><dd>{report.activeTaskCount}</dd></div>
        <div><dt>生产队列</dt><dd>{report.productionBusy ? "运行中" : "空闲"}</dd></div>
        <div><dt>运行时</dt><dd>{report.runtimeBusy ? "忙碌" : "空闲"}</dd></div>
        <div><dt>工作流</dt><dd>{workflowSummary.workflowReady} 可用 / {workflowSummary.workflowBlocked} 不可用 / {workflowSummary.workflowTotal} 总计</dd></div>
      </dl>
      {workflowSummary.items?.length ? (
        <div className="settings-endpoint-form">
          <h4>工作流明细</h4>
          {workflowSummary.items.map((workflow) => (
            <p key={`${workflow.name}-${workflow.version ?? ""}`} className={workflow.status === "READY" ? "settings-notice" : "settings-warning"}>
              {workflow.name}{workflow.version ? ` · ${workflow.version}` : ""} · {workflow.status === "READY" ? "可用" : workflow.status === "DISABLED" ? "已禁用" : "不可用"}
              {workflow.missingNodes?.length ? ` · 缺少：${workflow.missingNodes.join("、")}` : ""}
              {workflow.reason ? ` · ${workflow.reason}` : ""}
            </p>
          ))}
        </div>
      ) : null}
      <div className="settings-endpoint-form">
        <h4>问题</h4>
        {!report.issues.length && <p className="settings-notice">未发现阻塞问题。</p>}
        {report.issues.map((issue, index) => (
          <div className={issue.severity === "ERROR" ? "settings-warning" : issue.severity === "WARNING" ? "settings-warning" : "settings-notice"} key={`${issue.code}-${index}`}>
            <strong>{issue.title}</strong>
            <p>{issue.detail}</p>
            {issue.missingNodes?.length ? <p>缺少：{issue.missingNodes.join("、")}</p> : null}
            {issue.suggestedAction && <p>建议：{issue.suggestedAction}</p>}
          </div>
        ))}
      </div>
    </div>
  );
}

function formatVram(free?: number | null, total?: number | null): string {
  if (free == null && total == null) return "--";
  return `${free == null ? "--" : formatFileSize(free)} 空闲 / ${total == null ? "--" : formatFileSize(total)} 总量`;
}

function newProfileId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `comfy-environment-${Date.now()}`;
}

function normalizeEndpoint(endpoint: string | undefined): string {
  if (!endpoint) return "";
  try {
    return new URL(endpoint).toString().replace(/\/$/, "").toLowerCase();
  } catch {
    return endpoint.trim().replace(/\/+$/, "").toLowerCase();
  }
}

function preflightStatusClass(status: ComfyPreflightReport["status"]): string {
  if (status === "READY") return "comfy-connected";
  if (status === "WARNING") return "comfy-incompatible";
  return "comfy-offline";
}

function preflightStatusLabel(status: ComfyPreflightReport["status"]): string {
  if (status === "READY") return "READY · 运行环境就绪";
  if (status === "WARNING") return "WARNING · 存在警告";
  return "BLOCKED · 当前环境无法生产";
}

function preflightStatusDescription(status: ComfyPreflightReport["status"]): string {
  if (status === "READY") return "运行环境就绪";
  if (status === "WARNING") return "运行环境可用，但存在警告";
  return "当前环境无法生产";
}

function comfyStatusLabel(status: DiagnosticsSummary["comfyStatus"]): string {
  if (status === "CONNECTED") return "已连接";
  if (status === "INCOMPATIBLE") return "版本不兼容";
  return "离线";
}
