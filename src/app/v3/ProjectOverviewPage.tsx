import { useEffect, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { ProjectOverview } from "../../product/types";
import type { AppRoute } from "../routes/types";
import { projectDisplayName } from "../../i18n/statusLabels";
import { runtimeReadinessPresentation } from "./projectOverviewReadiness";

/** Translate the backend's chosen action, never recompute project priorities. */
export function overviewAction(projectId: string, action: ProjectOverview["nextAction"]): { label: string; route: AppRoute } {
  const shotId = action.shotId ?? undefined;
  switch (action.kind) {
    case "NO_SHOTS": return { label: "创建第一个镜头", route: { kind: "create", projectId, stage: "image" } };
    case "COMFY_BLOCKED": return { label: "检查运行环境", route: { kind: "system-settings", section: "general", returnTo: { kind: "project", projectId, page: "overview" } } };
    case "STRUCTURAL_BLOCKED": return { label: "检查项目设置", route: { kind: "project-settings", projectId, section: "general" } };
    case "ACTIVE_PRODUCTION":
    case "REVIEW_REQUIRED":
    case "AUTO_RESUMABLE": return { label: action.kind === "ACTIVE_PRODUCTION" ? "查看进行中的运行" : "查看待处理运行", route: { kind: "runs", projectId, run: action.taskId ? { source: "task", id: action.taskId } : action.batchId ? { source: "queue-batch", id: action.batchId } : undefined, filter: action.kind === "ACTIVE_PRODUCTION" ? "active" : "failed", context: { shotId } } };
    case "IMAGE_REVIEW": return { label: "查看图片结果", route: { kind: "create", projectId, shotId, stage: "image" } };
    case "VIDEO_REVIEW": return { label: "查看视频结果", route: { kind: "create", projectId, shotId, stage: "video" } };
    case "COMPLETE": return { label: "查看完成的结果", route: { kind: "library", projectId, resource: action.assetId ? { kind: "asset", id: action.assetId } : undefined } };
    default: return { label: "继续镜头创作", route: { kind: "create", projectId, shotId, stage: "image" } };
  }
}
export function ProjectOverviewPage({ projectId, navigate }: { projectId: string; navigate: (route: AppRoute) => void }) {
  const [state, setState] = useState<{ projectId: string; data?: ProjectOverview; error?: string }>({ projectId });
  useEffect(() => {
    let cancelled = false;
    setState({ projectId });
    void productClient.project.getOverview(projectId).then((data) => {
      if (!cancelled) setState({ projectId, data });
    }).catch((error: unknown) => { if (!cancelled) setState({ projectId, error: normalizeProductError(error).message }); });
    return () => { cancelled = true; };
  }, [projectId]);
  // A late response or one render during project switching cannot show A in B.
  if (state.projectId !== projectId || (!state.data && !state.error)) return <p role="status">正在加载项目概览…</p>;
  if (state.error) return <p role="alert">{state.error}</p>;
  const data = state.data!;
  const action = overviewAction(projectId, data.nextAction);
  const readiness = runtimeReadinessPresentation(data.runtimeReadiness);
  return <section className="v3-overview" aria-label="项目概览">
    <header><h1>{projectDisplayName(data.project.id, data.project.name)}</h1>{data.project.description && <p>{data.project.description}</p>}</header>
    <section className="v3-next-action"><h2>下一步</h2><button type="button" className="primary-button" onClick={() => navigate(action.route)}>{action.label}</button></section>
    <div className="v3-overview-grid">
      <section><h2>当前进度</h2><p>镜头 {data.progress.total} · 已完成 {data.progress.completed} · 失败 {data.progress.failed}</p></section>
      <section><h2>阻断与提醒</h2>{data.blockingIssues.length ? <ul>{data.blockingIssues.map((issue, i) => <li key={i}>{issue.title}</li>)}</ul> : <p>暂无项目阻断</p>}</section>
      <section aria-label="运行准备"><h2>运行准备</h2>
        <p>连接：{readiness.connectionLabel}</p><p>运行预检：{readiness.preflightLabel}</p>
        <p>生产工作流：{readiness.workflowsLabel}</p><p>运行资源：{readiness.resourcesLabel}</p>
        {readiness.needsAttention && <button type="button" onClick={() => navigate({ kind: "system-settings", section: "general", returnTo: { kind: "project", projectId, page: "overview" } })}>检查运行环境</button>}
      </section>
      <section><h2>当前运行</h2><p>运行中队列 {data.activeRuns.runningBatches} · 暂停队列 {data.activeRuns.pausedBatches} · 活动任务 {data.activeRuns.activeTasks}</p><button type="button" onClick={() => navigate({ kind: "runs", projectId })}>查看运行</button></section>
      <section><h2>项目结果</h2><p>图片 {data.recentResults.images} · 视频 {data.recentResults.videos}</p><button type="button" onClick={() => navigate({ kind: "library", projectId })}>查看素材</button></section>
    </div>
  </section>;
}
