import type { RunsProps, RunsController } from "./RunsController";
import { friendlyGenerator, inputText, labels } from "./runsModel";
import { RunResults } from "./RunResults";
import { runRecoveryPresentation } from "./runRecoveryPresentation";
export function RunDetail(props: RunsProps & { controller: RunsController }) {
  const c = props.controller; const run = c.detail;
  if (!run) return <section className="run-detail"><p>{c.missing ? "运行已不存在或不可访问。请从列表选择其他运行。" : "选择一个运行查看详情。"}</p></section>;
  const recovery = runRecoveryPresentation(run);
  return <section className="run-detail" aria-label="运行详情"><header><h2>{run.title}</h2><span className={`run-status status-${run.status.toLowerCase()}`}>{labels[run.status]}</span></header>
    <section><h3>进度</h3><p>{run.progress.succeeded}/{run.progress.total} 已完成 · {run.progress.failed} 项失败 · {run.progress.cancelled} 项取消</p><progress max={Math.max(1, run.progress.total)} value={run.progress.succeeded + run.progress.failed + run.progress.cancelled} /></section>
    <div className="run-actions">{(["START", "PAUSE", "CANCEL"] as const).filter(action => run.availableActions.includes(action)).map(action => <button type="button" disabled={c.busy} key={action} onClick={() => void c.action(action)}>{action === "START" ? run.status === "PAUSED" ? "继续运行" : "启动运行" : action === "PAUSE" ? "暂停" : "取消未完成项"}</button>)}</div>
    {recovery.showRecovery && <section role="status" aria-label="运行恢复说明"><h3>{run.status === "PAUSED" ? "暂停与继续" : "失败与恢复"}</h3><p>{recovery.summary}</p>{run.errorSummary && <p>{run.errorSummary}</p>}{recovery.retry && <><p>可恢复失败项：{recovery.retry.count} 项</p><p>{recovery.retry.explanation}</p><button type="button" disabled={c.busy} onClick={() => void c.action("RETRY")}>{recovery.retry.label}</button></>}{recovery.needsInputEdit && <p>{recovery.inputEditExplanation}</p>}</section>}
    {(recovery.resultsPreserved || c.results.length > 0) && <p>已完成的结果会保留。重试失败项或编辑输入创建新运行，不会删除当前运行已经产生的结果。</p>}
    <section><h3>来源</h3>{run.detail?.sources.map(source => <button type="button" key={`${source.id}:${source.stage}`} onClick={() => props.navigate({ kind: "create", projectId: props.route.projectId, shotId: source.id, stage: source.stage === "video" ? "video" : "image" })}>{source.name} · {source.stage === "video" ? "视频" : "图片"}</button>)}{!run.detail?.sources.length && <p>历史独立运行，没有关联镜头。</p>}</section>
    <section><h3>输入摘要</h3>{run.detail?.inputs.map((input, index) => <article className="run-input" key={input.taskId ?? input.itemId}><h4>{friendlyGenerator(input.generatorName)} · 输入 {index + 1}</h4>{inputText(input).map(item => <p key={item.key}><strong>{item.label}：</strong>{item.value}</p>)}{input.errorMessage && <p>失败原因：{input.errorMessage}</p>}<button type="button" disabled={c.busy || !input.selectionRef} onClick={() => void c.reuse(input)}>编辑这些输入并创建新运行</button><p>会打开 Create，并载入这次运行可安全恢复的历史输入。只有再次确认生成后，才会创建新的运行；不会覆盖原运行。</p>{input.reuseUnavailableReason && <p>{input.reuseUnavailableReason}</p>}</article>)}{!run.detail?.inputs.length && <p>尚未创建任务；启动前准入会重新检查输入。</p>}</section>
    <RunResults {...props} />
    <details><summary>高级技术详情</summary>{[...new Set([...(run.ref.source === "task" ? [run.ref.id] : []), ...(run.detail?.inputs.flatMap(input => input.taskId ? [input.taskId] : []) ?? [])])].map((id, index) => <button type="button" key={id} onClick={() => props.navigate({ kind: "system-settings", section: "diagnostics", returnTo: { kind: "runs", projectId: props.route.projectId, run: { source: "task", id } } })}>技术诊断 · 任务 {index + 1}</button>)}<pre>{JSON.stringify({ ref: run.ref, phase: run.phase, parent: run.preferredParent, inputs: run.detail?.inputs.map(input => ({ taskId: input.taskId, selectionRef: input.selectionRef })) }, null, 2)}</pre></details>
  </section>;
}
