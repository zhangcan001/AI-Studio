import { useCallback, useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { ProjectOverview } from "../../product/types";
import type { AppRoute } from "../routes/types";
import { projectDisplayName } from "../../i18n/statusLabels";
import { runtimeReadinessPresentation } from "./projectOverviewReadiness";
import { subscribeRunInvalidation } from "../../product/runInvalidation";

/** Translate the backend's chosen action, never recompute project priorities. */
export function overviewAction(projectId: string, action: ProjectOverview["nextAction"]): { label: string; route: AppRoute } {
  const shotId = action.shotId ?? undefined;
  switch (action.kind) {
    case "NO_SHOTS": return { label: "创建第一个镜头", route: { kind: "create", projectId, stage: "video" } };
    case "COMFY_BLOCKED": return { label: "检查运行环境", route: { kind: "system-settings", section: "general", returnTo: { kind: "project", projectId, page: "overview" } } };
    case "STRUCTURAL_BLOCKED": return { label: "检查项目设置", route: { kind: "project-settings", projectId, section: "general" } };
    case "ACTIVE_PRODUCTION":
    case "REVIEW_REQUIRED":
    case "AUTO_RESUMABLE": return { label: action.kind === "ACTIVE_PRODUCTION" ? "查看进行中的运行" : "查看待处理运行", route: { kind: "runs", projectId, run: action.taskId ? { source: "task", id: action.taskId } : action.batchId ? { source: "queue-batch", id: action.batchId } : undefined, filter: action.kind === "ACTIVE_PRODUCTION" ? "active" : "failed", context: { shotId } } };
    case "IMAGE_REVIEW": return { label: "查看图片结果", route: { kind: "create", projectId, shotId, stage: "image" } };
    case "VIDEO_REVIEW": return { label: "查看视频结果", route: { kind: "create", projectId, shotId, stage: "video" } };
    case "COMPLETE": return { label: "查看完成的结果", route: { kind: "library", projectId, resource: action.assetId ? { kind: "asset", id: action.assetId } : undefined } };
    default: return { label: "继续镜头创作", route: { kind: "create", projectId, shotId, stage: "video" } };
  }
}

function nextActionDescription(kind: ProjectOverview["nextAction"]["kind"]): string {
  switch (kind) {
    case "NO_SHOTS": return "项目还没有分镜镜头，先创建第一个镜头或批量导入素材开始制作。";
    case "COMFY_BLOCKED": return "运行环境尚未连接或未通过预检，请先在系统设置中完成检查。";
    case "STRUCTURAL_BLOCKED": return "项目配置存在结构阻断，请先检查项目与默认生成器设置。";
    case "ACTIVE_PRODUCTION": return "后台正在执行视频生产任务，可随时查看实时进度与队列状态。";
    case "REVIEW_REQUIRED":
    case "AUTO_RESUMABLE": return "存在失败或待处理的运行记录，可重试失败项或调整历史输入。";
    case "IMAGE_REVIEW":
    case "VIDEO_REVIEW": return "已有新生成的候选结果等待审片，前往创作台确认并选用。";
    case "COMPLETE": return "当前镜头生产已完成，可前往素材库浏览、复用或导出成片。";
    default: return "进入分镜创作台继续编写提示词、绑定角色场景或发起视频生成。";
  }
}

function readinessTone(value: string | null | undefined): "ok" | "warn" | "bad" | "muted" {
  if (value === "CONNECTED" || value === "READY") return "ok";
  if (value === "WARNING") return "warn";
  if (value === "OFFLINE" || value === "INCOMPATIBLE" || value === "BLOCKED") return "bad";
  return "muted";
}

export function ProjectOverviewPage({ projectId, navigate }: { projectId: string; navigate: (route: AppRoute) => void }) {
  const [state, setState] = useState<{ projectId: string; data?: ProjectOverview; error?: string }>({ projectId });
  const [refreshing, setRefreshing] = useState(false);
  const manualRefreshRef = useRef<(() => Promise<void>) | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    let reading = false;
    let pending = false;
    let shouldPoll = false;
    setState({ projectId });
    async function refresh(manual = false) {
      reading = true;
      if (manual && !cancelled) setRefreshing(true);
      try {
        const data = await productClient.project.getOverview(projectId);
        shouldPoll = (data.activeRuns.runningBatches + data.activeRuns.pausedBatches + data.activeRuns.activeTasks) > 0;
        if (!cancelled) {
          setState({ projectId, data });
          if (manual) {
            window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "已刷新项目概览", tone: "info" } }));
          }
        }
      } catch (error: unknown) {
        shouldPoll = true;
        if (!cancelled) setState(previous => ({
          projectId,
          data: previous.projectId === projectId ? previous.data : undefined,
          error: normalizeProductError(error).message,
        }));
      } finally {
        reading = false;
        if (manual && !cancelled) setRefreshing(false);
        if (!cancelled && pending) {
          pending = false;
          void refresh(false);
        }
      }
    }
    manualRefreshRef.current = () => refresh(true);
    const unsubscribe = subscribeRunInvalidation((changed, options) => {
      if (changed !== projectId) return;
      if (options?.statusOnly && shouldPoll) return;
      if (reading) pending = true;
      else void refresh(false);
    });
    const timer = window.setInterval(() => {
      if (shouldPoll && !reading && document.visibilityState !== "hidden") void refresh(false);
    }, 5000);
    void refresh(false);
    return () => {
      cancelled = true;
      manualRefreshRef.current = undefined;
      unsubscribe();
      window.clearInterval(timer);
    };
  }, [projectId]);

  const handleManualRefresh = useCallback(() => {
    void manualRefreshRef.current?.();
  }, []);

  // A late response or one render during project switching cannot show A in B.
  if (state.projectId !== projectId || (!state.data && !state.error)) {
    return (
      <section className="v3-overview v3-overview-loading" aria-label="项目概览">
        <p role="status" className="v3-overview-status-note">正在加载项目概览…</p>
        <div className="v3-overview-skeleton-hero" aria-hidden="true" />
        <div className="v3-overview-skeleton-grid" aria-hidden="true">
          <span className="v3-overview-skeleton-card" />
          <span className="v3-overview-skeleton-card" />
          <span className="v3-overview-skeleton-card" />
          <span className="v3-overview-skeleton-card" />
        </div>
      </section>
    );
  }
  if (!state.data) {
    return (
      <section className="v3-overview v3-overview-error-box" aria-label="项目概览">
        <p role="alert">{state.error}</p>
        <button type="button" onClick={handleManualRefresh} disabled={refreshing}>
          {refreshing ? "正在重试…" : "重新加载"}
        </button>
      </section>
    );
  }

  const data = state.data;
  const action = overviewAction(projectId, data.nextAction);
  const readiness = runtimeReadinessPresentation(data.runtimeReadiness);
  const totalShots = data.progress.total;
  const completedPct = totalShots > 0 ? Math.min(100, Math.round((data.progress.completed / totalShots) * 100)) : 0;
  const failedPct = totalShots > 0 ? Math.min(100 - completedPct, Math.round((data.progress.failed / totalShots) * 100)) : 0;
  const hasActiveRuns = data.activeRuns.runningBatches + data.activeRuns.pausedBatches + data.activeRuns.activeTasks > 0;
  const connectionTone = readinessTone(data.runtimeReadiness.connection);
  const preflightTone = readinessTone(data.runtimeReadiness.status);
  const workflowTone = data.runtimeReadiness.status === null
    ? "muted"
    : data.runtimeReadiness.workflowTotal > 0 && data.runtimeReadiness.workflowReady === data.runtimeReadiness.workflowTotal
      ? "ok"
      : data.runtimeReadiness.workflowReady > 0
        ? "warn"
        : "bad";
  const resourceTone = data.runtimeReadiness.status === null
    ? "muted"
    : (data.runtimeReadiness.runtimeBusy || data.runtimeReadiness.productionBusy || data.runtimeReadiness.activeTaskCount > 0)
      ? "warn"
      : "ok";

  return (
    <section className="v3-overview" aria-label="项目概览">
      {state.error && (
        <div className="v3-overview-refresh-warn" role="status">
          <span>暂时无法刷新项目概览。{state.error}</span>
          <button type="button" onClick={handleManualRefresh} disabled={refreshing}>重试</button>
        </div>
      )}

      <header className="v3-overview-header">
        <div className="v3-overview-title">
          <div className="v3-overview-eyebrow-row">
            <span className="v3-eyebrow">项目概览</span>
            {hasActiveRuns ? (
              <span className="v3-overview-pill tone-info">后台运行中</span>
            ) : data.blockingIssues.length > 0 ? (
              <span className="v3-overview-pill tone-bad">{data.blockingIssues.length} 项阻断</span>
            ) : (
              <span className="v3-overview-pill tone-ok">状态正常</span>
            )}
          </div>
          <h1>{projectDisplayName(data.project.id, data.project.name)}</h1>
          {data.project.description && <p className="v3-overview-desc" title={data.project.description}>{data.project.description}</p>}
        </div>
        <div className="v3-overview-shortcuts">
          <button type="button" onClick={handleManualRefresh} disabled={refreshing}>{refreshing ? "刷新中…" : "刷新"}</button>
          <button type="button" onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>默认生成器</button>
          <button type="button" onClick={() => navigate({ kind: "project-list" })}>项目管理与归档</button>
        </div>
      </header>

      <section className="v3-next-action">
        <div className="v3-next-action-copy">
          <h2>下一步</h2>
          <p>{nextActionDescription(data.nextAction.kind)}</p>
        </div>
        <button type="button" className="primary-button" onClick={() => navigate(action.route)}>{action.label}</button>
      </section>

      <nav className="v3-quick-nav" aria-label="项目快捷入口">
        <button type="button" onClick={() => navigate({ kind: "create", projectId, stage: "video" })}>
          <strong>分镜创作台</strong>
          <span>编写提示词、绑定角色并生成视频</span>
        </button>
        <button type="button" onClick={() => navigate({ kind: "library", projectId, filter: "characters" })}>
          <strong>角色与场景</strong>
          <span>管理角色、场景、道具与构图参考</span>
        </button>
        <button type="button" onClick={() => navigate({ kind: "runs", projectId })}>
          <strong>运行队列</strong>
          <span>监控批量队列、审片与失败重试</span>
        </button>
        <button type="button" onClick={() => navigate({ kind: "library", projectId })}>
          <strong>项目素材库</strong>
          <span>浏览生成的视频成片与导入图片</span>
        </button>
      </nav>

      <div className="v3-overview-grid">
        <section className="v3-progress-card">
          <div className="v3-card-head">
            <div className="v3-card-head-title">
              <h2>视频生产进度</h2>
              {totalShots > 0 && <span className="v3-progress-pct">完成率 {completedPct}%</span>}
            </div>
            <button type="button" className="v3-card-link" onClick={() => navigate({ kind: "create", projectId, stage: "video" })}>进入分镜工作台</button>
          </div>
          {totalShots > 0 && (
            <div className="v3-progress-track" aria-hidden="true">
              <span className="v3-progress-fill" style={{ width: `${completedPct}%` }} />
              {failedPct > 0 && <span className="v3-progress-fail" style={{ width: `${failedPct}%` }} />}
            </div>
          )}
          <dl className="v3-metrics">
            <div>
              <dt>镜头</dt>
              <dd>{data.progress.total}</dd>
            </div>
            <div className={data.progress.completed > 0 ? "v3-metric-ok" : undefined}>
              <dt>已完成</dt>
              <dd>{data.progress.completed}</dd>
            </div>
            <div className={data.progress.failed > 0 ? "v3-metric-bad" : undefined}>
              <dt>失败</dt>
              <dd>{data.progress.failed}</dd>
            </div>
          </dl>
        </section>

        <section className={data.blockingIssues.length ? "v3-card-alert" : undefined}>
          <div className="v3-card-head">
            <h2>阻断与提醒</h2>
            {data.blockingIssues.length > 0 && <span className="v3-overview-pill tone-bad">{data.blockingIssues.length} 项</span>}
          </div>
          {data.blockingIssues.length ? (
            <>
              <ul className="v3-blocking-list">
                {data.blockingIssues.map((issue, i) => <li key={i}>{issue.title}</li>)}
              </ul>
              <div className="v3-card-actions">
                <button type="button" onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>检查默认生成器</button>
                <button type="button" onClick={() => navigate({ kind: "system-settings", section: "general", returnTo: { kind: "project", projectId, page: "overview" } })}>系统设置</button>
              </div>
            </>
          ) : (
            <p className="v3-healthy-note">暂无项目阻断</p>
          )}
        </section>

        <section aria-label="运行准备">
          <div className="v3-card-head">
            <h2>运行准备</h2>
          </div>
          <div className="v3-readiness-grid">
            <p className={`v3-readiness-item tone-${connectionTone}`}>连接：{readiness.connectionLabel}</p>
            <p className={`v3-readiness-item tone-${preflightTone}`}>运行预检：{readiness.preflightLabel}</p>
            <p className={`v3-readiness-item tone-${workflowTone}`}>生产工作流：{readiness.workflowsLabel}</p>
            <p className={`v3-readiness-item tone-${resourceTone}`}>运行资源：{readiness.resourcesLabel}</p>
          </div>
          {readiness.needsAttention && (
            <button type="button" onClick={() => navigate({ kind: "system-settings", section: "general", returnTo: { kind: "project", projectId, page: "overview" } })}>检查运行环境</button>
          )}
        </section>

        <section>
          <div className="v3-card-head">
            <h2>当前运行</h2>
            {hasActiveRuns && <span className="v3-overview-pill tone-info">活跃</span>}
          </div>
          <div className="v3-mini-kpis">
            <div className={data.activeRuns.runningBatches > 0 ? "v3-mini-kpi active" : "v3-mini-kpi"}>
              <span>运行中队列</span>
              <strong>{data.activeRuns.runningBatches}</strong>
            </div>
            <div className={data.activeRuns.pausedBatches > 0 ? "v3-mini-kpi warn" : "v3-mini-kpi"}>
              <span>暂停队列</span>
              <strong>{data.activeRuns.pausedBatches}</strong>
            </div>
            <div className={data.activeRuns.activeTasks > 0 ? "v3-mini-kpi active" : "v3-mini-kpi"}>
              <span>活动任务</span>
              <strong>{data.activeRuns.activeTasks}</strong>
            </div>
          </div>
          <p className="v3-card-sub">运行中队列 {data.activeRuns.runningBatches} · 暂停队列 {data.activeRuns.pausedBatches} · 活动任务 {data.activeRuns.activeTasks}</p>
          <div className="v3-card-actions">
            <button type="button" onClick={() => navigate({ kind: "runs", projectId })}>查看运行</button>
            {data.progress.failed > 0 && (
              <button type="button" onClick={() => navigate({ kind: "runs", projectId, filter: "failed" })}>查看失败</button>
            )}
          </div>
        </section>

        <section>
          <div className="v3-card-head">
            <h2>项目素材与结果</h2>
          </div>
          <div className="v3-mini-kpis">
            <div className="v3-mini-kpi">
              <span>视频结果</span>
              <strong>{data.recentResults.videos}</strong>
            </div>
            <div className="v3-mini-kpi">
              <span>图片素材</span>
              <strong>{data.recentResults.images}</strong>
            </div>
          </div>
          <p className="v3-card-sub">图片素材与历史结果 {data.recentResults.images} · 视频 {data.recentResults.videos}</p>
          <div className="v3-card-actions">
            <button type="button" onClick={() => navigate({ kind: "library", projectId })}>查看素材</button>
            <button type="button" onClick={() => navigate({ kind: "library", projectId, filter: "characters" })}>角色与场景</button>
          </div>
        </section>
      </div>
    </section>
  );
}
