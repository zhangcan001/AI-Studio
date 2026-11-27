import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { routeProjectId, type AppRoute } from "../routes/types";
import type { ComfyStatus } from "../../types/comfy";
import { productClient } from "../../product/client";
import { subscribeRunInvalidation } from "../../product/runInvalidation";
import type { CreationShotSummary, ProductRun } from "../../product/types";
import { useTaskStore } from "../../stores/taskStore";
import { ShellIcon, ShellNavigation, ShellTopbar, type TopbarQueueSummary } from "./ShellChrome";
import "./AppShellV3.css";

function formatElapsedMmSs(startedAt?: string, nowMs = Date.now()): string | undefined {
  if (!startedAt) return undefined;
  const start = Date.parse(startedAt);
  if (!Number.isFinite(start)) return undefined;
  const sec = Math.max(0, Math.floor((nowMs - start) / 1000));
  return `${String(Math.floor(sec / 60)).padStart(2, "0")}:${String(sec % 60).padStart(2, "0")}`;
}

const SHORTCUT_GROUPS: { title: string; items: [string, string][] }[] = [
  {
    title: "全局导航",
    items: [
      ["Alt + K", "快速跳转页面与镜头"],
      ["Ctrl + Shift + K", "快速跳转面板（备选）"],
      ["Ctrl + K", "直达创作台 / 镜头内打开跳转"],
      ["?", "打开 / 关闭快捷键速查"],
      ["Esc", "关闭当前弹窗或浮层"],
    ],
  },
  {
    title: "创作与预览",
    items: [
      ["Ctrl + Enter", "提交当前镜头生成"],
      ["Space", "播放 / 暂停预览视频、灯箱与顺片"],
      ["[ / ]", "单镜编辑时切换上一镜 / 下一镜"],
      ["@", "在提示词中插入角色或场景参考"],
      ["Alt + 点击参考图", "灯箱放大查看高清参考图"],
    ],
  },
  {
    title: "运行与审片",
    items: [
      ["↑ / ↓", "运行页列表快速切换运行记录"],
      ["← / →", "审片弹窗切换上一个 / 下一个候选"],
      ["A / X / S", "审片弹窗快速通过 / 退回 / 选用"],
    ],
  },
];

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
  const [toasts, setToasts] = useState<{ id: number; message: string; tone: "info" | "success" | "warning" | "error" }[]>([]);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [nowTick, setNowTick] = useState(() => Date.now());
  const toastSeq = useRef(0);
  const paletteDialog = useRef<HTMLDialogElement>(null);
  const paletteInput = useRef<HTMLInputElement>(null);
  const shortcutsDialog = useRef<HTMLDialogElement>(null);
  const settingsMoreRef = useRef<HTMLDetailsElement>(null);
  const recentTasks = useTaskStore(state => state.recentTasks);
  const liveTask = useMemo(
    () => (projectId ? recentTasks.find(t => t.projectId === projectId && ["RUNNING", "COLLECTING", "PREPARING"].includes(t.status)) : undefined),
    [projectId, recentTasks],
  );
  useEffect(() => {
    if (!liveTask) return;
    setNowTick(Date.now());
    const id = window.setInterval(() => setNowTick(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [liveTask?.id, liveTask?.status]);
  useEffect(() => {
    if (shortcutsOpen) shortcutsDialog.current?.showModal?.();
  }, [shortcutsOpen]);

  useEffect(() => {
    const closeOutside = (e: MouseEvent) => {
      if (settingsMoreRef.current?.open && e.target instanceof Node && !settingsMoreRef.current.contains(e.target)) {
        settingsMoreRef.current.open = false;
      }
    };
    const closeEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape" && settingsMoreRef.current?.open) settingsMoreRef.current.open = false;
    };
    document.addEventListener("mousedown", closeOutside);
    window.addEventListener("keydown", closeEsc);
    return () => {
      document.removeEventListener("mousedown", closeOutside);
      window.removeEventListener("keydown", closeEsc);
    };
  }, []);

  useEffect(() => {
    const timers = new Map<number, ReturnType<typeof setTimeout>>();
    const onToast = (event: Event) => {
      const detail = (event as CustomEvent<{ message?: string; tone?: "info" | "success" | "warning" | "error" }>).detail;
      const message = detail?.message?.trim();
      if (!message) return;
      const id = ++toastSeq.current;
      const tone = detail?.tone ?? "info";
      setToasts(prev => [...prev.slice(-2), { id, message, tone }]);
      const timer = setTimeout(() => {
        timers.delete(id);
        setToasts(prev => prev.filter(t => t.id !== id));
      }, 3200);
      timers.set(id, timer);
    };
    window.addEventListener("ai-studio:toast", onToast);
    return () => {
      window.removeEventListener("ai-studio:toast", onToast);
      timers.forEach(clearTimeout);
    };
  }, []);

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
    return () => { alive = false; clearTimeout(timer); unsubscribe(); };
  }, [projectId]);

  const hasActiveRuns = Boolean(projectId && activeRuns?.projectId === projectId && activeRuns.items.length > 0);
  useEffect(() => {
    const listRuns = productClient.run?.list;
    if (!projectId || !hasActiveRuns || typeof listRuns !== "function") return;
    let alive = true;
    const poll = window.setInterval(() => {
      if (document.visibilityState === "hidden") return;
      void Promise.resolve().then(() => listRuns(projectId, "active", null)).then(page => {
        if (!alive || !page?.items) return;
        const owned = page.items.filter(item => item.projectId === projectId && ["QUEUED", "RUNNING", "PAUSED"].includes(item.status));
        setActiveRuns({ projectId, items: owned });
      }).catch(() => { /* Topbar pill is strictly opportunistic and never blocks workspace rendering */ });
    }, 8000);
    return () => { alive = false; window.clearInterval(poll); };
  }, [projectId, hasActiveRuns]);

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

  const createShotId = route.kind === "create" ? route.shotId : undefined;
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (event.repeat) return;
      if (event.key === "?" && !event.ctrlKey && !event.metaKey && !event.altKey) {
        const target = event.target as HTMLElement | null;
        if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable)) return;
        const openDialog = document.querySelector("dialog[open]");
        if (openDialog && openDialog !== shortcutsDialog.current) return;
        event.preventDefault();
        setShortcutsOpen(open => !open);
        return;
      }
      const key = event.key.toLowerCase();
      const isPaletteShortcut = (event.altKey && key === "k") || ((event.ctrlKey || event.metaKey) && event.shiftKey && key === "k");
      const isQuickCreateShortcut = (event.ctrlKey || event.metaKey) && !event.shiftKey && key === "k" && Boolean(projectId);
      if (!isPaletteShortcut && !isQuickCreateShortcut) return;
      const openDialog = document.querySelector("dialog[open]");
      if (openDialog && openDialog !== paletteDialog.current) return;
      if (isPaletteShortcut || (isQuickCreateShortcut && route.kind === "create" && createShotId)) {
        event.preventDefault();
        setPaletteOpen(open => !open);
        return;
      }
      if (isQuickCreateShortcut && projectId) {
        event.preventDefault();
        // All exits use the host's existing draft/project navigation guard.
        navigate({ kind: "create", projectId, stage: "video" });
      }
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [projectId, navigate, route.kind, createShotId]);

  useEffect(() => { document.getElementById("v3-main")?.focus({ preventScroll: true }); }, [route]);
  const settingsReturn = route.kind === "system-settings" ? route.returnTo : route;

  const ownedRuns = projectId && activeRuns?.projectId === projectId ? activeRuns.items : [];
  const queueSummary: TopbarQueueSummary | undefined = useMemo(() => {
    if (!projectId || (!ownedRuns.length && !liveTask)) return undefined;
    const running = ownedRuns.filter(r => r.status === "RUNNING");
    const paused = ownedRuns.filter(r => r.status === "PAUSED");
    const primary = running[0] ?? paused[0] ?? ownedRuns[0];
    const done = primary ? primary.progress.succeeded + primary.progress.failed + primary.progress.cancelled : 0;
    const progressPart = primary && primary.progress.total > 1 ? ` ${done}/${primary.progress.total}` : "";
    const stepPart = liveTask?.progress.mode === "step" && liveTask.progress.total && liveTask.progress.total > 0
      ? ` · ${liveTask.progress.current ?? 0}/${liveTask.progress.total}步 (${Math.min(100, Math.round(((liveTask.progress.current ?? 0) / liveTask.progress.total) * 100))}%)`
      : "";
    const elapsed = liveTask ? formatElapsedMmSs(liveTask.startedAt ?? liveTask.createdAt, nowTick) : undefined;
    const elapsedPart = elapsed ? ` · ${elapsed}` : "";
    const statusWord = (running.length || liveTask) ? "生成中" : paused.length ? "队列已暂停" : "队列排队中";
    const targetRef = primary?.ref ?? (liveTask ? { source: "task" as const, id: liveTask.id } : undefined);
    return {
      label: `${statusWord}${progressPart}${stepPart}${elapsedPart}${ownedRuns.length > 1 ? ` · ${ownedRuns.length}批` : ""}`,
      detail: `${primary?.title ?? "当前视频任务"} · 点击查看实时运行进度`,
      onOpen: () => navigate({ kind: "runs", projectId, filter: "active", run: targetRef }),
    };
  }, [projectId, ownedRuns, liveTask, nowTick, navigate]);

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
      shots.forEach((shot, idx) => {
        items.push({
          id: `shot-${shot.id}`,
          label: `第 ${idx + 1} 镜 · ${shot.name}`,
          group: "镜头",
          target: { kind: "create", projectId, stage: "video", shotId: shot.id },
        });
      });
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
      <button type="button" className="v3-brand" title="AI Studio · 视频创作工作台" onClick={() => navigate({ kind: "project-list" })}>
        <span className="v3-brand-mark" aria-hidden="true">AI</span><span className="v3-nav-label"><strong>AI Studio</strong><small>视频创作工作台</small></span>
      </button>
      <div className="v3-project-context" title={projectName ?? "选择项目"}><span className="v3-nav-label">当前项目</span><strong className="v3-nav-label">{projectName ?? "选择项目"}</strong></div>
      <ShellNavigation route={route} projectId={projectId} navigate={navigate} />
      <div className="v3-secondary">
        {(route.kind === "system-settings" || route.kind === "project-settings") && <nav aria-label="设置上下文">
          {projectId && <button type="button" title="项目设置" onClick={() => navigate({ kind: "project-settings", projectId, section: "general" })}>项目设置</button>}
          <button type="button" title="高级工具" onClick={() => navigate({ kind: "system-settings", section: "advanced-tools", returnTo: settingsReturn })}>高级工具</button>
        </nav>}
      </div>
      <button type="button" className="v3-collapse" aria-label={collapsed ? "展开导航" : "折叠导航"} title={collapsed ? "展开导航" : "折叠导航"} aria-expanded={!collapsed} onClick={() => setCollapsed(value => !value)}><ShellIcon name="collapse" /><span className="v3-nav-label">折叠导航</span></button>
    </aside>
    <div className="v3-layout">
      <ShellTopbar route={route} projectName={projectName} projectSelector={projectSelector} runtime={runtime} back={back} queueSummary={queueSummary} onOpenPalette={projectId ? () => setPaletteOpen(true) : undefined} onOpenShortcuts={() => setShortcutsOpen(true)} onOpenProjects={route.kind !== "project-list" ? () => navigate({ kind: "project-list" }) : undefined} />
      {(route.kind === "system-settings" || route.kind === "project-settings") && <nav className="v3-settings-bar" aria-label="设置分区切换">
        <button type="button" aria-current={route.kind === "system-settings" && route.section === "general" ? "page" : undefined} onClick={() => navigate({ kind: "system-settings", section: "general", returnTo: settingsReturn })}>运行环境与诊断</button>
        {projectId && <button type="button" aria-current={route.kind === "project-settings" && route.section === "general" ? "page" : undefined} onClick={() => navigate({ kind: "project-settings", projectId, section: "general" })}>当前项目与备份</button>}
        {projectId && <button type="button" aria-current={route.kind === "project-settings" && route.section === "generators" ? "page" : undefined} onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>默认视频生成器</button>}
        <button type="button" aria-current={route.kind === "system-settings" && route.section === "advanced-tools" ? "page" : undefined} onClick={() => navigate({ kind: "system-settings", section: "advanced-tools", returnTo: settingsReturn })}>本地工具中心</button>
        {projectId && (
          <details ref={settingsMoreRef} className="v3-settings-more" data-active={route.kind === "project-settings" && (route.section === "advanced-tasks" || route.section === "advanced-project") ? "true" : undefined}>
            <summary>更多 ▾</summary>
            <div className="v3-settings-more-menu">
              <button
                type="button"
                aria-current={route.kind === "project-settings" && route.section === "advanced-tasks" ? "page" : undefined}
                onClick={() => { if (settingsMoreRef.current) settingsMoreRef.current.open = false; navigate({ kind: "project-settings", projectId, section: "advanced-tasks" }); }}
              >
                任务审计与诊断
              </button>
              <button
                type="button"
                aria-current={route.kind === "project-settings" && route.section === "advanced-project" ? "page" : undefined}
                onClick={() => { if (settingsMoreRef.current) settingsMoreRef.current.open = false; navigate({ kind: "project-settings", projectId, section: "advanced-project" }); }}
              >
                外部交接包导入
              </button>
            </div>
          </details>
        )}
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
    {shortcutsOpen && <dialog ref={shortcutsDialog} open={typeof HTMLDialogElement.prototype.showModal !== "function" ? true : undefined} className="v3-shortcuts-dialog" aria-label="键盘快捷键速查" onCancel={e => { e.preventDefault(); setShortcutsOpen(false); }} onClick={e => { if (e.target === shortcutsDialog.current) setShortcutsOpen(false); }}>
      <div className="v3-shortcuts-box">
        <header className="v3-shortcuts-head">
          <h2>键盘快捷键速查</h2>
          <button type="button" onClick={() => setShortcutsOpen(false)}>关闭</button>
        </header>
        <div className="v3-shortcuts-grid">
          {SHORTCUT_GROUPS.map(group => (
            <section key={group.title} className="v3-shortcut-group">
              <h3>{group.title}</h3>
              {group.items.map(([keys, desc]) => (
                <div key={keys} className="v3-shortcut-row">
                  <span>{desc}</span>
                  <kbd className="v3-kbd">{keys}</kbd>
                </div>
              ))}
            </section>
          ))}
        </div>
      </div>
    </dialog>}
    {toasts.length > 0 && <div className="v3-toasts" aria-live="polite" aria-label="系统提示">
      {toasts.map(toast => <button key={toast.id} type="button" className={`v3-toast v3-toast-${toast.tone}`} onClick={() => setToasts(prev => prev.filter(t => t.id !== toast.id))}>{toast.message}</button>)}
    </div>}
  </div>;
}
