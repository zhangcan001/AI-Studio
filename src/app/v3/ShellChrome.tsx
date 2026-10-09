import type { ReactNode } from "react";
import type { ComfyStatus } from "../../types/comfy";
import type { AppRoute } from "../routes/types";
import { routeTitle } from "../routes/selectors";

type IconName = "overview" | "create" | "runs" | "library" | "settings" | "collapse";
const iconPaths: Record<IconName, string> = {
  overview: "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
  create: "m8 5 11 7-11 7z M3 4v16",
  runs: "M5 5h14 M5 12h14 M5 19h14",
  library: "M3 6h7l2 2h9v12H3z M3 6V4h7l2 2",
  settings: "M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
  collapse: "M3 4h18v16H3z M8 4v16 M15 9l-3 3 3 3",
};
export function ShellIcon({ name }: { name: IconName }) {
  return <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={iconPaths[name]} /></svg>;
}
export function RuntimeConnectionIndicator({ runtime }: { runtime?: ComfyStatus }) {
  const label = !runtime ? "状态未检查" : runtime.status === "OFFLINE" ? "未连接"
    : runtime.status === "INCOMPATIBLE" ? "版本不兼容" : runtime.status !== "CONNECTED" ? "状态未知"
    : runtime.capability ? "已连接 · 能力已检查" : "已连接 · 能力未确认";
  return <span className="v3-runtime" data-status={runtime?.status ?? "unchecked"} role="status" aria-live="polite" title="连接与节点检查不代表所有生成模型就绪">
    <span className="v3-runtime-dot" aria-hidden="true" /><span>ComfyUI <span>{label}</span></span>
  </span>;
}
export function ShellTopbar({ route, projectName, projectSelector, runtime, back }: {
  route: AppRoute; projectName?: string; projectSelector: ReactNode; runtime?: ComfyStatus; back: () => void;
}) {
  return <header className="v3-header">
    <button type="button" className="v3-back" onClick={back} aria-label="返回">←</button>
    <nav className="v3-breadcrumbs" aria-label="当前位置"><span title={projectName}>{projectName ?? "项目空间"}</span><span aria-hidden="true">/</span><strong aria-current="page">{routeTitle(route)}</strong></nav>
    <label className="v3-project-picker"><span>项目切换</span>{projectSelector}</label>
    <RuntimeConnectionIndicator runtime={runtime} />
  </header>;
}
export function ShellNavigation({ route, projectId, navigate }: { route: AppRoute; projectId?: string; navigate: (route: AppRoute) => void }) {
  const settings: AppRoute = { kind: "system-settings", section: "general", returnTo: route.kind === "system-settings" ? route.returnTo : route };
  const entries: { label: string; icon: IconName; target?: AppRoute; active: boolean }[] = [
    { label: "概览", icon: "overview", target: projectId ? { kind: "project", projectId, page: "overview" } : { kind: "project-list" }, active: route.kind === "project" && route.page === "overview" || route.kind === "project-list" },
    { label: "创作", icon: "create", target: projectId ? { kind: "create", projectId, stage: "video" } : undefined, active: route.kind === "create" },
    { label: "运行", icon: "runs", target: projectId ? { kind: "runs", projectId } : undefined, active: route.kind === "runs" },
    { label: "素材库", icon: "library", target: projectId ? { kind: "library", projectId } : undefined, active: route.kind === "library" },
    { label: "设置", icon: "settings", target: settings, active: route.kind === "system-settings" || route.kind === "project-settings" },
  ];
  return <nav aria-label="项目导航" className="v3-primary-nav">{entries.map(entry => <button key={entry.label} type="button" title={entry.label} aria-label={entry.label} aria-current={entry.active ? "page" : undefined} disabled={!entry.target} onClick={() => entry.target && navigate(entry.target)}>
    <ShellIcon name={entry.icon} /><span className="v3-nav-label">{entry.label}</span>
  </button>)}</nav>;
}
