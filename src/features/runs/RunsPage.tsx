import { useRunsController, type RunsProps } from "./RunsController";
import { RunDetail } from "./RunDetail";
import { filters, labels, runKey } from "./runsModel";
import "./RunsPage.css";
export function RunsPage(props: RunsProps) {
  const c = useRunsController(props);
  return <div className="runs-page"><header className="runs-header"><h1>近期运行</h1><p>此处用于查看近期运行和仍在队列中的项目。</p><button type="button" onClick={() => props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-tasks" })}>查看完整任务历史</button><nav aria-label="运行筛选">{filters.map(filter => <button type="button" key={filter.key} aria-pressed={filter.key === c.filter} onClick={() => props.navigate({ ...props.route, filter: filter.key })}>{filter.label}</button>)}</nav><button type="button" onClick={() => void c.refresh()}>刷新</button><details><summary>高级</summary><button type="button" onClick={() => props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-tasks" })}>任务审计与诊断</button><button type="button" onClick={() => props.navigate({ kind: "project-settings", projectId: props.route.projectId, section: "advanced-project" })}>生产分析与导入</button></details></header>
    {props.route.context && <section aria-label="历史运行定位"><p>已通过旧链接打开统一运行页面，原镜头、批次及审核上下文已保留。审核仍需显式操作，不会自动更改结果或镜头选择。</p><details><summary>原始定位信息</summary><pre>{JSON.stringify(props.route.context, null, 2)}</pre></details></section>}
    {c.error && <p role="alert">{c.error}</p>}{c.loading && <p role="status">正在读取运行…</p>}
    <div className="runs-content"><aside className="run-list" aria-label="运行列表">{c.list?.items.map(run => <button type="button" key={runKey(run.ref)} aria-pressed={props.route.run && runKey(props.route.run) === runKey(run.ref)} onClick={() => props.navigate({ ...props.route, run: run.ref })}><strong>{run.title}</strong><span className={`run-status status-${run.status.toLowerCase()}`}>{labels[run.status]}</span><span>{run.resultsSummary.length} 个结果 · {run.progress.failed} 项失败</span><time dateTime={run.updatedAt}>{new Date(run.updatedAt).toLocaleString("zh-CN")}</time></button>)}{c.list && !c.list.items.length && <p>当前筛选没有运行。</p>}<small>当前覆盖最近 50 个任务与生产运行，以及未归档队列；更早的任务记录请查看完整任务历史；此处不提供完整混合运行历史分页。</small></aside><RunDetail {...props} controller={c} /></div>
  </div>;
}
