import { useEffect, useState, type ReactNode } from "react";
import { routeProjectId, type AppRoute } from "../routes/types";
import type { ComfyStatus } from "../../types/comfy";
import { ShellIcon, ShellNavigation, ShellTopbar } from "./ShellChrome";
import "./AppShellV3.css";

export interface AppShellV3Props {
  route: AppRoute;
  projectName?: string;
  projectSelector: ReactNode;
  runtime?: ComfyStatus;
  navigate: (route: AppRoute) => void;
  back: () => void;
  children: ReactNode;
}
export function AppShellV3({ route, projectName, projectSelector, runtime, navigate, back, children }: AppShellV3Props) {
  const projectId = routeProjectId(route);
  const [collapsed, setCollapsed] = useState(() => typeof window !== "undefined" && (window.matchMedia?.("(max-width: 960px)").matches ?? false));
  useEffect(() => {
    const media = window.matchMedia?.("(max-width: 960px)");
    const resize = () => setCollapsed(media?.matches ?? false);
    media?.addEventListener("change", resize);
    return () => media?.removeEventListener("change", resize);
  }, []);
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (!event.repeat && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k" && projectId) {
        event.preventDefault();
        // All exits use the host's existing draft/project navigation guard.
        navigate({ kind: "create", projectId, stage: "video" });
      }
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [projectId, navigate]);
  useEffect(() => { document.getElementById("v3-main")?.focus({ preventScroll: true }); }, [route]);
  const settingsReturn = route.kind === "system-settings" ? route.returnTo : route;
  return <div className="v3-shell" data-collapsed={collapsed}>
    <a className="v3-skip" href="#v3-main">跳到主要内容</a>
    <aside className="v3-sidebar">
      <button type="button" className="v3-brand" title="AI Studio · MiniMax Video" onClick={() => navigate({ kind: "project-list" })}>
        <span className="v3-brand-mark" aria-hidden="true">AI</span><span className="v3-nav-label"><strong>AI Studio</strong><small>MiniMax Video</small></span>
      </button>
      <div className="v3-project-context" title={projectName ?? "选择项目"}><span className="v3-nav-label">当前项目</span><strong className="v3-nav-label">{projectName ?? "选择项目"}</strong></div>
      <ShellNavigation route={route} projectId={projectId} navigate={navigate} />
      <div className="v3-secondary">
        {(route.kind === "system-settings" || route.kind === "project-settings") && <nav aria-label="设置上下文">
          {projectId && <button type="button" title="项目设置" onClick={() => navigate({ kind: "project-settings", projectId, section: "general" })}>项目设置</button>}
          {route.kind === "project-settings" && projectId && <>
            <button type="button" onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>生成器</button>
            <details><summary>高级项目操作</summary>
              {[
                ["advanced-project", "生产分析与 Handoff 导入"], ["advanced-shots", "镜头结构与一致性编辑"],
                ["advanced-production", "高级生产编辑"], ["advanced-review", "产物审核工作区"], ["advanced-tasks", "任务审计与诊断"],
              ].map(([section, label]) => <button type="button" key={section} onClick={() => navigate({ kind: "project-settings", projectId, section })}>{label}</button>)}
            </details>
          </>}
          <button type="button" title="高级工作流" onClick={() => navigate({ kind: "system-settings", section: "advanced-workflows", returnTo: settingsReturn })}>高级工作流</button>
          <button type="button" title="高级工具" onClick={() => navigate({ kind: "system-settings", section: "advanced-tools", returnTo: settingsReturn })}>高级工具</button>
        </nav>}
        {route.kind === "library" && projectId && <button type="button" title="提示词" onClick={() => navigate({ kind: "library", projectId, filter: "prompts" })}>提示词</button>}
      </div>
      <button type="button" className="v3-collapse" aria-label={collapsed ? "展开导航" : "折叠导航"} title={collapsed ? "展开导航" : "折叠导航"} aria-expanded={!collapsed} onClick={() => setCollapsed(value => !value)}><ShellIcon name="collapse" /><span className="v3-nav-label">折叠导航</span></button>
    </aside>
    <div className="v3-layout">
      <ShellTopbar route={route} projectName={projectName} projectSelector={projectSelector} runtime={runtime} back={back} />
      <main id="v3-main" className="v3-main" tabIndex={-1}>{children}</main>
    </div>
  </div>;
}
