import { useEffect, useRef, useState } from "react";
import { useRunsController, type RunsProps } from "./RunsController";
import { RunDetail } from "./RunDetail";
import { coverageMessage, filters, runStatusLabel, runKey, sourceLabels } from "./runsModel";
import "./RunsPage.css";
export function RunsPage(props: RunsProps) {
  const c = useRunsController(props);
  const [runQuery, setRunQuery] = useState("");
  useEffect(() => { setRunQuery(""); }, [props.route.projectId]);
  const advancedRef = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const closeOutside = (e: MouseEvent) => {
      if (advancedRef.current?.open && e.target instanceof Node && !advancedRef.current.contains(e.target)) advancedRef.current.open = false;
    };
    const closeEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape" && advancedRef.current?.open) advancedRef.current.open = false;
    };
    document.addEventListener("mousedown", closeOutside);
    window.addEventListener("keydown", closeEsc);
    return () => { document.removeEventListener("mousedown", closeOutside); window.removeEventListener("keydown", closeEsc); };
  }, []);
  const q = runQuery.trim().toLowerCase();
  const visibleRuns = c.list?.items.filter(run => !q || `${run.title} ${runStatusLabel(run)} ${run.errorSummary ?? ""}`.toLowerCase().includes(q)) ?? [];
  return <div className="runs-page"><header className="runs-header"><h1>近期运行</h1><p>查看近期运行和仍在队列中的任务。</p><button type="button" onClick={() => props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-tasks" })}>查看完整任务历史</button><nav aria-label="运行筛选">{filters.map(filter => <button type="button" key={filter.key} aria-pressed={filter.key === c.filter} onClick={() => props.navigate({ ...props.route, filter: filter.key })}>{filter.label}</button>)}</nav><button type="button" onClick={() => void c.refresh()}>刷新</button><details ref={advancedRef} className="runs-advanced"><summary>高级</summary><div className="runs-advanced-menu"><button type="button" onClick={() => { if (advancedRef.current) advancedRef.current.open = false; props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-tasks" }); }}>任务审计与诊断</button><button type="button" onClick={() => { if (advancedRef.current) advancedRef.current.open = false; props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-project" }); }}>生产分析与导入</button></div></details></header>
    {props.route.context && <section aria-label="历史运行定位"><p>已打开相关的运行记录。审核仍需显式操作，不会自动更改结果或镜头选择。</p></section>}
    {c.error && <p role="alert">{c.error}</p>}{c.loading && <p role="status">正在读取运行…</p>}
    <div className="runs-content"><aside className="run-list" aria-label="运行列表">{c.list && c.list.items.length > 0 && <input type="search" className="run-list-search" aria-label="筛选运行记录" placeholder="按标题或错误筛选当前列表…" value={runQuery} onChange={e => setRunQuery(e.target.value)} />}{c.loading && !c.list && <div className="runs-skeleton-list" aria-hidden="true"><span className="runs-skeleton-card" /><span className="runs-skeleton-card" /><span className="runs-skeleton-card" /></div>}{visibleRuns.map(run => { const done = run.progress.succeeded + run.progress.failed + run.progress.cancelled; return <button type="button" key={runKey(run.ref)} aria-pressed={props.route.run && runKey(props.route.run) === runKey(run.ref)} onClick={() => props.navigate({ ...props.route, run: run.ref })}><strong>{run.title}</strong><span className="run-source">{sourceLabels[run.ref.source]}</span><span className={`run-status status-${run.status.toLowerCase()}`}>{runStatusLabel(run)}</span>{(run.progress.total > 1 || run.status === "RUNNING") && run.progress.total > 0 && <progress className="run-card-progress" max={run.progress.total} value={done} /> }<span>{run.resultsSummary.length} 个结果{run.progress.failed ? ` · ${run.progress.failed} 项失败` : ""}</span><time dateTime={run.updatedAt}>{new Date(run.updatedAt).toLocaleString("zh-CN")}</time></button>; })}{c.list && !c.list.items.length && <p>当前筛选没有运行。</p>}{c.list && c.list.items.length > 0 && !visibleRuns.length && <p>没有匹配搜索关键词的运行。</p>}<small data-coverage={c.list?.coverage ?? ""}>{coverageMessage(c.list?.coverage)}</small>{c.list?.nextCursor ? <p role="alert">服务端返回了意外游标；本页拒绝混合历史分页，请刷新。</p> : null}</aside><RunDetail {...props} controller={c} /></div>
  </div>;
}
