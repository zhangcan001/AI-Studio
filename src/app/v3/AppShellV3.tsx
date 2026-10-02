import { useEffect, type ReactNode } from "react";
import { routeProjectId, type AppRoute } from "../routes/types";
import { routeTitle } from "../routes/selectors";
import "./AppShellV3.css";

export interface AppShellV3Props {
  route: AppRoute;
  projectName?: string;
  projectSelector: ReactNode;
  navigate: (route: AppRoute) => void;
  back: () => void;
  children: ReactNode;
}
export function AppShellV3({ route, projectName, projectSelector, navigate, back, children }: AppShellV3Props) {
  const projectId = routeProjectId(route);
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k" && projectId) {
        event.preventDefault(); navigate({ kind: "create", projectId, stage: "image" });
      }
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [projectId, navigate]);
  const pages = projectId ? [
    { label: "概览", route: { kind: "project", projectId, page: "overview" } as AppRoute },
    { label: "创作", route: { kind: "create", projectId, stage: "image" } as AppRoute },
    { label: "运行", route: { kind: "runs", projectId } as AppRoute },
    { label: "素材库", route: { kind: "library", projectId } as AppRoute },
  ] : [];
  return <div className="v3-shell">
    <a className="v3-skip" href="#v3-main">跳到主要内容</a>
    <header className="v3-header">
      <button type="button" onClick={() => navigate({ kind: "project-list" })}>AI Studio · 项目</button>
      <label className="v3-project-picker">项目切换 {projectSelector}</label>
      <button type="button" onClick={() => navigate({ kind: "system-settings", section: "general" })}>系统设置</button>
    </header>
    <div className="v3-layout">
      <aside className="v3-sidebar">
        <strong>{projectName ?? "选择项目"}</strong>
        <nav aria-label="项目导航">{pages.map((page) => <button type="button" key={page.label} aria-current={route.kind === page.route.kind ? "page" : undefined} onClick={() => navigate(page.route)}>{page.label}</button>)}</nav>
        {projectId && <button type="button" onClick={() => navigate({ kind: "project-settings", projectId, section: "general" })}>项目设置</button>}
        {route.kind === "project-settings" && projectId && <nav aria-label="项目设置选项">
          <button type="button" onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>生成器</button>
          <button type="button" onClick={() => navigate({ kind: "system-settings", section: "advanced-workflows", returnTo: route })}>高级工作流</button>
        </nav>}
        {route.kind === "system-settings" && <button type="button" onClick={() => navigate({ kind: "system-settings", section: "advanced-tools", returnTo: route.returnTo })}>高级工具</button>}
        {route.kind === "system-settings" && <button type="button" onClick={() => navigate({ kind: "system-settings", section: "advanced-workflows", returnTo: route.returnTo })}>高级工作流</button>}
        {route.kind === "library" && projectId && <button type="button" onClick={() => navigate({ kind: "library", projectId, filter: "prompts" })}>提示词</button>}
      </aside>
      <main id="v3-main" className="v3-main" tabIndex={-1}>
        <div className="v3-breadcrumbs"><button type="button" onClick={back}>返回</button><span>{projectName ? `${projectName} / ` : ""}{routeTitle(route)}</span></div>
        {children}
      </main>
    </div>
  </div>;
}
