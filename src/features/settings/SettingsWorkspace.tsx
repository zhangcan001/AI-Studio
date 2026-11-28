import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyComfyEnvironmentProfile,
  checkpointDatabaseWal,
  cleanupRejectedVideos,
  deleteComfyEnvironmentProfile,
  freeComfyMemory,
  getComfyPreflight,
  getComfySettings,
  getDesktopPreferences,
  getStorageSummary,
  listComfyEnvironmentProfiles,
  openStorageDirectory,
  pickAndSetDataRoot,
  resetDataRoot,
  saveComfyEndpoint,
  saveComfyEnvironmentProfile,
  saveDesktopPreferences,
  testComfyConnection,
} from "../../services/tauriClient";
import { diagnosticsClient } from "../../services/diagnosticsClient";
import { ThemedConfirmDialog } from "../../app/WorkspaceErrorBoundary";
import type { DiagnosticsSummary } from "../../types/diagnostics";
import type { ComfyStatus } from "../../types/comfy";
import type {
  ComfyEndpointTest,
  ComfyEnvironmentProfile,
  ComfyPreflightReport,
  ComfySettingsView,
  DesktopPreferences,
  StorageSummaryView,
} from "../../types/settings";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";
import { formatFileSize, formatDateTime } from "../../i18n/statusLabels";
import { formatUiError } from "../../i18n/errorMessages";
import { ComfyStatus as ComfyStatusCard } from "../comfy/ComfyStatus";
import { RepairJobsStatusSection } from "./RepairJobsStatusSection";
import { DiagnosticsExecutionPanel, type DiagnosticsExecutionProps } from "./DiagnosticsExecutionPanel";

interface Props extends DiagnosticsExecutionProps {
  onOpenWorkflowDiagnostics?: () => void;
  onOpenProjectGenerators?: (projectId: string) => void;
  onOpenToolHub?: () => void;
  showWorkflowRepairStatus?: boolean;
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
  showWorkflowRepairStatus = true,
  projectId,
  initialTaskId,
  onOpenRun,
  onOpenAudit,
  onOpenProjectGenerators,
  onOpenToolHub,
}: Props) {
  const [summary, setSummary] = useState<DiagnosticsSummary>();
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<unknown>();
  const [notice, setNotice] = useState<string>();
  const [settings, setSettings] = useState<ComfySettingsView>();
  const [endpointDraft, setEndpointDraft] = useState(() => comfy?.endpoint ?? "");
  const [authTokenDraft, setAuthTokenDraft] = useState("");
  const [allowSystemProxyDraft, setAllowSystemProxyDraft] = useState(false);
  const [endpointTesting, setEndpointTesting] = useState(false);
  const [endpointApplying, setEndpointApplying] = useState(false);
  const [endpointTest, setEndpointTest] = useState<ComfyEndpointTest>();
  const [memoryReleasing, setMemoryReleasing] = useState(false);
  const [desktopPrefs, setDesktopPrefs] = useState<DesktopPreferences>({
    autoFreeVramBetweenItems: false,
    transientRetryLimit: 0,
    desktopNotifications: true,
    onboardingDismissed: false,
  });
  const [prefsSaving, setPrefsSaving] = useState(false);
  const [storageSummary, setStorageSummary] = useState<StorageSummaryView>();
  const [storageBusy, setStorageBusy] = useState(false);
  const [deleteProfileTarget, setDeleteProfileTarget] = useState<ComfyEnvironmentProfile>();
  const [cleanupConfirmOpen, setCleanupConfirmOpen] = useState(false);
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
  const [profileAuthDraft, setProfileAuthDraft] = useState("");
  const [profileProxyDraft, setProfileProxyDraft] = useState(false);
  const [migrateConfirmOpen, setMigrateConfirmOpen] = useState(false);
  const [preflight, setPreflight] = useState<ComfyPreflightReport>();
  const [preflightLoading, setPreflightLoading] = useState(false);

  const summaryRequest = useRef(0);
  const refresh = useCallback(async () => {
    const request = ++summaryRequest.current;
    setSummary(undefined);
    setLoading(true);
    setError(undefined);
    try {
      const next = await diagnosticsClient.summary();
      if (request === summaryRequest.current) setSummary(next);
    } catch (nextError) {
      if (request === summaryRequest.current) setError(nextError);
    } finally {
      if (request === summaryRequest.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    return () => { summaryRequest.current++; };
  }, [refresh]);

  useEffect(() => {
    let cancelled = false;
    void getComfySettings()
      .then((loaded) => {
        if (!cancelled) {
          setSettings(loaded);
          setEndpointDraft(loaded.endpoint);
          setAllowSystemProxyDraft(Boolean(loaded.allowSystemProxy));
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setError(loadError);
      });
    void getDesktopPreferences()
      .then((loaded) => {
        if (!cancelled) setDesktopPrefs(loaded);
      })
      .catch(() => { /* keep defaults in tests */ });
    void getStorageSummary()
      .then((loaded) => {
        if (!cancelled) setStorageSummary(loaded);
      })
      .catch(() => { /* ignore outside Tauri */ });
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
      const options = authTokenDraft.trim() || allowSystemProxyDraft !== Boolean(settings?.allowSystemProxy)
        ? {
            ...(authTokenDraft.trim() ? { authHeader: authTokenDraft.trim() } : {}),
            allowSystemProxy: allowSystemProxyDraft,
          }
        : undefined;
      setEndpointTest(options ? await testComfyConnection(endpointDraft, options) : await testComfyConnection(endpointDraft));
    } catch (testError: unknown) {
      setError(testError);
    } finally {
      setEndpointTesting(false);
    }
  }

  async function applyEndpoint(clearAuth = false) {
    setEndpointApplying(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const hasExtra = clearAuth || Boolean(authTokenDraft.trim()) || allowSystemProxyDraft !== Boolean(settings?.allowSystemProxy);
      const next = hasExtra
        ? await saveComfyEndpoint(endpointDraft, {
            ...(authTokenDraft.trim() ? { authHeader: authTokenDraft.trim() } : {}),
            ...(clearAuth ? { clearAuthHeader: true } : {}),
            allowSystemProxy: allowSystemProxyDraft,
          })
        : await saveComfyEndpoint(endpointDraft);
      setSettings(next);
      setEndpointDraft(next.endpoint);
      setAllowSystemProxyDraft(Boolean(next.allowSystemProxy));
      setAuthTokenDraft("");
      setEndpointTest(undefined);
      setNotice(clearAuth ? "已清除认证令牌并保存 ComfyUI 配置。" : "ComfyUI 地址已保存并应用。");
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "ComfyUI 连接配置已保存并应用", tone: "success" } }));
      onEndpointApplied?.();
    } catch (applyError: unknown) {
      setError(applyError);
    } finally {
      setEndpointApplying(false);
    }
  }

  async function updateDesktopPref(patch: Partial<DesktopPreferences>) {
    const next = { ...desktopPrefs, ...patch };
    setDesktopPrefs(next);
    setPrefsSaving(true);
    setError(undefined);
    try {
      const saved = await saveDesktopPreferences(next);
      setDesktopPrefs(saved);
      window.dispatchEvent(new CustomEvent("ai-studio:desktop-prefs-changed", { detail: saved }));
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "单机运行偏好已保存并实时生效", tone: "success" } }));
    } catch (saveError: unknown) {
      setError(saveError);
    } finally {
      setPrefsSaving(false);
    }
  }

  async function refreshStorageSummary() {
    setStorageBusy(true);
    try {
      setStorageSummary(await getStorageSummary());
    } catch (storageError: unknown) {
      setError(storageError);
    } finally {
      setStorageBusy(false);
    }
  }

  async function handlePickDataRoot() {
    setMigrateConfirmOpen(false);
    setStorageBusy(true);
    setError(undefined);
    try {
      const report = await pickAndSetDataRoot(true);
      if (report) {
        setNotice(
          `已安排迁移到 ${report.targetPath}。当前仍使用原目录，关闭应用后，下次启动前会复制最新数据并切换；原目录完整保留。`,
        );
        window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "已安排下次启动前迁移数据", tone: "success" } }));
        await refreshStorageSummary();
      }
    } catch (pickError: unknown) {
      setError(pickError);
    } finally {
      setStorageBusy(false);
    }
  }

  async function handleResetDataRoot() {
    setStorageBusy(true);
    setError(undefined);
    try {
      await resetDataRoot();
      setNotice("已恢复默认数据目录配置，重启 AI Studio 后生效。");
      await refreshStorageSummary();
    } catch (resetError: unknown) {
      setError(resetError);
    } finally {
      setStorageBusy(false);
    }
  }

  async function handleWalCheckpoint() {
    setStorageBusy(true);
    setError(undefined);
    try {
      await checkpointDatabaseWal();
      setNotice("数据库 WAL 日志已安全合并压缩。");
      await refreshStorageSummary();
    } catch (walError: unknown) {
      setError(walError);
    } finally {
      setStorageBusy(false);
    }
  }

  async function handleCleanupRejectedVideos() {
    if (!projectId) return;
    setCleanupConfirmOpen(false);
    setStorageBusy(true);
    setError(undefined);
    try {
      const report = await cleanupRejectedVideos(projectId);
      setNotice(
        report.deletedCount > 0
          ? `已安全清理 ${report.deletedCount} 个已退回废片，释放约 ${formatFileSize(report.freedBytes)}${report.skippedInUse > 0 ? `（另有 ${report.skippedInUse} 个仍在引用中已自动跳过）` : ""}。`
          : report.skippedInUse > 0
            ? `已退回的 ${report.skippedInUse} 个视频仍在镜头引用中，已自动保护跳过。`
            : "当前项目没有可清理的已退回废片视频。",
      );
      await refreshStorageSummary();
    } catch (cleanupError: unknown) {
      setError(cleanupError);
    } finally {
      setStorageBusy(false);
    }
  }

  async function testProfile(profile: ComfyEnvironmentProfile) {
    setProfileTestingId(profile.id);
    setProfileTestErrors((current) => ({ ...current, [profile.id]: "" }));
    try {
      const profileOptions = profile.authHeader || profile.allowSystemProxy
        ? { ...(profile.authHeader ? { authHeader: profile.authHeader } : {}), allowSystemProxy: Boolean(profile.allowSystemProxy) }
        : undefined;
      const result = profileOptions ? await testComfyConnection(
        profile.endpoint,
        profileOptions,
      ) : await testComfyConnection(profile.endpoint);
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
        ...(profileAuthDraft.trim() ? { authHeader: profileAuthDraft.trim() } : {}),
        allowSystemProxy: profileProxyDraft,
        createdAt: environmentProfiles.find((profile) => profile.id === editingProfileId)?.createdAt ?? now,
        updatedAt: now,
      });
      setEnvironmentProfiles((current) => [saved, ...current.filter((profile) => profile.id !== saved.id)]);
      setEditingProfileId(saved.id);
      setProfileNameDraft(saved.name);
      setProfileEndpointDraft(saved.endpoint);
      setProfileAuthDraft("");
      setProfileProxyDraft(Boolean(saved.allowSystemProxy));
      setNotice("ComfyUI 环境已保存。");
    } catch (saveError: unknown) {
      setError(saveError);
    } finally {
      setProfileSaving(false);
    }
  }

  function requestDeleteEnvironmentProfile(profile: ComfyEnvironmentProfile) {
    setDeleteProfileTarget(profile);
  }

  async function confirmDeleteEnvironmentProfile() {
    const profile = deleteProfileTarget;
    if (!profile) return;
    setDeleteProfileTarget(undefined);
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
      setAuthTokenDraft("");
      setAllowSystemProxyDraft(Boolean(next.allowSystemProxy));
      setEndpointTest(undefined);
      setNotice(`已切换到 ${profile.name}。`);
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: `已切换到 ${profile.name}`, tone: "success" } }));
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
    setProfileAuthDraft("");
    setProfileProxyDraft(Boolean(profile.allowSystemProxy));
    setNotice(undefined);
  }

  function resetProfileForm() {
    setEditingProfileId(undefined);
    setProfileNameDraft("");
    setProfileEndpointDraft("");
    setProfileAuthDraft("");
    setProfileProxyDraft(false);
  }

  async function exportBundle() {
    setExporting(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const exported = await diagnosticsClient.exportBundle();
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
          <button
            type="button"
            onClick={() => void releaseComfyMemory()}
            disabled={memoryReleasing || connectionLoading || comfy?.status !== "CONNECTED"}
            title="调用官方接口卸载模型释放显存/内存，不删除模型文件"
          >
            {memoryReleasing ? "正在释放……" : "快速释放显存"}
          </button>
          <button type="button" onClick={() => void refresh()} disabled={loading}>
            {loading ? "正在刷新……" : "刷新诊断"}
          </button>
          <button type="button" className="primary-action" onClick={() => void exportBundle()} disabled={exporting}>
            {exporting ? "正在导出……" : "导出诊断包"}
          </button>
        </div>
      </div>

      <nav className="settings-quick-anchors" aria-label="设置快捷定位">
        <a href="#comfy-endpoint">ComfyUI 连接配置</a>
        <a href="#settings-preflight-title">运行预检</a>
        <a href="#settings-desktop-prefs-title">单机生产偏好</a>
        <a href="#settings-storage-title">存储与空间管理</a>
        <a href="#settings-environment-profiles-title">多环境档案</a>
        <a href="#comfy-memory-release-title">显存与内存释放</a>
      </nav>

      {error !== undefined && <UiErrorNotice error={error} />}
      {notice && <p className="settings-notice" role="status">{notice}</p>}

      <section className="settings-card" aria-labelledby="settings-setup-title">
        <h3 id="settings-setup-title">运行环境准备</h3>
        <ol><li><a href="#comfy-endpoint">配置并应用 ComfyUI 地址</a></li><li><a href="#settings-preflight-title">执行运行预检</a></li><li>为当前项目确认生成器</li></ol>
        <p>ComfyUI 地址是全局运行环境；项目生成器决定当前项目使用哪个已有生成器，保存地址不会自动绑定生成器。</p>
        {projectId && onOpenProjectGenerators
          ? <button type="button" onClick={() => onOpenProjectGenerators(projectId)}>检查当前项目生成器</button>
          : <p>选择项目后，可在项目设置中确认图片/视频生成器。</p>}
      </section>

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
              value={endpointDraft}
              onChange={(event) => setEndpointDraft(event.target.value)}
              placeholder={settings?.endpoint || comfy?.endpoint || "http://127.0.0.1:8188 或 https://gpu.example.com/comfy"}
              spellCheck={false}
              disabled={endpointTesting || endpointApplying}
            />
            <button type="button" onClick={() => void testEndpoint()} disabled={endpointTesting || endpointApplying || !endpointDraft.trim()}>
              {endpointTesting ? "正在测试……" : "测试连接"}
            </button>
            <button type="button" className="primary-action" onClick={() => void applyEndpoint(false)} disabled={endpointTesting || endpointApplying || !endpointDraft.trim()}>
              {endpointApplying ? "正在保存……" : "保存并应用"}
            </button>
          </div>
          <div className="settings-endpoint-row settings-remote-auth-row">
            <input
              aria-label="云端或反代认证令牌（可选）"
              type="password"
              value={authTokenDraft}
              onChange={(event) => setAuthTokenDraft(event.target.value)}
              placeholder={settings?.authConfigured ? "已保存认证令牌（留空保持不变，输入新令牌可覆盖）" : "认证令牌 / Bearer Token（本地 ComfyUI 留空即可）"}
              spellCheck={false}
              disabled={endpointTesting || endpointApplying}
            />
            {settings?.authConfigured && (
              <button type="button" className="quiet-button" onClick={() => void applyEndpoint(true)} disabled={endpointTesting || endpointApplying}>
                清除令牌
              </button>
            )}
            <label className="settings-inline-check">
              <input
                type="checkbox"
                checked={allowSystemProxyDraft}
                disabled={endpointTesting || endpointApplying}
                onChange={(event) => setAllowSystemProxyDraft(event.target.checked)}
              />
              允许使用系统代理（云算力反代可用）
            </label>
          </div>
          <details className="settings-quiet-help"><summary>ⓘ 连接测试说明</summary>
            <p>测试连接仅测试当前输入的地址，不会保存或应用这个地址。</p>
            {settings && endpointDraft.trim() !== settings.endpoint && <p>当前输入只用于测试；点击“保存并应用”后才会成为 AI Studio 使用的运行环境。</p>}
          </details>
          {settings?.warning && <p className="settings-warning" role="status">{settings.warning}</p>}
          {endpointTest && (
            <p className="settings-notice" role="status">
              已连接：{endpointTest.version ?? "版本未知"} · GPU {endpointTest.gpu.join("、") || "不可用"} · 节点 {endpointTest.nodeCount}
            </p>
          )}
        </div>
      </section>

      <section className="settings-card" aria-labelledby="settings-preflight-title">
        <div className="settings-card-heading">
          <div>
            <h3 id="settings-preflight-title">运行预检</h3>
            <details className="settings-quiet-help"><summary>ⓘ 预检范围说明</summary>
              <p>只读检查当前已应用环境的连接、GPU、节点和生产工作流；不会切换环境、启动生成或修改工作流与模型。预检不保证真实生成成功，还需校验项目生成器与本次输入。</p>
            </details>
          </div>
          <button type="button" className="primary-action" onClick={() => void runPreflight()} disabled={preflightLoading}>
            {preflightLoading ? "正在预检……" : "立即预检"}
          </button>
        </div>
        {!preflight && !preflightLoading && <p>尚未运行预检。</p>}
        {preflight && <PreflightReportView report={preflight} />}
      </section>

      <section className="settings-card" aria-labelledby="settings-desktop-prefs-title">
        <div className="settings-card-heading">
          <div>
            <h3 id="settings-desktop-prefs-title">单机生产与桌面偏好</h3>
            <p>控制长时间批量生成时的显存释放与任务完成提醒。</p>
          </div>
        </div>
        <div className="settings-pref-list">
          <label className="settings-inline-check">
            <input
              type="checkbox"
              checked={desktopPrefs.autoFreeVramBetweenItems}
              disabled={prefsSaving}
              onChange={(event) => void updateDesktopPref({ autoFreeVramBetweenItems: event.target.checked })}
            />
            批量队列中每个镜头完成后自动请求 ComfyUI 释放显存（适合通宵跑大图/视频防 OOM）
          </label>
          <label className="settings-inline-check">
            <input
              type="checkbox"
              checked={desktopPrefs.desktopNotifications}
              disabled={prefsSaving}
              onChange={(event) => void updateDesktopPref({ desktopNotifications: event.target.checked })}
            />
            窗口在后台时，视频生成完成或失败发送系统桌面通知
          </label>
          <label className="settings-pref-row">
            <span>瞬态故障（OOM / 超时 / 瞬断）自动重试次数</span>
            <select
              aria-label="瞬态故障自动重试次数"
              value={desktopPrefs.transientRetryLimit}
              disabled={prefsSaving}
              onChange={(event) => void updateDesktopPref({ transientRetryLimit: Number(event.target.value) })}
            >
              <option value={0}>不自动重试（推荐手动排查）</option>
              <option value={1}>自动清显存后重试 1 次</option>
              <option value={2}>自动清显存后重试最多 2 次</option>
            </select>
          </label>
        </div>
      </section>

      <section className="settings-card" aria-labelledby="settings-storage-title">
        <div className="settings-card-heading">
          <div>
            <h3 id="settings-storage-title">存储与数据管理</h3>
            <p>所有项目、提示词、参考图与视频均保存在本机数据目录；可随时迁移到非系统盘或清理已退回废片。</p>
          </div>
          <button type="button" className="quiet-button" onClick={() => void refreshStorageSummary()} disabled={storageBusy}>
            {storageBusy ? "统计中……" : "刷新空间统计"}
          </button>
        </div>
        {storageSummary && (
          <dl className="settings-list settings-storage-grid">
            <div><dt>当前数据目录</dt><dd>{storageSummary.currentDataRoot}</dd></div>
            <div><dt>磁盘可用空间</dt><dd>{storageSummary.freeBytes != null ? `${formatFileSize(storageSummary.freeBytes)} 可用${storageSummary.totalBytes != null ? ` / ${formatFileSize(storageSummary.totalBytes)}` : ""}` : "--"}</dd></div>
            <div><dt>项目素材总计</dt><dd>{formatFileSize(storageSummary.projectsTotalBytes)}（{storageSummary.projects.length} 个项目）</dd></div>
            <div><dt>数据库 / 日志</dt><dd>{formatFileSize(storageSummary.databaseBytes)} / {formatFileSize(storageSummary.logsBytes)}</dd></div>
          </dl>
        )}
        {storageSummary?.pendingMigrationTarget && <p className="settings-notice" role="status">待迁移目录：{storageSummary.pendingMigrationTarget}。关闭应用后，下次启动前执行迁移，原目录保留。</p>}
        {storageSummary?.persistedCustomRoot && !storageSummary.envOverrideActive && (
          <p className="settings-notice" role="status">当前自定义数据目录：{storageSummary.persistedCustomRoot}</p>
        )}
        <div className="settings-actions">
          <button type="button" onClick={() => void openStorageDirectory("data")}>打开数据目录</button>
          <button type="button" onClick={() => void openStorageDirectory("logs")}>打开日志目录</button>
          <button type="button" onClick={() => void openStorageDirectory("backups")}>打开数据库备份目录</button>
          <button type="button" onClick={() => setMigrateConfirmOpen(true)} disabled={storageBusy || Boolean(storageSummary?.envOverrideActive)}>一键迁移数据目录（迁出 C 盘）</button>
          {storageSummary?.persistedCustomRoot && !storageSummary.envOverrideActive && (
            <button type="button" className="quiet-button" onClick={() => void handleResetDataRoot()} disabled={storageBusy}>恢复默认目录</button>
          )}
          <button type="button" className="quiet-button" onClick={() => void handleWalCheckpoint()} disabled={storageBusy}>整理数据库日志</button>
          {projectId && (
            <button type="button" className="danger-button" onClick={() => setCleanupConfirmOpen(true)} disabled={storageBusy}>
              清理当前项目已退回废片
            </button>
          )}
        </div>
      </section>

      <div className="settings-grid">
        <section className="settings-card" aria-labelledby="settings-app-info">
          <h3 id="settings-app-info">应用信息与隐私</h3>
          <dl className="settings-list">
            <div><dt>应用版本</dt><dd>{summary?.appVersion ?? "未知"}</dd></div>
            <div><dt>运行平台</dt><dd>{summary ? `${summary.platform} · ${summary.architecture}` : "未知"}</dd></div>
            <div><dt>运行模式</dt><dd>纯单机离线优先（{summary?.runMode ?? "local"}）</dd></div>
          </dl>
          <p className="settings-privacy-note">所有项目数据、提示词、参考图与生成成片均保存在本地磁盘，不上传云端、无需账号与在线授权；数据库升级前会自动在 backups 目录保留回滚快照。</p>
        </section>

        <section className="settings-card" aria-labelledby="settings-runtime-info">
          <h3 id="settings-runtime-info">运行状态</h3>
          <dl className="settings-list">
            <div><dt>本地数据库</dt><dd>{summary ? (summary.databaseHealthy ? "正常" : "暂不可用") : "未知"}</dd></div>
            <div><dt>工作流包</dt><dd>{summary?.validWorkflowPackages != null && summary.workflowPackages != null ? `${summary.validWorkflowPackages} 个可用 / ${summary.workflowPackages} 个总计` : "未知"}</dd></div>
            <div><dt>活动任务</dt><dd>{summary?.activeTaskCount ?? "未知"}</dd></div>
            <div><dt>生产队列</dt><dd>{summary?.productionBusy == null ? "未知" : summary.productionBusy ? "运行中" : "空闲"}</dd></div>
            <div><dt>日志</dt><dd>{summary ? (summary.loggingAvailable ? `可用，保留 ${summary.logRetentionDays} 天` : "不可用") : "未知"}</dd></div>
          </dl>
        </section>
      </div>

      <DiagnosticsExecutionPanel key={`${projectId ?? "none"}:${initialTaskId ?? ""}`} projectId={projectId} initialTaskId={initialTaskId} onOpenRun={onOpenRun} onOpenAudit={onOpenAudit} />
      <p role="note">生成工作流由软件内置提供；节点、模型和连接问题可在本页检查。已有运行与任务诊断不受影响。</p>

      <section className="settings-card" aria-label="本地工具登记说明">
        <p>本地工具中心显示已登记的工具、实例、版本和最近健康记录；不会自动发现、安装、启动或连接工具。</p>
        {onOpenToolHub && <button type="button" onClick={onOpenToolHub}>查看本地工具登记（高级）</button>}
      </section>

      <section className="settings-card" aria-label="运行环境高级操作">
        <section className="settings-endpoint-form" aria-labelledby="settings-environment-profiles-title">
          <div className="settings-card-heading">
            <div>
              <h4 id="settings-environment-profiles-title">已保存环境</h4>
              <p>测试已保存环境只读取连接情况，不会切换当前运行环境；只有点击“应用”才会经过安全切换与忙碌检查。</p>
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
                        <p>
                          {profile.endpoint}
                          {profile.authHeader ? " · 已配令牌" : ""}
                          {profile.allowSystemProxy ? " · 使用代理" : ""}
                        </p>
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
                      <button type="button" className="quiet-button" onClick={() => requestDeleteEnvironmentProfile(profile)} disabled={profileSaving || profileDeletingId === profile.id}>
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
            <input aria-label="环境名称" value={profileNameDraft} onChange={(event) => setProfileNameDraft(event.target.value)} maxLength={80} placeholder="环境名称，例如：云端 4090 / 本地 H3" disabled={profileSaving} />
            <input aria-label="环境 ComfyUI 地址" value={profileEndpointDraft} onChange={(event) => setProfileEndpointDraft(event.target.value)} placeholder="http://127.0.0.1:8188 或 https://gpu.example.com/comfy" spellCheck={false} disabled={profileSaving} />
            <input aria-label="环境认证令牌（可选）" type="password" value={profileAuthDraft} onChange={(event) => setProfileAuthDraft(event.target.value)} placeholder="认证令牌（可选）" spellCheck={false} disabled={profileSaving} />
            <label className="settings-inline-check">
              <input type="checkbox" checked={profileProxyDraft} onChange={(event) => setProfileProxyDraft(event.target.checked)} disabled={profileSaving} />
              代理
            </label>
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

      {showWorkflowRepairStatus && <RepairJobsStatusSection />}
      <ThemedConfirmDialog
        open={Boolean(deleteProfileTarget)}
        title="删除 ComfyUI 环境"
        message={deleteProfileTarget ? `确定删除已保存环境“${deleteProfileTarget.name}”吗？` : ""}
        confirmLabel="确认删除"
        danger
        onConfirm={() => void confirmDeleteEnvironmentProfile()}
        onCancel={() => setDeleteProfileTarget(undefined)}
      />
      <ThemedConfirmDialog
        open={migrateConfirmOpen}
        title="一键迁移数据目录（迁出 C 盘）"
        message="选择尚不存在 AIStudioData 的目标位置。当前可以继续工作；关闭应用后，下次启动前会复制最新数据库、素材、工作流和配置，验证成功后切换。原目录完整保留。"
        confirmLabel="选择目标并安排迁移"
        onConfirm={() => void handlePickDataRoot()}
        onCancel={() => setMigrateConfirmOpen(false)}
      />
      <ThemedConfirmDialog
        open={cleanupConfirmOpen}
        title="清理已退回废片视频"
        message="将永久删除当前项目中审核状态为「已退回」且未被任何镜头选用的视频文件以释放磁盘空间。正在使用的视频和已选用成片会自动保留。"
        confirmLabel="确认清理"
        danger
        onConfirm={() => void handleCleanupRejectedVideos()}
        onCancel={() => setCleanupConfirmOpen(false)}
      />
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
        <div><dt>解释器版本</dt><dd>{report.pythonVersion ?? "--"}</dd></div>
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
        <div className="settings-endpoint-form settings-preflight-workflows">
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
  if (status === "READY") return "通过 · 当前预检通过";
  if (status === "WARNING") return "警告 · 存在警告";
  return "已阻断 · 预检发现阻断问题";
}

function preflightStatusDescription(status: ComfyPreflightReport["status"]): string {
  if (status === "READY") return "当前已知依赖可用";
  if (status === "WARNING") return "预检有警告，请检查已知依赖";
  return "请先处理预检发现的阻断问题";
}

function comfyStatusLabel(status: DiagnosticsSummary["comfyStatus"]): string {
  if (status === "UNKNOWN") return "未知";
  if (status === "CONNECTED") return "已连接";
  if (status === "INCOMPATIBLE") return "版本不兼容";
  return "离线";
}
