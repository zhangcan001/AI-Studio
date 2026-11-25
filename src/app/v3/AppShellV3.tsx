import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { routeProjectId, type AppRoute } from "../routes/types";
import type { ComfyStatus } from "../../types/comfy";
import { productClient } from "../../product/client";
import { subscribeRunInvalidation } from "../../product/runInvalidation";
import type { CreationShotSummary, ProductRun } from "../../product/types";
import { ShellIcon, ShellNavigation, ShellTopbar, type TopbarQueueSummary } from "./ShellChrome";
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
  const [activeRuns, setActiveRuns] = useState<{ projectId: string; items: ProductRun[] }>();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState("");
  const [paletteIndex, setPaletteIndex] = useState(0);
  const [paletteShots, setPaletteShots] = useState<{ projectId: string; items: CreationShotSummary[] }>();
  const paletteDialog = useRef<HTMLDialogElement>(null);
  const paletteInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const media = window.matchMedia?.("(max-width: 960px)");
    const resize = () => setCollapsed(media?.matches ?? false);
    media?.addEventListener("change", resize);
    return () => media?.removeEventListener("change", resize);
  }, []);

  // Read-only active queue/run status projection for global topbar visibility across Library/Overview/Settings.
  useEffect(() => {
    setActiveRuns(undefined);
    const listRuns = productClient.run?.list;
    if (!projectId || typeof listRuns !== "function") return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const loadActive = () => {
      void Promise.resolve().then(() => listRuns(projectId, "active", null)).then(page => {
        if (!alive || !page?.items) return;
        const owned = page.items.filter(item => item.projectId === projectId && ["QUEUED", "RUNNING", "PAUSED"].includes(item.status));
        setActiveRuns({ projectId, items: owned });
      }).catch(() => { /* Topbar pill is strictly opportunistic and never blocks workspace rendering */ });
    };
    loadActive();
    const unsubscribe = subscribeRunInvalidation(changedProject => {
      if (changedProject !== projectId) return;
      clearTimeout(timer);
      timer = setTimeout(loadActive, 250);
    });
    const poll = window.setInterval(() => {
      if (document.visibilityState !== "hidden") loadActive();
    }, 8000);
    return () => { alive = false; clearTimeout(timer); window.clearInterval(poll); unsubscribe(); };
  }, [projectId]);

  useEffect(() => {
    if (!paletteOpen) return;
    setPaletteQuery("");
    setPaletteIndex(0);
    paletteDialog.current?.showModal?.();
    paletteInput.current?.focus();
    const listShots = productClient.creation?.shotsList;
    if (!projectId || typeof listShots !== "function") return;
    let alive = true;
    void Promise.resolve().then(() => listShots(projectId)).then(shots => {
      if (!alive || !Array.isArray(shots)) return;
      const ordered = shots
        .filter(s => s.projectId === projectId)
        .sort((a, b) => a.ordinal - b.ordinal || a.id.localeCompare(b.id))
        .map(s => ({ id: s.id, name: s.name, ordinal: s.ordinal }));
      setPaletteShots({ projectId, items: ordered });
    }).catch(() => { if (alive) setPaletteShots({ projectId, items: [] }); });
    return () => { alive = false; };
  }, [paletteOpen, projectId]);

  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (event.repeat) return;
      const key = event.key.toLowerCase();
      if ((event.altKey && key === "k") || ((event.ctrlKey || event.metaKey) && event.shiftKey && key === "k")) {
        event.preventDefault();
        setPaletteOpen(open => !open);
        return;
      }
      if ((event.ctrlKey || event.metaKey) && !event.shiftKey && key === "k" && projectId) {
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

  const ownedRuns = projectId && activeRuns?.projectId === projectId ? activeRuns.items : [];
  const queueSummary: TopbarQueueSummary | undefined = useMemo(() => {
    if (!projectId || !ownedRuns.length) return undefined;
    const running = ownedRuns.filter(r => r.status === "RUNNING");
    const paused = ownedRuns.filter(r => r.status === "PAUSED");
    const primary = running[0] ?? paused[0] ?? ownedRuns[0];
    const done = primary.progress.succeeded + primary.progress.failed + primary.progress.cancelled;
    const progressPart = primary.progress.total > 0 ? ` ${done}/${primary.progress.total}` : "";
    const statusWord = running.length ? "队列生成中" : paused.length ? "队列已暂停" : "队列排队中";
    return {
      label: `${statusWord}${progressPart}${ownedRuns.length > 1 ? ` · ${ownedRuns.length} 批` : ""}`,
      detail: `${primary.title} · 点击查看运行进度`,
      onOpen: () => navigate({ kind: "runs", projectId, filter: "active", run: primary.ref }),
    };
  }, [projectId, ownedRuns, navigate]);

  const paletteItems = useMemo(() => {
    const items: { id: string; label: string; group: string; target: AppRoute }[] = [];
    if (projectId) {
      items.push(
        { id: "page-overview", label: "项目概览", group: "页面", target: { kind: "project", projectId, page: "overview" } },
        { id: "page-storyboard", label: "创作 · 分镜工作台", group: "页面", target: { kind: "create", projectId, stage: "video" } },
        { id: "page-runs", label: "近期运行与生产队列", group: "页面", target: { kind: "runs", projectId } },
        { id: "page-library", label: "素材库（媒体 / 角色 / 场景 / 道具 / 构图）", group: "页面", target: { kind: "library", projectId } },
        { id: "page-generators", label: "项目生成器设置", group: "页面", target: { kind: "project-settings", projectId, section: "generators" } },
        { id: "page-projects", label: "项目管理与归档备份", group: "页面", target: { kind: "project-list" } },
      );
      const shots = paletteShots?.projectId === projectId ? paletteShots.items : [];
      for (const shot of shots) {
        items.push({
          id: `shot-${shot.id}`,
          label: `第 ${shot.ordinal + 1} 镜 · ${shot.name}`,
          group: "镜头",
          target: { kind: "create", projectId, stage: "video", shotId: shot.id },
        });
      }
    } else {
      items.push({ id: "page-projects", label: "项目列表", group: "页面", target: { kind: "project-list" } });
    }
    items.push({ id: "page-settings", label: "系统设置", group: "设置", target: { kind: "system-settings", section: "general", returnTo: settingsReturn } });
    const q = paletteQuery.trim().toLowerCase();
    return q ? items.filter(item => `${item.group} ${item.label}`.toLowerCase().includes(q)) : items;
  }, [projectId, paletteShots, paletteQuery, settingsReturn]);

  const choosePaletteItem = (target: AppRoute) => {
    setPaletteOpen(false);
    navigate(target);
  };

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
          <button type="button" title="高级工具" onClick={() => navigate({ kind: "system-settings", section: "advanced-tools", returnTo: settingsReturn })}>高级工具</button>
        </nav>}
        {route.kind === "library" && projectId && <button type="button" title="提示词" onClick={() => navigate({ kind: "library", projectId, filter: "prompts" })}>提示词</button>}
      </div>
      <button type="button" className="v3-collapse" aria-label={collapsed ? "展开导航" : "折叠导航"} title={collapsed ? "展开导航" : "折叠导航"} aria-expanded={!collapsed} onClick={() => setCollapsed(value => !value)}><ShellIcon name="collapse" /><span className="v3-nav-label">折叠导航</span></button>
    </aside>
    <div className="v3-layout">
      <ShellTopbar route={route} projectName={projectName} projectSelector={projectSelector} runtime={runtime} back={back} queueSummary={queueSummary} onOpenPalette={projectId ? () => setPaletteOpen(true) : undefined} onOpenProjects={route.kind !== "project-list" ? () => navigate({ kind: "project-list" }) : undefined} />
      {(route.kind === "system-settings" || route.kind === "project-settings") && <nav className="v3-settings-bar" aria-label="设置分区切换">
        <button type="button" aria-current={route.kind === "system-settings" && route.section === "general" ? "page" : undefined} onClick={() => navigate({ kind: "system-settings", section: "general", returnTo: settingsReturn })}>运行环境与诊断</button>
        {projectId && <button type="button" aria-current={route.kind === "project-settings" && route.section === "general" ? "page" : undefined} onClick={() => navigate({ kind: "project-settings", projectId, section: "general" })}>当前项目与备份</button>}
        {projectId && <button type="button" aria-current={route.kind === "project-settings" && route.section === "generators" ? "page" : undefined} onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>默认视频生成器</button>}
        <button type="button" aria-current={route.kind === "system-settings" && route.section === "advanced-tools" ? "page" : undefined} onClick={() => navigate({ kind: "system-settings", section: "advanced-tools", returnTo: settingsReturn })}>本地工具中心</button>
      </nav>}
      <main id="v3-main" className="v3-main" tabIndex={-1}>{children}</main>
    </div>
    {paletteOpen && <dialog ref={paletteDialog} open={typeof HTMLDialogElement.prototype.showModal !== "function" ? true : undefined} className="v3-palette" aria-label="快速跳转面板" onCancel={e => { e.preventDefault(); setPaletteOpen(false); }} onClick={e => { if (e.target === paletteDialog.current) setPaletteOpen(false); }}>
      <div className="v3-palette-box">
        <header className="v3-palette-head">
          <input ref={paletteInput} type="search" aria-label="搜索页面或镜头" placeholder="输入页面名称或镜头名，回车跳转…" value={paletteQuery} onChange={e => { setPaletteQuery(e.target.value); setPaletteIndex(0); }} onKeyDown={e => {
            if (e.key === "Escape") { e.preventDefault(); setPaletteOpen(false); }
            else if (e.key === "ArrowDown") { e.preventDefault(); setPaletteIndex(i => paletteItems.length ? (i + 1) % paletteItems.length : 0); }
            else if (e.key === "ArrowUp") { e.preventDefault(); setPaletteIndex(i => paletteItems.length ? (i - 1 + paletteItems.length) % paletteItems.length : 0); }
            else if (e.key === "Enter" && paletteItems[paletteIndex]) { e.preventDefault(); choosePaletteItem(paletteItems[paletteIndex].target); }
          }} />
          <button type="button" onClick={() => setPaletteOpen(false)}>关闭</button>
        </header>
        <ul className="v3-palette-list" role="listbox" aria-label="跳转目标">
          {paletteItems.map((item, idx) => <li key={item.id}><button type="button" role="option" aria-selected={idx === paletteIndex} className={idx === paletteIndex ? "v3-palette-active" : undefined} onMouseEnter={() => setPaletteIndex(idx)} onClick={() => choosePaletteItem(item.target)}><span>{item.label}</span><small>{item.group}</small></button></li>)}
          {!paletteItems.length && <li className="v3-palette-empty">没有匹配的页面或镜头</li>}
        </ul>
      </div>
    </dialog>}
  </div>;
}
