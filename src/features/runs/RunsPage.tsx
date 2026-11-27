import { useEffect, useRef, useState } from "react";
import { useRunsController, type RunsProps } from "./RunsController";
import { RunDetail } from "./RunDetail";
import { coverageMessage, filters, runStatusLabel, runKey, sourceLabels } from "./runsModel";
import "./RunsPage.css";

function formatCompactTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const now = new Date();
  const sameYear = date.getFullYear() === now.getFullYear();
  return date.toLocaleString("zh-CN", sameYear
    ? { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hour12: false }
    : { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hour12: false });
}

export function RunsPage(props: RunsProps) {
  const c = useRunsController(props);
  const [runQuery, setRunQuery] = useState("");
  useEffect(() => { setRunQuery(""); }, [props.route.projectId]);
  const advancedRef = useRef<HTMLDetailsElement>(null);
  const q = runQuery.trim().toLowerCase();
  const visibleRuns = c.list?.items.filter(run => !q || `${run.title} ${runStatusLabel(run)} ${run.errorSummary ?? ""}`.toLowerCase().includes(q)) ?? [];
  const visibleRunsRef = useRef(visibleRuns);
  visibleRunsRef.current = visibleRuns;
  const routeRef = useRef(props.route);
  routeRef.current = props.route;
  const navigateRef = useRef(props.navigate);
  navigateRef.current = props.navigate;

  useEffect(() => {
    const closeOutside = (e: MouseEvent) => {
      if (advancedRef.current?.open && e.target instanceof Node && !advancedRef.current.contains(e.target)) advancedRef.current.open = false;
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && advancedRef.current?.open) {
        advancedRef.current.open = false;
        return;
      }
      if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable || target.closest("dialog"))) return;
      const items = visibleRunsRef.current;
      if (!items.length) return;
      e.preventDefault();
      const current = routeRef.current.run ? runKey(routeRef.current.run) : undefined;
      const idx = current ? items.findIndex(item => runKey(item.ref) === current) : -1;
      const nextIdx = e.key === "ArrowDown"
        ? (idx < 0 ? 0 : Math.min(items.length - 1, idx + 1))
        : (idx < 0 ? items.length - 1 : Math.max(0, idx - 1));
      const next = items[nextIdx];
      if (next && runKey(next.ref) !== current) {
        navigateRef.current({ ...routeRef.current, run: next.ref });
      }
    };
    document.addEventListener("mousedown", closeOutside);
    window.addEventListener("keydown", onKeyDown);
    return () => { document.removeEventListener("mousedown", closeOutside); window.removeEventListener("keydown", onKeyDown); };
  }, []);

  return (
    <div className="runs-page">
      <header className="runs-header">
        <div className="runs-header-left">
          <h1>近期运行</h1>
          <nav aria-label="运行筛选">
            {filters.map(filter => (
              <button type="button" key={filter.key} aria-pressed={filter.key === c.filter} onClick={() => props.navigate({ ...props.route, filter: filter.key })}>
                {filter.label}
              </button>
            ))}
          </nav>
        </div>
        <div className="runs-header-actions">
          <button type="button" className="quiet-button" onClick={() => void c.refresh()} disabled={c.loading}>刷新</button>
          <button type="button" className="quiet-button" onClick={() => props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-tasks" })}>查看完整任务历史</button>
          <details ref={advancedRef} className="runs-advanced">
            <summary>高级</summary>
            <div className="runs-advanced-menu">
              <button type="button" onClick={() => { if (advancedRef.current) advancedRef.current.open = false; props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-tasks" }); }}>任务审计与诊断</button>
              <button type="button" onClick={() => { if (advancedRef.current) advancedRef.current.open = false; props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-project" }); }}>生产分析与导入</button>
            </div>
          </details>
        </div>
      </header>
      {props.route.context && <section className="runs-context-banner" aria-label="历史运行定位"><p>已打开相关的运行记录。审核仍需显式操作，不会自动更改结果或镜头选择。</p></section>}
      {c.error && <p className="runs-error-banner" role="alert">{c.error}</p>}
      {c.loading && !c.list && <p className="runs-loading-note" role="status">正在读取运行…</p>}
      <div className="runs-content">
        <aside className="run-list" aria-label="运行列表">
          {c.list && c.list.items.length > 0 && (
            <div className="run-list-search-bar">
              <input type="search" className="run-list-search" aria-label="筛选运行记录" placeholder="按标题、状态或错误筛选…" value={runQuery} onChange={e => setRunQuery(e.target.value)} />
              {q && <small className="run-list-search-count">{visibleRuns.length}/{c.list.items.length}</small>}
            </div>
          )}
          {c.loading && !c.list && (
            <div className="runs-skeleton-list" aria-hidden="true">
              <span className="runs-skeleton-card" />
              <span className="runs-skeleton-card" />
              <span className="runs-skeleton-card" />
            </div>
          )}
          {visibleRuns.map(run => {
            const done = run.progress.succeeded + run.progress.failed + run.progress.cancelled;
            const selected = Boolean(props.route.run && runKey(props.route.run) === runKey(run.ref));
            const fullTime = new Date(run.updatedAt).toLocaleString("zh-CN");
            return (
              <button
                type="button"
                key={runKey(run.ref)}
                aria-pressed={selected}
                onClick={() => props.navigate({ ...props.route, run: run.ref })}
              >
                <span className="run-card-head">
                  <strong title={run.title}>{run.title}</strong>
                  <span className={`run-status status-${run.status.toLowerCase()}`}>{runStatusLabel(run)}</span>
                </span>
                <span className="run-card-meta">
                  <span className="run-source">{sourceLabels[run.ref.source]}</span>
                  <span>{run.resultsSummary.length} 个结果{run.progress.failed ? ` · ${run.progress.failed} 项失败` : ""}</span>
                  <time dateTime={run.updatedAt} title={fullTime}>{formatCompactTime(run.updatedAt)}</time>
                </span>
                {(run.progress.total > 1 || run.status === "RUNNING") && run.progress.total > 0 && (
                  <progress className="run-card-progress" max={run.progress.total} value={done} />
                )}
                {run.errorSummary && <span className="run-card-error" title={run.errorSummary}>{run.errorSummary}</span>}
              </button>
            );
          })}
          {c.list && !c.list.items.length && <p className="runs-empty-note">当前筛选没有运行。</p>}
          {c.list && c.list.items.length > 0 && !visibleRuns.length && <p className="runs-empty-note">没有匹配搜索关键词的运行。</p>}
          <small className="run-list-coverage" data-coverage={c.list?.coverage ?? ""}>{coverageMessage(c.list?.coverage)}</small>
          {c.list?.nextCursor ? <p role="alert">服务端返回了意外游标；本页拒绝混合历史分页，请刷新。</p> : null}
        </aside>
        <RunDetail {...props} controller={c} />
      </div>
    </div>
  );
}
