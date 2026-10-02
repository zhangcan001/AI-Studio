import { useRunsController, type RunsProps } from "./RunsController";
import { RunDetail } from "./RunDetail";
import { filters, labels, runKey } from "./runsModel";
import "./RunsPage.css";
export function RunsPage(props: RunsProps) {
  const c = useRunsController(props);
  return <div className="runs-page"><header className="runs-header"><h1>运行</h1><nav aria-label="运行筛选">{filters.map(filter => <button type="button" key={filter.key} aria-pressed={filter.key === c.filter} onClick={() => props.navigate({ ...props.route, filter: filter.key })}>{filter.label}</button>)}</nav><button type="button" onClick={() => void c.refresh()}>刷新</button><details><summary>高级</summary>{[{ filter: "tasks", label: "历史任务" }, { filter: "production", label: "队列 / 生产" }, { filter: "review", label: "审核收件箱" }].map(item => <button type="button" key={item.filter} onClick={() => props.navigate({ ...props.route, filter: item.filter })}>{item.label}</button>)}</details></header>
    {c.error && <p role="alert">{c.error}</p>}{c.loading && <p role="status">正在读取运行…</p>}
    <div className="runs-content"><aside className="run-list" aria-label="运行列表">{c.list?.items.map(run => <button type="button" key={runKey(run.ref)} aria-pressed={props.route.run && runKey(props.route.run) === runKey(run.ref)} onClick={() => props.navigate({ ...props.route, run: run.ref })}><strong>{run.title}</strong><span className={`run-status status-${run.status.toLowerCase()}`}>{labels[run.status]}</span><span>{run.resultsSummary.length} 个结果 · {run.progress.failed} 项失败</span><time dateTime={run.updatedAt}>{new Date(run.updatedAt).toLocaleString("zh-CN")}</time></button>)}{c.list && !c.list.items.length && <p>当前筛选没有运行。</p>}<small>当前覆盖最近 50 个任务与生产运行，以及未归档队列；不提供完整历史分页。</small></aside><RunDetail {...props} controller={c} /></div>
  </div>;
}
